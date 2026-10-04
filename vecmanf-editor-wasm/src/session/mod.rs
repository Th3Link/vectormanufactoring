//! One open document's editing session: the plain-Rust orchestration
//! this crate's `wasm-bindgen` surface is a thin shell around.
//!
//! Deliberately free of any `wasm-bindgen`/`web-sys`/GPU type so it can
//! be exercised by ordinary `cargo test` — "no editing logic of its own"
//! (ADR 0001 §3) describes the `wasm-bindgen` boundary in `lib.rs`, not
//! this module: the logic itself is `vecmanf-document-core`'s,
//! `vecmanf-ui-core`'s and `vecmanf-render-core`'s, and this module's own
//! job is exactly "which tool is active, and where do its inputs and the
//! view transform come from" — nothing a browser is required to answer.
//!
//! `primitive-shapes` adds three shape tools (Rectangle, Ellipse,
//! Polygon/Star) alongside Pen and Node, plus "object to path"
//! (acceptance criteria 1-22).

use vecmanf_document_core::{
    AnchorKind, Document, InnerRatio, Length, NewAnchor, NodeId, OpenError, Point, PointCount,
    PrimitiveSnapshot, SaveError, Shape, Tolerance, ViewTransform, outline_of,
};
use vecmanf_render_core::{
    DecorationInput, DrawList, Hovered as RenderHovered, RenderShapeHandle, ShapeDecorationInput,
    ShapeHandleKind, build_draw_list, build_pen_preview, build_shape_draw_list,
};
use vecmanf_ui_core::{
    AnchorIdMinter, EllipseTool, HandleKind, Hit, HitTolerances, NodeTool, NodeToolbarState,
    PenTool, PolyStarMode, PolygonStarTool, PrimitiveSelection, RectangleTool, ShapeHitTolerances,
    hit_test, hit_test_primitive,
};

/// 8px node/handle hit-test radius (`docs/design-system.md`).
const POINT_TOLERANCE_PX: f64 = 8.0;
/// 4px segment hit-test tolerance (`docs/design-system.md`).
const SEGMENT_TOLERANCE_PX: f64 = 4.0;
/// How far (screen pixels) a pen-tool press must move before it counts
/// as a drag rather than a plain click (acceptance criteria 1 vs 2). Not
/// itself a named design-system token; a small, deliberately generous
/// value so an imprecise click is never misread as a drag.
const PEN_DRAG_THRESHOLD_PX: f64 = 3.0;

/// Which tool is active. Exactly one at a time — `specification.md`'s
/// tool rail has five buttons (Pen, Node, Rectangle, Ellipse,
/// Polygon/Star), and switching tools is a single active-tool state,
/// not independent flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// The pen tool (acceptance criteria 1-5, `path-node-editing`).
    Pen,
    /// The node tool (acceptance criteria 7-14, `path-node-editing`).
    Node,
    /// The rectangle tool (acceptance criteria 1-6).
    Rectangle,
    /// The ellipse/circle tool (acceptance criteria 7-9).
    Ellipse,
    /// The polygon/star tool (acceptance criteria 10-15).
    PolygonStar,
}

/// One open document's whole editing session.
pub struct Session {
    document: Document,
    minter: AnchorIdMinter,
    pen: PenTool,
    node: NodeTool,
    rectangle: RectangleTool,
    ellipse: EllipseTool,
    poly_star: PolygonStarTool,
    /// Shared across the three shape tools (acceptance criterion 22:
    /// two or more primitives can be selected together).
    primitive_selection: PrimitiveSelection,
    tool: Tool,
    view: ViewTransform,
    hovered: Option<Hit>,
    /// The primitive currently hovered while a shape tool is active —
    /// the shape-tool counterpart to `hovered` above.
    hovered_primitive: Option<NodeId>,
    /// The live pointer position in document space, tracked regardless
    /// of the active tool — the pen tool's rubber-band preview
    /// (`specification.md`'s UX notes) needs it even though it keeps no
    /// hit-test hover state of its own (see [`Session::pointer_hover`]).
    /// `None` before the first move, or once [`Session::pointer_leave`]
    /// says the pointer is off the canvas.
    pointer_position: Option<Point>,
}

