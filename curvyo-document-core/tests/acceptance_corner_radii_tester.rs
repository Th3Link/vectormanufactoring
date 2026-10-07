//! Independent black-box tests for `specs/rectangle-corner-radii/` PART 1
//! (model, codec, format, outline; criteria 9 to 11, 16 to 21), written from
//! the specification and `adrs.md` before reading the implementation. They use
//! the public API and raw `loro` documents only.
//!
//! The `oracle_*` functions below are a verbatim copy of `rect_outline` and
//! `effective_corner_radius` as they are on `origin/main` (single radius), so
//! "four equal radii give exactly today's output" is checked against the old
//! code, not against the new code's own idea of it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::items_after_statements,
    clippy::type_complexity,
    clippy::manual_assert,
    clippy::suboptimal_flops,
    clippy::doc_markdown,
    missing_docs
)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorKind, CURRENT_FORMAT_VERSION, Corner, CornerRadii, Document, Length, NodeId,
    ObjectSnapshot, OpenError, OutlineAnchor, Point, RectBounds, Shape, Vec2,
    effective_corner_radii, outline_of, pack, rect_outline, unpack,
};
use loro::{LoroDoc, LoroValue, TreeParentId};

const EPS: f64 = 1e-9;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn rb(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    }
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: mm(tl),
        tr: mm(tr),
        br: mm(br),
        bl: mm(bl),
    }
}

fn arr(r: CornerRadii) -> [f64; 4] {
    [r.tl.as_mm(), r.tr.as_mm(), r.br.as_mm(), r.bl.as_mm()]
}

fn eff(b: RectBounds, r: CornerRadii) -> [f64; 4] {
    arr(effective_corner_radii(b, r))
}

/// Deterministic xorshift, no external crate.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

// ---------------------------------------------------------------------
// The old (origin/main) code, verbatim, as an oracle.
// ---------------------------------------------------------------------

fn oracle_effective(bounds: RectBounds, corner_radius: Length) -> Length {
    let half_shorter_side = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
    Length::from_mm(corner_radius.as_mm().clamp(0.0, half_shorter_side.max(0.0)))
}

fn oc(point: Point) -> OutlineAnchor {
    OutlineAnchor {
        point,
        handle_in: Vec2::ZERO,
        handle_out: Vec2::ZERO,
        kind: AnchorKind::Corner,
    }
}

fn oracle_rect_outline(bounds: RectBounds, corner_radius: Length) -> Vec<OutlineAnchor> {
    let radius = oracle_effective(bounds, corner_radius).as_mm();
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let width = bounds.width.as_mm();
    let height = bounds.height.as_mm();

    if radius <= 0.0 {
        return vec![
            oc(pt(left, top)),
            oc(pt(left + width, top)),
            oc(pt(left + width, top + height)),
            oc(pt(left, top + height)),
        ];
    }

    let handle = curvyo_document_core::KAPPA * radius;
    let corner = AnchorKind::Corner;
    vec![
        OutlineAnchor {
            point: pt(left + radius, top),
            handle_in: Vec2::new(-handle, 0.0),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left + width - radius, top),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(handle, 0.0),
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left + width, top + radius),
            handle_in: Vec2::new(0.0, -handle),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left + width, top + height - radius),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(0.0, handle),
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left + width - radius, top + height),
            handle_in: Vec2::new(handle, 0.0),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left + radius, top + height),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(-handle, 0.0),
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left, top + height - radius),
            handle_in: Vec2::new(0.0, handle),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        OutlineAnchor {
            point: pt(left, top + radius),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(0.0, -handle),
            kind: corner,
        },
    ]
}

// ---------------------------------------------------------------------
// Criterion 9: effective radii (CSS rule)
// ---------------------------------------------------------------------

#[test]
fn ac9_spec_examples_on_100_by_40() {
    let b = rb(0.0, 0.0, 100.0, 40.0);
    assert_eq!(eff(b, radii(30.0, 30.0, 0.0, 0.0)), [30.0, 30.0, 0.0, 0.0]);
    let e = eff(b, radii(30.0, 30.0, 0.0, 30.0));
    for (got, want) in e.iter().zip([20.0, 20.0, 0.0, 20.0]) {
        assert!((got - want).abs() < EPS, "{e:?}");
    }
}

#[test]
fn ac9_one_factor_for_all_four_even_for_corners_that_do_not_overlap() {
    // TL+BL = 60 > 40: f = 2/3; BR is far from anything and shrinks as well.
    let b = rb(0.0, 0.0, 100.0, 40.0);
    let e = eff(b, radii(30.0, 5.0, 8.0, 30.0));
    let f = 40.0 / 60.0;
    for (got, want) in e.iter().zip([30.0 * f, 5.0 * f, 8.0 * f, 30.0 * f]) {
        assert!((got - want).abs() < EPS, "{e:?}");
    }
}

#[test]
fn ac9_diagonal_pairs_do_not_limit_each_other() {
    let b = rb(0.0, 0.0, 100.0, 40.0);
    // TL = BR = 40 on a 40 high box: TL+BL = 40 fits, TR+BR = 40 fits -> f = 1.
    assert_eq!(eff(b, radii(40.0, 0.0, 40.0, 0.0)), [40.0, 0.0, 40.0, 0.0]);
    assert_eq!(eff(b, radii(0.0, 40.0, 0.0, 40.0)), [0.0, 40.0, 0.0, 40.0]);
    // Square, both diagonal corners at 0.6 s: valid, no shrinking.
    let sq = rb(0.0, 0.0, 50.0, 50.0);
    assert_eq!(eff(sq, radii(30.0, 0.0, 30.0, 0.0)), [30.0, 0.0, 30.0, 0.0]);
    // A lone corner reaches the shorter side (three zeros), not half of it.
    let e = eff(sq, radii(1000.0, 0.0, 0.0, 0.0));
    assert!((e[0] - 50.0).abs() < EPS, "{e:?}");
    let e = eff(b, radii(0.0, 0.0, 1000.0, 0.0));
    assert!((e[2] - 40.0).abs() < EPS, "{e:?}");
}

#[test]
fn ac9_all_zero_and_zero_denominators_are_not_a_division_by_zero() {
    let b = rb(0.0, 0.0, 10.0, 10.0);
    assert_eq!(eff(b, radii(0.0, 0.0, 0.0, 0.0)), [0.0; 4]);
    for e in [
        eff(b, radii(0.0, 0.0, 5.0, 0.0)),
        eff(b, radii(7.0, 0.0, 0.0, 0.0)),
    ] {
        assert!(e.iter().all(|v| v.is_finite() && *v >= 0.0), "{e:?}");
    }
}

#[test]
fn ac9_with_four_equal_radii_it_is_todays_clamp() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..4000 {
        let w = rng.range(0.5, 400.0);
        let h = rng.range(0.5, 400.0);
        let r = rng.range(0.0, 300.0);
        let b = rb(rng.range(-50.0, 50.0), rng.range(-50.0, 50.0), w, h);
        let old = oracle_effective(b, mm(r)).as_mm();
        for v in eff(b, CornerRadii::uniform(mm(r))) {
            assert!((v - old).abs() < 1e-9, "w {w} h {h} r {r}: {v} vs {old}");
        }
    }
}

