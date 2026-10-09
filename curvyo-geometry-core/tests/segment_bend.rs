//! The shape rule of a segment bend (`specs/0031-segment-drag-bending` criteria 7 to 11): the
//! numbers of the specification as a table, and a property test that the bent curve passes
//! through the grabbed point carried by the displacement.

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::{bend_segment_handles, nearest_point_on_segment};
use proptest::prelude::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// The curve point at `t` of the cubic given by an anchor pair's own data.
fn point_at(start: Point, out: Vec2, handle_in: Vec2, end: Point, t: f64) -> Point {
    let p1 = start.translated(out);
    let p2 = end.translated(handle_in);
    let u = 1.0 - t;
    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    pt(
        b0 * start.x + b1 * p1.x + b2 * p2.x + b3 * end.x,
        b0 * start.y + b1 * p1.y + b2 * p2.y + b3 * end.y,
    )
}

/// Criterion 8: a straight segment from (0, 0) to (90, 0), grabbed in the middle and dragged to
/// (45, -30): t0 = 0.5, k = 4/3, the handles are (30, -40) and (-30, -40).
#[test]
fn a_line_dragged_in_the_middle() {
    let bent = bend_segment_handles(
        pt(0.0, 0.0),
        Vec2::ZERO,
        Vec2::ZERO,
        pt(90.0, 0.0),
        0.5,
        v(0.0, -30.0),
    )
    .unwrap();
    assert!(close(bent.start_handle_out, v(30.0, -40.0)), "{bent:?}");
    assert!(close(bent.end_handle_in, v(-30.0, -40.0)), "{bent:?}");
    let curve = point_at(
        pt(0.0, 0.0),
        bent.start_handle_out,
        bent.end_handle_in,
        pt(90.0, 0.0),
        0.5,
    );
    assert!(
        (curve.x - 45.0).abs() < 1e-9 && (curve.y + 30.0).abs() < 1e-9,
        "{curve:?}"
    );
}

/// Criterion 9: a curve grabbed at t0 = 0.3 and dragged by (5, -12): the new curve point at t0
/// is the old one plus d.
#[test]
fn a_curve_passes_through_the_carried_point() {
    let (a, out, into, b) = (pt(0.0, 0.0), v(20.0, 40.0), v(-20.0, 40.0), pt(90.0, 0.0));
    let d = v(5.0, -12.0);
    let before = point_at(a, out, into, b, 0.3);
    let bent = bend_segment_handles(a, out, into, b, 0.3, d).unwrap();
    let after = point_at(a, bent.start_handle_out, bent.end_handle_in, b, 0.3);
    assert!((after.x - (before.x + d.x)).abs() < 1e-6);
    assert!((after.y - (before.y + d.y)).abs() < 1e-6);
}

/// Criterion 10: a grab near a node (t0 = 0.1) is clamped to 1/6, so the handles move by 2.4 d
/// at most and the curve point at t0 moves by the fraction 0.648 of d.
#[test]
fn a_grab_near_a_node_is_clamped() {
    let (a, b) = (pt(0.0, 0.0), pt(90.0, 0.0));
    let d = v(0.0, -10.0);
    let bent = bend_segment_handles(a, Vec2::ZERO, Vec2::ZERO, b, 0.1, d).unwrap();
    // The handle moves by k d = 2.4 d.
    assert!(close(bent.start_handle_out, v(30.0, -24.0)), "{bent:?}");
    let before = point_at(a, v(30.0, 0.0), v(-30.0, 0.0), b, 0.1);
    let after = point_at(a, bent.start_handle_out, bent.end_handle_in, b, 0.1);
    let fraction = (after.y - before.y) / d.y;
    assert!((fraction - 0.648).abs() < 1e-9, "{fraction}");
}

