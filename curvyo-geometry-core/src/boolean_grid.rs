//! The 0.001 mm integer grid the boolean kernel works on: snapping to it, the overlay calls on it,
//! the cleanup of a result and its canonical output form.
//!
//! Everything between the snap and the final conversion back to millimetres is integer
//! arithmetic, which is what makes the kernel's output identical on every target
//! (`specs/0016-boolean-operations` criteria 39 and 43).

use curvyo_document_core::Point;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;

/// A vertex on the grid, in grid units.
pub(crate) type Point64 = IntPoint<i64>;
/// A closed polygon on the grid.
pub(crate) type Path64 = Vec<Point64>;
/// Polygons on the grid.
pub(crate) type Paths64 = Vec<Path64>;

/// Grid units per millimetre. A power of ten that is exact as a float, so snapping is one
/// multiplication and the way back one correctly rounded division.
const UNITS_PER_MM: f64 = 1000.0;

/// The grid pitch in millimetres. Coordinates that round to the same grid point coincide; edges
/// two pitches or more apart stay separate (criterion 39).
pub(crate) const GRID_MM: f64 = 0.001;

/// The largest absolute coordinate the kernel accepts, in millimetres. 10⁷ mm is 10¹⁰ grid units,
/// far inside the 64-bit engine's range of ±2⁶² units.
pub(crate) const MAX_COORDINATE_MM: f64 = 1.0e7;

/// Distance, in grid units, within which a vertex counts as lying on the line between its
/// neighbours and is dropped (criterion 24).
const SIMPLIFY_DISTANCE_UNITS: f64 = 1.0;

/// Snaps a point inside `MAX_COORDINATE_MM` to the nearest grid point.
pub(crate) fn snap(point: Point) -> Point64 {
    // Finite and within 10^10 after the range check, so the casts are exact.
    #[allow(clippy::cast_possible_truncation)]
    Point64::new(
        (point.x * UNITS_PER_MM).round() as i64,
        (point.y * UNITS_PER_MM).round() as i64,
    )
}

/// Snaps a polygon and drops consecutive duplicate vertices, the wrap-around pair included.
pub(crate) fn snap_polygon(points: &[Point]) -> Path64 {
    let mut path: Path64 = Vec::with_capacity(points.len());
    for point in points {
        let snapped = snap(*point);
        if path.last() != Some(&snapped) {
            path.push(snapped);
        }
    }
    while path.len() > 1 && path.first() == path.last() {
        path.pop();
    }
    path
}

/// Twice the signed area of a grid polygon, by the shoelace formula, in square grid units.
/// Positive for the winding the kernel gives outer outlines. Exact: 128-bit integers.
pub(crate) fn double_area(path: &[Point64]) -> i128 {
    let mut sum: i128 = 0;
    let mut previous = match path.last() {
        Some(point) => *point,
        None => return 0,
    };
    for point in path {
        sum += i128::from(previous.x) * i128::from(point.y)
            - i128::from(point.x) * i128::from(previous.y);
        previous = *point;
    }
    sum
}

/// Runs one overlay operation with the nonzero fill rule on both inputs. Outer outlines come back
/// with positive area (counter-clockwise in a Y-up plane), holes with negative.
pub(crate) fn run(rule: OverlayRule, subjects: &Paths64, clips: &Paths64) -> Paths64 {
    let capacity = subjects.iter().chain(clips).map(Vec::len).sum();
    let mut overlay = Overlay::<i64>::new(capacity);
    for contour in subjects {
        overlay.add_contour(contour, ShapeType::Subject);
    }
    for contour in clips {
        overlay.add_contour(contour, ShapeType::Clip);
    }
    overlay
        .overlay(rule, FillRule::NonZero)
        .into_iter()
        .flatten()
        .collect()
}

/// The painted region of a set of outlines under the nonzero rule: the set overlaid with
/// nothing. Outer outlines come back with positive area, holes with negative.
pub(crate) fn normalize(paths: &Paths64) -> Paths64 {
    run(OverlayRule::Subject, paths, &Paths64::new())
}

