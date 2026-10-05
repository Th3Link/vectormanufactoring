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
//! (acceptance criteria 1-22). Everything specific to those three tools
//! — tool-options, live preview/readout, decoration input, "object to
//! path" itself — lives in the `shapes` submodule (architect review:
//! this file alone grew past a size that still read as "one
//! responsibility"); it reaches this module's otherwise-private fields
//! because a child module shares its parent's privacy boundary, and its
//! methods join this type's `impl Session` the same way any other
//! `impl` block in the same crate would.

mod shapes;

use vecmanf_document_core::{
    AnchorKind, Document, Length, NodeId, OpenError, Point, SaveError, Tolerance, ViewTransform,
};
use vecmanf_render_core::{
    DecorationInput, DrawList, Hovered as RenderHovered, build_draw_list, build_pen_preview,
};
use vecmanf_ui_core::{
    AnchorIdMinter, EllipseTool, Hit, HitTolerances, NodeTool, NodeToolbarState, PenTool,
    PolygonStarTool, PrimitiveSelection, RectangleTool, hit_test,
};

// Re-exported only for `wasm_api`'s own `LiveReadout` wrapper (its only
// consumer, and itself `wasm32`-only) — `#[cfg]`-gated the same way so
// a host build does not see an unused public re-export.
#[cfg(target_arch = "wasm32")]
pub use shapes::LiveReadout;

