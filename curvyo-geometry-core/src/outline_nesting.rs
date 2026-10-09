//! How closed outlines nest inside each other (`specs/0035-combine-and-break-apart`, "Depth"):
//! an outline of even depth is a shape, one of odd depth a hole in its parent. Enclosure is the
//! test of the canvas, the nonzero rule on the exact curves ([`contains_point_in_outlines`]), of
//! one test point of the inner outline against the other outline.
//!
//! The test point is the inner outline's first node, or the next one when that node lies within
//! [`TOUCH_DISTANCE_MM`] of the other outline (a result of the boolean kernel may touch at a vertex);
//! an outline all of whose nodes touch the other counts as not enclosed by it. Bounding boxes
//! reject nearly every pair of side-by-side outlines.

use curvyo_document_core::Point;

use crate::outline_touch::{Flat, TOUCH_DISTANCE_MM, point_segment_distance};
use crate::{Outline, contains_point_in_outlines, signed_area_mm2};

/// Where one outline sits among the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nesting {
    /// How many other outlines enclose it: 0 is outermost, even depths are shapes, odd are holes.
    pub depth: usize,
    /// The innermost outline enclosing it, if any.
    pub parent: Option<usize>,
}

/// The nesting of `outlines`, one entry per outline in order.
#[must_use]
pub fn outline_nesting(outlines: &[Outline<'_>]) -> Vec<Nesting> {
    let flats: Vec<Flat> = outlines.iter().map(Flat::of).collect();
    let enclosers: Vec<Vec<usize>> = (0..outlines.len())
        .map(|inner| {
            (0..outlines.len())
                .filter(|&outer| outer != inner && encloses(outlines, &flats, outer, inner))
                .collect()
        })
        .collect();
    let depths: Vec<usize> = enclosers.iter().map(Vec::len).collect();
    enclosers
        .iter()
        .map(|list| Nesting {
            depth: list.len(),
            parent: list.iter().copied().max_by_key(|&outer| depths[outer]),
        })
        .collect()
}

/// Whether `outer` encloses `inner`.
fn encloses(outlines: &[Outline<'_>], flats: &[Flat], outer: usize, inner: usize) -> bool {
    let (outer_flat, inner_flat) = (&flats[outer], &flats[inner]);
    // The inner box must lie within the outer one, up to the touch distance.
    if inner_flat.min.x < outer_flat.min.x - TOUCH_DISTANCE_MM
        || inner_flat.max.x > outer_flat.max.x + TOUCH_DISTANCE_MM
        || inner_flat.min.y < outer_flat.min.y - TOUCH_DISTANCE_MM
        || inner_flat.max.y > outer_flat.max.y + TOUCH_DISTANCE_MM
    {
        return false;
    }
    for anchor in outlines[inner].anchors {
        let point = anchor.0;
        if distance_to_polyline(point, outer_flat) <= TOUCH_DISTANCE_MM {
            continue;
        }
        return contains_point_in_outlines(&[outlines[outer]], point);
    }
    false
}

/// The distance from `point` to the polyline of `flat`.
fn distance_to_polyline(point: Point, flat: &Flat) -> f64 {
    let count = flat.points.len();
    (0..count)
        .map(|index| {
            point_segment_distance(point, flat.points[index], flat.points[(index + 1) % count])
        })
        .fold(f64::INFINITY, f64::min)
}

/// The signed area of `outline`, positive for the winding of a shape in the canonical form of the
/// boolean kernel and negative for a hole, computed on the flattened outline. An outline whose
/// absolute area is not above 1e-6 mm² (one grid cell) encloses no area.
#[must_use]
pub fn outline_area_mm2(outline: &Outline<'_>) -> f64 {
    signed_area_mm2(&Flat::of(outline).points)
}
