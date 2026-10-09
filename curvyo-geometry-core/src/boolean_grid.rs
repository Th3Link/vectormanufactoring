//! The 0.001 mm integer grid the boolean kernel works on: snapping to it, the overlay calls on it
//! and the canonical output form.
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
    fn canonical_form_orders_outlines_by_their_start_vertex() {
        let far = path(&[(100, 100), (110, 100), (110, 110), (100, 110)]);
        let near = path(&[(10, 100), (20, 100), (20, 110), (10, 110)]);
        let result = canonical_millimetres(&vec![far, near]);
        assert_eq!(result[0][0], Point::new(0.01, 0.1));
        assert_eq!(result[1][0], Point::new(0.1, 0.1));
    }
}