impl Session {
    /// A brand-new, empty document (File → New), bound to `peer`.
    #[must_use]
    pub fn new(peer: u64) -> Self {
        Self {
            document: Document::new(peer),
            minter: AnchorIdMinter::new(peer),
            pen: PenTool::new(),
            node: NodeTool::new(),
            rectangle: RectangleTool::new(),
            ellipse: EllipseTool::new(),
            poly_star: PolygonStarTool::new(),
            primitive_selection: PrimitiveSelection::new(),
            // Pen is the default tool on an empty canvas
            // (`specification.md`'s UX notes: "there's nothing to select
            // or edit yet, and Pen is what lets the maker start
            // immediately"). None of the three new shape tools change
            // this default (`specs/primitive-shapes/specification.md`'s
            // own UX notes).
            tool: Tool::Pen,
            view: ViewTransform::identity(),
            hovered: None,
            hovered_primitive: None,
            pointer_position: None,
        }
    }

    /// Reopens a previously saved `.vmf` container's bytes (File → Open),
    /// bound to `peer`.
    ///
    /// # Errors
    /// See [`vecmanf_document_core::unpack`].
    pub fn open(peer: u64, bytes: &[u8]) -> Result<Self, OpenError> {
        let document = vecmanf_document_core::unpack(peer, bytes)?;
        Ok(Self {
            document,
            minter: AnchorIdMinter::new(peer),
            pen: PenTool::new(),
            node: NodeTool::new(),
            rectangle: RectangleTool::new(),
            ellipse: EllipseTool::new(),
            poly_star: PolygonStarTool::new(),
            primitive_selection: PrimitiveSelection::new(),
            tool: Tool::Node,
            view: ViewTransform::identity(),
            hovered: None,
            hovered_primitive: None,
            pointer_position: None,
        })
    }

    /// Packs the current document into `.vmf` container bytes (File →
    /// Save/Save As).
    ///
    /// # Errors
    /// See [`vecmanf_document_core::pack`].
    pub fn pack(&self, app_version: &str) -> Result<Vec<u8>, SaveError> {
        vecmanf_document_core::pack(&self.document, app_version)
    }

    /// The active tool.
    #[must_use]
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    /// Switches the active tool (`B`/`N`/`R`/`E`/`*` canvas-focus
    /// shortcuts, or the tool rail). Switching away from the pen tool
    /// mid-path does **not** discard it — only Escape or finishing does
    /// (`specification.md`'s pen tool is not itself scoped to stay
    /// active just because another tool was clicked; no acceptance
    /// criterion covers this edge, so the safer, less surprising choice
    /// — not silently losing work — is kept). Likewise, switching away
    /// from a shape tool mid-drag does not discard that drag either —
    /// no acceptance criterion exercises switching tools mid-drag, so
    /// the same conservative stance applies.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    /// Updates the view transform (pan/zoom).
    pub fn set_view(&mut self, view: ViewTransform) {
        self.view = view;
    }

    /// The current view transform, for the host's GPU layer to build
    /// this frame's screen transform from.
    #[must_use]
    pub const fn view(&self) -> ViewTransform {
        self.view
    }