/// 8px node hit-test radius (`docs/design-system.md`).
const POINT_TOLERANCE_PX: f64 = 8.0;
/// 16px handle hit-test radius (`docs/design-system.md`; 2026-10-05:
/// doubled from the node's own 8px alongside the handle glyph's doubled
/// visual size, `vecmanf-render-core::theme::HANDLE_DIAMETER_PX`'s own
/// doc comment).
const HANDLE_TOLERANCE_PX: f64 = 16.0;
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
            // this default (`specs/0003-primitive-shapes/specification.md`'s
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
        // Flushes any pending ratio-slider preview against the
        // selection it was actually previewed against, before anything
        // else can change that selection (architect re-verification: a
        // slider drag released outside the slider element never fires
        // the slider's own `pointerup`/`blur`, so without this the
        // preview would otherwise sit unflushed until some later event
        // commits it against whatever is selected *then* instead).
        self.commit_poly_star_ratio();
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

    fn handle_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(HANDLE_TOLERANCE_PX / self.view.scale())
    }

    fn segment_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(SEGMENT_TOLERANCE_PX / self.view.scale())
    }

    fn hit_tolerances(&self) -> HitTolerances {
        HitTolerances {
            point: self.point_tolerance(),
            handle: self.handle_tolerance(),
            segment: self.segment_tolerance(),
        }
    }

    fn paths(&self) -> Vec<vecmanf_document_core::PathSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.path(id))
            .collect()
    }

    /// [`Session::paths`], with the node tool's in-flight drag (if any)
    /// substituted into the relevant anchor's live, not-yet-committed
    /// position/handle values — resolved by [`NodeTool::live_drag`]
    /// itself (the same helpers [`NodeTool::pointer_up`] uses to commit),
    /// so this is a pure "apply already-resolved data" step with no
    /// geometry of its own. Falls back to the committed snapshot
    /// unmodified outside the node tool, with no drag in flight, or with
    /// the pointer off the canvas (`self.pointer_position` is `None`).
    fn live_node_drag_paths(&self) -> Vec<vecmanf_document_core::PathSnapshot> {
        let mut paths = self.paths();
        if self.tool != Tool::Node {
            return paths;
        }
        let Some(cursor) = self.pointer_position else {
            return paths;
        };
        let Some(live) = self.node.live_drag(cursor) else {
            return paths;
        };
        match live {
            vecmanf_ui_core::LiveNodeDrag::Nodes { path, positions } => {
                if let Some(snapshot) = paths.iter_mut().find(|p| p.id == path) {
                    for (id, point) in positions {
                        if let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == id) {
                            anchor.point = point;
                        }
                    }
                }
            }
            vecmanf_ui_core::LiveNodeDrag::Handle {
                path,
                anchor,
                slot,
                value,
                mirror,
            } => {
                if let Some(snapshot) = paths.iter_mut().find(|p| p.id == path)
                    && let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == anchor)
                {
                    let (own, other) = match slot {
                        vecmanf_document_core::HandleSlot::In => {
                            (&mut anchor.handle_in, &mut anchor.handle_out)
                        }
                        vecmanf_document_core::HandleSlot::Out => {
                            (&mut anchor.handle_out, &mut anchor.handle_in)
                        }
                    };
                    *own = value;
                    if let Some(mirror) = mirror {
                        *other = mirror;
                    }
                }
            }
        }
        paths
    }

    /// The pointer went down at `point` (document space).
    pub fn pointer_down(&mut self, point: Point, shift: bool) {
        // Same flush as `set_tool`'s own doc comment explains: a canvas
        // click can change the selection (e.g. selecting a different
        // star) before a pending ratio-slider preview ever gets a
        // chance to commit against the selection it was previewed
        // against, if the mouse was released outside the slider itself.
        self.commit_poly_star_ratio();
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
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                self.shape_pointer_down(point, shift);
            }
        }
    }

    fn point_tolerance_as_length(&self) -> Length {
        Length::from_mm(self.point_tolerance().as_mm())
    }

    /// The pointer moved to `point`. Always records `point` as the live
    /// cursor position — [`Session::draw_list`]'s pen-tool rubber-band
    /// preview needs it (`specification.md`'s UX notes) even though the
    /// pen tool keeps no hit-test hover state of its own. For the node
    /// tool, additionally updates hover state for the hover ring (same
    /// UX notes); for a shape tool, updates the hovered primitive for
    /// its bounding-box hover outline, *and* feeds whatever drag is in
    /// flight for the live preview (ux-engineer review: acceptance
    /// criteria 3, 4, 5, 9, 13, 14, 15's "updates live" wording).
    /// `constrain` is the Ctrl modifier's current state, consulted only
    /// by the rectangle/ellipse tools' create-drag preview (acceptance
    /// criteria 2, 8).
    pub fn pointer_hover(&mut self, point: Point, constrain: bool) {
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
                    self.handle_tolerance(),
                    self.segment_tolerance(),
                );
            }
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                self.shape_pointer_move(point, constrain);
                self.update_hovered_primitive(point);
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
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                self.shape_pointer_up(point, constrain);
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
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                self.shape_escape();
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

    /// Builds this frame's draw list from the document's current state,
    /// the active view transform, and the node tool's selection/hover —
    /// plus the pen tool's in-progress preview
    /// (`specification.md`'s UX notes) when it is active, every
    /// primitive's own stroke/selection/handle decorations, and (when a
    /// shape-tool drag is in flight) its own live preview outline
    /// (`specs/0003-primitive-shapes/specification.md`, "Live creation
    /// feedback").
    ///
    /// When the node tool has a node/handle drag in flight
    /// (acceptance criteria 8, 9, 10's "update live during the drag"),
    /// `live_node_drag_paths` (private: this module's own internal step,
    /// not part of its public surface) substitutes that drag's live,
    /// not-yet-committed position/handle values into the snapshot before
    /// anything downstream ever sees it — `vecmanf-render-core` needs no
    /// drag-specific code of its own for this: it already draws whatever
    /// `PathSnapshot` it is handed, so a locally live-overridden one
    /// reshapes the stroke and every decoration exactly as if it had
    /// already committed.
    #[must_use]
    pub fn draw_list(&self) -> DrawList {
        let paths = self.live_node_drag_paths();
        let mut list = build_draw_list(&paths, self.view, &self.decoration_input());
        let primitives = self.primitives_for_render();
        list.extend(vecmanf_render_core::build_shape_draw_list(
            &primitives,
            self.view,
            &self.shape_decoration_input(),
        ));
        if let Some(live_shape) = self.live_preview_shape() {
            list.extend(vecmanf_render_core::build_shape_live_preview(
                &live_shape,
                self.view,
            ));
        }
        if self.tool == Tool::Pen
            && let Some(nodes) = self.pen.in_progress_nodes()
        {
            // The id this pending anchor would actually get if the
            // gesture ended right now — `peek`, never `mint`: a preview
            // must not advance the minter's own counter out of step with
            // what might still be escaped or turn into a close gesture
            // instead (`AnchorIdMinter::peek`'s own doc comment).
            let pending = self.pointer_position.and_then(|cursor| {
                self.pen
                    .pending_anchor(self.minter.peek(), cursor, self.drag_threshold())
            });
            list.extend(build_pen_preview(
                nodes,
                self.pointer_position,
                pending.as_ref(),
                self.view,
                self.is_hovering_pen_close_target(),
            ));
        }
        list
    }
}

