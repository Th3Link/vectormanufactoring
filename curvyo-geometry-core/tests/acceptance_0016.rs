//! Independent acceptance tests for `specs/0016-boolean-operations`, PR 1 (the boolean kernel).
//!
//! Written from the specification only, before reading the implementation. The reference
//! is never the kernel itself: areas come from closed formulas or the shoelace formula,
//! membership from winding numbers over the operands' own outlines (or analytic shapes),
//! compared at sample points outside the tolerance band around every edge.

#![allow(
    missing_docs,
    clippy::pedantic,
    clippy::useless_vec,
    clippy::cloned_ref_to_slice_refs,
    clippy::unwrap_used,
    clippy::expect_used,
    dead_code
)]

use std::f64::consts::PI;
use std::time::Instant;

use curvyo_document_core::{Point, Tolerance, Vec2};
use curvyo_geometry_core::{
    BooleanError, BooleanOp, BooleanResult, Outline, OutlineTriple, boolean, signed_area_mm2,
};

const TOL: Tolerance = Tolerance::from_mm(0.01);
const GRID: f64 = 0.001;

type Anchors = Vec<OutlineTriple>;
type Operand = Vec<Anchors>;

// ---------------------------------------------------------------- builders

fn poly(points: &[(f64, f64)]) -> Anchors {
    points
        .iter()
        .map(|&(x, y)| (Point::new(x, y), Vec2::ZERO, Vec2::ZERO))
        .collect()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Anchors {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

fn circle(cx: f64, cy: f64, r: f64) -> Anchors {
    let k = 0.552_284_749_830_793_4 * r;
    let a = |x: f64, y: f64, hin: (f64, f64), hout: (f64, f64)| {
        (
            Point::new(cx + x, cy + y),
            Vec2::new(hin.0, hin.1),
            Vec2::new(hout.0, hout.1),
        )
    };
    vec![
        a(r, 0.0, (0.0, -k), (0.0, k)),
        a(0.0, r, (k, 0.0), (-k, 0.0)),
        a(-r, 0.0, (0.0, k), (0.0, -k)),
        a(0.0, -r, (-k, 0.0), (k, 0.0)),
    ]
}

/// Reverses an outline (swaps in/out handles), keeping the same curve.
fn reversed(a: &Anchors) -> Anchors {
    a.iter()
        .rev()
        .map(|&(p, hin, hout)| (p, hout, hin))
        .collect()
}

fn reverse_operand(o: &Operand) -> Operand {
    o.iter().map(reversed).collect()
}

fn one(a: Anchors) -> Operand {
    vec![a]
}

fn operand_from_polys(polys: &[Vec<Point>]) -> Operand {
    polys
        .iter()
        .map(|p| p.iter().map(|&q| (q, Vec2::ZERO, Vec2::ZERO)).collect())
        .collect()
}

fn run_with(
    op: BooleanOp,
    operands: &[Operand],
    tol: Tolerance,
) -> Result<BooleanResult, BooleanError> {
    run_flagged(op, operands, tol, true)
}

fn run_flagged(
    op: BooleanOp,
    operands: &[Operand],
    tol: Tolerance,
    closed: bool,
) -> Result<BooleanResult, BooleanError> {
    let outs: Vec<Vec<Outline>> = operands
        .iter()
        .map(|o| o.iter().map(|a| Outline::new(a, closed)).collect())
        .collect();
    let refs: Vec<&[Outline]> = outs.iter().map(|v| v.as_slice()).collect();
    boolean(op, &refs, tol)
}

fn run(op: BooleanOp, operands: &[Operand]) -> Result<BooleanResult, BooleanError> {
    run_with(op, operands, TOL)
}

fn area(r: &BooleanResult) -> f64 {
    r.outlines().iter().map(|o| signed_area_mm2(o)).sum()
}

fn total_length(r: &BooleanResult) -> f64 {
    r.outlines()
        .iter()
        .map(|o| {
            (0..o.len())
                .map(|i| {
                    let (a, b) = (o[i], o[(i + 1) % o.len()]);
                    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
                })
                .sum::<f64>()
        })
        .sum()
}

fn node_count(r: &BooleanResult) -> usize {
    r.outlines().iter().map(Vec::len).sum()
}

const ALL_OPS: [BooleanOp; 4] = [
    BooleanOp::Union,
    BooleanOp::Difference,
    BooleanOp::Intersection,
    BooleanOp::Exclusion,
];

// ---------------------------------------------------------------- reference

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn cross(a: Point, b: Point, p: (f64, f64)) -> f64 {
    (b.x - a.x) * (p.1 - a.y) - (p.0 - a.x) * (b.y - a.y)
}

fn winding(poly: &[Point], p: (f64, f64)) -> i32 {
    let mut w = 0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if a.y <= p.1 {
            if b.y > p.1 && cross(a, b, p) > 0.0 {
                w += 1;
            }
        } else if b.y <= p.1 && cross(a, b, p) < 0.0 {
            w -= 1;
        }
    }
    w
}

fn winding_sum(polys: &[Vec<Point>], p: (f64, f64)) -> i32 {
    polys.iter().map(|q| winding(q, p)).sum()
}

fn seg_dist(p: (f64, f64), a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.x) * dx + (p.1 - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    ((p.0 - a.x - t * dx).powi(2) + (p.1 - a.y - t * dy).powi(2)).sqrt()
}

fn edge_dist(polys: &[Vec<Point>], p: (f64, f64)) -> f64 {
    let mut d = f64::INFINITY;
    for q in polys {
        for i in 0..q.len() {
            d = d.min(seg_dist(p, q[i], q[(i + 1) % q.len()]));
        }
    }
    d
}

fn polys_of(o: &Operand) -> Vec<Vec<Point>> {
    o.iter().map(|a| a.iter().map(|t| t.0).collect()).collect()
}

fn expected_member(op: BooleanOp, flags: &[bool]) -> bool {
    match op {
        BooleanOp::Union => flags.iter().any(|&f| f),
        BooleanOp::Intersection => flags.iter().all(|&f| f),
        BooleanOp::Difference => flags[0] && !flags[1..].iter().any(|&f| f),
        BooleanOp::Exclusion => flags.iter().filter(|&&f| f).count() % 2 == 1,
    }
}

fn near_grid(v: f64) -> bool {
    let u = v / GRID;
    (u - u.round()).abs() < 1e-3
}

fn orient(a: (i128, i128), b: (i128, i128), c: (i128, i128)) -> i128 {
    ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).signum()
}

/// AC 41 / 24: every documented invariant of a result.
fn check_invariants(res: &BooleanResult) {
    let outs = res.outlines();
    assert!(!outs.is_empty());
    let mut pts: Vec<(i128, i128)> = Vec::new();
    for o in outs {
        assert!(o.len() >= 3, "outline with {} nodes", o.len());
        for p in o {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(near_grid(p.x) && near_grid(p.y), "off grid {p:?}");
            pts.push(((p.x / GRID).round() as i128, (p.y / GRID).round() as i128));
        }
        assert!(signed_area_mm2(o) != 0.0, "zero-area outline");
        for i in 0..o.len() {
            let (a, b) = (o[i], o[(i + 1) % o.len()]);
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
            assert!(d >= GRID - 1e-9, "nodes {d} apart");
        }
    }
    // Collinear nodes (except at touching vertices).
    for o in outs {
        for i in 0..o.len() {
            let (a, p, b) = (o[(i + o.len() - 1) % o.len()], o[i], o[(i + 1) % o.len()]);
            let key = ((p.x / GRID).round() as i128, (p.y / GRID).round() as i128);
            if pts.iter().filter(|&&q| q == key).count() > 1 {
                continue;
            }
            let d = seg_dist_line((p.x, p.y), a, b);
            assert!(
                d >= GRID - 1e-9,
                "node {p:?} lies {d} from line of its neighbours"
            );
        }
    }
    // No proper crossings.
    let mut edges = Vec::new();
    for o in outs {
        for i in 0..o.len() {
            let (a, b) = (o[i], o[(i + 1) % o.len()]);
            edges.push((
                ((a.x / GRID).round() as i128, (a.y / GRID).round() as i128),
                ((b.x / GRID).round() as i128, (b.y / GRID).round() as i128),
            ));
        }
    }
    if edges.len() <= 3000 {
        for i in 0..edges.len() {
            for j in i + 1..edges.len() {
                let (a, b) = edges[i];
                let (c, d) = edges[j];
                let (o1, o2) = (orient(a, b, c), orient(a, b, d));
                let (o3, o4) = (orient(c, d, a), orient(c, d, b));
                assert!(
                    !(o1 * o2 < 0 && o3 * o4 < 0),
                    "edges {a:?}-{b:?} and {c:?}-{d:?} cross"
                );
            }
        }
    }
    assert!(area(res) > 0.0, "net area must be positive");
}

fn seg_dist_line(p: (f64, f64), a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return ((p.0 - a.x).powi(2) + (p.1 - a.y).powi(2)).sqrt();
    }
    ((p.0 - a.x) * dy - (p.1 - a.y) * dx).abs() / len
}

// -------------------------------------------------------- random operands

fn rand_operand(r: &mut Rng, grid: bool) -> Operand {
    let n_out = if r.below(4) == 0 { 2 } else { 1 };
    let mut op: Operand = (0..n_out)
        .map(|_| {
            let n = 3 + r.below(6) as usize;
            let pts: Vec<(f64, f64)> = (0..n)
                .map(|_| {
                    if grid {
                        (r.below(9) as f64 * 2.5, r.below(9) as f64 * 2.5)
                    } else {
                        (r.unit() * 20.0, r.unit() * 20.0)
                    }
                })
                .collect();
            poly(&pts)
        })
        .collect();
    if r.below(2) == 0 {
        op = reverse_operand(&op);
    }
    op
}

fn rand_operand_on_grid_mm(r: &mut Rng) -> Operand {
    // Coordinates are exact multiples of 0.001 mm (no snapping error at all).
    let n_out = if r.below(4) == 0 { 2 } else { 1 };
    (0..n_out)
        .map(|_| {
            let n = 3 + r.below(6) as usize;
            let pts: Vec<(f64, f64)> = (0..n)
                .map(|_| {
                    (
                        r.below(20_001) as f64 / 1000.0,
                        r.below(20_001) as f64 / 1000.0,
                    )
                })
                .collect();
            poly(&pts)
        })
        .collect()
}

