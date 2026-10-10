//! The Select tool's drag on a group handle: scale, rotate or skew of a whole
//! multi-selection (`specs/0019-multi-object-transform/`, criteria 18 to 32 and
//! 40 to 42). [`GroupDrag::resolve`] is the one resolving function the live preview, the release
//! and the typed entry call (`adrs.md` decision 1): it derives one document-axes
//! [`GroupMap`] from the box at the press and the pointer, maps every start
//! snapshot with it, and refuses the whole result if any object would end up
//! invalid.

use curvyo_document_core::{Angle, Document, NodeId, ObjectSnapshot, PathSnapshot, Point};

use super::{SelectDrag, SelectTool};
use crate::anchor_id_minter::AnchorIdMinter;
use crate::conversion::{
    ConversionCounts, converted_counts, converted_paths, with_fresh_anchor_ids,
};
use crate::group_box::GroupSelection;
use crate::group_transform::{GroupMap, commit_group, group_map, group_pivot, map_all};
use crate::object_selection::ObjectSelection;
use crate::oriented_box::OrientedBox;
use crate::transform_drag::{DragOrigin, ScaleModes};
use crate::transform_handle_layout::EditHandle;

/// A resize, rotate or skew drag on the group box in flight.
#[derive(Debug, Clone)]
pub(super) struct GroupDrag {
    pub(super) origin: DragOrigin,
    /// The selected objects as they were at the press, in z-order.
    pub(super) starts: Vec<ObjectSnapshot>,
    /// The group box at the press: axis-aligned.
    pub(super) start_box: OrientedBox,
    pub(super) handle: EditHandle,
    /// The tool's switches as of the press (criterion 24).
    pub(super) modes: ScaleModes,
    /// The selection holds a shape a stretch would convert: a corner drag then
    /// always keeps one factor (criterion 19, lead decision E).
    pub(super) proportional_corners: bool,
    /// For a resize handle, beside each start, the path it becomes if the drag ends
    /// as a stretch (criterion 53); `None` for an object that keeps its kind. Built
    /// once, at the press, so that the preview and the commit are the same
    /// snapshots (criterion 32).
    pub(super) converted: Vec<Option<PathSnapshot>>,
}

impl GroupDrag {
    /// The map the pointer at `current` and the modifiers give, always computed
    /// from the state at the press (criterion 19); `None` for a drag that
    /// changes nothing (a skew angle of zero) or for a handle that is no group
    /// gesture.
    pub(super) fn map_at(&self, current: Point, shift: bool, ctrl: bool) -> Option<GroupMap> {
        group_map(
            &self.start_box,
            self.handle,
            (self.origin.down_at, current),
            (shift, ctrl),
            self.proportional_corners,
        )
    }

    /// What a release at `current` would commit: every start snapshot mapped, or
    /// all of them unchanged if any result is not finite and sane (criterion 22:
    /// all or nothing).
    pub(super) fn resolve(&self, current: Point, shift: bool, ctrl: bool) -> Vec<ObjectSnapshot> {
        match self.map_at(current, shift, ctrl) {
            Some(map) => map_all(&self.starts, &self.converted, &map, self.modes),
            None => self.starts.clone(),
        }
    }

    /// The pivot or fixed point the drag uses right now (the pivot marker).
    pub(super) fn pivot(&self, shift: bool) -> Option<Point> {
        group_pivot(&self.start_box, self.handle, shift)
    }

    /// Writes `results` (from [`GroupDrag::resolve`]) as one commit
    /// (criterion 31). A width the document refuses costs the maker nothing
    /// else: the write is repeated without widths. The shapes the commit turned
    /// into paths, or `None` if the document took nothing.
    pub(super) fn commit(
        &self,
        document: &Document,
        results: &[ObjectSnapshot],
    ) -> Option<ConversionCounts> {
        commit_group(document, results, self.modes.stroke)
            .then(|| converted_counts(&self.starts, results))
    }
}

impl SelectTool {
    /// Begins the resize, rotate or skew drag of `handle`, which
    /// [`crate::classify_press`] found on the group box of the multi-selection,
    /// and records it as the handle the press grabbed.
    pub(super) fn begin_group_press(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        origin: DragOrigin,
        handle: EditHandle,
    ) {
        let Some(group) = Self::group_of(objects, selection) else {
            return;
        };
        self.drag = SelectDrag::GroupTransforming(
            self.group_drag_for(objects, selection, &group, origin, handle),
        );
        self.last_press_handle = Some(handle);
    }

