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
use crate::param_entry::ParamEntry;
use crate::transform_entry::{EntryOutcome, TransformEntry};
use crate::transform_handle_layout::{EditHandle, TransformHandleTolerances};

/// The one open numeric entry: a transform entry (angle, size, outer radius)
/// or a parameter entry (corner radius, inner ratio). Two concrete types use
/// the enum.
#[derive(Debug, Clone)]
pub(super) enum OpenEntry {
    /// An angle, size or outer-radius entry.
    Transform(TransformEntry),
    /// A corner-radius or inner-ratio entry.
    Param(ParamEntry),
}

impl OpenEntry {
    pub(super) fn handle(&self) -> EditHandle {
        match self {
            Self::Transform(entry) => entry.handle(),
            Self::Param(entry) => entry.handle(),
        }
    }
}

impl SelectTool {
    /// The open angle, size or outer-radius entry, if any.
    #[must_use]
    pub const fn entry(&self) -> Option<&TransformEntry> {
        match &self.entry {
            Some(OpenEntry::Transform(entry)) => Some(entry),
            Some(OpenEntry::Param(_)) | None => None,
        }
    }

    /// The open corner-radius or inner-ratio entry, if any.
    #[must_use]
    pub const fn param_entry(&self) -> Option<&ParamEntry> {
        match &self.entry {
            Some(OpenEntry::Param(entry)) => Some(entry),
            Some(OpenEntry::Transform(_)) | None => None,
        }
    }

    /// Whether any numeric entry is open.
    #[must_use]
    pub const fn has_entry(&self) -> bool {
        self.entry.is_some()
    }

    /// The handle the open entry belongs to: it keeps its dragging look while
    /// the chip is open.
    #[must_use]
    pub fn entry_handle(&self) -> Option<EditHandle> {
        self.entry.as_ref().map(OpenEntry::handle)
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
        let outcome = match entry {
            OpenEntry::Transform(entry) => entry.commit(document, texts, last_edited),
            OpenEntry::Param(entry) => entry.commit(document, texts[0]),
        };
        if !matches!(outcome, EntryOutcome::Invalid { .. }) {
            self.entry = None;
        }
        outcome
    }

    /// Acceptance criteria 3, 18, 22, 23, 25-28, 32, 49: the double-click
    /// dispatch. A double-click on a handle (at the *second press's*
    /// position and modifiers, and only if the first press grabbed the same
    /// handle) opens the numeric entry for a rotate or
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
        // Kept (not consumed): a rapid third press is another double-click on
        // the same handle and keeps its entry.
        let first_press_handle = self.last_press_handle;
        if let Some((object, box_, handle)) =
            Self::handle_at(objects, selection, point, handle_tolerances, shift)
                .filter(|(_, _, handle)| first_press_handle == Some(*handle))
        {
            let entry = match handle {
                EditHandle::Rotate(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_rotate(object, &box_, direction, shift),
                )),
                EditHandle::Resize(direction) => Some(OpenEntry::Transform(
                    TransformEntry::for_resize(object, &box_, direction, (shift, ctrl), self.modes),
                )),
                EditHandle::Param(param) => {
                    ParamEntry::for_handle(object, &box_, param).map(OpenEntry::Param)
                }
                EditHandle::Skew(_) | EditHandle::Move => None,
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