/// Compares a kernel result against winding-number membership of the operands.
/// Returns the number of sample points compared.
fn compare_membership(op: BooleanOp, operands: &[Operand], samples: usize, rng: &mut Rng) -> usize {
    let polys: Vec<Vec<Vec<Point>>> = operands.iter().map(polys_of).collect();
    let result = run(op, operands);
    let result_polys: Vec<Vec<Point>> = match &result {
        Ok(r) => {
            check_invariants(r);
            r.outlines().to_vec()
        }
        Err(BooleanError::EmptyResult) => Vec::new(),
        Err(BooleanError::EmptyOperands(idx)) => {
            // Each refused operand must really have no painted area away from its edges.
            for &i in idx {
                for _ in 0..samples {
                    let p = (rng.unit() * 20.0, rng.unit() * 20.0);
                    if edge_dist(&polys[i], p) > 0.02 {
                        assert_eq!(
                            winding_sum(&polys[i], p),
                            0,
                            "operand {i} refused as empty but paints {p:?}: {:?}",
                            operands[i]
                        );
                    }
                }
            }
            return 0;
        }
        Err(e) => panic!("unexpected refusal {e:?} for {op:?} {operands:?}"),
    };
    let mut compared = 0;
    for _ in 0..samples {
        let p = (rng.unit() * 20.0, rng.unit() * 20.0);
        if polys.iter().any(|q| edge_dist(q, p) < 0.02) || edge_dist(&result_polys, p) < 0.02 {
            continue;
        }
        let flags: Vec<bool> = polys.iter().map(|q| winding_sum(q, p) != 0).collect();
        let want = expected_member(op, &flags);
        let w = winding_sum(&result_polys, p);
        assert!(
            w == 0 || w == 1,
            "result winding {w} at {p:?} for {op:?} on {operands:?}"
        );
        assert_eq!(
            w != 0,
            want,
            "membership mismatch at {p:?} for {op:?}, flags {flags:?}, operands {operands:?}"
        );
        compared += 1;
    }
    compared
}

// ============================================================== AC 4, 7, 8

#[test]
fn ac07_self_intersecting_pentagram_paints_its_centre() {
    let r = 10.0;
    let star: Vec<(f64, f64)> = [0, 2, 4, 1, 3]
        .iter()
        .map(|&k| {
            let a = -PI / 2.0 + 2.0 * PI * k as f64 / 5.0;
            (50.0 + r * a.cos(), 50.0 + r * a.sin())
        })
        .collect();
    let star_op = one(poly(&star));
    let inner = r * (72f64.to_radians()).cos() / (36f64.to_radians()).cos();
    let painted = 10.0 * 0.5 * r * inner * (36f64.to_radians()).sin();
    // Alone.
    let single = run(BooleanOp::Union, &[star_op.clone()]).unwrap();
    assert!(
        (area(&single) - painted).abs() < 0.04,
        "{} vs {painted}",
        area(&single)
    );
    // With a far square (AC 7 text).
    let sq = one(rect(0.0, 0.0, 10.0, 10.0));
    let u = run(BooleanOp::Union, &[star_op.clone(), sq.clone()]).unwrap();
    assert!((area(&u) - (painted + 100.0)).abs() < 0.05);
    assert_eq!(u.outlines().len(), 2);
    check_invariants(&u);
    // Centre is inside: intersect with small square around the centre.
    let c = one(rect(49.0, 49.0, 51.0, 51.0));
    let i = run(BooleanOp::Intersection, &[star_op, c]).unwrap();
    assert!((area(&i) - 4.0).abs() < 1e-6);
}

#[test]
fn ac04_bowtie_paints_both_lobes() {
    let bow = one(poly(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]));
    let r = run(BooleanOp::Union, &[bow]).unwrap();
    assert!((area(&r) - 50.0).abs() < 1e-6);
    check_invariants(&r);
}

#[test]
fn ac04_same_winding_nested_outlines_fill_nonzero_not_evenodd() {
    // Inner square wound the SAME way: nonzero => solid, no hole.
    let op = vec![rect(0.0, 0.0, 20.0, 20.0), rect(5.0, 5.0, 15.0, 15.0)];
    let r = run(BooleanOp::Union, &[op, one(rect(50.0, 0.0, 60.0, 10.0))]).unwrap();
    assert!((area(&r) - 500.0).abs() < 1e-6);
    assert_eq!(r.outlines().len(), 2);
}

#[test]
fn ac04_opposite_wound_inner_is_a_hole_and_direction_of_whole_operand_is_irrelevant() {
    let mut inner = rect(5.0, 5.0, 15.0, 15.0);
    inner.reverse();
    let op = vec![rect(0.0, 0.0, 20.0, 20.0), inner];
    let far = one(rect(50.0, 0.0, 60.0, 10.0));
    let a = run(BooleanOp::Union, &[op.clone(), far.clone()]).unwrap();
    assert!(
        (area(&a) - 400.0).abs() < 1e-6,
        "300 ring + 100 square, got {}",
        area(&a)
    );
    let b = run(BooleanOp::Union, &[reverse_operand(&op), far]).unwrap();
    assert_eq!(
        a, b,
        "reversing every outline of an operand must not change the result"
    );
}

#[test]
fn ac08_ring_united_with_concentric_disc() {
    let disc20 = one(circle(0.0, 0.0, 20.0));
    let disc10 = one(circle(0.0, 0.0, 10.0));
    let ring = run(BooleanOp::Difference, &[disc20, disc10]).unwrap();
    assert_eq!(ring.outlines().len(), 2);
    let signs: Vec<f64> = ring.outlines().iter().map(|o| signed_area_mm2(o)).collect();
    assert!(signs.iter().filter(|&&a| a > 0.0).count() == 1);
    assert!(signs.iter().filter(|&&a| a < 0.0).count() == 1);
    let ring_op = operand_from_polys(ring.outlines());
    let out = run(
        BooleanOp::Union,
        &[ring_op.clone(), one(circle(0.0, 0.0, 5.0))],
    )
    .unwrap();
    assert_eq!(out.outlines().len(), 3, "outer 20, hole 10, island 5");
    let exact = PI * (400.0 - 100.0 + 25.0);
    let bound = 0.01 * total_length(&out);
    assert!(
        (area(&out) - exact).abs() <= bound,
        "{} vs {exact} (bound {bound})",
        area(&out)
    );
    // Nesting winding: +, -, +  when sorted by |area| descending.
    let mut a: Vec<f64> = out.outlines().iter().map(|o| signed_area_mm2(o)).collect();
    a.sort_by(|x, y| y.abs().partial_cmp(&x.abs()).unwrap());
    assert!(a[0] > 0.0 && a[1] < 0.0 && a[2] > 0.0, "{a:?}");
    check_invariants(&out);
    // Holes stay holes when the ring is united with a far square too.
    let far = run(
        BooleanOp::Union,
        &[ring_op, one(rect(100.0, 100.0, 110.0, 110.0))],
    )
    .unwrap();
    assert_eq!(far.outlines().len(), 3);
}

