//! Pure resize and rotate arithmetic in an oriented box's local frame
//! (`specs/0005-object-transform/specification.md`, acceptance criteria
//! 4-17): per-axis resize extents and fixed points, the polygon/star and
//! stroke/corner-radius factors, the rotate pivot and the swept angle.
//! Split out of [`crate::transform_handle_layout`], which keeps handle
//! layout and hit-testing; [`crate::transform_drag`] is this module's
//! caller.

use vecmanf_document_core::{Angle, Length, Point, Vec2};

use crate::ResizeDirection;
use crate::angle_snap::snap_angle;
use crate::oriented_box::OrientedBox;
use crate::transform_handle_layout::{is_corner, resize_handle_local_position};

/// Whether this direction's drag changes the box's local X extent (every
/// corner, plus E/W).
const fn touches_x(direction: ResizeDirection) -> bool {
    !matches!(direction, ResizeDirection::N | ResizeDirection::S)
}

/// Whether this direction's drag changes the box's local Y extent (every
/// corner, plus N/S).
const fn touches_y(direction: ResizeDirection) -> bool {
    !matches!(direction, ResizeDirection::E | ResizeDirection::W)
}

/// A resize's resulting local-frame box plus the per-axis scale factors
/// relative to the starting box — the factors feed stroke-width/corner-
/// radius scaling (acceptance criteria 8, 9) without this function
/// knowing anything about either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizedBox {
    /// The new local-frame minimum corner.
    pub min: Point,
    /// The new local-frame maximum corner.
    pub max: Point,
    /// The new width divided by the starting width (1.0 if the
    /// direction never touched X).
    pub sx: f64,
    /// The new height divided by the starting height (1.0 if the
    /// direction never touched Y).
    pub sy: f64,
}

/// Resizes a local-frame box (acceptance criteria 4-7, 13): `direction`'s
/// handle moves by `local_delta` (already mapped into the box's own
/// local frame); the opposite corner/edge stays fixed, or — under
/// `shift` — the box's own center stays fixed and both edges on a
/// touched axis move symmetrically (acceptance criterion 7); `ctrl` on a
/// corner handle (acceptance criterion 5) forces both axes to the same
/// factor, taken from whichever axis `local_delta` moved further along
/// (`specification.md`: "matching the drag's dominant axis"). Every
/// dimension clamps at zero rather than crossing into negative
/// (acceptance criterion 13).
#[must_use]
pub fn resize_local_box(
    start_min: Point,
    start_max: Point,
    direction: ResizeDirection,
    local_delta: Vec2,
    shift: bool,
    ctrl: bool,
) -> ResizedBox {
    let start_width = start_max.x - start_min.x;
    let start_height = start_max.y - start_min.y;
    let center = Point::new(
        f64::midpoint(start_min.x, start_max.x),
        f64::midpoint(start_min.y, start_max.y),
    );

    let (mut new_width, mut x_touched) = (start_width, false);
    let (mut new_height, mut y_touched) = (start_height, false);
    if touches_x(direction) {
        x_touched = true;
        let dragged_sign = x_sign(direction);
        new_width = resized_extent(start_width, shift, dragged_sign, local_delta.x);
    }
    if touches_y(direction) {
        y_touched = true;
        let dragged_sign = y_sign(direction);
        new_height = resized_extent(start_height, shift, dragged_sign, local_delta.y);
    }

    if ctrl && is_corner(direction) {
        let dominant_is_x = local_delta.x.abs() >= local_delta.y.abs();
        let factor = if dominant_is_x {
            safe_factor(new_width, start_width)
        } else {
            safe_factor(new_height, start_height)
        };
        new_width = (start_width * factor).max(0.0);
        new_height = (start_height * factor).max(0.0);
    }

    let (min, max) = rebuild_box(
        start_min,
        start_max,
        center,
        &NewExtents {
            width: new_width,
            height: new_height,
            x_dragged_sign: x_sign(direction),
            y_dragged_sign: y_sign(direction),
        },
        shift,
    );
    ResizedBox {
        min,
        max,
        sx: if x_touched {
            safe_factor(new_width, start_width)
        } else {
            1.0
        },
        sy: if y_touched {
            safe_factor(new_height, start_height)
        } else {
            1.0
        },
    }
}

