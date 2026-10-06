//! Join and Split — the two path-restructuring commands
//! (`specs/0006-path-merge-split-and-node-types/adrs.md`, "Join and
//! Split are anchor-list surgery on the existing schema: no new
//! register, no curve evaluation"). Split out of [`crate::paths`]
//! (architect review: that module had grown past `CLAUDE.md` §5's
//! ~500-line guidance with no reason given) — this module owns exactly
//! the two topology-changing operations and the helpers only they need;
//! [`crate::paths`] keeps the per-anchor/per-path field edits plus the
//! `path_parts`/`create_path_uncommitted`/`tree_id_of` helpers both
//! modules call, now `pub(crate)` for that reason.

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, anchor_map_at, insert_anchor_at, write_closed,
    write_kind, write_point, write_vec2,
};
use crate::path_model::{AnchorId, AnchorKind, NewAnchor, NodeId, PathEditError, PathSnapshot};
use crate::units::{Point, Vec2};

/// One of [`Document::split_at_anchor`]'s two resulting coincident
/// anchors: which path it ended up on, and its own id. A named alias
/// rather than the bare nested tuple at each of its three call sites
/// (`clippy::type_complexity`).
type SplitEndpoint = (NodeId, AnchorId);

/// What `Document::resolve_join` found: the one-path case (acceptance
/// criterion 10) or the two-object case (criterion 9), each carrying
/// everything `join_endpoints` needs to act and nothing `check_join`
/// needs to re-derive.
enum JoinPlan {
    SamePath {
        path: NodeId,
        snapshot: PathSnapshot,
    },
    TwoObjects {
        a_path: NodeId,
        a_snapshot: PathSnapshot,
        a_index: usize,
        b_path: NodeId,
        b_snapshot: PathSnapshot,
        b_index: usize,
    },
}

impl Document {
    /// Merges two selected endpoint nodes into one
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`, "Join is
    /// `Document::join_endpoints`"; acceptance criteria 8, 9, 10, 11).
    /// Works identically whether `a_path`/`b_path` name the same path
    /// (criterion 10: the one path's own two endpoints close it into a
    /// loop) or two different path objects (criterion 9: the second
    /// object is deleted and its content folds into the first) — the
    /// document model makes no distinction between the two; only the
    /// maker's selection mechanism decides which call a given gesture
    /// becomes (the cross-object case needs `canvas-navigation-and-
    /// selection`'s `ObjectSelection` to select two path objects first,
    /// which is not wired up to this command yet; the same-path case
    /// needs only `path-node-editing`'s existing Node tool).
    ///
    /// `a_anchor`/`b_anchor` are in **selection order** (`a` = first-
    /// selected, `b` = second-selected) — load-bearing only for the
    /// cross-object case: `a`'s path survives with its own node order
    /// never reversed; `b`'s path is appended/prepended, reversed only
    /// when needed to meet at the junction (criterion 9's four bullet
    /// points). The one-path case's result does not depend on which
    /// endpoint is named `a` or which `b`.
    ///
    /// Refuses (writes nothing) unless both anchors resolve, both are
    /// the first-or-last anchor of an *open* path, they are not the same
    /// anchor, and — for two ends of the same path — that path has more
    /// than two anchors (joining a 2-anchor open path would produce a
    /// 1-anchor closed path `vecmanf-render-core::stroke` cannot draw).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// either path or anchor no longer exists;
    /// [`PathEditError::NotJoinable`] if the selection does not qualify,
    /// per the refusal conditions above.
    pub fn join_endpoints(
        &self,
        a_path: NodeId,
        a_anchor: AnchorId,
        b_path: NodeId,
        b_anchor: AnchorId,
    ) -> Result<(NodeId, AnchorId), PathEditError> {
        match self.resolve_join(a_path, a_anchor, b_path, b_anchor)? {
            JoinPlan::SamePath { path, snapshot } => self.join_same_path(path, &snapshot),
            JoinPlan::TwoObjects {
                a_path,
                a_snapshot,
                a_index,
                b_path,
                b_snapshot,
                b_index,
            } => self.join_two_objects(a_path, &a_snapshot, a_index, b_path, &b_snapshot, b_index),
        }
    }

    /// Whether [`Document::join_endpoints`] would succeed for this
    /// selection right now — read-only, no commit. The one place
    /// `vecmanf-ui-core`'s toolbar-enablement state (`NodeTool::
    /// can_join`) checks this, rather than independently re-deriving the
    /// refusal rule itself (acceptance criterion 8) — the same "one
    /// rule, one place" principle `resolve_handle_pair` already follows
    /// for the handle-drag rule, now extended to this refusal rule.
    #[must_use]
    pub fn check_join(
        &self,
        a_path: NodeId,
        a_anchor: AnchorId,
        b_path: NodeId,
        b_anchor: AnchorId,
    ) -> bool {
        self.resolve_join(a_path, a_anchor, b_path, b_anchor)
            .is_ok()
    }

