//! Independent acceptance tests for the shape rule of `0031-segment-drag-bending` (criteria 7 to
//! 11), written from the specification before the implementation was read. The cubic is
//! evaluated here, not by the code under test.

#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    missing_docs
)]

use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::bend_segment_handles;
use proptest::prelude::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

/// The cubic of an anchor pair at `t`, from the specification's P0..P3.
fn eval(p0: Point, out: Vec2, handle_in: Vec2, p3: Point, t: f64) -> (f64, f64) {
    let p1 = (p0.x + out.x, p0.y + out.y);
    let p2 = (p3.x + handle_in.x, p3.y + handle_in.y);
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p0.x + b * p1.0 + c * p2.0 + d * p3.x,
        a * p0.y + b * p1.1 + c * p2.1 + d * p3.y,
    )
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Criterion 8, the specification's own test.
#[test]
fn c8_line_dragged_in_the_middle_gives_the_documented_handles() {
    let r = bend_segment_handles(
        pt(0.0, 0.0),
        Vec2::ZERO,
        Vec2::ZERO,
        pt(90.0, 0.0),
        0.5,
        v(0.0, -30.0),
    )
    .expect("a line has a grab point");
    assert!(near(r.start_handle_out.x, 30.0, 1e-9) && near(r.start_handle_out.y, -40.0, 1e-9));
    assert!(near(r.end_handle_in.x, -30.0, 1e-9) && near(r.end_handle_in.y, -40.0, 1e-9));
    let (x, y) = eval(
        pt(0.0, 0.0),
        r.start_handle_out,
        r.end_handle_in,
        pt(90.0, 0.0),
        0.5,
    );
    assert!(near(x, 45.0, 1e-9) && near(y, -30.0, 1e-9), "({x}, {y})");
}

/// Criterion 7: handles of length exactly 1e-9 or less count as a line; a longer handle is used as
/// stored.
#[test]
fn c7_line_threshold_is_one_nanometre() {
    let d = v(0.0, -30.0);
    let line = bend_segment_handles(
        pt(0.0, 0.0),
        v(1e-10, 0.0),
        Vec2::ZERO,
        pt(90.0, 0.0),
        0.5,
        d,
    )
    .unwrap();
    // As a line: P1' = (30, 0) + 4/3 d.
    assert!(near(line.start_handle_out.x, 30.0, 1e-6), "{line:?}");
    assert!(near(line.start_handle_out.y, -40.0, 1e-6), "{line:?}");
    // A handle of 1e-6 is a curve: the stored handle plus k d.
    let curve = bend_segment_handles(
        pt(0.0, 0.0),
        v(1e-6, 0.0),
        Vec2::ZERO,
        pt(90.0, 0.0),
        0.5,
        d,
    )
    .unwrap();
    assert!(near(curve.start_handle_out.x, 1e-6, 1e-9), "{curve:?}");
    assert!(near(curve.start_handle_out.y, -40.0, 1e-6), "{curve:?}");
    assert!(near(curve.end_handle_in.x, 0.0, 1e-9), "{curve:?}");
}

/// Criterion 6: a segment of zero length with no handles cannot be bent.
#[test]
fn c6_zero_length_segment_has_no_grab_point() {
    assert!(
        bend_segment_handles(
            pt(5.0, 5.0),
            Vec2::ZERO,
            Vec2::ZERO,
            pt(5.0, 5.0),
            0.5,
            v(3.0, 3.0)
        )
        .is_none()
    );
}

/// Criterion 9, the specification's own numbers.
#[test]
fn c9_curve_passes_through_the_carried_point() {
    let p0 = pt(0.0, 0.0);
    let out = v(20.0, 40.0);
    let handle_in = v(-20.0, 40.0); // P2 = (70, 40)
    let p3 = pt(90.0, 0.0);
    let t0 = 0.3;
    let d = v(5.0, -12.0);
    let (ox, oy) = eval(p0, out, handle_in, p3, t0);
    let r = bend_segment_handles(p0, out, handle_in, p3, t0, d).unwrap();
    let (nx, ny) = eval(p0, r.start_handle_out, r.end_handle_in, p3, t0);
    assert!(near(nx, ox + d.x, 1e-6), "{nx} vs {}", ox + d.x);
    assert!(near(ny, oy + d.y, 1e-6), "{ny} vs {}", oy + d.y);
    // Criterion 11: stored P1 and P2 each move by k d, k = 1 / (3 t (1 - t)).
    let k = 1.0 / (3.0 * t0 * (1.0 - t0));
    assert!(near(r.start_handle_out.x, out.x + k * d.x, 1e-9));
    assert!(near(r.start_handle_out.y, out.y + k * d.y, 1e-9));
    assert!(near(r.end_handle_in.x, handle_in.x + k * d.x, 1e-9));
    assert!(near(r.end_handle_in.y, handle_in.y + k * d.y, 1e-9));
}

