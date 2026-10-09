//! Flattens a closed outline of cubic Bézier segments to a polyline whose chords stay within a
//! given distance of the curve, using only `+ - * /` and `sqrt` so the points are bit-identical on
//! every target.
//!
//! `kurbo`'s own flattening is not used here: it calls `powf`, which different platform math
//! libraries may round differently, and `specs/0016-boolean-operations` criterion 43 asks for the
//! same result on Linux, Windows, macOS and in the browser. The price is a few more points than
//! an adaptive scheme would give (about 6 % more on a circle).

use curvyo_document_core::Point;

use crate::OutlineTriple;

/// The absolute control points of the segment from `from` to `to`.
fn control_points(from: OutlineTriple, to: OutlineTriple) -> [Point; 4] {
    [
        from.0,
        from.0.translated(from.2),
        to.0.translated(to.1),
        to.0,
    ]
}

/// Squared distance from `point` to the segment from `a` to `b`.
fn distance_squared_to_segment(point: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_squared = dx * dx + dy * dy;
    let along = (point.x - a.x) * dx + (point.y - a.y) * dy;
    let t = if length_squared > 0.0 {
        (along / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (nx, ny) = (a.x + t * dx - point.x, a.y + t * dy - point.y);
    nx * nx + ny * ny
}

/// Number of equal parameter steps that keep every chord within `tolerance_mm` of the curve.
///
/// A chord over a parameter step `h` deviates from a curve by at most `h² · max|B''| / 8`. For a
/// cubic `B''` is linear in `t`, so its maximum is at one end: `6 · max(|d0|, |d1|)` with
/// `d0 = P2 - 2·P1 + P0` and `d1 = P3 - 2·P2 + P1`. Solving for the step count gives
/// `ceil(sqrt(6 · max / (8 · tolerance)))`.
fn step_count(control: &[Point; 4], tolerance_mm: f64) -> usize {
    let [p0, p1, p2, p3] = *control;
    // A cubic lies inside the convex hull of its control points. If both inner control points
    // are within the tolerance of the chord, so is the whole curve, and one step is enough. This
    // is what keeps a straight edge (zero handles) from being cut into pieces: its parameter
    // speed is not constant, so the second-derivative bound below would not be zero for it.
    let tolerance_squared = tolerance_mm * tolerance_mm;
    if distance_squared_to_segment(p1, p0, p3) <= tolerance_squared
        && distance_squared_to_segment(p2, p0, p3) <= tolerance_squared
    {
        return 1;
    }
    let norm = |x: f64, y: f64| (x * x + y * y).sqrt();
    let d0 = norm(p2.x - 2.0 * p1.x + p0.x, p2.y - 2.0 * p1.y + p0.y);
    let d1 = norm(p3.x - 2.0 * p2.x + p1.x, p3.y - 2.0 * p2.y + p1.y);
    let second_derivative_max = 6.0 * d0.max(d1);
    let steps = (second_derivative_max / (8.0 * tolerance_mm)).sqrt().ceil();
    // A finite, non-negative float: the cast saturates and cannot wrap. `max(1.0)` keeps a
    // straight segment (zero handles) at one step.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        steps.max(1.0) as usize
    }
}

fn eval(control: &[Point; 4], t: f64) -> Point {
    let [p0, p1, p2, p3] = *control;
    let mt = 1.0 - t;
    // Bernstein weights of the four control points.
    let weights = [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t];
    Point::new(
        weights[0] * p0.x + weights[1] * p1.x + weights[2] * p2.x + weights[3] * p3.x,
        weights[0] * p0.y + weights[1] * p1.y + weights[2] * p2.y + weights[3] * p3.y,
    )
}

/// The vertices of the closed outline through `anchors`, in order, without repeating the first
/// vertex at the end. The closing segment from the last anchor back to the first is the real
/// cubic through their handles, as the canvas paints it.
///
/// Every anchor is a vertex; between two anchors there are as many extra vertices as `tolerance_mm`
/// needs. `tolerance_mm` must be finite and greater than zero, and all control points finite; the
/// caller checks both (`boolean` does). Fewer than two anchors give just those anchors.
pub(crate) fn flatten_closed(anchors: &[OutlineTriple], tolerance_mm: f64) -> Vec<Point> {
    let mut points = Vec::with_capacity(anchors.len());
    for (index, anchor) in anchors.iter().enumerate() {
        points.push(anchor.0);
        if anchors.len() < 2 {
            continue;
        }
        let next = anchors[(index + 1) % anchors.len()];
        let control = control_points(*anchor, next);
        let steps = step_count(&control, tolerance_mm);
        for step in 1..steps {
            // Exact: `step` and `steps` are far below 2^53.
            #[allow(clippy::cast_precision_loss)]
            let t = step as f64 / steps as f64;
            points.push(eval(&control, t));
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::Vec2;

    /// Circle of `radius` about the origin as four Bézier arcs, the usual 0.5522847498 handles.
    fn circle(radius: f64) -> Vec<OutlineTriple> {
        let k = 0.552_284_749_830_793_4 * radius;
        vec![
            (
                Point::new(radius, 0.0),
                Vec2::new(0.0, -k),
                Vec2::new(0.0, k),
            ),
            (
                Point::new(0.0, radius),
                Vec2::new(k, 0.0),
                Vec2::new(-k, 0.0),
            ),
            (
                Point::new(-radius, 0.0),
                Vec2::new(0.0, k),
                Vec2::new(0.0, -k),
            ),
            (
                Point::new(0.0, -radius),
                Vec2::new(-k, 0.0),
                Vec2::new(k, 0.0),
            ),
        ]
    }

    #[test]
    fn straight_segments_add_no_vertices() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
            .map(|(x, y)| (Point::new(x, y), Vec2::ZERO, Vec2::ZERO));
        let points = flatten_closed(&square, 0.008);
        assert_eq!(points.len(), 4);
    }

    /// Criterion 26: a disc of radius 10 mm needs 71 nodes at the least for a 0.01 mm chord
    /// deviation, and the budget is twice that.
    #[test]
    fn a_disc_of_radius_ten_stays_inside_the_node_budget() {
        let points = flatten_closed(&circle(10.0), 0.008);
        assert!(
            (71..=142).contains(&points.len()),
            "got {} nodes",
            points.len()
        );
    }

    /// Criterion 25: every chord stays within the tolerance of the Bézier arcs. The circle is
    /// the reference, which the Bézier arcs approximate to better than 3e-4 of the radius.
    #[test]
    fn chords_of_a_disc_stay_within_the_tolerance() {
        let radius = 10.0;
        let tolerance = 0.008;
        let points = flatten_closed(&circle(radius), tolerance);
        for (index, a) in points.iter().enumerate() {
            let b = points[(index + 1) % points.len()];
            let mid = Point::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y));
            let deviation = radius - mid.x.hypot(mid.y);
            assert!(
                deviation <= tolerance + 0.003,
                "chord {index} deviates by {deviation}"
            );
        }
    }

    #[test]
    fn a_single_anchor_gives_itself() {
        let one = [(Point::new(1.0, 2.0), Vec2::new(5.0, 5.0), Vec2::ZERO)];
        assert_eq!(flatten_closed(&one, 0.008), vec![Point::new(1.0, 2.0)]);
        assert_eq!(flatten_closed(&[], 0.008), Vec::<Point>::new());
    }

    #[test]
    fn flattening_is_deterministic() {
        let a = flatten_closed(&circle(37.3), 0.008);
        let b = flatten_closed(&circle(37.3), 0.008);
        assert_eq!(a, b);
    }
}