    /// Resolves and validates a [`Document::join_endpoints`] call
    /// without writing anything — shared by `join_endpoints` itself
    /// (which also needs the resolved snapshots to act on) and
    /// [`Document::check_join`] (which only needs to know whether this
    /// succeeds), so the refusal rule exists in exactly one place.
    fn resolve_join(
        &self,
        a_path: NodeId,
        a_anchor: AnchorId,
        b_path: NodeId,
        b_anchor: AnchorId,
    ) -> Result<JoinPlan, PathEditError> {
        let a_snapshot = self.path(a_path).ok_or(PathEditError::NoSuchPath)?;
        let a_index = index_of(&a_snapshot, a_anchor)?;
        if a_snapshot.closed || !is_endpoint(&a_snapshot, a_index) {
            return Err(PathEditError::NotJoinable);
        }

        if a_path == b_path {
            if a_anchor == b_anchor {
                return Err(PathEditError::NotJoinable);
            }
            let b_index = index_of(&a_snapshot, b_anchor)?;
            if !is_endpoint(&a_snapshot, b_index) {
                return Err(PathEditError::NotJoinable);
            }
            if a_snapshot.anchors.len() <= 2 {
                return Err(PathEditError::NotJoinable);
            }
            return Ok(JoinPlan::SamePath {
                path: a_path,
                snapshot: a_snapshot,
            });
        }

        let b_snapshot = self.path(b_path).ok_or(PathEditError::NoSuchPath)?;
        let b_index = index_of(&b_snapshot, b_anchor)?;
        if b_snapshot.closed || !is_endpoint(&b_snapshot, b_index) {
            return Err(PathEditError::NotJoinable);
        }

        Ok(JoinPlan::TwoObjects {
            a_path,
            a_snapshot,
            a_index,
            b_path,
            b_snapshot,
            b_index,
        })
    }