/// Criterion 11: a curve is reshaped, its stored handles are kept and moved, not replaced.
#[test]
fn a_curve_keeps_its_own_handles() {
    let bent = bend_segment_handles(
        pt(0.0, 0.0),
        v(10.0, 5.0),
        v(-12.0, 7.0),
        pt(90.0, 0.0),
        0.5,
        v(0.0, -3.0),
    )
    .unwrap();
    // k = 4/3 at t = 0.5, so the push is (0, -4).
    assert!(close(bent.start_handle_out, v(10.0, 1.0)), "{bent:?}");
    assert!(close(bent.end_handle_in, v(-12.0, 3.0)), "{bent:?}");
}

/// Criterion 7: handles within 1e-9 mm count as retracted; a zero-length line cannot be bent.
#[test]
fn a_retracted_pair_is_a_line_and_a_point_has_no_grab() {
    let tiny = v(1e-10, 0.0);
    let bent =
        bend_segment_handles(pt(0.0, 0.0), tiny, tiny, pt(90.0, 0.0), 0.5, Vec2::ZERO).unwrap();
    assert!(close(bent.start_handle_out, v(30.0, 0.0)));
    assert!(close(bent.end_handle_in, v(-30.0, 0.0)));
    assert!(
        bend_segment_handles(
            pt(5.0, 5.0),
            Vec2::ZERO,
            Vec2::ZERO,
            pt(5.0, 5.0),
            0.5,
            v(1.0, 1.0)
        )
        .is_none()
    );
    // A curve whose endpoints coincide (a loop) has a grab point.
    assert!(
        bend_segment_handles(
            pt(5.0, 5.0),
            v(10.0, 0.0),
            v(10.0, 0.0),
            pt(5.0, 5.0),
            0.5,
            v(1.0, 1.0)
        )
        .is_some()
    );
}

proptest! {
    /// For a grab parameter inside [1/6, 5/6] the bent curve passes through the old curve point
    /// at that parameter plus the displacement (within 1e-6 mm), for lines and curves.
    #[test]
    fn the_bent_curve_follows_the_displacement(
        ax in -200.0..200.0_f64, ay in -200.0..200.0_f64,
        bx in -200.0..200.0_f64, by in -200.0..200.0_f64,
        ox in -80.0..80.0_f64, oy in -80.0..80.0_f64,
        ix in -80.0..80.0_f64, iy in -80.0..80.0_f64,
        t in (1.0 / 6.0)..=(5.0 / 6.0),
        dx in -100.0..100.0_f64, dy in -100.0..100.0_f64,
        as_line in any::<bool>(),
    ) {
        let (a, b) = (pt(ax, ay), pt(bx, by));
        prop_assume!(a.vector_to(b).length() > 1e-3);
        let (out, into) = if as_line { (Vec2::ZERO, Vec2::ZERO) } else { (v(ox, oy), v(ix, iy)) };
        let d = v(dx, dy);
        let (p1, p2) = if as_line {
            (a.vector_to(b).scaled(1.0 / 3.0), a.vector_to(b).scaled(-1.0 / 3.0))
        } else {
            (out, into)
        };
        let before = point_at(a, p1, p2, b, t);
        let bent = bend_segment_handles(a, out, into, b, t, d).unwrap();
        let after = point_at(a, bent.start_handle_out, bent.end_handle_in, b, t);
        prop_assert!((after.x - (before.x + d.x)).abs() < 1e-6);
        prop_assert!((after.y - (before.y + d.y)).abs() < 1e-6);
    }
}

/// The grab parameter of a press is the nearest point's parameter (the caller's one
/// `nearest_point_on_segment` call): on a straight segment it is the chord fraction.
#[test]
fn the_grab_parameter_comes_from_the_nearest_point() {
    let (t, _, _) = nearest_point_on_segment(
        pt(0.0, 0.0),
        Vec2::ZERO,
        Vec2::ZERO,
        pt(90.0, 0.0),
        pt(45.0, 2.0),
        curvyo_document_core::Tolerance::from_mm(1e-6),
    );
    assert!((t - 0.5).abs() < 1e-6);
}