#[test]
fn ac9_property_css_invariants_on_random_radii() {
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    for _ in 0..20000 {
        let w = rng.range(0.1, 300.0);
        let h = rng.range(0.1, 300.0);
        let b = rb(0.0, 0.0, w, h);
        let pick = |rng: &mut Rng| match (rng.next() * 4.0) as u32 {
            0 => 0.0,
            1 => rng.range(0.0, 10.0),
            2 => rng.range(0.0, 150.0),
            _ => rng.range(0.0, 600.0),
        };
        let s = [
            pick(&mut rng),
            pick(&mut rng),
            pick(&mut rng),
            pick(&mut rng),
        ];
        let stored = radii(s[0], s[1], s[2], s[3]);
        let e = eff(b, stored);
        // My own formula, independent of the implementation's helper structure.
        let mut f: f64 = 1.0;
        for (num, den) in [
            (w, s[0] + s[1]),
            (w, s[3] + s[2]),
            (h, s[0] + s[3]),
            (h, s[1] + s[2]),
        ] {
            if den > 0.0 {
                f = f.min(num / den);
            }
        }
        for i in 0..4 {
            assert!(
                (e[i] - f * s[i]).abs() < 1e-9 * (1.0 + s[i]),
                "{s:?} {e:?} f {f}"
            );
            assert!(e[i] >= 0.0 && e[i] <= s[i] + 1e-12);
        }
        // Adjacent sums never exceed the side (the arcs never overlap).
        assert!(
            e[0] + e[1] <= w + 1e-9 && e[3] + e[2] <= w + 1e-9,
            "{s:?} {e:?}"
        );
        assert!(
            e[0] + e[3] <= h + 1e-9 && e[1] + e[2] <= h + 1e-9,
            "{s:?} {e:?}"
        );
        // Proportions are kept.
        if s[0] > 1.0 && s[1] > 1.0 {
            assert!((e[0] / s[0] - e[1] / s[1]).abs() < 1e-9, "{s:?} {e:?}");
        }
        // Evaluating twice changes nothing (f = 1 the second time).
        let again = eff(b, effective_corner_radii(b, stored));
        for i in 0..4 {
            assert!(
                (again[i] - e[i]).abs() < 1e-9 * (1.0 + s[i]),
                "{s:?} {e:?} {again:?}"
            );
        }
    }
}

#[test]
fn ac10_f_equal_one_returns_the_stored_values_bit_for_bit() {
    let b = rb(0.0, 0.0, 100.0, 100.0);
    let weird = radii(0.1 + 0.2, 1.0 / 3.0, 2.719_281_828_459_045, 1e-7);
    let small_sizes = [rb(0.0, 0.0, 3.0, 3.0)];
    let e = effective_corner_radii(b, weird);
    assert_eq!(arr(e), arr(weird), "bit exact when f = 1");
    // And a rectangle shrunk to f < 1 and enlarged again evaluates as before:
    // evaluation is a pure function of (bounds, stored radii).
    let small = small_sizes[0];
    assert_ne!(eff(small, weird), arr(weird));
    assert_eq!(eff(b, weird), arr(weird));
}

#[test]
fn ac9_negative_stored_radius_counts_as_zero_and_nothing_is_nan() {
    let b = rb(0.0, 0.0, 10.0, 10.0);
    let e = eff(b, radii(-3.0, 4.0, -0.0, 2.0));
    assert_eq!(e[0], 0.0);
    assert_eq!(e[2], 0.0);
    assert!(
        (e[1] - 4.0).abs() < EPS && (e[3] - 2.0).abs() < EPS,
        "{e:?}"
    );
}

#[test]
fn ac9_huge_finite_radii_never_produce_nan_or_infinity() {
    let b = rb(0.0, 0.0, 10.0, 10.0);
    for r in [
        radii(1e300, 1e300, 1e300, 1e300),
        radii(f64::MAX, f64::MAX, f64::MAX, f64::MAX),
        radii(f64::MAX, 0.0, 0.0, 0.0),
        radii(f64::MAX, 1.0, 0.0, 0.0),
        radii(1e-300, 1e300, 0.0, 5.0),
    ] {
        let e = eff(b, r);
        assert!(
            e.iter().all(|v| v.is_finite() && *v >= 0.0),
            "{r:?} -> {e:?}"
        );
        let out = rect_outline(b, r);
        assert!(
            out.iter().all(|a| a.point.x.is_finite()
                && a.point.y.is_finite()
                && a.handle_in.x.is_finite()
                && a.handle_in.y.is_finite()
                && a.handle_out.x.is_finite()
                && a.handle_out.y.is_finite()),
            "{r:?}"
        );
    }
}

#[test]
fn ac9_degenerate_zero_size_rectangles_have_no_rounding_and_no_nan() {
    for b in [
        rb(0.0, 0.0, 0.0, 10.0),
        rb(0.0, 0.0, 10.0, 0.0),
        rb(0.0, 0.0, 0.0, 0.0),
    ] {
        let e = eff(b, radii(5.0, 5.0, 5.0, 5.0));
        assert!(e.iter().all(|v| v.is_finite() && v.abs() < EPS), "{e:?}");
        let out = rect_outline(b, radii(5.0, 5.0, 5.0, 5.0));
        assert_eq!(out.len(), 4);
    }
}

// ---------------------------------------------------------------------
// Criteria 11 and 16: the outline
// ---------------------------------------------------------------------

fn flatten(outline: &[OutlineAnchor], per_curve: usize) -> Vec<Point> {
    let n = outline.len();
    let mut pts = Vec::new();
    for i in 0..n {
        let a = &outline[i];
        let b = &outline[(i + 1) % n];
        let p0 = a.point;
        let p1 = pt(a.point.x + a.handle_out.x, a.point.y + a.handle_out.y);
        let p2 = pt(b.point.x + b.handle_in.x, b.point.y + b.handle_in.y);
        let p3 = b.point;
        let straight = a.handle_out == Vec2::ZERO && b.handle_in == Vec2::ZERO;
        let steps = if straight { 1 } else { per_curve };
        for k in 0..steps {
            let t = k as f64 / steps as f64;
            let u = 1.0 - t;
            pts.push(pt(
                u * u * u * p0.x
                    + 3.0 * u * u * t * p1.x
                    + 3.0 * u * t * t * p2.x
                    + t * t * t * p3.x,
                u * u * u * p0.y
                    + 3.0 * u * u * t * p1.y
                    + 3.0 * u * t * t * p2.y
                    + t * t * t * p3.y,
            ));
        }
    }
    pts
}

