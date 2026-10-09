//! Planning a boolean operation on the selected objects: which objects take part, in which
//! order, what the kernel is asked, and why it was refused (`specs/0016-boolean-operations`).
//!
//! Pure: it reads snapshots and the selection and returns a plan or a refusal. The one write
//! (`Document::replace_with_path`) is the session's. Refusals carry codes, counts and the ids of
//! the offending objects; the sentences are the host's (`docs/design-system.md`, "Action
//! notice").

use curvyo_document_core::{
    NewAnchor, NodeId, ObjectSnapshot, Point, Tolerance, outline_of_rotated,
};
use curvyo_geometry_core::{
    BooleanError, BooleanOp as KernelOp, Outline, OutlineTriple, boolean as run_kernel,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::object_selection::ObjectSelection;

/// The kernel tolerance: curves are flattened to straight segments that stay within 0.01 mm of
/// the true curve. Fixed, not a setting, and not the display tolerance (ADR 0003 §7).
const BOOLEAN_TOLERANCE: Tolerance = Tolerance::from_mm(0.01);

/// The five operations a maker can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// Everything covered by at least one operand.
    Union,
    /// The bottom-most operand minus the others.
    Difference,
    /// Only what every operand covers.
    Intersection,
    /// What an odd number of operands cover.
    Exclusion,
    /// The top-most operand minus the others.
    ReverseDifference,
}

impl BooleanOp {
    /// The operation named by its code (`"union"`, `"difference"`, `"intersection"`,
    /// `"exclusion"`, `"reverse_difference"`), the host's name for it.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "union" => Self::Union,
            "difference" => Self::Difference,
            "intersection" => Self::Intersection,
            "exclusion" => Self::Exclusion,
            "reverse_difference" => Self::ReverseDifference,
            _ => return None,
        })
    }

    /// The label of the commit this operation writes; the undo slice maps it to display text.
    #[must_use]
    pub const fn commit_label(self) -> &'static str {
        match self {
            Self::Union => "boolean_union",
            Self::Difference => "boolean_difference",
            Self::Intersection => "boolean_intersection",
            Self::Exclusion => "boolean_exclusion",
            Self::ReverseDifference => "boolean_reverse_difference",
        }
    }
}

/// Whether the buttons can act on the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanAvailability {
    /// Fewer than two objects are selected: the buttons are dimmed.
    NeedsTwo,
    /// Two or more are selected and `open` of the `of` selected objects are open paths: the
    /// buttons are enabled and an activation is refused with the count.
    OpenPaths {
        /// How many of the selected objects are open paths.
        open: usize,
        /// How many objects are selected.
        of: usize,
    },
    /// Two or more closed objects are selected.
    Ready,
}

/// Which selected objects the document still holds, in stacking order (bottom first).
fn operands_in_order<'a>(
    objects: &'a [ObjectSnapshot],
    selection: &ObjectSelection,
) -> Vec<&'a ObjectSnapshot> {
    objects
        .iter()
        .filter(|object| selection.contains(object.id()))
        .collect()
}

fn is_open(object: &ObjectSnapshot) -> bool {
    match object {
        ObjectSnapshot::Path(path) => path.subpaths().any(|subpath| !subpath.closed),
        ObjectSnapshot::Primitive(_) => false,
    }
}

/// What the buttons show for `selection`, counting only selected ids the document still holds.
/// The one rule the buttons and [`plan_boolean`] share, so a button and the command cannot
/// disagree. The caller passes an empty selection when the Select tool is not active.
#[must_use]
pub fn boolean_availability(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
) -> BooleanAvailability {
    let operands = operands_in_order(objects, selection);
    if operands.len() < 2 {
        return BooleanAvailability::NeedsTwo;
    }
    let open = operands.iter().filter(|object| is_open(object)).count();
    if open > 0 {
        BooleanAvailability::OpenPaths {
            open,
            of: operands.len(),
        }
    } else {
        BooleanAvailability::Ready
    }
}

