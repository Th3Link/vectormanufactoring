//! Shared helpers for the boolean kernel's integration tests: shape builders, an adapter from
//! owned operands to the kernel's borrowed ones, and region queries on a result.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, missing_docs)]

use curvyo_document_core::{Point, Tolerance, Vec2};
use curvyo_geometry_core::{
    BooleanError, BooleanOp, BooleanResult, Outline, OutlineTriple, boolean, signed_area_mm2,
};

/// The kernel tolerance of the story: 0.01 mm.
pub const KERNEL_TOLERANCE: Tolerance = Tolerance::from_mm(0.01);

/// One closed outline.
pub type Anchors = Vec<OutlineTriple>;

/// One operand: its outlines, painted with the nonzero rule.
pub type Operand = Vec<Anchors>;

pub fn polygon(points: &[(f64, f64)]) -> Anchors {
    points
        .iter()
        .map(|&(x, y)| (Point::new(x, y), Vec2::ZERO, Vec2::ZERO))
        .collect()
}

pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Anchors {
    polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

/// A circle as four Bézier arcs.
pub fn circle(cx: f64, cy: f64, r: f64) -> Anchors {
    let k = 0.552_284_749_830_793_4 * r;
    let anchor = |x: f64, y: f64, hin: (f64, f64), hout: (f64, f64)| {
        (
            Point::new(cx + x, cy + y),
            Vec2::new(hin.0, hin.1),
            Vec2::new(hout.0, hout.1),
        )
    };
    vec![
        anchor(r, 0.0, (0.0, -k), (0.0, k)),
        anchor(0.0, r, (k, 0.0), (-k, 0.0)),
        anchor(-r, 0.0, (0.0, k), (0.0, -k)),
        anchor(0.0, -r, (-k, 0.0), (k, 0.0)),
    ]
}

pub fn single(outline: Anchors) -> Operand {
    vec![outline]
}

/// A result fed back in as an operand: every outline with zero handles, windings kept, so a
/// hole stays a hole.
pub fn operand_of(result: &BooleanResult) -> Operand {
    result
        .outlines()
        .iter()
        .map(|outline| {
            outline
                .iter()
                .map(|&p| (p, Vec2::ZERO, Vec2::ZERO))
                .collect()
        })
        .collect()
}

pub fn run_at(
    op: BooleanOp,
    operands: &[Operand],
    tolerance: Tolerance,
) -> Result<BooleanResult, BooleanError> {
    let outlines: Vec<Vec<Outline<'_>>> = operands
        .iter()
        .map(|operand| {
            operand
                .iter()
                .map(|anchors| Outline::new(anchors, true))
                .collect()
        })
        .collect();
    let borrowed: Vec<&[Outline<'_>]> = outlines.iter().map(Vec::as_slice).collect();
    boolean(op, &borrowed, tolerance)
}

pub fn run(op: BooleanOp, operands: &[Operand]) -> Result<BooleanResult, BooleanError> {
    run_at(op, operands, KERNEL_TOLERANCE)
}

pub fn ok(op: BooleanOp, operands: &[Operand]) -> BooleanResult {
    run(op, operands).unwrap_or_else(|error| panic!("{op:?} was refused: {error}"))
}

/// Area of the region the result paints: holes carry a negative signed area.
pub fn area(result: &BooleanResult) -> f64 {
    result.outlines().iter().map(|o| signed_area_mm2(o)).sum()
}

/// Total length of all outlines.
pub fn perimeter(result: &BooleanResult) -> f64 {
    result
        .outlines()
        .iter()
        .map(|outline| {
            (0..outline.len())
                .map(|i| {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    (b.x - a.x).hypot(b.y - a.y)
                })
                .sum::<f64>()
        })
        .sum()
}

/// Winding number of the result's outlines around `p`.
pub fn winding(result: &BooleanResult, p: Point) -> i32 {
    let mut total = 0;
    for outline in result.outlines() {
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let side = (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
            if a.y <= p.y {
                if b.y > p.y && side > 0.0 {
                    total += 1;
                }
            } else if b.y <= p.y && side < 0.0 {
                total -= 1;
            }
        }
    }
    total
}

pub fn covers(result: &BooleanResult, p: Point) -> bool {
    winding(result, p) != 0
}

pub fn node_count(result: &BooleanResult) -> usize {
    result.outlines().iter().map(Vec::len).sum()
}

/// Signed area of one outline in square millimetres.
pub fn signed(outline: &[Point]) -> f64 {
    signed_area_mm2(outline)
}
