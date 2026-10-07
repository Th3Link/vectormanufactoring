//! Black-box + property tests for `curvyo-geometry-core`'s slice of
//! `specs/0002-path-node-editing/specification.md` (acceptance criteria 12, 14,
//! and the segment hit-testing that backs AC7/AC10 selection), written
//! against this crate's public API only (`nearest_point_on_segment`,
//! `subdivide_at_parameter`), independent of `curvyo-ui-core`'s and
//! `curvyo-document-core`'s own inline unit tests of the same functions.
//!
//! Also exercises known closed-form Bézier values (the de Casteljau
//! subdivision identity, and a pure quadratic-shaped cubic's exact
//! midpoint) and property-based invariants across random inputs
//! (`CLAUDE.md`'s tester role: "property tests with `proptest` for
//! invariants").
//!
//! `flatten_segment` was removed from this crate (architect review,
//! `specs/0002-path-node-editing/adrs.md`'s "Architect review notes": it had no
//! production caller, hit-testing landed on `nearest_point_on_segment`
//! instead), so its tests below were removed along with it
//! (`CLAUDE.md` §5, "delete dead code").

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{Length, Point, Tolerance, Vec2};
use curvyo_geometry_core::{nearest_point_on_segment, subdivide_at_parameter};
use proptest::prelude::*;

const TOLERANCE: Tolerance = Tolerance::from_mm(0.01);

fn finite_mm() -> impl Strategy<Value = f64> {
    -500.0..500.0f64
}

fn point_strategy() -> impl Strategy<Value = Point> {
    (finite_mm(), finite_mm()).prop_map(|(x, y)| Point::new(x, y))
}

fn vec2_strategy() -> impl Strategy<Value = Vec2> {
    (finite_mm(), finite_mm()).prop_map(|(x, y)| Vec2::new(x, y))
}

/// Closed-form check: a cubic Bézier built so its control points coincide
/// with the classic `(0,0) (0,8) (10,8) (10,0)` "C" shape has a known exact
/// midpoint (de Casteljau by hand): x=5, y=6.
#[test]
fn known_closed_form_midpoint_of_a_symmetric_cubic() {
    let (t, distance, point) = nearest_point_on_segment(
        Point::new(0.0, 0.0),
        Vec2::new(0.0, 8.0),
        Vec2::new(0.0, 8.0),
        Point::new(10.0, 0.0),
        Point::new(5.0, 6.0),
        TOLERANCE,
    );
    assert!((t - 0.5).abs() < 1e-3, "t={t}");
    assert!(distance.as_mm() < 1e-3);
    assert!((point.x - 5.0).abs() < 1e-3 && (point.y - 6.0).abs() < 1e-3);
}

/// AC14 / AC12: subdividing at `t=0` leaves the new anchor exactly at the
/// start point (closed-form boundary case kurbo's own de Casteljau must
/// honor exactly, not just approximately).
#[test]
fn subdivide_at_t_zero_lands_exactly_on_the_start_point() {
    let start = Point::new(2.0, 3.0);
    let end = Point::new(12.0, 3.0);
    let subdivision =
        subdivide_at_parameter(start, Vec2::new(0.0, 5.0), Vec2::new(0.0, 5.0), end, 0.0);
    assert!((subdivision.new_point.x - start.x).abs() < 1e-9);
    assert!((subdivision.new_point.y - start.y).abs() < 1e-9);
}

/// Symmetric boundary case: `t=1` lands exactly on the end point.
#[test]
fn subdivide_at_t_one_lands_exactly_on_the_end_point() {
    let start = Point::new(2.0, 3.0);
    let end = Point::new(12.0, 3.0);
    let subdivision =
        subdivide_at_parameter(start, Vec2::new(0.0, 5.0), Vec2::new(0.0, 5.0), end, 1.0);
    assert!((subdivision.new_point.x - end.x).abs() < 1e-9);
    assert!((subdivision.new_point.y - end.y).abs() < 1e-9);
}

/// `nearest_point_on_segment` on a degenerate (zero-length, zero-handle)
/// segment returns the one point available, not NaN/garbage.
#[test]
fn nearest_point_on_a_degenerate_segment_is_finite() {
    let (t, distance, point) = nearest_point_on_segment(
        Point::new(3.0, 3.0),
        Vec2::ZERO,
        Vec2::ZERO,
        Point::new(3.0, 3.0),
        Point::new(100.0, -40.0),
        TOLERANCE,
    );
    assert!(t.is_finite());
    assert!(distance.as_mm().is_finite());
    assert!(point.x.is_finite() && point.y.is_finite());
}

