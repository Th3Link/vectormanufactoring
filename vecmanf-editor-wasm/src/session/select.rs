//! `Session`'s Select-tool glue (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23): dispatching
//! pointer events to [`vecmanf_ui_core::SelectTool`], its hover/decoration
//! input, and the double-click handoff. Split out of `session/mod.rs` the
//! same way `session/shapes.rs` is for the shape tools (architect review:
//! one responsibility per file) — a child module of `session`, so it
//! shares `Session`'s privacy boundary and its methods below join the
//! same type's `impl Session` `session/mod.rs` itself defines.

use vecmanf_document_core::{ObjectSnapshot, Point, Shape};
use vecmanf_render_core::{SelectDecorationInput, TransformDecorationInput, TransformHandleGlyph};
use vecmanf_ui_core::{
    SelectDoubleClickOutcome, SelectTool, TransformHandle, TransformHandleTolerances, double_click,
    hit_test_object, object_bounds,
};

use super::{
    Session, TRANSFORM_RESIZE_HANDLE_TOLERANCE_PX, TRANSFORM_ROTATE_HANDLE_OFFSET_PX,
    TRANSFORM_ROTATE_HANDLE_TOLERANCE_PX, Tool,
};

/// Which tool a primitive of this shape is created/edited with — the one
/// mapping both directions funnel through
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "`Session` maps
/// kind to `Tool` with **one** function, `tool_for`... The existing
/// `shape_matches_active_tool` is then derived from it, so the two
/// directions of the mapping cannot drift apart").
pub(super) fn tool_for_shape(shape: &Shape) -> Tool {
    match shape {
        Shape::Rect { .. } => Tool::Rectangle,
        Shape::Ellipse { .. } => Tool::Ellipse,
        Shape::Polygon { .. } | Shape::Star { .. } => Tool::PolygonStar,
    }
}

/// Which tool a double-clicked object hands off to (acceptance criteria
/// 22, 23): a path to the Node tool, a primitive to its own creation
/// tool via [`tool_for_shape`].
pub(super) fn tool_for(object: &ObjectSnapshot) -> Tool {
    match object {
        ObjectSnapshot::Path(_) => Tool::Node,
        ObjectSnapshot::Primitive(primitive) => tool_for_shape(&primitive.shape),
    }
}