/// Criterion 10: t0 = 0.1 uses t = 1/6; the point at t0 moves by 0.648 d, the handles by 2.4 d.
#[test]
fn c10_grab_near_a_node_is_clamped() {
    let p0 = pt(0.0, 0.0);
    let p3 = pt(90.0, 0.0);
    let d = v(0.0, -10.0);
    let r = bend_segment_handles(p0, Vec2::ZERO, Vec2::ZERO, p3, 0.1, d).unwrap();
    // The line as a cubic: P1 = (30, 0).
    assert!(near(r.start_handle_out.x, 30.0, 1e-9));
    assert!(near(r.start_handle_out.y, -24.0, 1e-9), "{r:?}");
    assert!(near(r.end_handle_in.y, -24.0, 1e-9), "{r:?}");
    // The old line as the cubic with its control points at the thirds.
    let (ox, oy) = eval(p0, v(30.0, 0.0), v(-30.0, 0.0), p3, 0.1);
    let (nx, ny) = eval(p0, r.start_handle_out, r.end_handle_in, p3, 0.1);
    let moved = ((nx - ox).powi(2) + (ny - oy).powi(2)).sqrt();
    assert!(near(moved, 0.648 * 10.0, 1e-3), "moved {moved}");
    // Symmetric on the far side: t0 = 0.9 behaves like t = 5/6.
    let r2 = bend_segment_handles(p0, Vec2::ZERO, Vec2::ZERO, p3, 0.9, d).unwrap();
    assert!(near(r2.start_handle_out.y, -24.0, 1e-9), "{r2:?}");
}

/// The extremes t0 = 0 and t0 = 1 (a press exactly at a node) are clamped, never infinite.
#[test]
fn c10_extreme_parameters_stay_finite() {
    for t0 in [0.0, 1.0, -0.5, 1.5] {
        let r = bend_segment_handles(
            pt(0.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            pt(50.0, 0.0),
            t0,
            v(0.0, 7.0),
        )
        .unwrap();
        assert!(
            near(r.start_handle_out.y, 7.0 * 2.4, 1e-9),
            "t0 {t0}: {r:?}"
        );
        assert!(r.end_handle_in.x.is_finite() && r.end_handle_in.y.is_finite());
    }
}

/// A zero displacement returns the handles unchanged (a curve) or the line's thirds (a line);
/// callers decide whether to write.
#[test]
fn zero_displacement_keeps_a_curves_handles() {
    let out = v(10.0, 20.0);
    let handle_in = v(-5.0, 15.0);
    let r =
        bend_segment_handles(pt(0.0, 0.0), out, handle_in, pt(40.0, 0.0), 0.4, Vec2::ZERO).unwrap();
    assert!(near(r.start_handle_out.x, out.x, 1e-12) && near(r.start_handle_out.y, out.y, 1e-12));
    assert!(near(r.end_handle_in.x, handle_in.x, 1e-12));
    assert!(near(r.end_handle_in.y, handle_in.y, 1e-12));
}

fn coord() -> impl Strategy<Value = f64> {
    -500.0f64..500.0
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Criterion 9 for any curve: the curve point at t0 in [1/6, 5/6] after the bend is the old one
    /// plus d, and P0 / P3 are not part of the result (no node moves by construction).
    #[test]
    fn curve_follows_the_pointer_in_the_middle_two_thirds(
        x0 in coord(), y0 in coord(), x3 in coord(), y3 in coord(),
        ox in coord(), oy in coord(), ix in coord(), iy in coord(),
        t0 in (1.0f64 / 6.0)..=(5.0f64 / 6.0),
        dx in coord(), dy in coord(),
    ) {
        let p0 = pt(x0, y0);
        let p3 = pt(x3, y3);
        let out = v(ox, oy);
        let handle_in = v(ix, iy);
        let r = bend_segment_handles(p0, out, handle_in, p3, t0, v(dx, dy)).unwrap();
        let (a, b) = eval(p0, out, handle_in, p3, t0);
        let (c, d) = eval(p0, r.start_handle_out, r.end_handle_in, p3, t0);
        prop_assert!(near(c, a + dx, 1e-6), "x {c} vs {}", a + dx);
        prop_assert!(near(d, b + dy, 1e-6), "y {d} vs {}", b + dy);
    }

    /// Criterion 10: outside [1/6, 5/6] the handles move by at most 2.4 d, and the fraction of d the
    /// curve point at t0 moves by is 3 t0 (1 - t0) / (3 t (1 - t)) with t the clamped value.
    #[test]
    fn near_a_node_the_effect_fades_and_never_exceeds_2_4_d(
        x3 in 20.0f64..500.0,
        t0 in 0.0f64..1.0,
        dx in coord(), dy in coord(),
    ) {
        let p0 = pt(0.0, 0.0);
        let p3 = pt(x3, 0.0);
        let d = v(dx, dy);
        let r = bend_segment_handles(p0, Vec2::ZERO, Vec2::ZERO, p3, t0, d).unwrap();
        let dl = (dx * dx + dy * dy).sqrt();
        let thirds = (x3 / 3.0, 0.0);
        let moved = |h: Vec2, base: (f64, f64)| ((h.x - base.0).powi(2) + (h.y - base.1).powi(2)).sqrt();
        prop_assert!(moved(r.start_handle_out, thirds) <= 2.4 * dl + 1e-9);
        prop_assert!(moved(r.end_handle_in, (-x3 / 3.0, 0.0)) <= 2.4 * dl + 1e-9);
        let t = t0.clamp(1.0 / 6.0, 5.0 / 6.0);
        let (a, b) = eval(p0, v(x3 / 3.0, 0.0), v(-x3 / 3.0, 0.0), p3, t0);
        let (c, e) = eval(p0, r.start_handle_out, r.end_handle_in, p3, t0);
        let fraction = 3.0 * t0 * (1.0 - t0) / (3.0 * t * (1.0 - t));
        prop_assert!(near(c - a, fraction * dx, 1e-6));
        prop_assert!(near(e - b, fraction * dy, 1e-6));
    }
}
