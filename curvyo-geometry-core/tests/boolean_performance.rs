//! The performance budgets of `specs/0016-boolean-operations` criteria 44 and 45: two operands
//! of 1,000 curved nodes each within 100 ms, and of 10,000 within 2 s, for each of union,
//! difference and intersection (kernel call only, release build).
//!
//! The budgets are asserted in release builds only; a debug build runs the same cases (about
//! 0.05 s and 0.5 s) and prints the timings. CI runs them in release, in the `boolean-budgets`
//! job; locally: `cargo test --release -p curvyo-geometry-core --test boolean_performance`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    missing_docs
)]

mod common;

use std::time::{Duration, Instant};

use common::{Anchors, Operand, run};
use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::BooleanOp;

/// A closed wavy outline of `nodes` smooth nodes (Catmull-Rom handles) around `(cx, cy)`, about
/// 1 mm between nodes, with a radial wave of 2 mm amplitude and ten nodes to the period. The
/// segments are strongly curved (a chord of 1 mm needs about four flat steps at 0.008 mm), the
/// worst realistic case for the flattening.
fn wavy(nodes: usize, cx: f64, cy: f64) -> Anchors {
    let radius = nodes as f64 / std::f64::consts::TAU;
    let position = |i: usize| {
        let angle = std::f64::consts::TAU * i as f64 / nodes as f64;
        let r = radius + 2.0 * (nodes as f64 / 10.0 * angle).sin();
        Point::new(cx + r * angle.cos(), cy + r * angle.sin())
    };
    (0..nodes)
        .map(|i| {
            let (prev, here, next) = (
                position((i + nodes - 1) % nodes),
                position(i),
                position((i + 1) % nodes),
            );
            let tangent = Vec2::new((next.x - prev.x) / 6.0, (next.y - prev.y) / 6.0);
            (here, Vec2::new(-tangent.x, -tangent.y), tangent)
        })
        .collect()
}

fn measure(nodes: usize, budget: Duration) {
    let radius = nodes as f64 / std::f64::consts::TAU;
    let operands: [Operand; 2] = [
        vec![wavy(nodes, 0.0, 0.0)],
        vec![wavy(nodes, radius * 0.7, radius * 0.3)],
    ];
    for (name, op) in [
        ("union", BooleanOp::Union),
        ("difference", BooleanOp::Difference),
        ("intersection", BooleanOp::Intersection),
    ] {
        let started = Instant::now();
        let result = run(op, &operands).unwrap();
        let elapsed = started.elapsed();
        println!(
            "2 x {nodes} nodes, {name}: {elapsed:?} ({} outlines, {} nodes out)",
            result.outlines().len(),
            common::node_count(&result)
        );
        if !cfg!(debug_assertions) {
            assert!(
                elapsed < budget,
                "{name} took {elapsed:?}, budget {budget:?}"
            );
        }
    }
}

/// Criterion 44.
#[test]
fn two_operands_of_one_thousand_nodes() {
    measure(1_000, Duration::from_millis(100));
}

/// Criterion 45.
#[test]
fn two_operands_of_ten_thousand_nodes() {
    measure(10_000, Duration::from_secs(2));
}
