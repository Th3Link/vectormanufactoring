//! The Ctrl angle-snap table for rotate and skew drags
//! (`specs/object-transform-refinements/specification.md`, acceptance
//! criteria 33-36, 47): the stops are the union of multiples of 15° and of
//! 22.5°, repeated through every quadrant, nearest stop wins.

use curvyo_document_core::Angle;

/// The stops inside one 45° period; the whole table is these plus `45° · k`.
/// (Multiples of 15° give 0, 15, 30, 45; multiples of 22.5° give 0, 22.5, 45.)
const STOPS_IN_PERIOD_DEG: [f64; 5] = [0.0, 15.0, 22.5, 30.0, 45.0];

/// The period of the stop table, in degrees.
const PERIOD_DEG: f64 = 45.0;

/// Two candidate stops closer than this (radians) are equidistant from the
/// raw angle: a tie, which goes to the stop nearer the start.
const TIE_EPSILON_RAD: f64 = 1e-9;

/// The largest skew a Ctrl-snapped skew may reach (degrees): the last stop
/// below ±90°, where a shear degenerates (criterion 47).
pub const MAX_SKEW_SNAP_DEG: f64 = 75.0;

/// `raw` snapped to the nearest stop of {k × 15°} ∪ {k × 22.5°} (criteria
/// 33, 34). Computed on the magnitude with the sign restored, so a negative
/// angle mirrors the positive one. At an exact midpoint between two stops
/// the one nearer zero (the object's start angle) wins. A non-finite `raw`
/// snaps to zero.
#[must_use]
pub fn snap_angle(raw: Angle) -> Angle {
    let radians = raw.as_radians();
    if !radians.is_finite() {
        return Angle::from_radians(0.0);
    }
    let sign = if radians < 0.0 { -1.0 } else { 1.0 };
    let magnitude = radians.abs();
    let period = PERIOD_DEG.to_radians();
    let whole_periods = (magnitude / period).floor();
    let within = magnitude - whole_periods * period;

    let mut best = STOPS_IN_PERIOD_DEG[0].to_radians();
    for stop in STOPS_IN_PERIOD_DEG {
        let stop = stop.to_radians();
        // Strictly nearer replaces; an equal distance keeps the earlier
        // (smaller) stop, which is the one nearer the start angle.
        if (within - stop).abs() < (within - best).abs() - TIE_EPSILON_RAD {
            best = stop;
        }
    }
    Angle::from_radians(sign * (whole_periods * period + best))
}

