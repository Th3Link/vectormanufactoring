//! The Select tool's own transform-handle vocabulary
//! (`specs/0005-object-transform/specification.md`, acceptance criteria
//! 1, 4-17): 8 resize handles (corner + edge-midpoint) plus one rotate
//! handle, laid out in an [`OrientedBox`]'s local frame and mapped to
//! document space through it (`adrs.md`: "every handle position... is
//! computed in the object's own local, rotated coordinate frame").
//! Distinct from [`crate::handle_layout`], which lays out each
//! primitive's *own* shape-tool handles (corner radius, inner radius) —
//! this module is the Select tool's one, kind-independent vocabulary
//! that sits on top of any object's [`OrientedBox`]. The resize and
//! rotate arithmetic lives next door in [`crate::transform_math`].

use vecmanf_document_core::{Angle, Point, Tolerance};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;

/// The 8 resize handles a rectangle, ellipse or path shows (acceptance
/// criterion 1).
pub const ALL_EIGHT: [ResizeDirection; 8] = ResizeDirection::ALL_EIGHT;

/// The 4 corner-only handles a polygon or star shows (acceptance
/// criterion 11: "corner handles only — no edge handles" — note this is
/// the diagonal corners, distinct from `crate::handle_layout`'s own
/// cardinal-only polygon/star handles).
pub const CORNERS_FOUR: [ResizeDirection; 4] = [
    ResizeDirection::Ne,
    ResizeDirection::Se,
    ResizeDirection::Sw,
    ResizeDirection::Nw,
];

/// One of the Select tool's own transform handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformHandle {
    /// A resize handle (corner = free/proportional resize, edge
    /// midpoint = single-axis resize).
    Resize(ResizeDirection),
    /// The dedicated rotate handle (acceptance criteria 15-17).
    Rotate,
}

/// Whether `direction` is one of the four diagonal corners (free or
/// Ctrl-proportional resize) as opposed to an edge midpoint
/// (single-axis only).
#[must_use]
pub const fn is_corner(direction: ResizeDirection) -> bool {
    matches!(
        direction,
        ResizeDirection::Ne | ResizeDirection::Se | ResizeDirection::Sw | ResizeDirection::Nw
    )
}

/// `direction`'s handle position in `box_`'s own local frame.
#[must_use]
pub fn resize_handle_local_position(box_: &OrientedBox, direction: ResizeDirection) -> Point {
    let (x0, y0, x1, y1) = (box_.min.x, box_.min.y, box_.max.x, box_.max.y);
    let (mx, my) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    match direction {
        ResizeDirection::N => Point::new(mx, y0),
        ResizeDirection::Ne => Point::new(x1, y0),
        ResizeDirection::E => Point::new(x1, my),
        ResizeDirection::Se => Point::new(x1, y1),
        ResizeDirection::S => Point::new(mx, y1),
        ResizeDirection::Sw => Point::new(x0, y1),
        ResizeDirection::W => Point::new(x0, my),
        ResizeDirection::Nw => Point::new(x0, y0),
    }
}

/// The rotate handle's local position: `offset_mm` along the box's own
/// local "up" (negative-Y) from the top-edge handle's midpoint
/// (acceptance criterion 1; UX notes' 20px screen-space offset, already
/// converted to document millimetres by the caller the same way every
/// other handle's screen-space hit size is).
#[must_use]
pub fn rotate_handle_local_position(box_: &OrientedBox, offset_mm: f64) -> Point {
    let top_mid = resize_handle_local_position(box_, ResizeDirection::N);
    Point::new(top_mid.x, top_mid.y - offset_mm)
}

