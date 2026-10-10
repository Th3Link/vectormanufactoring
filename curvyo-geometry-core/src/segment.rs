//! Pure geometry over one path segment's resolved control points — the
//! two operations the path/node crate boundary assigns to this crate
//! because they need to know what a cubic Bézier is
//! (`specs/0002-path-node-editing/adrs.md`, "the path/node crate boundary"):
//! nearest-point-on-segment and de Casteljau subdivision. `kurbo` per
//! ADR 0003 §2; no `kurbo` type crosses out of this module's public
//! functions.
//!
//! A third operation, flatten-for-hit-test, was removed (architect review,
//! `specs/0002-path-node-editing/adrs.md`'s "Architect review notes"):
//! hit-testing landed on [`nearest_point_on_segment`] instead, so
//! flattening had no production caller (`CLAUDE.md` §5, "delete dead
//! code").
//!
//! Every function here takes a segment as its two endpoint anchors' own
//! data — `start`/`end` points plus the two *relative* handles that face
//! this segment (`specs/0002-path-node-editing/adrs.md` decision 2) — never a
//! pre-built absolute curve, so a caller in `curvyo-ui-core` can pass a
//! [`curvyo_document_core::AnchorSnapshot`] pair's fields straight
//! through without converting anything itself.

use curvyo_document_core::{Length, Point, Tolerance, Vec2};
use kurbo::{CubicBez, ParamCurve, ParamCurveExtrema, ParamCurveNearest, Point as KurboPoint};

/// Search accuracy handed to `kurbo`'s nearest-point query, in millimetres:
/// far below any hit radius or machine tolerance, so the reported distance
/// is the true distance for all practical purposes.
const NEAREST_ACCURACY_MM: f64 = 1e-6;

fn to_kurbo(point: Point) -> KurboPoint {
    KurboPoint::new(point.x, point.y)
}

fn from_kurbo(point: KurboPoint) -> Point {
    Point::new(point.x, point.y)
}

/// Builds the four absolute control points of one segment from its two
/// endpoint anchors' own data.
fn cubic_bez(start: Point, start_handle_out: Vec2, end_handle_in: Vec2, end: Point) -> CubicBez {
    let c1 = start.translated(start_handle_out);
    let c2 = end.translated(end_handle_in);
    CubicBez::new(to_kurbo(start), to_kurbo(c1), to_kurbo(c2), to_kurbo(end))
}

/// Finds the parameter, distance and point on one path segment nearest to
/// `query` — acceptance criterion 12's "double-click a point on a
/// segment" and acceptance criterion 14's "click on a point of a
/// segment".
///
/// `tolerance` is an upper bound on the search's own internal accuracy
/// (handed to `kurbo` as `min(tolerance, NEAREST_ACCURACY_MM)`), not a
/// maximum-distance cutoff. A coarse caller tolerance (a hit radius of
/// several mm) must not coarsen the search: `kurbo` may return a point up
/// to `accuracy` away from the true nearest one, so a coarse accuracy made
/// on-curve queries report distances of several mm. Callers decide
/// whether the returned distance counts as "on the segment" themselves,
/// against their own (typically coarser, screen-space-derived) tolerance.
#[must_use]
pub fn nearest_point_on_segment(
    start: Point,
    start_handle_out: Vec2,
    end_handle_in: Vec2,
    end: Point,
    query: Point,
    tolerance: Tolerance,
) -> (f64, Length, Point) {
    let cubic = cubic_bez(start, start_handle_out, end_handle_in, end);
    let nearest = cubic.nearest(to_kurbo(query), tolerance.as_mm().min(NEAREST_ACCURACY_MM));
    let point = from_kurbo(cubic.eval(nearest.t));
    (
        nearest.t,
        Length::from_mm(nearest.distance_sq.sqrt()),
        point,
    )
}

/// The exact axis-aligned bounding box of one cubic Bézier segment — its
/// curve extrema, not its (looser) control-point hull
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "a path's box must
/// be tight... and tight means curve extrema"). Returns `(min, max)`
/// corners. `curvyo-ui-core::object_bounds` unions this over every segment
/// of a path to get the whole path's selection-box bounds (acceptance
/// criterion 14).
#[must_use]
pub fn segment_bounds(
    start: Point,
    start_handle_out: Vec2,
    end_handle_in: Vec2,
    end: Point,
) -> (Point, Point) {
    let cubic = cubic_bez(start, start_handle_out, end_handle_in, end);
    let bbox = cubic.bounding_box();
    (Point::new(bbox.x0, bbox.y0), Point::new(bbox.x1, bbox.y1))
}

