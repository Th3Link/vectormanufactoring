//! Cleanup of a raw overlay result so that it meets the output rules of criteria 24 and 41.
//!
//! # Drift
//!
//! Vertices are removed Douglas-Peucker style: a vertex is dropped only if it lies within one
//! grid unit of the chord between the two vertices that are kept around its run, never of a
//! neighbour that is itself dropped later. A run of near-collinear vertices therefore cannot
//! drift: every dropped vertex stays within one grid unit (0.001 mm) of the outline that is kept.
//! A second step removes kept vertices that ended up within a unit of their neighbours' line, as
//! criterion 24 asks, but only while every input vertex of the run stays within a unit of the new
//! line. So no input vertex is ever more than one unit from the result of a round. A round starts
//! from the outline the previous round left, so the bound for `MAX_CLEANUP_ROUNDS` rounds is that
//! many units in theory; rounds after the first only remove vertices that the repairing union
//! has just created, which are few, and the measured deviation of a polyline circle of 100,000
//! vertices is under one unit.

use crate::boolean_grid::{Path64, Paths64, Point64, double_area, normalize};

/// Distance, in grid units, within which a vertex counts as lying on the chord that replaces it
/// (criterion 24).
const SIMPLIFY_DISTANCE_UNITS: f64 = 1.0;

/// The same distance, squared, for comparisons that avoid the root.
const UNIT_SQUARED: f64 = SIMPLIFY_DISTANCE_UNITS * SIMPLIFY_DISTANCE_UNITS;

/// Outlines whose average width is below one grid pitch are dropped (see `is_sliver`).
const SLIVER_WIDTH_UNITS: f64 = 1.0;

/// Most simplify-and-repair rounds `cleanup` runs. A round removes vertices and a union repairs
/// what that crossed, which can leave a new rounded crossing near a line; two rounds settle every
/// case seen so far, and the bound keeps the cost and the drift fixed for any input.
const MAX_CLEANUP_ROUNDS: usize = 4;

/// Squared distance, in grid units squared, from `p` to the segment `a`-`b`. The products are
/// exact in 128-bit integers; the one division is an IEEE operation, the same on every target.
fn distance_squared_to_segment(p: Point64, a: Point64, b: Point64) -> f64 {
    let (abx, aby) = (
        i128::from(b.x) - i128::from(a.x),
        i128::from(b.y) - i128::from(a.y),
    );
    let (apx, apy) = (
        i128::from(p.x) - i128::from(a.x),
        i128::from(p.y) - i128::from(a.y),
    );
    let length_squared = abx * abx + aby * aby;
    let along = apx * abx + apy * aby;
    // Precision loss above 2^53 only moves a threshold by far less than a grid unit.
    #[allow(clippy::cast_precision_loss)]
    if length_squared == 0 || along <= 0 {
        (apx * apx + apy * apy) as f64
    } else if along >= length_squared {
        let (bpx, bpy) = (
            i128::from(p.x) - i128::from(b.x),
            i128::from(p.y) - i128::from(b.y),
        );
        (bpx * bpx + bpy * bpy) as f64
    } else {
        let cross = (abx * apy - aby * apx) as f64;
        cross * cross / length_squared as f64
    }
}

/// Whether every vertex of the closed polygon from index `from` (exclusive) to `to` (exclusive),
/// walking forward and wrapping, is within one grid unit of the line through `from` and `to`.
fn run_is_within_unit(path: &Path64, from: usize, to: usize) -> bool {
    let n = path.len();
    let mut index = (from + 1) % n;
    while index != to {
        if distance_squared_to_line(path[index], path[from], path[to]) > UNIT_SQUARED {
            return false;
        }
        index = (index + 1) % n;
    }
    true
}

/// Squared distance from `p` to the infinite line through `a` and `b` (to `a` if they coincide).
fn distance_squared_to_line(p: Point64, a: Point64, b: Point64) -> f64 {
    let (abx, aby) = (
        i128::from(b.x) - i128::from(a.x),
        i128::from(b.y) - i128::from(a.y),
    );
    let (apx, apy) = (
        i128::from(p.x) - i128::from(a.x),
        i128::from(p.y) - i128::from(a.y),
    );
    let length_squared = abx * abx + aby * aby;
    // Precision loss above 2^53 only moves a threshold by far less than a grid unit.
    #[allow(clippy::cast_precision_loss)]
    if length_squared == 0 {
        (apx * apx + apy * apy) as f64
    } else {
        let cross = (abx * apy - aby * apx) as f64;
        cross * cross / length_squared as f64
    }
}

