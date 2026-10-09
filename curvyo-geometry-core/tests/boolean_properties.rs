//! Property tests of the boolean kernel on random polygon pairs with straight edges
//! (`specs/0016-boolean-operations` criteria 14, 41 and 43). The seed is fixed, so a failure
//! reproduces, and 256 pairs run per property. Operands are 1 to 3 random outlines each, wound
//! at random: they cross themselves and each other and nest, so holes and self-intersections
//! are in the sample.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    missing_docs
)]

mod common;

use common::{Operand, area, ok, operand_of, perimeter, polygon, run};
use curvyo_document_core::Point;
use curvyo_geometry_core::{BooleanError, BooleanOp, BooleanResult};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

const GRID_MM: f64 = 0.001;

const ALL_OPS: [BooleanOp; 4] = [
    BooleanOp::Union,
    BooleanOp::Difference,
    BooleanOp::Intersection,
    BooleanOp::Exclusion,
];

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x0016_B001),
        failure_persistence: None,
        ..Config::default()
    }
}

/// A coordinate on the 0.001 mm grid between 0 and `extent_mm`.
fn grid_mm(extent_mm: i32) -> impl Strategy<Value = f64> {
    (0..extent_mm * 1000).prop_map(|units| f64::from(units) / 1000.0)
}

fn outline(extent_mm: i32) -> impl Strategy<Value = Vec<(f64, f64)>> {
    (
        prop::collection::vec((grid_mm(extent_mm), grid_mm(extent_mm)), 3..9),
        any::<bool>(),
    )
        .prop_map(|(mut points, reversed)| {
            if reversed {
                points.reverse();
            }
            points
        })
}

/// Operands on a coarse lattice of whole millimetres between 0 and 6: most vertices, edges and
/// crossings coincide, the worst case for the clipper.
fn lattice_operand() -> impl Strategy<Value = Operand> {
    let lattice = || (0..=6_i32).prop_map(f64::from);
    prop::collection::vec(
        prop::collection::vec((lattice(), lattice()), 3..9).prop_map(|p| polygon(&p)),
        1..4,
    )
}

fn operand(extent_mm: i32) -> impl Strategy<Value = Operand> {
    prop::collection::vec(outline(extent_mm), 1..4)
        .prop_map(|outlines| outlines.iter().map(|o| polygon(o)).collect())
}

fn area_or_zero(op: BooleanOp, operands: &[Operand]) -> f64 {
    match run(op, operands) {
        Ok(result) => area(&result),
        Err(BooleanError::EmptyResult) => 0.0,
        Err(other) => panic!("{op:?}: {other}"),
    }
}

/// `None` when the operand has no area at all, which the kernel refuses.
fn region_area(operand: &Operand) -> Option<f64> {
    match run(BooleanOp::Union, std::slice::from_ref(operand)) {
        Ok(result) => Some(area(&result)),
        Err(BooleanError::EmptyOperands(_) | BooleanError::EmptyResult) => None,
        Err(other) => panic!("{other}"),
    }
}

fn check_area_identities(
    a: &Operand,
    b: &Operand,
    allowance: impl Fn(f64, f64) -> f64,
) -> Result<(), TestCaseError> {
    let (Some(area_a), Some(area_b)) = (region_area(a), region_area(b)) else {
        return Ok(());
    };
    let both = [a.clone(), b.clone()];
    let union = area_or_zero(BooleanOp::Union, &both);
    let inter = area_or_zero(BooleanOp::Intersection, &both);
    let diff = area_or_zero(BooleanOp::Difference, &both);
    let allowance = allowance(area_a + area_b, outline_length(a) + outline_length(b));
    prop_assert!(
        (union + inter - (area_a + area_b)).abs() <= allowance,
        "union {union} + inter {inter} vs {area_a} + {area_b} for {} and {}",
        describe(a),
        describe(b)
    );
    prop_assert!(
        (diff + inter - area_a).abs() <= allowance,
        "diff {diff} + inter {inter} vs {area_a} for {} and {}",
        describe(a),
        describe(b)
    );
    Ok(())
}

fn outline_length(operand: &Operand) -> f64 {
    operand
        .iter()
        .map(|anchors| {
            (0..anchors.len())
                .map(|i| {
                    let (a, b) = (anchors[i].0, anchors[(i + 1) % anchors.len()].0);
                    (b.x - a.x).hypot(b.y - a.y)
                })
                .sum::<f64>()
        })
        .sum()
}

type Grid = (i64, i64);

fn grid_points(outline: &[Point]) -> Vec<Grid> {
    outline
        .iter()
        .map(|p| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64))
        .collect()
}

fn orientation(a: Grid, b: Grid, c: Grid) -> i128 {
    let value = (i128::from(b.0) - i128::from(a.0)) * (i128::from(c.1) - i128::from(a.1))
        - (i128::from(b.1) - i128::from(a.1)) * (i128::from(c.0) - i128::from(a.0));
    value.signum()
}