/// A handle shorter than this (millimetres) is a retracted handle: a segment whose two handles
/// are both within it is a straight line (`specs/0031-segment-drag-bending` criterion 7).
const LINE_HANDLE_TOLERANCE: Tolerance = Tolerance::from_mm(1e-9);

/// The grab parameter is clamped to this range so the handles never run away when the maker
/// grabs next to a node (`0031` criterion 10): `k` is then at most 2.4.
const GRAB_T_MIN: f64 = 1.0 / 6.0;
const GRAB_T_MAX: f64 = 5.0 / 6.0;

/// The two handles a bend writes, each relative to its own anchor
/// (`specs/0031-segment-drag-bending` criterion 8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BentHandles {
    /// The start anchor's new outgoing handle.
    pub start_handle_out: Vec2,
    /// The end anchor's new incoming handle.
    pub end_handle_in: Vec2,
}

/// The new handles of a segment dragged by `displacement` from the point at parameter `grab_t`
/// (`specs/0031-segment-drag-bending` criteria 7 to 11).
///
/// A segment whose two handles are both retracted is a line and is treated as the cubic with its
/// control points at a third and two thirds of the chord (the same shape). With `t` the grab
/// parameter clamped to `[1/6, 5/6]` and `k = 1 / (3 t (1 - t))`, both control points move by
/// `k * displacement`, so no node moves, and the curve point at `grab_t` is carried by the
/// displacement when `grab_t` lies in the clamped range.
///
/// `None` for a zero-length line (both handles retracted and the endpoints coincident): it has no
/// grab point.
#[must_use]
pub fn bend_segment_handles(
    start: Point,
    start_handle_out: Vec2,
    end_handle_in: Vec2,
    end: Point,
    grab_t: f64,
    displacement: Vec2,
) -> Option<BentHandles> {
    let retracted = |handle: Vec2| handle.length() <= LINE_HANDLE_TOLERANCE.as_mm();
    let chord = start.vector_to(end);
    let (out, handle_in) = if retracted(start_handle_out) && retracted(end_handle_in) {
        if chord.length() <= LINE_HANDLE_TOLERANCE.as_mm() {
            return None;
        }
        (chord.scaled(1.0 / 3.0), chord.scaled(-1.0 / 3.0))
    } else {
        (start_handle_out, end_handle_in)
    };
    let t = grab_t.clamp(GRAB_T_MIN, GRAB_T_MAX);
    let push = displacement.scaled(1.0 / (3.0 * t * (1.0 - t)));
    Some(BentHandles {
        start_handle_out: out + push,
        end_handle_in: handle_in + push,
    })
}

/// The caller-resolved geometry of splitting one segment at a parameter —
/// acceptance criterion 12; every field is already relative to its own
/// anchor, ready to pass straight into
/// [`curvyo_document_core::Document::insert_anchor`]
/// (`specs/0002-path-node-editing/adrs.md`, "commands carry resolved geometry,
/// never geometric intent").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Subdivision {
    /// The segment's start anchor's new `handle_out`.
    pub prev_out: Vec2,
    /// The new anchor's absolute position.
    pub new_point: Point,
    /// The new anchor's `handle_in`, relative to `new_point`.
    pub new_handle_in: Vec2,
    /// The new anchor's `handle_out`, relative to `new_point`.
    pub new_handle_out: Vec2,
    /// The segment's end anchor's new `handle_in`.
    pub next_in: Vec2,
}