/// The local-frame point a resize with `direction`/`shift` keeps fixed
/// (acceptance criteria 4, 7): the box's own center under `shift`, or
/// the opposite corner/edge-midpoint otherwise — the same anchor
/// [`resize_local_box`] itself resizes around, exposed separately for a
/// caller that also needs it as a document-space pivot (a path's own
/// [`vecmanf_document_core::PathSnapshot::scaled`] resize-anchor pivot,
/// via [`OrientedBox::to_document`]). On the axis `direction` does not touch
/// at all, any point works equally well (that axis's own factor is
/// always `1.0`), so the box's own center is used there too, for a
/// single predictable answer rather than an arbitrary one.
#[must_use]
pub fn resize_anchor_local_position(
    start_min: Point,
    start_max: Point,
    direction: ResizeDirection,
    shift: bool,
) -> Point {
    let center = Point::new(
        f64::midpoint(start_min.x, start_max.x),
        f64::midpoint(start_min.y, start_max.y),
    );
    if shift {
        return center;
    }
    let x = match x_sign(direction) {
        s if s < 0.0 => start_max.x,
        s if s > 0.0 => start_min.x,
        _ => center.x,
    };
    let y = match y_sign(direction) {
        s if s < 0.0 => start_max.y,
        s if s > 0.0 => start_min.y,
        _ => center.y,
    };
    Point::new(x, y)
}

/// The sign (in local X) of the side `direction`'s handle sits on: `+1`
/// for a handle on the right (Ne/E/Se), `-1` on the left (Nw/W/Sw), `0`
/// for N/S (no X component at all — never consulted, since those
/// directions don't `touches_x`).
const fn x_sign(direction: ResizeDirection) -> f64 {
    match direction {
        ResizeDirection::Ne | ResizeDirection::E | ResizeDirection::Se => 1.0,
        ResizeDirection::Nw | ResizeDirection::W | ResizeDirection::Sw => -1.0,
        ResizeDirection::N | ResizeDirection::S => 0.0,
    }
}

/// The sign (in local Y) of the side `direction`'s handle sits on: `+1`
/// for a handle on the bottom (Se/S/Sw), `-1` on the top (Ne/N/Nw).
const fn y_sign(direction: ResizeDirection) -> f64 {
    match direction {
        ResizeDirection::Se | ResizeDirection::S | ResizeDirection::Sw => 1.0,
        ResizeDirection::Ne | ResizeDirection::N | ResizeDirection::Nw => -1.0,
        ResizeDirection::E | ResizeDirection::W => 0.0,
    }
}

/// One touched axis's new extent (acceptance criterion 13: clamped at
/// zero, never negative): the dragged side's own signed delta component
/// (`dragged_sign * delta_component`) is added to the starting extent
/// (the fixed-opposite-side case, `!shift`) or to half of it, doubled
/// back (the symmetric-about-center case, `shift` — both sides move the
/// same clamped half-extent).
fn resized_extent(start_extent: f64, shift: bool, dragged_sign: f64, delta_component: f64) -> f64 {
    if shift {
        let half = start_extent / 2.0;
        let new_half = (half + dragged_sign * delta_component).max(0.0);
        new_half * 2.0
    } else {
        (start_extent + dragged_sign * delta_component).max(0.0)
    }
}

/// `new_extent / start_extent`, or `1.0` if `start_extent` is not
/// (numerically) positive — there is no meaningful factor to scale a
/// degenerate starting dimension by, and `1.0` (no-op) is the safest
/// default rather than a division by zero.
fn safe_factor(new_extent: f64, start_extent: f64) -> f64 {
    if start_extent > f64::EPSILON {
        new_extent / start_extent
    } else {
        1.0
    }
}

/// The new width/height [`rebuild_box`] positions, plus which side of
/// each axis was the one actually dragged ([`x_sign`]/[`y_sign`]) —
/// bundled into one type so `rebuild_box` takes a `CLAUDE.md`-§5-sized
/// argument list instead of four loose scalars on top of its other
/// parameters.
struct NewExtents {
    width: f64,
    height: f64,
    x_dragged_sign: f64,
    y_dragged_sign: f64,
}