    /// The one-path half of [`Document::join_endpoints`] (acceptance
    /// criterion 10): `anchors[0]` survives (keeps its id) at list
    /// position 0, `anchors[len - 1]` is removed, `closed` becomes
    /// `true`. Same midpoint/handle/Corner-kind rule as the two-object
    /// case — `anchors[0]`'s own interior-facing handle (`handle_out`,
    /// since it is the path's first anchor) is already correct and left
    /// untouched; only `point`, `handle_in` (transplanted from the
    /// removed anchor's own interior-facing `handle_in`) and `kind` are
    /// written.
    fn join_same_path(
        &self,
        path: NodeId,
        snapshot: &PathSnapshot,
    ) -> Result<(NodeId, AnchorId), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let len = snapshot.anchors.len();
        let first = &snapshot.anchors[0];
        let last = &snapshot.anchors[len - 1];
        let map = anchor_map_at(&anchors, 0);
        write_point(&map, KEY_POINT, midpoint_of(first.point, last.point));
        write_vec2(&map, KEY_HANDLE_IN, last.handle_in);
        write_kind(&map, AnchorKind::Corner);
        // invariant: `len` was just read from this same (just-resolved)
        // snapshot, so `len - 1` is this list's own last valid index.
        #[allow(clippy::unwrap_used)]
        anchors.delete(len - 1, 1).unwrap();
        write_closed(&meta, true);
        self.commit_with_label("join_endpoints");
        Ok((path, first.id))
    }

    /// The two-object half of [`Document::join_endpoints`] (acceptance
    /// criterion 9). `a_path` survives with its own `NodeId` and every
    /// non-anchor register; `b_path`'s tree node is deleted outright.
    /// `b`'s remaining anchors (every one except the join anchor itself)
    /// are copied — keeping their own `AnchorId`s, per `specs/0006-path-
    /// merge-split-and-node-types/adrs.md`'s "moved anchors keep their
    /// `AnchorId`s (globally unique), so selection survives" — into
    /// `a_path`'s own movable list, reversed (and each one's handles
    /// swapped) exactly when `a` and `b` are the *same* kind of end
    /// (both first, or both last), so the two selected nodes land
    /// adjacent at the junction regardless of which combination of ends
    /// was selected.
    fn join_two_objects(
        &self,
        a_path: NodeId,
        a_snapshot: &PathSnapshot,
        a_index: usize,
        b_path: NodeId,
        b_snapshot: &PathSnapshot,
        b_index: usize,
    ) -> Result<(NodeId, AnchorId), PathEditError> {
        let a_anchor = &a_snapshot.anchors[a_index];
        let b_anchor = &b_snapshot.anchors[b_index];
        let a_is_last = a_index == a_snapshot.anchors.len() - 1;
        let b_is_last = b_index == b_snapshot.anchors.len() - 1;
        let midpoint = midpoint_of(a_anchor.point, b_anchor.point);

        // Each side's own interior-facing handle, read *before* any
        // reversal — a structural fact of being a first- or last-anchor,
        // unaffected by how the list is later reordered.
        let a_interior = if a_is_last {
            a_anchor.handle_in
        } else {
            a_anchor.handle_out
        };
        let b_interior = if b_is_last {
            b_anchor.handle_in
        } else {
            b_anchor.handle_out
        };

        let mut rest: Vec<NewAnchor> = b_snapshot
            .anchors
            .iter()
            .enumerate()
            .filter(|&(index, _)| index != b_index)
            .map(|(_, anchor)| *anchor)
            .collect();
        if a_is_last == b_is_last {
            rest.reverse();
            for anchor in &mut rest {
                std::mem::swap(&mut anchor.handle_in, &mut anchor.handle_out);
            }
        }

        let (_, a_anchors) = self.path_parts(a_path)?;
        let a_map = anchor_map_at(&a_anchors, a_index);
        write_point(&a_map, KEY_POINT, midpoint);
        write_kind(&a_map, AnchorKind::Corner);
        if a_is_last {
            write_vec2(&a_map, KEY_HANDLE_IN, a_interior);
            write_vec2(&a_map, KEY_HANDLE_OUT, b_interior);
            for (offset, anchor) in rest.iter().enumerate() {
                insert_anchor_at(&a_anchors, a_index + 1 + offset, anchor);
            }
        } else {
            write_vec2(&a_map, KEY_HANDLE_OUT, a_interior);
            write_vec2(&a_map, KEY_HANDLE_IN, b_interior);
            for (offset, anchor) in rest.iter().enumerate() {
                insert_anchor_at(&a_anchors, offset, anchor);
            }
        }

        let tree = self.loro().get_tree(OBJECTS_TREE);
        let b_tree_id = crate::paths::tree_id_of(b_path);
        // invariant: `self.path(b_path)` resolved this same node moments
        // ago in `join_endpoints`.
        #[allow(clippy::unwrap_used)]
        tree.delete(b_tree_id).unwrap();

        self.commit_with_label("join_endpoints");
        Ok((a_path, a_anchor.id))
    }

    /// Breaks a path apart at one selected node (`specs/0006-path-merge-
    /// split-and-node-types/adrs.md`, "Split is `Document::
    /// split_at_anchor`, Join's inverse"; acceptance criteria 12-15).
    ///
    /// `new_id` is a fresh [`AnchorId`] the caller mints
    /// (`vecmanf-ui-core`'s `AnchorIdMinter`) for the second of the two
    /// resulting coincident copies — this crate never mints ids itself
    /// (`CLAUDE.md` §6).
    ///
    /// Refuses (writes nothing) unless `anchor` resolves and is either an
    /// interior node of an open path or any node of a closed path — the
    /// first or last anchor of an open path has nothing on one side to
    /// split off (acceptance criterion 12).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path` or `anchor` no longer exists;
    /// [`PathEditError::NotSplittable`] if the selection does not
    /// qualify.
    pub fn split_at_anchor(
        &self,
        path: NodeId,
        anchor: AnchorId,
        new_id: AnchorId,
    ) -> Result<(SplitEndpoint, SplitEndpoint), PathEditError> {
        let (snapshot, index) = self.resolve_split(path, anchor)?;
        if snapshot.closed {
            self.split_closed_path(path, &snapshot, index, new_id)
        } else {
            self.split_open_path(path, &snapshot, index, new_id)
        }
    }

    /// Whether [`Document::split_at_anchor`] would succeed for this
    /// selection right now — read-only, no commit, `new_id` not even
    /// needed since no id is minted here. The one place `vecmanf-ui-
    /// core`'s toolbar-enablement state (`NodeTool::can_split`) checks
    /// this, rather than independently re-deriving the refusal rule
    /// (acceptance criterion 12) — see [`Document::check_join`]'s own
    /// doc comment for the same reasoning.
    #[must_use]
    pub fn check_split(&self, path: NodeId, anchor: AnchorId) -> bool {
        self.resolve_split(path, anchor).is_ok()
    }

    /// Resolves and validates a [`Document::split_at_anchor`] call
    /// without writing anything — shared by `split_at_anchor` itself and
    /// [`Document::check_split`], so the refusal rule exists in exactly
    /// one place.
    fn resolve_split(
        &self,
        path: NodeId,
        anchor: AnchorId,
    ) -> Result<(PathSnapshot, usize), PathEditError> {
        let snapshot = self.path(path).ok_or(PathEditError::NoSuchPath)?;
        let index = index_of(&snapshot, anchor)?;
        if !snapshot.closed && is_endpoint(&snapshot, index) {
            return Err(PathEditError::NotSplittable);
        }
        Ok((snapshot, index))
    }

    /// The open-path half of [`Document::split_at_anchor`] (acceptance
    /// criterion 13). The original object keeps `anchors[0..=index]`,
    /// its own `NodeId`, and every register — only the split anchor's
    /// outgoing handle is retracted and its kind set to `Corner`. A new
    /// object (a fresh `tree.create`, same as [`Document::create_path`],
    /// copying `a_path`'s own stroke/width rather than resetting to the
    /// placeholder default) gets a second copy of the split anchor
    /// (incoming handle retracted) followed by copies of
    /// `anchors[index + 1..]`, keeping their own `AnchorId`s — then moved
    /// to sit directly above the original in z-order
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`: "placed
    /// directly above the original in z-order"), not wherever
    /// `tree.create` happened to put it among the document's other root
    /// siblings.
    fn split_open_path(
        &self,
        path: NodeId,
        snapshot: &PathSnapshot,
        index: usize,
        new_id: AnchorId,
    ) -> Result<(SplitEndpoint, SplitEndpoint), PathEditError> {
        let split_anchor = &snapshot.anchors[index];
        let (_, anchors) = self.path_parts(path)?;

        let original_map = anchor_map_at(&anchors, index);
        write_vec2(&original_map, KEY_HANDLE_OUT, Vec2::ZERO);
        write_kind(&original_map, AnchorKind::Corner);
        let len = anchors.len();
        if index + 1 < len {
            // invariant: `len` was just read from this same list.
            #[allow(clippy::unwrap_used)]
            anchors.delete(index + 1, len - index - 1).unwrap();
        }

        let new_path_anchors: Vec<NewAnchor> = std::iter::once(NewAnchor {
            id: new_id,
            point: split_anchor.point,
            handle_in: Vec2::ZERO,
            handle_out: split_anchor.handle_out,
            kind: AnchorKind::Corner,
        })
        .chain(snapshot.anchors[index + 1..].iter().copied())
        .collect();
        let new_path = self.create_path_uncommitted(
            &new_path_anchors,
            false,
            snapshot.stroke_width.as_mm(),
            snapshot.stroke,
            snapshot.rotation,
        );

        // `mov_after` (and every other positional tree op) needs the
        // fractional-index feature turned on for this tree; it is not
        // persisted on the handle across `get_tree` calls as far as this
        // crate can rely on, so this enables it every time rather than
        // assuming an earlier call already did.
        let tree = self.loro().get_tree(OBJECTS_TREE);
        tree.enable_fractional_index(0);
        // invariant: `path` was just resolved via `self.path_parts` above
        // (still the original, surviving object), and `new_path` was
        // just created by `create_path_uncommitted` immediately above —
        // both tree ids are live root siblings at this point.
        #[allow(clippy::unwrap_used)]
        tree.mov_after(
            crate::paths::tree_id_of(new_path),
            crate::paths::tree_id_of(path),
        )
        .unwrap();

        self.commit_with_label("split_at_anchor");
        Ok(((path, split_anchor.id), (new_path, new_id)))
    }

    /// The closed-path half of [`Document::split_at_anchor`] (acceptance
    /// criterion 14). Same object, same `NodeId`, `closed` becomes
    /// `false`, one more anchor than before. The list is rotated with
    /// movable-list `mov` operations (one anchor moved from front to
    /// back per step) rather than deleted and reinserted, so every
    /// surviving anchor keeps its own list element
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`). Resulting
    /// order: a fresh copy (outgoing handle, first node), the anchors
    /// after the split point around the loop, the split anchor itself
    /// (incoming handle kept, outgoing retracted, last node).
    fn split_closed_path(
        &self,
        path: NodeId,
        snapshot: &PathSnapshot,
        index: usize,
        new_id: AnchorId,
    ) -> Result<(SplitEndpoint, SplitEndpoint), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let len = anchors.len();
        let split_anchor = &snapshot.anchors[index];

        for _ in 0..=index {
            // invariant: `len` (`anchors.len()`) is unchanged by `mov`;
            // `0` and `len - 1` are valid positions of this non-empty
            // list (it holds at least the anchor just resolved above).
            #[allow(clippy::unwrap_used)]
            anchors.mov(0, len - 1).unwrap();
        }

        let last_map = anchor_map_at(&anchors, len - 1);
        write_vec2(&last_map, KEY_HANDLE_OUT, Vec2::ZERO);
        write_kind(&last_map, AnchorKind::Corner);

        let new_anchor = NewAnchor {
            id: new_id,
            point: split_anchor.point,
            handle_in: Vec2::ZERO,
            handle_out: split_anchor.handle_out,
            kind: AnchorKind::Corner,
        };
        insert_anchor_at(&anchors, 0, &new_anchor);

        write_closed(&meta, false);
        self.commit_with_label("split_at_anchor");
        Ok(((path, new_id), (path, split_anchor.id)))
    }
}

