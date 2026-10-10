//! The one command that replaces a set of objects by a new path with one or
//! several outlines in a single commit: the document half of a boolean
//! operation (`specs/0016-boolean-operations/adrs.md`, "where the code
//! lives"). It computes no geometry; the caller passes the result.

use std::collections::HashSet;

use loro::TreeParentId;

use crate::document::{Document, OBJECTS_TREE};
use crate::objects::ObjectEditError;
use crate::path_codec::{self, anchor_map_at, insert_anchors_container, node_exists, push_anchor};
use crate::path_model::{AnchorId, NewAnchor, NodeId};
use crate::paths::tree_id_of;
use crate::style_codec;
use crate::subpath_codec::{all_anchor_lists, write_extra_subpaths};

impl Document {
    /// Replaces `operands` by one new path made of `outlines`, in **one
    /// commit** labelled `label` (criteria 19 to 22, 27 and 28 of
    /// `specs/0016-boolean-operations`):
    ///
    /// - the new object sits directly after `base` in the stacking order
    ///   (`base` must be one of `operands`), so objects between the operands
    ///   that were not replaced keep their places;
    /// - it takes `base`'s complete style, whichever kind `base` is, and no
    ///   rotation (the register is absent, so it reads as 0);
    /// - the first outline is stored in the ordinary keys and the others as
    ///   [`crate::PathSnapshot::extra_subpaths`], so one outline gives an
    ///   ordinary path;
    /// - every operand is deleted.
    ///
    /// Every id is resolved and every anchor id checked before the first
    /// write, so a refusal changes nothing and the operands are never removed
    /// without the result being added. A peer's concurrent edit of an operand
    /// is lost with the operand, as with a delete. Returns the new object's id.
    ///
    /// Each outline is `(anchors, closed)`; the caller mints the anchor ids,
    /// which must be different over all outlines (this crate never mints one).
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if an operand no longer exists;
    /// [`ObjectEditError::BaseNotAnOperand`] if `base` is not one of
    /// `operands`; [`ObjectEditError::NoOutlines`] if there is no outline or an
    /// outline has no anchor; [`ObjectEditError::AnchorIds`] if two anchors
    /// share an id or an id already belongs to an object that is not replaced.
    ///
    /// # Panics
    /// Does not panic in practice: every write below goes to a tree node and
    /// meta map this call just created, or to one it just resolved.
    pub fn replace_with_path(
        &self,
        operands: &[NodeId],
        base: NodeId,
        outlines: &[(Vec<NewAnchor>, bool)],
        label: &str,
    ) -> Result<NodeId, ObjectEditError> {
        if !operands.contains(&base) {
            return Err(ObjectEditError::BaseNotAnOperand);
        }
        if outlines.is_empty() || outlines.iter().any(|(anchors, _)| anchors.is_empty()) {
            return Err(ObjectEditError::NoOutlines);
        }
        let mut seen: HashSet<AnchorId> = HashSet::new();
        if !outlines
            .iter()
            .flat_map(|(anchors, _)| anchors)
            .all(|anchor| seen.insert(anchor.id))
        {
            return Err(ObjectEditError::AnchorIds);
        }
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let mut doomed = Vec::with_capacity(operands.len());
        for &operand in operands {
            let tree_id = tree_id_of(operand);
            if !node_exists(&tree, tree_id) {
                return Err(ObjectEditError::NoSuchObject);
            }
            if !doomed.contains(&tree_id) {
                doomed.push(tree_id);
            }
        }
        check_new_anchor_ids(&tree, &doomed, &seen)?;
        let base_meta = tree
            .get_meta(tree_id_of(base))
            .map_err(|_| ObjectEditError::NoSuchObject)?;
        let style = style_codec::read_style(&base_meta);

        tree.enable_fractional_index(0);
        let new_tree_id = create_path_after(&tree, tree_id_of(base), &style, outlines);
        for tree_id in doomed {
            // invariant: every id here was just confirmed present above.
            #[allow(clippy::unwrap_used)]
            tree.delete(tree_id).unwrap();
        }
        self.commit_with_label(label);
        Ok(NodeId::from_parts(new_tree_id.peer, new_tree_id.counter))
    }