/// Rebuilds the box's four edges from the new width/height: anchored at
/// the opposite corner/edge (`!shift`) or symmetric about `center`
/// (`shift`) — the same anchor rule [`resized_extent`] already applied
/// per axis, now applied to the two axes together to produce the box's
/// actual corners. `extents`'s dragged-side signs are the same
/// [`x_sign`]/[`y_sign`] values [`resized_extent`] used, so the fixed
/// anchor here is always the edge that sign did *not* mark as dragged.
fn rebuild_box(
    start_min: Point,
    start_max: Point,
    center: Point,
    extents: &NewExtents,
    shift: bool,
) -> (Point, Point) {
    if shift {
        (
            Point::new(
                center.x - extents.width / 2.0,
                center.y - extents.height / 2.0,
            ),
            Point::new(
                center.x + extents.width / 2.0,
                center.y + extents.height / 2.0,
            ),
        )
    } else {
        let (new_min_x, new_max_x) = anchor_edge(
            start_min.x,
            start_max.x,
            extents.width,
            extents.x_dragged_sign,
        );
        let (new_min_y, new_max_y) = anchor_edge(
            start_min.y,
            start_max.y,
            extents.height,
            extents.y_dragged_sign,
        );
        (
            Point::new(new_min_x, new_min_y),
            Point::new(new_max_x, new_max_y),
        )
    }
}

/// One axis's new `(min, max)` when the opposite side is the fixed
/// anchor: a negative `dragged_sign` (the min side is the one being
/// dragged, e.g. the W handle) keeps `start_max` fixed and grows the min
/// side outward from it; a non-negative sign (the max side is dragged,
/// e.g. E — or the axis is untouched, sign `0`, where `new_extent`
/// already equals the original extent so either branch gives the same
/// answer) keeps `start_min` fixed instead.
fn anchor_edge(start_min: f64, start_max: f64, new_extent: f64, dragged_sign: f64) -> (f64, f64) {
    if dragged_sign < 0.0 {
        (start_max - new_extent, start_max)
    } else {
        (start_min, start_min + new_extent)
    }
}

/// A polygon/star's own uniform-scale factor from dragging corner
/// handle `direction` (acceptance criterion 11: always uniform,
/// regardless of Ctrl). The handle sits at the corner of the shape's
/// frame box, `radius * √2` from the center along its diagonal, so the
/// pointer's displacement along that diagonal moves the *corner*: the
/// new corner distance is `radius·√2 + projected`, and the radius is
/// that over `√2` — the handle then follows the pointer exactly instead
/// of overshooting it. Clamped at zero; `1.0` for a degenerate zero
/// starting radius (the same `safe_factor` rule every other factor here
/// uses).
#[must_use]
pub fn polygon_star_resize_factor(
    start_radius: f64,
    direction: ResizeDirection,
    local_delta: Vec2,
) -> f64 {
    let unit = direction.unit_vector().normalized_to(1.0);
    let projected = local_delta.x * unit.x + local_delta.y * unit.y;
    let new_radius = (start_radius + projected / std::f64::consts::SQRT_2).max(0.0);
    safe_factor(new_radius, start_radius)
}

/// The stroke-width/corner-radius scale factor for a resize with
/// per-axis factors `sx`/`sy` (acceptance criteria 8, 9): `√(sx·sy)` —
/// Inkscape's own rule, which collapses to plain `sx` whenever `sx ==
/// sy` (a proportional resize), so one formula covers both of the
/// specification's two stated cases. Negative inputs (never expected
/// from this module's own resize factors, which are always `>= 0`) are
/// defended against by clamping to zero before the square root, rather
/// than ever producing a `NaN`.
#[must_use]
pub fn stroke_or_radius_factor(sx: f64, sy: f64) -> f64 {
    (sx.max(0.0) * sy.max(0.0)).sqrt()
}

/// The handle opposite `grabbed` across the box center: corner to opposite
/// corner, side to opposite side (criterion 13). Taken in the box's own
/// (rotated) frame.
#[must_use]
pub const fn opposite_direction(grabbed: ResizeDirection) -> ResizeDirection {
    match grabbed {
        ResizeDirection::N => ResizeDirection::S,
        ResizeDirection::Ne => ResizeDirection::Sw,
        ResizeDirection::E => ResizeDirection::W,
        ResizeDirection::Se => ResizeDirection::Nw,
        ResizeDirection::S => ResizeDirection::N,
        ResizeDirection::Sw => ResizeDirection::Ne,
        ResizeDirection::W => ResizeDirection::E,
        ResizeDirection::Nw => ResizeDirection::Se,
    }
}

