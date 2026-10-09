//! One open document's editing session: the plain-Rust orchestration
//! this crate's `wasm-bindgen` surface is a thin shell around.
//!
//! Deliberately free of any `wasm-bindgen`/`web-sys`/GPU type so it can
//! be exercised by ordinary `cargo test` — "no editing logic of its own"
//! (ADR 0001 §3) describes the `wasm-bindgen` boundary in `lib.rs`, not
//! this module: the logic itself is `curvyo-document-core`'s,
//! `curvyo-ui-core`'s and `curvyo-render-core`'s, and this module's own
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

mod boolean;
#[cfg(test)]
mod box_refit_tests;
mod corner_readout;
mod document;
mod draw;
mod frame;
mod keys;
mod move_entry;
mod move_indicators;
mod navigation;
mod node;
mod open_error;
mod pen;
mod ruler;
mod select;
mod select_bar;
mod select_gesture;
mod select_view;
mod shapes;
mod style;
mod style_view;
mod tolerances;
mod transform_entry;

use curvyo_document_core::{Document, NodeId, ObjectSnapshot, OpenError, Point, SaveError};
use curvyo_ui_core::{
    AnchorIdMinter, EllipseTool, Hit, Modifiers, NodeTool, ObjectSelection, PenTool,
    PolygonStarTool, RectangleTool, SelectTool, StyleEditor, Viewport, hit_test,
};

pub use boolean::BooleanOutcome;
pub use document::{DocumentSide, FitOutcome, SizeOutcome};
pub use keys::{EscapeStep, KeyHint, KeyInput, KeyOutcome};
pub use move_indicators::MoveIndicators;
pub use select::DoubleClickHint;
#[cfg(target_arch = "wasm32")]
pub use style_view::StylePanelView;

#[cfg(target_arch = "wasm32")]
pub use open_error::map_open_error;

// Re-exported only for `wasm_api`'s own `LiveReadout` wrapper (its only
// consumer, and itself `wasm32`-only) — `#[cfg]`-gated the same way so
// a host build does not see an unused public re-export.
#[cfg(target_arch = "wasm32")]
pub use move_entry::MoveEntryView;
#[cfg(target_arch = "wasm32")]
pub use shapes::LiveReadout;
#[cfg(target_arch = "wasm32")]
pub use transform_entry::{EntryFieldView, EntryView};

/// The largest pointer coordinate (document millimetres) a tool ever sees.
/// A finite value beyond it — one past `f32` range panics the draw-list
/// tessellator — is clamped to it; real pointer events are nowhere near.
const MAX_POINTER_COORDINATE_MM: f64 = 1e9;

/// `point` with both coordinates clamped to ±[`MAX_POINTER_COORDINATE_MM`],
/// or `None` if either is NaN or infinite. Pointer events come from the
/// host; non-finite values must never reach a tool, where they could turn
/// into a NaN `rotation` or an infinite size in a saved file
/// (`specs/0005-object-transform/adrs.md`: "a drag must never make a file
/// unopenable"), and absurd finite ones must not reach the renderer.
fn sanitized_point(point: Point) -> Option<Point> {
    if !(point.x.is_finite() && point.y.is_finite()) {
        return None;
    }
    let clamp = |v: f64| v.clamp(-MAX_POINTER_COORDINATE_MM, MAX_POINTER_COORDINATE_MM);
    Some(Point::new(clamp(point.x), clamp(point.y)))
}