#[test]
fn ac06_rounded_rectangle_minus_circle_across_a_corner_matches_analytic_shapes() {
    // Rounded rect 0..30 x 0..20, corner radius 4, built from beziers.
    let (w, h, c) = (30.0, 20.0, 4.0);
    let k = 0.552_284_749_830_793_4 * c;
    let t = |x: f64, y: f64, a: (f64, f64), b: (f64, f64)| {
        (Point::new(x, y), Vec2::new(a.0, a.1), Vec2::new(b.0, b.1))
    };
    let rr: Anchors = vec![
        t(c, 0.0, (-k, 0.0), (0.0, 0.0)),
        t(w - c, 0.0, (0.0, 0.0), (k, 0.0)),
        t(w, c, (0.0, -k), (0.0, 0.0)),
        t(w, h - c, (0.0, 0.0), (0.0, k)),
        t(w - c, h, (k, 0.0), (0.0, 0.0)),
        t(c, h, (0.0, 0.0), (-k, 0.0)),
        t(0.0, h - c, (0.0, k), (0.0, 0.0)),
        t(0.0, c, (0.0, 0.0), (0.0, -k)),
    ];
    let disc = circle(2.0, 1.5, 8.0);
    let res = run(BooleanOp::Difference, &[one(rr), one(disc)]).unwrap();
    check_invariants(&res);
    let in_rr = |p: (f64, f64)| {
        let (cx, cy) = (p.0.clamp(c, w - c), p.1.clamp(c, h - c));
        (p.0 >= 0.0 && p.0 <= w && p.1 >= 0.0 && p.1 <= h)
            && ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() <= c
    };
    let mut rng = Rng(7);
    let mut n = 0;
    for _ in 0..20_000 {
        let p = (rng.unit() * 34.0 - 2.0, rng.unit() * 24.0 - 2.0);
        // distance to boundaries (analytic)
        let d_circle = (((p.0 - 2.0).powi(2) + (p.1 - 1.5).powi(2)).sqrt() - 8.0).abs();
        let (cx, cy) = (p.0.clamp(c, w - c), p.1.clamp(c, h - c));
        let dr = (((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() - c).abs();
        let d_rr = if (p.0 >= c && p.0 <= w - c) || (p.1 >= c && p.1 <= h - c) {
            [p.0.abs(), (p.0 - w).abs(), p.1.abs(), (p.1 - h).abs()]
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min)
                .min(dr)
        } else {
            dr
        };
        if d_circle.min(d_rr) < 0.05 {
            continue;
        }
        let want = in_rr(p) && ((p.0 - 2.0).powi(2) + (p.1 - 1.5).powi(2)).sqrt() > 8.0;
        assert_eq!(winding_sum(res.outlines(), p) != 0, want, "at {p:?}");
        n += 1;
    }
    assert!(n > 5000);
}

// ============================================================ AC 10 - 13

#[test]
fn ac10_union_two_offset_squares() {
    let r = run(
        BooleanOp::Union,
        &[
            one(rect(0.0, 0.0, 20.0, 20.0)),
            one(rect(10.0, 10.0, 30.0, 30.0)),
        ],
    )
    .unwrap();
    assert_eq!(r.outlines().len(), 1);
    assert_eq!(node_count(&r), 8);
    assert!((area(&r) - 700.0).abs() < 1e-9);
    check_invariants(&r);
}

#[test]
fn ac11_difference_three_operands_removes_union_of_the_rest_even_when_they_overlap() {
    let a = one(rect(0.0, 0.0, 30.0, 10.0));
    let b = one(rect(5.0, -5.0, 15.0, 15.0));
    let c = one(rect(10.0, -5.0, 20.0, 15.0));
    let r = run(BooleanOp::Difference, &[a.clone(), b.clone(), c.clone()]).unwrap();
    assert!((area(&r) - 150.0).abs() < 1e-9, "got {}", area(&r));
    assert_eq!(r.outlines().len(), 2);
    // The order of the subtrahends is irrelevant.
    let r2 = run(BooleanOp::Difference, &[a, c, b]).unwrap();
    assert_eq!(r, r2);
}

#[test]
fn ac11_difference_uses_first_operand_as_base_and_is_not_symmetric() {
    let a = one(rect(0.0, 0.0, 10.0, 10.0));
    let b = one(rect(5.0, 5.0, 15.0, 15.0));
    let ab = run(BooleanOp::Difference, &[a.clone(), b.clone()]).unwrap();
    let ba = run(BooleanOp::Difference, &[b, a]).unwrap();
    assert!((area(&ab) - 75.0).abs() < 1e-9);
    assert!((area(&ba) - 75.0).abs() < 1e-9);
    assert_ne!(ab, ba);
    // A-B is an L: 6 nodes.
    assert_eq!(node_count(&ab), 6);
}

#[test]
fn ac12_intersection_of_three_discs_is_the_lens_common_to_all() {
    let discs = [
        one(circle(0.0, 0.0, 10.0)),
        one(circle(8.0, 0.0, 10.0)),
        one(circle(4.0, 7.0, 10.0)),
    ];
    let r = run(BooleanOp::Intersection, &discs).unwrap();
    check_invariants(&r);
    let cs = [(0.0, 0.0), (8.0, 0.0), (4.0, 7.0)];
    let mut rng = Rng(11);
    let mut inside = 0;
    for _ in 0..20_000 {
        let p = (rng.unit() * 24.0 - 6.0, rng.unit() * 24.0 - 6.0);
        let d: Vec<f64> = cs
            .iter()
            .map(|c| ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() - 10.0)
            .collect();
        if d.iter().any(|x| x.abs() < 0.05) {
            continue;
        }
        let want = d.iter().all(|&x| x < 0.0);
        assert_eq!(winding_sum(r.outlines(), p) != 0, want, "at {p:?}");
        inside += want as usize;
    }
    assert!(inside > 100);
}

#[test]
fn ac12_pairwise_overlaps_but_no_common_area_is_empty() {
    let a = one(rect(0.0, 0.0, 10.0, 10.0));
    let b = one(rect(5.0, 0.0, 15.0, 10.0));
    let c = one(rect(12.0, 0.0, 20.0, 10.0));
    assert_eq!(
        run(BooleanOp::Intersection, &[a.clone(), b.clone(), c.clone()]),
        Err(BooleanError::EmptyResult)
    );
    // ...but any two of them do overlap.
    assert!(run(BooleanOp::Intersection, &[a, b.clone()]).is_ok());
    assert!(run(BooleanOp::Intersection, &[b, c]).is_ok());
}

#[test]
fn ac13_exclusion_is_odd_coverage_for_three_operands() {
    // Three mutually overlapping discs: the common centre is covered 3x => IN; pair-only
    // zones covered 2x => OUT; single zones 1x => IN.
    let discs = [
        one(circle(0.0, 0.0, 10.0)),
        one(circle(8.0, 0.0, 10.0)),
        one(circle(4.0, 7.0, 10.0)),
    ];
    let r = run(BooleanOp::Exclusion, &discs).unwrap();
    check_invariants(&r);
    assert!(
        winding_sum(r.outlines(), (4.0, 2.5)) != 0,
        "triple zone must be inside"
    );
    let cs = [(0.0, 0.0), (8.0, 0.0), (4.0, 7.0)];
    let mut rng = Rng(5);
    for _ in 0..20_000 {
        let p = (rng.unit() * 26.0 - 7.0, rng.unit() * 26.0 - 7.0);
        let d: Vec<f64> = cs
            .iter()
            .map(|c| ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() - 10.0)
            .collect();
        if d.iter().any(|x| x.abs() < 0.05) {
            continue;
        }
        let n = d.iter().filter(|&&x| x < 0.0).count();
        assert_eq!(
            winding_sum(r.outlines(), p) != 0,
            n % 2 == 1,
            "at {p:?} count {n}"
        );
    }
}

#[test]
fn ac13_reverse_difference_is_difference_with_the_top_operand_first() {
    // The kernel API has no ReverseDifference; the caller puts the top operand first.
    let bottom = one(rect(0.0, 0.0, 10.0, 10.0));
    let top = one(rect(5.0, 5.0, 15.0, 15.0));
    let extra = one(rect(0.0, 0.0, 3.0, 3.0));
    let rev = run(
        BooleanOp::Difference,
        &[top.clone(), bottom.clone(), extra.clone()],
    )
    .unwrap();
    // top - (bottom U extra) = 100 - 25.
    assert!((area(&rev) - 75.0).abs() < 1e-9);
}

// ================================================================== AC 14

/// Returns `None` when an operand is refused as empty, else the worst violation of the
/// three identities relative to the allowed `1e-6 * (|A| + |B|)`.
fn identities(a: &Operand, b: &Operand, bounded: bool) -> Option<f64> {
    let (ra, rb, len) = match (
        run(BooleanOp::Union, std::slice::from_ref(a)),
        run(BooleanOp::Union, std::slice::from_ref(b)),
    ) {
        (Ok(x), Ok(y)) => (area(&x), area(&y), total_length(&x) + total_length(&y)),
        _ => return None,
    };
    let area_of = |r: Result<BooleanResult, BooleanError>| match r {
        Ok(x) => area(&x),
        Err(BooleanError::EmptyResult) => 0.0,
        Err(e) => panic!("unexpected {e:?}"),
    };
    let ab = [a.clone(), b.clone()];
    let u = area_of(run(BooleanOp::Union, &ab));
    let i = area_of(run(BooleanOp::Intersection, &ab));
    let d = area_of(run(BooleanOp::Difference, &ab));
    let x = area_of(run(BooleanOp::Exclusion, &ab));
    // `bounded`: additionally allow one grid pitch of boundary shift along the perimeters, the
    // error AC 24's node removal (nodes within 0.001 mm of a line) can cause.
    let tol = 1e-6 * (ra + rb) + if bounded { GRID * len } else { 0.0 };
    let worst = [
        (u + i - (ra + rb)).abs(),
        (d + i - ra).abs(),
        (x - (ra + rb - 2.0 * i)).abs() / 2.0,
    ]
    .into_iter()
    .fold(0.0, f64::max);
    Some(worst / tol)
}

fn rand_star_shaped(r: &mut Rng) -> Operand {
    // Simple (non self-intersecting) polygon, coordinates on the 0.001 mm grid.
    let n = 3 + r.below(10) as usize;
    let (cx, cy) = (5.0 + r.unit() * 10.0, 5.0 + r.unit() * 10.0);
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let th = 2.0 * PI * (k as f64 + 0.9 * r.unit()) / n as f64;
            let rad = 1.0 + r.unit() * 4.0;
            (
                ((cx + rad * th.cos()) * 1000.0).round() / 1000.0,
                ((cy + rad * th.sin()) * 1000.0).round() / 1000.0,
            )
        })
        .collect();
    one(poly(&pts))
}

fn run_area_identities(seed: u64, pairs: usize, bounded: bool, make: fn(&mut Rng) -> Operand) {
    let mut rng = Rng(seed);
    let (mut checked, mut failed, mut worst) = (0, 0, 0.0f64);
    let mut first = None;
    while checked < pairs {
        let a = make(&mut rng);
        let b = make(&mut rng);
        if let Some(w) = identities(&a, &b, bounded) {
            checked += 1;
            if w > 1.0 {
                failed += 1;
                worst = worst.max(w);
                first.get_or_insert((a, b));
            }
        }
    }
    assert!(
        failed == 0,
        "{failed} of {pairs} pairs violate the 1e-6 area identity, worst = {worst:.1} x the allowed error; first: {first:?}"
    );
}

#[test]
fn ac14_bounded_area_identities_hold_to_one_grid_pitch_of_boundary() {
    run_area_identities(0x5151, 300, true, rand_star_shaped);
    run_area_identities(0xA11CE, 300, true, rand_operand_on_grid_mm);
    run_area_identities(0xB0B, 300, true, |r| rand_operand(r, true));
    run_area_identities(0xC0FFEE, 300, true, |r| rand_operand(r, false));
}

/// Coarse-coordinate pairs without sub-pitch features: the identity holds exactly.
#[test]
fn ac14_area_identities_hold_at_1e_6_for_pairs_without_sub_pitch_features() {
    fn rects(r: &mut Rng) -> Operand {
        let (x, y) = (r.below(20) as f64, r.below(20) as f64);
        let mut o = vec![rect(
            x,
            y,
            x + 1.0 + r.below(15) as f64,
            y + 1.0 + r.below(15) as f64,
        )];
        if r.below(3) == 0 {
            let mut h = rect(x + 0.5, y + 0.5, x + 1.0, y + 1.0);
            h.reverse();
            o.push(h);
        }
        o
    }
    run_area_identities(0xFEED, 300, false, rects);
}

// =============================================== membership vs reference

#[test]
fn membership_matches_winding_reference_for_two_to_four_operands() {
    let mut rng = Rng(0xDEC0DE);
    let mut total = 0;
    for case in 0..240 {
        let grid = case % 2 == 0;
        let n = 2 + (case / 2) % 3;
        let operands: Vec<Operand> = (0..n).map(|_| rand_operand(&mut rng, grid)).collect();
        for op in ALL_OPS {
            total += compare_membership(op, &operands, 300, &mut rng);
        }
    }
    assert!(total > 50_000, "only {total} samples compared");
}

