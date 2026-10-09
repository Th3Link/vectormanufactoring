//! Boolean operations on closed outlines: union, difference, intersection and exclusion of any
//! number of operands (`specs/0016-boolean-operations`, ADR 0003 §3).
//!
//! # Pipeline
//!
//! 1. Open outlines, non-finite coordinates and coordinates beyond 10⁷ mm are refused.
//! 2. Every outline is flattened to a polyline that stays within the tolerance minus two grid
//!    pitches of its curve (the rest of the budget goes to snapping and cleanup).
//! 3. Vertices are snapped to the 0.001 mm integer grid.
//! 4. Each operand is normalized on its own by a nonzero self-union. That gives its painted
//!    region (a self-intersecting outline paints its crossing, a compound operand keeps its
//!    holes) and makes two operands wound in opposite directions behave the same as two wound
//!    alike. An operand without area is refused.
//! 5. The operation runs: union once over all operands; difference as the first operand minus
//!    the union of the others; intersection and exclusion folded pairwise, because the
//!    subject/clip form computes `A ∩ (B ∪ C)` and its XOR is not odd-parity for three operands.
//! 6. Vertices within one grid unit of the line between their neighbours are removed, a nonzero
//!    union repairs the crossings that rounding left, and outlines thinner than one grid pitch on
//!    average are dropped as rounding noise.
//! 7. The result is put into a canonical form (see [`BooleanResult`]).
//!
//! Steps 3 to 7 are integer arithmetic, and step 2 uses only `+ - * /` and `sqrt`, so a result
//! is identical on every target and on every run.

use curvyo_document_core::{Point, Tolerance};

use crate::OutlineTriple;
use crate::boolean_cleanup::cleanup;
use crate::boolean_grid::{
    GRID_MM, MAX_COORDINATE_MM, Paths64, canonical_millimetres, normalize, run, snap_polygon,
};
use crate::flatten::flatten_closed;
use i_overlay::core::overlay_rule::OverlayRule;

/// What to compute over the operands' regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// Everything covered by at least one operand.
    Union,
    /// The first operand minus everything covered by the other operands. For a reverse
    /// difference the caller puts the top-most operand first.
    Difference,
    /// Only what every operand covers.
    Intersection,
    /// What an odd number of operands cover.
    Exclusion,
}

/// One outline of an operand: anchors as `(point, handle_in, handle_out)`, handles relative to
/// the point, exactly as `curvyo-ui-core` reads a path. Only closed outlines are accepted; the
/// `closed` flag is carried so the kernel itself refuses an open path.
#[derive(Debug, Clone, Copy)]
pub struct Outline<'a> {
    /// The anchors in path order.
    pub anchors: &'a [OutlineTriple],
    /// Whether the outline is closed.
    pub closed: bool,
}

impl<'a> Outline<'a> {
    /// An outline through `anchors`.
    #[must_use]
    pub const fn new(anchors: &'a [OutlineTriple], closed: bool) -> Self {
        Self { anchors, closed }
    }
}

/// Why an operation was refused. Nothing is computed or changed in any of these cases.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BooleanError {
    /// No operands were given.
    #[error("no operands")]
    NoOperands,
    /// The tolerance leaves no room for the grid: it must exceed two grid pitches (0.002 mm).
    #[error("tolerance too small for the 0.001 mm grid")]
    ToleranceTooSmall,
    /// These operands (indices into the operand list) contain an open outline.
    #[error("operands {0:?} contain an open outline")]
    OpenOperands(Vec<usize>),
    /// These operands have a coordinate or handle that is not finite or lies beyond 10⁷ mm.
    #[error("operands {0:?} have a coordinate that is not finite or out of range")]
    OutOfRange(Vec<usize>),
    /// These operands enclose no area.
    #[error("operands {0:?} enclose no area")]
    EmptyOperands(Vec<usize>),
    /// The operation's region is empty.
    #[error("the result is empty")]
    EmptyResult,
}

/// The result of an operation: closed polygons, outer outlines and holes together.
///
/// The form is fixed so that results compare equal node for node (criteria 41 and 43):
///
/// - Every outline has at least three vertices, a non-zero area and no repeated vertex next to
///   itself. Outlines do not cross each other or themselves, but may touch at a vertex or, by
///   less than one grid pitch, along an edge. Consecutive vertices are at least 0.001 mm apart
///   and no vertex lies within 0.001 mm of the line between its neighbours, except where outlines
///   touch (a vertex shared by two outlines, or visited twice by one).
/// - **Winding:** an outer outline has a positive [`signed_area_mm2`] (clockwise on screen,
///   where Y grows downward); a hole has a negative one. A hole lies inside one outer outline,
///   an island inside a hole. Filled with the nonzero rule, the outlines together paint exactly
///   the result region.
/// - Every outline starts at its smallest vertex (x, then y). Outlines are sorted by that
///   vertex, ties by area (larger first).
/// - Every coordinate is a whole multiple of 0.001 mm.
#[derive(Debug, Clone, PartialEq)]
pub struct BooleanResult {
    outlines: Vec<Vec<Point>>,
}

impl BooleanResult {
    /// The outlines in canonical order. Never empty.
    #[must_use]
    pub fn outlines(&self) -> &[Vec<Point>] {
        &self.outlines
    }