/// Returns the one-sentence message the frontend's `ErrorDialog` shows
/// for `error` — moved here from `vecmanf-app`'s native `open_error.rs`
/// (`specs/0001-project-file-foundation/specification.md`, "Error handling —
/// invalid/corrupt file") now that [`Session::open`] (and the
/// `Document::open` it wraps) only ever runs inside this wasm session,
/// never natively (`specs/0002-path-node-editing/adrs.md`'s PR review: "the
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
    use vecmanf_document_core::{AnchorKind, Vec2};

    use super::*;

    #[test]
    fn a_new_session_defaults_to_the_pen_tool_on_an_empty_document() {
        let session = Session::new(1);
        assert_eq!(session.tool(), Tool::Pen);
        assert_eq!(session.document.object_ids(), Vec::new());
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
        assert_eq!(session.document.object_ids(), Vec::new());
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
        assert_eq!(session.draw_list().triangles.len(), 0);
    }

    #[test]
    fn draw_list_includes_geometry_once_a_path_exists() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();
        assert_ne!(session.draw_list().triangles.len(), 0);
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

        session.pointer_hover(Point::new(10.0, 0.0), false);
        let with_rubber_band = session.draw_list().triangle_count();
        assert!(
            with_rubber_band > one_node,
            "the rubber-band line to the cursor adds geometry"
        );

        session.pointer_leave();
        let after_leave = session.draw_list().triangle_count();
        assert_eq!(after_leave, one_node, "no cursor, no rubber-band line");
    }

    /// The bug this run fixes: acceptance criterion 2's live
    /// drag-to-curve preview — dragging while placing a new node (mouse
    /// held down, not yet released) must render strictly more than the
    /// plain rubber-band line a hover alone draws, through the actual
    /// `Session::draw_list` path the host calls every frame (not just
    /// `vecmanf-render-core`'s own unit test of `build_pen_preview`
    /// directly).
    #[test]
    fn draw_list_shows_the_live_curve_preview_during_a_pen_drag() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);

        // Press down at C and move the cursor without releasing — a drag
        // in flight, same as `pointer_up`'s own AC2 test fixture.
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_hover(Point::new(10.0, 0.0), false);
        let press_with_no_movement_yet = session.draw_list().triangle_count();

        session.pointer_hover(Point::new(13.0, 4.0), false);
        let mid_drag = session.draw_list().triangle_count();
        assert!(
            mid_drag > press_with_no_movement_yet,
            "the live curve segment and C's growing handle lines/endpoints must add geometry \
             as the drag moves, not just a static rubber-band line"
        );
    }

    /// The bug this run fixes: acceptance criteria 8/10's "the two
    /// adjoining segments update live during the drag" — dragging a
    /// selected node with the node tool (mouse held down, not yet
    /// released) must already draw the node at its live position,
    /// through the actual `Session::draw_list` path, not only after
    /// `pointer_up` commits it.
    #[test]
    fn draw_list_shows_the_live_node_position_during_a_node_drag() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_hover(Point::new(0.0, 0.0), false);
        let press_with_no_movement_yet = session.draw_list();

        session.pointer_hover(Point::new(40.0, 40.0), false);
        let mid_drag = session.draw_list();

        // The committed document must not have moved yet — this is a
        // rendering-only preview (ADR 0009 §2: ephemeral, not written).
        assert_eq!(session.paths()[0].anchors[0].point, Point::new(0.0, 0.0));
        assert_ne!(
            mid_drag, press_with_no_movement_yet,
            "the dragged node (and the segment reshaping with it) must draw at its live \
             position mid-drag, not the stale committed one"
        );

        // On release, the commit matches what was just being previewed.
        session.pointer_up(Point::new(40.0, 40.0), false);
        assert_eq!(session.paths()[0].anchors[0].point, Point::new(40.0, 40.0));
    }

    /// Same bug, the handle-drag half (acceptance criterion 9): dragging
    /// a selected node's handle must show it (and, for a smooth node, its
    /// mirrored opposite) at the live value mid-drag.
    #[test]
    fn draw_list_shows_the_live_handle_value_during_a_handle_drag() {
        let mut session = Session::new(1);
        // AC2: a click-drag places a smooth node with symmetric handles.
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(13.0, 4.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        // Select the node first — handles are only hittable once selected.
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false);
        let selected_not_dragging = session.draw_list();

        // Press on the handle endpoint (anchor + handle_out, (13, 4)) and
        // drag it without releasing.
        session.pointer_down(Point::new(13.0, 4.0), false);
        session.pointer_hover(Point::new(20.0, 8.0), false);
        let mid_drag = session.draw_list();

        assert_eq!(
            session.paths()[0].anchors[1].handle_out,
            Vec2::new(3.0, 4.0),
            "not committed yet"
        );
        assert_ne!(
            mid_drag, selected_not_dragging,
            "the dragged handle (and its mirrored opposite) must draw at its live value \
             mid-drag"
        );
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

        session.pointer_hover(Point::new(0.1, 0.1), false);
        assert!(session.is_hovering_pen_close_target());

        session.pointer_hover(Point::new(10.0, 0.0), false);
        assert!(
            !session.is_hovering_pen_close_target(),
            "near the last node, not the first"
        );

        session.set_tool(Tool::Node);
        session.pointer_hover(Point::new(0.1, 0.1), false);
        assert!(
            !session.is_hovering_pen_close_target(),
            "the node tool never shows a pen cursor"
        );
    }

    /// Tester verification (PR #20, handles-doubled fix, 2026-10-05):
    /// `HANDLE_TOLERANCE_PX` is 16.0 now (was 8.0, the same as
    /// `POINT_TOLERANCE_PX`); at the identity view (1 screen px per
    /// document mm, `ViewTransform::identity`) a click 13px from a
    /// selected smooth node's handle endpoint — outside the old 8px
    /// radius, inside the new 16px one — must register as a handle hit.
    /// The click point is also kept far from the anchor itself and from
    /// the segment, so this cannot pass by accidentally hitting
    /// something else.
    #[test]
    fn a_click_13px_from_a_handle_hits_under_the_doubled_tolerance() {
        let mut session = Session::new(1);
        // A at (0, 0), a plain corner click.
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false);
        // B at (50, 0), dragged so its handle_out lands at (50, 20) —
        // a handle endpoint 20px straight up from B.
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 20.0), false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        // Select B first: handles are only hittable on a selected node.
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false);

        let b = session.paths()[0].anchors[1].id;
        assert_eq!(
            session.paths()[0].anchors[1].handle_out,
            Vec2::new(0.0, 20.0),
            "handle endpoint is at document (50, 20)"
        );

        // 13px from the handle endpoint (50, 20); ~23.8px from B itself
        // and far from the A-B segment, so only the handle tolerance can
        // explain a hit here.
        session.pointer_hover(Point::new(63.0, 20.0), false);
        assert_eq!(
            session.hovered,
            Some(Hit::Handle {
                path: session.paths()[0].id,
                anchor: b,
                slot: vecmanf_document_core::HandleSlot::Out,
            }),
            "13px is outside the old 8px handle radius but inside the new 16px one"
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
}
