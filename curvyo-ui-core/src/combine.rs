//! Planning Combine on the selected objects: which outlines go into the one compound path, how
//! each must run so that inner shapes become holes, and why the command was refused
//! (`specs/0035-combine-and-break-apart`).
//!
//! Pure: it reads snapshots and the selection and returns a plan or a refusal. The one write
//! (`Document::replace_with_path`) is the session's. Nothing is flattened or moved: the plan holds
//! the operands' own nodes, only reversed where the winding needs it.

use curvyo_document_core::{
    AnchorId, NewAnchor, NodeId, ObjectSnapshot, outline_of_rotated, reversed_anchors,
};
use curvyo_geometry_core::{
    MAX_COORDINATE_MM, Outline, OutlineTriple, outline_area_mm2, outline_has_area, outline_nesting,
    touching_outlines,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::boolean::{BooleanAvailability, boolean_availability, is_open, operands_in_order};
use crate::object_selection::ObjectSelection;

/// The commit label of Combine; the undo slice maps it to display text.
pub const COMBINE_COMMIT_LABEL: &str = "combine_paths";

/// What the two buttons of the Path card show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathAvailability {
    /// Combine follows the rule of the Boolean buttons: two or more selected objects, an open
    /// path among them enabling the button but refusing the press.
    pub combine: BooleanAvailability,
    /// Break apart is enabled when the selection holds a compound path.
    pub break_apart: bool,
}

/// What the Path card shows for `selection`, counting only selected ids the document still
/// holds. The one rule the buttons and the plans share. The caller passes an empty selection when
/// the Select tool is not active.
#[must_use]
pub fn path_availability(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
) -> PathAvailability {
    PathAvailability {
        combine: boolean_availability(objects, selection),
        break_apart: operands_in_order(objects, selection)
            .iter()
            .any(|object| is_compound(object)),
    }
}

/// Whether `object` is a path with more than one outline.
pub(crate) fn is_compound(object: &ObjectSnapshot) -> bool {
    matches!(object, ObjectSnapshot::Path(path) if path.is_compound())
}