/// Whether `b` may be dropped from the run `a`, `b`, `c`: it lies within `SIMPLIFY_DISTANCE_UNITS`
/// of the line through `a` and `c`. That covers a vertex on a straight run and the tip of a spike
/// thinner than the grid (a spike where `c` equals `a` has no width at all). The products are
/// exact in 128-bit integers; the final comparison uses IEEE `sqrt`, the same on every target.
fn is_removable(a: Point64, b: Point64, c: Point64) -> bool {
    let (abx, aby) = (
        i128::from(b.x) - i128::from(a.x),
        i128::from(b.y) - i128::from(a.y),
    );
    let (acx, acy) = (
        i128::from(c.x) - i128::from(a.x),
        i128::from(c.y) - i128::from(a.y),
    );
    let length_squared = acx * acx + acy * acy;
    if length_squared == 0 {
        return true;
    }
    let cross = abx * acy - aby * acx;
    // The distance from `b` to the line is |cross| / |ac|. Precision loss above 2^53 only moves
    // the threshold by far less than a grid unit.
    #[allow(clippy::cast_precision_loss)]
    let within = (cross as f64).abs() <= SIMPLIFY_DISTANCE_UNITS * (length_squared as f64).sqrt();
    within
}

/// Drops the vertices of a closed polygon that are within one grid unit of the line between
/// their neighbours, in one pass with a stack, then around the wrap. Each drop is judged against
/// the neighbours that remain, so a long gentle run collapses to its end points only if the run
/// stays within about a unit of them.
///
fn remove_near_collinear(path: &Path64) -> Path64 {
    let mut kept: Path64 = Vec::with_capacity(path.len());
    for &point in path {
        while kept.len() >= 2 && is_removable(kept[kept.len() - 2], kept[kept.len() - 1], point) {
            kept.pop();
        }
        if kept.last() != Some(&point) {
            kept.push(point);
        }
    }
    // The wrap-around: the end and the start are neighbours too.
    loop {
        let n = kept.len();
        if n >= 3 && is_removable(kept[n - 2], kept[n - 1], kept[0]) {
            kept.pop();
        } else if n >= 3 && is_removable(kept[n - 1], kept[0], kept[1]) {
            kept.remove(0);
        } else {
            return kept;
        }
    }
}

/// Outlines whose average width is below one grid pitch are dropped (see `is_sliver`).
const SLIVER_WIDTH_UNITS: f64 = 1.0;

/// Most simplify-and-repair rounds `cleanup` runs. A round removes vertices and a second union
/// repairs what that crossed, which can leave a new rounded crossing near a line; two rounds
/// settle every case seen so far, and the bound keeps the cost fixed for any input.
const MAX_CLEANUP_ROUNDS: usize = 4;

/// Drops the near-collinear vertices of every polygon (see `remove_near_collinear`).
fn simplify(paths: &Paths64) -> Paths64 {
    paths.iter().map(remove_near_collinear).collect()
}

/// Cleans a raw result: removes vertices within one grid unit of the line between their
/// neighbours, repairs the crossings that may cause with a nonzero union, drops outlines of fewer
/// than three vertices, of no area, or thinner than one grid pitch on average, and repeats while
/// something is removed, at most `MAX_CLEANUP_ROUNDS` times (criteria 24 and 41).
pub(crate) fn cleanup(paths: &Paths64) -> Paths64 {
    // Always at least one round: removing a vertex can make an outline cross another, and only
    // the repairing union makes the result simple again. Slivers go inside the loop because
    // dropping one can turn a touching vertex of its neighbour into a plain collinear one.
    let mut current = without_slivers(normalize(&simplify(paths)));
    for _ in 1..MAX_CLEANUP_ROUNDS {
        let simplified = simplify(&current);
        if simplified == current {
            break;
        }
        current = without_slivers(normalize(&simplified));
    }
    current
}

/// `paths` without outlines of fewer than three vertices or thinner than a grid pitch.
fn without_slivers(mut paths: Paths64) -> Paths64 {
    paths.retain(|path| path.len() >= 3 && !is_sliver(path));
    paths
}