/// Every handle a selected object of this resize-handle set currently
/// shows, in document space, paired with its own kind.
#[must_use]
pub fn transform_handles(
    box_: &OrientedBox,
    resize_directions: &[ResizeDirection],
    rotate_offset_mm: f64,
) -> Vec<(TransformHandle, Point)> {
    let mut handles: Vec<(TransformHandle, Point)> = resize_directions
        .iter()
        .map(|&direction| {
            (
                TransformHandle::Resize(direction),
                box_.to_document(resize_handle_local_position(box_, direction)),
            )
        })
        .collect();
    handles.push((
        TransformHandle::Rotate,
        box_.to_document(rotate_handle_local_position(box_, rotate_offset_mm)),
    ));
    handles
}

/// Hit-tests `point` (document space) against every handle in `handles`,
/// returning the nearest one within `tolerance_mm` — distances are taken
/// in document space directly (a rotation preserves distance, so no
/// frame mapping is needed for the comparison itself).
#[must_use]
pub fn hit_test_transform_handle(
    handles: &[(TransformHandle, Point)],
    point: Point,
    tolerance_mm: f64,
) -> Option<TransformHandle> {
    let mut best: Option<(f64, TransformHandle)> = None;
    for &(handle, position) in handles {
        let distance = position.vector_to(point).length();
        if distance > tolerance_mm {
            continue;
        }
        if best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, handle));
        }
    }
    best.map(|(_, handle)| handle)
}

/// The on-screen direction, in degrees clockwise from the horizontal, a
/// resize handle's double-headed-arrow cursor must point
/// (`specs/0005-object-transform/specification.md`'s UX notes, "Cursor
/// feedback": "object rotation + handle's own base angle"): `0` for the
/// E/W handles, `90` for N/S, `45` for the Se/Nw diagonal, `-45` for
/// Ne/Sw — all in Y-down screen space — plus `rotation`. Reduced to
/// `[0, 180)` since a double-headed arrow looks the same turned half a
/// circle.
#[must_use]
pub fn resize_cursor_angle_degrees(direction: ResizeDirection, rotation: Angle) -> f64 {
    let base = match direction {
        ResizeDirection::E | ResizeDirection::W => 0.0,
        ResizeDirection::Se | ResizeDirection::Nw => 45.0,
        ResizeDirection::S | ResizeDirection::N => 90.0,
        ResizeDirection::Sw | ResizeDirection::Ne => 135.0,
    };
    (base + rotation.as_radians().to_degrees()).rem_euclid(180.0)
}

/// How far inside the box's edge, as a fraction of the resize hit radius
/// (6 of 16 px), a resize handle still wins a press over the body.
const INNER_HIT_BAND: f64 = 6.0 / 16.0;

