//! The skew gesture's arithmetic in an oriented box's local frame
//! (`specs/object-transform-refinements/specification.md`, criteria 38-40,
//! 47): the fixed line and lever of a skew, the angle a drag resolves to and
//! the shear factor. Split out of [`crate::transform_math`], which keeps the
//! resize and rotate arithmetic.

use vecmanf_document_core::{Angle, Point};

use crate::angle_snap::snap_skew_angle;
use crate::oriented_box::OrientedBox;
use crate::transform_handle_layout::Side;

/// A skew's geometry in the box's local frame: which axis it shears along,
/// the signed lever from the fixed line to the grabbed side, and the fixed
/// line's reference point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkewFrame {
    /// Whether the shear acts along `u` (top and bottom handles: an x
    /// skew) rather than `v`.
    pub along_u: bool,
    /// The local coordinate of the grabbed side minus the fixed line's,
    /// across the shear axis (`c_g − c_0`): signed, half the side distance
    /// under Shift.
    pub lever: f64,
    /// A point on the fixed line, in document space: the fixed edge's
    /// midpoint, or the box center under Shift.
    pub fixed_point: Point,
}

/// The skew frame of a drag of `side` on `box_` (criteria 38, 39, 40).
#[must_use]
pub fn skew_frame(box_: &OrientedBox, side: Side, shift: bool) -> SkewFrame {
    let center = box_.local_center();
    let (grabbed, fixed, fixed_local) = match side {
        Side::Top => {
            let fixed = if shift { center.y } else { box_.max.y };
            (box_.min.y, fixed, Point::new(center.x, fixed))
        }
        Side::Bottom => {
            let fixed = if shift { center.y } else { box_.min.y };
            (box_.max.y, fixed, Point::new(center.x, fixed))
        }
        Side::Left => {
            let fixed = if shift { center.x } else { box_.max.x };
            (box_.min.x, fixed, Point::new(fixed, center.y))
        }
        Side::Right => {
            let fixed = if shift { center.x } else { box_.min.x };
            (box_.max.x, fixed, Point::new(fixed, center.y))
        }
    };
    SkewFrame {
        along_u: side.skews_along_u(),
        lever: grabbed - fixed,
        fixed_point: box_.to_document(fixed_local),
    }
}

/// The smallest lever (millimetres) a skew resolves against: below it the
/// drag is "no change" (the UI hides such handles at 24 px anyway).
pub(crate) const MIN_SKEW_LEVER_MM: f64 = 1e-6;

/// The skew angle of a drag (criteria 38, 40, 47): `atan(d / |lever|)` with
/// `d` the pointer's local displacement along the grabbed side's direction,
/// so it stays strictly inside ±90°; snapped under `ctrl` to the 15°/22.5°
/// stops and capped at ±75°. `0` for a degenerate lever or non-finite
/// pointer. Positive in the direction of the displacement.
#[must_use]
pub fn skew_angle(
    start_box: &OrientedBox,
    side: Side,
    down_at: Point,
    current: Point,
    shift: bool,
    ctrl: bool,
) -> Angle {
    let frame = skew_frame(start_box, side, shift);
    let local_delta = start_box
        .to_local(down_at)
        .vector_to(start_box.to_local(current));
    let displacement = if frame.along_u {
        local_delta.x
    } else {
        local_delta.y
    };
    if frame.lever.abs() < MIN_SKEW_LEVER_MM || !displacement.is_finite() {
        return Angle::from_radians(0.0);
    }
    let raw = Angle::from_radians((displacement / frame.lever.abs()).atan());
    if ctrl { snap_skew_angle(raw) } else { raw }
}