/// Whether an outline is thinner than one grid pitch on average, which is rounding noise: its
/// width, estimated as twice the area over the perimeter (exact for a long strip, half the side
/// for a square), is below what the grid can place.
fn is_sliver(path: &[Point64]) -> bool {
    let mut perimeter = 0.0_f64;
    let mut previous = path[path.len() - 1];
    for point in path {
        // IEEE `sqrt` of a sum of squares, not `hypot`, which platform libraries may round
        // differently (criterion 43). Precision loss above 2^53 only moves a threshold.
        #[allow(clippy::cast_precision_loss)]
        let (dx, dy) = ((point.x - previous.x) as f64, (point.y - previous.y) as f64);
        perimeter += (dx * dx + dy * dy).sqrt();
        previous = *point;
    }
    #[allow(clippy::cast_precision_loss)]
    let double_area = double_area(path).unsigned_abs() as f64;
    double_area < SLIVER_WIDTH_UNITS * perimeter
}

/// An outline rotated to its canonical start, with the area that orders it.
struct Canonical {
    path: Path64,
    double_area: i128,
}

/// Rotates `path` so that it starts at its smallest vertex (x, then y). If the outline touches
/// itself there, so that the smallest vertex occurs twice, the rotation whose vertex sequence
/// compares smallest wins.
fn rotate_to_start(path: &Path64) -> Path64 {
    let key = |point: &Point64| (point.x, point.y);
    let smallest = path.iter().map(key).min();
    let mut best: Option<usize> = None;
    for (index, point) in path.iter().enumerate() {
        if Some(key(point)) != smallest {
            continue;
        }
        best = match best {
            None => Some(index),
            Some(current) => {
                let candidate = (0..path.len()).map(|k| key(&path[(index + k) % path.len()]));
                let incumbent = (0..path.len()).map(|k| key(&path[(current + k) % path.len()]));
                if candidate.lt(incumbent) {
                    Some(index)
                } else {
                    Some(current)
                }
            }
        };
    }
    let start = best.unwrap_or(0);
    let mut rotated = Path64::with_capacity(path.len());
    rotated.extend_from_slice(&path[start..]);
    rotated.extend_from_slice(&path[..start]);
    rotated
}

/// Puts a result into its canonical form and converts it back to millimetres.
///
/// Each outline starts at its smallest vertex; outlines are sorted by that vertex, then by area
/// (larger first), then by their vertices. The winding is whatever the nonzero overlay gives:
/// positive area for outer outlines, negative for holes. Criteria 41 and 43.
pub(crate) fn canonical_millimetres(paths: &Paths64) -> Vec<Vec<Point>> {
    let mut outlines: Vec<Canonical> = paths
        .iter()
        .map(|path| Canonical {
            double_area: double_area(path),
            path: rotate_to_start(path),
        })
        .collect();
    outlines.sort_by(|a, b| {
        let start = |c: &Canonical| (c.path[0].x, c.path[0].y);
        start(a)
            .cmp(&start(b))
            .then_with(|| b.double_area.cmp(&a.double_area))
            .then_with(|| {
                let points = |c: &Canonical| c.path.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
                points(a).cmp(&points(b))
            })
    });
    outlines
        .iter()
        .map(|outline| outline.path.iter().map(to_millimetres).collect())
        .collect()
}