fn shoelace(pts: &[Point]) -> f64 {
    let mut s = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

fn dist_seg(p: Point, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.0) * dx + (p.y - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Distance from `p` to the boundary of the rounded rectangle with the given
/// effective radii, computed analytically.
fn dist_boundary(p: Point, b: RectBounds, e: [f64; 4]) -> f64 {
    let (x0, y0) = (b.origin.x, b.origin.y);
    let (x1, y1) = (x0 + b.width.as_mm(), y0 + b.height.as_mm());
    let mut d = f64::MAX;
    d = d.min(dist_seg(p, (x0 + e[0], y0), (x1 - e[1], y0)));
    d = d.min(dist_seg(p, (x1, y0 + e[1]), (x1, y1 - e[2])));
    d = d.min(dist_seg(p, (x1 - e[2], y1), (x0 + e[3], y1)));
    d = d.min(dist_seg(p, (x0, y1 - e[3]), (x0, y0 + e[0])));
    let arcs = [
        (x0 + e[0], y0 + e[0], -1.0, -1.0, e[0]),
        (x1 - e[1], y0 + e[1], 1.0, -1.0, e[1]),
        (x1 - e[2], y1 - e[2], 1.0, 1.0, e[2]),
        (x0 + e[3], y1 - e[3], -1.0, 1.0, e[3]),
    ];
    for (cx, cy, sx, sy, r) in arcs {
        if r <= 0.0 {
            d = d.min(((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt());
            continue;
        }
        let (vx, vy) = (p.x - cx, p.y - cy);
        if vx * sx >= 0.0 && vy * sy >= 0.0 {
            d = d.min((vx.hypot(vy) - r).abs());
        }
        for end in [(cx + sx * r, cy), (cx, cy + sy * r)] {
            d = d.min(((p.x - end.0).powi(2) + (p.y - end.1).powi(2)).sqrt());
        }
    }
    d
}

fn boundary_samples(b: RectBounds, e: [f64; 4], n: usize) -> Vec<Point> {
    let (x0, y0) = (b.origin.x, b.origin.y);
    let (x1, y1) = (x0 + b.width.as_mm(), y0 + b.height.as_mm());
    let mut v = Vec::new();
    let line = |v: &mut Vec<Point>, a: (f64, f64), c: (f64, f64)| {
        for k in 0..=n {
            let t = k as f64 / n as f64;
            v.push(pt(a.0 + t * (c.0 - a.0), a.1 + t * (c.1 - a.1)));
        }
    };
    line(&mut v, (x0 + e[0], y0), (x1 - e[1], y0));
    line(&mut v, (x1, y0 + e[1]), (x1, y1 - e[2]));
    line(&mut v, (x1 - e[2], y1), (x0 + e[3], y1));
    line(&mut v, (x0, y1 - e[3]), (x0, y0 + e[0]));
    let arcs = [
        (x0 + e[0], y0 + e[0], -1.0, -1.0, e[0]),
        (x1 - e[1], y0 + e[1], 1.0, -1.0, e[1]),
        (x1 - e[2], y1 - e[2], 1.0, 1.0, e[2]),
        (x0 + e[3], y1 - e[3], -1.0, 1.0, e[3]),
    ];
    for (cx, cy, sx, sy, r) in arcs {
        if r <= 0.0 {
            v.push(pt(cx, cy));
            continue;
        }
        for k in 0..=n {
            let a = std::f64::consts::FRAC_PI_2 * k as f64 / n as f64;
            v.push(pt(cx + sx * r * a.cos(), cy + sy * r * a.sin()));
        }
    }
    v
}

fn dist_polyline(p: Point, poly: &[Point]) -> f64 {
    let mut d = f64::MAX;
    for i in 0..poly.len() {
        let a = poly[i];
        let c = poly[(i + 1) % poly.len()];
        d = d.min(dist_seg(p, (a.x, a.y), (c.x, c.y)));
    }
    d
}

fn expected_node_count(e: [f64; 4]) -> usize {
    4 + e.iter().filter(|v| **v > 1e-9).count()
}

fn check_outline(b: RectBounds, stored: CornerRadii) {
    let e = eff(b, stored);
    let out = rect_outline(b, stored);
    let ctx = format!("{b:?} {stored:?} eff {e:?}");
    assert_eq!(out.len(), expected_node_count(e), "node count: {ctx}");
    assert!((4..=8).contains(&out.len()), "{ctx}");
    assert!(
        out.iter().all(|a| a.kind == AnchorKind::Corner),
        "kinds: {ctx}"
    );

    // First node: the end of TL's arc, or the TL corner point when sharp.
    let first = out[0].point;
    let want_first = if e[0] > 1e-9 {
        pt(b.origin.x + e[0], b.origin.y)
    } else {
        b.origin
    };
    assert!(
        (first.x - want_first.x).abs() < 1e-9 && (first.y - want_first.y).abs() < 1e-9,
        "first node {first:?} want {want_first:?}: {ctx}"
    );

    let rmax = e.iter().copied().fold(0.0, f64::max);
    let tol = 3.0e-4 * rmax + 1e-9;
    let poly = flatten(&out, 64);
    for p in &poly {
        let d = dist_boundary(*p, b, e);
        assert!(
            d <= tol,
            "outline point {p:?} is {d} off the boundary (tol {tol}): {ctx}"
        );
    }
    for p in boundary_samples(b, e, 40) {
        let d = dist_polyline(p, &poly);
        assert!(
            d <= tol,
            "boundary point {p:?} is {d} off the outline: {ctx}"
        );
    }

    // Clockwise on screen (y down): positive shoelace; area as analytic.
    let area = shoelace(&poly);
    let w = b.width.as_mm();
    let h = b.height.as_mm();
    let cut: f64 = e
        .iter()
        .map(|r| r * r * (1.0 - std::f64::consts::FRAC_PI_4))
        .sum();
    assert!(area > 0.0, "winding: {ctx}");
    let want = w * h - cut;
    assert!(
        (area - want).abs() <= 1e-3 * (rmax * rmax) + 1e-6 * want,
        "area {area} want {want}: {ctx}"
    );

    // Closing: the segment last -> first is TL's arc if rounded (handles) or a
    // straight edge if TL is sharp; every curved segment belongs to a rounded corner.
    let curved = (0..out.len())
        .filter(|&i| {
            let j = (i + 1) % out.len();
            out[i].handle_out != Vec2::ZERO || out[j].handle_in != Vec2::ZERO
        })
        .count();
    assert_eq!(
        curved,
        e.iter().filter(|v| **v > 1e-9).count(),
        "curved segments: {ctx}"
    );
    if e[0] > 1e-9 {
        let last = out.len() - 1;
        assert!(
            out[last].handle_out != Vec2::ZERO && out[0].handle_in != Vec2::ZERO,
            "TL arc closes the loop: {ctx}"
        );
    }
}

#[test]
fn ac16_every_combination_of_sharp_and_rounded_corners() {
    let sizes = [(100.0, 40.0), (40.0, 100.0), (60.0, 60.0), (7.5, 3.25)];
    for (w, h) in sizes {
        let b = rb(12.0, -7.0, w, h);
        let s = h.min(w);
        for mask in 0u32..16 {
            for scale in [0.1, 0.3, 0.5] {
                let r = |bit: u32, v: f64| if mask & (1 << bit) != 0 { v } else { 0.0 };
                let stored = radii(
                    r(0, s * scale),
                    r(1, s * scale * 0.9),
                    r(2, s * scale * 0.8),
                    r(3, s * scale * 0.7),
                );
                check_outline(b, stored);
                let e = eff(b, stored);
                assert_eq!(
                    rect_outline(b, stored).len(),
                    4 + mask.count_ones() as usize,
                    "{mask} {e:?}"
                );
            }
        }
    }
}

#[test]
fn ac16_spec_examples_and_special_shapes() {
    let b = rb(0.0, 0.0, 100.0, 40.0);
    for r in [
        radii(30.0, 30.0, 0.0, 0.0),
        radii(30.0, 30.0, 0.0, 30.0),
        radii(40.0, 0.0, 40.0, 0.0),
        radii(0.0, 40.0, 0.0, 40.0),
        radii(40.0, 0.0, 0.0, 0.0),
        radii(1000.0, 0.0, 0.0, 0.0),
        radii(20.0, 20.0, 20.0, 20.0),
        radii(500.0, 500.0, 500.0, 500.0),
        radii(5.0, 0.0, 0.0, 0.0),
        radii(0.0, 0.0, 0.0, 0.0),
        radii(20.0, 20.0, 30.0, 10.0),
        radii(50.0, 50.0, 0.0, 0.0),
    ] {
        check_outline(b, r);
        check_outline(rb(0.0, 0.0, 40.0, 40.0), r);
    }
}

#[test]
fn ac16_two_arcs_meeting_keep_the_zero_length_segment() {
    // tl = tr = 50 on 100 x 80: arcs meet at the top middle.
    let b = rb(0.0, 0.0, 100.0, 80.0);
    let out = rect_outline(b, radii(50.0, 50.0, 0.0, 0.0));
    assert_eq!(out.len(), 6);
    assert!((out[0].point.x - 50.0).abs() < EPS && (out[1].point.x - 50.0).abs() < EPS);
    check_outline(b, radii(50.0, 50.0, 0.0, 0.0));
}

#[test]
fn ac16_arc_deviation_is_inside_one_tenth_of_a_percent() {
    let b = rb(0.0, 0.0, 200.0, 200.0);
    for r in [0.5, 3.0, 40.0, 99.0] {
        let out = rect_outline(b, radii(r, 0.0, 0.0, 0.0));
        // The TL arc runs last -> first.
        let a = &out[out.len() - 1];
        let c = &out[0];
        let p0 = a.point;
        let p1 = pt(p0.x + a.handle_out.x, p0.y + a.handle_out.y);
        let p3 = c.point;
        let p2 = pt(p3.x + c.handle_in.x, p3.y + c.handle_in.y);
        let centre = pt(r, r);
        for k in 0..=100 {
            let t = f64::from(k) / 100.0;
            let u = 1.0 - t;
            let x = u * u * u * p0.x
                + 3.0 * u * u * t * p1.x
                + 3.0 * u * t * t * p2.x
                + t * t * t * p3.x;
            let y = u * u * u * p0.y
                + 3.0 * u * u * t * p1.y
                + 3.0 * u * t * t * p2.y
                + t * t * t * p3.y;
            let d = (x - centre.x).hypot(y - centre.y);
            assert!((d - r).abs() <= 1e-3 * r, "r {r} t {t} d {d}");
        }
    }
}

#[test]
fn ac11_sharp_tolerance_is_one_nanometre_and_inclusive() {
    let b = rb(0.0, 0.0, 50.0, 50.0);
    assert_eq!(rect_outline(b, CornerRadii::uniform(mm(0.0))).len(), 4);
    assert_eq!(rect_outline(b, CornerRadii::uniform(mm(1e-10))).len(), 4);
    assert_eq!(rect_outline(b, CornerRadii::uniform(mm(1e-9))).len(), 4);
    assert_eq!(rect_outline(b, CornerRadii::uniform(mm(2e-9))).len(), 8);
    assert_eq!(rect_outline(b, radii(1e-10, 5.0, 1e-10, 5.0)).len(), 6);
    assert_eq!(rect_outline(b, radii(-1.0, 5.0, -2.0, 5.0)).len(), 6);
    assert_eq!(curvyo_document_core::SHARP_CORNER_EPSILON_MM, 1e-9);
}

#[test]
fn ac11_a_corner_squeezed_to_zero_by_the_clamp_is_sharp_in_the_outline() {
    // W = 0 forces f = 0: every corner is sharp.
    let out = rect_outline(rb(0.0, 0.0, 0.0, 10.0), CornerRadii::uniform(mm(5.0)));
    assert_eq!(out.len(), 4);
}

#[test]
fn ac16_equal_radii_equal_todays_output_exactly_where_the_clamp_is_idle() {
    let mut count = 0;
    for w in [1.0, 2.5, 10.0, 40.0, 100.0, 333.3, 0.75] {
        for h in [1.0, 2.5, 10.0, 40.0, 100.0, 333.3, 0.75] {
            for (ox, oy) in [(0.0, 0.0), (-13.7, 22.1)] {
                let b = rb(ox, oy, w, h);
                let half = w.min(h) / 2.0;
                for r in [
                    0.0,
                    1e-6,
                    0.1,
                    0.3,
                    1.0,
                    half * 0.25,
                    half * 0.5,
                    half * 0.999,
                    half,
                    half * 1.0001,
                    half * 2.0,
                    1000.0,
                ] {
                    let new = rect_outline(b, CornerRadii::uniform(mm(r)));
                    let old = oracle_rect_outline(b, mm(r));
                    assert_eq!(new.len(), old.len(), "count {b:?} r {r}");
                    let clamped = r > half;
                    for (n, o) in new.iter().zip(&old) {
                        assert_eq!(n.kind, o.kind);
                        if clamped {
                            // f * r may differ from W/2 by an ulp.
                            assert!((n.point.x - o.point.x).abs() < 1e-9, "{b:?} r {r}");
                            assert!((n.point.y - o.point.y).abs() < 1e-9);
                            assert!((n.handle_in.x - o.handle_in.x).abs() < 1e-9);
                            assert!((n.handle_in.y - o.handle_in.y).abs() < 1e-9);
                            assert!((n.handle_out.x - o.handle_out.x).abs() < 1e-9);
                            assert!((n.handle_out.y - o.handle_out.y).abs() < 1e-9);
                        } else {
                            assert_eq!(n, o, "exact equality: {b:?} r {r}");
                        }
                    }
                    count += 1;
                }
            }
        }
    }
    assert!(count > 1000);
}

#[test]
fn ac16_outline_of_a_shape_goes_through_the_same_function() {
    let b = rb(1.0, 2.0, 100.0, 40.0);
    let r = radii(30.0, 5.0, 0.0, 12.0);
    let shape = Shape::Rect {
        bounds: b,
        corner_radii: r,
    };
    assert_eq!(outline_of(&shape), rect_outline(b, r));
}

// ---------------------------------------------------------------------
// Raw-file helpers
// ---------------------------------------------------------------------

fn zip_with(manifest_version: u32, loro_bytes: &[u8], json: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": manifest_version,
        "loro_snapshot_version": 1,
        "app_version": "tester",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(json).unwrap();
    writer.finish().unwrap().into_inner()
}

/// A file whose one object is a rectangle 100 x 40 at the origin, with whatever
/// extra keys `extra` inserts.
fn raw_rect_file(version: u32, extra: impl FnOnce(&loro::LoroMap)) -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    let tree = loro.get_tree("paths");
    let node = tree.create(TreeParentId::Root).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert("shape", "rect").unwrap();
    meta.insert("rect_bounds", vec![0.0, 0.0, 100.0, 40.0])
        .unwrap();
    extra(&meta);
    loro.commit();
    let snap = loro.export(loro::ExportMode::Snapshot).unwrap();
    zip_with(version, &snap, b"{}")
}

fn open_radii(bytes: &[u8]) -> Result<CornerRadii, OpenError> {
    let d = unpack(2, bytes)?;
    let id = d.object_ids()[0];
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!("primitive")
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!("rect")
    };
    Ok(corner_radii)
}

fn ins_f(m: &loro::LoroMap, k: &str, v: f64) {
    m.insert(k, v).unwrap();
}

// ---------------------------------------------------------------------
// Criteria 17 and 18, ADR 2: reading, legacy fallback, no rewrite
// ---------------------------------------------------------------------

#[test]
fn ac18_legacy_only_reads_as_four_equal_corners() {
    for v in [0.0, 0.5, 7.5, 20.0, 1000.0] {
        let bytes = raw_rect_file(5, |m| ins_f(m, "corner_radius", v));
        let r = open_radii(&bytes).unwrap();
        assert_eq!(arr(r), [v; 4], "legacy {v}");
    }
}

#[test]
fn ac17_own_keys_read_per_corner_and_need_no_legacy_key() {
    let bytes = raw_rect_file(6, |m| {
        ins_f(m, "corner_radius_tl", 1.0);
        ins_f(m, "corner_radius_tr", 2.0);
        ins_f(m, "corner_radius_br", 3.0);
        ins_f(m, "corner_radius_bl", 4.0);
    });
    assert_eq!(arr(open_radii(&bytes).unwrap()), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn adr2_mixed_own_and_legacy_keys_fall_back_per_corner() {
    let bytes = raw_rect_file(5, |m| {
        ins_f(m, "corner_radius", 5.0);
        ins_f(m, "corner_radius_tl", 1.0);
        ins_f(m, "corner_radius_br", 0.0); // own 0 wins over legacy 5
    });
    assert_eq!(arr(open_radii(&bytes).unwrap()), [1.0, 5.0, 0.0, 5.0]);
    // All four own keys plus a legacy key: legacy is ignored.
    let bytes = raw_rect_file(6, |m| {
        ins_f(m, "corner_radius", 9.0);
        for (k, v) in [("tl", 1.0), ("tr", 2.0), ("br", 3.0), ("bl", 4.0)] {
            ins_f(m, &format!("corner_radius_{k}"), v);
        }
    });
    assert_eq!(arr(open_radii(&bytes).unwrap()), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn ac20_a_mistyped_own_key_is_damaged_and_not_masked_by_the_legacy_key() {
    type Setter = fn(&loro::LoroMap);
    let mistyped: [(&str, Setter); 5] = [
        ("string", |m| m.insert("corner_radius_tr", "abc").unwrap()),
        ("bool", |m| m.insert("corner_radius_tr", true).unwrap()),
        ("list", |m| {
            m.insert("corner_radius_tr", vec![1.0_f64]).unwrap();
        }),
        ("null", |m| {
            m.insert("corner_radius_tr", LoroValue::Null).unwrap();
        }),
        ("map", |m| {
            m.insert_container("corner_radius_tr", loro::LoroMap::new())
                .unwrap();
        }),
    ];
    for (name, set) in mistyped {
        let bytes = raw_rect_file(6, |m| {
            ins_f(m, "corner_radius", 5.0);
            set(m);
        });
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "mistyped own key ({name}) with a valid legacy key must be Damaged"
        );
    }
}

#[test]
fn ac20_negative_non_finite_own_and_legacy_radii_are_damaged_not_clamped() {
    for bad in [
        -0.001,
        -5.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN,
    ] {
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            let bytes = raw_rect_file(6, |m| {
                for k in ["tl", "tr", "br", "bl"] {
                    ins_f(m, &format!("corner_radius_{k}"), 3.0);
                }
                ins_f(m, key, bad);
            });
            assert!(
                matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
                "{key} = {bad}"
            );
            // Own-key corruption is not masked by a good legacy key either.
            let bytes = raw_rect_file(6, |m| {
                ins_f(m, "corner_radius", 3.0);
                ins_f(m, key, bad);
            });
            assert!(
                matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
                "{key} = {bad} + legacy"
            );
        }
        // Legacy-only file with a bad value (a v5 file damaged on disk).
        let bytes = raw_rect_file(5, |m| ins_f(m, "corner_radius", bad));
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "legacy = {bad}"
        );
    }
}

#[test]
fn ac20_missing_radius_information_is_damaged() {
    assert!(matches!(
        unpack(2, &raw_rect_file(6, |_| {})),
        Err(OpenError::Damaged)
    ));
    // Three own keys, no legacy: one corner has no source.
    let bytes = raw_rect_file(6, |m| {
        for k in ["tl", "tr", "br"] {
            ins_f(m, &format!("corner_radius_{k}"), 1.0);
        }
    });
    assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
    // A mistyped legacy key used as the fallback of a missing corner.
    let bytes = raw_rect_file(6, |m| {
        m.insert("corner_radius", "five").unwrap();
        ins_f(m, "corner_radius_tl", 1.0);
    });
    assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
    let bytes = raw_rect_file(5, |m| m.insert("corner_radius", "five").unwrap());
    assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
}

#[test]
fn adr10_a_bad_legacy_key_that_no_corner_needs_is_not_read() {
    let bytes = raw_rect_file(6, |m| {
        m.insert("corner_radius", "garbage").unwrap();
        for (k, v) in [("tl", 1.0), ("tr", 2.0), ("br", 3.0), ("bl", 4.0)] {
            ins_f(m, &format!("corner_radius_{k}"), v);
        }
    });
    // ADR 0 decision 10: "a corner without its own key needs a valid legacy key".
    assert_eq!(arr(open_radii(&bytes).unwrap()), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn ac20_huge_finite_and_over_the_side_radii_are_not_damaged() {
    // Sum above the side (what a merge of two peers can produce).
    let bytes = raw_rect_file(6, |m| {
        for (k, v) in [("tl", 80.0), ("tr", 80.0), ("br", 0.0), ("bl", 0.0)] {
            ins_f(m, &format!("corner_radius_{k}"), v);
        }
    });
    let d = unpack(2, &bytes).unwrap();
    let id = d.object_ids()[0];
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let Shape::Rect {
        bounds,
        corner_radii,
    } = p.shape
    else {
        panic!()
    };
    assert_eq!(arr(corner_radii), [80.0, 80.0, 0.0, 0.0], "stored raw");
    let e = eff(bounds, corner_radii);
    // 80 + 80 > 100 and 80 + 0 > 40: f = 40 / 80 = 0.5.
    assert!(
        (e[0] - 40.0).abs() < 1e-9 && (e[1] - 40.0).abs() < 1e-9,
        "{e:?}"
    );
    // Huge but finite.
    for v in [1e15, 1e300, f64::MAX] {
        let bytes = raw_rect_file(6, |m| {
            for k in ["tl", "tr", "br", "bl"] {
                ins_f(m, &format!("corner_radius_{k}"), v);
            }
        });
        let r = open_radii(&bytes).unwrap_or_else(|e| panic!("{v}: {e:?}"));
        assert_eq!(arr(r), [v; 4]);
        let outline = rect_outline(rb(0.0, 0.0, 100.0, 40.0), r);
        assert!(
            outline
                .iter()
                .all(|a| a.point.x.is_finite() && a.point.y.is_finite()),
            "{v}"
        );
    }
}

#[test]
fn ac20_negative_zero_and_integer_typed_values_probe() {
    // Probe only (spec is silent): -0.0 is not negative.
    let bytes = raw_rect_file(6, |m| {
        for k in ["tl", "tr", "br", "bl"] {
            ins_f(m, &format!("corner_radius_{k}"), -0.0);
        }
    });
    assert!(open_radii(&bytes).is_ok(), "-0.0 is a valid radius");
}

// ---------------------------------------------------------------------
// Fixtures: independent reading of the committed golden files
// ---------------------------------------------------------------------

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn member(bytes: &[u8], name: &str) -> Vec<u8> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut f = z.by_name(name).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).unwrap();
    v
}

fn manifest_version(bytes: &[u8]) -> u64 {
    let v: serde_json::Value = serde_json::from_slice(&member(bytes, "manifest.json")).unwrap();
    v["format_version"].as_u64().unwrap()
}

fn raw_nodes(bytes: &[u8]) -> Vec<serde_json::Value> {
    let l = LoroDoc::new();
    l.import(&member(bytes, "document.loro")).unwrap();
    let v = serde_json::to_value(l.get_deep_value()).unwrap();
    let tree = v["paths"].as_array().unwrap().clone();
    tree.iter().map(|n| n["meta"].clone()).collect()
}

#[test]
fn legacy_fixture_is_really_a_v5_file_with_only_the_legacy_key() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    assert_eq!(manifest_version(&bytes), 5);
    let nodes = raw_nodes(&bytes);
    assert!(nodes.len() >= 2, "a rounded and a sharp rectangle");
    let mut values = Vec::new();
    for n in &nodes {
        let o = n.as_object().unwrap();
        assert!(o.contains_key("corner_radius"), "legacy key present: {n}");
        assert!(
            !o.keys().any(|k| k.starts_with("corner_radius_")),
            "no own key in a v5 fixture: {n}"
        );
        values.push(o["corner_radius"].as_f64().unwrap());
    }
    assert!(
        values.iter().any(|v| *v > 0.0) && values.contains(&0.0),
        "{values:?}"
    );
}

#[test]
fn legacy_fixture_opens_as_four_equal_corners_and_outlines_like_the_old_code() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let legacy: Vec<f64> = raw_nodes(&bytes)
        .iter()
        .map(|n| n["corner_radius"].as_f64().unwrap())
        .collect();
    let d = unpack(2, &bytes).unwrap();
    let ids = d.object_ids();
    assert_eq!(ids.len(), legacy.len());
    let mut seen = Vec::new();
    for id in ids {
        let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect {
            bounds,
            corner_radii,
        } = p.shape
        else {
            panic!()
        };
        let r = corner_radii.tl.as_mm();
        assert_eq!(arr(corner_radii), [r; 4]);
        assert!(legacy.contains(&r), "{r} not in {legacy:?}");
        seen.push(r);
        // Old code oracle on the opened parameters (outline_of_rotated applies rotation).
        let old = oracle_rect_outline(bounds, mm(r));
        let new = outline_of(&p.shape);
        assert_eq!(new, old);
    }
    seen.sort_by(f64::total_cmp);
    let mut want = legacy;
    want.sort_by(f64::total_cmp);
    assert_eq!(seen, want);
}

#[test]
fn ac18_opening_and_saving_a_legacy_file_does_not_rewrite_the_nodes() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let before_nodes = raw_nodes(&bytes);
    let before_vv = {
        let l = LoroDoc::new();
        l.import(&member(&bytes, "document.loro")).unwrap();
        l.oplog_vv()
    };
    let d = unpack(2, &bytes).unwrap();
    // The document's own oplog after open must equal the file's.
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    assert_eq!(l.oplog_vv(), before_vv, "open wrote operations");
    // Save: new manifest version, node keys untouched.
    let saved = pack(&d, "t").unwrap();
    assert_eq!(manifest_version(&saved), u64::from(CURRENT_FORMAT_VERSION));
    assert_eq!(
        raw_nodes(&saved),
        before_nodes,
        "keys of legacy nodes unchanged after save"
    );
    let l2 = LoroDoc::new();
    l2.import(&member(&saved, "document.loro")).unwrap();
    assert_eq!(l2.oplog_vv(), before_vv);
}