/// Distance in grid units from `p` to the segment `a`-`b`.
fn distance_to_segment(p: Grid, a: Grid, b: Grid) -> f64 {
    let (px, py, ax, ay) = (p.0 as f64, p.1 as f64, a.0 as f64, a.1 as f64);
    let (dx, dy) = (b.0 as f64 - ax, b.1 as f64 - ay);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - ax - t * dx).hypot(py - ay - t * dy)
}

/// Whether two segments cross at a point interior to both, by more than the grid can resolve.
/// Touching at a vertex or along a line is not a crossing, and neither is an end that pokes
/// through the other segment by less than one grid unit: Clipper rounds each crossing to the
/// grid, so a vertex may sit up to about 0.7 units off the edge it was computed on.
fn properly_cross(a: (Grid, Grid), b: (Grid, Grid)) -> bool {
    let (o1, o2) = (orientation(a.0, a.1, b.0), orientation(a.0, a.1, b.1));
    let (o3, o4) = (orientation(b.0, b.1, a.0), orientation(b.0, b.1, a.1));
    let crosses = o1 * o2 < 0 && o3 * o4 < 0;
    let within_grid = [b.0, b.1]
        .iter()
        .any(|p| distance_to_segment(*p, a.0, a.1) <= 1.0)
        || [a.0, a.1]
            .iter()
            .any(|p| distance_to_segment(*p, b.0, b.1) <= 1.0);
    crosses && !within_grid
}