/// Why Combine was refused. Nothing was changed in any of these cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombineRefusal {
    /// Fewer than two objects are selected.
    NeedsTwo,
    /// `offenders` of the `of` selected objects are open paths.
    OpenPaths {
        /// The open paths, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// `offenders` of the `of` selected objects have a coordinate that is not finite or beyond
    /// [`MAX_COORDINATE_MM`].
    OutOfRange {
        /// The objects out of range, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// `offenders` of the `of` selected objects have an outline that encloses no area.
    NoArea {
        /// The objects without area, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// `offenders` of the `of` selected objects touch or cross another selected object.
    Touching {
        /// Every object that touches another, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
    /// No two objects touch, but `offenders` of the `of` selected objects touch or cross
    /// themselves (a figure eight, or a compound path whose outlines touch).
    SelfTouching {
        /// The objects that touch themselves, to outline.
        offenders: Vec<NodeId>,
        /// How many objects are selected.
        of: usize,
    },
}

/// What [`plan_combine`] decided, ready for `Document::replace_with_path`.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinePlan {
    /// The operands, bottom-most first.
    pub operands: Vec<NodeId>,
    /// The operand the result takes its style and place from: the bottom-most.
    pub base: NodeId,
    /// Every outline of every operand with fresh anchor ids, in stacking order and each
    /// operand's own order, reversed where the winding needs it.
    pub outlines: Vec<(Vec<NewAnchor>, bool)>,
    /// How many outlines are holes (odd depth).
    pub holes: usize,
    /// Whether the operands' complete styles are not all equal.
    pub styles_differ: bool,
}

/// One closed outline of an operand, with the index of the operand it belongs to.
struct Contour {
    owner: usize,
    anchors: Vec<NewAnchor>,
}

/// The outlines of `object` with their own nodes. A primitive gives the outline "Object to path"
/// would write; its anchor ids are placeholders, replaced when the plan is made.
fn contours_of(owner: usize, object: &ObjectSnapshot) -> Vec<Contour> {
    match object {
        ObjectSnapshot::Path(path) => path
            .subpaths()
            .map(|subpath| Contour {
                owner,
                anchors: subpath.anchors.to_vec(),
            })
            .collect(),
        ObjectSnapshot::Primitive(primitive) => {
            let placeholder = AnchorId::new(0, 0);
            vec![Contour {
                owner,
                anchors: outline_of_rotated(&primitive.shape, primitive.rotation)
                    .into_iter()
                    .map(|a| NewAnchor {
                        id: placeholder,
                        point: a.point,
                        handle_in: a.handle_in,
                        handle_out: a.handle_out,
                        kind: a.kind,
                    })
                    .collect(),
            }]
        }
    }
}

fn triples(anchors: &[NewAnchor]) -> Vec<OutlineTriple> {
    anchors
        .iter()
        .map(|a| (a.point, a.handle_in, a.handle_out))
        .collect()
}

/// Whether a node or the end of one of its handles is not finite or lies beyond the kernel's
/// range.
fn out_of_range(anchors: &[NewAnchor]) -> bool {
    anchors.iter().any(|a| {
        [
            a.point.x,
            a.point.y,
            a.point.x + a.handle_in.x,
            a.point.y + a.handle_in.y,
            a.point.x + a.handle_out.x,
            a.point.y + a.handle_out.y,
        ]
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_MM)
    })
}

/// The ids of the operands whose flag is set, in stacking order.
fn ids_flagged(operands: &[&ObjectSnapshot], flags: &[bool]) -> Vec<NodeId> {
    let mut ids = Vec::new();
    for (object, &flag) in operands.iter().zip(flags) {
        if flag {
            ids.push(object.id());
        }
    }
    ids
}

/// The ids of the operands that own one of the `contours` picked by `bad`, in stacking order.
fn owners_where(
    operands: &[&ObjectSnapshot],
    contours: &[Contour],
    mut bad: impl FnMut(usize, &Contour) -> bool,
) -> Vec<NodeId> {
    let mut flagged = vec![false; operands.len()];
    for (index, contour) in contours.iter().enumerate() {
        if bad(index, contour) {
            flagged[contour.owner] = true;
        }
    }
    ids_flagged(operands, &flagged)
}

/// Plans Combine over the selected objects.
///
/// The stacking order decides the outline order and the base operand, never the order of the
/// selection (criterion 8). Checks run in a fixed order and list every offender: open paths,
/// range, no area, then touching (different objects first, an object touching itself second).
///
/// # Errors
/// A [`CombineRefusal`] with the objects to outline.
pub fn plan_combine(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    minter: &mut AnchorIdMinter,
) -> Result<CombinePlan, CombineRefusal> {
    let operands = operands_in_order(objects, selection);
    let of = operands.len();
    if of < 2 {
        return Err(CombineRefusal::NeedsTwo);
    }
    let open: Vec<NodeId> = operands
        .iter()
        .filter(|object| is_open(object))
        .map(|object| object.id())
        .collect();
    if !open.is_empty() {
        return Err(CombineRefusal::OpenPaths {
            offenders: open,
            of,
        });
    }
    let contours: Vec<Contour> = operands
        .iter()
        .enumerate()
        .flat_map(|(owner, object)| contours_of(owner, object))
        .collect();
    let offenders = owners_where(&operands, &contours, |_, c| out_of_range(&c.anchors));
    if !offenders.is_empty() {
        return Err(CombineRefusal::OutOfRange { offenders, of });
    }
    let owned: Vec<Vec<OutlineTriple>> = contours.iter().map(|c| triples(&c.anchors)).collect();
    let outlines: Vec<Outline<'_>> = owned.iter().map(|t| Outline::new(t, true)).collect();
    let areas: Vec<f64> = outlines.iter().map(outline_area_mm2).collect();
    let offenders = owners_where(&operands, &contours, |index, _| {
        !outline_has_area(&outlines[index])
    });
    if !offenders.is_empty() {
        return Err(CombineRefusal::NoArea { offenders, of });
    }
    check_touches(&operands, &contours, &outlines, of)?;

    let nesting = outline_nesting(&outlines);
    let holes = nesting.iter().filter(|n| n.depth % 2 == 1).count();
    let result: Vec<(Vec<NewAnchor>, bool)> = contours
        .iter()
        .zip(&nesting)
        .zip(&areas)
        .map(|((contour, nest), &area)| {
            let wants_shape = nest.depth % 2 == 0;
            let anchors = if (area > 0.0) == wants_shape {
                contour.anchors.clone()
            } else {
                reversed_anchors(&contour.anchors)
            };
            (fresh_ids(anchors, minter), true)
        })
        .collect();
    let first_style = operands[0].style();
    Ok(CombinePlan {
        operands: operands.iter().map(|object| object.id()).collect(),
        base: operands[0].id(),
        outlines: result,
        holes,
        styles_differ: operands.iter().any(|object| object.style() != first_style),
    })
}

/// Refuses when two outlines touch or cross: different objects first, then one object alone.
fn check_touches(
    operands: &[&ObjectSnapshot],
    contours: &[Contour],
    outlines: &[Outline<'_>],
    of: usize,
) -> Result<(), CombineRefusal> {
    let pairs = touching_outlines(outlines);
    let mut across = vec![false; operands.len()];
    let mut alone = vec![false; operands.len()];
    for &(a, b) in &pairs {
        let (owner_a, owner_b) = (contours[a].owner, contours[b].owner);
        if owner_a == owner_b {
            alone[owner_a] = true;
        } else {
            across[owner_a] = true;
            across[owner_b] = true;
        }
    }
    let offenders = ids_flagged(operands, &across);
    if !offenders.is_empty() {
        return Err(CombineRefusal::Touching { offenders, of });
    }
    let offenders = ids_flagged(operands, &alone);
    if !offenders.is_empty() {
        return Err(CombineRefusal::SelfTouching { offenders, of });
    }
    Ok(())
}

/// `anchors` with a fresh id on each.
pub(crate) fn fresh_ids(anchors: Vec<NewAnchor>, minter: &mut AnchorIdMinter) -> Vec<NewAnchor> {
    anchors
        .into_iter()
        .map(|anchor| NewAnchor {
            id: minter.mint(),
            ..anchor
        })
        .collect()
}