/// Which tool is active. Exactly one at a time — `specification.md`'s
/// tool rail has six buttons (Select, Pen, Node, Rectangle, Ellipse,
/// Polygon/Star), and switching tools is a single active-tool state,
/// not independent flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// The Select tool (acceptance criteria 12-24,
    /// `canvas-navigation-and-selection`) — first in the tool rail, and
    /// the launch default for both `New` and `Open` (acceptance
    /// criterion 13).
    Select,
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
    select: SelectTool,
    /// Shared across the three shape tools (acceptance criterion 22:
    /// two or more primitives can be selected together) and the Select
    /// tool (`specs/0004-canvas-navigation-and-selection/adrs.md`: "one
    /// object selection, shared by the Select tool and the shape tools").
    selection: ObjectSelection,
    tool: Tool,
    /// The Style panel's drag in flight: the ephemeral override drawn in place
    /// of the stored style, committed once on release
    /// (`specs/0007-stroke-and-fill-styling` criterion 36).
    style: StyleEditor,
    /// Pan/zoom view state (ADR 0009 §2: ephemeral — never written to
    /// the document, resets on `New`/`Open`).
    viewport: Viewport,
    hovered: Option<Hit>,
    /// The object currently hovered while the Select tool is active —
    /// the Select-tool counterpart to `hovered` above (acceptance criteria
    /// 14, 15's hover box). A creation tool has no hover highlight.
    hovered_object: Option<NodeId>,
    /// The live pointer position in document space, tracked regardless
    /// of the active tool — the pen tool's rubber-band preview
    /// (`specification.md`'s UX notes) needs it even though it keeps no
    /// hit-test hover state of its own (see [`Session::pointer_hover`]).
    /// `None` before the first move, or once [`Session::pointer_leave`]
    /// says the pointer is off the canvas.
    pointer_position: Option<Point>,
    /// The Shift, Ctrl and Alt modifiers' live state, as of the most recent
    /// [`Session::pointer_hover`] or [`Session::modifiers_changed`] call —
    /// the Select tool's resize/rotate live preview needs them at *render*
    /// time (`specs/0005-object-transform/specification.md`'s pivot-swap
    /// modifier; acceptance criteria 5, 17), when `draw_list` has no event
    /// of its own to read them from, and the lasso, the marquee's mode
    /// inversion and the cursor read Alt the same way (`advanced-selection`).
    held: Modifiers,
    /// The objects as the document held them when a Select-tool drag began,
    /// kept for the drag's life: a drag writes nothing until its release, so
    /// the document cannot change under it, and reading every object out of
    /// the document costs more than the rest of a frame with many objects
    /// (`specs/0009-unified-object-editing` criterion 15). Cleared by the first
    /// [`Session::objects`] after the drag ends.
    drag_objects: std::cell::RefCell<Option<Vec<ObjectSnapshot>>>,
    /// `window.devicePixelRatio` as of the last attach or resize, so the
    /// selection box can snap to whole device pixels
    /// (`edit-interaction-polish` criterion 65). Always positive and finite.
    device_pixel_ratio: f64,
    /// Whether the pointer button is down: set by a press, cleared by a
    /// release or [`Session::pointer_cancelled`]. Only the Escape cascade
    /// reads it, to stop at the drag while the button is held
    /// (`specs/0010-edit-interaction-polish/` criterion 49); the key gate reads
    /// each tool's own drag state instead.
    button_down: bool,
    /// The "r 12.0 mm max" notice shown for 1.5 s at a knob after a typed
    /// radius was limited (`specs/0013-rectangle-corner-radii/` criterion 6): a
    /// limit is never silent. The host clears it after the delay
    /// ([`Session::clear_limit_notice`]); a press clears it too.
    limit_notice: Option<shapes::LiveReadout>,
    /// The objects a refused boolean operation is drawn around (red, hollow, never stored), and
    /// the selection it was refused for: the outline ends with the selection or the tool.
    boolean_refusal: Option<boolean::RefusalMarks>,
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
            select: SelectTool::new(),
            selection: ObjectSelection::new(),
            // Select is the launch default for New and Open alike
            // (acceptance criterion 13; `specs/0004-canvas-navigation-and-
            // selection/adrs.md`: "Select is the launch tool for New and
            // for Open" — matching Inkscape's own default and replacing
            // `path-node-editing`'s provisional Pen default, which that
            // slice's own spec explicitly flagged "revisit once a general
            // selection tool exists").
            tool: Tool::Select,
            style: StyleEditor::default(),
            viewport: Viewport::new(),
            hovered: None,
            hovered_object: None,
            pointer_position: None,
            held: Modifiers::NONE,
            drag_objects: std::cell::RefCell::new(None),
            device_pixel_ratio: 1.0,
            button_down: false,
            limit_notice: None,
            boolean_refusal: None,
        }
    }

    /// Reopens a previously saved `.curvyo` container's bytes (File → Open),
    /// bound to `peer`.
    ///
    /// # Errors
    /// See [`curvyo_document_core::unpack`].
    pub fn open(peer: u64, bytes: &[u8]) -> Result<Self, OpenError> {
        let document = curvyo_document_core::unpack(peer, bytes)?;
        Ok(Self {
            document,
            minter: AnchorIdMinter::new(peer),
            pen: PenTool::new(),
            node: NodeTool::new(),
            rectangle: RectangleTool::new(),
            ellipse: EllipseTool::new(),
            poly_star: PolygonStarTool::new(),
            select: SelectTool::new(),
            selection: ObjectSelection::new(),
            tool: Tool::Select,
            style: StyleEditor::default(),
            viewport: Viewport::new(),
            hovered: None,
            hovered_object: None,
            pointer_position: None,
            held: Modifiers::NONE,
            drag_objects: std::cell::RefCell::new(None),
            device_pixel_ratio: 1.0,
            button_down: false,
            limit_notice: None,
            boolean_refusal: None,
        })
    }

    /// Packs the current document into `.curvyo` container bytes (File →
    /// Save/Save As).
    ///
    /// # Errors
    /// See [`curvyo_document_core::pack`].
    pub fn pack(&self, app_version: &str) -> Result<Vec<u8>, SaveError> {
        curvyo_document_core::pack(&self.document, app_version)
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
        self.flush_select_bar_preview();
        self.flush_style_preview();
        self.select.cancel_entry();
        self.select.forget_press();
        self.select.cancel_gesture();
        // Leaving the Pen ends its path as drawn (a lone node is dropped): no
        // unfinished path stays behind that the Document section's resize would
        // not move (`specs/0015-document-size-and-rulers/` criterion 14a).
        if self.tool == Tool::Pen && tool != Tool::Pen {
            self.pen.finish(&self.document);
            self.pen.escape();
        }
        // A creation tool starts from an empty selection: no selection box
        // stays behind from the Select tool. Creating a shape then selects
        // the new one (`shape_pointer_up`).
        if matches!(tool, Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar) {
            self.selection.clear();
            self.hovered_object = None;
        }
        if tool != self.tool {
            self.boolean_refusal = None;
        }
        self.tool = tool;
    }

    /// The paths the Node and Pen tools work on. A compound path is left out:
    /// its nodes cannot be edited yet (`specs/0016-boolean-operations`
    /// criteria 38 and 38a), so it contributes no node, handle or segment.
    fn paths(&self) -> Vec<curvyo_document_core::PathSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.path(id))
            .filter(|path| !path.is_compound())
            .collect()
    }

    /// Every object in the document, any kind, in z-order — the Select
    /// tool's own counterpart to [`Session::paths`]/[`Session::
    /// primitives`]: both a path and a primitive are "any object" to
    /// `hit_test_object`/`object_bounds`.
    fn objects(&self) -> Vec<ObjectSnapshot> {
        if self.tool == Tool::Select && self.select.drag_in_flight() {
            return self
                .drag_objects
                .borrow_mut()
                .get_or_insert_with(|| self.read_objects())
                .clone();
        }
        *self.drag_objects.borrow_mut() = None;
        self.read_objects()
    }

    /// Reads every object out of the document, in z-order.
    fn read_objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    /// The pointer went down at `point` (document space).
    pub fn pointer_down(&mut self, point: Point, shift: bool) {
        let Some(point) = sanitized_point(point) else {
            return;
        };
        // Same flush as `set_tool`'s own doc comment explains: a canvas
        // click can change the selection (e.g. selecting a different
        // star) before a pending ratio-slider preview ever gets a
        // chance to commit against the selection it was previewed
        // against, if the mouse was released outside the slider itself.
        self.flush_select_bar_preview();
        self.flush_style_preview();
        self.button_down = true;
        match self.tool {
            Tool::Select => {
                self.select_pointer_down(point, shift);
            }
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
                self.shape_pointer_down(point);
            }
        }
    }

    /// The pointer moved to `point`. Always records `point` as the live
    /// cursor position — [`Session::draw_list`]'s pen-tool rubber-band
    /// preview needs it (`specification.md`'s UX notes) even though the
    /// pen tool keeps no hit-test hover state of its own. For the node
    /// tool, additionally updates hover state for the hover ring (same
    /// UX notes); for a creation tool, feeds the create-drag in flight
    /// for the live preview (ux-engineer review) and nothing else: no
    /// hover state (`unified-object-editing` criterion 25).
    /// `constrain` is the Ctrl modifier's current state, consulted only
    /// by the rectangle/ellipse tools' create-drag preview (acceptance
    /// criteria 2, 8; `shift` centres the box on the press point,
    /// `shape-creation-from-center`) and — since `object-transform` — by the Select
    /// tool's own live resize/rotate preview, alongside `shift`
    /// (acceptance criteria 5, 7, 16, 17); both are cached
    /// (`held`) so [`Session::draw_list`]
    /// can read their current state with no event of its own.
    pub fn pointer_hover(&mut self, point: Point, shift: bool, constrain: bool) {
        let Some(point) = sanitized_point(point) else {
            return;
        };
        self.pointer_position = Some(point);
        self.hovered = None;
        self.hovered_object = None;
        self.held.shift = shift;
        self.held.ctrl = constrain;
        match self.tool {
            Tool::Select => {
                self.select_hover(point, self.held);
            }
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
                self.shape_pointer_move(point, Modifiers::new(shift, constrain));
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
    }

    /// The pointer released at `point`, ending whatever gesture
    /// [`Session::pointer_down`] began. `constrain` is the Ctrl
    /// modifier's state at release — consulted by the rectangle and
    /// ellipse tools (acceptance criteria 2, 8) and, since
    /// `object-transform`, by the Select tool's own resize/rotate commit
    /// alongside `shift` (acceptance criteria 5, 7, 16, 17).
    pub fn pointer_up(&mut self, point: Point, shift: bool, constrain: bool) {
        self.button_down = false;
        let Some(point) = sanitized_point(point) else {
            // A release at a non-finite position cannot be committed to
            // anything; cancel the gesture rather than write NaN.
            self.cancel_gesture();
            return;
        };
        match self.tool {
            Tool::Select => {
                self.select_pointer_up(point, shift, constrain);
            }
            Tool::Pen => {
                let threshold = self.drag_threshold();
                self.pen
                    .pointer_up(&mut self.minter, &self.document, point, threshold);
            }
            Tool::Node => {
                self.node.pointer_up(&self.document, point);
            }
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {
                self.shape_pointer_up(point, Modifiers::new(shift, constrain));
            }
        }
    }

    /// The one double-click dispatch point
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "the host
    /// detects a double-click... and calls `double_click(x, y)`.
    /// `Session` dispatches it"): the pen tool finishes its in-progress
    /// path (acceptance criterion 3), the node tool inserts a node on the
    /// hit segment (acceptance criterion 12, via [`Session::insert_at`]),
    /// the Select tool opens a handle's typed entry, hands a path off to
    /// the Node tool or, on a primitive, changes nothing
    /// (`specs/0009-unified-object-editing/` criteria 31 to 34), and the
    /// creation tools ignore it (the first click of a double-click was an
    /// ordinary press with no movement, which writes nothing). Returns
    /// whether the host should show the edit hint chip (criterion 32). The shorthand of the
    /// tests that predate [`Session::double_click_hint`], which the wasm facade calls.
    pub fn double_click(&mut self, point: Point, shift: bool, ctrl: bool) -> bool {
        self.double_click_hint(point, shift, ctrl) == DoubleClickHint::EditHint
    }

    /// [`Session::double_click`], telling the host which hint to show, if any.
    pub fn double_click_hint(&mut self, point: Point, shift: bool, ctrl: bool) -> DoubleClickHint {
        let Some(point) = sanitized_point(point) else {
            return DoubleClickHint::None;
        };
        match self.tool {
            Tool::Pen => self.finish_pen(),
            Tool::Node => self.insert_at(point),
            Tool::Select => return self.select_double_click(point, shift, ctrl),
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => {}
        }
        DoubleClickHint::None
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorKind, Vec2};
    use curvyo_ui_core::NodeToolbarState;

    use super::*;

    /// Acceptance criterion 13: Select, not Pen, is the launch default —
    /// `path-node-editing`'s own Pen default was provisional, explicitly
    /// flagged "revisit once a general selection tool exists".
    #[test]
    fn a_new_session_defaults_to_the_select_tool_on_an_empty_document() {
        let session = Session::new(1);
        assert_eq!(session.tool(), Tool::Select);
        assert_eq!(session.document.object_ids(), Vec::new());
    }

    #[test]
    fn drawing_an_open_path_with_the_pen_tool_then_reading_it_back() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();

        let paths = session.paths();
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].closed);
    }

    #[test]
    fn escape_discards_the_in_progress_pen_path_only() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.escape();
        assert_eq!(session.document.object_ids(), Vec::new());
    }

    #[test]
    fn switching_to_the_node_tool_selects_and_edits_a_finished_path() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(5.0, 5.0), false, false);

        let paths = session.paths();
        assert_eq!(paths[0].anchors[0].point, Point::new(5.0, 5.0));
    }

    #[test]
    fn convert_and_delete_dispatch_only_when_the_node_tool_is_active() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();

        // Pen tool is still active: these are no-ops.
        session.convert_selected(AnchorKind::Symmetric);
        session.delete_selected();
        assert_eq!(session.paths()[0].anchors.len(), 2);

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.convert_selected(AnchorKind::Symmetric);
        assert_eq!(session.paths()[0].anchors[0].kind, AnchorKind::Symmetric);
    }

    #[test]
    fn draw_list_is_empty_for_a_brand_new_document() {
        let session = Session::new(1);
        assert_eq!(session.draw_list().triangles.len(), 0);
    }

    #[test]
    fn draw_list_includes_geometry_once_a_path_exists() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();
        assert_ne!(session.draw_list().triangles.len(), 0);
    }

    /// The pen tool's in-progress preview (not yet committed) also shows
    /// up in `draw_list`, and tracks the live pointer position via
    /// `pointer_hover` for its rubber-band line.
    #[test]
    fn draw_list_includes_the_pen_tools_in_progress_preview() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        let empty = session.draw_list().triangle_count();

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        let one_node = session.draw_list().triangle_count();
        assert!(one_node > empty, "the placed node's glyph/hover ring draw");

        session.pointer_hover(Point::new(10.0, 0.0), false, false);
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
    /// `curvyo-render-core`'s own unit test of `build_pen_preview`
    /// directly).
    #[test]
    fn draw_list_shows_the_live_curve_preview_during_a_pen_drag() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);

        // Press down at C and move the cursor without releasing — a drag
        // in flight, same as `pointer_up`'s own AC2 test fixture.
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_hover(Point::new(10.0, 0.0), false, false);
        let press_with_no_movement_yet = session.draw_list().triangle_count();

        session.pointer_hover(Point::new(13.0, 4.0), false, false);
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
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_hover(Point::new(0.0, 0.0), false, false);
        let press_with_no_movement_yet = session.draw_list();

        session.pointer_hover(Point::new(40.0, 40.0), false, false);
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
        session.pointer_up(Point::new(40.0, 40.0), false, false);
        assert_eq!(session.paths()[0].anchors[0].point, Point::new(40.0, 40.0));
    }

    /// Same bug, the handle-drag half (acceptance criterion 9): dragging
    /// a selected node's handle must show it (and, for a smooth node, its
    /// mirrored opposite) at the live value mid-drag.
    #[test]
    fn draw_list_shows_the_live_handle_value_during_a_handle_drag() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        // AC2: a click-drag places a smooth node with symmetric handles.
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(13.0, 4.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        // Select the node first — handles are only hittable once selected.
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        let selected_not_dragging = session.draw_list();

        // Press on the handle endpoint (anchor + handle_out, (13, 4)) and
        // drag it without releasing.
        session.pointer_down(Point::new(13.0, 4.0), false);
        session.pointer_hover(Point::new(20.0, 8.0), false, false);
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
    ///
    /// 2026-10-05 (node-size round): the second node moved from (10, 0)
    /// to (50, 0) — at the identity view used here, 1 document mm is 1
    /// screen px, and `POINT_TOLERANCE_PX` doubling to 16 means the old
    /// 10mm separation would have put "hovering the last node" (distance
    /// 10 from the first) *inside* the now-16mm close tolerance, turning
    /// this into a false positive unrelated to what the test actually
    /// guards. 50mm stays unambiguously outside tolerance regardless.
    #[test]
    fn is_hovering_pen_close_target_matches_the_real_close_decision() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        assert!(!session.is_hovering_pen_close_target(), "idle: no path yet");

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);

        session.pointer_hover(Point::new(0.1, 0.1), false, false);
        assert!(session.is_hovering_pen_close_target());

        session.pointer_hover(Point::new(50.0, 0.0), false, false);
        assert!(
            !session.is_hovering_pen_close_target(),
            "near the last node, not the first"
        );

        session.set_tool(Tool::Node);
        session.pointer_hover(Point::new(0.1, 0.1), false, false);
        assert!(
            !session.is_hovering_pen_close_target(),
            "the node tool never shows a pen cursor"
        );
    }

    /// Resets `session`'s viewport to an identity-equivalent view (1
    /// screen px per document mm, origin at the document origin) — these
    /// two tests were written and pinned against `ViewTransform::
    /// identity()`, back when `Session`'s only view was a bare,
    /// never-defaulted-to-100%-zoom `ViewTransform`. `canvas-navigation-
    /// and-selection` gives `Session` a real `Viewport` defaulting to
    /// 100% zoom (`Zoom::default()`, acceptance criterion 7's `96/25.4`
    /// px/mm) instead, so the exact-pixel-distance reasoning these two
    /// tests pin needs an explicit identity view rather than relying on
    /// the session's own default.
    fn reset_to_identity_view(session: &mut Session) {
        session.viewport = Viewport::new();
        session
            .viewport
            .zoom_about(0.0, 0.0, 1.0 / curvyo_ui_core::PX_PER_MM_AT_100);
    }

    /// Tester verification (PR #20, handles-doubled fix, 2026-10-05):
    /// `HANDLE_TOLERANCE_PX` is 16.0 (was 8.0 before that round; by this
    /// round `POINT_TOLERANCE_PX` is 16.0 too, but was still 8.0 when
    /// this test was written); at the identity view (1 screen px per
    /// document mm, `ViewTransform::identity`) a click 13px from a
    /// selected smooth node's handle endpoint — outside the old 8px
    /// radius, inside the new 16px one — must register as a handle hit.
    /// The click point is also kept far from the anchor itself (~23.8px,
    /// still outside even the now-doubled 16px node tolerance) and from
    /// the segment, so this cannot pass by accidentally hitting
    /// something else.
    #[test]
    fn a_click_13px_from_a_handle_hits_under_the_doubled_tolerance() {
        let mut session = Session::new(1);
        reset_to_identity_view(&mut session);
        session.set_tool(Tool::Pen);
        // A at (0, 0), a plain corner click.
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        // B at (50, 0), dragged so its handle_out lands at (50, 20) —
        // a handle endpoint 20px straight up from B.
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 20.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        // Select B first: handles are only hittable on a selected node.
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);

        let b = session.paths()[0].anchors[1].id;
        assert_eq!(
            session.paths()[0].anchors[1].handle_out,
            Vec2::new(0.0, 20.0),
            "handle endpoint is at document (50, 20)"
        );

        // 13px from the handle endpoint (50, 20); ~23.8px from B itself
        // and far from the A-B segment, so only the handle tolerance can
        // explain a hit here.
        session.pointer_hover(Point::new(63.0, 20.0), false, false);
        assert_eq!(
            session.hovered,
            Some(Hit::Handle {
                path: session.paths()[0].id,
                anchor: b,
                slot: curvyo_document_core::HandleSlot::Out,
            }),
            "13px is outside the old 8px handle radius but inside the new 16px one"
        );
    }

    /// Pins the node-size round's own hit-test doubling, the same way
    /// `a_click_13px_from_a_handle_hits_under_the_doubled_tolerance`
    /// pins the earlier handle one: `POINT_TOLERANCE_PX` is 16.0 now
    /// (was 8.0). At the identity view, a click 13px from node A (a
    /// classic 5-12-13 offset) — outside the old 8px radius, inside the
    /// new 16px one — must register as a node hit. B sits far away
    /// (200, 0) so neither it nor the long A-B segment can explain a hit
    /// here; only the node tolerance can.
    #[test]
    fn a_click_13px_from_a_node_hits_under_the_doubled_tolerance() {
        let mut session = Session::new(1);
        reset_to_identity_view(&mut session);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(200.0, 0.0), false);
        session.pointer_up(Point::new(200.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        let a = session.paths()[0].anchors[0].id;

        // 13px from A (0, 0) — a 5-12-13 offset, well clear of the
        // 200px-long A-B segment and of B itself.
        session.pointer_hover(Point::new(5.0, 12.0), false, false);
        assert_eq!(
            session.hovered,
            Some(Hit::Node {
                path: session.paths()[0].id,
                anchor: a,
            }),
            "13px is outside the old 8px node radius but inside the new 16px one"
        );
    }

    #[test]
    fn pack_then_open_round_trips_a_drawn_path() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
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
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);

        let state = session.node_toolbar_state();
        assert!(state.can_delete);
        assert!(state.can_convert_to_corner);
        assert!(state.can_convert_to_symmetric);
        assert!(!state.can_insert);
        assert!(!state.can_make_line);
        assert!(!state.can_make_curve);
    }

    /// A selected line segment enables insert and make-curve, not
    /// make-line; `insert_selected` then splits it and clears the
    /// selection.
    #[test]
    fn node_toolbar_state_and_insert_selected_for_a_line_segment() {
        // A long segment: its midpoint sits well outside the 16px/mm
        // point-hit tolerance around either endpoint (2026-10-05: was
        // 8px/mm, doubled alongside the node glyph), so the click below
        // lands on the segment itself rather than being read as a node
        // hit of whichever endpoint happens to be nearest.
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(100.0, 0.0), false);
        session.pointer_up(Point::new(100.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);

        let state = session.node_toolbar_state();
        assert!(state.can_insert);
        assert!(!state.can_make_line, "already a line");
        assert!(state.can_make_curve);
        assert!(!state.can_delete, "a segment, not a node, is selected");

        session.insert_selected();
        assert_eq!(session.paths()[0].anchors.len(), 3);
        assert_eq!(session.node_toolbar_state(), NodeToolbarState::default());
    }

    /// Acceptance criteria 8-11: selecting the two ends of one open path
    /// and triggering Join through the whole `Session` surface closes
    /// it, as one merged node.
    #[test]
    fn join_selected_closes_an_open_path_through_the_session() {
        // Far enough apart that the third click does not land inside the
        // (doubled, 16px/mm at this identity view) close-path tolerance
        // around the first node — the same pitfall
        // `is_hovering_pen_close_target_matches_the_real_close_decision`'s
        // own doc comment already names for this exact reason.
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);
        session.pointer_down(Point::new(25.0, 50.0), false);
        session.pointer_up(Point::new(25.0, 50.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(25.0, 50.0), true);
        session.pointer_up(Point::new(25.0, 50.0), false, false);
        assert!(session.node_toolbar_state().can_join);

        session.join_selected();

        let paths = session.paths();
        assert_eq!(paths.len(), 1, "still one object, now closed");
        assert!(paths[0].closed);
        assert_eq!(paths[0].anchors.len(), 2);
    }

    /// Acceptance criteria 12-15: splitting an interior node through the
    /// whole `Session` surface produces two objects.
    #[test]
    fn split_selected_on_an_interior_node_through_the_session() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        session.pointer_down(Point::new(20.0, 0.0), false);
        session.pointer_up(Point::new(20.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        assert!(session.node_toolbar_state().can_split);

        session.split_selected();

        assert_eq!(session.paths().len(), 2, "two separate objects now");
        // `edit-interaction-polish` criteria 50 and 52 (superseding `0006`
        // criterion 15): exactly one of the two coincident nodes is selected,
        // so Join (needs two) and Split (needs an interior node) are off.
        let state = session.node_toolbar_state();
        assert!(!state.can_join && !state.can_split);
        assert!(state.can_delete, "one node is selected");
        // A drag from the shared point moves one end only.
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 8.0), false, false);
        let ends: Vec<Point> = session
            .paths()
            .iter()
            .flat_map(|path| {
                [
                    path.anchors[0].point,
                    path.anchors[path.anchors.len() - 1].point,
                ]
            })
            .filter(|p| (p.x - 10.0).abs() < 1e-9)
            .collect();
        assert_eq!(ends.len(), 2, "both pieces still end at x = 10");
        assert!(ends.contains(&Point::new(10.0, 0.0)));
        assert!(ends.contains(&Point::new(10.0, 8.0)));
    }

    /// `specs/0006-path-merge-split-and-node-types/specification.md`
    /// acceptance criteria 6, 7, 9: the real UI flow, through the whole
    /// `Session` surface — select two *pre-existing*, unrelated path
    /// objects with the Select tool (shift-click, `canvas-navigation-
    /// and-selection`), switch to the Node tool (a rail click / `N`, not
    /// the double-click handoff), click one endpoint and shift-click an
    /// endpoint on the *other* visible path, then Join — merging two
    /// objects that were never touched by Split at all, unlike every
    /// other Join test in this file.
    #[test]
    fn select_two_objects_then_join_their_endpoints_across_paths() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);
        session.finish_pen();

        session.pointer_down(Point::new(0.0, 100.0), false);
        session.pointer_up(Point::new(0.0, 100.0), false, false);
        session.pointer_down(Point::new(50.0, 100.0), false);
        session.pointer_up(Point::new(50.0, 100.0), false, false);
        session.finish_pen();

        assert_eq!(session.paths().len(), 2, "two separate, unrelated objects");

        // Select tool: shift-click selects both objects together
        // (acceptance criterion 17).
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(0.0, 100.0), true);
        session.pointer_up(Point::new(0.0, 100.0), false, false);

        // Switch to the Node tool via the rail/shortcut, not a double-
        // click — acceptance criterion 6: every selected path object's
        // nodes are visible and editable in this one Node-tool session.
        session.set_tool(Tool::Node);
        let paths = session.paths();
        assert_eq!(paths.len(), 2, "both objects still exist, unmerged so far");

        // Click one endpoint, then shift-click an endpoint on the
        // *other* visible path (acceptance criterion 7).
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(0.0, 100.0), true);
        session.pointer_up(Point::new(0.0, 100.0), false, false);

        assert!(
            session.node_toolbar_state().can_join,
            "two endpoint nodes of two different open path objects: joinable (AC 8, 9)"
        );

        session.join_selected();

        let paths = session.paths();
        assert_eq!(paths.len(), 1, "the two objects merged into one (AC 9)");
        assert_eq!(
            paths[0].anchors.len(),
            3,
            "2 + 2 anchors, minus the merged pair"
        );
        assert!(!paths[0].closed);
    }

    /// `docs/design-system.md`: path nodes are drawn by the Node tool
    /// only — not in Select (or any other) tool, where they used to
    /// appear as grey squares on every path.
    #[test]
    fn path_nodes_are_drawn_only_while_the_node_tool_is_active() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);
        session.finish_pen();

        session.set_tool(Tool::Select);
        assert!(!session.decoration_input().show_nodes);
        let select_triangles = session.draw_list().triangle_count();

        session.set_tool(Tool::Node);
        assert!(session.decoration_input().show_nodes);
        // Two nodes, each outline + fill quad (4 triangles per node).
        assert_eq!(session.draw_list().triangle_count(), select_triangles + 8);
    }

    /// The bug this round fixes: `decoration_input()` used to zip
    /// `selection.nodes()` against `selection.path()`, which is `None`
    /// for a genuine cross-path selection — so neither of two selected
    /// nodes on two different paths ever reached `DecorationInput`, and
    /// neither drew as selected. Confirms both land in `selected_nodes`
    /// now, directly, without rendering a frame.
    #[test]
    fn decoration_input_includes_every_selected_node_across_two_different_paths() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Pen);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0), false, false);
        session.finish_pen();
        session.pointer_down(Point::new(0.0, 100.0), false);
        session.pointer_up(Point::new(0.0, 100.0), false, false);
        session.pointer_down(Point::new(50.0, 100.0), false);
        session.pointer_up(Point::new(50.0, 100.0), false, false);
        session.finish_pen();
        let (path_a, path_b) = {
            let paths = session.paths();
            (paths[0].id, paths[1].id)
        };

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0), false, false);
        session.pointer_down(Point::new(0.0, 100.0), true);
        session.pointer_up(Point::new(0.0, 100.0), false, false);

        let input = session.decoration_input();
        assert_eq!(input.selected_nodes.len(), 2);
        assert!(input.selected_nodes.iter().any(|&(p, _)| p == path_a));
        assert!(input.selected_nodes.iter().any(|&(p, _)| p == path_b));
    }
}
