//! `Document`'s path-editing command methods (`specs/path-node-editing/
//! adrs.md`, "the anchor schema, and the three merge choices inside it").
//! [`crate::path_model`] defines the pure data types a caller builds or
//! reads; [`crate::path_codec`] owns the Loro value shapes and keys these
//! methods write and read. This module is the command funnel itself —
//! `specs/path-node-editing/adrs.md`'s architect review note: "a
//! serializable `Command` enum is deferred to `undo-redo`; what is not
//! deferred is that each mutating method ends in exactly one Loro commit"
//! (ADR 0002 §9).

use loro::{LoroMap, LoroMovableList, TreeID, TreeParentId};

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    self, KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, adjacent_segment_indices, anchor_index,
    anchor_map_at, anchors_container, insert_anchor_at, insert_anchors_container,
    neighbour_tangent, node_exists, push_anchor, read_closed, read_kind, read_point, write_kind,
    write_path_fields, write_point, write_vec2,
};
use crate::path_model::{
    AnchorId, AnchorKind, HandleSlot, NewAnchor, NodeId, PathEditError, PathSnapshot,
};
use crate::units::{Point, Vec2};

/// Default handle length a corner→smooth conversion pulls out
/// (acceptance criterion 11; no acceptance criterion pins an exact value).
const DEFAULT_SMOOTH_HANDLE_LENGTH_MM: f64 = 10.0;

/// Default fraction of the segment chord a "make curve" extends its two
/// adjoining handles by (acceptance criterion 14; a standard Bézier
/// approximation fraction, not a value any acceptance criterion pins).
const DEFAULT_CURVE_HANDLE_FRACTION: f64 = 1.0 / 3.0;

impl Document {
    /// Commits a finished pen-tool session as one new path object
    /// (`specs/path-node-editing/adrs.md`, "a pen session is one commit";
    /// acceptance criteria 1, 2, 3, 5).
    ///
    /// # Panics
    /// Does not panic in practice: it only creates a root-level tree node
    /// and inserts known-valid keys and values into its freshly created
    /// meta map and movable list, none of which Loro's API can reject.
    #[must_use]
    pub fn create_path(&self, anchors: &[NewAnchor], closed: bool) -> NodeId {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: creating a root-level node on a freshly obtained tree
        // handle cannot fail.
        #[allow(clippy::unwrap_used)]
        let tree_id = tree.create(TreeParentId::Root).unwrap();
        // invariant: reading the meta map of a node this call just created
        // cannot fail.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id).unwrap();
        write_path_fields(&meta, closed);
        let anchor_list = insert_anchors_container(&meta);
        for anchor in anchors {
            push_anchor(&anchor_list, anchor);
        }
        self.commit_with_label("create_path");
        NodeId::from_parts(tree_id.peer, tree_id.counter)
    }

