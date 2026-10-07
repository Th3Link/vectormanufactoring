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

use vecmanf_document_core::Point;
use vecmanf_ui_core::{
    CornerRadiusScaling, SelectDoubleClickOutcome, StrokeScaling, TransformHandleTolerances,
};

use super::{Session, Tool};

impl Session {
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

    /// The "Scale corner radius" switch (`specs/unified-object-editing/`,
    /// criterion 23): whether a Select-tool resize scales a rectangle's
    /// corner radius with it. Off in every new session; never written to the
    /// document.
    #[must_use]
    pub fn scale_corner_radius(&self) -> bool {
        self.select.corner_radius_scaling() == CornerRadiusScaling::Proportional
    }

    /// Sets the switch for the *next* resize drag or typed size (a drag in
    /// flight and an open entry keep the value they started with). Clicking
    /// the switch while a numeric entry is open closes the entry without
    /// writing (`object-transform-refinements` criterion 31).
    pub fn set_scale_corner_radius(&mut self, on: bool) {
        self.select.cancel_entry();
        self.select.set_corner_radius_scaling(if on {
            CornerRadiusScaling::Proportional
        } else {
            CornerRadiusScaling::Keep
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
        // Nothing lights up while a drag runs; hover returns after release.
        self.hovered_object = if self.select.drag_in_flight() {
            None
        } else {
            vecmanf_ui_core::hit_test_object(&objects, point, tolerance)
        };
    }

    /// `unified-object-editing` criteria 31 to 34, and 3, 18, 25-32, 49 of the
    /// refinements: a double-click while the Select tool is active, at the
    /// second press's position with its modifiers. On a handle with a typed
    /// entry (rotate, resize, skew, parameter) and on the drawn centre handle
    /// (the typed move, `edit-interaction-polish` criterion 15) it opens the
    /// entry; on empty canvas nothing happens; on a path (outline, or inside
    /// its selected box away from the centre handle) the Node tool is
    /// activated with the path selected, "ready for node editing with no
    /// nodes selected" (`adrs.md`); on a primitive nothing changes and the
    /// host is asked to show the edit hint (the returned `true`). No
    /// double-click switches to a primitive's own tool: there is none.
    pub(super) fn select_double_click(&mut self, point: Point, shift: bool, ctrl: bool) -> bool {
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
        match outcome {
            SelectDoubleClickOutcome::Hit(_) => {
                self.node.cancel_drag();
                self.node.clear_selection();
                self.tool = Tool::Node;
                false
            }
            SelectDoubleClickOutcome::EditHint => true,
            SelectDoubleClickOutcome::Miss | SelectDoubleClickOutcome::EntryOpened => false,
        }
    }
}