/// The resize handle (if any) a press at `point` grabs — without
/// letting the handles swallow a small object's body.
///
/// Every handle has a hit radius `r` (16 px at the usual zoom), so on
/// an object under about `2r` across the radii would cover the whole
/// outline and nothing could be moved. Two rules keep the body
/// reachable: the radius shrinks to a third of the box's smaller side
/// (never below a quarter of `r`, so a line or a point stays
/// grabbable); and a point *inside* the box only grabs a handle
/// within [`INNER_HIT_BAND`] of `r` from its edge — a press deeper in
/// belongs to the body. Outside the box a handle always wins.
pub(crate) fn resize_handle_hit(
    resize_handles: &[(TransformHandle, Point)],
    box_: &OrientedBox,
    point: Point,
    tolerance: Tolerance,
) -> Option<TransformHandle> {
    let full = tolerance.as_mm();
    let radius = (box_.width().min(box_.height()) / 3.0).clamp(full / 4.0, full);
    let hit = hit_test_transform_handle(resize_handles, point, radius)?;
    let local = box_.to_local(point);
    let inside = local.x > box_.min.x
        && local.x < box_.max.x
        && local.y > box_.min.y
        && local.y < box_.max.y;
    if inside {
        let depth = (local.x - box_.min.x)
            .min(box_.max.x - local.x)
            .min(local.y - box_.min.y)
            .min(box_.max.y - local.y);
        if depth > radius * INNER_HIT_BAND {
            return None;
        }
    }
    Some(hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unrotated_box(min: Point, max: Point) -> OrientedBox {
        OrientedBox {
            min,
            max,
            angle: Angle::from_radians(0.0),
            pivot: Point::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)),
        }
    }

    /// Acceptance criterion 1: the handle set is 8 resize + 1 rotate.
    #[test]
    fn transform_handles_for_a_rect_is_eight_resize_plus_one_rotate() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let handles = transform_handles(&box_, &ALL_EIGHT, 20.0);
        assert_eq!(handles.len(), 9);
        assert_eq!(
            handles
                .iter()
                .filter(|(h, _)| *h == TransformHandle::Rotate)
                .count(),
            1
        );
    }

    /// Acceptance criterion 11: a polygon/star shows only the 4 corner
    /// handles (plus rotate) — 5 total.
    #[test]
    fn transform_handles_for_a_polygon_star_is_four_corners_plus_rotate() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let handles = transform_handles(&box_, &CORNERS_FOUR, 20.0);
        assert_eq!(handles.len(), 5);
    }

    /// The rotate handle sits `offset_mm` above the top-edge handle's
    /// midpoint, along the box's own local "up".
    #[test]
    fn rotate_handle_sits_above_the_top_edge_midpoint() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let position = box_.to_document(rotate_handle_local_position(&box_, 20.0));
        assert!((position.x - 5.0).abs() < 1e-9);
        assert!((position.y - (-20.0)).abs() < 1e-9);
    }

    /// UX notes' rotated resize cursors: base angle plus the object's
    /// own rotation, never one of the four fixed browser cursors once
    /// rotated.
    #[test]
    fn resize_cursor_angle_adds_the_objects_rotation_to_the_handles_base_angle() {
        let zero = Angle::from_radians(0.0);
        assert!((resize_cursor_angle_degrees(ResizeDirection::E, zero) - 0.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::N, zero) - 90.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::Se, zero) - 45.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::Ne, zero) - 135.0).abs() < 1e-9);
        // A "top" handle on a 45-degree-rotated object points along the
        // 45-degree diagonal's perpendicular-to-edge direction: 90 + 45.
        let rotated = Angle::from_radians(45.0_f64.to_radians());
        assert!((resize_cursor_angle_degrees(ResizeDirection::N, rotated) - 135.0).abs() < 1e-9);
        // Opposite handles share one cursor angle.
        assert!(
            (resize_cursor_angle_degrees(ResizeDirection::N, rotated)
                - resize_cursor_angle_degrees(ResizeDirection::S, rotated))
            .abs()
                < 1e-9
        );
    }

    /// Hit-testing picks the nearest handle within tolerance, `None`
    /// beyond it.
    #[test]
    fn hit_test_transform_handle_finds_the_nearest_within_tolerance() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let handles = transform_handles(&box_, &ALL_EIGHT, 20.0);
        let hit = hit_test_transform_handle(&handles, Point::new(10.2, 10.1), 1.0);
        assert_eq!(hit, Some(TransformHandle::Resize(ResizeDirection::Se)));
        let miss = hit_test_transform_handle(&handles, Point::new(500.0, 500.0), 1.0);
        assert_eq!(miss, None);
    }

    /// A rotated box's resize handle still reports the same *local*
    /// position; only its mapped document position changes — proving
    /// handle layout math is computed in the local frame first, per
    /// `adrs.md`'s "oriented bounding box" rule.
    #[test]
    fn resize_handle_local_position_is_independent_of_the_boxs_own_rotation() {
        let unrotated = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let rotated = OrientedBox {
            angle: Angle::from_radians(0.7),
            ..unrotated
        };
        assert_eq!(
            resize_handle_local_position(&unrotated, ResizeDirection::Ne),
            resize_handle_local_position(&rotated, ResizeDirection::Ne)
        );
    }
}