#[test]
fn v6_fixture_reads_exact_values_and_round_trips_exactly() {
    let bytes = fixture("corner_radii_per_corner.curvyo");
    assert_eq!(manifest_version(&bytes), 6);
    let nodes = raw_nodes(&bytes);
    // Read each node's raw expectation independently from loro.
    let d = unpack(2, &bytes).unwrap();
    let ids = d.object_ids();
    assert_eq!(ids.len(), nodes.len());
    let mut kinds = (false, false, false, false); // unequal, sharp TL, sum>side, own+legacy
    for (id, n) in ids.iter().zip(&nodes) {
        let o = n.as_object().unwrap();
        let ObjectSnapshot::Primitive(p) = d.object(*id).unwrap() else {
            panic!()
        };
        let Shape::Rect {
            bounds,
            corner_radii,
        } = p.shape
        else {
            continue;
        };
        let legacy = o.get("corner_radius").and_then(serde_json::Value::as_f64);
        let mut want = [0.0; 4];
        for (i, k) in ["tl", "tr", "br", "bl"].iter().enumerate() {
            want[i] = o
                .get(&format!("corner_radius_{k}"))
                .and_then(serde_json::Value::as_f64)
                .or(legacy)
                .unwrap();
        }
        assert_eq!(arr(corner_radii), want, "raw values read exactly");
        kinds.0 |= want.iter().any(|v| (*v - want[0]).abs() > 1e-9);
        kinds.1 |= want[0] == 0.0 && want.iter().any(|v| *v > 0.0);
        let w = bounds.width.as_mm();
        let h = bounds.height.as_mm();
        kinds.2 |= want[0] + want[1] > w
            || want[3] + want[2] > w
            || want[0] + want[3] > h
            || want[1] + want[2] > h;
        kinds.3 |= legacy.is_some() && o.keys().any(|k| k.starts_with("corner_radius_"));
        let _ = p.rotation;
    }
    assert!(
        kinds.0 && kinds.1 && kinds.2 && kinds.3,
        "fixture covers the promised cases {kinds:?}"
    );
    // Rotated rectangle present.
    let any_rot = ids.iter().any(|id| {
        matches!(d.object(*id).unwrap(), ObjectSnapshot::Primitive(p) if p.rotation.as_radians().abs() > 1e-6)
    });
    assert!(any_rot, "a rotated rectangle");
    // Save + reopen exact (criterion 17).
    let saved = pack(&d, "t").unwrap();
    let d2 = unpack(3, &saved).unwrap();
    for id in &ids {
        assert_eq!(
            format!("{:?}", d.object(*id).unwrap()),
            format!("{:?}", d2.object(*id).unwrap())
        );
    }
    // Opening wrote nothing.
    let l0 = LoroDoc::new();
    l0.import(&member(&bytes, "document.loro")).unwrap();
    let l1 = LoroDoc::new();
    l1.import(&d.export_loro_snapshot().unwrap()).unwrap();
    assert_eq!(l0.oplog_vv(), l1.oplog_vv());
}