proptest! {
    /// Property: `nearest_point_on_segment` never reports a distance
    /// larger than the straight-line distance from the query point to
    /// either endpoint (the curve always contains at least its own
    /// endpoints, so the true nearest point can only be closer).
    #[test]
    fn nearest_point_distance_never_exceeds_distance_to_either_endpoint(
        start in point_strategy(),
        h_out in vec2_strategy(),
        h_in in vec2_strategy(),
        end in point_strategy(),
        query in point_strategy(),
    ) {
        let (_, distance, _) = nearest_point_on_segment(start, h_out, h_in, end, query, TOLERANCE);
        let to_start = start.vector_to(query).length();
        let to_end = end.vector_to(query).length();
        prop_assert!(distance.as_mm() <= to_start.max(to_end) + 1e-6);
    }

    /// Property: `nearest_point_on_segment`'s returned parameter `t` is
    /// always within the segment's own domain `[0, 1]` — a caller (e.g.
    /// AC12's insert-on-double-click) subdivides at this `t` and an
    /// out-of-range value would subdivide outside the segment entirely.
    #[test]
    fn nearest_point_parameter_is_always_in_unit_range(
        start in point_strategy(),
        h_out in vec2_strategy(),
        h_in in vec2_strategy(),
        end in point_strategy(),
        query in point_strategy(),
    ) {
        let (t, _, _) = nearest_point_on_segment(start, h_out, h_in, end, query, TOLERANCE);
        prop_assert!((0.0..=1.0).contains(&t), "t={t} out of range");
    }

    /// Property (AC12/AC14's "shape unchanged at the instant of
    /// insertion"): subdividing at any `t` in (0, 1) produces a split
    /// point that lies exactly on the original curve at that parameter,
    /// for every random segment and split point, not just the hand-picked
    /// cases in the inline unit tests.
    #[test]
    fn subdivide_new_point_always_lies_on_the_original_curve(
        start in point_strategy(),
        h_out in vec2_strategy(),
        h_in in vec2_strategy(),
        end in point_strategy(),
        t in 0.05f64..0.95,
    ) {
        let subdivision = subdivide_at_parameter(start, h_out, h_in, end, t);

        // Recompute the exact point at `t` on the original curve via
        // `nearest_point_on_segment` queried AT that exact split point:
        // the reported distance must be ~0 (the split point is genuinely
        // on the curve, not just near it).
        let (_, distance, _) = nearest_point_on_segment(
            start, h_out, h_in, end, subdivision.new_point, Tolerance::from_mm(1e-6),
        );
        prop_assert!(distance.as_mm() < 1e-4, "split point not on original curve: {distance:?}");
    }

    /// Property: subdividing a *line* (both handles exactly zero) always
    /// yields a new anchor exactly on the straight chord, with every
    /// returned handle still exactly zero — a maker inserting a node on a
    /// straight segment must always get a straight segment back, for
    /// every `t` and every pair of endpoints, not just the one example in
    /// the inline unit test.
    #[test]
    fn subdividing_a_line_always_stays_a_line(
        start in point_strategy(),
        end in point_strategy(),
        t in 0.0f64..=1.0,
    ) {
        let subdivision = subdivide_at_parameter(start, Vec2::ZERO, Vec2::ZERO, end, t);
        prop_assert_eq!(subdivision.prev_out, Vec2::ZERO);
        prop_assert_eq!(subdivision.new_handle_in, Vec2::ZERO);
        prop_assert_eq!(subdivision.new_handle_out, Vec2::ZERO);
        prop_assert_eq!(subdivision.next_in, Vec2::ZERO);

        let expected = start.translated(start.vector_to(end).scaled(t));
        prop_assert!((subdivision.new_point.x - expected.x).abs() < 1e-6);
        prop_assert!((subdivision.new_point.y - expected.y).abs() < 1e-6);
    }
}

/// Sanity: `Length`/`Tolerance`/`Point`/`Vec2` are the only types crossing
/// this crate's public API (`CLAUDE.md` §5 "units are types") — a
/// compile-time check that a bare `f64` cannot be passed where a typed
/// value is required. This test exists purely so a future accidental
/// widening of the API (e.g. `fn nearest_point_on_segment(x: f64, y: f64,
/// ...)`) shows up as a diff here, not just as a documentation claim.
#[test]
fn public_api_uses_typed_units_not_bare_f64() {
    type Nearest = fn(Point, Vec2, Vec2, Point, Point, Tolerance) -> (f64, Length, Point);
    let _: Nearest = nearest_point_on_segment;
}