/// The active pivot of a rotate drag or entry from the `grabbed` rotate
/// handle (criteria 12, 13): the box center, or — under `shift` — the
/// corner diagonally opposite the grabbed one, or the midpoint of the
/// opposite side. In document space.
#[must_use]
pub fn rotate_pivot(box_: &OrientedBox, grabbed: ResizeDirection, shift: bool) -> Point {
    let local = if shift {
        resize_handle_local_position(box_, opposite_direction(grabbed))
    } else {
        box_.local_center()
    };
    box_.to_document(local)
}

/// The rotate drag's delta angle (criteria 15, 33): the signed angle from
/// `pivot`→`down_at` to `pivot`→`current`, snapped under `ctrl` to the
/// nearest stop of the 15°/22.5° table ([`snap_angle`]) — the *delta*
/// snaps, so the result is always a stop away from wherever the object
/// started, not an absolute document-space snap. `0` when either point
/// coincides with `pivot` (no direction to turn through).
#[must_use]
pub fn rotate_delta_angle(pivot: Point, down_at: Point, current: Point, ctrl: bool) -> Angle {
    let from = pivot.vector_to(down_at);
    let to = pivot.vector_to(current);
    // `!(x > eps)` also rules out NaN: a non-finite pointer sweeps no
    // angle rather than leaking NaN into `rotation`.
    let usable = |v: Vec2| v.length() > f64::EPSILON && v.length().is_finite();
    if !usable(from) || !usable(to) {
        return Angle::from_radians(0.0);
    }
    let from_angle = from.y.atan2(from.x);
    let to_angle = to.y.atan2(to.x);
    let raw = Angle::from_radians(to_angle - from_angle).normalized();
    if ctrl {
        snap_angle(raw).normalized()
    } else {
        raw
    }
}

/// The local displacement of `direction`'s handle that makes the box reach
/// the given width and/or height (the inverse of
/// [`resize_local_box`]'s per-axis extent, criterion 27): without `shift`
/// `sign · (target − start)`, with `shift` half that (both edges move).
/// An axis the direction does not touch, or a `None` target, gets `0`.
#[must_use]
pub fn local_delta_for_size(
    start_box: &OrientedBox,
    direction: ResizeDirection,
    width: Option<f64>,
    height: Option<f64>,
    shift: bool,
) -> Vec2 {
    let delta_for = |start: f64, target: Option<f64>, sign: f64| {
        target.map_or(0.0, |target| {
            let change = target - start;
            sign * if shift { change / 2.0 } else { change }
        })
    };
    let x = if touches_x(direction) {
        delta_for(start_box.width(), width, x_sign(direction))
    } else {
        0.0
    };
    let y = if touches_y(direction) {
        delta_for(start_box.height(), height, y_sign(direction))
    } else {
        0.0
    };
    Vec2::new(x, y)
}

/// The local displacement of a polygon or star's corner handle that makes
/// its outer radius `target_radius` (criterion 26): along the corner's
/// diagonal by `√2 · (target − start)`, which
/// [`polygon_star_resize_factor`] maps back to exactly `target_radius`.
#[must_use]
pub fn local_delta_for_radius(
    start_radius: f64,
    direction: ResizeDirection,
    target_radius: f64,
) -> Vec2 {
    direction
        .unit_vector()
        .normalized_to(1.0)
        .scaled(std::f64::consts::SQRT_2 * (target_radius - start_radius))
}