// ---------------------------------------------------------------------
// Criterion 19: format version
// ---------------------------------------------------------------------

#[test]
fn ac19_every_saved_file_declares_the_current_format_version() {
    let current = u64::from(CURRENT_FORMAT_VERSION);
    // An empty document and one with a rectangle: the same version (not only per-corner files).
    let empty = Document::new(1);
    assert_eq!(manifest_version(&pack(&empty, "t").unwrap()), current);
    let d = Document::new(1);
    let _ = d.create_rect(rb(0.0, 0.0, 10.0, 10.0));
    assert_eq!(manifest_version(&pack(&d, "t").unwrap()), current);
}

#[test]
fn ac19_a_newer_file_is_refused_and_this_version_and_older_are_accepted() {
    let l = LoroDoc::new();
    l.set_peer_id(1).unwrap();
    l.commit();
    let snap = l.export(loro::ExportMode::Snapshot).unwrap();
    for v in [1, 5, 6, CURRENT_FORMAT_VERSION] {
        assert!(unpack(2, &zip_with(v, &snap, b"{}")).is_ok(), "version {v}");
    }
    for v in [
        CURRENT_FORMAT_VERSION + 1,
        CURRENT_FORMAT_VERSION + 2,
        CURRENT_FORMAT_VERSION + 100,
    ] {
        match unpack(2, &zip_with(v, &snap, b"{}")) {
            Err(OpenError::FormatTooNew { found, supported }) => {
                assert_eq!((found, supported), (v, CURRENT_FORMAT_VERSION));
            }
            other => panic!("version {v}: {:?}", other.err()),
        }
    }
}