/// Why an operation was refused. Nothing was changed in any of these cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanRefusal {
    /// Fewer than two objects are selected.
    NeedsTwo,
    /// `offenders` of the `of` selected objects are open paths.
    OpenPaths {
        /// The open paths, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// `offenders` of the `of` selected objects enclose no area.
    NoArea {
        /// The objects without area, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// `offenders` of the `of` selected objects have a coordinate the kernel cannot work with
    /// (not finite, or beyond 10 km from the origin).
    OutOfRange {
        /// The objects out of range, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// The result region is empty.
    Empty,
}

/// What [`plan_boolean`] decided: the objects to replace, the base operand and the outlines of
/// the result, ready for `Document::replace_with_path`.
#[derive(Debug, Clone, PartialEq)]
pub struct BooleanPlan {
    /// The operands, bottom-most first.
    pub operands: Vec<NodeId>,
    /// The operand the result is made from (its style, its place): the bottom-most one, except for
    /// a reverse difference, where it is the top-most.
    pub base: NodeId,
    /// The result: corner-only closed outlines with fresh anchor ids, outer outlines first by
    /// the kernel's canonical order.
    pub outlines: Vec<(Vec<NewAnchor>, bool)>,
    /// Whether the result has several outlines (a compound path).
    pub compound: bool,
}

/// The outlines of one object as the kernel reads them: a path's own outlines, a primitive's
/// rotated outline.
fn outlines_of(object: &ObjectSnapshot) -> Vec<(Vec<OutlineTriple>, bool)> {
    match object {
        ObjectSnapshot::Path(path) => path
            .subpaths()
            .map(|subpath| {
                (
                    subpath
                        .anchors
                        .iter()
                        .map(|a| (a.point, a.handle_in, a.handle_out))
                        .collect(),
                    subpath.closed,
                )
            })
            .collect(),
        ObjectSnapshot::Primitive(primitive) => vec![(
            outline_of_rotated(&primitive.shape, primitive.rotation)
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect(),
            true,
        )],
    }
}

/// The operands in the order the kernel takes them, the kernel's operation, and the base operand
/// (its style and its place in the stacking order). The first operand is the base of a
/// difference; a reverse difference puts the top-most first, the rest keeping their stacking
/// order.
///
/// `operands` holds two or more objects, bottom first ([`plan_boolean`] checked).
fn kernel_order<'a>(
    op: BooleanOp,
    operands: &[&'a ObjectSnapshot],
) -> (Vec<&'a ObjectSnapshot>, KernelOp, &'a ObjectSnapshot) {
    let mut order = operands.to_vec();
    let bottom = operands[0];
    match op {
        BooleanOp::Union => (order, KernelOp::Union, bottom),
        BooleanOp::Difference => (order, KernelOp::Difference, bottom),
        BooleanOp::Intersection => (order, KernelOp::Intersection, bottom),
        BooleanOp::Exclusion => (order, KernelOp::Exclusion, bottom),
        BooleanOp::ReverseDifference => {
            let top = order.pop().unwrap_or(bottom);
            order.insert(0, top);
            (order, KernelOp::Difference, top)
        }
    }
}

/// The refusal the kernel's `error` stands for. `order` is the operand order the kernel was
/// given (its indices name operands in it); `of` is how many objects are selected.
fn refusal_of(error: BooleanError, order: &[&ObjectSnapshot], of: usize) -> BooleanRefusal {
    let ids_at =
        |indices: &[usize]| -> Vec<NodeId> { indices.iter().map(|&i| order[i].id()).collect() };
    match error {
        BooleanError::OpenOperands(indices) => BooleanRefusal::OpenPaths {
            offenders: ids_at(&indices),
            of,
        },
        BooleanError::EmptyOperands(indices) => BooleanRefusal::NoArea {
            offenders: ids_at(&indices),
            of,
        },
        BooleanError::OutOfRange(indices) => BooleanRefusal::OutOfRange {
            offenders: ids_at(&indices),
            of,
        },
        BooleanError::EmptyResult => BooleanRefusal::Empty,
        BooleanError::NoOperands | BooleanError::ToleranceTooSmall => {
            // invariant: `plan_boolean` passes two or more operands and a constant tolerance
            // above two grid pitches. Shown as "empty" if a change ever makes them reachable,
            // after failing every test that gets here.
            debug_assert!(false, "unreachable kernel refusal: {error:?}");
            BooleanRefusal::Empty
        }
    }
}

/// Plans `op` over the selected objects.
///
/// The stacking order decides which operand is which, never the order of the selection
/// (criterion 9). Open paths are refused before the kernel runs (criterion 15), so the counts
/// match [`boolean_availability`]'s.
///
/// # Errors
/// A [`BooleanRefusal`] with the objects to outline.
pub fn plan_boolean(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    op: BooleanOp,
    minter: &mut AnchorIdMinter,
) -> Result<BooleanPlan, BooleanRefusal> {
    let operands = operands_in_order(objects, selection);
    let of = operands.len();
    match boolean_availability(objects, selection) {
        BooleanAvailability::NeedsTwo => return Err(BooleanRefusal::NeedsTwo),
        BooleanAvailability::OpenPaths { .. } => {
            return Err(BooleanRefusal::OpenPaths {
                offenders: operands
                    .iter()
                    .filter(|object| is_open(object))
                    .map(|object| object.id())
                    .collect(),
                of,
            });
        }
        BooleanAvailability::Ready => {}
    }
    let (order, kernel_op, base) = kernel_order(op, &operands);
    let owned: Vec<Vec<(Vec<OutlineTriple>, bool)>> =
        order.iter().map(|object| outlines_of(object)).collect();
    let borrowed: Vec<Vec<Outline<'_>>> = owned
        .iter()
        .map(|outlines| {
            outlines
                .iter()
                .map(|(anchors, closed)| Outline::new(anchors, *closed))
                .collect()
        })
        .collect();
    let slices: Vec<&[Outline<'_>]> = borrowed.iter().map(Vec::as_slice).collect();
    let result = run_kernel(kernel_op, &slices, BOOLEAN_TOLERANCE)
        .map_err(|error| refusal_of(error, &order, of))?;
    let outlines: Vec<(Vec<NewAnchor>, bool)> = result
        .into_outlines()
        .into_iter()
        .map(|points| (corner_anchors(&points, minter), true))
        .collect();
    Ok(BooleanPlan {
        operands: operands.iter().map(|object| object.id()).collect(),
        base: base.id(),
        compound: outlines.len() > 1,
        outlines,
    })
}

/// A closed outline of corner anchors with no handles and fresh ids.
fn corner_anchors(points: &[Point], minter: &mut AnchorIdMinter) -> Vec<NewAnchor> {
    points
        .iter()
        .map(|&point| NewAnchor::corner(minter.mint(), point))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{AnchorId, Document, Length, RectBounds};

    fn rect(document: &Document, x: f64, y: f64, side: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(side),
            height: Length::from_mm(side),
        })
    }

    fn selection_of(ids: &[NodeId]) -> ObjectSelection {
        let mut selection = ObjectSelection::new();
        for &id in ids {
            selection.add(id);
        }
        selection
    }

    fn objects(document: &Document) -> Vec<ObjectSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect()
    }

    #[test]
    fn the_availability_counts_only_selected_objects_the_document_still_holds() {
        let document = Document::new(1);
        let a = rect(&document, 0.0, 0.0, 10.0);
        let b = rect(&document, 5.0, 5.0, 10.0);
        let gone = rect(&document, 50.0, 0.0, 10.0);
        document.delete_objects(&[gone]).unwrap();
        let all = objects(&document);
        assert_eq!(
            boolean_availability(&all, &selection_of(&[a])),
            BooleanAvailability::NeedsTwo
        );
        assert_eq!(
            boolean_availability(&all, &selection_of(&[a, gone])),
            BooleanAvailability::NeedsTwo
        );
        assert_eq!(
            boolean_availability(&all, &selection_of(&[a, b])),
            BooleanAvailability::Ready
        );
        assert_eq!(
            boolean_availability(&all, &ObjectSelection::new()),
            BooleanAvailability::NeedsTwo
        );
    }

    #[test]
    fn an_open_path_makes_the_buttons_name_the_count() {
        let document = Document::new(1);
        let a = rect(&document, 0.0, 0.0, 10.0);
        let open = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(5.0, 5.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(9.0, 0.0)),
            ],
            false,
        );
        let b = rect(&document, 20.0, 0.0, 10.0);
        let all = objects(&document);
        assert_eq!(
            boolean_availability(&all, &selection_of(&[a, open, b])),
            BooleanAvailability::OpenPaths { open: 1, of: 3 }
        );
        let mut minter = AnchorIdMinter::new(5);
        assert_eq!(
            plan_boolean(
                &all,
                &selection_of(&[a, open, b]),
                BooleanOp::Union,
                &mut minter
            ),
            Err(BooleanRefusal::OpenPaths {
                offenders: vec![open],
                of: 3
            })
        );
    }

    #[test]
    fn stacking_order_decides_and_the_selection_order_does_not() {
        let document = Document::new(1);
        let low = rect(&document, 0.0, 0.0, 20.0);
        let high = rect(&document, 10.0, 10.0, 20.0);
        let all = objects(&document);
        let mut minter = AnchorIdMinter::new(5);
        let a = plan_boolean(
            &all,
            &selection_of(&[low, high]),
            BooleanOp::Difference,
            &mut minter,
        )
        .unwrap();
        let b = plan_boolean(
            &all,
            &selection_of(&[high, low]),
            BooleanOp::Difference,
            &mut AnchorIdMinter::new(5),
        )
        .unwrap();
        assert_eq!(a, b, "the same objects give the same plan");
        assert_eq!(a.operands, vec![low, high]);
        assert_eq!(a.base, low);
        let reverse = plan_boolean(
            &all,
            &selection_of(&[low, high]),
            BooleanOp::ReverseDifference,
            &mut AnchorIdMinter::new(5),
        )
        .unwrap();
        assert_eq!(
            reverse.base, high,
            "the base of a reverse difference is the top"
        );
        assert_ne!(reverse.outlines[0].0[0].point, a.outlines[0].0[0].point);
    }

    #[test]
    fn a_ring_is_a_compound_plan_and_an_empty_result_is_refused() {
        let document = Document::new(1);
        let outer = rect(&document, 0.0, 0.0, 40.0);
        let inner = rect(&document, 10.0, 10.0, 20.0);
        let all = objects(&document);
        let mut minter = AnchorIdMinter::new(5);
        let ring = plan_boolean(
            &all,
            &selection_of(&[outer, inner]),
            BooleanOp::Difference,
            &mut minter,
        )
        .unwrap();
        assert!(ring.compound);
        assert_eq!(ring.outlines.len(), 2);
        // The top object is covered by the bottom one completely: nothing is left of it.
        assert_eq!(
            plan_boolean(
                &all,
                &selection_of(&[outer, inner]),
                BooleanOp::ReverseDifference,
                &mut minter,
            ),
            Err(BooleanRefusal::Empty)
        );
        let far = rect(&document, 100.0, 0.0, 10.0);
        let all = objects(&document);
        assert_eq!(
            plan_boolean(
                &all,
                &selection_of(&[outer, far]),
                BooleanOp::Intersection,
                &mut minter,
            ),
            Err(BooleanRefusal::Empty)
        );
    }
}
