//! The operations of `specs/0016-boolean-operations` at kernel level: acceptance criteria 7, 8,
//! 10 to 13, 17, 24 to 26, 39 and 42 on exact, hand-checked shapes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    missing_docs
)]

mod common;

use common::{
    Operand, area, circle, covers, node_count, ok, operand_of, perimeter, polygon, rect, run,
    run_at, single,
};
use curvyo_document_core::{Point, Tolerance};
use curvyo_geometry_core::{BooleanError, BooleanOp};

const ALL_OPS: [BooleanOp; 4] = [
    BooleanOp::Union,
    BooleanOp::Difference,
    BooleanOp::Intersection,
    BooleanOp::Exclusion,
];

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} is not within {tolerance} of {expected}"
    );
}

/// Criterion 10: two squares of side 20 mm, the second offset by (10, 10), unite to one outline
/// of 8 nodes and an area of 700 mm².
#[test]
fn union_of_two_offset_squares_is_one_outline_of_eight_nodes() {
    let result = ok(
        BooleanOp::Union,
        &[
            single(rect(0.0, 0.0, 20.0, 20.0)),
            single(rect(10.0, 10.0, 30.0, 30.0)),
        ],
    );
    assert_eq!(result.outlines().len(), 1);
    assert_eq!(node_count(&result), 8);
    assert_close(area(&result), 700.0, 1e-9, "area");
}

/// Criterion 24: the union of two squares sharing a full edge has 4 nodes.
#[test]
fn union_of_squares_sharing_an_edge_has_four_nodes() {
    let result = ok(
        BooleanOp::Union,
        &[
            single(rect(0.0, 0.0, 20.0, 20.0)),
            single(rect(20.0, 0.0, 40.0, 20.0)),
        ],
    );
    assert_eq!(node_count(&result), 4);
    assert_close(area(&result), 800.0, 1e-9, "area");
}

/// Criterion 11: three operands A (first), B, C give A - (B u C).
#[test]
fn difference_subtracts_the_union_of_all_other_operands() {
    let a = single(rect(0.0, 0.0, 40.0, 40.0));
    let b = single(rect(-10.0, -10.0, 20.0, 20.0));
    let c = single(rect(10.0, 10.0, 50.0, 50.0));
    let result = ok(BooleanOp::Difference, &[a, b, c]);
    // 1600 - (20x20 of B inside A) - (30x30 of C inside A) + (10x10 shared by B and C)
    assert_close(area(&result), 1600.0 - 400.0 - 900.0 + 100.0, 1e-9, "area");
    assert!(covers(&result, Point::new(35.0, 5.0)));
    assert!(!covers(&result, Point::new(15.0, 15.0)));
    assert!(!covers(&result, Point::new(30.0, 30.0)));
}

/// Criterion 12: three overlapping discs give the lens common to all three.
#[test]
fn intersection_of_three_discs_is_their_common_lens() {
    let discs = [
        single(circle(0.0, 0.0, 10.0)),
        single(circle(6.0, 0.0, 10.0)),
        single(circle(3.0, 5.0, 10.0)),
    ];
    let result = ok(BooleanOp::Intersection, &discs);
    for ix in -20..=20 {
        for iy in -20..=20 {
            let p = Point::new(f64::from(ix) * 0.7, f64::from(iy) * 0.7);
            let depth = [(0.0, 0.0), (6.0, 0.0), (3.0, 5.0)]
                .iter()
                .map(|&(cx, cy)| 10.0 - (p.x - cx).hypot(p.y - cy))
                .fold(f64::INFINITY, f64::min);
            if depth.abs() > 0.05 {
                assert_eq!(covers(&result, p), depth > 0.0, "at {p:?}");
            }
        }
    }
}

/// Criterion 13: exclusion covers what an odd number of operands cover, for three operands too.
#[test]
fn exclusion_of_three_operands_is_odd_coverage() {
    let rects = [
        rect(0.0, 0.0, 20.0, 20.0),
        rect(10.0, 0.0, 30.0, 20.0),
        rect(15.0, 0.0, 35.0, 20.0),
    ];
    let operands: Vec<Operand> = rects.iter().cloned().map(single).collect();
    let result = ok(BooleanOp::Exclusion, &operands);
    for ix in 0..36 {
        let x = f64::from(ix) + 0.5;
        let count = [(0.0, 20.0), (10.0, 30.0), (15.0, 35.0)]
            .iter()
            .filter(|(lo, hi)| (*lo..*hi).contains(&x))
            .count();
        assert_eq!(
            covers(&result, Point::new(x, 10.0)),
            count % 2 == 1,
            "x={x}"
        );
    }
}

