//! The handles of a multi-selection's group box and the queries over them
//! (`specs/0019-multi-object-transform/`, criteria 9 to 15): the drawn set, the
//! one hit rule for a press, a hover and a cursor, where a typed entry is
//! anchored. The group's counterpart of `handles.rs`, which serves a sole
//! selected object; a child module of `select_tool`.

use curvyo_document_core::{ObjectSnapshot, Point};

use super::SelectTool;
use crate::group_box::{GroupSelection, group_handles, hit_group_handle, is_drawn_group_handle};
use crate::group_transform::group_pivot;
use crate::object_selection::ObjectSelection;
use crate::transform_handle_layout::{EditHandle, TransformHandleTolerances};

impl SelectTool {
    /// The group of the current selection: its box and what it holds. `None`
    /// for fewer than two (existing) selected objects.
    #[must_use]
    pub fn group_of(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
    ) -> Option<GroupSelection> {
        if selection.ids().len() < 2 {
            return None;
        }
        GroupSelection::from_objects(objects, selection.ids())
    }

    /// Every handle the group box of the current multi-selection shows right
    /// now, in document space (criterion 9); empty for no selection or one
    /// object. Independent of a drag in flight; `side_rotate` is
    /// [`SelectTool::side_rotate_revealed`].
    #[must_use]
    pub fn group_handle_positions(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        tolerances: TransformHandleTolerances,
        side_rotate: bool,
    ) -> Vec<(EditHandle, Point)> {
        Self::group_of(objects, selection).map_or_else(Vec::new, |group| {
            group_handles(&group, group.bounds(), &tolerances, side_rotate)
        })
    }

    /// The group handle under `point`, with the group, by the one hit rule
    /// (criterion 11). With `include_move` the centre handle is the fallback
    /// inside its hover region, for hover, cursor, hint and the press (its
    /// move grip is real in a multi-selection, criterion 16). `None` for no
    /// group. Group handles never compete with any object.
    #[must_use]
    pub fn group_handle_at(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
        include_move: bool,
    ) -> Option<(GroupSelection, EditHandle)> {
        let group = Self::group_of(objects, selection)?;
        let handles = group_handles(&group, group.bounds(), &tolerances, shift);
        let hit = hit_group_handle(
            &group,
            &handles,
            group.bounds(),
            point,
            &tolerances,
            include_move,
        )?;
        Some((group, hit))
    }

    /// Whether `handle` of `group` is drawn (an edge resize handle of a box
    /// under 24 px is hit-testable but not drawn, criterion 10).
    #[must_use]
    pub fn group_handle_is_drawn(
        handle: EditHandle,
        group: &GroupSelection,
        tolerances: &TransformHandleTolerances,
    ) -> bool {
        is_drawn_group_handle(handle, group, tolerances)
    }

    /// The pivot the group handle under the pointer *would* use if pressed now
    /// (the pivot marker's preview while Shift is held with no drag running).
    /// `None` for the centre handle.
    #[must_use]
    pub fn group_hover_pivot(
        group: &GroupSelection,
        handle: EditHandle,
        shift: bool,
    ) -> Option<Point> {
        group_pivot(group.bounds(), handle, shift)
    }
}