/// The stroke width (or corner radius) `factor` applied to
/// `start_value`, floored at `min_value` rather than crossing to zero or
/// negative (acceptance criterion 8's "the drag has no further effect
/// in the direction that would cross the zero line, and the stroke
/// width stays at the smallest value still above zero"). Corner radius
/// (acceptance criterion 9) passes `min_value = 0.0`, for which a floor
/// of exactly zero is itself a meaningful, allowed value.
#[must_use]
pub fn scaled_and_floored(start_value: Length, factor: f64, min_value: Length) -> Length {
    let scaled = start_value.as_mm() * factor;
    Length::from_mm(scaled.max(min_value.as_mm()))
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

    /// Acceptance criterion 4: a free corner resize follows the dragged
    /// corner 1:1, opposite corner fixed.
    #[test]
    fn resize_local_box_free_corner_drag() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::Se,
            Vec2::new(5.0, 2.0),
            false,
            false,
        );
        assert_eq!(resized.min, Point::new(0.0, 0.0));
        assert_eq!(resized.max, Point::new(15.0, 12.0));
        assert!((resized.sx - 1.5).abs() < 1e-9);
        assert!((resized.sy - 1.2).abs() < 1e-9);
    }

    /// `resize_anchor_local_position` matches the fixed corner
    /// `resize_local_box` itself keeps put, for every direction.
    #[test]
    fn resize_anchor_local_position_matches_the_fixed_corner() {
        let min = Point::new(0.0, 0.0);
        let max = Point::new(10.0, 10.0);
        assert_eq!(
            resize_anchor_local_position(min, max, ResizeDirection::Se, false),
            Point::new(0.0, 0.0),
            "Se drags bottom-right; top-left stays"
        );
        assert_eq!(
            resize_anchor_local_position(min, max, ResizeDirection::Nw, false),
            Point::new(10.0, 10.0),
            "Nw drags top-left; bottom-right stays"
        );
        assert_eq!(
            resize_anchor_local_position(min, max, ResizeDirection::Se, true),
            Point::new(5.0, 5.0),
            "Shift anchors at the center regardless of direction"
        );
    }

    /// The W handle drags the *min* side, keeping the max side fixed.
    #[test]
    fn resize_local_box_west_handle_keeps_the_right_edge_fixed() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::W,
            Vec2::new(-4.0, 0.0),
            false,
            false,
        );
        assert_eq!(resized.min, Point::new(-4.0, 0.0));
        assert_eq!(resized.max, Point::new(10.0, 10.0), "right edge fixed");
    }

    /// Acceptance criterion 6: an edge handle only changes its own
    /// perpendicular dimension.
    #[test]
    fn resize_local_box_edge_handle_changes_only_one_dimension() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(5.0, 99.0), // Y component must be ignored for an E handle.
            false,
            false,
        );
        assert_eq!(resized.max.y, 10.0);
        assert!((resized.sx - 1.5).abs() < 1e-9);
        assert!((resized.sy - 1.0).abs() < 1e-9);
    }

    /// Acceptance criterion 7: Shift anchors at the center, growing
    /// symmetrically.
    #[test]
    fn resize_local_box_shift_anchors_at_the_center() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(4.0, 0.0),
            true,
            false,
        );
        // Center (5,5) stays the center; width grows by 2*4=8 -> 18.
        assert!((resized.min.x - (-4.0)).abs() < 1e-9);
        assert!((resized.max.x - 14.0).abs() < 1e-9);
        assert_eq!(resized.min.y, 0.0);
        assert_eq!(resized.max.y, 10.0);
    }

    /// Acceptance criterion 5: Ctrl on a corner forces both axes to the
    /// dominant axis's own factor.
    #[test]
    fn resize_local_box_ctrl_corner_is_proportional_to_the_dominant_axis() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::Se,
            Vec2::new(10.0, 1.0), // X moved further: dominant.
            false,
            true,
        );
        assert!((resized.sx - 2.0).abs() < 1e-9);
        assert!((resized.sy - 2.0).abs() < 1e-9, "Y forced to match X");
        assert!((resized.max.x - 20.0).abs() < 1e-9);
        assert!((resized.max.y - 20.0).abs() < 1e-9);
    }

    /// Ctrl has no effect on an edge handle (only one axis exists to
    /// constrain).
    #[test]
    fn resize_local_box_ctrl_on_an_edge_handle_has_no_extra_effect() {
        let with_ctrl = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(5.0, 0.0),
            false,
            true,
        );
        let without_ctrl = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(5.0, 0.0),
            false,
            false,
        );
        assert_eq!(with_ctrl, without_ctrl);
    }

    /// Acceptance criterion 13: a drag past the opposite edge clamps to
    /// zero width, never flips negative.
    #[test]
    fn resize_local_box_clamps_at_zero_never_flips() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(-100.0, 0.0),
            false,
            false,
        );
        assert_eq!(resized.max.x, 0.0, "clamped, not flipped");
        assert_eq!(resized.min.x, 0.0, "anchor side untouched");
        assert!((resized.sx - 0.0).abs() < 1e-9);
    }

    /// Shift + Ctrl combine: proportional, from the center.
    #[test]
    fn resize_local_box_shift_and_ctrl_combine() {
        let resized = resize_local_box(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            ResizeDirection::Se,
            Vec2::new(10.0, 1.0),
            true,
            true,
        );
        // Under Shift, the center-anchored growth is double the
        // half-extent delta: X's own factor would be 3.0 (dominant,
        // |10| >= |1|), forced onto Y too.
        assert!((resized.sx - 3.0).abs() < 1e-9);
        assert!((resized.sy - 3.0).abs() < 1e-9);
        // Still centered at (5, 5).
        let center = Point::new(
            f64::midpoint(resized.min.x, resized.max.x),
            f64::midpoint(resized.min.y, resized.max.y),
        );
        assert!((center.x - 5.0).abs() < 1e-9);
        assert!((center.y - 5.0).abs() < 1e-9);
    }

    /// Acceptance criterion 8/9: `√(sx·sy)` collapses to plain `sx` when
    /// `sx == sy`.
    #[test]
    fn stroke_or_radius_factor_matches_plain_factor_when_proportional() {
        assert!((stroke_or_radius_factor(1.5, 1.5) - 1.5).abs() < 1e-9);
        assert!((stroke_or_radius_factor(4.0, 1.0) - 2.0).abs() < 1e-9);
    }

    /// Acceptance criterion 8: the stroke width never crosses to zero —
    /// it floors above it.
    #[test]
    fn scaled_and_floored_keeps_stroke_width_above_zero() {
        let width = scaled_and_floored(Length::from_mm(0.25), 0.0, Length::from_mm(0.01));
        assert!((width.as_mm() - 0.01).abs() < 1e-9);
    }

    /// Acceptance criterion 9: a corner radius may legitimately reach
    /// exactly zero.
    #[test]
    fn scaled_and_floored_allows_corner_radius_to_reach_exactly_zero() {
        let radius = scaled_and_floored(Length::from_mm(2.0), 0.0, Length::from_mm(0.0));
        assert!(radius.as_mm().abs() < 1e-9);
    }

    /// Acceptance criterion 11: a polygon/star's corner-handle drag is
    /// always a uniform radius scale.
    #[test]
    fn polygon_star_resize_factor_scales_the_radius() {
        // The Ne handle sits r√2 = 14.14 from the center; pulling it
        // 7.07 further along its diagonal puts it at 21.2 = 15·√2, so the
        // radius is 15 (factor 1.5) — the handle ends exactly under the
        // pointer.
        let diagonal = ResizeDirection::Ne
            .unit_vector()
            .normalized_to(10.0 * std::f64::consts::SQRT_2 / 2.0);
        let factor = polygon_star_resize_factor(10.0, ResizeDirection::Ne, diagonal);
        assert!((factor - 1.5).abs() < 1e-9, "factor was {factor}");
    }

    /// Criterion 12: no modifier rotates about the box center, whatever
    /// handle was grabbed.
    #[test]
    fn rotate_pivot_defaults_to_the_box_center() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        for direction in ResizeDirection::ALL_EIGHT {
            assert_eq!(rotate_pivot(&box_, direction, false), Point::new(5.0, 5.0));
        }
    }

    /// Criterion 13: under Shift the pivot is the corner diagonally
    /// opposite the grabbed corner handle, or the midpoint of the opposite
    /// side.
    #[test]
    fn rotate_pivot_under_shift_is_the_opposite_corner_or_side_midpoint() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let pivot = |direction| rotate_pivot(&box_, direction, true);
        assert_eq!(pivot(ResizeDirection::Ne), Point::new(0.0, 20.0));
        assert_eq!(pivot(ResizeDirection::Se), Point::new(0.0, 0.0));
        assert_eq!(pivot(ResizeDirection::Sw), Point::new(10.0, 0.0));
        assert_eq!(pivot(ResizeDirection::Nw), Point::new(10.0, 20.0));
        assert_eq!(pivot(ResizeDirection::N), Point::new(5.0, 20.0));
        assert_eq!(pivot(ResizeDirection::S), Point::new(5.0, 0.0));
        assert_eq!(pivot(ResizeDirection::E), Point::new(0.0, 10.0));
        assert_eq!(pivot(ResizeDirection::W), Point::new(10.0, 10.0));
    }

    /// Criterion 13: "opposite" is taken in the object's own rotated frame.
    #[test]
    fn rotate_pivot_opposite_is_taken_in_the_rotated_frame() {
        let box_ = OrientedBox {
            angle: Angle::from_radians(std::f64::consts::FRAC_PI_2),
            ..unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 10.0))
        };
        // Local Sw (0, 10) is the opposite of local Ne; it turned a quarter
        // turn about (5, 5): offset (-5, 5) becomes (-5, -5).
        let pivot = rotate_pivot(&box_, ResizeDirection::Ne, true);
        assert!(
            (pivot.x - 0.0).abs() < 1e-9 && (pivot.y - 0.0).abs() < 1e-9,
            "{pivot:?}"
        );
    }

    /// Acceptance criterion 15: dragging a quarter turn around the
    /// pivot reports a quarter-turn delta.
    #[test]
    fn rotate_delta_angle_measures_the_swept_angle() {
        let pivot = Point::new(0.0, 0.0);
        let delta = rotate_delta_angle(pivot, Point::new(1.0, 0.0), Point::new(0.0, 1.0), false);
        assert!((delta.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    /// Acceptance criterion 17: Ctrl snaps the delta to 15° increments.
    #[test]
    fn rotate_delta_angle_snaps_to_15_degrees_under_ctrl() {
        let pivot = Point::new(0.0, 0.0);
        // A swept angle of 10 degrees should snap to 15.
        let ten_degrees = 10.0_f64.to_radians();
        let current = Point::new(ten_degrees.cos(), ten_degrees.sin());
        let delta = rotate_delta_angle(pivot, Point::new(1.0, 0.0), current, true);
        assert!(
            (delta.as_radians() - 15.0_f64.to_radians()).abs() < 1e-9,
            "got {} degrees",
            delta.as_radians().to_degrees()
        );
    }
    /// Criterion 27 / flag 1: the typed size's local delta, fed back through
    /// the drag's own box arithmetic, reaches exactly that size, for every
    /// handle, with and without Shift.
    #[test]
    fn local_delta_for_size_round_trips_through_resize_local_box() {
        let start = unrotated_box(Point::new(2.0, 3.0), Point::new(12.0, 8.0));
        for direction in ResizeDirection::ALL_EIGHT {
            for shift in [false, true] {
                let delta = local_delta_for_size(&start, direction, Some(33.5), Some(4.25), shift);
                let resized =
                    resize_local_box(start.min, start.max, direction, delta, shift, false);
                let width = resized.max.x - resized.min.x;
                let height = resized.max.y - resized.min.y;
                let touches_w = !matches!(direction, ResizeDirection::N | ResizeDirection::S);
                let touches_h = !matches!(direction, ResizeDirection::E | ResizeDirection::W);
                assert!(
                    (width - if touches_w { 33.5 } else { 10.0 }).abs() < 1e-9,
                    "{direction:?} shift {shift}: width {width}"
                );
                assert!(
                    (height - if touches_h { 4.25 } else { 5.0 }).abs() < 1e-9,
                    "{direction:?} shift {shift}: height {height}"
                );
            }
        }
    }

    /// Criterion 26: the typed radius's delta maps back to exactly that
    /// radius through `polygon_star_resize_factor`.
    #[test]
    fn local_delta_for_radius_round_trips_through_the_polygon_factor() {
        for direction in CORNERS {
            let delta = local_delta_for_radius(10.0, direction, 17.5);
            let factor = polygon_star_resize_factor(10.0, direction, delta);
            assert!(
                (10.0 * factor - 17.5).abs() < 1e-9,
                "{direction:?}: {factor}"
            );
        }
    }

    const CORNERS: [ResizeDirection; 4] = [
        ResizeDirection::Ne,
        ResizeDirection::Se,
        ResizeDirection::Sw,
        ResizeDirection::Nw,
    ];
}