    /// The outlines, by value.
    #[must_use]
    pub fn into_outlines(self) -> Vec<Vec<Point>> {
        self.outlines
    }
}

/// The signed area of a polygon in square millimetres: positive for the winding of an outer
/// outline of a [`BooleanResult`], negative for a hole.
#[must_use]
pub fn signed_area_mm2(polygon: &[Point]) -> f64 {
    let Some(&last) = polygon.last() else {
        return 0.0;
    };
    let mut previous = last;
    let mut twice = 0.0;
    for point in polygon {
        twice += previous.x * point.y - point.x * previous.y;
        previous = *point;
    }
    twice / 2.0
}

/// Whether every number that describes the outline is finite and within range.
fn outline_in_range(outline: &Outline<'_>) -> bool {
    let in_range = |value: f64| value.is_finite() && value.abs() <= MAX_COORDINATE_MM;
    outline
        .anchors
        .iter()
        .all(|(point, handle_in, handle_out)| {
            [
                point.x,
                point.y,
                point.x + handle_in.x,
                point.y + handle_in.y,
                point.x + handle_out.x,
                point.y + handle_out.y,
            ]
            .into_iter()
            .all(in_range)
        })
}

/// Indices of the operands for which `bad` holds for any outline.
fn operands_where(operands: &[&[Outline<'_>]], bad: impl Fn(&Outline<'_>) -> bool) -> Vec<usize> {
    operands
        .iter()
        .enumerate()
        .filter(|(_, outlines)| outlines.iter().any(&bad))
        .map(|(index, _)| index)
        .collect()
}

/// Flattens and snaps every outline of an operand into one set of grid polygons.
fn grid_paths(operand: &[Outline<'_>], flatten_tolerance_mm: f64) -> Paths64 {
    operand
        .iter()
        .map(|outline| snap_polygon(&flatten_closed(outline.anchors, flatten_tolerance_mm)))
        .collect()
}

/// Computes `op` over the regions of `operands`; the first operand is the base of a difference.
///
/// Each operand is a list of outlines whose painted region (nonzero rule) is the operand's
/// region, so a compound operand passes its outer outline and its holes together. `tolerance` is
/// the largest distance the result's edges may lie from the exact result of the operands'
/// curves; the kernel keeps two grid pitches (0.002 mm) of it for snapping and cleanup and
/// flattens to the rest.
///
/// # Errors
///
/// Refuses, changing nothing, when there are no operands, the tolerance is not above 0.002 mm,
/// an operand has an open outline, a non-finite or out-of-range coordinate, or no area, or the
/// result is empty. The checks run in that order, and a refusal lists every offending operand.
pub fn boolean(
    op: BooleanOp,
    operands: &[&[Outline<'_>]],
    tolerance: Tolerance,
) -> Result<BooleanResult, BooleanError> {
    if operands.is_empty() {
        return Err(BooleanError::NoOperands);
    }
    let flatten_tolerance_mm = tolerance.as_mm() - 2.0 * GRID_MM;
    if !(flatten_tolerance_mm > 0.0 && flatten_tolerance_mm.is_finite()) {
        return Err(BooleanError::ToleranceTooSmall);
    }
    let open = operands_where(operands, |outline| !outline.closed);
    if !open.is_empty() {
        return Err(BooleanError::OpenOperands(open));
    }
    let out_of_range = operands_where(operands, |outline| !outline_in_range(outline));
    if !out_of_range.is_empty() {
        return Err(BooleanError::OutOfRange(out_of_range));
    }

    let mut regions: Vec<Paths64> = Vec::with_capacity(operands.len());
    let mut empty = Vec::new();
    for (index, operand) in operands.iter().enumerate() {
        let region = normalize(&grid_paths(operand, flatten_tolerance_mm));
        if region.is_empty() {
            empty.push(index);
        }
        regions.push(region);
    }
    if !empty.is_empty() {
        return Err(BooleanError::EmptyOperands(empty));
    }

    let raw = combine(op, regions);
    let outlines = canonical_millimetres(&cleanup(&raw));
    if outlines.is_empty() {
        return Err(BooleanError::EmptyResult);
    }
    Ok(BooleanResult { outlines })
}

/// Step 5: the operation on normalized regions, at least one.
fn combine(op: BooleanOp, regions: Vec<Paths64>) -> Paths64 {
    let mut regions = regions.into_iter();
    let first = regions.next().unwrap_or_default();
    match op {
        BooleanOp::Union => {
            let all: Paths64 = std::iter::once(first).chain(regions).flatten().collect();
            normalize(&all)
        }
        BooleanOp::Difference => {
            let rest: Paths64 = regions.flatten().collect();
            if rest.is_empty() {
                return first;
            }
            run(OverlayRule::Difference, &first, &rest)
        }
        BooleanOp::Intersection => fold(OverlayRule::Intersect, first, regions),
        BooleanOp::Exclusion => fold(OverlayRule::Xor, first, regions),
    }
}

/// Applies a binary overlay operation left to right, stopping early on an empty accumulator for
/// an intersection, where nothing can come back.
fn fold(rule: OverlayRule, first: Paths64, rest: impl Iterator<Item = Paths64>) -> Paths64 {
    let mut accumulator = first;
    for next in rest {
        accumulator = run(rule, &accumulator, &next);
        if accumulator.is_empty() && rule == OverlayRule::Intersect {
            break;
        }
    }
    accumulator
}