    /// Every object's identity (path or primitive alike), in sibling
    /// (z-)order (ADR 0002 §5) — never an array offset a caller may rely
    /// on staying stable. Renamed from `path_ids` in `primitive-shapes`
    /// (architect review): it always listed every root tree node, and
    /// now that some of those are primitives, the old name was
    /// misleading. [`Document::path`] and [`Document::primitive`] each
    /// filter the result to their own kind.
    #[must_use]
    pub fn object_ids(&self) -> Vec<NodeId> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        tree.roots()
            .into_iter()
            .map(|id| NodeId::from_parts(id.peer, id.counter))
            .collect()
    }

    /// Reads one path's full current data, or `None` if it no longer
    /// exists (deleted locally, or by a collaborator, since the caller
    /// last read a snapshot — ADR 0009 §2) **or is a primitive instead**
    /// (`specs/primitive-shapes/adrs.md`: "`Document::path(id)` returns
    /// `None` for a primitive node. It must not return an empty
    /// `PathSnapshot`, so slice 2's node tool and hit-testing never
    /// mistake a primitive for a path with no anchors.").
    #[must_use]
    pub fn path(&self, id: NodeId) -> Option<PathSnapshot> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(id);
        if !node_exists(&tree, tree_id) {
            return None;
        }
        let meta = tree.get_meta(tree_id).ok()?;
        if crate::shape_codec::read_shape_tag(&meta).is_some() {
            return None;
        }
        Some(path_codec::read_path_snapshot(id, &meta))
    }

    /// Moves every named anchor to its new absolute position, one commit
    /// for the whole drag — handles are untouched because they are stored
    /// relative to their own anchor (`specs/path-node-editing/adrs.md`
    /// decision 2; acceptance criteria 8, 10).
    ///
    /// Resolves every id to an index *before* writing any of them, so a
    /// refused move (one stale id anywhere in `moves`) never leaves the
    /// path half-moved.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] if `path` no longer exists;
    /// [`PathEditError::NoSuchAnchor`] if any named anchor no longer
    /// exists.
    pub fn move_anchors(
        &self,
        path: NodeId,
        moves: &[(AnchorId, Point)],
    ) -> Result<(), PathEditError> {
        let (_, anchors) = self.path_parts(path)?;
        let resolved: Vec<(usize, Point)> = moves
            .iter()
            .map(|&(anchor_id, point)| {
                anchor_index(&anchors, anchor_id).map(|index| (index, point))
            })
            .collect::<Result<_, _>>()?;
        for (index, point) in resolved {
            write_point(&anchor_map_at(&anchors, index), KEY_POINT, point);
        }
        self.commit_with_label("move_anchors");
        Ok(())
    }

    /// Sets one anchor's handle, mirroring the opposite handle when the
    /// anchor is [`AnchorKind::Smooth`] (`handle_in = -handle_out`) and
    /// touching only the named handle when it is
    /// [`AnchorKind::Corner`] (`specs/path-node-editing/adrs.md`
    /// decision 1; acceptance criterion 9).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path` or `anchor` no longer exists.
    pub fn set_handle(
        &self,
        path: NodeId,
        anchor: AnchorId,
        slot: HandleSlot,
        value: Vec2,
    ) -> Result<(), PathEditError> {
        let (_, anchors) = self.path_parts(path)?;
        let index = anchor_index(&anchors, anchor)?;
        let map = anchor_map_at(&anchors, index);
        let kind = read_kind(&map);
        let (own_key, mirror_key) = match slot {
            HandleSlot::In => (KEY_HANDLE_IN, KEY_HANDLE_OUT),
            HandleSlot::Out => (KEY_HANDLE_OUT, KEY_HANDLE_IN),
        };
        write_vec2(&map, own_key, value);
        if kind == AnchorKind::Smooth {
            write_vec2(&map, mirror_key, value.negated());
        }
        self.commit_with_label("set_handle");
        Ok(())
    }

    /// Converts every named anchor between [`AnchorKind::Corner`] and
    /// [`AnchorKind::Smooth`] as one commit — a multi-node selection's
    /// convert is one interaction, not `anchors.len()` of them
    /// (acceptance criterion 11).
    ///
    /// Every id is resolved to an index before any of them is written, so
    /// one stale id anywhere in `anchors` refuses the whole call rather
    /// than converting a prefix of it.
    ///
    /// Corner → smooth pulls out two handles of equal default length,
    /// collinear through the anchor along the chord between its
    /// neighbours. Smooth → corner leaves both handles exactly where they
    /// are and only flips `kind` — geometry cannot tell the two apart,
    /// which is exactly why `kind` is a stored field
    /// (`specs/path-node-editing/adrs.md` decision 3).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] if `path` no longer exists;
    /// [`PathEditError::NoSuchAnchor`] if any named anchor no longer
    /// exists.
    pub fn convert_anchor_kind(
        &self,
        path: NodeId,
        anchors_to_convert: &[AnchorId],
        kind: AnchorKind,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let closed = read_closed(&meta);
        let indices: Vec<usize> = anchors_to_convert
            .iter()
            .map(|&id| anchor_index(&anchors, id))
            .collect::<Result<_, _>>()?;
        for index in indices {
            let map = anchor_map_at(&anchors, index);
            if kind == AnchorKind::Smooth {
                let point = read_point(&map, KEY_POINT);
                let tangent = neighbour_tangent(&anchors, closed, index, point)
                    .normalized_to(DEFAULT_SMOOTH_HANDLE_LENGTH_MM);
                write_vec2(&map, KEY_HANDLE_OUT, tangent);
                write_vec2(&map, KEY_HANDLE_IN, tangent.negated());
            }
            write_kind(&map, kind);
        }
        self.commit_with_label("convert_anchor_kind");
        Ok(())
    }

    /// Inserts a new anchor right after `after`, carrying the caller-
    /// resolved subdivision geometry for the two adjoining handles
    /// (`specs/path-node-editing/adrs.md`, "commands carry resolved
    /// geometry, never geometric intent"; acceptance criterion 12).
    ///
    /// `prev_out` becomes `after`'s new `handle_out`; `next_in` becomes the
    /// following anchor's new `handle_in` (wrapping to the first anchor
    /// when the path is closed and `after` is its last anchor).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path` or `after` no longer exists.
    pub fn insert_anchor(
        &self,
        path: NodeId,
        after: AnchorId,
        new_anchor: NewAnchor,
        prev_out: Vec2,
        next_in: Vec2,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let closed = read_closed(&meta);
        let after_index = anchor_index(&anchors, after)?;
        write_vec2(
            &anchor_map_at(&anchors, after_index),
            KEY_HANDLE_OUT,
            prev_out,
        );

        let len = anchors.len();
        let next_existing_index = if after_index + 1 < len {
            Some(after_index + 1)
        } else if closed && len > 0 {
            Some(0)
        } else {
            None
        };
        if let Some(next_index) = next_existing_index {
            write_vec2(&anchor_map_at(&anchors, next_index), KEY_HANDLE_IN, next_in);
        }

        insert_anchor_at(&anchors, after_index + 1, &new_anchor);
        self.commit_with_label("insert_anchor");
        Ok(())
    }

    /// Removes the named anchors, or the entire path object if fewer than
    /// two anchors would remain (acceptance criterion 13).
    ///
    /// Anchor ids that no longer exist are silently ignored rather than
    /// refused: local selection can hold ids a collaborator has already
    /// deleted, and ADR 0009 §2 asks for that case to resolve lazily
    /// rather than error.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] if `path` no longer exists.
    ///
    /// # Panics
    /// Does not panic in practice: every index this deletes was just
    /// confirmed present in the same movable list.
    pub fn delete_anchors(&self, path: NodeId, ids: &[AnchorId]) -> Result<(), PathEditError> {
        let (_, anchors) = self.path_parts(path)?;
        let mut indices: Vec<usize> = ids
            .iter()
            .filter_map(|&id| anchor_index(&anchors, id).ok())
            .collect();
        indices.sort_unstable();
        indices.dedup();

        let remaining = anchors.len().saturating_sub(indices.len());
        if remaining < 2 {
            let tree = self.loro().get_tree(OBJECTS_TREE);
            let tree_id = tree_id_of(path);
            // invariant: `path_parts` above already confirmed this node
            // exists.
            #[allow(clippy::unwrap_used)]
            tree.delete(tree_id).unwrap();
            self.commit_with_label("delete_anchors");
            return Ok(());
        }

        // Delete from the highest index down so earlier indices stay valid.
        for index in indices.into_iter().rev() {
            // invariant: `index` was found in this same list just above.
            #[allow(clippy::unwrap_used)]
            anchors.delete(index, 1).unwrap();
        }
        self.commit_with_label("delete_anchors");
        Ok(())
    }

    /// Retracts a selected segment's two adjoining handles to the exact
    /// zero vector, making it a straight line (acceptance criterion 14).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path`, `start` or `end` no longer exist;
    /// [`PathEditError::NotAnAdjacentSegment`] if they are not two
    /// path-adjacent anchors.
    pub fn set_segment_line(
        &self,
        path: NodeId,
        start: AnchorId,
        end: AnchorId,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let closed = read_closed(&meta);
        let (start_index, end_index) = adjacent_segment_indices(&anchors, closed, start, end)?;
        write_vec2(
            &anchor_map_at(&anchors, start_index),
            KEY_HANDLE_OUT,
            Vec2::ZERO,
        );
        write_vec2(
            &anchor_map_at(&anchors, end_index),
            KEY_HANDLE_IN,
            Vec2::ZERO,
        );
        self.commit_with_label("set_segment_line");
        Ok(())
    }

    /// Extends a selected segment's two adjoining handles to a default
    /// fraction of the segment's chord, making it a curve, without moving
    /// either endpoint anchor (acceptance criterion 14).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path`, `start` or `end` no longer exist;
    /// [`PathEditError::NotAnAdjacentSegment`] if they are not two
    /// path-adjacent anchors.
    pub fn set_segment_curve(
        &self,
        path: NodeId,
        start: AnchorId,
        end: AnchorId,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let closed = read_closed(&meta);
        let (start_index, end_index) = adjacent_segment_indices(&anchors, closed, start, end)?;
        let start_map = anchor_map_at(&anchors, start_index);
        let end_map = anchor_map_at(&anchors, end_index);
        let chord = read_point(&start_map, KEY_POINT).vector_to(read_point(&end_map, KEY_POINT));
        write_vec2(
            &start_map,
            KEY_HANDLE_OUT,
            chord.scaled(DEFAULT_CURVE_HANDLE_FRACTION),
        );
        write_vec2(
            &end_map,
            KEY_HANDLE_IN,
            chord.scaled(-DEFAULT_CURVE_HANDLE_FRACTION),
        );
        self.commit_with_label("set_segment_curve");
        Ok(())
    }

    /// Looks up one path's meta map and anchors movable list together, so
    /// callers that need both (e.g. the `closed` flag for wraparound) read
    /// them from the same lookup.
    fn path_parts(&self, path: NodeId) -> Result<(LoroMap, LoroMovableList), PathEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(path);
        if !node_exists(&tree, tree_id) {
            return Err(PathEditError::NoSuchPath);
        }
        let meta = tree
            .get_meta(tree_id)
            .map_err(|_| PathEditError::NoSuchPath)?;
        let anchors = anchors_container(&meta);
        Ok((meta, anchors))
    }
}