#[test]
fn ac19_the_future_version_fixture_is_still_newer_than_current() {
    let bytes = fixture("future_format_version.curvyo");
    let v = manifest_version(&bytes);
    assert!(v > u64::from(CURRENT_FORMAT_VERSION), "fixture says {v}");
    assert!(matches!(
        unpack(2, &bytes),
        Err(OpenError::FormatTooNew { .. })
    ));
}

// ---------------------------------------------------------------------
// Writing: only changed registers (ADR 4), commands
// ---------------------------------------------------------------------

fn vv_of(d: &Document) -> loro::VersionVector {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l.oplog_vv()
}

fn ops_since(d: &Document, from: &loro::VersionVector) -> String {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    format!("{:?}", l.export_json_updates(from, &l.oplog_vv()))
}

fn count(ops: &str, key: &str) -> usize {
    ops.matches(&format!("\"{key}\"")).count()
}

fn corner_writes(ops: &str) -> [usize; 4] {
    [
        count(ops, "corner_radius_tl"),
        count(ops, "corner_radius_tr"),
        count(ops, "corner_radius_br"),
        count(ops, "corner_radius_bl"),
    ]
}

fn read_radii(d: &Document, id: NodeId) -> CornerRadii {
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!()
    };
    corner_radii
}

fn v6_doc(r: [f64; 4]) -> (Document, NodeId, Vec<u8>) {
    let d = Document::new(1);
    let id = d.create_rect(rb(0.0, 0.0, 100.0, 40.0));
    d.set_corner_radii(&[(id, radii(r[0], r[1], r[2], r[3]))])
        .unwrap();
    let bytes = pack(&d, "t").unwrap();
    (unpack(2, &bytes).unwrap(), id, bytes)
}

#[test]
fn create_rect_writes_four_registers_at_zero_and_not_the_legacy_key() {
    let d = Document::new(1);
    let id = d.create_rect(rb(0.0, 0.0, 10.0, 10.0));
    assert_eq!(arr(read_radii(&d, id)), [0.0; 4]);
    let ops = ops_since(&d, &loro::VersionVector::default());
    assert_eq!(corner_writes(&ops), [1, 1, 1, 1], "{ops}");
    assert_eq!(
        count(&ops, "corner_radius"),
        0,
        "legacy key never written: {ops}"
    );
}

#[test]
fn set_corner_radii_writes_only_the_registers_that_change() {
    let (d, id, _) = v6_doc([1.0, 2.0, 3.0, 4.0]);
    let before = vv_of(&d);
    d.set_corner_radii(&[(id, radii(1.0, 2.0, 9.0, 4.0))])
        .unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [0, 0, 1, 0], "{ops}");
    assert_eq!(arr(read_radii(&d, id)), [1.0, 2.0, 9.0, 4.0]);

    // Same values again: nothing committed at all.
    let before = vv_of(&d);
    d.set_corner_radii(&[(id, radii(1.0, 2.0, 9.0, 4.0))])
        .unwrap();
    assert_eq!(vv_of(&d), before, "no-op write committed something");

    // Two change, two written.
    let before = vv_of(&d);
    d.set_corner_radii(&[(id, radii(7.0, 2.0, 9.0, 8.0))])
        .unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [1, 0, 0, 1], "{ops}");
    assert_eq!(count(&ops, "corner_radius"), 0);
}

#[test]
fn set_corner_radius_all_four_writes_only_the_corners_that_differ() {
    let (d, id, _) = v6_doc([5.0, 5.0, 5.0, 0.0]);
    let before = vv_of(&d);
    d.set_corner_radius(&[id], mm(5.0)).unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [0, 0, 0, 1], "{ops}");
    assert_eq!(arr(read_radii(&d, id)), [5.0; 4]);
    let before = vv_of(&d);
    d.set_corner_radius(&[id], mm(5.0)).unwrap();
    assert_eq!(vv_of(&d), before);
    // Negative is floored, stored raw otherwise (no clamping to the side).
    d.set_corner_radius(&[id], mm(-3.0)).unwrap();
    assert_eq!(arr(read_radii(&d, id)), [0.0; 4]);
    d.set_corner_radius(&[id], mm(500.0)).unwrap();
    assert_eq!(
        arr(read_radii(&d, id)),
        [500.0; 4],
        "stored raw, clamped only on evaluation"
    );
    d.set_corner_radii(&[(id, radii(-1.0, 2.0, f64::from(3u8), -0.5))])
        .unwrap();
    assert_eq!(arr(read_radii(&d, id)), [0.0, 2.0, 3.0, 0.0]);
}