// ============================================================ AC 15 - 17

#[test]
fn ac15_open_operands_are_refused_with_all_indexes_and_nothing_else_checked_first() {
    let sq = one(rect(0.0, 0.0, 10.0, 10.0));
    let open = one(poly(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0)]));
    for op in ALL_OPS {
        let outs: Vec<Vec<Outline>> = [&open, &sq, &open, &sq, &open]
            .iter()
            .enumerate()
            .map(|(i, o)| o.iter().map(|a| Outline::new(a, i % 2 == 1)).collect())
            .collect();
        let refs: Vec<&[Outline]> = outs.iter().map(|v| v.as_slice()).collect();
        assert_eq!(
            boolean(op, &refs, TOL),
            Err(BooleanError::OpenOperands(vec![0, 2, 4])),
            "{op:?}"
        );
    }
}

#[test]
fn ac15_a_compound_operand_with_one_open_outline_is_open() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let b = rect(20.0, 0.0, 30.0, 10.0);
    let outs = vec![vec![Outline::new(&a, true), Outline::new(&b, false)]];
    let refs: Vec<&[Outline]> = outs.iter().map(|v| v.as_slice()).collect();
    assert_eq!(
        boolean(BooleanOp::Union, &refs, TOL),
        Err(BooleanError::OpenOperands(vec![0]))
    );
}

#[test]
fn ac16_operands_without_area_are_refused_with_all_indexes() {
    let sq = one(rect(0.0, 0.0, 10.0, 10.0));
    let collinear = one(poly(&[(0.0, 0.0), (5.0, 5.0), (10.0, 10.0), (3.0, 3.0)]));
    let one_point = one(poly(&[(4.0, 4.0)]));
    let two_nodes = one(poly(&[(1.0, 1.0), (6.0, 6.0)]));
    let same_point = one(poly(&[(2.0, 2.0), (2.0, 2.0), (2.0, 2.0)]));
    let no_anchors: Operand = vec![vec![]];
    let no_outlines: Operand = vec![];
    for op in ALL_OPS {
        let ops = [
            collinear.clone(),
            sq.clone(),
            one_point.clone(),
            two_nodes.clone(),
            same_point.clone(),
            no_anchors.clone(),
            no_outlines.clone(),
        ];
        assert_eq!(
            run(op, &ops),
            Err(BooleanError::EmptyOperands(vec![0, 2, 3, 4, 5, 6])),
            "{op:?}"
        );
    }
}

#[test]
fn ac16_a_sub_grid_square_has_no_area_but_a_one_pitch_square_does_not_panic() {
    let tiny = one(rect(5.0, 5.0, 5.0004, 5.0004));
    let sq = one(rect(0.0, 0.0, 1.0, 1.0));
    assert_eq!(
        run(BooleanOp::Union, &[sq.clone(), tiny]),
        Err(BooleanError::EmptyOperands(vec![1]))
    );
    let pitch = one(rect(5.0, 5.0, 5.001, 5.001));
    match run(BooleanOp::Union, &[sq, pitch]) {
        Ok(r) => check_invariants(&r),
        Err(BooleanError::EmptyOperands(v)) => assert_eq!(v, vec![1]),
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn ac17_empty_results_are_refused() {
    let sq = |x0, y0, x1, y1| one(rect(x0, y0, x1, y1));
    // Intersection of two discs that do not touch.
    assert_eq!(
        run(
            BooleanOp::Intersection,
            &[one(circle(0.0, 0.0, 5.0)), one(circle(20.0, 0.0, 5.0))]
        ),
        Err(BooleanError::EmptyResult)
    );
    // Squares sharing only an edge / only a corner.
    assert_eq!(
        run(
            BooleanOp::Intersection,
            &[sq(0., 0., 10., 10.), sq(10., 0., 20., 10.)]
        ),
        Err(BooleanError::EmptyResult)
    );
    assert_eq!(
        run(
            BooleanOp::Intersection,
            &[sq(0., 0., 10., 10.), sq(10., 10., 20., 20.)]
        ),
        Err(BooleanError::EmptyResult)
    );
    // Difference where the top covers the bottom completely (also exactly equal).
    assert_eq!(
        run(
            BooleanOp::Difference,
            &[sq(2., 2., 8., 8.), sq(0., 0., 10., 10.)]
        ),
        Err(BooleanError::EmptyResult)
    );
    assert_eq!(
        run(
            BooleanOp::Difference,
            &[sq(0., 0., 10., 10.), sq(0., 0., 10., 10.)]
        ),
        Err(BooleanError::EmptyResult)
    );
    // Exclusion of two identical squares, also with opposite winding.
    let s = sq(0., 0., 10., 10.);
    assert_eq!(
        run(BooleanOp::Exclusion, &[s.clone(), s.clone()]),
        Err(BooleanError::EmptyResult)
    );
    assert_eq!(
        run(BooleanOp::Exclusion, &[s.clone(), reverse_operand(&s)]),
        Err(BooleanError::EmptyResult)
    );
    // Exclusion of three identical squares is odd => the square.
    let r = run(BooleanOp::Exclusion, &[s.clone(), s.clone(), s.clone()]).unwrap();
    assert!((area(&r) - 100.0).abs() < 1e-9);
}

#[test]
fn input_errors_have_the_documented_priority() {
    assert_eq!(run(BooleanOp::Union, &[]), Err(BooleanError::NoOperands));
    let sq = one(rect(0.0, 0.0, 10.0, 10.0));
    let nan = one(poly(&[(0.0, 0.0), (f64::NAN, 5.0), (10.0, 0.0)]));
    let inf = one(poly(&[(0.0, 0.0), (f64::INFINITY, 5.0), (10.0, 0.0)]));
    let ninf = one(poly(&[(0.0, 0.0), (5.0, f64::NEG_INFINITY), (10.0, 0.0)]));
    let degenerate = one(poly(&[(1.0, 1.0)]));
    for op in ALL_OPS {
        assert_eq!(
            run(
                op,
                &[
                    sq.clone(),
                    nan.clone(),
                    inf.clone(),
                    ninf.clone(),
                    degenerate.clone()
                ]
            ),
            Err(BooleanError::OutOfRange(vec![1, 2, 3])),
            "{op:?}"
        );
    }
    // NaN in a handle is refused too.
    let mut bad_handle = circle(0.0, 0.0, 5.0);
    bad_handle[1].1 = Vec2::new(f64::NAN, 0.0);
    assert_eq!(
        run(BooleanOp::Union, &[sq.clone(), one(bad_handle)]),
        Err(BooleanError::OutOfRange(vec![1]))
    );
    let mut inf_handle = circle(0.0, 0.0, 5.0);
    inf_handle[2].2 = Vec2::new(0.0, f64::INFINITY);
    assert_eq!(
        run(BooleanOp::Union, &[sq.clone(), one(inf_handle)]),
        Err(BooleanError::OutOfRange(vec![1]))
    );
    // Beyond 10^7 mm in either direction.
    for v in [1.0e7 + 1.0, -1.0e7 - 1.0, 1.0e300] {
        assert_eq!(
            run(
                BooleanOp::Union,
                &[sq.clone(), one(rect(0.0, 0.0, v, 10.0))]
            ),
            Err(BooleanError::OutOfRange(vec![1])),
            "{v}"
        );
    }
    // Open outranks out of range, which outranks empty operands.
    let open_nan = vec![vec![Outline::new(&nan[0], false)]];
    let outs = vec![open_nan[0].clone(), vec![Outline::new(&nan[0], true)]];
    let refs: Vec<&[Outline]> = outs.iter().map(|v| v.as_slice()).collect();
    assert_eq!(
        boolean(BooleanOp::Union, &refs, TOL),
        Err(BooleanError::OpenOperands(vec![0]))
    );
}

#[test]
fn tolerance_must_exceed_two_grid_pitches() {
    let sq = [one(rect(0.0, 0.0, 10.0, 10.0))];
    for t in [0.0, 0.001, 0.002, -1.0, f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(
            run_with(BooleanOp::Union, &sq, Tolerance::from_mm(t)),
            Err(BooleanError::ToleranceTooSmall),
            "{t}"
        );
    }
    for t in [0.0021, 0.003, 0.01, 0.1, 5.0] {
        assert!(
            run_with(BooleanOp::Union, &sq, Tolerance::from_mm(t)).is_ok(),
            "{t}"
        );
    }
    // Invalid tolerance is reported before operand problems? (documented: tolerance first)
    let open = one(poly(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)]));
    assert_eq!(
        run_flagged(BooleanOp::Union, &[open], Tolerance::from_mm(0.0), false),
        Err(BooleanError::ToleranceTooSmall)
    );
}

// ============================================================ AC 24 - 26

#[test]
fn ac24_union_of_two_squares_sharing_a_full_edge_has_four_nodes() {
    let r = run(
        BooleanOp::Union,
        &[
            one(rect(0.0, 0.0, 20.0, 20.0)),
            one(rect(20.0, 0.0, 40.0, 20.0)),
        ],
    )
    .unwrap();
    assert_eq!(node_count(&r), 4);
    assert_eq!(r.outlines().len(), 1);
    check_invariants(&r);
}

#[test]
fn ac24_collinear_and_duplicate_input_nodes_do_not_survive() {
    let mut pts = Vec::new();
    for &(x, y) in &[
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 0.0),
        (10.0, 5.0),
        (10.0, 10.0),
        (5.0, 10.0),
        (0.0, 10.0),
        (0.0, 5.0),
        (0.0, 0.0),
    ] {
        pts.push((x, y));
    }
    let r = run(
        BooleanOp::Union,
        &[one(poly(&pts)), one(rect(50.0, 0.0, 60.0, 10.0))],
    )
    .unwrap();
    assert_eq!(node_count(&r), 8);
    check_invariants(&r);
}

fn sd_union_two_discs(p: (f64, f64), c1: (f64, f64), c2: (f64, f64), r: f64) -> f64 {
    let d1 = ((p.0 - c1.0).powi(2) + (p.1 - c1.1).powi(2)).sqrt() - r;
    let d2 = ((p.0 - c2.0).powi(2) + (p.1 - c2.1).powi(2)).sqrt() - r;
    d1.min(d2)
}

