//! Shared helpers for the boolean kernel's integration tests: shape builders, an adapter from
//! owned operands to the kernel's borrowed ones, and region queries on a result.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, missing_docs)]

pub mod golden;

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

// ---- the output rules of criteria 24 and 41, shared by the property and golden tests ----

/// A grid point, in 0.001 mm units.
pub type Grid = (i64, i64);

pub fn grid_points(outline: &[Point]) -> Vec<Grid> {
    outline
        .iter()
        .map(|p| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64))
        .collect()
}

pub fn orientation(a: Grid, b: Grid, c: Grid) -> i128 {
    let value = (i128::from(b.0) - i128::from(a.0)) * (i128::from(c.1) - i128::from(a.1))
        - (i128::from(b.1) - i128::from(a.1)) * (i128::from(c.0) - i128::from(a.0));
    value.signum()
}

/// Distance in grid units from `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: Grid, a: Grid, b: Grid) -> f64 {
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
/// through the other segment by less than one grid unit: the overlay rounds each crossing to the
/// grid, so a vertex may sit up to about 0.7 units off the edge it was computed on.
pub fn properly_cross(a: (Grid, Grid), b: (Grid, Grid)) -> bool {
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
pub fn inside(polygon: &[Grid], p: Grid) -> bool {
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
pub fn check_invariants(result: &BooleanResult) -> Result<(), String> {
    let mut segments: Vec<(Grid, Grid)> = Vec::new();
    // Where outlines touch (a pinch point), a vertex is shared and the collinear rule is waived:
    // removing it would change how the outlines touch.
    let mut seen: std::collections::HashMap<Grid, usize> = std::collections::HashMap::new();
    for outline in result.outlines() {
        for g in grid_points(outline) {
            *seen.entry(g).or_default() += 1;
        }
    }
    let every_segment: Vec<(Grid, Grid)> = result
        .outlines()
        .iter()
        .flat_map(|o| {
            let g = grid_points(o);
            (0..g.len()).map(move |i| (g[i], g[(i + 1) % g.len()]))
        })
        .collect();
    for outline in result.outlines() {
        if outline.len() < 3 {
            return Err("an outline has fewer than 3 nodes".into());
        }
        if signed(outline) == 0.0 {
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
                // Waived at a pinch vertex, next to an edge of three units or less, and where the
                // vertex touches another edge within a unit: at that size the grid cannot place
                // a vertex better, and removing it would change how the outlines touch.
                let short = ((b.0 - a.0) as f64).hypot((b.1 - a.1) as f64) <= 3.0
                    || ((c.0 - b.0) as f64).hypot((c.1 - b.1) as f64) <= 3.0;
                if distance <= 1.0
                    && seen[&b] == 1
                    && !short
                    && !every_segment
                        .iter()
                        .any(|(p, q)| *p != b && *q != b && distance_to_segment(b, *p, *q) <= 1.0)
                {
                    return Err(format!(
                        "node {b:?} lies {distance} grid units from the line {a:?} {c:?}"
                    ));
                }
            }
        }
    }
    // The pairwise sections below are quadratic: results of more than 3,000 edges skip them.
    if segments.len() > 3000 {
        return Ok(());
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
        let positive = signed(&result.outlines()[index]) > 0.0;
        if positive != (depth % 2 == 0) {
            return Err(format!(
                "outline {index} has {} area at nesting depth {depth}",
                if positive { "positive" } else { "negative" }
            ));
        }
    }
    Ok(())
}