    fn point_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(POINT_TOLERANCE_PX / self.view.scale())
    }

    fn segment_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(SEGMENT_TOLERANCE_PX / self.view.scale())
    }

    fn hit_tolerances(&self) -> HitTolerances {
        HitTolerances {
            point: self.point_tolerance(),
            segment: self.segment_tolerance(),
        }
    }

    /// The same two tolerances, reused for every shape tool
    /// (`specs/primitive-shapes/specification.md`: "reuses that slice's
    ///... hit-testing tolerances").
    fn shape_tolerances(&self) -> ShapeHitTolerances {
        ShapeHitTolerances {
            outline: self.segment_tolerance(),
            handle: self.point_tolerance(),
        }
    }

    fn paths(&self) -> Vec<vecmanf_document_core::PathSnapshot> {
        self.document
            .path_ids()
            .into_iter()
            .filter_map(|id| self.document.path(id))
            .collect()
    }

    /// Every primitive currently in the document, in z-order — the
    /// shape-tool counterpart to [`Session::paths`].
    fn primitives(&self) -> Vec<PrimitiveSnapshot> {
        self.document
            .path_ids()
            .into_iter()
            .filter_map(|id| self.document.primitive(id))
            .collect()
    }

    /// Whether `shape` is the kind the currently active shape tool
    /// creates/edits — the "tool mismatch" rule
    /// (`specification.md`'s "Selection and hover convention for
    /// primitives": a primitive's selection visuals only render while
    /// its own matching tool is active).
    const fn shape_matches_active_tool(&self, shape: &Shape) -> bool {
        matches!(
            (self.tool, shape),
            (Tool::Rectangle, Shape::Rect { .. })
                | (Tool::Ellipse, Shape::Ellipse { .. })
                | (
                    Tool::PolygonStar,
                    Shape::Polygon { .. } | Shape::Star { .. }
                )
        )
    }

    /// The pointer went down at `point` (document space).
    pub fn pointer_down(&mut self, point: Point, shift: bool) {
        match self.tool {
            Tool::Pen => {
                let tolerance = self.point_tolerance_as_length();
                self.pen.pointer_down(point, tolerance);
            }
            Tool::Node => {
                let paths = self.paths();
                let tolerances = self.hit_tolerances();
                self.node.pointer_down(&paths, point, tolerances, shift);
            }
            Tool::Rectangle => {
                let primitives = self.primitives();
                let tolerances = self.shape_tolerances();
                self.rectangle.pointer_down(
                    &primitives,
                    &mut self.primitive_selection,
                    point,
                    tolerances,
                    shift,
                );
            }
            Tool::Ellipse => {
                let primitives = self.primitives();
                let tolerances = self.shape_tolerances();
                self.ellipse.pointer_down(
                    &primitives,
                    &mut self.primitive_selection,
                    point,
                    tolerances,
                    shift,
                );
            }
            Tool::PolygonStar => {
                let primitives = self.primitives();
                let tolerances = self.shape_tolerances();
                self.poly_star.pointer_down(
                    &primitives,
                    &mut self.primitive_selection,
                    point,
                    tolerances,
                    shift,
                );
            }
        }
    }

    fn point_tolerance_as_length(&self) -> Length {
        Length::from_mm(self.point_tolerance().as_mm())
    }

    /// The pointer moved to `point` with no button held. Always records
    /// `point` as the live cursor position — [`Session::draw_list`]'s
    /// pen-tool rubber-band preview needs it (`specification.md`'s UX
    /// notes) even though the pen tool keeps no hit-test hover state of
    /// its own. For the node tool, additionally updates hover state for
    /// the hover ring (same UX notes); for a shape tool, updates the
    /// hovered primitive for its bounding-box hover outline.
    pub fn pointer_hover(&mut self, point: Point) {
        self.pointer_position = Some(point);
        self.hovered = None;
        self.hovered_primitive = None;
        match self.tool {
            Tool::Node => {
                let paths = self.paths();
                self.hovered = hit_test(
                    &paths,
                    self.node.selection(),
                    point,
                    self.point_tolerance(),
                    self.segment_tolerance(),
                );
            }
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                let primitives: Vec<PrimitiveSnapshot> = self
                    .primitives()
                    .into_iter()
                    .filter(|p| self.shape_matches_active_tool(&p.shape))
                    .collect();
                self.hovered_primitive =
                    hit_test_primitive(&primitives, point, self.segment_tolerance());
            }
            Tool::Pen => {}
        }
    }

    /// The pointer left the canvas entirely — clears the live cursor
    /// position so the pen tool's rubber-band preview disappears rather
    /// than sticking at the last position inside the canvas.
    pub fn pointer_leave(&mut self) {
        self.pointer_position = None;
        self.hovered = None;
        self.hovered_primitive = None;
    }

    /// The pointer released at `point`, ending whatever gesture
    /// [`Session::pointer_down`] began. `constrain` is the Ctrl
    /// modifier's state at release — consulted only by the rectangle and
    /// ellipse tools (acceptance criteria 2, 8).
    pub fn pointer_up(&mut self, point: Point, constrain: bool) {
        match self.tool {
            Tool::Pen => {
                let threshold = self.drag_threshold();
                self.pen
                    .pointer_up(&mut self.minter, &self.document, point, threshold);
            }
            Tool::Node => {
                self.node.pointer_up(&self.document, point);
            }
            Tool::Rectangle => {
                self.rectangle.pointer_up(&self.document, point, constrain);
            }
            Tool::Ellipse => {
                self.ellipse.pointer_up(&self.document, point, constrain);
            }
            Tool::PolygonStar => {
                self.poly_star.pointer_up(&self.document, point);
            }
        }
    }

    fn drag_threshold(&self) -> Length {
        Length::from_mm(PEN_DRAG_THRESHOLD_PX / self.view.scale())
    }

    /// Acceptance criterion 3 / the dedicated "finish path" action
    /// (Enter, or a double-click the host has already recognized).
    pub fn finish_pen(&mut self) {
        self.pen.finish(&self.document);
    }

    /// Escape: discards the in-progress pen path (acceptance criterion
    /// 4), clears the node tool's selection, or cancels whichever shape
    /// tool's in-progress drag, depending on the active tool — never
    /// more than one, matching `specification.md`'s own rule that
    /// Escape only ever touches the active tool's own state.
    pub fn escape(&mut self) {
        match self.tool {
            Tool::Pen => {
                self.pen.escape();
            }
            Tool::Node => {
                self.node.escape();
            }
            Tool::Rectangle => {
                self.rectangle.escape();
            }
            Tool::Ellipse => {
                self.ellipse.escape();
            }
            Tool::PolygonStar => {
                self.poly_star.escape();
            }
        }
    }

    /// Acceptance criterion 13 (Delete/Backspace, or the contextual
    /// toolbar's Delete button). A no-op for the pen tool.
    pub fn delete_selected(&mut self) {
        if self.tool == Tool::Node {
            self.node.delete_selected(&self.document);
        }
    }

    /// Acceptance criterion 11 (the contextual toolbar's convert
    /// buttons). A no-op for the pen tool.
    pub fn convert_selected(&mut self, kind: AnchorKind) {
        if self.tool == Tool::Node {
            self.node.convert_selected(&self.document, kind);
        }
    }

    /// Acceptance criterion 14's "make line" (the contextual toolbar).
    pub fn make_line(&mut self) {
        if self.tool == Tool::Node {
            self.node.make_line(&self.document);
        }
    }

    /// Acceptance criterion 14's "make curve" (the contextual toolbar).
    pub fn make_curve(&mut self) {
        if self.tool == Tool::Node {
            self.node.make_curve(&self.document);
        }
    }

    /// Acceptance criterion 12: a double-click (already recognized by
    /// the host) at `point`.
    pub fn insert_at(&mut self, point: Point) {
        if self.tool == Tool::Node {
            let paths = self.paths();
            let tolerances = self.hit_tolerances();
            self.node
                .insert_at(&mut self.minter, &self.document, &paths, point, tolerances);
        }
    }

    /// The contextual toolbar's "Insert node" action: splits the
    /// currently selected segment at its midpoint. A no-op outside the
    /// node tool or without a segment selected.
    pub fn insert_selected(&mut self) {
        if self.tool == Tool::Node {
            let paths = self.paths();
            self.node
                .insert_on_selected_segment(&mut self.minter, &self.document, &paths);
        }
    }

    /// Which contextual-toolbar actions apply right now. Everything is
    /// `false` when the node tool isn't active, since the toolbar itself
    /// is only shown then.
    #[must_use]
    pub fn node_toolbar_state(&self) -> NodeToolbarState {
        if self.tool != Tool::Node {
            return NodeToolbarState::default();
        }
        self.node.toolbar_state(&self.document)
    }

    /// Acceptance criterion 6's "remove rounding" action: zeroes the
    /// corner radius of every currently selected rectangle. A no-op
    /// outside the rectangle tool.
    pub fn remove_corner_rounding(&mut self) {
        if self.tool == Tool::Rectangle {
            self.rectangle
                .remove_rounding(&self.document, &self.primitive_selection);
        }
    }

    /// The polygon/star tool-options bar's current mode.
    #[must_use]
    pub fn poly_star_mode(&self) -> PolyStarMode {
        self.poly_star.mode()
    }

    /// The polygon/star tool-options bar's current point count.
    #[must_use]
    pub fn poly_star_point_count(&self) -> PointCount {
        self.poly_star.point_count()
    }

    /// The polygon/star tool-options bar's current ratio.
    #[must_use]
    pub fn poly_star_ratio(&self) -> InnerRatio {
        self.poly_star.ratio()
    }

    /// The mode toggle (acceptance criteria 11 vs. 12): only affects
    /// shapes drawn after the switch.
    pub fn set_poly_star_mode(&mut self, mode: PolyStarMode) {
        self.poly_star.set_mode(mode);
    }

    /// The point-count stepper (acceptance criteria 10, 15).
    pub fn set_poly_star_point_count(&mut self, count: PointCount) {
        self.poly_star
            .set_point_count(count, &self.document, &self.primitive_selection);
    }

    /// The ratio field/slider (acceptance criteria 12, 14).
    pub fn set_poly_star_ratio(&mut self, ratio: InnerRatio) {
        self.poly_star
            .set_ratio(ratio, &self.document, &self.primitive_selection);
    }

    /// "Object to path" (acceptance criteria 17, 21, 22): converts every
    /// currently selected primitive to a path, in one
    /// [`vecmanf_document_core::Document::convert_to_paths`] call, then
    /// switches to the node tool. The shape handles/tool-options bar
    /// disappear outright because the object is no longer a primitive
    /// at all (`specification.md`'s "Primitive vs. path: handles don't
    /// coexist") — nothing re-renders them since
    /// this crate's own shape-decoration input builder only ever looks at
    /// primitives.
    ///
    /// When exactly one primitive was selected, every one of its new
    /// anchors is selected in the node tool afterward, so it reads as
    /// "immediately editable" (acceptance criterion 17). A multi-object
    /// conversion (acceptance criterion 22) selects the first converted
    /// path's anchors the same way — `vecmanf-ui-core`'s
    /// [`vecmanf_ui_core::NodeSelection`] has no representation for
    /// "these anchors across several different paths are selected
    /// together", so a multi-object conversion's node-tool selection
    /// after the fact is an approximation of AC22's "remain selected
    /// together" wording, not a literal one; see this crate's own
    /// report for the open point.
    pub fn convert_selected_to_paths(&mut self) {
        let ids = self.primitive_selection.ids().to_vec();
        if ids.is_empty() {
            return;
        }
        let mut conversions = Vec::with_capacity(ids.len());
        for &id in &ids {
            let Some(primitive) = self.document.primitive(id) else {
                continue;
            };
            let outline = outline_of(&primitive.shape);
            let anchors: Vec<NewAnchor> = outline
                .into_iter()
                .map(|anchor| NewAnchor {
                    id: self.minter.mint(),
                    point: anchor.point,
                    handle_in: anchor.handle_in,
                    handle_out: anchor.handle_out,
                    kind: anchor.kind,
                })
                .collect();
            conversions.push((id, anchors));
        }
        if conversions.is_empty() {
            return;
        }
        let first_converted = conversions[0].0;
        if self.document.convert_to_paths(&conversions).is_ok() {
            self.primitive_selection.remove_all(&ids);
            self.tool = Tool::Node;
            if let Some(path) = self.document.path(first_converted) {
                self.node.select_all_anchors(&path);
            }
        }
    }

    /// The in-progress pen path's placed nodes, for the host's
    /// rubber-band/live-curve preview — `None` when idle or the node
    /// tool is active.
    #[must_use]
    pub fn pen_in_progress(&self) -> Option<&[vecmanf_document_core::NewAnchor]> {
        if self.tool == Tool::Pen {
            self.pen.in_progress_nodes()
        } else {
            None
        }
    }

    /// Acceptance criterion 5's cursor cue (`specification.md`'s
    /// "Cursors": "cursor swaps to a pen-with-small-circle... variant"):
    /// whether the live cursor is currently over the in-progress pen
    /// path's own close target. The host uses this to pick the cursor
    /// class; `false` outside the pen tool, with no path in progress, or
    /// before the pointer has ever moved over the canvas.
    #[must_use]
    pub fn is_hovering_pen_close_target(&self) -> bool {
        if self.tool != Tool::Pen {
            return false;
        }
        let Some(point) = self.pointer_position else {
            return false;
        };
        self.pen
            .is_hovering_close_target(point, self.point_tolerance_as_length())
    }

    fn decoration_input(&self) -> DecorationInput {
        if self.tool != Tool::Node {
            return DecorationInput::default();
        }
        let selection = self.node.selection();
        let selected_nodes = selection
            .nodes()
            .iter()
            .filter_map(|&id| selection.path().map(|path| (path, id)))
            .collect();
        let selected_segment = selection
            .segment()
            .and_then(|(start, end)| selection.path().map(|path| (path, start, end)));
        let hovered = self.hovered.and_then(|hit| match hit {
            Hit::Node { path, anchor } => Some(RenderHovered::Node(path, anchor)),
            Hit::Handle { path, anchor, slot } => Some(RenderHovered::Handle(path, anchor, slot)),
            Hit::Segment { .. } => None,
        });
        DecorationInput {
            selected_nodes,
            selected_segment,
            hovered,
        }
    }

    /// Builds this frame's primitive-shape decoration input
    /// (`specs/primitive-shapes/specification.md`'s "Tool mismatch"
    /// rule): empty unless a shape tool is active, and even then only
    /// for primitives of that tool's own kind.
    ///
    /// The "filled solid while being dragged" handle-glyph state
    /// (`docs/design-system.md`) is a flagged simplification: every
    /// handle here always reports `dragging: false`. Wiring a live
    /// per-handle "currently being dragged" flag through from each
    /// shape tool's own (private) drag state would need a new public
    /// accessor on each tool returning which exact handle (kind *and*,
    /// for a resize handle, which of the eight directions) is in
    /// flight; nothing in acceptance criteria 1-22 pins that visual
    /// detail, so it is left for a follow-up rather than guessed at
    /// here.
    fn shape_decoration_input(&self) -> ShapeDecorationInput {
        if !matches!(
            self.tool,
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar
        ) {
            return ShapeDecorationInput::default();
        }
        let primitives = self.primitives();
        let selected: Vec<NodeId> = self
            .primitive_selection
            .ids()
            .iter()
            .copied()
            .filter(|id| {
                primitives
                    .iter()
                    .find(|p| p.id == *id)
                    .is_some_and(|p| self.shape_matches_active_tool(&p.shape))
            })
            .collect();
        let handles = selected
            .iter()
            .filter_map(|&id| {
                let snapshot = primitives.iter().find(|p| p.id == id)?;
                let rendered: Vec<RenderShapeHandle> = vecmanf_ui_core::handles_for(snapshot)
                    .into_iter()
                    .map(|h| RenderShapeHandle {
                        kind: match h.kind {
                            HandleKind::Resize(_) => ShapeHandleKind::Resize,
                            HandleKind::CornerRadius => ShapeHandleKind::CornerRadius,
                            HandleKind::CornerRadiusEcho => ShapeHandleKind::CornerRadiusEcho,
                            HandleKind::InnerRadius => ShapeHandleKind::InnerRadius,
                        },
                        position: h.position,
                        draggable: h.draggable,
                        dragging: false,
                    })
                    .collect();
                Some((id, rendered))
            })
            .collect();
        let hovered = self.hovered_primitive.filter(|&id| {
            primitives
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| self.shape_matches_active_tool(&p.shape))
        });
        ShapeDecorationInput {
            selected,
            hovered,
            handles,
        }
    }

    /// Builds this frame's draw list from the document's current state,
    /// the active view transform, and the node tool's selection/hover —
    /// plus the pen tool's in-progress preview
    /// (`specification.md`'s UX notes) when it is active, and every
    /// primitive's own stroke/selection/handle decorations
    /// (`specs/primitive-shapes/specification.md`).
    #[must_use]
    pub fn draw_list(&self) -> DrawList {
        let paths = self.paths();
        let mut list = build_draw_list(&paths, self.view, &self.decoration_input());
        let primitives = self.primitives();
        list.extend(build_shape_draw_list(
            &primitives,
            self.view,
            &self.shape_decoration_input(),
        ));
        if self.tool == Tool::Pen
            && let Some(nodes) = self.pen.in_progress_nodes()
        {
            list.extend(build_pen_preview(
                nodes,
                self.pointer_position,
                self.view,
                self.is_hovering_pen_close_target(),
            ));
        }
        list
    }
}