fn check_two_discs(cx: f64, cy: f64, tol_mm: f64) {
    let c1 = (cx, cy);
    let c2 = (cx + 5.0, cy);
    // A tiny curve error of the cubic circle itself: the exact result of the *operands'
    // curves* is what counts; the cubic circle deviates from the true circle by <0.0003 r.
    let curve_dev = 0.000_272 * 10.0;
    let r = run_with(
        BooleanOp::Union,
        &[one(circle(c1.0, c1.1, 10.0)), one(circle(c2.0, c2.1, 10.0))],
        Tolerance::from_mm(tol_mm),
    )
    .unwrap();
    check_invariants(&r);
    assert_eq!(r.outlines().len(), 1);
    let o = &r.outlines()[0];
    for i in 0..o.len() {
        let (a, b) = (o[i], o[(i + 1) % o.len()]);
        let dn = sd_union_two_discs((a.x, a.y), c1, c2, 10.0).abs();
        assert!(
            dn <= tol_mm + curve_dev,
            "node {a:?} {dn} off at tol {tol_mm}"
        );
        for s in 1..8 {
            let t = s as f64 / 8.0;
            let m = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            // Interior chord points lie inside the disc: distance to the true boundary.
            let dm = sd_union_two_discs(m, c1, c2, 10.0).abs();
            assert!(
                dm <= tol_mm + curve_dev,
                "chord point {m:?} {dm} off at tol {tol_mm}"
            );
        }
    }
}

#[test]
fn ac25_two_discs_union_stays_within_the_kernel_tolerance() {
    check_two_discs(0.0, 0.0, 0.01);
}

#[test]
fn ac25_flatten_accuracy_holds_for_other_tolerances() {
    for t in [0.003, 0.005, 0.02, 0.05] {
        check_two_discs(3.3, -7.1, t);
    }
}

#[test]
fn ac42_accuracy_holds_100_000_mm_from_the_origin() {
    check_two_discs(100_000.0, 100_000.0, 0.01);
    check_two_discs(-100_000.0, 99_999.123, 0.01);
}

#[test]
fn ac26_disc_node_budget_between_71_and_142() {
    let r = run(
        BooleanOp::Union,
        &[
            one(circle(0.0, 0.0, 10.0)),
            one(rect(100.0, 0.0, 120.0, 20.0)),
        ],
    )
    .unwrap();
    assert_eq!(r.outlines().len(), 2);
    let disc = r.outlines().iter().find(|o| o.len() != 4).unwrap();
    assert!(
        (71..=142).contains(&disc.len()),
        "disc has {} nodes",
        disc.len()
    );
    let sq = r.outlines().iter().find(|o| o.len() == 4).unwrap();
    assert_eq!(sq.len(), 4);
}

#[test]
fn ac26_node_budget_does_not_depend_on_position_or_winding() {
    for (cx, cy) in [(100_000.0, -100_000.0), (0.0003, 0.0007), (-55.5, 12.25)] {
        for flip in [false, true] {
            let c = circle(cx, cy, 10.0);
            let c = if flip { reversed(&c) } else { c };
            let r = run(
                BooleanOp::Union,
                &[one(c), one(rect(cx + 100.0, cy, cx + 120.0, cy + 20.0))],
            )
            .unwrap();
            let disc = r.outlines().iter().find(|o| o.len() != 4).unwrap();
            assert!(
                (71..=142).contains(&disc.len()),
                "{} nodes at {cx},{cy}",
                disc.len()
            );
        }
    }
}

// ========================================================= AC 39, 40, 41