#[test]
fn set_corner_radius_on_several_rectangles_is_one_commit_and_refuses_bad_ids_whole() {
    let d = Document::new(1);
    let a = d.create_rect(rb(0.0, 0.0, 10.0, 10.0));
    let b = d.create_rect(rb(0.0, 0.0, 10.0, 10.0));
    let e = d.create_ellipse(curvyo_document_core::EllipseFrame {
        center: pt(0.0, 0.0),
        rx: mm(1.0),
        ry: mm(1.0),
    });
    let before = vv_of(&d);
    assert!(d.set_corner_radius(&[a, b, e], mm(3.0)).is_err());
    assert_eq!(
        vv_of(&d),
        before,
        "a wrong id refuses the whole call, nothing written"
    );
    d.set_corner_radius(&[a, b], mm(3.0)).unwrap();
    assert_eq!(arr(read_radii(&d, a)), [3.0; 4]);
    assert_eq!(arr(read_radii(&d, b)), [3.0; 4]);
    assert!(
        d.set_corner_radii(&[
            (a, radii(1.0, 1.0, 1.0, 1.0)),
            (e, radii(1.0, 1.0, 1.0, 1.0))
        ])
        .is_err()
    );
    assert_eq!(arr(read_radii(&d, a)), [3.0; 4], "refused whole");
}

#[test]
fn legacy_node_edit_writes_only_changed_corners_and_never_the_legacy_key() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let d = unpack(2, &bytes).unwrap();
    // Find the rounded rect.
    let id = d
        .object_ids()
        .into_iter()
        .find(|id| read_radii(&d, *id).tl.as_mm() > 0.0)
        .unwrap();
    let r0 = read_radii(&d, id);
    let legacy = r0.tl.as_mm();
    // Setting the same radius as the legacy value changes nothing.
    let before = vv_of(&d);
    d.set_corner_radius(&[id], mm(legacy)).unwrap();
    assert_eq!(
        vv_of(&d),
        before,
        "setting the legacy value rewrites nothing"
    );
    // Changing one corner writes one register, the others fall back to legacy.
    let before = vv_of(&d);
    d.set_corner_radii(&[(id, r0.with(Corner::Br, mm(1.25)))])
        .unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [0, 0, 1, 0], "{ops}");
    assert_eq!(count(&ops, "corner_radius"), 0);
    assert_eq!(arr(read_radii(&d, id)), [legacy, legacy, 1.25, legacy]);
    // The legacy key is still in the node, unchanged (dead data).
    let saved = pack(&d, "t").unwrap();
    let nodes = raw_nodes(&saved);
    assert!(nodes.iter().any(|n| {
        n.get("corner_radius").and_then(serde_json::Value::as_f64) == Some(legacy)
            && n.get("corner_radius_br")
                .and_then(serde_json::Value::as_f64)
                == Some(1.25)
    }));
    // Setting all four to a new value writes four own registers (legacy fallback differs).
    let before = vv_of(&d);
    d.set_corner_radius(&[id], mm(legacy + 1.0)).unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [1, 1, 1, 1], "{ops}");
}

#[test]
fn resize_rect_keep_writes_no_corner_register_and_proportional_writes_changed_ones() {
    let (d, id, _) = v6_doc([5.0, 0.0, 10.0, 2.0]);
    let r = read_radii(&d, id);
    let before = vv_of(&d);
    d.resize_rect(id, rb(0.0, 0.0, 200.0, 80.0), r, None)
        .unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(
        corner_writes(&ops),
        [0, 0, 0, 0],
        "Keep must write no register: {ops}"
    );
    assert!(count(&ops, "rect_bounds") == 1, "{ops}");
    assert_eq!(arr(read_radii(&d, id)), [5.0, 0.0, 10.0, 2.0]);
    // A scaled set: the zero corner stays zero and is not rewritten.
    let before = vv_of(&d);
    let k = 2.0_f64.sqrt();
    d.resize_rect(
        id,
        rb(0.0, 0.0, 400.0, 80.0),
        radii(5.0 * k, 0.0, 10.0 * k, 2.0 * k),
        None,
    )
    .unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [1, 0, 1, 1], "{ops}");
}

// ---------------------------------------------------------------------
// Merging (ADR 4)
// ---------------------------------------------------------------------

fn merged(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let bytes = zip_with(
        CURRENT_FORMAT_VERSION,
        &loro.export(loro::ExportMode::Snapshot).unwrap(),
        b"{}",
    );
    unpack(9, &bytes).expect("merged document opens")
}

#[test]
fn two_peers_editing_different_corners_both_survive_in_either_merge_order() {
    let (_, id, bytes) = v6_doc([1.0, 2.0, 3.0, 4.0]);
    for swap in [false, true] {
        let a = unpack(11, &bytes).unwrap();
        let b = unpack(12, &bytes).unwrap();
        a.set_corner_radii(&[(id, read_radii(&a, id).with(Corner::Tl, mm(20.0)))])
            .unwrap();
        b.set_corner_radii(&[(id, read_radii(&b, id).with(Corner::Br, mm(30.0)))])
            .unwrap();
        let m = if swap { merged(&b, &a) } else { merged(&a, &b) };
        assert_eq!(
            arr(read_radii(&m, id)),
            [20.0, 2.0, 30.0, 4.0],
            "swap {swap}"
        );
    }
}

#[test]
fn a_linked_write_that_changes_two_corners_cannot_beat_a_concurrent_edit_of_a_third() {
    let (_, id, bytes) = v6_doc([5.0, 5.0, 5.0, 5.0]);
    let a = unpack(11, &bytes).unwrap();
    let b = unpack(12, &bytes).unwrap();
    // A: "set all to 5" (no-op) plus changes nothing.  B: TR to 9.
    a.set_corner_radius(&[id], mm(5.0)).unwrap();
    b.set_corner_radii(&[(id, read_radii(&b, id).with(Corner::Tr, mm(9.0)))])
        .unwrap();
    let m = merged(&a, &b);
    assert_eq!(arr(read_radii(&m, id)), [5.0, 9.0, 5.0, 5.0]);
}

#[test]
fn two_peers_on_the_same_corner_converge_to_one_of_the_two_values() {
    let (_, id, bytes) = v6_doc([1.0, 2.0, 3.0, 4.0]);
    let a = unpack(11, &bytes).unwrap();
    let b = unpack(12, &bytes).unwrap();
    a.set_corner_radii(&[(id, read_radii(&a, id).with(Corner::Tr, mm(7.0)))])
        .unwrap();
    b.set_corner_radii(&[(id, read_radii(&b, id).with(Corner::Tr, mm(8.0)))])
        .unwrap();
    let ab = arr(read_radii(&merged(&a, &b), id));
    let ba = arr(read_radii(&merged(&b, &a), id));
    assert_eq!(ab, ba, "order independent");
    assert!(ab[1] == 7.0 || ab[1] == 8.0, "{ab:?}");
    assert_eq!([ab[0], ab[2], ab[3]], [1.0, 3.0, 4.0]);
}

#[test]
fn legacy_node_edited_at_two_corners_by_two_peers_merges() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let probe = unpack(2, &bytes).unwrap();
    let id = probe
        .object_ids()
        .into_iter()
        .find(|id| read_radii(&probe, *id).tl.as_mm() > 0.0)
        .unwrap();
    let legacy = read_radii(&probe, id).tl.as_mm();
    let a = unpack(11, &bytes).unwrap();
    let b = unpack(12, &bytes).unwrap();
    a.set_corner_radii(&[(id, read_radii(&a, id).with(Corner::Tl, mm(1.0)))])
        .unwrap();
    b.set_corner_radii(&[(id, read_radii(&b, id).with(Corner::Br, mm(2.0)))])
        .unwrap();
    let m = merged(&a, &b);
    assert_eq!(arr(read_radii(&m, id)), [1.0, legacy, 2.0, legacy]);
}