impl Session {
    /// The three tolerances the Select tool's own transform handles need
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criterion 1), converted from screen pixels to document
    /// millimetres at the current zoom — the same conversion every
    /// other hit-test tolerance in this module already uses.
    fn transform_handle_tolerances(&self) -> TransformHandleTolerances {
        let scale = self.view().scale();
        TransformHandleTolerances {
            resize: vecmanf_document_core::Tolerance::from_mm(
                TRANSFORM_RESIZE_HANDLE_TOLERANCE_PX / scale,
            ),
            rotate: vecmanf_document_core::Tolerance::from_mm(
                TRANSFORM_ROTATE_HANDLE_TOLERANCE_PX / scale,
            ),
            rotate_offset_mm: TRANSFORM_ROTATE_HANDLE_OFFSET_PX / scale,
        }
    }

    /// Acceptance criteria 1, 4-18: dispatches a press to the Select
    /// tool — first against the current single-object selection's own
    /// transform handles, then (unchanged) against every object's body.
    pub(super) fn select_pointer_down(&mut self, point: Point, shift: bool) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let handle_tolerances = self.transform_handle_tolerances();
        self.select.pointer_down(
            &objects,
            &mut self.selection,
            point,
            tolerance,
            handle_tolerances,
            shift,
        );
    }

    /// Acceptance criteria 3, 15-18, 20: commits whatever move/resize/
    /// rotate drag [`Session::select_pointer_down`] began. `shift`/
    /// `ctrl` are the modifiers' state at release.
    pub(super) fn select_pointer_up(&mut self, point: Point, shift: bool, ctrl: bool) {
        let objects = self.objects();
        self.select.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            point,
            shift,
            ctrl,
        );
    }

    /// Updates the Select tool's own hover state (UX notes: "Hover, tool
    /// not yet clicked into selection: same box at `--accent-hover`").
    pub(super) fn select_hover(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        self.hovered_object = hit_test_object(&objects, point, tolerance);
    }

    /// Acceptance criteria 22, 23: a double-click while the Select tool
    /// is active hands off to the hit object's own tool — clearing the
    /// node tool's selection for a path (so it is "ready for node
    /// editing with no nodes selected", `adrs.md`), or selecting the
    /// primitive (so its own shape tool shows its handles at once).
    pub(super) fn select_double_click(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let outcome = double_click(&objects, point, tolerance);
        let SelectDoubleClickOutcome::Hit(object) = outcome else {
            return;
        };
        let target = tool_for(&object);
        match &object {
            ObjectSnapshot::Path(_) => {
                self.node.escape();
            }
            ObjectSnapshot::Primitive(primitive) => {
                self.selection.select_single(primitive.id);
            }
        }
        self.tool = target;
    }

    /// Builds the Select tool's decoration input for this frame: every
    /// selected object's own box, plus a hovered-but-unselected one — each
    /// box computed from the *live* (possibly drag-translated) geometry,
    /// via [`vecmanf_ui_core::object_bounds`], the same bounds rule
    /// `vecmanf-render-core`'s own primitive-selection box used to compute
    /// privately.
    pub(super) fn select_decoration_input(&self) -> SelectDecorationInput {
        if self.tool != Tool::Select {
            return SelectDecorationInput::default();
        }
        let mut objects: Vec<ObjectSnapshot> = self
            .live_node_drag_paths()
            .into_iter()
            .map(ObjectSnapshot::Path)
            .collect();
        objects.extend(
            self.primitives_for_render()
                .into_iter()
                .map(ObjectSnapshot::Primitive),
        );
        let selected = self
            .selection
            .ids()
            .iter()
            .filter_map(|&id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, object_bounds(object)))
            })
            .collect();
        let hovered = self
            .hovered_object
            .filter(|&id| !self.selection.contains(id))
            .and_then(|id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, object_bounds(object)))
            });
        SelectDecorationInput { selected, hovered }
    }

    /// Builds the Select tool's transform-handle overlay for this frame
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criteria 1, 14-17, 22): every handle of the current single-object
    /// selection, using the *live* (possibly drag-resized/rotated)
    /// object so the handles themselves track the live preview, plus the
    /// active pivot marker while a drag is in flight.
    ///
    /// The bounding-box outline itself (`select_decoration_input`,
    /// above) still draws the plain, axis-aligned box inherited from
    /// `canvas-navigation-and-selection` — drawing it oriented to a
    /// rotated object's own angle (acceptance criterion 18) is follow-up
    /// work for whoever wires the exact on-canvas chrome next; this
    /// method's own handle positions are already fully rotation-aware
    /// (`vecmanf_ui_core::SelectTool::transform_handles`), which is the
    /// part every acceptance criterion in this slice actually exercises.
    pub(super) fn select_transform_decoration_input(&self) -> TransformDecorationInput {
        if self.tool != Tool::Select {
            return TransformDecorationInput::default();
        }
        let mut objects: Vec<ObjectSnapshot> = self
            .live_node_drag_paths()
            .into_iter()
            .map(ObjectSnapshot::Path)
            .collect();
        objects.extend(
            self.primitives_for_render()
                .into_iter()
                .map(ObjectSnapshot::Primitive),
        );
        let tolerances = self.transform_handle_tolerances();
        let dragging = self.select.dragging_handle();
        let handles = SelectTool::transform_handles(&objects, &self.selection, tolerances)
            .into_iter()
            .map(|(handle, position)| TransformHandleGlyph {
                position,
                is_rotate: matches!(handle, TransformHandle::Rotate),
                dragging: dragging == Some(handle),
            })
            .collect();
        let pivot_marker = self.select.live_pivot(self.select_shift_held);
        TransformDecorationInput {
            handles,
            pivot_marker,
        }
    }
}