    /// The drag a press on `handle` of `group` begins: the selected objects, the
    /// box and the switches as they are now.
    fn group_drag_for(
        &self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        group: &GroupSelection,
        origin: DragOrigin,
        handle: EditHandle,
    ) -> GroupDrag {
        let selected: std::collections::HashSet<NodeId> = selection.ids().iter().copied().collect();
        let starts: Vec<ObjectSnapshot> = objects
            .iter()
            .filter(|object| selected.contains(&object.id()))
            .cloned()
            .collect();
        let converted = if matches!(handle, EditHandle::Resize(_)) {
            converted_paths(&starts)
        } else {
            Vec::new()
        };
        GroupDrag {
            origin,
            starts,
            start_box: *group.bounds(),
            handle,
            modes: self.modes,
            proportional_corners: group.proportional_corners(),
            converted,
        }
    }

    /// The shapes the drag in flight (a group's or a single polygon's or star's) would turn into paths if a release
    /// at `current` committed it: counted by kind, empty while the preview is not
    /// a stretch, before the dead zone is left, or for no group drag (the readout's
    /// second line, criterion 23).
    #[must_use]
    pub fn live_conversion_counts(
        &self,
        current: Point,
        shift: bool,
        ctrl: bool,
    ) -> ConversionCounts {
        match &self.drag {
            SelectDrag::GroupTransforming(drag) if drag.origin.is_active_at(current) => {
                converted_counts(&drag.starts, &drag.resolve(current, shift, ctrl))
            }
            SelectDrag::Transforming(drag) if drag.origin.is_active_at(current) => {
                let result = drag.resolve(current, shift, ctrl);
                converted_counts(std::slice::from_ref(&drag.start), &[result])
            }
            _ => ConversionCounts::default(),
        }
    }

    /// The angle a rotate drag of the group has turned by, with the pointer at
    /// `current` (the readout "Δ 37.4°"): `None` unless a group rotate is in
    /// flight and past the dead zone.
    #[must_use]
    pub fn live_group_rotation(&self, current: Point, shift: bool, ctrl: bool) -> Option<Angle> {
        let SelectDrag::GroupTransforming(drag) = &self.drag else {
            return None;
        };
        if !drag.origin.is_active_at(current) {
            return None;
        }
        match drag.map_at(current, shift, ctrl)? {
            GroupMap::Rotate { delta, .. } => Some(delta),
            GroupMap::Scale { .. } | GroupMap::Shear { .. } => None,
        }
    }

    /// The group box as a rotate drag shows it: the box at the press turned by
    /// the live angle about the live pivot (criterion 28). `None` unless a
    /// group rotate is in flight and past the dead zone.
    #[must_use]
    pub fn group_turned_box(&self, current: Point, shift: bool, ctrl: bool) -> Option<OrientedBox> {
        let SelectDrag::GroupTransforming(drag) = &self.drag else {
            return None;
        };
        let GroupMap::Rotate { pivot, delta } = drag.map_at(current, shift, ctrl)? else {
            return None;
        };
        drag.origin.is_active_at(current).then_some(OrientedBox {
            angle: delta,
            pivot,
            ..drag.start_box
        })
    }

    /// Whether a group resize, rotate or skew drag is in flight and has left its
    /// dead zone at `current`: what a readout needs to show.
    #[must_use]
    pub fn group_drag_active_at(&self, current: Point) -> bool {
        matches!(&self.drag, SelectDrag::GroupTransforming(drag) if drag.origin.is_active_at(current))
    }

    /// Whether the multi-selection's group is being resized by a drag in flight.
    #[must_use]
    pub const fn group_drag_in_flight(&self) -> bool {
        matches!(self.drag, SelectDrag::GroupTransforming(_))
    }
}

impl SelectTool {
    /// Gives the paths a drag or an entry in flight would convert shapes into the
    /// session's own anchor ids, minted from `minter`: called by the session right
    /// after a press or an entry opens, so that what is previewed and what is
    /// written carry real ids. Ids minted for a drag that ends as a uniform scale
    /// or is cancelled are never written, and a burnt counter costs nothing.
    pub fn mint_conversion_ids(&mut self, minter: &mut AnchorIdMinter) {
        let paths: &mut [Option<PathSnapshot>] = match &mut self.drag {
            SelectDrag::GroupTransforming(drag) => &mut drag.converted,
            SelectDrag::Transforming(drag) => std::slice::from_mut(&mut drag.converted),
            _ => match &mut self.entry {
                Some(entry) => entry.converted_paths_mut(),
                None => return,
            },
        };
        for path in paths.iter_mut().flatten() {
            with_fresh_anchor_ids(path, minter);
        }
    }
}
