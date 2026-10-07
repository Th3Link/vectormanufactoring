//! Classifies what a press on the canvas lands on while the Select tool is
//! active (`specs/edit-interaction-polish/adrs.md`, decision 5): the one
//! function [`SelectTool::pointer_down`] acts on and the copy badge asks
//! ("wherever a press with Ctrl would start a move"), so the badge cannot
//! disagree with the press.

use curvyo_document_core::{NodeId, ObjectSnapshot, Point, Tolerance};

use super::handles::sole_selected;
use super::move_drag::MoveDrag;
use super::{SelectDrag, SelectTool};
use crate::hit_test_object::hit_test_object;
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
    /// Inside the sole selected object's box but on no handle (Shift up): a
    /// move begins.
    InsideSelectedBox,
    /// On this object's outline or body: a move begins (and selects it, or,
    /// with Shift, toggles it).
    Object(NodeId),
    /// On nothing the Select tool acts on.
    Empty,
}

impl PressTarget {
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

/// What a press at `point` lands on. `shift` is the Shift state of the press:
/// it reveals the side rotate handles, and a Shift press inside the sole
/// selected box away from the object is not a move, so the outline hit is
/// tried first (a Shift-click adds to the selection over a filled shape).
#[must_use]
pub fn classify_press(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    point: Point,
    tolerance: Tolerance,
    handle_tolerances: TransformHandleTolerances,
    shift: bool,
) -> PressTarget {
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
    // object could not be moved at all (slice 5 criterion 23).
    if !shift && SelectTool::is_inside_selected_box(objects, selection, point) {
        return PressTarget::InsideSelectedBox;
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