/// A reverse difference is a difference with the top-most operand first (adrs.md).
#[test]
fn swapping_the_operands_of_a_difference_gives_the_reverse_difference() {
    let low = single(rect(0.0, 0.0, 20.0, 20.0));
    let top = single(rect(10.0, 10.0, 30.0, 30.0));
    let forward = ok(BooleanOp::Difference, &[low.clone(), top.clone()]);
    let reverse = ok(BooleanOp::Difference, &[top, low]);
    assert_close(area(&forward), 300.0, 1e-9, "forward");
    assert_close(area(&reverse), 300.0, 1e-9, "reverse");
    assert!(covers(&forward, Point::new(5.0, 5.0)));
    assert!(covers(&reverse, Point::new(25.0, 25.0)));
    assert!(!covers(&reverse, Point::new(5.0, 5.0)));
}

/// An operand wound the other way paints the same region (the kernel normalizes each operand on
/// its own); under a plain nonzero union the two would cancel where they overlap.
#[test]
fn operands_wound_in_opposite_directions_behave_alike() {
    let a = rect(0.0, 0.0, 20.0, 20.0);
    let mut b = rect(10.0, 10.0, 30.0, 30.0);
    b.reverse();
    for op in ALL_OPS {
        let alike = run(
            op,
            &[single(a.clone()), single(rect(10.0, 10.0, 30.0, 30.0))],
        );
        let opposite = run(op, &[single(a.clone()), single(b.clone())]);
        assert_eq!(alike, opposite, "{op:?}");
    }
}

/// Criterion 7: a five-point star drawn as one closed path of five nodes that crosses itself
/// has its centre in its region; its area is the star outline's.
#[test]
fn a_self_intersecting_star_includes_its_centre() {
    let radius = 10.0;
    let vertex = |k: i32| {
        let angle = std::f64::consts::TAU * f64::from(k) / 5.0 - std::f64::consts::FRAC_PI_2;
        (radius * angle.cos(), radius * angle.sin())
    };
    let star = polygon(&[vertex(0), vertex(2), vertex(4), vertex(1), vertex(3)]);
    let square = rect(100.0, 0.0, 120.0, 20.0);
    let result = ok(BooleanOp::Union, &[single(star), single(square)]);
    assert!(covers(&result, Point::new(0.0, 0.0)));
    // Ten-sided star outline: R = 10, r = R cos(72 deg) / cos(36 deg).
    let inner = radius * (72.0_f64.to_radians().cos() / 36.0_f64.to_radians().cos());
    let star_area = 5.0 * radius * inner * 36.0_f64.to_radians().sin();
    assert_close(
        area(&result),
        star_area + 400.0,
        0.01 * perimeter(&result),
        "area",
    );
}

/// Criterion 8: a ring (disc of radius 20 minus a concentric disc of radius 10) united with a
/// concentric disc of radius 5 gives three outlines and an area of pi (400 - 100 + 25).
#[test]
fn a_compound_operand_keeps_its_holes() {
    let ring = ok(
        BooleanOp::Difference,
        &[
            single(circle(0.0, 0.0, 20.0)),
            single(circle(0.0, 0.0, 10.0)),
        ],
    );
    assert_eq!(ring.outlines().len(), 2);
    let island = single(circle(0.0, 0.0, 5.0));
    let result = ok(BooleanOp::Union, &[operand_of(&ring), island]);
    assert_eq!(result.outlines().len(), 3);
    let exact = std::f64::consts::PI * (400.0 - 100.0 + 25.0);
    assert_close(area(&result), exact, 0.01 * perimeter(&result), "area");
    assert!(
        !covers(&result, Point::new(8.0, 0.0)),
        "the hole stays a hole"
    );
    assert!(covers(&result, Point::new(2.0, 0.0)));
    assert!(covers(&result, Point::new(15.0, 0.0)));
}

