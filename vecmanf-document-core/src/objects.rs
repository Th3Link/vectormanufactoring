//! `Document`'s general "any object" command methods
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`: "moving and
//! deleting objects writes to the document. Both are settled below, and
//! both reuse registers that already exist"). Unlike [`crate::paths`] and
//! [`crate::shapes`], these two commands act identically regardless of an
//! object's kind — a move never reads a path's curvature, and a delete
//! never reads geometry at all — so they live in their own module rather
//! than being added as a third kind-specific method set.

use std::collections::HashSet;

use loro::TreeID;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    self, KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, anchor_map_at, anchors_container, node_exists,
    write_point, write_vec2,
};
use crate::path_model::NodeId;
use crate::primitive_model::{ObjectSnapshot, translate_shape};
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

/// Deduplicates `ids`, keeping each one's first occurrence's position.
/// Both [`Document::translate_objects`] and [`Document::delete_objects`]
/// are public APIs on this crate's own `Document` — not reachable with a
/// duplicate id via the shipped UI today (`vecmanf-ui-core`'s own
/// `ObjectSelection::select_single`/`toggle` can't produce one), but
/// nothing in either function's own signature forbids a caller from
/// passing one, and without this a duplicate would double-apply an
/// offset in `translate_objects`, or panic on a second delete of an
/// already-deleted tree node in `delete_objects`.
fn dedup_ids(ids: &[NodeId]) -> Vec<NodeId> {
    let mut seen = HashSet::with_capacity(ids.len());
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
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
        let ids = dedup_ids(ids);
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

    /// Writes a resolved rotation — the object after
    /// [`ObjectSnapshot::rotated`] — in **one commit**
    /// (`specs/0005-object-transform/specification.md` acceptance
    /// criteria 15-18, 20): a primitive's frame (only if the rotation
    /// moved it, i.e. a pivot off its center) and `rotation`; a path's
    /// anchor points and handles and `rotation`. `vecmanf-ui-core`
    /// resolves the geometry with the same [`ObjectSnapshot::rotated`]
    /// its live preview renders, so preview and commit share one rule
    /// and this method holds no rotation arithmetic of its own.
    ///
    /// A rotate about the object's own center writes `rotation` alone: an
    /// unchanged frame is not rewritten, so a concurrent resize of the
    /// same primitive survives the merge next to it (both are separate
    /// registers, `adrs.md`'s merge-granularity table).
    ///
    /// Everything is resolved before the first write; one stale anchor,
    /// or an object that has since changed kind, refuses the whole call.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if the object no longer exists
    /// as the kind `rotated` describes, or one of a path's anchors is
    /// gone.
    pub fn rotate_object(&self, rotated: &ObjectSnapshot) -> Result<(), ObjectEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(rotated.id());
        if !node_exists(&tree, tree_id) {
            return Err(ObjectEditError::NoSuchObject);
        }
        let meta = tree
            .get_meta(tree_id)
            .map_err(|_| ObjectEditError::NoSuchObject)?;

        match (rotated, shape_codec::read_shape_tag(&meta)) {
            (ObjectSnapshot::Primitive(primitive), Some(tag)) => {
                let current = shape_codec::read_shape(&meta, &tag);
                if current != Some(primitive.shape) {
                    shape_codec::write_shape_frame(&meta, &primitive.shape);
                }
            }
            (ObjectSnapshot::Path(path), None) => {
                let anchors = anchors_container(&meta);
                let resolved: Vec<_> = path
                    .anchors
                    .iter()
                    .map(|a| {
                        path_codec::anchor_index(&anchors, a.id)
                            .map(|index| (index, a))
                            .map_err(|_| ObjectEditError::NoSuchObject)
                    })
                    .collect::<Result<_, _>>()?;
                for (index, anchor) in resolved {
                    let map = anchor_map_at(&anchors, index);
                    write_point(&map, KEY_POINT, anchor.point);
                    write_vec2(&map, KEY_HANDLE_IN, anchor.handle_in);
                    write_vec2(&map, KEY_HANDLE_OUT, anchor.handle_out);
                }
            }
            _ => return Err(ObjectEditError::NoSuchObject),
        }
        path_codec::write_rotation(&meta, rotated.rotation());
        self.commit_with_label("rotate_object");
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
        let ids = dedup_ids(ids);
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
    shape_codec::write_shape_frame(meta, &translate_shape(shape, offset));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, NewAnchor};
    use crate::primitive_model::Shape;
    use crate::units::{Angle, Length, Point};

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

    /// Acceptance criterion 15: rotating a rectangle about its own
    /// center leaves the frame center fixed and writes only `rotation`.
    #[test]
    fn rotate_object_rect_about_its_own_center_writes_only_rotation() {
        let document = Document::new(1);
        let id = document.create_rect(crate::primitive_model::RectBounds {
            origin: crate::units::Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let center = crate::units::Point::new(5.0, 5.0);
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(center, Angle::from_radians(std::f64::consts::FRAC_PI_2)),
            )
            .expect("rotate");
        let snapshot = document.primitive(id).expect("exists");
        assert!((snapshot.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!((bounds.origin.x - 0.0).abs() < 1e-9);
        assert!((bounds.origin.y - 0.0).abs() < 1e-9);
    }

    /// Acceptance criterion 16: a Shift-pivot rotate about a point other
    /// than the object's own center moves the frame too.
    #[test]
    fn rotate_object_rect_about_an_off_center_pivot_moves_the_frame() {
        let document = Document::new(1);
        let id = document.create_rect(crate::primitive_model::RectBounds {
            origin: crate::units::Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let pivot = crate::units::Point::new(0.0, 5.0);
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(pivot, Angle::from_radians(std::f64::consts::PI)),
            )
            .expect("rotate");
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        // Center was (5, 5); 180 degrees about (0, 5) -> (-5, 5), so the
        // origin (top-left) is now (-10, 0).
        assert!((bounds.origin.x - (-10.0)).abs() < 1e-9);
        assert!((bounds.origin.y - 0.0).abs() < 1e-9);
    }

    /// Acceptance criterion 20: rotating a path bakes anchors and
    /// advances its own `rotation` register, one commit.
    #[test]
    fn rotate_object_path_bakes_anchors_and_advances_rotation_in_one_commit() {
        let document = Document::new(1);
        let id = two_node_path(&document);
        let before = document.loro().len_changes();
        document
            .rotate_object(&document.object(id).expect("object exists").rotated(
                crate::units::Point::new(0.0, 0.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_2),
            ))
            .expect("rotate");
        let after = document.loro().len_changes();
        assert_eq!(after - before, 1, "one commit");
        let snapshot = document.path(id).expect("exists");
        // anchor[1] was (10, 0) -> rotated 90 degrees about origin -> (0, 10).
        assert!((snapshot.anchors[1].point.x - 0.0).abs() < 1e-6);
        assert!((snapshot.anchors[1].point.y - 10.0).abs() < 1e-6);
        assert!((snapshot.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    /// Rotating an unknown id is refused, writing nothing.
    #[test]
    fn rotate_object_on_an_unknown_id_is_refused() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let rotated = document
            .object(path)
            .expect("object exists")
            .rotated(Point::new(0.0, 0.0), Angle::from_radians(1.0));
        document.delete_objects(&[path]).expect("delete");
        assert_eq!(
            document.rotate_object(&rotated),
            Err(ObjectEditError::NoSuchObject)
        );
    }

    /// A resolved rotation for a path, applied after the object became a
    /// primitive (or the reverse), is refused rather than written into the
    /// wrong kind of node.
    #[test]
    fn rotate_object_refuses_a_snapshot_of_the_wrong_kind() {
        let document = Document::new(1);
        let rect = document.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(4.0),
            height: Length::from_mm(4.0),
        });
        let path_snapshot = document
            .object(two_node_path(&document))
            .expect("exists")
            .rotated(Point::new(0.0, 0.0), Angle::from_radians(0.5));
        let ObjectSnapshot::Path(mut path) = path_snapshot else {
            panic!("path");
        };
        path.id = rect;
        assert_eq!(
            document.rotate_object(&ObjectSnapshot::Path(path)),
            Err(ObjectEditError::NoSuchObject)
        );
    }

    /// A pivot a hair off the center (a rectangle's derived `shape_center`
    /// versus the box center `ui-core` computes can differ by an ulp) is
    /// still a center rotate: the frame is returned unchanged.
    #[test]
    fn rotating_about_a_pivot_within_tolerance_of_the_center_leaves_the_frame_alone() {
        let document = Document::new(1);
        let id = document.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(0.1, 0.2),
            width: Length::from_mm(10.3),
            height: Length::from_mm(7.7),
        });
        let before = document.primitive(id).expect("exists");
        let center = crate::primitive_model::shape_center(&before.shape);
        let off_by_an_ulp = Point::new(center.x + 1e-13, center.y - 1e-13);
        let rotated = document
            .object(id)
            .expect("exists")
            .rotated(off_by_an_ulp, Angle::from_radians(0.7));
        let ObjectSnapshot::Primitive(p) = &rotated else {
            panic!("primitive");
        };
        assert_eq!(p.shape, before.shape, "frame not rewritten");
        assert!((p.rotation.as_radians() - 0.7).abs() < 1e-12);
    }

    /// Architect item 1: peer A resizes while peer B rotates about the
    /// centre; after the merge the resize *and* the rotation both
    /// survive, because the centre rotate wrote `rotation` alone.
    #[test]
    fn a_concurrent_resize_and_centre_rotate_both_survive_the_merge() {
        use crate::primitive_model::{RectBounds, Shape};
        let a = Document::new(1);
        let id = a.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let b = Document::from_loro_snapshot(2, &a.export_loro_snapshot().expect("snapshot"))
            .expect("peer B opens the same document");

        a.resize_rect(
            id,
            RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(25.0),
                height: Length::from_mm(10.0),
            },
            Length::from_mm(0.0),
            Some(Length::from_mm(0.25)),
        )
        .expect("A resizes");
        // B rotates about the centre it sees (the old 10 x 10 frame).
        let center = Point::new(5.0, 5.0);
        let rotated = b
            .object(id)
            .expect("B sees it")
            .rotated(center, Angle::from_radians(0.9));
        b.rotate_object(&rotated).expect("B rotates");

        let from_b = b
            .loro()
            .export(loro::ExportMode::all_updates())
            .expect("export B");
        let from_a = a
            .loro()
            .export(loro::ExportMode::all_updates())
            .expect("export A");
        a.loro().import(&from_b).expect("A merges B");
        b.loro().import(&from_a).expect("B merges A");

        for peer in [&a, &b] {
            let merged = peer.primitive(id).expect("exists");
            let Shape::Rect { bounds, .. } = merged.shape else {
                panic!("rect");
            };
            assert!(
                (bounds.width.as_mm() - 25.0).abs() < 1e-9,
                "resize survived"
            );
            assert!(
                (merged.rotation.as_radians() - 0.9).abs() < 1e-9,
                "rotation survived"
            );
        }
    }
}
