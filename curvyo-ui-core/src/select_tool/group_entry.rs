//! The typed entries of a multi-selection (`specs/0019-multi-object-transform/`,
//! criteria 33 to 37): the keys M, R, S and K and the double-click on a group
//! handle open the same chips a single object has, over the group box. A child
//! module of `select_tool`.

use std::collections::HashMap;

use curvyo_document_core::{NodeId, ObjectSnapshot, Point};

use super::entry::OpenEntry;
use super::{EntryKey, KeyEntryRefusal, SelectDoubleClickOutcome, SelectTool};
use crate::ResizeDirection;
use crate::group_box::GroupSelection;
use crate::group_entry::GroupEntry;
use crate::move_entry::MoveEntry;
use crate::object_selection::ObjectSelection;
use crate::transform_handle_layout::{EditHandle, Side, TransformHandleTolerances};

/// The selected objects in selection order.
fn selected_in_order(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
) -> Vec<ObjectSnapshot> {
    let by_id: HashMap<NodeId, &ObjectSnapshot> =
        objects.iter().map(|object| (object.id(), object)).collect();
    selection
        .ids()
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .cloned()
        .collect()
}

impl SelectTool {
    /// The keys M, R, S, K and Shift+K for a multi-selection: opens the entry
    /// of the matching group handle (criteria 33 to 37), as a double-click on it
    /// does, with no Shift pivot and no Ctrl link. S scales about the group box
    /// centre (`edit-interaction-polish` criterion 57a). The entries do not
    /// depend on the handle being drawn: the chip is placed from the box.
    pub(super) fn open_group_entry_for_key(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        group: &GroupSelection,
        key: EntryKey,
    ) -> Result<(), KeyEntryRefusal> {
        let members = selected_in_order(objects, selection);
        let skew = |side| {
            GroupEntry::for_skew(&members, group, side, false)
                .map(OpenEntry::Group)
                .ok_or(KeyEntryRefusal::SkewNeedsPath)
        };
        let entry = match key {
            EntryKey::Move => OpenEntry::Move(MoveEntry::for_group(&members, group)),
            EntryKey::Angle => OpenEntry::Group(GroupEntry::for_rotate(
                &members,
                group,
                ResizeDirection::Ne,
                false,
            )),
            EntryKey::Size => OpenEntry::Group(
                GroupEntry::for_resize(
                    &members,
                    group,
                    ResizeDirection::Se,
                    (true, false),
                    self.modes,
                )
                .with_centre_chip(),
            ),
            EntryKey::SkewX => skew(Side::Top)?,
            EntryKey::SkewY => skew(Side::Right)?,
        };
        self.entry = Some(entry);
        Ok(())
    }

    /// A double-click on a group handle (criterion 47): opens the entry of that
    /// handle. `None` when the second press is not on a handle the first press
    /// grabbed.
    pub(super) fn group_double_click(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        modifiers: (bool, bool),
    ) -> Option<SelectDoubleClickOutcome> {
        let (shift, ctrl) = modifiers;
        let (group, handle) =
            Self::group_handle_at(objects, selection, point, tolerances, shift, true)?;
        if self.last_press_handle != Some(handle) {
            return None;
        }
        let members = selected_in_order(objects, selection);
        let entry = match handle {
            EditHandle::Move => {
                OpenEntry::Move(MoveEntry::for_group(&members, &group).with_copy_preset(ctrl))
            }
            EditHandle::Rotate(direction) => {
                OpenEntry::Group(GroupEntry::for_rotate(&members, &group, direction, shift))
            }
            EditHandle::Resize(direction) => OpenEntry::Group(GroupEntry::for_resize(
                &members,
                &group,
                direction,
                (shift, ctrl),
                self.modes,
            )),
            EditHandle::Skew(side) => {
                OpenEntry::Group(GroupEntry::for_skew(&members, &group, side, shift)?)
            }
            EditHandle::Param(_) => return None,
        };
        self.entry = Some(entry);
        Some(SelectDoubleClickOutcome::EntryOpened)
    }

    /// The open group entry (an angle, a size or a skew of a multi-selection), if any.
    #[must_use]
    pub const fn group_entry(&self) -> Option<&GroupEntry> {
        match &self.entry {
            Some(OpenEntry::Group(entry)) => Some(entry),
            _ => None,
        }
    }
}
