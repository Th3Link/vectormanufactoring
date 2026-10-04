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

use vecmanf_document_core::{
    AnchorKind, Document, Length, OpenError, Point, SaveError, Tolerance, ViewTransform,
};
use vecmanf_render_core::{
    DecorationInput, DrawList, Hovered as RenderHovered, build_draw_list, build_pen_preview,
};
use vecmanf_ui_core::{
    AnchorIdMinter, Hit, HitTolerances, NodeTool, NodeToolbarState, PenTool, hit_test,
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
/// tool rail has two buttons, Pen and Node, and switching tools is a
/// single active-tool state, not independent flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// The pen tool (acceptance criteria 1-5).
    Pen,
    /// The node tool (acceptance criteria 7-14).
    Node,
}

/// One open document's whole editing session.
pub struct Session {
    document: Document,
    minter: AnchorIdMinter,
    pen: PenTool,
    node: NodeTool,
    tool: Tool,
    view: ViewTransform,
    hovered: Option<Hit>,
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
            // Pen is the default tool on an empty canvas
            // (`specification.md`'s UX notes: "there's nothing to select
            // or edit yet, and Pen is what lets the maker start
            // immediately").
            tool: Tool::Pen,
            view: ViewTransform::identity(),
            hovered: None,
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
            tool: Tool::Node,
            view: ViewTransform::identity(),
            hovered: None,
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

    /// Switches the active tool (`B`/`N` canvas-focus shortcuts, or the
    /// tool rail). Switching away from the pen tool mid-path does
    /// **not** discard it — only Escape or finishing does
    /// (`specification.md`'s pen tool is not itself scoped to stay
    /// active just because another tool was clicked; no acceptance
    /// criterion covers this edge, so the safer, less surprising choice
    /// — not silently losing work — is kept).
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

    fn paths(&self) -> Vec<vecmanf_document_core::PathSnapshot> {
        self.document
            .path_ids()
            .into_iter()
            .filter_map(|id| self.document.path(id))
            .collect()
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
    /// the hover ring (same UX notes).
    pub fn pointer_hover(&mut self, point: Point) {
        self.pointer_position = Some(point);
        if self.tool != Tool::Node {
            self.hovered = None;
            return;
        }
        let paths = self.paths();
        self.hovered = hit_test(
            &paths,
            self.node.selection(),
            point,
            self.point_tolerance(),
            self.segment_tolerance(),
        );
    }

    /// The pointer left the canvas entirely — clears the live cursor
    /// position so the pen tool's rubber-band preview disappears rather
    /// than sticking at the last position inside the canvas.
    pub fn pointer_leave(&mut self) {
        self.pointer_position = None;
        self.hovered = None;
    }

    /// The pointer released at `point`, ending whatever gesture
    /// [`Session::pointer_down`] began.
    pub fn pointer_up(&mut self, point: Point) {
        match self.tool {
            Tool::Pen => {
                let threshold = self.drag_threshold();
                self.pen
                    .pointer_up(&mut self.minter, &self.document, point, threshold);
            }
            Tool::Node => {
                self.node.pointer_up(&self.document, point);
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
    /// 4), or clears the node tool's selection, whichever tool is
    /// active — never both, matching `specification.md`'s own rule that
    /// Escape with the node tool active never discards a pen path.
    pub fn escape(&mut self) {
        match self.tool {
            Tool::Pen => {
                self.pen.escape();
            }
            Tool::Node => {
                self.node.escape();
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
    /// (`specification.md`'s UX notes) when it is active.
    #[must_use]
    pub fn draw_list(&self) -> DrawList {
        let paths = self.paths();
        let mut list = build_draw_list(&paths, self.view, &self.decoration_input());
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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
        session.finish_pen();

        let paths = session.paths();
        assert_eq!(paths.len(), 1);
        assert!(!paths[0].closed);
    }

    #[test]
    fn escape_discards_the_in_progress_pen_path_only() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0));
        session.escape();
        assert_eq!(session.document.path_ids(), Vec::new());
    }

    #[test]
    fn switching_to_the_node_tool_selects_and_edits_a_finished_path() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(5.0, 5.0));

        let paths = session.paths();
        assert_eq!(paths[0].anchors[0].point, Point::new(5.0, 5.0));
    }

    #[test]
    fn convert_and_delete_dispatch_only_when_the_node_tool_is_active() {
        let mut session = Session::new(1);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
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
        session.pointer_up(Point::new(0.0, 0.0));
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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));

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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(10.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0));
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(0.0, 0.0));

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
        session.pointer_up(Point::new(0.0, 0.0));
        session.pointer_down(Point::new(100.0, 0.0), false);
        session.pointer_up(Point::new(100.0, 0.0));
        session.finish_pen();

        session.set_tool(Tool::Node);
        session.pointer_down(Point::new(50.0, 0.0), false);
        session.pointer_up(Point::new(50.0, 0.0));

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