/// Criterion 17: the four ways a result is empty.
#[test]
fn an_empty_result_is_refused() {
    let disc = |cx| single(circle(cx, 0.0, 10.0));
    let sq = |x0, x1| single(rect(x0, 0.0, x1, 10.0));
    assert_eq!(
        run(BooleanOp::Intersection, &[disc(0.0), disc(50.0)]),
        Err(BooleanError::EmptyResult)
    );
    assert_eq!(
        run(BooleanOp::Intersection, &[sq(0.0, 10.0), sq(10.0, 20.0)]),
        Err(BooleanError::EmptyResult),
        "squares sharing only an edge"
    );
    assert_eq!(
        run(
            BooleanOp::Intersection,
            &[
                single(rect(0.0, 0.0, 10.0, 10.0)),
                single(rect(10.0, 10.0, 20.0, 20.0))
            ]
        ),
        Err(BooleanError::EmptyResult),
        "squares sharing only a corner"
    );
    assert_eq!(
        run(BooleanOp::Difference, &[sq(2.0, 8.0), sq(0.0, 10.0)]),
        Err(BooleanError::EmptyResult),
        "covered completely"
    );
    assert_eq!(
        run(BooleanOp::Exclusion, &[sq(0.0, 10.0), sq(0.0, 10.0)]),
        Err(BooleanError::EmptyResult)
    );
}