    /// Replaces each compound path of `parts` by one object per region, in **one commit** labelled
    /// `break_apart` (`specs/0035-combine-and-break-apart` criteria 13 to 15 and 17). `parts`
    /// pairs a compound path with its regions, each region a list of outlines `(anchors, closed)`
    /// whose first is the region's shape and the others its holes. The pieces sit consecutively
    /// at the compound's place, in the order given, each with the compound's complete style and
    /// no rotation; the compound is deleted. A region with one outline is an ordinary path.
    ///
    /// Every id is resolved and every anchor id checked before the first write, so a refusal
    /// changes nothing. Returns the new objects' ids in order, part by part.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if a compound path no longer exists;
    /// [`ObjectEditError::NoOutlines`] if a part has no region, a region no outline or an outline
    /// no anchor; [`ObjectEditError::AnchorIds`] as for [`Document::replace_with_path`].
    ///
    /// # Panics
    /// Does not panic in practice, for the reasons given at [`Document::replace_with_path`].
    #[allow(clippy::type_complexity)]
    pub fn break_apart(
        &self,
        parts: &[(NodeId, Vec<Vec<(Vec<NewAnchor>, bool)>>)],
    ) -> Result<Vec<NodeId>, ObjectEditError> {
        let mut seen: HashSet<AnchorId> = HashSet::new();
        for (_, regions) in parts {
            if regions.is_empty() {
                return Err(ObjectEditError::NoOutlines);
            }
            for outlines in regions {
                if outlines.is_empty() || outlines.iter().any(|(anchors, _)| anchors.is_empty()) {
                    return Err(ObjectEditError::NoOutlines);
                }
                for anchor in outlines.iter().flat_map(|(anchors, _)| anchors) {
                    if !seen.insert(anchor.id) {
                        return Err(ObjectEditError::AnchorIds);
                    }
                }
            }
        }
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let mut doomed = Vec::with_capacity(parts.len());
        let mut styles = Vec::with_capacity(parts.len());
        for (compound, _) in parts {
            let tree_id = tree_id_of(*compound);
            if !node_exists(&tree, tree_id) {
                return Err(ObjectEditError::NoSuchObject);
            }
            let meta = tree
                .get_meta(tree_id)
                .map_err(|_| ObjectEditError::NoSuchObject)?;
            styles.push(style_codec::read_style(&meta));
            if !doomed.contains(&tree_id) {
                doomed.push(tree_id);
            }
        }
        check_new_anchor_ids(&tree, &doomed, &seen)?;

        tree.enable_fractional_index(0);
        let mut created = Vec::new();
        for ((compound, regions), style) in parts.iter().zip(&styles) {
            let mut after = tree_id_of(*compound);
            for outlines in regions {
                let piece = create_path_after(&tree, after, style, outlines);
                created.push(NodeId::from_parts(piece.peer, piece.counter));
                after = piece;
            }
        }
        for tree_id in doomed {
            // invariant: every id here was just confirmed present above.
            #[allow(clippy::unwrap_used)]
            tree.delete(tree_id).unwrap();
        }
        self.commit_with_label("break_apart");
        Ok(created)
    }
}

/// Creates a new path object directly after `after` in the stacking order, with `style`, no
/// rotation, and `outlines` (the first stored in the ordinary keys, the rest as extra subpaths).
fn create_path_after(
    tree: &loro::LoroTree,
    after: loro::TreeID,
    style: &crate::style_model::Style,
    outlines: &[(Vec<NewAnchor>, bool)],
) -> loro::TreeID {
    // `mov_after` needs the fractional-index feature on for this tree (the caller enabled it).
    // invariant: a root-level create on a live tree cannot fail, and reading the meta map of the
    // node just created cannot fail.
    #[allow(clippy::unwrap_used)]
    let new_tree_id = tree.create(TreeParentId::Root).unwrap();
    #[allow(clippy::unwrap_used)]
    let meta = tree.get_meta(new_tree_id).unwrap();
    // invariant: the caller checked `outlines` to be non-empty.
    #[allow(clippy::unwrap_used)]
    let (first, rest) = outlines.split_first().unwrap();
    path_codec::write_path_style(&meta, first.1, style);
    let anchor_list = insert_anchors_container(&meta);
    for anchor in &first.0 {
        push_anchor(&anchor_list, anchor);
    }
    write_extra_subpaths(&meta, rest);
    // invariant: `after` was resolved by the caller and the new node was just created: both are
    // live root siblings.
    #[allow(clippy::unwrap_used)]
    tree.mov_after(new_tree_id, after).unwrap();
    new_tree_id
}

/// An anchor id names a node across the whole document: none of `seen`, the ids a command is
/// about to write, may belong to an object that stays (`doomed` are the objects the command
/// deletes).
fn check_new_anchor_ids(
    tree: &loro::LoroTree,
    doomed: &[loro::TreeID],
    seen: &HashSet<AnchorId>,
) -> Result<(), ObjectEditError> {
    for root in tree.roots() {
        if doomed.contains(&root) {
            continue;
        }
        // invariant: a root of the tree has a meta map.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(root).unwrap();
        if crate::shape_codec::read_shape_tag(&meta).is_some() {
            continue;
        }
        for list in all_anchor_lists(&meta) {
            for index in 0..list.len() {
                let existing = path_codec::read_anchor_id(&anchor_map_at(&list, index));
                if existing.is_some_and(|id| seen.contains(&id)) {
                    return Err(ObjectEditError::AnchorIds);
                }
            }
        }
    }
    Ok(())
}
