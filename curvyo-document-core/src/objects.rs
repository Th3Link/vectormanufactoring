//! `Document`'s general "any object" command methods
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`: "moving and
//! deleting objects writes to the document. Both are settled below, and
//! both reuse registers that already exist"). Unlike [`crate::paths`] and
//! [`crate::shapes`], these two commands act identically regardless of an
//! object's kind — a move never reads a path's curvature, and a delete
//! never reads geometry at all — so they live in their own module rather
//! than being added as a third kind-specific method set.

use std::collections::HashSet;

use loro::{Container, TreeID, ValueOrContainer};

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    self, KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_ID, KEY_POINT, anchor_map_at, node_exists,
    write_point, write_vec2,
};
use crate::path_model::{AnchorId, NodeId};
use crate::primitive_model::{ObjectSnapshot, translate_shape};
use crate::shape_codec;
use crate::subpath_codec::{all_anchor_lists, anchor_positions, total_anchor_count};
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
    /// A path source of [`Document::duplicate_objects`] came with a number of
    /// fresh [`AnchorId`]s that differs from its anchor count (a primitive
    /// takes none).
    #[error("the fresh anchor ids do not match the source's anchor count")]
    AnchorIds,
    /// The base object of [`Document::replace_with_path`] is not among the
    /// objects it replaces.
    #[error("the base object is not one of the replaced objects")]
    BaseNotAnOperand,
    /// [`Document::replace_with_path`] was given no outline, or an outline
    /// without an anchor.
    #[error("a replacement path needs at least one outline, each with an anchor")]
    NoOutlines,
    /// A stroke width given to [`Document::transform_objects`] is not finite
    /// and above zero: nothing was written.
    #[error("a stroke width must be finite and above zero")]
    InvalidStrokeWidth,
    /// A corner radius given to [`Document::transform_objects`] is not finite:
    /// nothing was written.
    #[error("a corner radius must be finite")]
    InvalidRadius,
    /// A position, size, handle or rotation given to
    /// [`Document::transform_objects`] is not finite: nothing was written.
    #[error("every coordinate and angle must be finite")]
    NonFiniteGeometry,
    /// A path result given to [`Document::transform_objects`] for an object that
    /// is still a primitive is not one closed outline of at least two anchors with
    /// different ids: nothing was written.
    #[error("a converted shape must be one closed outline of two or more anchors")]
    InvalidConversion,
}

/// One object to duplicate with [`Document::duplicate_objects`], and the
/// fresh anchor ids its copy takes: a path needs exactly one per anchor of all its
/// outlines, in anchor order (the first outline first); a primitive needs none. The caller mints them (this crate
/// never does): an anchor id names a node across the whole document, so a copy
/// that shared its original's ids would put two equal ids into one path the
/// first time a maker joined one to the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopySource {
    /// The object to copy.
    pub id: NodeId,
    /// The copy's anchor ids, one per anchor of a path source; empty for a
    /// primitive.
    pub anchor_ids: Vec<AnchorId>,
}

fn tree_id_of(id: NodeId) -> TreeID {
    TreeID::new(id.peer, id.counter)
}

/// Deduplicates `ids`, keeping each one's first occurrence's position.
/// Both [`Document::translate_objects`] and [`Document::delete_objects`]
/// are public APIs on this crate's own `Document` — not reachable with a
/// duplicate id via the shipped UI today (`curvyo-ui-core`'s own
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
            translate_meta(meta, offset);
        }
        self.commit_with_label("translate_objects");
        Ok(())
    }

    /// Copies every named object, displaced by `offset`, in **one commit**
    /// (`specs/0010-edit-interaction-polish/adrs.md`, decision 2), and returns the
    /// new ids in source order. The copy takes **every key of its source's
    /// meta map** as it is (a register a later story adds, or one a newer
    /// build wrote that this build does not know, comes along with no change
    /// here) and, for a path, every key of every anchor map except the id,
    /// which is the caller's fresh one from [`CopySource::anchor_ids`]. It
    /// sits directly above its own original in z-order, so a selection of
    /// several keeps its relative order (A, B becomes A, A', B, B'). The
    /// displacement is the one [`Document::translate_objects`] writes, so a
    /// copy lies exactly where the blue outline of its preview was.
    ///
    /// A copy is a new tree node with no link to its original: a peer's
    /// concurrent edit of the original does not touch it. A zero `offset` is
    /// the caller's refusal to make, not this function's.
    ///
    /// Every source is resolved and checked before the first write, so one
    /// stale id or one wrong id count refuses the whole call and writes
    /// nothing. A source named twice is copied once.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if a source no longer exists;
    /// [`ObjectEditError::AnchorIds`] if a path source's `anchor_ids` are not
    /// exactly one per anchor and all different, or a primitive source has
    /// any.
    ///
    /// # Panics
    /// Does not panic in practice: every write below goes to a tree node and
    /// meta map this call just created, or to one it just resolved.
    pub fn duplicate_objects(
        &self,
        sources: &[CopySource],
        offset: Vec2,
    ) -> Result<Vec<NodeId>, ObjectEditError> {
        let mut seen = HashSet::with_capacity(sources.len());
        let sources: Vec<&CopySource> = sources.iter().filter(|s| seen.insert(s.id)).collect();
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let metas: Vec<loro::LoroMap> = sources
            .iter()
            .map(|source| {
                let tree_id = tree_id_of(source.id);
                if !node_exists(&tree, tree_id) {
                    return Err(ObjectEditError::NoSuchObject);
                }
                let meta = tree
                    .get_meta(tree_id)
                    .map_err(|_| ObjectEditError::NoSuchObject)?;
                check_anchor_ids(&meta, &source.anchor_ids)?;
                Ok(meta)
            })
            .collect::<Result<_, _>>()?;

        // `mov_after` needs the fractional-index feature on for this tree;
        // enabled on every call, as `split_at_anchor` does.
        tree.enable_fractional_index(0);
        let mut created = Vec::with_capacity(sources.len());
        for (source, source_meta) in sources.iter().zip(&metas) {
            // invariant: a root-level create on a live tree cannot fail, and
            // reading the meta map of the node just created cannot fail.
            #[allow(clippy::unwrap_used)]
            let new_tree_id = tree.create(loro::TreeParentId::Root).unwrap();
            #[allow(clippy::unwrap_used)]
            let meta = tree.get_meta(new_tree_id).unwrap();
            copy_map(source_meta, &meta);
            if shape_codec::read_shape_tag(&meta).is_none() {
                renumber_anchors(&meta, &source.anchor_ids);
            }
            translate_meta(&meta, offset);
            // invariant: the source was resolved above and the new node was
            // just created: both are live root siblings.
            #[allow(clippy::unwrap_used)]
            tree.mov_after(new_tree_id, tree_id_of(source.id)).unwrap();
            created.push(NodeId::from_parts(new_tree_id.peer, new_tree_id.counter));
        }
        self.commit_with_label("duplicate_objects");
        Ok(created)
    }

    /// Writes a resolved rotation — the object after
    /// [`ObjectSnapshot::rotated`] — in **one commit**
    /// (`specs/0005-object-transform/specification.md` acceptance
    /// criteria 15-18, 20): a primitive's frame (only if the rotation
    /// moved it, i.e. a pivot off its center) and `rotation`; a path's
    /// anchor points and handles and `rotation`. `curvyo-ui-core`
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
                let positions = anchor_positions(&meta);
                let resolved: Vec<_> = path
                    .all_anchors()
                    .map(|a| {
                        positions
                            .get(a.id)
                            .map(|(list, index)| (list, index, a))
                            .ok_or(ObjectEditError::NoSuchObject)
                    })
                    .collect::<Result<_, _>>()?;
                for (list, index, anchor) in resolved {
                    let map = anchor_map_at(&list, index);
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

/// Checks a copy source's fresh anchor ids against its meta map: a primitive
/// takes none, a path exactly one per anchor, all different.
fn check_anchor_ids(meta: &loro::LoroMap, anchor_ids: &[AnchorId]) -> Result<(), ObjectEditError> {
    let expected = if shape_codec::read_shape_tag(meta).is_some() {
        0
    } else {
        total_anchor_count(meta)
    };
    let distinct = anchor_ids.iter().collect::<HashSet<_>>().len();
    if anchor_ids.len() == expected && distinct == expected {
        Ok(())
    } else {
        Err(ObjectEditError::AnchorIds)
    }
}

/// Copies every key of `from` into `into` as it is: a plain value as the same
/// value, a nested map or movable list (a path's anchors) entry by entry. No
/// key is named here, so a register added later is copied with no change.
fn copy_map(from: &loro::LoroMap, into: &loro::LoroMap) {
    for key in from.keys() {
        let Some(value) = from.get(&key) else {
            continue;
        };
        // invariant: every insert below goes to a map that is attached to the
        // document and takes a fresh key or an own container.
        #[allow(clippy::unwrap_used)]
        match &value {
            ValueOrContainer::Value(plain) => into.insert(&key, plain.clone()).unwrap(),
            ValueOrContainer::Container(Container::Map(map)) => {
                let nested = into.insert_container(&key, loro::LoroMap::new()).unwrap();
                copy_map(map, &nested);
            }
            ValueOrContainer::Container(Container::MovableList(list)) => {
                let nested = into
                    .insert_container(&key, loro::LoroMovableList::new())
                    .unwrap();
                for index in 0..list.len() {
                    match list.get(index) {
                        Some(ValueOrContainer::Container(Container::Map(map))) => {
                            let element = nested.push_container(loro::LoroMap::new()).unwrap();
                            copy_map(&map, &element);
                        }
                        Some(ValueOrContainer::Value(plain)) => nested.push(plain).unwrap(),
                        _ => {}
                    }
                }
            }
            // No other container kind is stored today; its plain value keeps
            // the data, and the deep-value test of a copy would show the loss.
            ValueOrContainer::Container(_) => {
                into.insert(&key, value.get_deep_value()).unwrap();
            }
        }
    }
}

/// Gives a freshly copied path its own anchor ids, one per anchor in order,
/// over all its outlines (the first outline first).
fn renumber_anchors(meta: &loro::LoroMap, anchor_ids: &[AnchorId]) {
    let mut ids = anchor_ids.iter();
    for list in all_anchor_lists(meta) {
        for index in 0..list.len() {
            let Some(id) = ids.next() else {
                return;
            };
            let map = anchor_map_at(&list, index);
            // invariant: the anchor map is attached and `id` is a plain string.
            #[allow(clippy::unwrap_used)]
            map.insert(KEY_ID, path_codec::anchor_id_to_value(*id))
                .unwrap();
        }
    }
}

/// Shifts one object by `offset`, whichever kind it is: the one place that
/// knows how each kind stores its position. [`Document::translate_objects`],
/// [`Document::duplicate_objects`], [`Document::resize`] and
/// [`Document::fit_to_content`] all move objects through it. Writes nothing
/// to the commit log by itself; the caller commits.
pub(crate) fn translate_meta(meta: &loro::LoroMap, offset: Vec2) {
    match shape_codec::read_shape_tag(meta) {
        Some(tag) => translate_primitive_meta(meta, &tag, offset),
        None => translate_path_meta(meta, offset),
    }
}

/// Shifts every one of a path's anchor `point`s, in every outline, by
/// `offset`, leaving handles (relative to their own anchor) untouched.
fn translate_path_meta(meta: &loro::LoroMap, offset: Vec2) {
    for anchors in all_anchor_lists(meta) {
        for index in 0..anchors.len() {
            let map = path_codec::anchor_map_at(&anchors, index);
            let point = path_codec::read_point(&map, KEY_POINT);
            path_codec::write_point(&map, KEY_POINT, point.translated(offset));
        }
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
    use crate::corner_radii::CornerRadii;
    use crate::path_model::{AnchorId, NewAnchor};
    use crate::primitive_model::Shape;
    use crate::units::{Angle, Length, Point};
    use loro::ToJson;

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

    /// Decision 2: the copy is a copy of the meta map, so it holds every key,
    /// including one this build does not know. A zero offset makes the two
    /// deep values equal apart from the anchor ids.
    #[test]
    fn a_copy_holds_every_key_of_the_original_including_an_unknown_one() {
        let document = Document::new(1);
        let path = two_node_path(&document);
        let rect = document.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(1.0, 2.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(4.0),
        });
        let tree = document.loro().get_tree(OBJECTS_TREE);
        for id in [path, rect] {
            let meta = tree.get_meta(tree_id_of(id)).expect("meta");
            meta.insert("future_register", 42_i64).expect("insert");
        }
        let copies = document
            .duplicate_objects(
                &[
                    CopySource {
                        id: path,
                        anchor_ids: vec![AnchorId::new(9, 1), AnchorId::new(9, 2)],
                    },
                    CopySource {
                        id: rect,
                        anchor_ids: vec![],
                    },
                ],
                Vec2::ZERO,
            )
            .expect("duplicate");
        for (original, copy) in [path, rect].into_iter().zip(copies) {
            let deep = |id: NodeId| {
                let mut value = tree
                    .get_meta(tree_id_of(id))
                    .expect("meta")
                    .get_deep_value()
                    .to_json_value();
                if let Some(anchors) = value.get_mut("anchors").and_then(|a| a.as_array_mut()) {
                    for anchor in anchors {
                        anchor["id"] = serde_json::Value::Null;
                    }
                }
                value
            };
            assert_eq!(deep(original), deep(copy));
            assert_eq!(deep(copy)["future_register"], 42);
        }
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
            CornerRadii::uniform(Length::from_mm(0.0)),
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