#[test]
fn a_merge_may_exceed_the_side_and_is_neither_damaged_nor_repaired() {
    let (_, id, bytes) = v6_doc([0.0, 0.0, 0.0, 0.0]);
    let a = unpack(11, &bytes).unwrap();
    let b = unpack(12, &bytes).unwrap();
    // 100 mm wide: each within its own limit, sum 160.
    a.set_corner_radii(&[(id, radii(40.0, 0.0, 0.0, 0.0))])
        .unwrap();
    b.set_corner_radii(&[(id, radii(0.0, 40.0, 0.0, 0.0))])
        .unwrap();
    let m = merged(&a, &b);
    assert_eq!(arr(read_radii(&m, id)), [40.0, 40.0, 0.0, 0.0]);
    // Save and reopen survives.
    let again = unpack(3, &pack(&m, "t").unwrap()).unwrap();
    assert_eq!(arr(read_radii(&again, id)), [40.0, 40.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------
// Object to path strips every key (ADR 2) and duplicate copies them
// ---------------------------------------------------------------------

#[test]
fn convert_to_paths_strips_all_five_keys_and_keeps_the_node_id() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let d = unpack(2, &bytes).unwrap();
    let ids = d.object_ids();
    let mut conv = Vec::new();
    for (n, id) in ids.iter().enumerate() {
        let ObjectSnapshot::Primitive(p) = d.object(*id).unwrap() else {
            panic!()
        };
        let anchors: Vec<_> = outline_of(&p.shape)
            .into_iter()
            .enumerate()
            .map(|(i, a)| curvyo_document_core::NewAnchor {
                id: curvyo_document_core::AnchorId::new(77 + n as u64, i as u64 + 1),
                point: a.point,
                handle_in: a.handle_in,
                handle_out: a.handle_out,
                kind: a.kind,
            })
            .collect();
        conv.push((*id, anchors));
    }
    d.convert_to_paths(&conv).unwrap();
    let saved = pack(&d, "t").unwrap();
    for n in raw_nodes(&saved) {
        let o = n.as_object().unwrap();
        assert!(
            !o.keys().any(|k| k.starts_with("corner_radius")),
            "a converted node keeps no radius key: {n}"
        );
        assert!(
            !o.contains_key("rect_bounds") && !o.contains_key("shape"),
            "{n}"
        );
    }
    for id in ids {
        assert!(d.path(id).is_some(), "same id is now a path");
    }
}

#[test]
fn convert_to_paths_of_a_v6_rectangle_with_all_five_keys_present_strips_them() {
    let bytes = raw_rect_file(6, |m| {
        ins_f(m, "corner_radius", 5.0);
        for (k, v) in [("tl", 1.0), ("tr", 2.0), ("br", 3.0), ("bl", 4.0)] {
            ins_f(m, &format!("corner_radius_{k}"), v);
        }
    });
    let d = unpack(2, &bytes).unwrap();
    let id = d.object_ids()[0];
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let anchors: Vec<_> = outline_of(&p.shape)
        .into_iter()
        .enumerate()
        .map(|(i, a)| curvyo_document_core::NewAnchor {
            id: curvyo_document_core::AnchorId::new(5, i as u64 + 1),
            point: a.point,
            handle_in: a.handle_in,
            handle_out: a.handle_out,
            kind: a.kind,
        })
        .collect();
    assert_eq!(
        anchors.len(),
        8,
        "four different non-zero corners -> 8 nodes"
    );
    d.convert_to_paths(&[(id, anchors)]).unwrap();
    for n in raw_nodes(&pack(&d, "t").unwrap()) {
        assert!(
            !n.as_object()
                .unwrap()
                .keys()
                .any(|k| k.starts_with("corner_radius")),
            "{n}"
        );
    }
}

// ---------------------------------------------------------------------
// document.json
// ---------------------------------------------------------------------

fn find_key<'a>(v: &'a serde_json::Value, key: &str, out: &mut Vec<&'a serde_json::Value>) {
    match v {
        serde_json::Value::Object(o) => {
            for (k, x) in o {
                if k == key {
                    out.push(x);
                }
                find_key(x, key, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| find_key(x, key, out)),
        _ => {}
    }
}

#[test]
fn document_json_lists_the_four_stored_radii_in_millimetres() {
    let (d, id, _) = v6_doc([60.0, 70.0, 0.0, 1.25]); // sum > width: stored raw, not effective
    let _ = id;
    let json: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    let mut found = Vec::new();
    find_key(&json, "corner_radii", &mut found);
    assert_eq!(found.len(), 1, "{json}");
    let o = found[0].as_object().unwrap();
    assert_eq!(o["tl"].as_f64(), Some(60.0));
    assert_eq!(o["tr"].as_f64(), Some(70.0));
    assert_eq!(o["br"].as_f64(), Some(0.0));
    assert_eq!(o["bl"].as_f64(), Some(1.25));
    assert_eq!(o.len(), 4);
    let mut legacy = Vec::new();
    find_key(&json, "corner_radius", &mut legacy);
    assert!(
        legacy.is_empty(),
        "the legacy name must not appear in document.json: {json}"
    );
    // The packed member is the same document.
    let bytes = pack(&d, "t").unwrap();
    let member_json: serde_json::Value =
        serde_json::from_slice(&member(&bytes, "document.json")).unwrap();
    assert_eq!(member_json, json);
}

#[test]
fn document_json_of_a_legacy_file_shows_four_equal_radii() {
    let d = unpack(2, &fixture("legacy_corner_radius_v5.curvyo")).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    let mut found = Vec::new();
    find_key(&json, "corner_radii", &mut found);
    assert!(found.len() >= 2, "{json}");
    for o in found {
        let o = o.as_object().unwrap();
        let tl = o["tl"].as_f64().unwrap();
        for k in ["tr", "br", "bl"] {
            assert_eq!(o[k].as_f64().unwrap(), tl);
        }
    }
}

// ---------------------------------------------------------------------
// duplicate copies all registers
// ---------------------------------------------------------------------

#[test]
fn duplicate_keeps_all_four_radii_and_a_legacy_node_copies_as_four_equal() {
    let (d, id, _) = v6_doc([1.0, 2.0, 3.0, 4.0]);
    let copies = d
        .duplicate_objects(
            &[curvyo_document_core::CopySource {
                id,
                anchor_ids: vec![],
            }],
            Vec2::new(5.0, 5.0),
        )
        .unwrap();
    assert_eq!(arr(read_radii(&d, copies[0])), [1.0, 2.0, 3.0, 4.0]);

    let legacy = unpack(2, &fixture("legacy_corner_radius_v5.curvyo")).unwrap();
    let ids = legacy.object_ids();
    for id in ids {
        let want = read_radii(&legacy, id);
        let c = legacy
            .duplicate_objects(
                &[curvyo_document_core::CopySource {
                    id,
                    anchor_ids: vec![],
                }],
                Vec2::new(5.0, 5.0),
            )
            .unwrap();
        assert_eq!(arr(read_radii(&legacy, c[0])), arr(want));
    }
}

#[test]
fn ac14_translate_and_rotate_write_no_radius_register_and_keep_the_radii() {
    let (d, id, _) = v6_doc([1.0, 2.0, 3.0, 4.0]);
    let before = vv_of(&d);
    d.translate_objects(&[id], Vec2::new(7.0, -3.0)).unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [0; 4], "{ops}");
    let before = vv_of(&d);
    let rotated = d.object(id).unwrap().rotated(
        pt(10.0, 10.0),
        curvyo_document_core::Angle::from_radians(0.7),
    );
    d.rotate_object(&rotated).unwrap();
    let ops = ops_since(&d, &before);
    assert_eq!(corner_writes(&ops), [0; 4], "{ops}");
    assert_eq!(arr(read_radii(&d, id)), [1.0, 2.0, 3.0, 4.0]);
    // And the rotation round-trips with the radii through a save.
    let again = unpack(3, &pack(&d, "t").unwrap()).unwrap();
    assert_eq!(arr(read_radii(&again, id)), [1.0, 2.0, 3.0, 4.0]);
    let ObjectSnapshot::Primitive(p) = again.object(id).unwrap() else {
        panic!()
    };
    assert!((p.rotation.as_radians() - 0.7).abs() < 1e-12);
}