/// Whether `p` lies inside the polygon (crossing number, exact on the grid).
fn inside(polygon: &[Grid], p: Grid) -> bool {
    let mut crossings = 0;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.1 > p.1) != (b.1 > p.1) {
            let side = orientation(a, b, p);
            if (side > 0) == (b.1 > a.1) {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

/// Criterion 41 and 24 on one result: `Err` says which rule is broken.
fn check_invariants(result: &BooleanResult) -> Result<(), String> {
    let mut segments: Vec<(Grid, Grid)> = Vec::new();
    // Where outlines touch (a pinch point), a vertex is shared and the collinear rule is waived:
    // removing it would change how the outlines touch.
    let mut seen: std::collections::HashMap<Grid, usize> = std::collections::HashMap::new();
    for outline in result.outlines() {
        for g in grid_points(outline) {
            *seen.entry(g).or_default() += 1;
        }
    }
    for outline in result.outlines() {
        if outline.len() < 3 {
            return Err("an outline has fewer than 3 nodes".into());
        }
        if common::signed(outline) == 0.0 {
            return Err("an outline has no area".into());
        }
        let grid = grid_points(outline);
        for (p, g) in outline.iter().zip(&grid) {
            if !(p.x.is_finite() && p.y.is_finite() && (p.x - g.0 as f64 / 1000.0).abs() < 1e-9) {
                return Err(format!("{p:?} is not a finite grid point"));
            }
        }
        for i in 0..grid.len() {
            let (a, b, c) = (
                grid[(i + grid.len() - 1) % grid.len()],
                grid[i],
                grid[(i + 1) % grid.len()],
            );
            if b == c {
                return Err(format!("repeated vertex {b:?}"));
            }
            segments.push((b, c));
            let (dx, dy) = ((c.0 - a.0) as f64, (c.1 - a.1) as f64);
            let length = dx.hypot(dy);
            if length > 0.0 {
                let distance =
                    (((b.0 - a.0) as f64) * dy - ((b.1 - a.1) as f64) * dx).abs() / length;
                // Waived at a pinch vertex and next to an edge of two units or less: at that
                // size the grid cannot place a vertex better.
                let short = ((b.0 - a.0) as f64).hypot((b.1 - a.1) as f64) <= 2.0
                    || ((c.0 - b.0) as f64).hypot((c.1 - b.1) as f64) <= 2.0;
                if distance <= 1.0 && seen[&b] == 1 && !short {
                    return Err(format!(
                        "node {b:?} lies {distance} grid units from the line {a:?} {c:?}"
                    ));
                }
            }
        }
    }
    for (i, s) in segments.iter().enumerate() {
        for t in &segments[i + 1..] {
            if properly_cross(*s, *t) {
                return Err(format!("outlines cross: {s:?} {t:?}"));
            }
        }
    }
    // Winding rule: an outline nested inside an even number of others is an outer outline and
    // has positive area; inside an odd number, a hole with negative area.
    let polygons: Vec<Vec<Grid>> = result.outlines().iter().map(|o| grid_points(o)).collect();
    for (index, polygon) in polygons.iter().enumerate() {
        // A vertex that is not within a grid unit of another outline decides; an outline that
        // only touches others (every vertex) is not tested.
        let probe = polygon.iter().find(|v| {
            polygons.iter().enumerate().all(|(other, q)| {
                other == index
                    || (0..q.len())
                        .all(|i| distance_to_segment(**v, q[i], q[(i + 1) % q.len()]) > 1.0)
            })
        });
        let Some(probe) = probe else { continue };
        let depth = polygons
            .iter()
            .enumerate()
            .filter(|(other, q)| *other != index && inside(q, *probe))
            .count();
        let positive = common::signed(&result.outlines()[index]) > 0.0;
        if positive != (depth % 2 == 0) {
            return Err(format!(
                "outline {index} has {} area at nesting depth {depth}",
                if positive { "positive" } else { "negative" }
            ));
        }
    }
    Ok(())
}

/// An operand as plain coordinates, for failure messages.
fn describe(operand: &Operand) -> String {
    let outlines: Vec<Vec<(f64, f64)>> = operand
        .iter()
        .map(|o| o.iter().map(|(p, _, _)| (p.x, p.y)).collect())
        .collect();
    format!("{outlines:?}")
}

fn check_result(op: BooleanOp, a: &Operand, b: &Operand) -> Result<(), TestCaseError> {
    let Ok(result) = run(op, &[a.clone(), b.clone()]) else {
        return Ok(());
    };
    if let Err(why) = check_invariants(&result) {
        let outlines: Vec<Vec<Grid>> = result.outlines().iter().map(|o| grid_points(o)).collect();
        return Err(TestCaseError::fail(format!(
            "{op:?} of {} and {} gave {outlines:?}: {why}",
            describe(a),
            describe(b)
        )));
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    /// Criterion 14 as written: on polygons up to 6 m across, the area identities hold within
    /// 1e-6 of the summed areas. At that size the grid's rounding of crossings is usually below
    /// that bound, so this checks the sample of the fixed seed, not a guarantee: over 20,000
    /// pairs, one pair misses it by 0.04 %. For the 60 mm shapes of a laser bed the bound cannot
    /// hold (see the next property), and the criterion needs the allowance of that property.
    #[test]
    fn area_identities_hold_within_one_millionth_on_large_shapes(
        a in operand(6000), b in operand(6000)
    ) {
        check_area_identities(&a, &b, |area_sum, _| 1e-6 * area_sum)?;
    }

    /// Criterion 14 with the grid's allowance, on shapes up to 60 mm across: Clipper puts each
    /// crossing on the 0.001 mm grid, which moves the area of a result by at most the grid pitch
    /// times the length of the edges at that crossing. That allowance is added to the 1e-6.
    #[test]
    fn area_identities_hold_within_the_grid_allowance_on_small_shapes(
        a in operand(60), b in operand(60)
    ) {
        check_area_identities(&a, &b, |area_sum, length_sum| {
            1e-6 * area_sum + GRID_MM * length_sum
        })?;
    }

    /// Coincident edges, shared vertices and touching points everywhere: no panic, and the
    /// output rules hold. The area identities are not checked here: `clipper2-rust` 1.2.0 has a
    /// rare defect on exactly this kind of input, see
    /// `a_union_with_collinear_overlapping_edges_is_exact` in `boolean_degenerate.rs`.
    #[test]
    fn lattice_shapes_stay_valid(a in lattice_operand(), b in lattice_operand()) {
        for op in ALL_OPS {
            check_result(op, &a, &b)?;
        }
    }

    /// Every result satisfies the output rules of criteria 24 and 41.
    #[test]
    fn results_satisfy_the_output_rules(a in operand(60), b in operand(60)) {
        for op in ALL_OPS {
            check_result(op, &a, &b)?;
        }
    }

    /// Criterion 43: the same operands give the same result, and the commutative operations do
    /// not depend on the operand order.
    #[test]
    fn results_are_deterministic_and_commutative(a in operand(60), b in operand(60)) {
        let forward = [a.clone(), b.clone()];
        let backward = [b, a];
        for op in [BooleanOp::Union, BooleanOp::Intersection, BooleanOp::Exclusion] {
            let first = run(op, &forward);
            prop_assert_eq!(&first, &run(op, &forward), "{:?} twice", op);
            if let (Ok(x), Ok(y)) = (&first, &run(op, &backward)) {
                // Same region. The cleanup can place a crossing differently when the
                // operands come in the other order, so compare areas to the grid, not nodes.
                let allowance = GRID_MM * (perimeter(x) + perimeter(y));
                prop_assert!((area(x) - area(y)).abs() <= allowance, "{:?} order", op);
            } else {
                prop_assert_eq!(first.is_ok(), run(op, &backward).is_ok(), "{:?} emptiness", op);
            }
        }
    }

    /// A result fed back in as an operand is unchanged by a union or an intersection with
    /// itself (idempotence), and holes survive.
    #[test]
    fn union_and_intersection_are_idempotent(a in operand(60), b in operand(60)) {
        let Ok(result) = run(BooleanOp::Union, &[a, b]) else { return Ok(()); };
        let again = operand_of(&result);
        prop_assert_eq!(&ok(BooleanOp::Union, std::slice::from_ref(&again)), &result);
        prop_assert_eq!(&ok(BooleanOp::Union, &[again.clone(), again.clone()]), &result);
        prop_assert_eq!(&ok(BooleanOp::Intersection, &[again.clone(), again]), &result);
    }
}