/// Drops the vertices of a closed polygon that are within one grid unit of the chord that
/// replaces them, so that no input vertex ends up more than a unit from the result.
///
/// Step 1 is Douglas-Peucker. The two anchors, the smallest vertex (x, then y) and the one
/// farthest from it, start the kept set; between them the farthest vertex of a run is kept while
/// it is more than a unit from the run's chord, and the two halves are searched in turn.
/// Step 2 removes a kept vertex that turned out to lie within a unit of the line between its final
/// neighbours (criterion 24; this includes the tip of a needle thinner than the grid), but only if
/// all input vertices between those neighbours stay within a unit of that line, so the bound
/// survives. A vertex that fails the second test stays, and then lies within a unit of its
/// neighbours' line: criterion 24 gives way to accuracy there.
fn remove_near_collinear(path: &Path64) -> Path64 {
    let n = path.len();
    if n < 4 {
        return path.clone();
    }
    let first = (0..n).min_by_key(|&i| (path[i].x, path[i].y)).unwrap_or(0);
    let far = |i: &usize| {
        let (dx, dy) = (
            i128::from(path[*i].x) - i128::from(path[first].x),
            i128::from(path[*i].y) - i128::from(path[first].y),
        );
        dx * dx + dy * dy
    };
    let second = (0..n).max_by_key(far).unwrap_or(first);
    if second == first {
        return path.clone();
    }
    let second_unwrapped = if second > first { second } else { second + n };
    let mut keep = vec![false; n];
    keep[first] = true;
    keep[second] = true;
    let mut runs = vec![(first, second_unwrapped), (second_unwrapped, first + n)];
    while let Some((lo, hi)) = runs.pop() {
        let mut worst = (0.0_f64, lo);
        for k in lo + 1..hi {
            let distance = distance_squared_to_segment(path[k % n], path[lo % n], path[hi % n]);
            if distance > worst.0 {
                worst = (distance, k);
            }
        }
        if worst.0 > UNIT_SQUARED {
            keep[worst.1 % n] = true;
            runs.push((lo, worst.1));
            runs.push((worst.1, hi));
        }
    }
    drop_redundant_kept(path, &keep)
}

/// Step 2 of `remove_near_collinear`: the kept vertices of `path` after removing those that are
/// within a unit of their final neighbours' line without breaking the bound for the vertices
/// between them.
fn drop_redundant_kept(path: &Path64, keep: &[bool]) -> Path64 {
    let n = path.len();
    let kept: Vec<usize> = (0..n).filter(|&i| keep[i]).collect();
    let count = kept.len();
    // Doubly linked ring over the kept vertices, by their index in `path`.
    let mut next = vec![0; n];
    let mut previous = vec![0; n];
    for (position, &index) in kept.iter().enumerate() {
        next[index] = kept[(position + 1) % count];
        previous[index] = kept[(position + count - 1) % count];
    }
    let mut alive = keep.to_vec();
    let mut remaining = count;
    let mut pending = kept;
    while let Some(vertex) = pending.pop() {
        if !alive[vertex] || remaining <= 3 {
            continue;
        }
        let (before, after) = (previous[vertex], next[vertex]);
        let near =
            distance_squared_to_line(path[vertex], path[before], path[after]) <= UNIT_SQUARED;
        if near && run_is_within_unit(path, before, after) {
            alive[vertex] = false;
            remaining -= 1;
            next[before] = after;
            previous[after] = before;
            pending.push(before);
            pending.push(after);
        }
    }
    path.iter()
        .zip(&alive)
        .filter(|(_, kept)| **kept)
        .map(|(point, _)| *point)
        .collect()
}

/// Drops the near-collinear vertices of every polygon (see `remove_near_collinear`).
fn simplify(paths: &Paths64) -> Paths64 {
    paths.iter().map(remove_near_collinear).collect()
}

/// Cleans a raw result: removes vertices within one grid unit of the chord that replaces them,
/// repairs the crossings that may cause with a nonzero union, drops outlines of fewer than three
/// vertices, of no area, or thinner than one grid pitch on average, and repeats while something
/// is removed, at most `MAX_CLEANUP_ROUNDS` times.
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
    fn simplify_drops_run_vertices_and_the_vertex_at_the_wrap_around() {
        // A square with a vertex on every edge, the start vertex lying on the closing edge.
        let square = path(&[
            (0, 5_000),
            (0, 0),
            (5_000, 0),
            (10_000, 0),
            (10_000, 10_000),
            (5_000, 10_000),
            (0, 10_000),
        ]);
        assert_eq!(
            remove_near_collinear(&square),
            path(&[(0, 0), (10_000, 0), (10_000, 10_000), (0, 10_000)])
        );
    }

    #[test]
    fn simplify_keeps_a_corner() {
        let corner = path(&[(0, 0), (1_000, 0), (1_000, 1_000), (0, 1_000)]);
        assert_eq!(remove_near_collinear(&corner), corner);
    }

    /// The failure that made the cleanup drift-free: judged against neighbours that remain, a
    /// dense arc collapsed to a few nodes up to 0.4 mm off its input.
    #[test]
    fn a_dense_circle_stays_within_one_unit_of_its_input() {
        let radius = 10_000.0_f64;
        for count in [5_000_usize, 20_000, 100_000] {
            #[allow(clippy::cast_precision_loss)]
            let circle: Path64 = (0..count)
                .map(|i| {
                    let angle = std::f64::consts::TAU * i as f64 / count as f64;
                    #[allow(clippy::cast_possible_truncation)]
                    Point64::new(
                        (radius * angle.cos()).round() as i64,
                        (radius * angle.sin()).round() as i64,
                    )
                })
                .collect();
            let kept = remove_near_collinear(&circle);
            assert!(kept.len() < count, "{count}: nothing removed");
            let worst = circle
                .iter()
                .map(|p| {
                    (0..kept.len())
                        .map(|k| {
                            distance_squared_to_segment(*p, kept[k], kept[(k + 1) % kept.len()])
                        })
                        .fold(f64::INFINITY, f64::min)
                })
                .fold(0.0, f64::max);
            assert!(worst <= 1.0, "{count}: {} nodes, worst {worst}", kept.len());
        }
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
}
