//! Which handles the Select tool's sole selected object offers, and the one
//! hit rule over them for a press, a hover, a cursor and a double-click
//! (`specs/0005-object-transform/adrs.md`; `specs/unified-object-editing/`):
//! the drawn set (transform handles of the box's tiers, the centre handle where
//! it is drawn, the parameter handles from 72 px) and the queries over it.
//! Split out of `select_tool.rs`; a child module of it.

use vecmanf_document_core::{ObjectSnapshot, Point};

use super::SelectTool;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::{OrientedBox, oriented_bounds};
use crate::param_handles::{centre_drawn, handle_tiers, param_handles};
use crate::transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, TransformHandleTolerances,
    hit_transform_handle, transform_handles,
};
use crate::transform_math::is_polygon_or_star;

/// The handle kinds `object` shows (criteria 11, 37, 50): corner resize
/// only for a polygon or star; skew handles only for a path; the side
/// rotate handles when `side_rotate`.
fn handle_spec_for(object: &ObjectSnapshot, side_rotate: bool) -> HandleSpec {
    HandleSpec {
        resize_directions: if is_polygon_or_star(object) {
            &CORNERS_FOUR
        } else {
            &ALL_EIGHT
        },
        skew: matches!(object, ObjectSnapshot::Path(_)),
        side_rotate,
    }
}

/// Every handle of `object` the hit rule sees right now, in document space:
/// the transform handles (an edge resize handle of a box under 24 px stays
/// hit-testable although it is not drawn, slice 5's rule; see
/// [`crate::is_drawn_handle`]), the centre handle only where it is drawn,
/// and the parameter handles from 72 px. A parameter handle that is not drawn
/// is not in the list and so has no hit area (criteria 6, 7, 8 of
/// `specs/unified-object-editing/`).
fn drawn_edit_handles(
    object: &ObjectSnapshot,
    box_: &OrientedBox,
    tolerances: &TransformHandleTolerances,
    side_rotate: bool,
) -> Vec<(EditHandle, Point)> {
    let mut handles = transform_handles(box_, handle_spec_for(object, side_rotate), tolerances);
    let params = param_handles(object, box_, tolerances);
    let tiers = handle_tiers(box_.width().min(box_.height()), tolerances);
    let centre = box_.to_document(box_.local_center());
    if !centre_drawn(tiers, &params, centre, tolerances, false) {
        handles.retain(|(handle, _)| *handle != EditHandle::Move);
    }
    handles.extend(
        params
            .into_iter()
            .map(|(param, at)| (EditHandle::Param(param), at)),
    );
    handles
}

/// The sole selected object in `objects`, if the selection is exactly one.
pub(super) fn sole_selected<'a>(
    objects: &'a [ObjectSnapshot],
    selection: &ObjectSelection,
) -> Option<&'a ObjectSnapshot> {
    let [only_id] = selection.ids() else {
        return None;
    };
    objects.iter().find(|o| o.id() == *only_id)
}

impl SelectTool {
    /// Every transform handle the current single-object selection shows
    /// right now, in document space — `empty` for no selection or a
    /// multi-selection (acceptance criterion 2 of slice 5). Independent of
    /// whether a drag is in flight; `side_rotate` is
    /// [`SelectTool::side_rotate_revealed`].
    #[must_use]
    pub fn transform_handles(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        tolerances: TransformHandleTolerances,
        side_rotate: bool,
    ) -> Vec<(EditHandle, Point)> {
        let Some(object) = sole_selected(objects, selection) else {
            return Vec::new();
        };
        drawn_edit_handles(object, &oriented_bounds(object), &tolerances, side_rotate)
    }

    /// The transform handle of the current single-object selection under
    /// `point`, if any, with that object and its oriented box — the one hit
    /// rule [`SelectTool::pointer_down`] (to start a drag), a hover cursor
    /// query and a double-click all use, so what the cursor promises and
    /// what a press does can never disagree (criterion 9). `shift` is the
    /// live Shift state: it decides whether the side rotate handles exist.
    /// The centre handle is never returned (it is a body press); see
    /// [`SelectTool::hover_handle_at`]. `None` for no selection or a
    /// multi-selection.
    #[must_use]
    pub fn handle_at<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        Self::query_handle(objects, selection, point, tolerances, shift, false)
    }

    /// [`SelectTool::handle_at`] for hover, cursor and hint: also reports the
    /// centre move handle where no other handle is hit.
    #[must_use]
    pub fn hover_handle_at<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        Self::query_handle(objects, selection, point, tolerances, shift, true)
    }

    fn query_handle<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
        include_move: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        let object = sole_selected(objects, selection)?;
        let box_ = oriented_bounds(object);
        let handles = drawn_edit_handles(object, &box_, &tolerances, shift);
        let hit = hit_transform_handle(&handles, &box_, point, &tolerances, include_move)?;
        Some((object, box_, hit))
    }

    /// Whether `point` lies inside the oriented box of the sole selected
    /// object (edges included). `false` for no selection or several.
    pub(super) fn is_inside_selected_box(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
    ) -> bool {
        let Some(object) = sole_selected(objects, selection) else {
            return false;
        };
        let box_ = oriented_bounds(object);
        let local = box_.to_local(point);
        local.x >= box_.min.x
            && local.x <= box_.max.x
            && local.y >= box_.min.y
            && local.y <= box_.max.y
    }
}