/// Criteria 15 and 16: open paths and operands without area are refused with every offender.
#[test]
fn open_and_arealess_operands_are_refused_with_all_offenders() {
    use curvyo_geometry_core::{Outline, boolean};
    let square = rect(0.0, 0.0, 10.0, 10.0);
    let line = polygon(&[(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)]);
    let point = polygon(&[(3.0, 3.0), (3.0, 3.0)]);
    let tol = common::KERNEL_TOLERANCE;

    let closed_square = [Outline::new(&square, true)];
    let open_square = [Outline::new(&square, false)];
    let operands: [&[Outline<'_>]; 3] = [&open_square, &closed_square, &open_square];
    assert_eq!(
        boolean(BooleanOp::Union, &operands, tol),
        Err(BooleanError::OpenOperands(vec![0, 2]))
    );

    let flat = [Outline::new(&line, true)];
    let dot = [Outline::new(&point, true)];
    let operands: [&[Outline<'_>]; 3] = [&closed_square, &flat, &dot];
    for op in ALL_OPS {
        assert_eq!(
            boolean(op, &operands, tol),
            Err(BooleanError::EmptyOperands(vec![1, 2])),
            "{op:?}"
        );
    }
    let none: [&[Outline<'_>]; 1] = [&[]];
    assert_eq!(
        boolean(BooleanOp::Union, &none, tol),
        Err(BooleanError::EmptyOperands(vec![0]))
    );
    assert_eq!(
        boolean(BooleanOp::Union, &[], tol),
        Err(BooleanError::NoOperands)
    );
}

#[test]
fn non_finite_and_huge_coordinates_are_refused() {
    for bad in [f64::NAN, f64::INFINITY, -f64::INFINITY, 2.0e7] {
        let operands = [
            single(rect(0.0, 0.0, 10.0, 10.0)),
            single(rect(0.0, 0.0, bad, 10.0)),
        ];
        assert_eq!(
            run(BooleanOp::Union, &operands),
            Err(BooleanError::OutOfRange(vec![1])),
            "{bad}"
        );
    }
}

#[test]
fn a_tolerance_below_two_grid_pitches_is_refused() {
    let operands = [single(rect(0.0, 0.0, 10.0, 10.0))];
    for tolerance in [0.0, 0.002, f64::NAN, -1.0] {
        assert_eq!(
            run_at(BooleanOp::Union, &operands, Tolerance::from_mm(tolerance)),
            Err(BooleanError::ToleranceTooSmall),
            "{tolerance}"
        );
    }
    assert!(run_at(BooleanOp::Union, &operands, Tolerance::from_mm(0.003)).is_ok());
}

/// Criterion 39: coordinates that round to the same grid point coincide; edges 0.002 mm apart
/// stay separate.
#[test]
fn the_grid_decides_what_coincides() {
    let left = single(rect(0.0, 0.0, 20.0, 20.0));
    let touching = run(
        BooleanOp::Union,
        &[left.clone(), single(rect(20.0004, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert_eq!((touching.outlines().len(), node_count(&touching)), (1, 4));

    let apart = run(
        BooleanOp::Union,
        &[left, single(rect(20.002, 0.0, 40.0, 20.0))],
    )
    .unwrap();
    assert_eq!(apart.outlines().len(), 2);
}

/// Criteria 25 and 26: two discs of radius 10 mm, centres 5 mm apart. Every node of the result
/// is on the true outline and every chord stays within 0.01 mm of it. The Bézier arcs of the
/// operands differ from the true circle by up to 0.003 mm, which is allowed for here.
#[test]
fn the_union_of_two_discs_follows_the_true_outline() {
    let allowance = 0.01 + 0.003;
    let result = ok(
        BooleanOp::Union,
        &[
            single(circle(0.0, 0.0, 10.0)),
            single(circle(5.0, 0.0, 10.0)),
        ],
    );
    assert_eq!(result.outlines().len(), 1);
    let outline = &result.outlines()[0];
    // Distance to the boundary of the union of the two true discs.
    let boundary_distance = |p: Point| {
        let d1 = (p.x.hypot(p.y) - 10.0).abs();
        let d2 = ((p.x - 5.0).hypot(p.y) - 10.0).abs();
        let inside1 = p.x.hypot(p.y) < 10.0 + allowance;
        let inside2 = (p.x - 5.0).hypot(p.y) < 10.0 + allowance;
        // A point of circle 1 only counts if it is not buried inside disc 2, and vice versa.
        let buried1 = (p.x - 5.0).hypot(p.y) < 10.0 - allowance;
        let buried2 = p.x.hypot(p.y) < 10.0 - allowance;
        let mut best = f64::INFINITY;
        if inside1 && !buried1 {
            best = best.min(d1);
        }
        if inside2 && !buried2 {
            best = best.min(d2);
        }
        best
    };
    for (index, node) in outline.iter().enumerate() {
        assert!(
            boundary_distance(*node) <= allowance,
            "node {index} {node:?}"
        );
        let next = outline[(index + 1) % outline.len()];
        let mid = Point::new(f64::midpoint(node.x, next.x), f64::midpoint(node.y, next.y));
        // A chord lies inside the true outline by at most its sagitta, and never outside.
        assert!(boundary_distance(mid) <= allowance, "chord {index} {mid:?}");
    }
}

/// Criterion 26: a disc of radius 10 mm among the operands of a union keeps between 71 and 142
/// nodes.
#[test]
fn a_disc_keeps_a_node_count_inside_the_budget() {
    let result = ok(
        BooleanOp::Union,
        &[
            single(circle(0.0, 0.0, 10.0)),
            single(rect(100.0, 0.0, 120.0, 20.0)),
        ],
    );
    let disc = result
        .outlines()
        .iter()
        .find(|outline| outline.len() > 4)
        .unwrap();
    assert!((71..=142).contains(&disc.len()), "{} nodes", disc.len());
}

/// Criterion 42: operands 100 000 mm from the origin give the same shape as at the origin.
#[test]
fn a_far_away_union_matches_the_one_at_the_origin() {
    let build = |dx: f64| {
        ok(
            BooleanOp::Union,
            &[
                single(circle(dx, dx, 10.0)),
                single(circle(dx + 5.0, dx, 10.0)),
                single(rect(dx - 30.0, dx - 3.0, dx - 5.0, dx + 3.0)),
            ],
        )
    };
    let near = build(0.0);
    let far = build(100_000.0);
    assert_eq!(near.outlines().len(), far.outlines().len());
    assert_eq!(node_count(&near), node_count(&far));
    assert_close(area(&near), area(&far), 0.002, "area");
}

/// Criterion 43: the same operands twice give the same result, node for node.
#[test]
fn the_same_operands_give_the_same_result() {
    let operands = [
        single(circle(0.0, 0.0, 10.0)),
        single(circle(7.0, 3.0, 8.0)),
        single(rect(-4.0, -4.0, 6.0, 9.0)),
    ];
    for op in ALL_OPS {
        assert_eq!(run(op, &operands), run(op, &operands), "{op:?}");
    }
}

/// A union does not depend on the order of its operands, outline for outline.
#[test]
fn union_does_not_depend_on_operand_order() {
    let a = single(circle(0.0, 0.0, 10.0));
    let b = single(circle(7.0, 3.0, 8.0));
    let c = single(rect(-4.0, -4.0, 6.0, 9.0));
    let forward = run(BooleanOp::Union, &[a.clone(), b.clone(), c.clone()]);
    let backward = run(BooleanOp::Union, &[c, b, a]);
    assert_eq!(forward, backward);
}

fn distance_to_polyline(p: Point, polyline: &[Point]) -> f64 {
    (0..polyline.len())
        .map(|i| {
            let (a, b) = (polyline[i], polyline[(i + 1) % polyline.len()]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length_squared = dx * dx + dy * dy;
            let t = if length_squared == 0.0 {
                0.0
            } else {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / length_squared).clamp(0.0, 1.0)
            };
            (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Criterion 25 on random curved operands, which also bounds the drift of the cleanup: the
/// result of a union with a far-away rectangle stays within 0.01 mm of an independent, dense
/// flattening of the cubics, in both directions.
#[test]
fn random_curved_operands_stay_within_the_kernel_tolerance() {
    use curvyo_document_core::Vec2;
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    for case in 0..30 {
        let count = 4 + (next() * 6.0) as usize;
        let radius = 5.0 + next() * 40.0;
        let anchors: Vec<_> = (0..count)
            .map(|i| {
                let angle = std::f64::consts::TAU
                    * (f64::from(i32::try_from(i).unwrap()) + 0.3 * next())
                    / f64::from(i32::try_from(count).unwrap());
                let r = radius * (0.6 + 0.4 * next());
                let handle = |scale: f64| Vec2::new(-angle.sin() * scale, angle.cos() * scale);
                let reach = r * (0.1 + 0.4 * next());
                (
                    Point::new(r * angle.cos(), r * angle.sin()),
                    handle(-reach),
                    handle(reach),
                )
            })
            .collect();
        let mut dense = Vec::new();
        for i in 0..anchors.len() {
            let (from, to) = (anchors[i], anchors[(i + 1) % anchors.len()]);
            let control = [
                from.0,
                from.0.translated(from.2),
                to.0.translated(to.1),
                to.0,
            ];
            for step in 0..400 {
                let t = f64::from(step) / 400.0;
                let m = 1.0 - t;
                let w = [m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t];
                dense.push(Point::new(
                    w.iter().zip(&control).map(|(w, p)| w * p.x).sum(),
                    w.iter().zip(&control).map(|(w, p)| w * p.y).sum(),
                ));
            }
        }
        let result = ok(
            BooleanOp::Union,
            &[single(anchors), single(rect(500.0, 0.0, 510.0, 10.0))],
        );
        let curve = result
            .outlines()
            .iter()
            .find(|outline| outline.len() != 4)
            .unwrap();
        let outward = curve
            .iter()
            .map(|p| distance_to_polyline(*p, &dense))
            .fold(0.0, f64::max);
        let inward = dense
            .iter()
            .map(|p| distance_to_polyline(*p, curve))
            .fold(0.0, f64::max);
        assert!(
            outward <= 0.01 && inward <= 0.01,
            "case {case}: {outward} / {inward}"
        );
    }
}

/// Criterion 24b: a polyline circle of 100,000 vertices united with a far-away square keeps its
/// shape. The cleanup judges runs of near-collinear vertices against their end points, so nothing
/// drifts: the result is within a grid unit of the input polyline plus the snapping (0.0007 mm),
/// in both directions, and the area is that of the input within 0.001 mm times the outline length.
#[test]
fn a_dense_polyline_circle_keeps_its_shape() {
    let count = 100_000_u32;
    let radius = 10.0;
    let input: Vec<Point> = (0..count)
        .map(|i| {
            let angle = std::f64::consts::TAU * f64::from(i) / f64::from(count);
            Point::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    let pairs: Vec<(f64, f64)> = input.iter().map(|p| (p.x, p.y)).collect();
    let result = ok(
        BooleanOp::Union,
        &[
            single(polygon(&pairs)),
            single(rect(100.0, 0.0, 110.0, 10.0)),
        ],
    );
    let circle = result
        .outlines()
        .iter()
        .find(|outline| outline.len() > 4)
        .unwrap();
    let limit = 0.0017 + 1e-9;
    // Every 50th input vertex keeps the test fast; the cleanup itself saw all of them.
    let outward = input
        .iter()
        .step_by(50)
        .map(|p| distance_to_polyline(*p, circle))
        .fold(0.0, f64::max);
    let inward = circle
        .iter()
        .map(|p| distance_to_polyline(*p, &input))
        .fold(0.0, f64::max);
    assert!(outward <= limit && inward <= limit, "{outward} / {inward}");
    let input_area = curvyo_geometry_core::signed_area_mm2(&input);
    let circle_area = curvyo_geometry_core::signed_area_mm2(circle);
    let length = std::f64::consts::TAU * radius;
    assert!(
        (circle_area - input_area).abs() <= 0.001 * length,
        "{circle_area} vs {input_area}"
    );
}
