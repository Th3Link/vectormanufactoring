//! Planning Break apart on the selected compound paths: which outlines form a region (a shape
//! with the holes directly inside it) and what the pieces are
//! (`specs/0035-combine-and-break-apart`, criteria 13 to 16).
//!
//! Pure: it reads snapshots and the selection and returns a plan or a refusal. The one write
//! (`Document::break_apart`) is the session's. Nodes are copied as they are, no flattening and no
//! reversal: the windings of a compound path are canonical already.

use curvyo_document_core::{NewAnchor, NodeId, ObjectSnapshot, SubpathRef};
use curvyo_geometry_core::{Outline, OutlineTriple, outline_nesting};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::boolean::operands_in_order;
use crate::combine::{fresh_ids, is_compound};
use crate::object_selection::ObjectSelection;

/// The commit label of Break apart; the undo slice maps it to display text.
pub const BREAK_APART_COMMIT_LABEL: &str = "break_apart";

/// One piece: the outlines of a region, the shape first and its holes after it.
pub type Piece = Vec<(Vec<NewAnchor>, bool)>;

/// Why Break apart was refused. Nothing was changed in either case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakApartRefusal {
    /// The selection holds no compound path (the button is dimmed then; reachable only through
    /// the session function).
    NoCompound,
    /// Every selected compound path is one region: `compounds` of them, each already one piece
    /// with its holes.
    OnePiece {
        /// How many compound paths are selected.
        compounds: usize,
    },
}

/// What [`plan_break_apart`] decided, ready for `Document::break_apart`.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakApartPlan {
    /// The compound paths to replace, bottom-most first, each with its pieces in the order of
    /// their first outline and with fresh anchor ids.
    pub parts: Vec<(NodeId, Vec<Piece>)>,
    /// How many pieces there are in all.
    pub pieces: usize,
    /// Whether a piece has a hole.
    pub with_holes: bool,
    /// The selected objects that stay as they are: ordinary objects and compound paths of one
    /// region.
    pub kept: Vec<NodeId>,
    /// How many of those are compound paths of one region.
    pub left_alone: usize,
}

/// The regions of a compound path, each as indices into its outlines: an even-depth outline, then
/// the outlines directly inside it. Ordered by the first outline of each region.
fn regions_of(outlines: &[SubpathRef<'_>]) -> Vec<Vec<usize>> {
    let owned: Vec<Vec<OutlineTriple>> = outlines
        .iter()
        .map(|outline| {
            outline
                .anchors
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect()
        })
        .collect();
    let views: Vec<Outline<'_>> = owned
        .iter()
        .zip(outlines)
        .map(|(triples, outline)| Outline::new(triples, outline.closed))
        .collect();
    let nesting = outline_nesting(&views);
    let mut regions: Vec<Vec<usize>> = Vec::new();
    let mut region_of = vec![usize::MAX; outlines.len()];
    for (index, nest) in nesting.iter().enumerate() {
        if nest.depth % 2 == 0 {
            region_of[index] = regions.len();
            regions.push(vec![index]);
        }
    }
    for (index, nest) in nesting.iter().enumerate() {
        if nest.depth % 2 == 1 {
            // invariant: a hole's parent is the enclosing outline of depth one less, an even one,
            // which was given a region above.
            if let Some(region) = nest
                .parent
                .map(|parent| region_of[parent])
                .filter(|&region| region != usize::MAX)
            {
                regions[region].push(index);
            }
        }
    }
    regions
}

/// Plans Break apart over the selected objects.
///
/// # Errors
/// A [`BreakApartRefusal`]; nothing is outlined.
pub fn plan_break_apart(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    minter: &mut AnchorIdMinter,
) -> Result<BreakApartPlan, BreakApartRefusal> {
    let operands = operands_in_order(objects, selection);
    let compounds = operands.iter().filter(|object| is_compound(object)).count();
    if compounds == 0 {
        return Err(BreakApartRefusal::NoCompound);
    }
    let mut parts = Vec::new();
    let mut kept = Vec::new();
    let mut left_alone = 0;
    let (mut pieces, mut with_holes) = (0, false);
    for object in &operands {
        let ObjectSnapshot::Path(path) = object else {
            kept.push(object.id());
            continue;
        };
        if !path.is_compound() {
            kept.push(object.id());
            continue;
        }
        let outlines: Vec<SubpathRef<'_>> = path.subpaths().collect();
        let regions = regions_of(&outlines);
        if regions.len() < 2 {
            kept.push(object.id());
            left_alone += 1;
            continue;
        }
        pieces += regions.len();
        with_holes |= regions.iter().any(|region| region.len() > 1);
        let part = regions
            .iter()
            .map(|region| {
                region
                    .iter()
                    .map(|&index| {
                        let outline = &outlines[index];
                        (fresh_ids(outline.anchors.to_vec(), minter), outline.closed)
                    })
                    .collect()
            })
            .collect();
        parts.push((object.id(), part));
    }
    if parts.is_empty() {
        return Err(BreakApartRefusal::OnePiece { compounds });
    }
    Ok(BreakApartPlan {
        parts,
        pieces,
        with_holes,
        kept,
        left_alone,
    })
}