fn tree_id_of(id: NodeId) -> TreeID {
    TreeID::new(id.peer, id.counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::Color;

    fn anchor(id: u64, x: f64, y: f64) -> NewAnchor {
        NewAnchor::corner(AnchorId::new(1, id), Point::new(x, y))
    }

    fn smooth_anchor(id: u64, x: f64, y: f64, handle_out: Vec2) -> NewAnchor {
        NewAnchor {
            id: AnchorId::new(1, id),
            point: Point::new(x, y),
            handle_in: handle_out.negated(),
            handle_out,
            kind: AnchorKind::Smooth,
        }
    }

    #[test]
    fn create_path_round_trips_a_two_node_open_path() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let snapshot = document.path(id).expect("path exists");
        assert!(!snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 2);
        assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0));
        assert_eq!(snapshot.anchors[1].point, Point::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Corner);
    }

    #[test]
    fn created_path_has_the_placeholder_style() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 1.0)], false);
        let snapshot = document.path(id).expect("path exists");
        assert!((snapshot.stroke_width.as_mm() - 0.25).abs() < f64::EPSILON);
        assert_eq!(snapshot.stroke, Color::BLACK);
        assert_eq!(snapshot.fill, None);
    }

    #[test]
    fn object_ids_lists_every_created_object_in_order() {
        let document = Document::new(1);
        let first = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        let second = document.create_path(&[anchor(3, 0.0, 0.0), anchor(4, 1.0, 0.0)], false);
        assert_eq!(document.object_ids(), vec![first, second]);
    }

    #[test]
    fn path_returns_none_for_an_unknown_id() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        document
            .delete_anchors(id, &[AnchorId::new(1, 1), AnchorId::new(1, 2)])
            .expect("delete");
        assert_eq!(document.path(id), None);
    }

    #[test]
    fn move_anchors_moves_the_point_but_not_the_handles() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(5.0, 0.0)),
                anchor(2, 10.0, 0.0),
            ],
            false,
        );
        document
            .move_anchors(id, &[(AnchorId::new(1, 1), Point::new(3.0, 4.0))])
            .expect("move");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(3.0, 4.0));
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(5.0, 0.0));
    }

    #[test]
    fn move_anchors_on_a_stale_anchor_is_refused() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        let result = document.move_anchors(id, &[(AnchorId::new(9, 9), Point::new(0.0, 0.0))]);
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
    }

    /// A refused batch move must not be half-applied: every id is
    /// resolved before any point is written, so a valid id earlier in the
    /// list stays untouched when a later id in the same call is stale.
    #[test]
    fn move_anchors_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let result = document.move_anchors(
            id,
            &[
                (AnchorId::new(1, 1), Point::new(99.0, 99.0)),
                (AnchorId::new(9, 9), Point::new(0.0, 0.0)),
            ],
        );
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(
            snapshot.anchors[0].point,
            Point::new(0.0, 0.0),
            "the valid id earlier in the batch must not have moved either"
        );
    }

    #[test]
    fn set_handle_on_a_smooth_anchor_mirrors_the_opposite_handle() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(5.0, 0.0)),
                anchor(2, 10.0, 0.0),
            ],
            false,
        );
        document
            .set_handle(
                id,
                AnchorId::new(1, 1),
                HandleSlot::Out,
                Vec2::new(3.0, 4.0),
            )
            .expect("set handle");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-3.0, -4.0));
    }

    #[test]
    fn set_handle_on_a_corner_anchor_touches_only_that_handle() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        document
            .set_handle(
                id,
                AnchorId::new(1, 1),
                HandleSlot::Out,
                Vec2::new(3.0, 4.0),
            )
            .expect("set handle");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::ZERO);
    }

    #[test]
    fn convert_corner_to_smooth_pulls_out_mirrored_handles_along_the_chord() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Smooth)
            .expect("convert");
        let snapshot = document.path(id).expect("path exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Smooth);
        assert_eq!(middle.handle_in, middle.handle_out.negated());
        assert!((middle.handle_out.length() - DEFAULT_SMOOTH_HANDLE_LENGTH_MM).abs() < 1e-9);
        // Tangent points toward the next neighbour (+X here).
        assert!(middle.handle_out.x > 0.0);
    }

    #[test]
    fn convert_smooth_to_corner_leaves_handles_exactly_where_they_are() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(5.0, 0.0)),
                anchor(2, 10.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 1)], AnchorKind::Corner)
            .expect("convert");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Corner);
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(5.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-5.0, 0.0));

        // And now the two handles move independently, per criterion 9.
        document
            .set_handle(
                id,
                AnchorId::new(1, 1),
                HandleSlot::Out,
                Vec2::new(1.0, 1.0),
            )
            .expect("set handle");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(1.0, 1.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-5.0, 0.0));
    }

    #[test]
    fn convert_anchor_kind_converts_a_whole_multi_selection_at_once() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(
                id,
                &[AnchorId::new(1, 1), AnchorId::new(1, 3)],
                AnchorKind::Smooth,
            )
            .expect("convert");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Smooth);
        assert_eq!(snapshot.anchors[2].kind, AnchorKind::Smooth);
        assert_eq!(
            snapshot.anchors[1].kind,
            AnchorKind::Corner,
            "the anchor not named in the slice is untouched"
        );
    }

    #[test]
    fn convert_anchor_kind_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        let result = document.convert_anchor_kind(
            id,
            &[AnchorId::new(1, 1), AnchorId::new(9, 9)],
            AnchorKind::Smooth,
        );
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(
            snapshot.anchors[0].kind,
            AnchorKind::Corner,
            "the valid id in the batch must not have been converted either"
        );
    }

    #[test]
    fn insert_anchor_splits_a_segment_with_caller_resolved_geometry() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 20.0, 0.0)], false);
        document
            .insert_anchor(
                id,
                AnchorId::new(1, 1),
                anchor(3, 10.0, 0.0),
                Vec2::new(2.0, 0.0),
                Vec2::new(-2.0, 0.0),
            )
            .expect("insert");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors.len(), 3);
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(2.0, 0.0));
        assert_eq!(snapshot.anchors[1].point, Point::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[2].handle_in, Vec2::new(-2.0, 0.0));
    }

    #[test]
    fn insert_anchor_on_a_closed_path_wraps_to_the_first_anchor() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 5.0, 10.0),
            ],
            true,
        );
        document
            .insert_anchor(
                id,
                AnchorId::new(1, 3),
                anchor(4, 2.5, 5.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(-1.0, -1.0),
            )
            .expect("insert");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors.len(), 4);
        // The wrapped-to anchor is the first one in traversal order.
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-1.0, -1.0));
    }

    #[test]
    fn delete_anchors_joins_the_remaining_neighbours() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .delete_anchors(id, &[AnchorId::new(1, 2)])
            .expect("delete");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors.len(), 2);
        assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0));
        assert_eq!(snapshot.anchors[1].point, Point::new(20.0, 0.0));
    }

    #[test]
    fn delete_anchors_removes_the_whole_path_below_two_anchors() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        document
            .delete_anchors(id, &[AnchorId::new(1, 1)])
            .expect("delete");
        assert_eq!(document.path(id), None);
        assert_eq!(document.object_ids(), Vec::new());
    }

    #[test]
    fn delete_anchors_ignores_already_stale_ids() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .delete_anchors(id, &[AnchorId::new(1, 2), AnchorId::new(9, 9)])
            .expect("delete tolerates a stale id alongside a real one");
        assert_eq!(document.path(id).expect("still exists").anchors.len(), 2);
    }

    #[test]
    fn set_segment_line_retracts_both_adjoining_handles() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(3.0, 0.0)),
                smooth_anchor(2, 10.0, 0.0, Vec2::new(3.0, 0.0)),
            ],
            false,
        );
        document
            .set_segment_line(id, AnchorId::new(1, 1), AnchorId::new(1, 2))
            .expect("make line");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::ZERO);
        assert_eq!(snapshot.anchors[1].handle_in, Vec2::ZERO);
        // The far handle, not part of this segment, is untouched.
        assert_eq!(snapshot.anchors[1].handle_out, Vec2::new(3.0, 0.0));
    }

    #[test]
    fn set_segment_curve_extends_both_adjoining_handles_along_the_chord() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 30.0, 0.0)], false);
        document
            .set_segment_curve(id, AnchorId::new(1, 1), AnchorId::new(1, 2))
            .expect("make curve");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0));
        assert_eq!(snapshot.anchors[1].point, Point::new(30.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[1].handle_in, Vec2::new(-10.0, 0.0));
    }

    #[test]
    fn segment_ops_refuse_non_adjacent_anchors() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        let result = document.set_segment_line(id, AnchorId::new(1, 1), AnchorId::new(1, 3));
        assert_eq!(result, Err(PathEditError::NotAnAdjacentSegment));
    }

    #[test]
    fn segment_ops_on_a_closed_path_see_the_wraparound_segment() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 5.0, 10.0),
            ],
            true,
        );
        document
            .set_segment_curve(id, AnchorId::new(1, 3), AnchorId::new(1, 1))
            .expect("the wraparound segment is adjacent on a closed path");
        let snapshot = document.path(id).expect("path exists");
        assert_ne!(snapshot.anchors[2].handle_out, Vec2::ZERO);
        assert_ne!(snapshot.anchors[0].handle_in, Vec2::ZERO);
    }

    #[test]
    fn operations_on_an_unknown_path_are_refused() {
        let document = Document::new(1);
        let other = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        document
            .delete_anchors(other, &[AnchorId::new(1, 1), AnchorId::new(1, 2)])
            .expect("delete");
        let unknown = other;
        assert_eq!(
            document.move_anchors(unknown, &[]),
            Err(PathEditError::NoSuchPath)
        );
    }
}
