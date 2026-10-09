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

use common::{
    Grid, Operand, area, check_invariants, grid_points, ok, operand_of, perimeter, polygon, run,
};
use curvyo_geometry_core::{BooleanError, BooleanOp};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

const GRID_MM: f64 = 0.001;

const ALL_OPS: [BooleanOp; 4] = [
    BooleanOp::Union,
    BooleanOp::Difference,
    BooleanOp::Intersection,
    BooleanOp::Exclusion,
];

/// 256 cases per property unless `PROPTEST_CASES` says otherwise; the seed is fixed either way.
fn config() -> Config {
    Config {
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
/// crossings coincide, the worst case for the overlay.
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
    let excl = area_or_zero(BooleanOp::Exclusion, &both);
    let allowance = allowance(area_a + area_b, outline_length(a) + outline_length(b));
    prop_assert!(
        (union + inter - (area_a + area_b)).abs() <= allowance,
        "union {union} + inter {inter} vs {area_a} + {area_b} for {} and {}",
        describe(a),
        describe(b)
    );
    prop_assert!(
        (excl - (area_a + area_b - 2.0 * inter)).abs() <= 2.0 * allowance,
        "excl {excl} vs {area_a} + {area_b} - 2 * {inter}"
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

    /// Criterion 14 on large shapes, up to 6 m across: the area identities hold within the bound
    /// of the criterion, `1e-6 * (|A| + |B|)` plus one grid pitch of boundary along the outlines.
    /// The allowance dominates below some metres, so this mainly guards against a gross error at
    /// coordinates far from the origin.
    #[test]
    fn area_identities_hold_within_the_bound_on_large_shapes(
        a in operand(6000), b in operand(6000)
    ) {
        check_area_identities(&a, &b, |area_sum, length_sum| {
            1e-6 * area_sum + GRID_MM * length_sum
        })?;
    }

    /// Criterion 14 on shapes up to 60 mm across, the size of a laser bed. The overlay puts each
    /// crossing on the 0.001 mm grid, which moves the area of a result by at most the grid pitch
    /// times the length of the edges at that crossing.
    #[test]
    fn area_identities_hold_within_the_bound_on_small_shapes(
        a in operand(60), b in operand(60)
    ) {
        check_area_identities(&a, &b, |area_sum, length_sum| {
            1e-6 * area_sum + GRID_MM * length_sum
        })?;
    }

    /// Coincident edges, shared vertices and touching points everywhere: no panic, the area
    /// identities hold to the grid and the output rules hold. This input broke the area
    /// identities of the previous kernel library in 0.2 % of the pairs.
    #[test]
    fn lattice_shapes_stay_valid(a in lattice_operand(), b in lattice_operand()) {
        check_area_identities(&a, &b, |area_sum, length_sum| {
            1e-6 * area_sum + GRID_MM * length_sum
        })?;
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