/// Returns the one-sentence message the frontend's `ErrorDialog` shows
/// for `error` — moved here from `vecmanf-app`'s native `open_error.rs`
/// (`specs/project-file-foundation/specification.md`, "Error handling —
/// invalid/corrupt file") now that [`Session::open`] (and the
/// `Document::open` it wraps) only ever runs inside this wasm session,
/// never natively (`specs/path-node-editing/adrs.md`'s PR review: "the
/// host does byte I/O only"). Plain Rust, not `wasm_api`'s `wasm32`-only
/// shell, so it stays exercised by ordinary `cargo test` — its only
/// caller is `wasm_api::WasmSession::open`, which is itself `wasm32`-
/// only, so this function is `cfg`-gated the same way plus `test`
/// (otherwise a host `cargo build`/`clippy` sees it as genuinely unused
/// dead code, since its one caller does not exist in that build).
#[cfg(any(test, target_arch = "wasm32"))]
#[must_use]
pub const fn map_open_error(error: &OpenError) -> &'static str {
    match error {
        OpenError::NotAVmf => "This file isn't a vecmanf project (.vmf) file.",
        OpenError::Damaged => "This file is damaged and can't be read.",
        OpenError::FormatTooNew { .. } => {
            "This file was saved by a newer version of vecmanf. Update the app to open it."
        }
    }
}