/// The shear factor `k` for `angle` on `frame`: `d / lever` with
/// `d = |lever| · tan(angle)`, so `k = ±tan(angle)`; an exact `0` for a zero
/// angle (the caller then returns the start snapshot unchanged).
#[must_use]
pub fn skew_factor(frame: &SkewFrame, angle: Angle) -> f64 {
    if frame.lever.abs() < MIN_SKEW_LEVER_MM {
        return 0.0;
    }
    angle.as_radians().tan() * frame.lever.signum()
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

    /// Criterion 38: the angle is `atan(d / h)` with `h` the distance from
    /// the fixed edge to the grabbed side, and stays inside ±90°.
    #[test]
    fn skew_angle_is_the_arctangent_of_the_displacement_over_the_lever() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let down = Point::new(5.0, 0.0);
        let angle = skew_angle(&box_, Side::Top, down, Point::new(25.0, 3.0), false, false);
        assert!(
            (angle.as_radians() - 1.0_f64.atan()).abs() < 1e-12,
            "d = 20, h = 20"
        );
        let far = skew_angle(&box_, Side::Top, down, Point::new(1e9, 0.0), false, false);
        assert!(far.as_radians() < std::f64::consts::FRAC_PI_2);
        let left = skew_angle(
            &box_,
            Side::Left,
            Point::new(0.0, 10.0),
            Point::new(-4.0, 15.0),
            false,
            false,
        );
        assert!(
            (left.as_radians() - (5.0_f64 / 10.0).atan()).abs() < 1e-12,
            "y skew: d = 5, h = 10"
        );
    }

    /// Criterion 39: under Shift the lever is half the side distance.
    #[test]
    fn skew_angle_under_shift_uses_half_the_side_distance() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let down = Point::new(5.0, 0.0);
        let angle = skew_angle(&box_, Side::Top, down, Point::new(15.0, 0.0), true, false);
        assert!(
            (angle.as_radians() - 1.0_f64.atan()).abs() < 1e-12,
            "d = 10, h = 10"
        );
    }

    /// Criterion 47: Ctrl snaps to the stops and caps at ±75°.
    #[test]
    fn skew_angle_under_ctrl_snaps_and_caps() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let down = Point::new(5.0, 0.0);
        let snapped = skew_angle(
            &box_,
            Side::Bottom,
            Point::new(5.0, 20.0),
            Point::new(5.0 + 20.0 * 19.0_f64.to_radians().tan(), 20.0),
            false,
            true,
        );
        assert!((snapped.as_radians().to_degrees() - 22.5).abs() < 1e-9);
        let capped = skew_angle(
            &box_,
            Side::Top,
            down,
            Point::new(5.0 + 20.0 * 85.0_f64.to_radians().tan(), 0.0),
            false,
            true,
        );
        assert!((capped.as_radians().to_degrees() - 75.0).abs() < 1e-9);
        let negative = skew_angle(
            &box_,
            Side::Top,
            down,
            Point::new(5.0 - 20.0 * 85.0_f64.to_radians().tan(), 0.0),
            false,
            true,
        );
        assert!((negative.as_radians().to_degrees() + 75.0).abs() < 1e-9);
    }

    /// The factor moves the grabbed side by `d` and leaves the fixed line:
    /// top moves by +d for a positive displacement even though its lever
    /// is negative.
    #[test]
    fn skew_factor_signs_move_the_grabbed_side_with_the_pointer() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        let top = skew_frame(&box_, Side::Top, false);
        let bottom = skew_frame(&box_, Side::Bottom, false);
        let alpha = Angle::from_radians(0.5);
        // x' = x + k (y - y0): top (y = 0) from fixed y0 = 20.
        let top_shift = skew_factor(&top, alpha) * (0.0 - 20.0);
        let bottom_shift = skew_factor(&bottom, alpha) * (20.0 - 0.0);
        assert!((top_shift - 20.0 * 0.5_f64.tan()).abs() < 1e-12);
        assert!((bottom_shift - 20.0 * 0.5_f64.tan()).abs() < 1e-12);
        assert_eq!(skew_factor(&top, Angle::from_radians(0.0)), 0.0);
    }

    /// Criterion 40: the fixed line's reference point is the fixed edge's
    /// midpoint, or the box center under Shift.
    #[test]
    fn skew_frame_reports_the_fixed_point() {
        let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        assert_eq!(
            skew_frame(&box_, Side::Top, false).fixed_point,
            Point::new(5.0, 20.0)
        );
        assert_eq!(
            skew_frame(&box_, Side::Left, false).fixed_point,
            Point::new(10.0, 10.0)
        );
        assert_eq!(
            skew_frame(&box_, Side::Top, true).fixed_point,
            Point::new(5.0, 10.0)
        );
        assert_eq!(
            skew_frame(&box_, Side::Right, true).fixed_point,
            Point::new(5.0, 10.0)
        );
    }

    /// A zero-lever box resolves to no skew.
    #[test]
    fn a_zero_lever_is_no_change() {
        let flat = unrotated_box(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let angle = skew_angle(
            &flat,
            Side::Top,
            Point::new(5.0, 0.0),
            Point::new(50.0, 0.0),
            false,
            false,
        );
        assert!(angle.as_radians().abs() < 1e-15);
    }
}