/// [`snap_angle`] for a skew angle, capped at ±[`MAX_SKEW_SNAP_DEG`]
/// (criterion 47): a raw angle of 80° or 89° snaps to 75°.
#[must_use]
pub fn snap_skew_angle(raw: Angle) -> Angle {
    let cap = MAX_SKEW_SNAP_DEG.to_radians();
    Angle::from_radians(snap_angle(raw).as_radians().clamp(-cap, cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapped_deg(raw_deg: f64) -> f64 {
        snap_angle(Angle::from_radians(raw_deg.to_radians()))
            .as_radians()
            .to_degrees()
    }

    fn assert_deg(actual: f64, expected: f64, context: &str) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{context}: expected {expected}, got {actual}"
        );
    }

    /// Criterion 34's ten values.
    #[test]
    fn the_ten_documented_raw_angles_snap_as_listed() {
        let cases = [
            (10.0, 15.0),
            (19.0, 22.5),
            (18.5, 15.0),
            (26.0, 22.5),
            (26.5, 30.0),
            (40.0, 45.0),
            (55.0, 60.0),
            (64.0, 67.5),
            (71.0, 67.5),
            (100.0, 105.0),
        ];
        for (raw, expected) in cases {
            assert_deg(snapped_deg(raw), expected, &format!("+{raw}"));
            assert_deg(snapped_deg(-raw), -expected, &format!("-{raw}"));
        }
    }

    /// Criterion 33: every stop of the full circle, in both directions.
    #[test]
    fn every_stop_through_all_four_quadrants_snaps_to_itself() {
        let half_turn = [
            0.0, 15.0, 22.5, 30.0, 45.0, 60.0, 67.5, 75.0, 90.0, 105.0, 112.5, 120.0, 135.0, 150.0,
            157.5, 165.0, 180.0,
        ];
        let second_half = [
            195.0, 202.5, 210.0, 225.0, 240.0, 247.5, 255.0, 270.0, 285.0, 292.5, 300.0, 315.0,
            330.0, 337.5, 345.0, 360.0,
        ];
        for stop in half_turn.into_iter().chain(second_half) {
            assert_deg(snapped_deg(stop), stop, "positive stop");
            assert_deg(snapped_deg(-stop), -stop, "negative stop");
        }
    }

    /// Criterion 33: 67.5° and 337.5° exist exactly like 22.5°.
    #[test]
    fn quadrant_symmetry_67_5_and_337_5_behave_like_22_5() {
        assert_deg(snapped_deg(66.0), 67.5, "near 67.5");
        assert_deg(snapped_deg(339.0), 337.5, "near 337.5");
        assert_deg(snapped_deg(23.0), 22.5, "near 22.5");
    }

    /// Criterion 34: on an exact midpoint the stop nearer the start wins;
    /// checked for every midpoint of every period, both signs, and either
    /// side by 1e-6 degrees.
    #[test]
    fn midpoints_go_to_the_stop_nearer_zero_and_either_side_goes_to_the_nearest() {
        // (lower stop, upper stop) within one 45 degree period.
        let gaps = [(0.0, 15.0), (15.0, 22.5), (22.5, 30.0), (30.0, 45.0)];
        for period in 0..8 {
            let base = 45.0 * f64::from(period);
            for (low, high) in gaps {
                let mid = f64::midpoint(low, high);
                for sign in [1.0, -1.0] {
                    assert_deg(
                        snapped_deg(sign * (base + mid)),
                        sign * (base + low),
                        &format!("midpoint {mid} in period {period}"),
                    );
                    assert_deg(
                        snapped_deg(sign * (base + mid - 1e-6)),
                        sign * (base + low),
                        "just below the midpoint",
                    );
                    assert_deg(
                        snapped_deg(sign * (base + mid + 1e-6)),
                        sign * (base + high),
                        "just above the midpoint",
                    );
                }
            }
        }
    }

    #[test]
    fn zero_and_half_turns_are_fixed_points() {
        assert_deg(snapped_deg(0.0), 0.0, "zero");
        assert_deg(snapped_deg(180.0), 180.0, "half turn");
        assert_deg(snapped_deg(-180.0), -180.0, "negative half turn");
        assert_deg(snapped_deg(0.4), 0.0, "tiny positive");
    }

    #[test]
    fn a_non_finite_angle_snaps_to_zero() {
        assert!(snap_angle(Angle::from_radians(f64::NAN)).as_radians().abs() < 1e-12);
        assert!(
            snap_angle(Angle::from_radians(f64::INFINITY))
                .as_radians()
                .abs()
                < 1e-12
        );
    }

    /// Criterion 47: a skew snap is capped at ±75°.
    #[test]
    fn skew_snap_is_capped_at_seventy_five_degrees() {
        let skew = |raw_deg: f64| {
            snap_skew_angle(Angle::from_radians(raw_deg.to_radians()))
                .as_radians()
                .to_degrees()
        };
        assert_deg(skew(80.0), 75.0, "80");
        assert_deg(skew(89.0), 75.0, "89");
        assert_deg(skew(-80.0), -75.0, "-80");
        assert_deg(skew(-89.9), -75.0, "-89.9");
        assert_deg(skew(22.0), 22.5, "ordinary stop");
        assert_deg(skew(-44.0), -45.0, "ordinary negative stop");
    }
}
