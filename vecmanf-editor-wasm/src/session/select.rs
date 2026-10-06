//! `Session`'s Select-tool glue (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23; `specs/0005-
//! object-transform`; `specs/object-transform-refinements`): dispatching
//! pointer, modifier and double-click events to [`vecmanf_ui_core::
//! SelectTool`], its live move/transform preview, the stroke switch and the
//! double-click handoff. What the Select tool *shows* (decoration input,
//! cursor, hint, readout) is in `session/select_view.rs`, the typed entry
//! in `session/transform_entry.rs`. A child module of `session`, so it
//! shares `Session`'s privacy boundary and its methods join the same type's
//! `impl Session`.

use vecmanf_document_core::{ObjectSnapshot, Point, Shape, Vec2};
use vecmanf_ui_core::{SelectDoubleClickOutcome, StrokeScaling, TransformHandleTolerances};

use super::{Session, Tool};

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
    /// The Select tool's own live, uncommitted move offset while a drag
    /// is in flight (acceptance criterion 20's "live") — `None` outside
    /// the Select tool, with no drag in flight, inside the 3 px dead zone,
    /// or before the pointer has ever moved over the canvas.
    pub(super) fn select_live_offset(&self) -> Option<Vec2> {
        if self.tool != Tool::Select {
            return None;
        }
        let cursor = self.pointer_position?;
        self.select.live_offset(cursor)
    }

    /// The Select tool's own live, uncommitted resize, rotate or skew
    /// preview while one of those drags is in flight (`specs/0005-object-
    /// transform/specification.md`, acceptance criteria 14, 22; 40 of
    /// `object-transform-refinements`) — `None` outside the Select tool,
    /// with no such drag in flight, inside the dead zone, or before the
    /// pointer has ever moved over the canvas. The cached
    /// `select_shift_held`/`select_ctrl_held` (set by every
    /// [`Session::pointer_hover`] and [`Session::modifiers_changed`] call)
    /// are what let this be read at render time, with no event of its own.
    pub(super) fn select_live_transform(&self) -> Option<ObjectSnapshot> {
        if self.tool != Tool::Select {
            return None;
        }
        let cursor = self.pointer_position?;
        self.select
            .live_transform(cursor, self.select_shift_held, self.select_ctrl_held)
    }

    /// The "Scale stroke width" switch (`specs/0005-object-transform/
    /// specification.md` AC 26-31): whether a Select-tool resize scales the
    /// stroke width. Off in every new session (`Session::new`/`open` build a
    /// fresh `SelectTool`); never written to the document.
    #[must_use]
    pub fn scale_stroke_width(&self) -> bool {
        self.select.stroke_scaling() == StrokeScaling::Proportional
    }

    /// Sets the switch for the *next* resize drag (AC 28: a drag in flight
    /// keeps the value it started with). Clicking the switch while a numeric
    /// entry is open closes the entry without writing (criterion 31).
    pub fn set_scale_stroke_width(&mut self, on: bool) {
        self.select.cancel_entry();
        self.select.set_stroke_scaling(if on {
            StrokeScaling::Proportional
        } else {
            StrokeScaling::Keep
        });
    }

    /// The Shift and Ctrl modifiers changed with no pointer movement
    /// (`adrs.md`, "Shift state"): the host calls this from window-level key
    /// events, and with `(false, false)` when the window or canvas loses
    /// focus. The cached state is all the next frame, cursor and hint
    /// queries need — side rotate handles appear and vanish in the frame the
    /// key changes (criterion 6), the pivot marker and live preview follow
    /// (criterion 14).
    pub fn modifiers_changed(&mut self, shift: bool, ctrl: bool) {
        self.select_shift_held = shift;
        self.select_ctrl_held = ctrl;
    }

    /// The Select tool's handle tolerances at the current zoom
    /// (`docs/design-system.md`, "Transform handle layout"), converted from
    /// screen pixels to document millimetres.
    pub(super) fn transform_handle_tolerances(&self) -> TransformHandleTolerances {
        TransformHandleTolerances::at_scale(self.view().scale())
    }

    /// Acceptance criteria 1, 4-18: dispatches a press to the Select
    /// tool — first against the current single-object selection's own
    /// transform handles, then (unchanged) against every object's body.
    /// An open numeric entry closes first; this press is processed as usual
    /// (criterion 20).
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

    /// Acceptance criteria 3, 15-18, 20, 41: commits whatever move/resize/
    /// rotate/skew drag [`Session::select_pointer_down`] began. `shift`/
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
    /// not yet clicked into selection: same box at `--accent-hover`") and
    /// lets a drag in flight note the pointer (the dead zone, once left,
    /// stays left).
    pub(super) fn select_hover(&mut self, point: Point) {
        self.select.pointer_moved(point);
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        self.hovered_object = vecmanf_ui_core::hit_test_object(&objects, point, tolerance);
    }

    /// Acceptance criteria 3, 18, 22, 23, 25-32, 49 and slice 4's 22, 23:
    /// a double-click while the Select tool is active, at the second press's
    /// position with its modifiers. On a rotate or resize handle it opens
    /// the typed entry; on a skew handle it does nothing; elsewhere inside
    /// the sole selected object's box (the centre handle included) or on an
    /// outline it hands off to the object's own tool — clearing the node
    /// tool's selection for a path (so it is "ready for node editing with
    /// no nodes selected", `adrs.md`), or selecting the primitive (so its
    /// own shape tool shows its handles at once).
    pub(super) fn select_double_click(&mut self, point: Point, shift: bool, ctrl: bool) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let handle_tolerances = self.transform_handle_tolerances();
        let outcome = self.select.double_click(
            &objects,
            &self.selection,
            point,
            tolerance,
            handle_tolerances,
            (shift, ctrl),
        );
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
}