/// Subdivides one path segment at `t` via de Casteljau (acceptance
/// criterion 12), leaving the path's visible shape unchanged at the
/// instant of insertion — the two resulting sub-curves retrace the
/// original one exactly.
///
/// A segment whose two handles are both already the exact zero vector —
/// a line (`specs/0002-path-node-editing/adrs.md` decision 2) — is split along
/// the straight chord directly rather than through `kurbo`'s de
/// Casteljau: a cubic "disguised" as a line (`p1 == p0`, `p2 == p3`)
/// still subdivides to the right *point*, but its new control handles
/// come out collinear rather than exactly zero, which would silently
/// stop reading as a line under this slice's own "a retracted handle is
/// the exact zero vector, no `is_line` flag exists" rule. Splitting the
/// line directly keeps both new segments lines, as a maker inserting a
/// node on a straight segment expects.
#[must_use]
pub fn subdivide_at_parameter(
    start: Point,
    start_handle_out: Vec2,
    end_handle_in: Vec2,
    end: Point,
    t: f64,
) -> Subdivision {
    if start_handle_out == Vec2::ZERO && end_handle_in == Vec2::ZERO {
        return Subdivision {
            prev_out: Vec2::ZERO,
            new_point: start.translated(start.vector_to(end).scaled(t)),
            new_handle_in: Vec2::ZERO,
            new_handle_out: Vec2::ZERO,
            next_in: Vec2::ZERO,
        };
    }

    let cubic = cubic_bez(start, start_handle_out, end_handle_in, end);
    let left = cubic.subsegment(0.0..t);
    let right = cubic.subsegment(t..1.0);
    let new_point = from_kurbo(left.p3);

    Subdivision {
        prev_out: start.vector_to(from_kurbo(left.p1)),
        new_point,
        new_handle_in: new_point.vector_to(from_kurbo(left.p2)),
        new_handle_out: new_point.vector_to(from_kurbo(right.p1)),
        next_in: end.vector_to(from_kurbo(right.p2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: Tolerance = Tolerance::from_mm(0.01);

    #[test]
    fn nearest_point_on_a_straight_segment_is_the_perpendicular_projection() {
        let (t, distance, point) = nearest_point_on_segment(
            Point::new(0.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Point::new(10.0, 0.0),
            Point::new(5.0, 2.0),
            TOLERANCE,
        );
        assert!((t - 0.5).abs() < 1e-3);
        assert!((distance.as_mm() - 2.0).abs() < 1e-3);
        assert!((point.x - 5.0).abs() < 1e-3 && point.y.abs() < 1e-3);
    }

    #[test]
    fn nearest_point_at_an_endpoint_is_that_endpoint() {
        let (t, distance, point) = nearest_point_on_segment(
            Point::new(0.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Point::new(10.0, 0.0),
            Point::new(0.0, 0.0),
            TOLERANCE,
        );
        assert!(t.abs() < 1e-6);
        assert!(distance.as_mm() < 1e-6);
        assert_eq!(point, Point::new(0.0, 0.0));
    }

    /// Hit-test tolerances seen in practice (mm): about 1 mm at 283 % zoom
    /// up to a coarse 5 mm, plus the old tiny test value.
    const HIT_TOLERANCES_MM: [f64; 5] = [0.01, 0.35, 1.06, 2.5, 5.0];

    /// Regression: the caller's hit tolerance used to be handed to `kurbo`
    /// as its search accuracy, so a point exactly on a 40 mm straight
    /// segment reported a distance of several mm unless it sat near
    /// t = 0, 0.5 or 1.
    #[test]
    fn points_on_a_straight_segment_report_zero_distance_at_any_tolerance() {
        let start = Point::new(0.0, 0.0);
        let end = Point::new(40.0, 0.0);
        for tolerance_mm in HIT_TOLERANCES_MM {
            for step in 0..=39 {
                let x = f64::from(step) + 0.37;
                let (_, distance, point) = nearest_point_on_segment(
                    start,
                    Vec2::ZERO,
                    Vec2::ZERO,
                    end,
                    Point::new(x, 0.0),
                    Tolerance::from_mm(tolerance_mm),
                );
                assert!(
                    distance.as_mm() < 1e-3,
                    "x={x} tolerance={tolerance_mm}: distance {}",
                    distance.as_mm()
                );
                assert!((point.x - x).abs() < 1e-3);
            }
        }
    }

    /// The same sweep on a curved segment: every point sampled from the
    /// curve itself must come back at distance ~0.
    #[test]
    fn points_on_a_curved_segment_report_zero_distance_at_any_tolerance() {
        let start = Point::new(0.0, 0.0);
        let start_handle_out = Vec2::new(0.0, 15.0);
        let end_handle_in = Vec2::new(0.0, 15.0);
        let end = Point::new(40.0, 0.0);
        let curve = cubic_bez(start, start_handle_out, end_handle_in, end);
        for tolerance_mm in HIT_TOLERANCES_MM {
            for step in 0..=40 {
                let on_curve = from_kurbo(curve.eval(f64::from(step) / 40.0));
                let (_, distance, _) = nearest_point_on_segment(
                    start,
                    start_handle_out,
                    end_handle_in,
                    end,
                    on_curve,
                    Tolerance::from_mm(tolerance_mm),
                );
                assert!(
                    distance.as_mm() < 1e-3,
                    "step={step} tolerance={tolerance_mm}: distance {}",
                    distance.as_mm()
                );
            }
        }
    }

    /// Off-curve points still report their true distance, so a caller's
    /// "distance <= tolerance" comparison keeps rejecting them.
    #[test]
    fn points_off_a_straight_segment_report_their_true_distance_at_any_tolerance() {
        let start = Point::new(0.0, 0.0);
        let end = Point::new(40.0, 0.0);
        for tolerance_mm in HIT_TOLERANCES_MM {
            for step in 0..=39 {
                let x = f64::from(step) + 0.37;
                let off = tolerance_mm + 0.5;
                let (_, distance, _) = nearest_point_on_segment(
                    start,
                    Vec2::ZERO,
                    Vec2::ZERO,
                    end,
                    Point::new(x, off),
                    Tolerance::from_mm(tolerance_mm),
                );
                assert!(
                    (distance.as_mm() - off).abs() < 1e-3,
                    "x={x} tolerance={tolerance_mm}: distance {} != {off}",
                    distance.as_mm()
                );
            }
        }
    }

    #[test]
    fn segment_bounds_of_a_straight_horizontal_segment_is_its_own_extent() {
        let (min, max) = segment_bounds(
            Point::new(0.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Point::new(10.0, 0.0),
        );
        assert_eq!(min, Point::new(0.0, 0.0));
        assert_eq!(max, Point::new(10.0, 0.0));
    }

    /// The tight-bounds case the control-point hull would get wrong: a
    /// curve whose handles bulge well outside the straight line between
    /// its two endpoints must report the *curve's* own extent, not just
    /// the two endpoints.
    #[test]
    fn segment_bounds_of_a_curved_segment_includes_its_bulge() {
        let (min, max) = segment_bounds(
            Point::new(0.0, 0.0),
            Vec2::new(0.0, 20.0),
            Vec2::new(0.0, 20.0),
            Point::new(10.0, 0.0),
        );
        // Both endpoints sit at y=0, but the curve bulges downward
        // (Y-down document space) well past y=0 toward the handles.
        assert!(max.y > 5.0, "the curve's bulge must be included: {max:?}");
        assert!(min.x <= 0.0 && max.x >= 10.0);
    }

    #[test]
    fn subdivide_a_straight_segment_at_its_midpoint() {
        let subdivision = subdivide_at_parameter(
            Point::new(0.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Point::new(10.0, 0.0),
            0.5,
        );
        assert!((subdivision.new_point.x - 5.0).abs() < 1e-9);
        assert!(subdivision.new_point.y.abs() < 1e-9);
        // A straight segment subdivides into two more straight segments:
        // every new handle is the zero vector.
        assert_eq!(subdivision.prev_out, Vec2::ZERO);
        assert_eq!(subdivision.new_handle_in, Vec2::ZERO);
        assert_eq!(subdivision.new_handle_out, Vec2::ZERO);
        assert_eq!(subdivision.next_in, Vec2::ZERO);
    }

    #[test]
    fn subdivide_preserves_the_curve_shape_at_the_instant_of_insertion() {
        let start = Point::new(0.0, 0.0);
        let start_handle_out = Vec2::new(0.0, 8.0);
        let end = Point::new(10.0, 0.0);
        let end_handle_in = Vec2::new(0.0, 8.0);
        let t = 0.37;

        let subdivision = subdivide_at_parameter(start, start_handle_out, end_handle_in, end, t);
        let original = cubic_bez(start, start_handle_out, end_handle_in, end);

        // The new anchor sits exactly on the original curve at `t`.
        let expected_split_point = from_kurbo(original.eval(t));
        assert!((subdivision.new_point.x - expected_split_point.x).abs() < 1e-9);
        assert!((subdivision.new_point.y - expected_split_point.y).abs() < 1e-9);

        // Rebuilding the two sub-curves from the returned (anchor-
        // relative) handles and evaluating each at its own local midpoint
        // must land exactly on the original curve's point at the
        // corresponding global parameter — the shape at the instant of
        // insertion is unchanged, not merely close.
        let left = cubic_bez(
            start,
            subdivision.prev_out,
            subdivision.new_handle_in,
            subdivision.new_point,
        );
        let left_mid = from_kurbo(left.eval(0.5));
        let expected_left_mid = from_kurbo(original.eval(t * 0.5));
        assert!((left_mid.x - expected_left_mid.x).abs() < 1e-9);
        assert!((left_mid.y - expected_left_mid.y).abs() < 1e-9);

        let right = cubic_bez(
            subdivision.new_point,
            subdivision.new_handle_out,
            subdivision.next_in,
            end,
        );
        let right_mid = from_kurbo(right.eval(0.5));
        let expected_right_mid = from_kurbo(original.eval(t + (1.0 - t) * 0.5));
        assert!((right_mid.x - expected_right_mid.x).abs() < 1e-9);
        assert!((right_mid.y - expected_right_mid.y).abs() < 1e-9);
    }
}
