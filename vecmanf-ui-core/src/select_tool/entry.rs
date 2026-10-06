//! The Select tool's numeric-entry and double-click dispatch
//! (`specs/object-transform-refinements/specification.md`, criteria 3, 18,
//! 22, 23, 25-32, 49): opening, committing and cancelling the typed entry,
//! and routing a double-click to a handle, the handoff or nothing. Split
//! out of `select_tool.rs`; a child module, so it shares `SelectTool`'s
//! private state.

use vecmanf_document_core::{Document, ObjectSnapshot, Point, Tolerance};

use super::{SelectDoubleClickOutcome, SelectDrag, SelectTool, sole_selected};
use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;
use crate::transform_entry::{EntryOutcome, TransformEntry};
use crate::transform_handle_layout::{TransformHandle, TransformHandleTolerances};

impl SelectTool {
    /// The open numeric entry, if any.
    #[must_use]
    pub const fn entry(&self) -> Option<&TransformEntry> {
        self.entry.as_ref()
    }

    /// Closes the numeric entry without writing (criterion 20): idempotent.
    pub fn cancel_entry(&mut self) {
        self.entry = None;
    }

    /// Validates and commits the numeric entry (criteria 19, 21, 27, 30,
    /// 31): `texts` are the field texts, `last_edited` the index of the
    /// field the maker edited last. A committed or unchanged entry closes;
    /// an invalid one stays open. An entry whose object changed or went
    /// away since it opened is dropped without writing.
    pub fn commit_entry(
        &mut self,
        document: &Document,
        texts: [&str; 2],
        last_edited: usize,
    ) -> EntryOutcome {
        let Some(entry) = self.entry.as_ref() else {
            return EntryOutcome::Unchanged;
        };
        let outcome = entry.commit(document, texts, last_edited);
        if !matches!(outcome, EntryOutcome::Invalid { .. }) {
            self.entry = None;
        }
        outcome
    }

    /// Acceptance criteria 3, 18, 22, 23, 25-28, 32, 49: the double-click
    /// dispatch. A double-click on a handle (at the *second press's*
    /// position and modifiers) opens the numeric entry for a rotate or
    /// resize handle and does nothing for a skew handle — neither hands
    /// off to the object's own tool. Anywhere else inside the box of the
    /// sole selected object, the centre handle included, hands off to it;
    /// outside, slice 4's outline hit decides.
    pub fn double_click(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerance: Tolerance,
        handle_tolerances: TransformHandleTolerances,
        modifiers: (bool, bool),
    ) -> SelectDoubleClickOutcome {
        let (shift, ctrl) = modifiers;
        self.drag = SelectDrag::None;
        if let Some((object, box_, handle)) =
            Self::handle_at(objects, selection, point, handle_tolerances, shift)
        {
            let entry = match handle {
                TransformHandle::Rotate(direction) => {
                    Some(TransformEntry::for_rotate(object, &box_, direction, shift))
                }
                TransformHandle::Resize(direction) => Some(TransformEntry::for_resize(
                    object,
                    &box_,
                    direction,
                    (shift, ctrl),
                    self.stroke_scaling,
                )),
                TransformHandle::Skew(_) | TransformHandle::Move => None,
            };
            return match entry {
                Some(entry) => {
                    self.entry = Some(entry);
                    SelectDoubleClickOutcome::EntryOpened
                }
                None => SelectDoubleClickOutcome::Ignored,
            };
        }
        if Self::is_inside_selected_box(objects, selection, point)
            && let Some(object) = sole_selected(objects, selection)
        {
            return SelectDoubleClickOutcome::Hit(object.clone());
        }
        double_click(objects, point, tolerance)
    }
}

/// Acceptance criteria 22, 23 of slice 4: what a double-click on an
/// object's outline hit, for `Session` to map to the object's own tool
/// (`tool_for`) and hand off to.
#[must_use]
pub fn double_click(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> SelectDoubleClickOutcome {
    let Some(hit) = hit_test_object(objects, point, tolerance) else {
        return SelectDoubleClickOutcome::Miss;
    };
    objects
        .iter()
        .find(|object| object.id() == hit)
        .cloned()
        .map_or(
            SelectDoubleClickOutcome::Miss,
            SelectDoubleClickOutcome::Hit,
        )
}
