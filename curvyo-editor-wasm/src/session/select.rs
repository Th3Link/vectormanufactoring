//! `Session`'s Select-tool glue (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23; `specs/0005-
//! object-transform`; `specs/0008-object-transform-refinements`): dispatching
//! pointer, modifier and double-click events to [`curvyo_ui_core::
//! SelectTool`], its live move/transform preview, the stroke switch and the
//! double-click handoff. What the Select tool *shows* (decoration input,
//! cursor, hint, readout) is in `session/select_view.rs`, the typed entry
//! in `session/transform_entry.rs`. A child module of `session`, so it
//! shares `Session`'s privacy boundary and its methods join the same type's
//! `impl Session`.

use curvyo_document_core::Point;
use curvyo_ui_core::{
    CornerLinking, CornerRadiusScaling, Modifiers, SelectDoubleClickOutcome, StrokeScaling,
    TransformHandleTolerances,
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

    /// The "Scale corner radius" switch (`specs/0009-unified-object-editing/`,
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

    /// The "Link corners" switch (`specs/0013-rectangle-corner-radii/` criterion 2):
    /// whether a corner radius handle sets all four radii (the default) or
    /// only its own corner. On in every new session (`Session::new`/`open`
    /// build a fresh `SelectTool`, so New and Open reset it); never written
    /// to the document.
    #[must_use]
    pub fn link_corners(&self) -> bool {
        self.select.corner_linking() == CornerLinking::Linked
    }

    /// Sets the switch for the *next* corner radius drag or entry. Changes no
    /// radius and writes nothing; an open numeric entry closes without writing
    /// (criterion 8).
    pub fn set_link_corners(&mut self, on: bool) {
        self.select.set_corner_linking(if on {
            CornerLinking::Linked
        } else {
            CornerLinking::Unlinked
        });
    }

    /// Removes the "max" notice of a limited typed radius (the host calls this
    /// 1.5 s after it appeared).
    pub fn clear_limit_notice(&mut self) {
        self.limit_notice = None;
    }

    /// The Shift, Ctrl and Alt modifiers changed with no pointer movement
    /// (`adrs.md`, "Shift state"): the host calls this from window-level key
    /// events, before every pointer event with that event's modifiers, and
    /// with `(false, false, false)` when the window or canvas loses focus.
    /// The cached state is all the next frame, cursor and hint queries need —
    /// side rotate handles appear and vanish in the frame the key changes
    /// (criterion 6), the pivot marker and live preview follow (criterion 14),
    /// and so do the marquee's mode and the lasso cursor
    /// (`advanced-selection` criteria 11, 14).
    ///
    /// In the Select tool the hover is refreshed at the pointer's last position
    /// with the new modifiers, so the hover box (an Alt press arms a lasso: no
    /// object lights) and the object a press would select follow the key by
    /// themselves, whatever the host re-sends.
    pub fn modifiers_changed(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.held = Modifiers::new(shift, ctrl).with_alt(alt);
        if self.tool == Tool::Select
            && let Some(point) = self.pointer_position
        {
            self.select_hover(point, self.held);
        }
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
        self.limit_notice = None;
        let objects = self.objects();
        let tolerance = self.object_tolerance();
        let handle_tolerances = self.transform_handle_tolerances();
        // Ctrl and Alt are the cached state (the host sends them with the
        // press); Alt decides at this press whether the drag is a lasso.
        let modifiers = Modifiers { shift, ..self.held };
        self.select.pointer_down(
            &objects,
            &mut self.selection,
            point,
            tolerance,
            handle_tolerances,
            modifiers,
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
            Modifiers::new(shift, ctrl).with_alt(self.held.alt),
            &mut self.minter,
        );
    }

    /// Updates the Select tool's own hover state (UX notes: "Hover, tool
    /// not yet clicked into selection: same box at `--accent-hover`") and
    /// lets a drag in flight note the pointer (the dead zone, once left,
    /// stays left).
    pub(super) fn select_hover(&mut self, point: Point, modifiers: Modifiers) {
        self.select
            .pointer_moved(point, modifiers, &mut self.selection);
        let objects = self.objects();
        // Nothing lights up while a drag runs; hover returns after release.
        // Otherwise hover lights the object a press at this point would
        // select, and nothing where a press would grab a handle, move the
        // selection or start a marquee (`0007` criterion 28).
        // (An Alt press arms a lasso, `PressTarget::Lasso`, so nothing lights.)
        self.hovered_object = if self.select.drag_in_flight() {
            None
        } else {
            match curvyo_ui_core::classify_press(
                &objects,
                &self.selection,
                point,
                self.object_tolerance(),
                self.transform_handle_tolerances(),
                modifiers,
            ) {
                curvyo_ui_core::PressTarget::Object(id) => Some(id),
                _ => None,
            }
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
        let tolerance = self.object_tolerance();
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
