//! Classifies what a press on the canvas lands on while the Select tool is
//! active (`specs/0010-edit-interaction-polish/adrs.md`, decision 5): the one
//! function [`SelectTool::pointer_down`] acts on and the copy badge asks
//! ("wherever a press with Ctrl would start a move"), so the badge cannot
//! disagree with the press.

use curvyo_document_core::{NodeId, ObjectSnapshot, Point, Tolerance};

use super::handles::sole_selected;
use super::move_drag::MoveDrag;
use super::{SelectDrag, SelectTool};
use crate::hit_test_object::{filled_interior_above, hit_test_object};
use crate::modifiers::Modifiers;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::oriented_bounds;
use crate::transform_drag::DragOrigin;
use crate::transform_handle_layout::{EditHandle, TransformHandleTolerances};

/// What a press lands on, in the order [`classify_press`] tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressTarget {
    /// A resize, rotate, skew or parameter handle of the sole selected
    /// object: a handle drag begins.
    Handle(EditHandle),
    /// The drawn centre move handle: a move begins, with or without a
    /// modifier, and no modifier toggles the selection
    /// (`edit-interaction-polish` criterion 38).
    CentreHandle,
    /// Inside the sole selected object's box but on no handle (Shift and Ctrl up) and
    /// on no filled object above it: a move begins.
    InsideSelectedBox,
    /// On this object's outline or filled interior: a move begins (and selects
    /// it, or, with Shift, toggles it).
    Object(NodeId),
    /// On nothing the Select tool acts on: a marquee is armed.
    Empty,
    /// Alt is down: a lasso is armed, wherever the press lands, handles and
    /// objects included (`unified-object-editing` criterion 35;
    /// `advanced-selection` criteria 16, 17).
    Lasso,
}

impl PressTarget {
    /// Whether a press with these modifiers arms the lasso, before anything
    /// under the pointer is looked at. The one definition of the Alt rule:
    /// [`classify_press`] returns [`PressTarget::Lasso`] by it, and the
    /// cursor, which needs no object, asks it directly.
    #[must_use]
    pub const fn arms_lasso(modifiers: Modifiers) -> bool {
        modifiers.alt
    }

    /// Whether a press on this target begins a move drag, which a Ctrl held
    /// at the release turns into a copy: the target of every press except a
    /// handle drag and a press on nothing (and a Shift press inside the box
    /// away from the object, which is `Empty` here and arms no move).
    #[must_use]
    pub const fn begins_move(self) -> bool {
        matches!(
            self,
            Self::CentreHandle | Self::InsideSelectedBox | Self::Object(_)
        )
    }
}

/// What a press at `point` lands on, with the modifiers of the press. Alt
/// arms the lasso before anything else. Shift reveals the side rotate
/// handles; Shift and Ctrl each make a press inside the sole selected box away
/// from every object not a move, so the outline hit is tried first (a
/// Shift-click adds to the selection over a filled shape) and an empty result
/// arms the marquee: Shift adds, Ctrl removes (`edit-interaction-polish`
/// criterion 37, `unified-object-editing` criterion 35). Ctrl on an outline or
/// on the centre handle is still a move, which Ctrl turns into a copy.
#[must_use]
pub fn classify_press(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    point: Point,
    tolerance: Tolerance,
    handle_tolerances: TransformHandleTolerances,
    modifiers: Modifiers,
) -> PressTarget {
    let shift = modifiers.shift;
    if PressTarget::arms_lasso(modifiers) {
        return PressTarget::Lasso;
    }
    if let Some((_, _, handle)) =
        SelectTool::handle_at(objects, selection, point, handle_tolerances, shift)
    {
        return PressTarget::Handle(handle);
    }
    if matches!(
        SelectTool::hover_handle_at(objects, selection, point, handle_tolerances, shift),
        Some((_, _, EditHandle::Move))
    ) {
        return PressTarget::CentreHandle;
    }
    // A plain press inside the sole selected object's box that is on no
    // handle is a move, before any outline hit (an outline of another object
    // inside the box does not take the press): without it a small unfilled
    // object could not be moved at all (slice 5 criterion 23). The one
    // exception is a *filled* object lying above the selected one at that
    // point: the press goes to it, or it could never be reached without
    // deselecting first (`0007` criterion 29).
    if !shift && !modifiers.ctrl && SelectTool::is_inside_selected_box(objects, selection, point) {
        let above = sole_selected(objects, selection)
            .and_then(|selected| filled_interior_above(objects, selected.id(), point));
        return above.map_or(PressTarget::InsideSelectedBox, PressTarget::Object);
    }
    hit_test_object(objects, point, tolerance).map_or(PressTarget::Empty, PressTarget::Object)
}

/// The move a press on `hit`'s outline or body begins, with the selection
/// change its modifier asks for. Without Shift a different object becomes the
/// single selection (criterion 16) and a selected one keeps the group so it
/// can be dragged together (criterion 18). With Shift the selection does not
/// change at the press (criterion 29): a release inside the dead zone toggles
/// `hit`, a drag that leaves it moves the selection along one axis, and an
/// unselected `hit` joins it then.
pub(super) fn begin_object_press(
    selection: &mut ObjectSelection,
    hit: NodeId,
    shift: bool,
    origin: DragOrigin,
) -> MoveDrag {
    let mut drag = MoveDrag::new(origin, false);
    if shift {
        drag.pending_toggle = Some(hit);
        if !selection.contains(hit) {
            drag.joins = Some(hit);
        }
    } else if !selection.contains(hit) {
        selection.select_single(hit);
    }
    drag
}

impl SelectTool {
    /// Begins the resize, rotate, skew or parameter drag of `handle`, which
    /// [`classify_press`] found on the sole selected object, and records it as
    /// the handle the press grabbed (a double-click acts only on that one).
    pub(super) fn begin_handle_press(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        origin: DragOrigin,
        handle: EditHandle,
        tolerances: &TransformHandleTolerances,
    ) {
        // A handle belongs to the sole selected object, so it exists here.
        let Some(object) = sole_selected(objects, selection) else {
            return;
        };
        let box_ = oriented_bounds(object);
        self.drag = SelectDrag::Transforming(
            self.begin_handle_drag(origin, object, box_, handle, tolerances),
        );
        self.last_press_handle = Some(handle);
    }
}
