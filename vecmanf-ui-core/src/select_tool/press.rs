//! Classifies what a press on the canvas lands on while the Select tool is
//! active (`specs/edit-interaction-polish/adrs.md`, decision 5): the one
//! function [`SelectTool::pointer_down`] acts on and the copy badge asks
//! ("wherever a press with Ctrl would start a move"), so the badge cannot
//! disagree with the press.

use vecmanf_document_core::{NodeId, ObjectSnapshot, Point, Tolerance};

use super::SelectTool;
use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;
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