/// The point exactly between `a` and `b` — Join's own, no-snapping
/// placement rule (acceptance criterion 9).
fn midpoint_of(a: Point, b: Point) -> Point {
    Point::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y))
}

/// `id`'s index within `snapshot`'s own anchor list.
fn index_of(snapshot: &PathSnapshot, id: AnchorId) -> Result<usize, PathEditError> {
    snapshot
        .anchors
        .iter()
        .position(|anchor| anchor.id == id)
        .ok_or(PathEditError::NoSuchAnchor)
}

/// Whether `index` is the first or last anchor of a path with this many
/// anchors — Join's and Split's shared "is this an endpoint" test
/// (acceptance criteria 8, 12).
fn is_endpoint(snapshot: &PathSnapshot, index: usize) -> bool {
    index == 0 || index == snapshot.anchors.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::NewAnchor;
    use crate::units::Point;

    fn anchor(id: u64, x: f64, y: f64) -> NewAnchor {
        NewAnchor::corner(AnchorId::new(1, id), Point::new(x, y))
    }

    /// Acceptance criterion 10: the first and last anchor of one open
    /// path join into one node, closing it, with one fewer anchor.
    #[test]
    fn join_endpoints_on_one_open_path_closes_it() {
        let document = Document::new(1);
        let first = AnchorId::new(1, 1);
        let last = AnchorId::new(1, 3);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: first,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-1.0, -1.0),
                    handle_out: Vec2::new(1.0, 1.0),
                    kind: AnchorKind::Symmetric,
                },
                anchor(2, 10.0, 0.0),
                NewAnchor {
                    id: last,
                    point: Point::new(20.0, 20.0),
                    handle_in: Vec2::new(-2.0, -2.0),
                    handle_out: Vec2::new(2.0, 2.0),
                    kind: AnchorKind::Symmetric,
                },
            ],
            false,
        );
        let (merged_path, merged_anchor) = document
            .join_endpoints(path, first, path, last)
            .expect("join");
        assert_eq!(merged_path, path);
        assert_eq!(merged_anchor, first, "a's own id survives");

        let snapshot = document.path(path).expect("exists");
        assert!(snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 2);
        let merged = &snapshot.anchors[0];
        assert_eq!(merged.point, Point::new(10.0, 10.0), "exact midpoint");
        assert_eq!(merged.kind, AnchorKind::Corner);
        // a's own interior handle (handle_out, a is the first anchor)
        // untouched; handle_in transplanted from the removed last anchor's
        // own interior handle (its handle_in).
        assert_eq!(merged.handle_out, Vec2::new(1.0, 1.0));
        assert_eq!(merged.handle_in, Vec2::new(-2.0, -2.0));
    }

    /// Joining the two ends of a 2-anchor open path is refused: the
    /// result would be a 1-anchor closed path nothing can draw.
    #[test]
    fn join_endpoints_refuses_a_two_anchor_open_path() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        assert_eq!(
            document.join_endpoints(path, a, path, b),
            Err(PathEditError::NotJoinable)
        );
    }

    /// An interior node is never an endpoint: refused.
    #[test]
    fn join_endpoints_refuses_an_interior_node() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        assert_eq!(
            document.join_endpoints(id, AnchorId::new(1, 2), id, AnchorId::new(1, 1)),
            Err(PathEditError::NotJoinable)
        );
    }

    /// A closed path has no endpoints at all: refused (`specification.md`
    /// "Out of scope": "Joining any node of a closed path").
    #[test]
    fn join_endpoints_refuses_a_closed_path() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 5.0, 10.0),
            ],
            true,
        );
        let _ = (a, b, c);
        assert_eq!(
            document.join_endpoints(id, AnchorId::new(1, 1), id, AnchorId::new(1, 2)),
            Err(PathEditError::NotJoinable)
        );
    }

    /// Acceptance criterion 9, combination 1: `a` last, `b` first — the
    /// simplest case, no reversal needed. `a`'s path survives, `b`'s is
    /// appended after it unchanged, `b`'s object is deleted.
    #[test]
    fn join_endpoints_two_objects_a_last_b_first_appends_unchanged() {
        let document = Document::new(1);
        let a1 = AnchorId::new(1, 1);
        let a2 = AnchorId::new(1, 2);
        let b1 = AnchorId::new(1, 11);
        let b2 = AnchorId::new(1, 12);
        let a_path = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                NewAnchor {
                    id: a2,
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(-1.0, 0.0),
                    handle_out: Vec2::new(3.0, 0.0),
                    kind: AnchorKind::Corner,
                },
            ],
            false,
        );
        let b_path = document.create_path(
            &[
                NewAnchor {
                    id: b1,
                    point: Point::new(14.0, 0.0),
                    handle_in: Vec2::new(-4.0, 0.0),
                    handle_out: Vec2::new(1.0, 0.0),
                    kind: AnchorKind::Corner,
                },
                anchor(12, 30.0, 0.0),
            ],
            false,
        );
        let _ = (a1, b2);

        let (merged_path, merged_anchor) = document
            .join_endpoints(a_path, a2, b_path, b1)
            .expect("join");
        assert_eq!(merged_path, a_path, "a's object survives");
        assert_eq!(merged_anchor, a2);
        assert_eq!(document.path(b_path), None, "b's object is deleted");

        let snapshot = document.path(a_path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        assert_eq!(snapshot.anchors[0].id, a1);
        let merged = &snapshot.anchors[1];
        assert_eq!(merged.id, a2);
        assert_eq!(merged.point, Point::new(12.0, 0.0), "exact midpoint");
        assert_eq!(merged.kind, AnchorKind::Corner);
        assert_eq!(
            merged.handle_in,
            Vec2::new(-1.0, 0.0),
            "a's own interior handle"
        );
        assert_eq!(
            merged.handle_out,
            Vec2::new(1.0, 0.0),
            "b's own interior handle"
        );
        assert_eq!(snapshot.anchors[2].id, b2);
        assert_eq!(snapshot.anchors[2].point, Point::new(30.0, 0.0));
    }

    /// Acceptance criterion 9, combination 2: `a` last, `b` last — `b`'s
    /// path must be reversed (and its handles swapped) so the two
    /// selected nodes land adjacent.
    #[test]
    fn join_endpoints_two_objects_a_last_b_last_reverses_b() {
        let document = Document::new(1);
        let a2 = AnchorId::new(1, 2);
        let b1 = AnchorId::new(1, 11);
        let b2 = AnchorId::new(1, 12);
        let a_path = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let b_path = document.create_path(
            &[
                anchor(11, 30.0, 0.0),
                NewAnchor {
                    id: b2,
                    point: Point::new(14.0, 0.0),
                    handle_in: Vec2::new(-1.0, 0.0),
                    handle_out: Vec2::new(4.0, 0.0),
                    kind: AnchorKind::Corner,
                },
            ],
            false,
        );
        let _ = b1;

        let (merged_path, merged_anchor) = document
            .join_endpoints(a_path, a2, b_path, b2)
            .expect("join");
        assert_eq!(merged_path, a_path);
        assert_eq!(merged_anchor, a2);

        let snapshot = document.path(a_path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        // b's path, reversed: b2 (merged) then b1.
        assert_eq!(snapshot.anchors[2].id, AnchorId::new(1, 11));
        assert_eq!(snapshot.anchors[2].point, Point::new(30.0, 0.0));
        let merged = &snapshot.anchors[1];
        // b2's own interior handle before reversal was handle_in
        // (-1, 0); after the merge it faces "after" the junction.
        assert_eq!(merged.handle_out, Vec2::new(-1.0, 0.0));
    }

    /// Acceptance criterion 9, combination 3: `a` first, `b` first — `b`
    /// must also be reversed.
    #[test]
    fn join_endpoints_two_objects_a_first_b_first_reverses_b() {
        let document = Document::new(1);
        let a1 = AnchorId::new(1, 1);
        let b1 = AnchorId::new(1, 11);
        let a_path = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let b_path = document.create_path(&[anchor(11, -10.0, 0.0), anchor(12, -20.0, 0.0)], false);

        let (merged_path, merged_anchor) = document
            .join_endpoints(a_path, a1, b_path, b1)
            .expect("join");
        assert_eq!(merged_path, a_path);
        assert_eq!(merged_anchor, a1);

        let snapshot = document.path(a_path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        // b's path, reversed, prepended: b2 then merged (b1/a1) then a2.
        assert_eq!(snapshot.anchors[0].id, AnchorId::new(1, 12));
        assert_eq!(snapshot.anchors[1].id, a1);
        assert_eq!(snapshot.anchors[2].id, AnchorId::new(1, 2));
    }

    /// Acceptance criterion 9, combination 4: `a` first, `b` last — no
    /// reversal; `b`'s path is prepended unchanged.
    #[test]
    fn join_endpoints_two_objects_a_first_b_last_prepends_unchanged() {
        let document = Document::new(1);
        let a1 = AnchorId::new(1, 1);
        let b2 = AnchorId::new(1, 12);
        let a_path = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let b_path = document.create_path(&[anchor(11, -20.0, 0.0), anchor(12, -10.0, 0.0)], false);

        let (merged_path, merged_anchor) = document
            .join_endpoints(a_path, a1, b_path, b2)
            .expect("join");
        assert_eq!(merged_path, a_path);
        assert_eq!(merged_anchor, a1);

        let snapshot = document.path(a_path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        assert_eq!(snapshot.anchors[0].id, AnchorId::new(1, 11));
        assert_eq!(snapshot.anchors[1].id, a1);
        assert_eq!(snapshot.anchors[2].id, AnchorId::new(1, 2));
    }

    /// Join's outward, dangling handles (the two sides *not* facing the
    /// junction) are discarded, not transplanted anywhere.
    #[test]
    fn join_endpoints_discards_the_two_dangling_handles() {
        let document = Document::new(1);
        let a2 = AnchorId::new(1, 2);
        let b1 = AnchorId::new(1, 11);
        let a_path = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-9.0, -9.0), // a1's dangling handle
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Corner,
                },
                NewAnchor {
                    id: a2,
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(9.0, 9.0), // a2's dangling handle
                    kind: AnchorKind::Corner,
                },
            ],
            false,
        );
        let b_path = document.create_path(
            &[
                NewAnchor {
                    id: b1,
                    point: Point::new(14.0, 0.0),
                    handle_in: Vec2::new(8.0, 8.0), // b1's own dangling handle (b1 is b_path's *first* anchor)
                    handle_out: Vec2::new(1.0, 0.0), // b1's own interior handle, kept
                    kind: AnchorKind::Corner,
                },
                NewAnchor {
                    id: AnchorId::new(1, 12),
                    point: Point::new(30.0, 0.0),
                    handle_in: Vec2::new(-7.0, -7.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Corner,
                },
            ],
            false,
        );
        document
            .join_endpoints(a_path, a2, b_path, b1)
            .expect("join");
        let snapshot = document.path(a_path).expect("exists");
        let merged = snapshot
            .anchors
            .iter()
            .find(|anchor| anchor.id == a2)
            .expect("merged anchor keeps a2's id");
        assert_eq!(
            merged.handle_in,
            Vec2::ZERO,
            "a2's own interior handle (handle_in, a2 is a_path's last anchor) kept, not its \
             dangling handle_out (9, 9)"
        );
        assert_eq!(
            merged.handle_out,
            Vec2::new(1.0, 0.0),
            "b1's own interior handle (handle_out, b1 is b_path's first anchor) kept, not its \
             dangling handle_in (8, 8)"
        );
    }

    /// Join has no distance limit: two far-apart endpoints still merge,
    /// at the exact midpoint.
    #[test]
    fn join_endpoints_has_no_distance_limit() {
        let document = Document::new(1);
        let a2 = AnchorId::new(1, 2);
        let b1 = AnchorId::new(1, 11);
        let a_path = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 0.0, 0.0)], false);
        let b_path = document.create_path(
            &[anchor(11, 1000.0, 2000.0), anchor(12, 1000.0, 2000.0)],
            false,
        );
        let _ = (a2, b1);
        let (merged_path, merged_anchor) = document
            .join_endpoints(a_path, AnchorId::new(1, 2), b_path, AnchorId::new(1, 11))
            .expect("join, however far apart");
        let snapshot = document.path(merged_path).expect("exists");
        let merged = snapshot
            .anchors
            .iter()
            .find(|a| a.id == merged_anchor)
            .unwrap();
        assert_eq!(merged.point, Point::new(500.0, 1000.0));
    }

    /// Acceptance criterion 12: the first or last anchor of an open path
    /// cannot be split.
    #[test]
    fn split_at_anchor_refuses_an_open_paths_endpoint() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        assert_eq!(
            document.split_at_anchor(id, AnchorId::new(1, 1), AnchorId::new(9, 1)),
            Err(PathEditError::NotSplittable)
        );
        assert_eq!(
            document.split_at_anchor(id, AnchorId::new(1, 3), AnchorId::new(9, 1)),
            Err(PathEditError::NotSplittable)
        );
    }

    /// Acceptance criterion 13: splitting an interior node of an open
    /// path produces two separate open path objects.
    #[test]
    fn split_at_anchor_on_an_open_path_produces_two_objects() {
        let document = Document::new(1);
        let middle = AnchorId::new(1, 2);
        let id = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(1.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: middle,
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(2.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: AnchorId::new(1, 3),
                    point: Point::new(20.0, 0.0),
                    handle_in: Vec2::new(-1.0, 0.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Symmetric,
                },
            ],
            false,
        );
        let new_id = AnchorId::new(9, 1);
        let ((first_path, first_anchor), (second_path, second_anchor)) =
            document.split_at_anchor(id, middle, new_id).expect("split");
        assert_eq!(first_path, id, "the original object keeps its NodeId");
        assert_eq!(first_anchor, middle, "the original copy keeps the id");
        assert_ne!(second_path, first_path, "a brand-new object");
        assert_eq!(second_anchor, new_id);

        let first = document.path(first_path).expect("exists");
        assert!(!first.closed);
        assert_eq!(first.anchors.len(), 2);
        assert_eq!(first.anchors[1].point, Point::new(10.0, 0.0));
        assert_eq!(first.anchors[1].handle_in, Vec2::new(-2.0, 0.0), "kept");
        assert_eq!(first.anchors[1].handle_out, Vec2::ZERO, "retracted");
        assert_eq!(first.anchors[1].kind, AnchorKind::Corner);

        let second = document.path(second_path).expect("exists");
        assert!(!second.closed);
        assert_eq!(second.anchors.len(), 2);
        assert_eq!(second.anchors[0].point, Point::new(10.0, 0.0));
        assert_eq!(second.anchors[0].handle_in, Vec2::ZERO, "retracted");
        assert_eq!(second.anchors[0].handle_out, Vec2::new(2.0, 0.0), "kept");
        assert_eq!(second.anchors[0].kind, AnchorKind::Corner);
        assert_eq!(second.anchors[1].point, Point::new(20.0, 0.0));
    }

    /// Acceptance criterion 13 / architect review: the new object lands
    /// directly above the original in z-order, not at the very end of
    /// the document's sibling order — a third, pre-existing object
    /// above the original must stay above the *new* split-off object
    /// too.
    #[test]
    fn split_at_anchor_on_an_open_path_places_the_new_object_directly_above_the_original() {
        let document = Document::new(1);
        let middle = AnchorId::new(1, 2);
        let original = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        let third = document.create_path(&[anchor(4, 0.0, 50.0), anchor(5, 10.0, 50.0)], false);

        let (_, (new_path, _)) = document
            .split_at_anchor(original, middle, AnchorId::new(9, 1))
            .expect("split");

        assert_eq!(
            document.object_ids(),
            vec![original, new_path, third],
            "the split-off object sits directly above the original, below the pre-existing \
             third object — not pushed to the top of the whole document"
        );
    }

    /// The new object from an open-path split copies the original's
    /// style (stroke/width) rather than resetting to the placeholder
    /// default. There is no public setter for stroke width yet
    /// (`stroke-and-fill-styling` is a later slice), so this pokes the
    /// Loro value directly — the same technique `container.rs`'s own
    /// fixture generator already uses — purely to prove the new object's
    /// style is *read from the original's snapshot*, not hardcoded to
    /// the placeholder default.
    #[test]
    fn split_at_anchor_on_an_open_path_copies_style_to_the_new_object() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        let tree = document.loro().get_tree(OBJECTS_TREE);
        let meta = tree.get_meta(crate::paths::tree_id_of(id)).expect("meta");
        meta.insert(crate::path_codec::KEY_STROKE_WIDTH, 3.0)
            .expect("set width");
        document.commit_with_label("test setup");

        let (_, (second_path, _)) = document
            .split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(9, 1))
            .expect("split");
        let second = document.path(second_path).expect("exists");
        assert!((second.stroke_width.as_mm() - 3.0).abs() < 1e-9);
    }

    /// `specs/0005-object-transform/adrs.md`'s architect note: the new
    /// object from an open-path split copies the original's `rotation`
    /// register.
    #[test]
    fn split_at_anchor_on_an_open_path_copies_rotation_to_the_new_object() {
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
            .rotate_object(&document.object(id).expect("object exists").rotated(
                crate::units::Point::new(0.0, 0.0),
                crate::units::Angle::from_radians(0.4),
            ))
            .expect("rotate");

        let (_, (second_path, _)) = document
            .split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(9, 1))
            .expect("split");
        let second = document.path(second_path).expect("exists");
        assert!((second.rotation.as_radians() - 0.4).abs() < 1e-9);
    }

    /// Acceptance criterion 14: splitting a closed path opens it, with
    /// one more anchor, in the order "outgoing copy, anchors after the
    /// split point around the loop, incoming copy".
    #[test]
    fn split_at_anchor_on_a_closed_path_opens_it_with_one_more_anchor() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                NewAnchor {
                    id: b,
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(2.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                anchor(3, 5.0, 10.0),
            ],
            true,
        );
        let _ = c;
        let new_id = AnchorId::new(9, 1);
        let ((first_path, first_anchor), (second_path, second_anchor)) =
            document.split_at_anchor(id, b, new_id).expect("split");
        assert_eq!(first_path, id);
        assert_eq!(second_path, id, "same object, not a new one");
        assert_eq!(
            first_anchor, new_id,
            "outgoing-handle copy is the new first node"
        );
        assert_eq!(second_anchor, b, "incoming-handle copy keeps b's id");

        let snapshot = document.path(id).expect("exists");
        assert!(!snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 4);
        // Order: outgoing copy (new_id), anchors after the split point
        // (c), around the loop (a), incoming copy (b).
        assert_eq!(snapshot.anchors[0].id, new_id);
        assert_eq!(snapshot.anchors[0].point, Point::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::ZERO);
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(2.0, 0.0));
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Corner);
        assert_eq!(snapshot.anchors[1].id, AnchorId::new(1, 3));
        assert_eq!(snapshot.anchors[2].id, a);
        assert_eq!(snapshot.anchors[3].id, b);
        assert_eq!(snapshot.anchors[3].point, Point::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[3].handle_in, Vec2::new(-2.0, 0.0));
        assert_eq!(snapshot.anchors[3].handle_out, Vec2::ZERO);
        assert_eq!(snapshot.anchors[3].kind, AnchorKind::Corner);
    }

    /// Any node of a closed path may be split, including what was index
    /// 0 — exercising `mov`'s rotation at the boundary.
    #[test]
    fn split_at_anchor_on_a_closed_path_at_index_zero() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 5.0, 10.0),
            ],
            true,
        );
        let new_id = AnchorId::new(9, 1);
        let ((_, first_anchor), (_, second_anchor)) =
            document.split_at_anchor(id, a, new_id).expect("split");
        assert_eq!(first_anchor, new_id);
        assert_eq!(second_anchor, a);
        let snapshot = document.path(id).expect("exists");
        assert!(!snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 4);
        assert_eq!(snapshot.anchors[0].id, new_id);
        assert_eq!(snapshot.anchors[1].id, AnchorId::new(1, 2));
        assert_eq!(snapshot.anchors[2].id, AnchorId::new(1, 3));
        assert_eq!(snapshot.anchors[3].id, a);
    }

    /// Split and Join are exact-enough inverses: splitting an open path
    /// and immediately re-joining the two new endpoints restores the
    /// original topology (not bit-for-bit, since Join drops the dangling
    /// handles Split had just reintroduced as zero — but the anchor
    /// count and positions round-trip).
    #[test]
    fn split_then_join_round_trips_anchor_count_and_positions() {
        let document = Document::new(1);
        let middle = AnchorId::new(1, 2);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                anchor(2, 10.0, 0.0),
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        let new_id = AnchorId::new(9, 1);
        let ((first_path, first_anchor), (second_path, second_anchor)) =
            document.split_at_anchor(id, middle, new_id).expect("split");

        let (_, rejoined_anchor) = document
            .join_endpoints(first_path, first_anchor, second_path, second_anchor)
            .expect("re-join");
        let snapshot = document.path(first_path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3, "back to three anchors");
        assert_eq!(snapshot.anchors[1].point, Point::new(10.0, 0.0));
        assert_eq!(rejoined_anchor, first_anchor);
    }
}