#[cfg(test)]
mod map_open_error_tests {
    use super::map_open_error;
    use vecmanf_document_core::OpenError;

    #[test]
    fn not_a_vmf_names_the_specific_cause() {
        assert_eq!(
            map_open_error(&OpenError::NotAVmf),
            "This file isn't a vecmanf project (.vmf) file."
        );
    }

    #[test]
    fn damaged_names_the_specific_cause() {
        assert_eq!(
            map_open_error(&OpenError::Damaged),
            "This file is damaged and can't be read."
        );
    }

    #[test]
    fn format_too_new_names_the_specific_cause() {
        let error = OpenError::FormatTooNew {
            found: 2,
            supported: 1,
        };
        assert_eq!(
            map_open_error(&error),
            "This file was saved by a newer version of vecmanf. Update the app to open it."
        );
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::AnchorKind;

    use super::*;

    #[test]
    fn a_new_session_defaults_to_the_pen_tool_on_an_empty_document() {
        let session = Session::new(1);
        assert_eq!(session.tool(), Tool::Pen);
        assert_eq!(session.document.path_ids(), Vec::new());
    }

    #[test]
    fn drawing_an_open_path_with_the_pen_tool_then_reading_it_back() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        let paths = session.paths();
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].closed);
    }

    #[test]
    fn escape_discards_the_in_progress_pen_path_only() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.escape();
        assert_eq!(session.document.path_ids(), Vec::new());
    }

    #[test]
    fn switching_to_the_node_tool_selects_and_edits_a_finished_path() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(5.0, 5.0), false);

        let paths = session.paths();
        assert_eq!(paths[0].anchors[0].point, Point::new(5.0, 5.0));
    }

    #[test]
    fn convert_and_delete_dispatch_only_when_the_node_tool_is_active() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        // Pen tool is still active: these are no-ops.
        session.convert_selected(AnchorKind::Smooth);
        session.delete_selected();
        assert_eq!(session.paths()[0].anchors.len(), 2);

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.convert_selected(AnchorKind::Smooth);
        assert_eq!(session.paths()[0].anchors[0].kind, AnchorKind::Smooth);
    }

    #[test]
    fn draw_list_is_empty_for_a_brand_new_document() {
        let session = Session::new(1);
        assert!(session.draw_list().triangles.is_empty());
    }

    #[test]
    fn draw_list_includes_geometry_once_a_path_exists() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();
        assert!(!session.draw_list().triangles.is_empty());
    }

    /// The pen tool's in-progress preview (not yet committed) also shows
    /// up in `draw_list`, and tracks the live pointer position via
    /// `pointer_hover` for its rubber-band line.
    #[test]
    fn draw_list_includes_the_pen_tools_in_progress_preview() {
        let mut session = Session::new(1);
        let empty = session.draw_list().triangle_count();

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        let one_node = session.draw_list().triangle_count();
        assert!(one_node > empty, "the placed node's glyph/hover ring draw");

        session.pointer_hover(Point::new(10.0, 0.0));
        let with_rubber_band = session.draw_list().triangle_count();
        assert!(
            with_rubber_band > one_node,
            "the rubber-band line to the cursor adds geometry"
        );

        session.pointer_leave();
        let after_leave = session.draw_list().triangle_count();
        assert_eq!(after_leave, one_node, "no cursor, no rubber-band line");
    }

    /// Acceptance criterion 5's cursor cue: hovering near the
    /// in-progress path's own first node, with enough nodes placed,
    /// reports the close target; the node tool, idle pen tool, and
    /// hovering elsewhere all report `false`.
    #[test]
    fn is_hovering_pen_close_target_matches_the_real_close_decision() {
        let mut session = Session::new(1);
        assert!(!session.is_hovering_pen_close_target(), "idle: no path yet");

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);

        session.pointer_hover(Point::new(0.1, 0.1));
        assert!(session.is_hovering_pen_close_target());

        session.pointer_hover(Point::new(10.0, 0.0));
        assert!(
            !session.is_hovering_pen_close_target(),
            "near the last node, not the first"
        );

        session.set_tool(Tool::Node);
        session.pointer_hover(Point::new(0.1, 0.1));
        assert!(
            !session.is_hovering_pen_close_target(),
            "the node tool never shows a pen cursor"
        );
    }

    #[test]
    fn pack_then_open_round_trips_a_drawn_path() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        let bytes = session.pack("0.1.0").expect("pack");
        let reopened = Session::open(2, &bytes).expect("open");
        assert_eq!(reopened.paths().len(), 1);
    }

    /// Nothing selected, pen tool active: every toolbar action disabled.
    #[test]
    fn node_toolbar_state_is_all_false_outside_the_node_tool() {
        let session = Session::new(1);
        assert_eq!(session.node_toolbar_state(), NodeToolbarState::default());
    }

    /// A selected node enables delete/convert but not insert/make-line/
    /// make-curve (those need a segment selection).
    #[test]
    fn node_toolbar_state_for_a_selected_node() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);

        let state = session.node_toolbar_state();
        assert!(state.can_delete);
        assert!(state.can_convert_to_corner);
        assert!(state.can_convert_to_smooth);
        assert!(!state.can_insert);
        assert!(!state.can_make_line);
        assert!(!state.can_make_curve);
    }

    /// A selected line segment enables insert and make-curve, not
    /// make-line; `insert_selected` then splits it and clears the
    /// selection.
    #[test]
    fn node_toolbar_state_and_insert_selected_for_a_line_segment() {
        // A long segment: its midpoint sits well outside the 8px/mm
        // point-hit tolerance around either endpoint, so the click below
        // lands on the segment itself rather than being read as a node
        // hit of whichever endpoint happens to be nearest.
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(100.0, 0.0), false);
        session.pointer_up(Point::new(100.0, 0.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false);

        let state = session.node_toolbar_state();
        assert!(state.can_insert);
        assert!(!state.can_make_line, "already a line");
        assert!(state.can_make_curve);
        assert!(!state.can_delete, "a segment, not a node, is selected");

        session.insert_selected();
        assert_eq!(session.paths()[0].anchors.len(), 3);
        assert_eq!(session.node_toolbar_state(), NodeToolbarState::default());
    }

    /// AC1: drawing a rectangle with the rectangle tool, from an empty
    /// canvas, with no UI setup beyond switching tools.
    #[test]
    fn ac1_rectangle_tool_creates_a_rectangle() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Rect { .. }));
    }

    /// AC7: same for the ellipse tool.
    #[test]
    fn ac7_ellipse_tool_creates_an_ellipse() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Ellipse { .. }));
    }

    /// AC11: same for the polygon/star tool, in polygon mode by default.
    #[test]
    fn ac11_polygon_star_tool_creates_a_polygon_by_default() {
        let mut session = Session::new(1);
        session.set_tool(Tool::PolygonStar);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Polygon { .. }));
    }

    /// AC17, AC21: "object to path" only runs when explicitly invoked —
    /// switching tools and selecting a different object leave the
    /// primitive as it is; `convert_selected_to_paths` then replaces it
    /// with a path, keeping its id, and selects it in the node tool.
    #[test]
    fn ac17_ac21_object_to_path_is_explicit_and_keeps_the_id() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false);
        let id = session.primitives()[0].id;

        // Switching tools and back does not convert it.
        session.set_tool(Tool::Node);
        session.set_tool(Tool::Rectangle);
        assert!(session.document.primitive(id).is_some());

        // Re-select it (a fresh `Session::pointer_down` on its outline)
        // before converting, since switching tools does not itself
        // clear the shared primitive selection, but this test does not
        // rely on that — it re-selects explicitly for clarity.
        session.pointer_down(Point::new(5.0, 0.0), false);
        session.convert_selected_to_paths();

        assert!(session.document.primitive(id).is_none());
        let path = session.document.path(id).expect("same id, now a path");
        assert!(path.closed);
        assert_eq!(path.anchors.len(), 4);
        assert_eq!(session.tool(), Tool::Node);
        assert_eq!(
            session.node.selection().nodes().len(),
            4,
            "AC17: immediately editable"
        );
    }

    /// AC22: two different primitive kinds selected together both
    /// convert, independently, in one call.
    #[test]
    fn ac22_multi_object_conversion() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false);
        let rect_id = session.primitives()[0].id;

        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(50.0, 50.0), false);
        session.pointer_up(Point::new(60.0, 60.0), false);
        let ellipse_id = session
            .primitives()
            .into_iter()
            .find(|p| p.id != rect_id)
            .expect("ellipse exists")
            .id;

        session.primitive_selection.select_single(rect_id);
        session.primitive_selection.toggle(ellipse_id);
        session.convert_selected_to_paths();

        assert!(session.document.path(rect_id).is_some());
        assert!(session.document.path(ellipse_id).is_some());
    }
}