#[test]
fn ac39_grid_snapping_exact_cases() {
    let a = one(rect(0.0, 0.0, 20.0, 20.0));
    let near = run(
        BooleanOp::Union,
        &[a.clone(), one(rect(20.0004, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert_eq!(near.outlines().len(), 1);
    assert_eq!(node_count(&near), 4, "{:?}", near.outlines());
    let apart = run(
        BooleanOp::Union,
        &[a.clone(), one(rect(20.002, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert_eq!(apart.outlines().len(), 2, "{:?}", apart.outlines());
    // Overlap by 0.0004: also one rectangle.
    let over = run(
        BooleanOp::Union,
        &[a.clone(), one(rect(19.9996, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert_eq!(over.outlines().len(), 1);
    assert_eq!(node_count(&over), 4);
    // Difference of squares whose edges differ by 0.0004 leaves nothing (no sliver).
    assert_eq!(
        run(
            BooleanOp::Difference,
            &[a.clone(), one(rect(0.0, 0.0, 20.0004, 20.0))]
        ),
        Err(BooleanError::EmptyResult)
    );
    // A 0.002 wide sliver is real and stays.
    let sl = run(
        BooleanOp::Difference,
        &[one(rect(0.0, 0.0, 20.002, 20.0)), a.clone()],
    )
    .unwrap();
    assert!((area(&sl) - 0.002 * 20.0).abs() < 1e-9, "{}", area(&sl));
    // Intersection of squares overlapping by exactly 0.002.
    let ov = run(
        BooleanOp::Intersection,
        &[a, one(rect(19.998, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert!((area(&ov) - 0.04).abs() < 1e-9);
}

#[test]
fn ac40_a_identical_squares() {
    let s = one(rect(0.0, 0.0, 10.0, 10.0));
    for op in [BooleanOp::Union, BooleanOp::Intersection] {
        let r = run(op, &[s.clone(), s.clone()]).unwrap();
        assert_eq!(node_count(&r), 4);
        assert!((area(&r) - 100.0).abs() < 1e-9);
    }
    for op in [BooleanOp::Difference, BooleanOp::Exclusion] {
        assert_eq!(
            run(op, &[s.clone(), s.clone()]),
            Err(BooleanError::EmptyResult)
        );
    }
}

#[test]
fn ac40_b_squares_sharing_a_full_edge() {
    let a = one(rect(0.0, 0.0, 10.0, 10.0));
    let b = one(rect(10.0, 0.0, 20.0, 10.0));
    let u = run(BooleanOp::Union, &[a.clone(), b.clone()]).unwrap();
    assert_eq!((u.outlines().len(), node_count(&u)), (1, 4));
    assert_eq!(
        run(BooleanOp::Intersection, &[a.clone(), b.clone()]),
        Err(BooleanError::EmptyResult)
    );
    let d = run(BooleanOp::Difference, &[a.clone(), b.clone()]).unwrap();
    assert!((area(&d) - 100.0).abs() < 1e-9);
    assert_eq!(node_count(&d), 4);
    let x = run(BooleanOp::Exclusion, &[a, b]).unwrap();
    assert!((area(&x) - 200.0).abs() < 1e-9);
    assert_eq!(
        node_count(&x),
        4,
        "exclusion of edge-sharing squares is the merged rectangle"
    );
}

#[test]
fn ac40_c_squares_sharing_only_a_corner() {
    let a = one(rect(0.0, 0.0, 10.0, 10.0));
    let b = one(rect(10.0, 10.0, 20.0, 20.0));
    let u = run(BooleanOp::Union, &[a.clone(), b.clone()]).unwrap();
    assert_eq!(u.outlines().len(), 2, "{:?}", u.outlines());
    assert!(u.outlines().iter().all(|o| o.len() == 4));
    assert!((area(&u) - 200.0).abs() < 1e-9);
    assert_eq!(
        run(BooleanOp::Intersection, &[a.clone(), b.clone()]),
        Err(BooleanError::EmptyResult)
    );
    let d = run(BooleanOp::Difference, &[a.clone(), b.clone()]).unwrap();
    assert_eq!((d.outlines().len(), node_count(&d)), (1, 4));
    let x = run(BooleanOp::Exclusion, &[a, b]).unwrap();
    assert_eq!(x.outlines().len(), 2);
}

#[test]
fn ac40_d_square_inside_with_one_shared_edge() {
    let big = one(rect(0.0, 0.0, 10.0, 10.0));
    let small = one(rect(0.0, 0.0, 5.0, 5.0));
    let u = run(BooleanOp::Union, &[big.clone(), small.clone()]).unwrap();
    assert_eq!(node_count(&u), 4);
    let i = run(BooleanOp::Intersection, &[big.clone(), small.clone()]).unwrap();
    assert_eq!(node_count(&i), 4);
    assert!((area(&i) - 25.0).abs() < 1e-9);
    let d = run(BooleanOp::Difference, &[big.clone(), small.clone()]).unwrap();
    assert_eq!((d.outlines().len(), node_count(&d)), (1, 6));
    assert!((area(&d) - 75.0).abs() < 1e-9);
    let x = run(BooleanOp::Exclusion, &[small.clone(), big.clone()]).unwrap();
    assert!((area(&x) - 75.0).abs() < 1e-9);
    assert_eq!(
        run(BooleanOp::Difference, &[small, big]),
        Err(BooleanError::EmptyResult)
    );
}

#[test]
fn ac40_e_f_duplicated_node_and_zero_length_segment() {
    let dup = poly(&[
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 10.0),
    ]);
    let zero = {
        let mut p = rect(0.0, 0.0, 10.0, 10.0);
        let q = p[1];
        p.insert(1, q);
        p
    };
    for shape in [dup, zero] {
        for op in ALL_OPS {
            let r = run(op, &[one(shape.clone()), one(rect(5.0, 5.0, 15.0, 15.0))]);
            match r {
                Ok(r) => check_invariants(&r),
                Err(e) => panic!("{op:?} refused: {e:?}"),
            }
        }
        let u = run(BooleanOp::Union, &[one(shape.clone()), one(shape.clone())]).unwrap();
        assert_eq!(node_count(&u), 4);
    }
}

#[test]
fn ac40_g_bow_tie_all_operations() {
    let bow = one(poly(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]));
    let sq = one(rect(2.0, 2.0, 8.0, 8.0));
    for op in ALL_OPS {
        match run(op, &[bow.clone(), sq.clone()]) {
            Ok(r) => check_invariants(&r),
            Err(BooleanError::EmptyResult) => {}
            Err(e) => panic!("{op:?}: {e:?}"),
        }
    }
    // bow-tie lobes (left and right triangles meeting at (5,5)): area 50; square covers
    // the middle: intersection = square ∩ lobes.
    let i = run(BooleanOp::Intersection, &[bow.clone(), sq.clone()]).unwrap();
    // left lobe triangle (0,0),(5,5),(0,10); right lobe (10,0),(5,5),(10,10); the square
    // 2..8 takes from each a trapezoid-ish piece: compare with winding reference.
    let polys = polys_of(&bow);
    let mut rng = Rng(3);
    for _ in 0..10_000 {
        let p = (rng.unit() * 12.0 - 1.0, rng.unit() * 12.0 - 1.0);
        if edge_dist(&polys, p) < 0.02 || edge_dist(i.outlines(), p) < 0.02 {
            continue;
        }
        let want = winding_sum(&polys, p) != 0 && p.0 > 2.0 && p.0 < 8.0 && p.1 > 2.0 && p.1 < 8.0;
        assert_eq!(winding_sum(i.outlines(), p) != 0, want, "{p:?}");
    }
}

#[test]
fn ac40_h_sliver_is_kept_and_accurate() {
    let sl = one(rect(0.0, 0.0, 0.005, 100.0));
    let far = one(rect(50.0, 0.0, 60.0, 10.0));
    let u = run(BooleanOp::Union, &[sl.clone(), far.clone()]).unwrap();
    assert_eq!(u.outlines().len(), 2);
    assert!(
        (area(&u) - 100.5).abs() < 0.05,
        "sliver lost or deformed: {}",
        area(&u)
    );
    check_invariants(&u);
    for op in ALL_OPS {
        match run(op, &[sl.clone(), one(rect(-1.0, 40.0, 1.0, 60.0))]) {
            Ok(r) => check_invariants(&r),
            Err(BooleanError::EmptyResult) => {}
            Err(e) => panic!("{op:?}: {e:?}"),
        }
    }
    let i = run(
        BooleanOp::Intersection,
        &[sl.clone(), one(rect(-1.0, 40.0, 1.0, 60.0))],
    )
    .unwrap();
    assert!((area(&i) - 0.005 * 20.0).abs() < 1e-3, "{}", area(&i));
    // Sliver against itself.
    let d = run(BooleanOp::Difference, &[sl.clone(), sl.clone()]);
    assert_eq!(d, Err(BooleanError::EmptyResult));
    let uu = run(BooleanOp::Union, &[sl.clone(), sl]).unwrap();
    assert!((area(&uu) - 0.5).abs() < 0.01);
}

#[test]
fn ac40_i_five_thousand_disjoint_tiny_squares_in_one_operand() {
    let mut op: Operand = Vec::new();
    for i in 0..5000 {
        let (cx, cy) = ((i % 100) as f64 * 3.0, (i / 100) as f64 * 3.0);
        op.push(rect(cx, cy, cx + 1.0, cy + 1.0));
    }
    let big = one(rect(-10.0, -10.0, -5.0, -5.0));
    for o in ALL_OPS {
        let t = Instant::now();
        let r = run(o, &[op.clone(), big.clone()]);
        let el = t.elapsed();
        eprintln!("5000 squares {o:?}: {el:?}");
        match (o, r) {
            (BooleanOp::Union | BooleanOp::Exclusion, Ok(r)) => {
                assert_eq!(r.outlines().len(), 5001);
                assert!((area(&r) - 5025.0).abs() < 1e-6);
                assert!(r.outlines().iter().all(|p| p.len() == 4));
            }
            (BooleanOp::Difference, Ok(r)) => {
                assert_eq!(r.outlines().len(), 5000);
                assert!((area(&r) - 5000.0).abs() < 1e-6);
            }
            (BooleanOp::Intersection, Err(BooleanError::EmptyResult)) => {}
            (o, r) => panic!("{o:?}: {:?}", r.map(|r| r.outlines().len())),
        }
        assert!(
            el.as_secs_f64() < 2.0 * if cfg!(debug_assertions) { 10.0 } else { 1.0 },
            "{o:?} took {el:?}"
        );
    }
}

#[test]
fn degenerate_touching_vertices_and_coincident_edges() {
    // Figure eight / hourglass: two squares touching at one vertex as ONE operand.
    let eight = vec![rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 10.0, 20.0, 20.0)];
    let r = run(
        BooleanOp::Union,
        &[eight.clone(), one(rect(100.0, 0.0, 101.0, 1.0))],
    )
    .unwrap();
    assert!((area(&r) - 201.0).abs() < 1e-9);
    check_invariants(&r);
    // Partial edge sharing (L-ish union of 8 nodes).
    let r = run(
        BooleanOp::Union,
        &[
            one(rect(0.0, 0.0, 10.0, 10.0)),
            one(rect(10.0, 5.0, 20.0, 15.0)),
        ],
    )
    .unwrap();
    assert_eq!((r.outlines().len(), node_count(&r)), (1, 8));
    assert!((area(&r) - 200.0).abs() < 1e-9);
    // Vertex of one square on the middle of another's edge.
    let tri = one(poly(&[(5.0, 10.0), (3.0, 13.0), (7.0, 13.0)]));
    let r = run(BooleanOp::Union, &[one(rect(0.0, 0.0, 10.0, 10.0)), tri]).unwrap();
    assert!((area(&r) - 106.0).abs() < 1e-9);
    assert_eq!(
        r.outlines().len(),
        2,
        "square and triangle touch in one point only"
    );
    check_invariants(&r);
    // Ten squares chained edge-to-edge as separate operands: one rectangle.
    let ops: Vec<Operand> = (0..10)
        .map(|i| one(rect(i as f64 * 5.0, 0.0, i as f64 * 5.0 + 5.0, 5.0)))
        .collect();
    let r = run(BooleanOp::Union, &ops).unwrap();
    assert_eq!(node_count(&r), 4);
    assert!((area(&r) - 250.0).abs() < 1e-9);
}

#[test]
fn many_holes_one_operand_and_many_subtrahends() {
    let n = 20;
    let mut outer = vec![rect(0.0, 0.0, 100.0, 100.0)];
    let mut holes = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let (x, y) = (1.0 + i as f64 * 5.0, 1.0 + j as f64 * 5.0);
            let mut h = rect(x, y, x + 3.0, y + 3.0);
            h.reverse();
            outer.push(h);
            holes.push(one(rect(x, y, x + 3.0, y + 3.0)));
        }
    }
    let expect_area = 10_000.0 - (n * n) as f64 * 9.0;
    let far = one(rect(200.0, 0.0, 210.0, 10.0));
    let u = run(BooleanOp::Union, &[outer, far.clone()]).unwrap();
    assert_eq!(u.outlines().len(), n * n + 2);
    assert!((area(&u) - expect_area - 100.0).abs() < 1e-6);
    check_invariants(&u);
    let mut ops = vec![one(rect(0.0, 0.0, 100.0, 100.0))];
    ops.extend(holes);
    let d = run(BooleanOp::Difference, &ops).unwrap();
    assert_eq!(d.outlines().len(), n * n + 1);
    assert!((area(&d) - expect_area).abs() < 1e-6);
    check_invariants(&d);
}

#[test]
fn deep_nesting_of_alternating_windings() {
    let levels = 25;
    let mut outlines = Vec::new();
    let mut expect = 0.0;
    for k in (1..=levels).rev() {
        let h = k as f64 * 2.0;
        let mut o = rect(-h, -h, h, h);
        let positive = (levels - k) % 2 == 0;
        if !positive {
            o.reverse();
        }
        expect += if positive { 1.0 } else { -1.0 } * (2.0 * h) * (2.0 * h);
        outlines.push(o);
    }
    let far = one(rect(500.0, 0.0, 510.0, 10.0));
    let r = run(BooleanOp::Union, &[outlines, far]).unwrap();
    assert_eq!(r.outlines().len(), levels + 1);
    assert!((area(&r) - expect - 100.0).abs() < 1e-6);
    check_invariants(&r);
}

// ================================================================ AC 42

#[test]
fn ac42_squares_far_from_the_origin_behave_exactly_like_squares_at_the_origin() {
    for off in [100_000.0, -100_000.0, 1.0e6, 9_999_000.0] {
        let r = run(
            BooleanOp::Union,
            &[
                one(rect(off, off, off + 20.0, off + 20.0)),
                one(rect(off + 10.0, off + 10.0, off + 30.0, off + 30.0)),
            ],
        );
        let r = r.unwrap_or_else(|e| panic!("offset {off}: {e:?}"));
        assert_eq!(node_count(&r), 8, "offset {off}");
        assert!(
            (area(&r) - 700.0).abs() < 1e-3,
            "offset {off}: {}",
            area(&r)
        );
        let i = run(
            BooleanOp::Intersection,
            &[
                one(rect(off, off, off + 20.0, off + 20.0)),
                one(rect(off + 10.0, off + 10.0, off + 30.0, off + 30.0)),
            ],
        )
        .unwrap_or_else(|e| panic!("offset {off}: {e:?}"));
        assert!(
            (area(&i) - 100.0).abs() < 1e-3,
            "offset {off}: {}",
            area(&i)
        );
    }
}

#[test]
fn ac42_large_shapes_spanning_the_whole_allowed_range() {
    // 1e7 mm is the documented bound; shapes that span it must still work or be refused,
    // never panic or return garbage.
    let big = one(rect(-9.0e6, -9.0e6, 9.0e6, 9.0e6));
    let small = one(rect(0.0, 0.0, 1.0, 1.0));
    let r = run(BooleanOp::Difference, &[big.clone(), small.clone()]);
    match r {
        Ok(r) => {
            assert_eq!(r.outlines().len(), 2);
            assert!(
                (area(&r) - (18.0e6f64 * 18.0e6 - 1.0)).abs() < 1e6,
                "{}",
                area(&r)
            );
        }
        Err(e) => panic!("range inside 1e7 mm refused: {e:?}"),
    }
    let r = run(BooleanOp::Intersection, &[big, small]).unwrap();
    assert!((area(&r) - 1.0).abs() < 1e-6);
}

// ================================================================ AC 43

#[test]
fn ac43_repeated_runs_are_identical_node_for_node() {
    let mut rng = Rng(99);
    for _ in 0..60 {
        let operands: Vec<Operand> = (0..3).map(|_| rand_operand(&mut rng, false)).collect();
        for op in ALL_OPS {
            let a = run(op, &operands);
            let b = run(op, &operands);
            assert_eq!(a, b);
        }
    }
}

#[test]
fn ac43_extra_canonical_form_ignores_start_node_winding_and_operand_order() {
    let mut rng = Rng(1234);
    let mut mismatches = Vec::new();
    let mut permuted_fold_diffs = 0;
    for case in 0..120 {
        let operands: Vec<Operand> = (0..3)
            .map(|_| rand_operand(&mut rng, case % 2 == 0))
            .collect();
        for op in ALL_OPS {
            let base = run(op, &operands);
            // Reverse every operand.
            let rev: Vec<Operand> = operands.iter().map(reverse_operand).collect();
            // Rotate the start node of each outline.
            let rot: Vec<Operand> = operands
                .iter()
                .map(|o| {
                    o.iter()
                        .map(|a| {
                            let mut a = a.clone();
                            let n = a.len();
                            a.rotate_left(1 % n.max(1));
                            a
                        })
                        .collect()
                })
                .collect();
            // Permute operands (keep the base first for Difference).
            let mut perm = operands.clone();
            if op == BooleanOp::Difference {
                perm[1..].reverse();
            } else {
                perm.reverse();
            }
            for (name, v) in [("reversed", rev), ("rotated", rot), ("permuted", perm)] {
                let other = run(op, &v);
                if name == "permuted" && other.is_err() && base.is_err() {
                    continue; // refusal indexes are operand positions
                }
                if other != base {
                    let d = match (&other, &base) {
                        (Ok(x), Ok(y)) => (area(x) - area(y)).abs(),
                        _ => f64::NAN,
                    };
                    let fold_op = matches!(op, BooleanOp::Intersection | BooleanOp::Exclusion);
                    if name == "permuted" && fold_op {
                        // Observation, bounded below: the pairwise fold order changes rounding.
                        assert!(
                            d < GRID * base.as_ref().map_or(0.0, total_length),
                            "{op:?} permuted area diff {d}: {:?} vs {:?} operands {operands:?}",
                            other.as_ref().map(area),
                            base.as_ref().map(area)
                        );
                        permuted_fold_diffs += 1;
                    } else {
                        mismatches.push(format!("{op:?} {name} case {case} area diff {d:.6}"));
                    }
                }
            }
        }
    }
    eprintln!(
        "Intersection/Exclusion results that differ node for node after permuting operands: {permuted_fold_diffs} of 240"
    );
    let by = |k: &str| mismatches.iter().filter(|m| m.contains(k)).count();
    assert!(
        mismatches.is_empty(),
        "{} mismatches (reversed {}, rotated {}, permuted {}), first: {:?}",
        mismatches.len(),
        by("reversed"),
        by("rotated"),
        by("permuted"),
        &mismatches[..mismatches.len().min(6)]
    );
}

#[test]
fn ac43_documented_orientation_and_ordering() {
    let r = run(
        BooleanOp::Union,
        &[
            one(rect(30.0, 0.0, 40.0, 10.0)),
            one(rect(0.0, 0.0, 10.0, 10.0)),
        ],
    )
    .unwrap();
    // Outlines sorted by smallest vertex (x then y); every outline starts there.
    let firsts: Vec<(f64, f64)> = r.outlines().iter().map(|o| (o[0].x, o[0].y)).collect();
    assert_eq!(firsts, vec![(0.0, 0.0), (30.0, 0.0)]);
    for o in r.outlines() {
        let min = o
            .iter()
            .map(|p| (p.x, p.y))
            .fold(
                (f64::INFINITY, f64::INFINITY),
                |a, b| if b < a { b } else { a },
            );
        assert_eq!((o[0].x, o[0].y), min);
        assert!(signed_area_mm2(o) > 0.0, "outer outlines positive");
    }
    // The signed area convention itself.
    let ccw = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ];
    assert!((signed_area_mm2(&ccw) - 1.0).abs() < 1e-12);
    assert_eq!(signed_area_mm2(&[]), 0.0);
    assert_eq!(signed_area_mm2(&[Point::new(1.0, 1.0)]), 0.0);
}

// ============================================================ AC 44 - 46

fn wobble(cx: f64, cy: f64, r: f64, n: usize, phase: f64) -> Anchors {
    let pos = |i: usize| {
        let th = 2.0 * PI * (i % n) as f64 / n as f64;
        let rr = r * (1.0 + 0.1 * (7.0 * th + phase).sin());
        (cx + rr * th.cos(), cy + rr * th.sin())
    };
    (0..n)
        .map(|i| {
            let (p, prev, next) = (pos(i), pos(i + n - 1), pos(i + 1));
            let t = ((next.0 - prev.0) / 6.0, (next.1 - prev.1) / 6.0);
            (
                Point::new(p.0, p.1),
                Vec2::new(-t.0, -t.1),
                Vec2::new(t.0, t.1),
            )
        })
        .collect()
}

fn perf_case(n: usize, budget_ms: u128) {
    let a = one(wobble(0.0, 0.0, 100.0, n, 0.0));
    let b = one(wobble(40.0, 10.0, 100.0, n, 1.0));
    for op in [
        BooleanOp::Union,
        BooleanOp::Difference,
        BooleanOp::Intersection,
    ] {
        let t = Instant::now();
        let r = run(op, &[a.clone(), b.clone()]).unwrap();
        let ms = t.elapsed().as_millis();
        eprintln!(
            "{n} nodes x2 {op:?}: {ms} ms, {} outlines, {} nodes",
            r.outlines().len(),
            node_count(&r)
        );
        check_invariants_light(&r);
        if !cfg!(debug_assertions) {
            assert!(
                ms <= budget_ms,
                "{op:?} with {n} nodes took {ms} ms (budget {budget_ms})"
            );
        }
    }
}

fn check_invariants_light(r: &BooleanResult) {
    for o in r.outlines() {
        assert!(o.len() >= 3 && signed_area_mm2(o) != 0.0);
    }
    assert!(area(r) > 0.0);
}

#[test]
fn ac44_two_operands_of_1000_curved_nodes_within_100_ms_release() {
    perf_case(1000, 100);
}

#[test]
fn ac45_two_operands_of_10000_curved_nodes_within_2_s_release() {
    perf_case(10_000, 2000);
}

#[test]
fn ac46_kernel_union_of_1000_overlapping_rectangles_within_1_s_release() {
    let mut ops = Vec::new();
    for i in 0..1000 {
        let (x, y) = ((i % 40) as f64 * 9.0, (i / 40) as f64 * 9.0);
        ops.push(one(rect(x, y, x + 10.0, y + 10.0)));
    }
    let t = Instant::now();
    let r = run(BooleanOp::Union, &ops).unwrap();
    let el = t.elapsed();
    eprintln!("1000 rects union {el:?}");
    assert_eq!(r.outlines().len(), 1);
    if !cfg!(debug_assertions) {
        assert!(el.as_millis() <= 1000);
    }
    // Same for the other folds (not budgeted by the spec, but must not blow up).
    for op in [
        BooleanOp::Exclusion,
        BooleanOp::Intersection,
        BooleanOp::Difference,
    ] {
        let t = Instant::now();
        let _ = run(op, &ops);
        eprintln!("1000 rects {op:?} {:?}", t.elapsed());
    }
}

/// A noisy polygon whose nodes cannot be simplified away: the result keeps ~all nodes.
fn noisy(cx: f64, cy: f64, r: f64, n: usize, seed: u64) -> Anchors {
    let mut rng = Rng(seed);
    let spacing = 2.0 * PI * r / n as f64;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let th = 2.0 * PI * i as f64 / n as f64;
            let rr = r + (rng.unit() - 0.5) * spacing * 0.8;
            (cx + rr * th.cos(), cy + rr * th.sin())
        })
        .collect();
    poly(&pts)
}

#[test]
fn ac44_ac45_incompressible_noisy_operands_within_budget_release() {
    for (n, budget_ms) in [(1000usize, 100u128), (10_000, 2000)] {
        let a = one(noisy(0.0, 0.0, 100.0, n, 1));
        let b = one(noisy(40.0, 10.0, 100.0, n, 2));
        for op in [
            BooleanOp::Union,
            BooleanOp::Difference,
            BooleanOp::Intersection,
        ] {
            let t = Instant::now();
            let r = run(op, &[a.clone(), b.clone()]).unwrap();
            let ms = t.elapsed().as_millis();
            eprintln!("noisy {n} x2 {op:?}: {ms} ms, {} nodes out", node_count(&r));
            assert!(
                node_count(&r) > n / 2,
                "the noise must survive for the case to mean anything"
            );
            check_invariants_light(&r);
            if !cfg!(debug_assertions) {
                assert!(ms <= budget_ms, "{op:?} with {n} noisy nodes took {ms} ms");
            }
        }
    }
}

// ============================================================ white-box edges

/// Directed Hausdorff distance: the largest distance from a vertex or edge midpoint of `from`
/// to the closed polyline `to`.
fn directed_hausdorff(from: &[Point], to: &[Point]) -> f64 {
    let mut worst = 0.0f64;
    let n = from.len();
    for i in 0..n {
        let (a, b) = (from[i], from[(i + 1) % n]);
        for t in [0.0, 0.5] {
            let p = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            worst = worst.max(edge_dist(&[to.to_vec()], p));
        }
    }
    worst
}

#[test]
fn white_dense_polyline_circle_stays_within_tolerance_of_its_input() {
    // The near-collinear cleanup judges each vertex against the neighbours that remain, so a
    // densely sampled arc can drift away from the input by more than the 0.01 mm tolerance.
    let r = 10.0;
    let mut worst = Vec::new();
    for n in [500usize, 2000, 3000, 5000, 20_000, 100_000] {
        let pts: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let th = 2.0 * PI * i as f64 / n as f64;
                (r * th.cos(), r * th.sin())
            })
            .collect();
        let input: Vec<Point> = pts.iter().map(|&(x, y)| Point::new(x, y)).collect();
        let res = run(
            BooleanOp::Union,
            &[one(poly(&pts)), one(rect(100.0, 0.0, 110.0, 10.0))],
        )
        .unwrap();
        let disc = res.outlines().iter().find(|o| o.len() != 4).unwrap();
        let out_in = directed_hausdorff(disc, &input);
        let in_out = directed_hausdorff(&input, disc);
        eprintln!(
            "n={n}: {n} -> {} nodes, result->input {out_in:.5}, input->result {in_out:.5}",
            disc.len()
        );
        if out_in > 0.0101 || in_out > 0.0101 {
            worst.push(format!(
                "n={n} ({} nodes out): {:.4} mm",
                disc.len(),
                out_in.max(in_out)
            ));
        }
    }
    assert!(
        worst.is_empty(),
        "result strays more than 0.01 mm from its input polyline: {worst:?}"
    );
}

#[test]
fn white_curved_result_against_independent_dense_flatten() {
    let a = wobble(100_000.0, -100_000.0, 100.0, 12, 0.3);
    // Independent dense flatten of the cubic outline.
    let mut dense = Vec::new();
    for i in 0..a.len() {
        let (p0, p3) = (a[i], a[(i + 1) % a.len()]);
        let c = [p0.0, p0.0.translated(p0.2), p3.0.translated(p3.1), p3.0];
        for s in 0..600 {
            let t = s as f64 / 600.0;
            let m = 1.0 - t;
            let w = [m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t];
            dense.push(Point::new(
                w.iter().zip(&c).map(|(w, p)| w * p.x).sum(),
                w.iter().zip(&c).map(|(w, p)| w * p.y).sum(),
            ));
        }
    }
    let far = one(rect(
        100_000.0 + 1000.0,
        -100_000.0,
        100_000.0 + 1010.0,
        -99_990.0,
    ));
    let res = run(BooleanOp::Union, &[one(a), far]).unwrap();
    check_invariants(&res);
    let curve = res.outlines().iter().find(|o| o.len() != 4).unwrap();
    let d1 = directed_hausdorff(curve, &dense);
    let d2 = directed_hausdorff(&dense[..], curve);
    eprintln!(
        "result->curve {d1:.5}, curve->result {d2:.5}, {} nodes",
        curve.len()
    );
    assert!(d1 <= 0.0101, "result strays {d1} mm from the curve");
    assert!(d2 <= 0.0101, "curve strays {d2} mm from the result");
}

#[test]
fn white_two_node_closed_path_with_curved_handles_encloses_area() {
    let lens: Anchors = vec![
        (
            Point::new(0.0, 0.0),
            Vec2::new(3.0, 6.0),
            Vec2::new(3.0, -6.0),
        ),
        (
            Point::new(10.0, 0.0),
            Vec2::new(-3.0, -6.0),
            Vec2::new(-3.0, 6.0),
        ),
    ];
    // Reference area by dense sampling and shoelace over the real cubics.
    let mut pts = Vec::new();
    for i in 0..2 {
        let (p0, p3) = (lens[i], lens[(i + 1) % 2]);
        let c = [p0.0, p0.0.translated(p0.2), p3.0.translated(p3.1), p3.0];
        for s in 0..5000 {
            let t = s as f64 / 5000.0;
            let m = 1.0 - t;
            let w = [m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t];
            pts.push(Point::new(
                w.iter().zip(&c).map(|(w, p)| w * p.x).sum(),
                w.iter().zip(&c).map(|(w, p)| w * p.y).sum(),
            ));
        }
    }
    let expect = signed_area_mm2(&pts).abs();
    assert!(expect > 30.0, "test shape must enclose area, got {expect}");
    let r = run(BooleanOp::Union, &[one(lens)]).expect("a curved two-node outline paints an area");
    // An inscribed polygon within 0.01 mm of the curve loses at most 0.01 mm x length of area.
    assert!(
        (area(&r) - expect).abs() <= 0.01 * total_length(&r),
        "{} vs {expect}",
        area(&r)
    );
}

#[test]
fn white_one_node_closed_path_is_refused_not_panicking() {
    let single: Anchors = vec![(
        Point::new(1.0, 1.0),
        Vec2::new(-3.0, 4.0),
        Vec2::new(3.0, 4.0),
    )];
    for op in ALL_OPS {
        match run(op, &[one(single.clone()), one(rect(0.0, 0.0, 5.0, 5.0))]) {
            Err(BooleanError::EmptyOperands(v)) => assert_eq!(v, vec![0]),
            other => panic!("{op:?}: {:?}", other.map(|r| r.outlines().len())),
        }
    }
}

#[test]
fn white_extreme_tolerances_never_panic() {
    let sq = [one(circle(0.0, 0.0, 10.0))];
    for t in [f64::INFINITY, 1.0e300, 1.0e10, f64::MIN_POSITIVE, 0.0021] {
        let r = run_with(BooleanOp::Union, &sq, Tolerance::from_mm(t));
        match r {
            Ok(r) => check_invariants(&r),
            Err(BooleanError::ToleranceTooSmall) => {
                assert!(t.is_infinite() || t <= 0.002, "{t} must be accepted")
            }
            Err(e) => panic!("{t}: {e:?}"),
        }
    }
    // A huge tolerance is legal and yields a coarse polygon (at least the 4 anchors).
    let coarse = run_with(BooleanOp::Union, &sq, Tolerance::from_mm(1.0e6)).unwrap();
    assert!(node_count(&coarse) >= 3);
}

#[test]
fn white_coordinates_at_exactly_the_bound_are_accepted() {
    let a = one(rect(0.0, 0.0, 1.0e7, 1.0e7));
    let b = one(rect(-1.0e7, -1.0e7, 5.0e6, 5.0e6));
    let u = run(BooleanOp::Union, &[a.clone(), b.clone()]).expect("1e7 is the documented bound");
    assert_eq!(node_count(&u), 8);
    let i = run(BooleanOp::Intersection, &[a, b]).unwrap();
    assert!((area(&i) - 2.5e13).abs() < 1.0);
}

#[test]
fn white_near_coincident_curves_leave_no_slivers_or_spikes() {
    for shift in [0.0, 0.0004, 0.0006, 0.001, 0.0014, 0.002, 0.003, 0.0101] {
        let a = one(circle(0.0, 0.0, 10.0));
        let b = one(circle(shift, 0.0, 10.0));
        for op in ALL_OPS {
            match run(op, &[a.clone(), b.clone()]) {
                Ok(r) => {
                    check_invariants(&r);
                    // A crescent can only be as wide as the shift; with 2 grid pitches or
                    // less of noise it must have been dropped rather than kept as a ribbon.
                    if matches!(op, BooleanOp::Exclusion | BooleanOp::Difference) {
                        let max_width = shift + 0.011;
                        assert!(
                            area(&r) <= 2.0 * PI * 10.0 * max_width,
                            "{op:?} shift {shift}"
                        );
                    }
                }
                Err(BooleanError::EmptyResult) => {
                    assert!(
                        matches!(op, BooleanOp::Difference | BooleanOp::Exclusion),
                        "{op:?} shift {shift}"
                    );
                }
                Err(e) => panic!("{op:?} shift {shift}: {e:?}"),
            }
        }
    }
}

#[test]
fn white_a_result_fed_back_is_a_fixed_point() {
    // Chained use: the maker unites, then unites the result with something far away, or
    // intersects it with itself. The kernel's own output must survive a second pass.
    let mut rng = Rng(777);
    let (mut checked, mut moved) = (0, Vec::new());
    for case in 0..200 {
        let operands: Vec<Operand> = (0..2)
            .map(|_| rand_operand(&mut rng, case % 2 == 0))
            .collect();
        for op in ALL_OPS {
            let Ok(first) = run(op, &operands) else {
                continue;
            };
            let again = operand_from_polys(first.outlines());
            for (name, second) in [
                ("union", run(BooleanOp::Union, std::slice::from_ref(&again))),
                (
                    "intersect self",
                    run(BooleanOp::Intersection, &[again.clone(), again.clone()]),
                ),
                (
                    "minus far",
                    run(
                        BooleanOp::Difference,
                        &[again.clone(), one(rect(500.0, 500.0, 501.0, 501.0))],
                    ),
                ),
            ] {
                checked += 1;
                if second.as_ref().ok() != Some(&first) {
                    moved.push(format!("{op:?} then {name} (case {case})"));
                }
            }
        }
    }
    assert!(
        moved.is_empty(),
        "{} of {checked} second passes changed the result, first: {:?}",
        moved.len(),
        &moved[..moved.len().min(4)]
    );
}

#[test]
fn white_invariant_stress_on_many_random_inputs() {
    let mut rng = Rng(0xFACADE);
    let mut results = 0;
    for case in 0..1500 {
        let n = 2 + (case % 3);
        let operands: Vec<Operand> = (0..n)
            .map(|_| match case % 3 {
                0 => rand_operand(&mut rng, true),
                1 => rand_operand(&mut rng, false),
                _ => rand_operand_on_grid_mm(&mut rng),
            })
            .collect();
        for op in ALL_OPS {
            if let Ok(r) = run(op, &operands) {
                check_invariants(&r);
                results += 1;
            }
        }
    }
    assert!(results > 3000);
}

#[test]
fn white_huge_handles_do_not_explode() {
    // A single cubic with handles reaching the range bound must flatten to a bounded number of
    // points quickly (step count grows with sqrt of the handle size).
    let wild: Anchors = vec![
        (
            Point::new(0.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(9.0e6, 9.0e6),
        ),
        (Point::new(10.0, 0.0), Vec2::new(-9.0e6, 9.0e6), Vec2::ZERO),
        (Point::new(10.0, 10.0), Vec2::ZERO, Vec2::ZERO),
    ];
    let t = Instant::now();
    let r = run(
        BooleanOp::Union,
        &[one(wild), one(rect(0.0, 0.0, 5.0, 5.0))],
    );
    eprintln!(
        "wild handles: {:?} in {:?}",
        r.as_ref().map(node_count),
        t.elapsed()
    );
    assert!(t.elapsed().as_secs_f64() < if cfg!(debug_assertions) { 60.0 } else { 5.0 });
}
