//! `Document`'s general "any object" command methods
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`: "moving and
//! deleting objects writes to the document. Both are settled below, and
//! both reuse registers that already exist"). Unlike [`crate::paths`] and
//! [`crate::shapes`], these two commands act identically regardless of an
//! object's kind — a move never reads a path's curvature, and a delete
//! never reads geometry at all — so they live in their own module rather
//! than being added as a third kind-specific method set.

use loro::TreeID;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{self, KEY_POINT, anchors_container, node_exists};
use crate::path_model::NodeId;
use crate::primitive_model::translate_shape;
use crate::shape_codec;
use crate::units::Vec2;

/// Why an any-object [`Document`] method refused to apply — mirrors
/// [`crate::path_model::PathEditError`]/[`crate::shapes::ShapeEditError`]'s
/// own framing: a caller error against a snapshot that is already stale
/// (ADR 0009 §2), not a defect in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ObjectEditError {
    /// No object with this [`NodeId`] exists (deleted locally or by a
    /// collaborator since the caller last read a snapshot).
    #[error("no such object")]
    NoSuchObject,
}

fn tree_id_of(id: NodeId) -> TreeID {
    TreeID::new(id.peer, id.counter)
}

impl Document {
    /// Moves every named object by `offset`, in **one commit** for the
    /// whole batch (acceptance criteria 18, 20) — a path's anchor `point`s
    /// shift (its handles are relative, so they need no write); a
    /// primitive's frame origin/center shifts, via the exact same
    /// `translate_shape` rule
    /// [`crate::primitive_model::ObjectSnapshot::translated`]'s live
    /// preview uses, so the two can never disagree.
    ///
    /// Every id is resolved before the first write, so one stale id
    /// anywhere in `ids` refuses the whole call rather than moving a
    /// prefix of it.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if any named id no longer exists.
    pub fn translate_objects(&self, ids: &[NodeId], offset: Vec2) -> Result<(), ObjectEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| {
                let tree_id = tree_id_of(id);
                if !node_exists(&tree, tree_id) {
                    return Err(ObjectEditError::NoSuchObject);
                }
                tree.get_meta(tree_id)
                    .map_err(|_| ObjectEditError::NoSuchObject)
            })
            .collect::<Result<_, _>>()?;

        for meta in &metas {
            match shape_codec::read_shape_tag(meta) {
                Some(tag) => translate_primitive_meta(meta, &tag, offset),
                None => translate_path_meta(meta, offset),
            }
        }
        self.commit_with_label("translate_objects");
        Ok(())
    }

    /// Deletes every named object as one tree delete, one commit for the
    /// whole batch (acceptance criteria 19, 21) — identical for a path, a
    /// rectangle, an ellipse or a polygon/star, since a delete never reads
    /// geometry. A deleted path's anchors go with their own node; any
    /// selection still naming a deleted id drops it on the next lazy
    /// resolve (ADR 0009 §2).
    ///
    /// Every id is resolved before the first delete, so one stale id
    /// anywhere in `ids` refuses the whole call.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if any named id no longer exists.
    ///
    /// # Panics
    /// Does not panic in practice: every tree id deleted below was just
    /// confirmed present in the same tree handle.
    pub fn delete_objects(&self, ids: &[NodeId]) -> Result<(), ObjectEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_ids: Vec<TreeID> = ids
            .iter()
            .map(|&id| {
                let tree_id = tree_id_of(id);
                if node_exists(&tree, tree_id) {
                    Ok(tree_id)
                } else {
                    Err(ObjectEditError::NoSuchObject)
                }
            })
            .collect::<Result<_, _>>()?;

        for tree_id in tree_ids {
            // invariant: every id here was just confirmed present above.
            #[allow(clippy::unwrap_used)]
            tree.delete(tree_id).unwrap();
        }
        self.commit_with_label("delete_objects");
        Ok(())
    }
}

/// Shifts every one of a path's anchor `point`s by `offset`, leaving
/// handles (relative to their own anchor) untouched.
fn translate_path_meta(meta: &loro::LoroMap, offset: Vec2) {
    let anchors = anchors_container(meta);
    for index in 0..anchors.len() {
        let map = path_codec::anchor_map_at(&anchors, index);
        let point = path_codec::read_point(&map, KEY_POINT);
        path_codec::write_point(&map, KEY_POINT, point.translated(offset));
    }
}

/// Shifts a primitive's frame by `offset`, via the one shared
/// [`translate_shape`] rule — reads the full current [`crate::Shape`],
/// translates it, and writes back only the fields that rule actually
/// changed (point count/inner ratio/corner radius are never rewritten
/// here).
fn translate_primitive_meta(meta: &loro::LoroMap, shape_tag: &str, offset: Vec2) {
    let Some(shape) = shape_codec::read_shape(meta, shape_tag) else {
        return;
    };
    let translated = translate_shape(shape, offset);
    match translated {
        crate::primitive_model::Shape::Rect { bounds, .. } => {
            shape_codec::write_rect_bounds(meta, bounds);
        }
        crate::primitive_model::Shape::Ellipse { frame } => {
            shape_codec::write_ellipse_frame(meta, frame);
        }
        crate::primitive_model::Shape::Polygon { frame, .. }
        | crate::primitive_model::Shape::Star { frame, .. } => {
            shape_codec::write_star_frame(meta, frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, NewAnchor};
    use crate::primitive_model::Shape;
    use crate::units::{Length, Point};

    fn two_node_path(document: &Document) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            ],
            false,
        )
    }

    #[test]
    fn translate_objects_moves_a_path_and_a_rect_together_in_one_commit() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let rect = document.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });

        let before = document.loro().len_changes();
        document
            .translate_objects(&[path, rect], Vec2::new(5.0, 7.0))
            .expect("translate");
        let after = document.loro().len_changes();
        assert_eq!(after - before, 1, "one commit for the whole batch");

        let path_snapshot = document.path(path).expect("exists");
        assert_eq!(path_snapshot.anchors[0].point, Point::new(5.0, 7.0));
        assert_eq!(path_snapshot.anchors[1].point, Point::new(15.0, 7.0));

        let Shape::Rect { bounds, .. } = document.primitive(rect).expect("exists").shape else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(5.0, 7.0));
    }

    #[test]
    fn translate_objects_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let stale = {
            let throwaway = two_node_path(&document);
            document
                .delete_objects(&[throwaway])
                .expect("delete throwaway");
            throwaway
        };

        let result = document.translate_objects(&[path, stale], Vec2::new(1.0, 1.0));
        assert_eq!(result, Err(ObjectEditError::NoSuchObject));
        let snapshot = document.path(path).expect("exists");
        assert_eq!(
            snapshot.anchors[0].point,
            Point::new(0.0, 0.0),
            "the valid id earlier in the batch must not have moved either"
        );
    }

    #[test]
    fn delete_objects_removes_a_path_and_a_primitive_together_in_one_commit() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let rect = document.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });

        let before = document.loro().len_changes();
        document.delete_objects(&[path, rect]).expect("delete both");
        let after = document.loro().len_changes();
        assert_eq!(after - before, 1, "one commit for the whole batch");

        assert_eq!(document.path(path), None);
        assert_eq!(document.primitive(rect), None);
    }

    #[test]
    fn delete_objects_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let unknown = {
            let throwaway = two_node_path(&document);
            document
                .delete_objects(&[throwaway])
                .expect("delete throwaway");
            throwaway
        };

        let result = document.delete_objects(&[path, unknown]);
        assert_eq!(result, Err(ObjectEditError::NoSuchObject));
        assert!(
            document.path(path).is_some(),
            "untouched by the refused batch"
        );
    }
}