#[allow(clippy::cast_precision_loss)] // |units| <= 10^10, exact in an f64
fn to_millimetres(point: &Point64) -> Point {
    Point::new(point.x as f64 / UNITS_PER_MM, point.y as f64 / UNITS_PER_MM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(coordinates: &[(i64, i64)]) -> Path64 {
        coordinates
            .iter()
            .map(|&(x, y)| Point64::new(x, y))
            .collect()
    }

    #[test]
    fn coordinates_round_to_the_nearest_grid_point() {
        assert_eq!(snap(Point::new(20.0004, -0.0006)), Point64::new(20_000, -1));
        assert_eq!(snap(Point::new(20.0016, 0.0)), Point64::new(20_002, 0));
    }

    #[test]
    fn snapping_drops_duplicates_including_the_wrap_around() {
        let polygon = [
            Point::new(0.0, 0.0),
            Point::new(0.0001, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 5.0),
            Point::new(0.0, 0.0),
        ];
        assert_eq!(
            snap_polygon(&polygon),
            path(&[(0, 0), (5_000, 0), (5_000, 5_000)])
        );
    }

    #[test]
    fn the_double_area_is_signed_and_exact() {
        let counter = path(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        assert_eq!(double_area(&counter), 200);
        let mut clockwise = counter.clone();
        clockwise.reverse();
        assert_eq!(double_area(&clockwise), -200);
        assert_eq!(double_area(&[]), 0);
    }

    #[test]
    fn rotation_starts_at_the_smallest_vertex() {
        let rotated = rotate_to_start(&path(&[(5, 5), (0, 9), (0, 3), (7, 1)]));
        assert_eq!(rotated, path(&[(0, 3), (7, 1), (5, 5), (0, 9)]));
    }

    #[test]
    fn rotation_breaks_a_tie_at_a_pinch_vertex_by_the_following_vertices() {
        // A bow-tie traced through the pinch (0, 0) twice.
        let tied = path(&[(0, 0), (4, 4), (4, -4), (0, 0), (-4, -4), (-4, 4)]);
        let a = rotate_to_start(&tied);
        let mut shifted = tied.clone();
        shifted.rotate_left(3);
        assert_eq!(rotate_to_start(&shifted), a);
    }

    #[test]
    fn normalizing_flips_a_clockwise_outline_to_positive_area() {
        let clockwise = vec![path(&[(0, 10), (10, 10), (10, 0), (0, 0)])];
        let normalized = normalize(&clockwise);
        assert_eq!(normalized.len(), 1);
        assert_eq!(double_area(&normalized[0]), 200);
    }

    #[test]
    fn cleanup_removes_a_vertex_on_the_line_between_its_neighbours() {
        let square = vec![path(&[
            (0, 0),
            (5_000, 0),
            (10_000, 0),
            (10_000, 10_000),
            (0, 10_000),
        ])];
        let cleaned = cleanup(&square);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0].len(), 4);
    }

    #[test]
    fn simplify_drops_run_vertices_spike_tips_and_the_wrap_around_vertex() {
        // A square with a vertex on every edge, a spike of no width, and the start vertex lying
        // on the closing edge.
        let square = path(&[
            (0, 5_000),
            (0, 0),
            (5_000, 0),
            (10_000, 0),
            (10_000, 10_000),
            (5_000, 10_000),
            (5_000, 20_000),
            (5_000, 10_000),
            (0, 10_000),
        ]);
        let simplified = remove_near_collinear(&square);
        assert_eq!(
            simplified,
            path(&[(0, 0), (10_000, 0), (10_000, 10_000), (0, 10_000)])
        );
    }

    #[test]
    fn simplify_keeps_a_corner() {
        let corner = path(&[(0, 0), (1_000, 0), (1_000, 1_000), (0, 1_000)]);
        assert_eq!(remove_near_collinear(&corner), corner);
    }

    #[test]
    fn slivers_thinner_than_a_grid_unit_are_recognised() {
        assert!(is_sliver(&path(&[
            (0, 0),
            (100_000, 0),
            (100_000, 1),
            (0, 1)
        ])));
        assert!(!is_sliver(&path(&[
            (0, 0),
            (100_000, 0),
            (100_000, 5),
            (0, 5)
        ])));
        assert!(!is_sliver(&path(&[(0, 0), (3, 0), (3, 3), (0, 3)])));
    }

    #[test]
    fn canonical_form_orders_outlines_by_their_start_vertex() {
        let far = path(&[(100, 100), (110, 100), (110, 110), (100, 110)]);
        let near = path(&[(10, 100), (20, 100), (20, 110), (10, 110)]);
        let result = canonical_millimetres(&vec![far, near]);
        assert_eq!(result[0][0], Point::new(0.01, 0.1));
        assert_eq!(result[1][0], Point::new(0.1, 0.1));
    }
}
