//! `Document`'s path-editing command methods (`specs/0002-path-node-editing/
//! adrs.md`, "the anchor schema, and the three merge choices inside it").
//! [`crate::path_model`] defines the pure data types a caller builds or
//! reads; [`crate::path_codec`] owns the Loro value shapes and keys these
//! methods write and read. This module is the command funnel itself —
//! `specs/0002-path-node-editing/adrs.md`'s architect review note: "a
//! serializable `Command` enum is deferred to `undo-redo`; what is not
//! deferred is that each mutating method ends in exactly one Loro commit"
//! (ADR 0002 §9).

use loro::{LoroMap, LoroMovableList, TreeID, TreeParentId};

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::{
    self, KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, adjacent_segment_indices, anchor_index,
    anchor_map_at, anchors_container, insert_anchor_at, insert_anchors_container,
    neighbour_tangent, node_exists, push_anchor, read_closed, read_kind, read_point, read_vec2,
    write_closed, write_kind, write_point, write_vec2,
};
use crate::path_model::{
    AnchorId, AnchorKind, Color, HandleSlot, NewAnchor, NodeId, PathEditError, PathSnapshot,
};
use crate::units::{Point, Vec2};

/// Default handle length a corner→symmetric/asymmetric conversion pulls
/// out (`specs/0002-path-node-editing/specification.md` acceptance
/// criterion 11; no acceptance criterion pins an exact value). Renamed
/// from `DEFAULT_HANDLE_LENGTH_MM`
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`: "the wasm
/// binding string... is not persisted and is renamed outright.
/// `DEFAULT_HANDLE_LENGTH_MM` becomes `DEFAULT_HANDLE_LENGTH_MM`").
const DEFAULT_HANDLE_LENGTH_MM: f64 = 10.0;

/// Below this length, a handle counts as "no existing length to keep" for
/// acceptance criterion 2's "its own current length, if that side already
/// had a non-zero handle; otherwise the slice's existing default handle
/// length" — not a geometric [`crate::units::Tolerance`] (`CLAUDE.md` §5):
/// this is a plain zero/non-zero classification of a stored value, the
/// same kind of exact check `specs/0002-path-node-editing/adrs.md` already
/// uses for "a retracted handle is the exact zero vector".
const ZERO_HANDLE_EPSILON: f64 = f64::EPSILON;

/// One of [`Document::split_at_anchor`]'s two resulting coincident
/// anchors: which path it ended up on, and its own id. A named alias
/// rather than the bare nested tuple at each of its three call sites
/// (`clippy::type_complexity`).
type SplitEndpoint = (NodeId, AnchorId);

/// Default fraction of the segment chord a "make curve" extends its two
/// adjoining handles by (acceptance criterion 14; a standard Bézier
/// approximation fraction, not a value any acceptance criterion pins).
const DEFAULT_CURVE_HANDLE_FRACTION: f64 = 1.0 / 3.0;

/// Resolves what an anchor's whole `(handle_in, handle_out)` pair becomes
/// when `slot`'s handle is set to `value` — acceptance criterion 9's one
/// rule (mirror for [`AnchorKind::Symmetric`], touch only the named handle
/// for [`AnchorKind::Corner`]), as a single pure function rather than
/// logic duplicated at each of its two callers. [`Document::set_handle`]
/// calls this to build what it writes; `vecmanf-ui-core`'s live handle-
/// drag preview calls the exact same function to build what it
/// *previews*, so the two can never independently drift apart (the same
/// gap the pen-tool preview/commit split had — `specs/0002-path-node-
/// editing/adrs.md`'s "commands carry resolved geometry, never geometric
/// intent", read here as "one resolution rule, not two").
///
/// `handle_in`/`handle_out` are the anchor's values *before* this write —
/// needed only to pass the untouched side through unchanged for a
/// [`AnchorKind::Corner`] anchor, since this function has no document to
/// read them from itself.
#[must_use]
pub fn resolve_handle_pair(
    kind: AnchorKind,
    slot: HandleSlot,
    value: Vec2,
    handle_in: Vec2,
    handle_out: Vec2,
) -> (Vec2, Vec2) {
    match (slot, kind) {
        (HandleSlot::In, AnchorKind::Symmetric) => (value, value.negated()),
        (HandleSlot::Out, AnchorKind::Symmetric) => (value.negated(), value),
        (HandleSlot::In, AnchorKind::Corner) => (value, handle_out),
        (HandleSlot::Out, AnchorKind::Corner) => (handle_in, value),
        // `specs/0006-path-merge-split-and-node-types/adrs.md`'s rule
        // table: the opposite handle rotates to stay collinear through
        // the anchor at the dragged handle's new angle, but its own
        // distance from the anchor does not change — only the dragged
        // handle's own length changes (acceptance criterion 3).
        (HandleSlot::In, AnchorKind::Asymmetric) => (value, asymmetric_opposite(value, handle_out)),
        (HandleSlot::Out, AnchorKind::Asymmetric) => (asymmetric_opposite(value, handle_in), value),
    }
}

/// The opposite handle's new value when the dragged handle (on an
/// [`AnchorKind::Asymmetric`] anchor) is set to `dragged`: collinear with
/// `dragged` through the anchor, at `opposite`'s own (unchanged) length.
///
/// Edge cases (`specs/0006-path-merge-split-and-node-types/adrs.md`):
/// `dragged` = zero leaves `opposite` unchanged — there is no direction to
/// follow, and [`Vec2::normalized_to`] would otherwise collapse it to zero
/// instead of leaving it alone. An `opposite` of length zero stays zero
/// either way, since [`Vec2::normalized_to`] with a zero target length
/// always returns [`Vec2::ZERO`] regardless of direction.
#[must_use]
fn asymmetric_opposite(dragged: Vec2, opposite: Vec2) -> Vec2 {
    if dragged == Vec2::ZERO {
        opposite
    } else {
        dragged.normalized_to(opposite.length()).negated()
    }
}

/// Acceptance criterion 2's "its own current length, if that side already
/// had a non-zero handle; otherwise the slice's existing default handle
/// length" — one side of a Corner→Asymmetric conversion.
#[must_use]
fn kept_length_or_default(existing_handle: Vec2) -> f64 {
    let length = existing_handle.length();
    if length > ZERO_HANDLE_EPSILON {
        length
    } else {
        DEFAULT_HANDLE_LENGTH_MM
    }
}

impl Document {
    /// Commits a finished pen-tool session as one new path object
    /// (`specs/0002-path-node-editing/adrs.md`, "a pen session is one commit";
    /// acceptance criteria 1, 2, 3, 5).
    ///
    /// # Panics
    /// Does not panic in practice: it only creates a root-level tree node
    /// and inserts known-valid keys and values into its freshly created
    /// meta map and movable list, none of which Loro's API can reject.
    #[must_use]
    pub fn create_path(&self, anchors: &[NewAnchor], closed: bool) -> NodeId {
        let id = self.create_path_uncommitted(
            anchors,
            closed,
            path_codec::DEFAULT_STROKE_WIDTH_MM,
            Color::BLACK,
        );
        self.commit_with_label("create_path");
        id
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
    /// (`specs/0003-primitive-shapes/adrs.md`: "`Document::path(id)` returns
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
    /// relative to their own anchor (`specs/0002-path-node-editing/adrs.md`
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
    /// anchor is [`AnchorKind::Symmetric`] (`handle_in = -handle_out`) and
    /// touching only the named handle when it is
    /// [`AnchorKind::Corner`] (`specs/0002-path-node-editing/adrs.md`
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
        // `resolve_handle_pair` is the one place this rule lives. Reading
        // the current `handle_in`/`handle_out` here is just a read, not
        // a write — it is needed for `AnchorKind::Asymmetric`'s opposite
        // (which must rotate while keeping *its own* current length,
        // `specs/0006-path-merge-split-and-node-types/adrs.md`), and is
        // harmless for `AnchorKind::Corner`, whose own two
        // `resolve_handle_pair` branches echo them back unused — this
        // method still never *writes* a `Corner` anchor's untouched
        // side, only the mirror write below is skipped for it, which is
        // what actually matters for the LWW-register hazard this
        // slice's own "a press and release with no pointer movement
        // writes nothing" rule guards against
        // (`specs/0002-path-node-editing/adrs.md`): a redundant write
        // with a newer clock can still beat a collaborator's concurrent
        // edit to that same field.
        let current_handle_in = read_vec2(&map, KEY_HANDLE_IN);
        let current_handle_out = read_vec2(&map, KEY_HANDLE_OUT);
        let (new_in, new_out) =
            resolve_handle_pair(kind, slot, value, current_handle_in, current_handle_out);
        let own_value = match slot {
            HandleSlot::In => new_in,
            HandleSlot::Out => new_out,
        };
        write_vec2(&map, own_key, own_value);
        if kind == AnchorKind::Symmetric || kind == AnchorKind::Asymmetric {
            let mirror_value = match slot {
                HandleSlot::In => new_out,
                HandleSlot::Out => new_in,
            };
            write_vec2(&map, mirror_key, mirror_value);
        }
        self.commit_with_label("set_handle");
        Ok(())
    }

    /// Converts every named anchor to `kind` as one commit — a multi-node
    /// selection's convert is one interaction, not `anchors.len()` of them
    /// (`specs/0002-path-node-editing/specification.md` acceptance
    /// criterion 11).
    ///
    /// Every id is resolved to an index before any of them is written, so
    /// one stale id anywhere in `anchors` refuses the whole call rather
    /// than converting a prefix of it.
    ///
    /// Per-anchor rule (`specs/0006-path-merge-split-and-node-types/
    /// adrs.md`'s conversion table):
    /// - **to the kind the anchor already has: no write at all**
    ///   (acceptance criterion 16 — this amends
    ///   `specs/0002-path-node-editing/specification.md` AC 11, under
    ///   which re-applying "make smooth" to an already-smooth node used
    ///   to reset its handles every time).
    /// - **to [`AnchorKind::Symmetric`]** (from any other kind): both
    ///   handles reset to `DEFAULT_HANDLE_LENGTH_MM`, collinear through
    ///   the anchor along the chord between its neighbours — any existing
    ///   independent lengths are discarded, not averaged (acceptance
    ///   criteria 5 and `path-node-editing`'s own AC 11).
    /// - **[`AnchorKind::Corner`] → [`AnchorKind::Asymmetric`]**: same
    ///   tangent direction, but each side keeps its own current length if
    ///   it was already non-zero, otherwise the default (acceptance
    ///   criterion 2).
    /// - **every other conversion** (`Symmetric`→`Corner`, `Asymmetric`→
    ///   `Corner`, `Symmetric`→`Asymmetric`): both handles stay exactly
    ///   where they are, shape-preserving — geometry cannot tell the
    ///   kinds apart, which is exactly why `kind` is a stored field
    ///   (`specs/0002-path-node-editing/adrs.md` decision 3; acceptance
    ///   criterion 4).
    ///
    /// If every named anchor already has `kind`, nothing is written and
    /// no commit happens at all — not even an empty one — matching
    /// `specs/0002-path-node-editing/adrs.md`'s "a click must not be able
    /// to [win against a collaborator's concurrent edit]" rule, now
    /// extended from a pure no-move click to a pure no-op convert.
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
        let mut changed = false;
        for index in indices {
            let map = anchor_map_at(&anchors, index);
            let current = read_kind(&map);
            if current == kind {
                // Acceptance criterion 16: converting to the kind the
                // anchor already has is a no-op, not a reset.
                continue;
            }
            changed = true;
            match kind {
                AnchorKind::Symmetric => {
                    let point = read_point(&map, KEY_POINT);
                    let tangent = neighbour_tangent(&anchors, closed, index, point)
                        .normalized_to(DEFAULT_HANDLE_LENGTH_MM);
                    write_vec2(&map, KEY_HANDLE_OUT, tangent);
                    write_vec2(&map, KEY_HANDLE_IN, tangent.negated());
                }
                AnchorKind::Asymmetric if current == AnchorKind::Corner => {
                    let point = read_point(&map, KEY_POINT);
                    let handle_in = read_vec2(&map, KEY_HANDLE_IN);
                    let handle_out = read_vec2(&map, KEY_HANDLE_OUT);
                    let unit = neighbour_tangent(&anchors, closed, index, point).normalized_to(1.0);
                    let out_len = kept_length_or_default(handle_out);
                    let in_len = kept_length_or_default(handle_in);
                    write_vec2(&map, KEY_HANDLE_OUT, unit.scaled(out_len));
                    write_vec2(&map, KEY_HANDLE_IN, unit.negated().scaled(in_len));
                }
                // `Symmetric`→`Corner`, `Asymmetric`→`Corner`,
                // `Symmetric`→`Asymmetric`: shape-preserving, kind only.
                AnchorKind::Corner | AnchorKind::Asymmetric => {}
            }
            write_kind(&map, kind);
        }
        if changed {
            self.commit_with_label("convert_anchor_kind");
        }
        Ok(())
    }

    /// Inserts a new anchor right after `after`, carrying the caller-
    /// resolved subdivision geometry for the two adjoining handles
    /// (`specs/0002-path-node-editing/adrs.md`, "commands carry resolved
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
            return self.join_same_path(a_path, &a_snapshot);
        }

        let b_snapshot = self.path(b_path).ok_or(PathEditError::NoSuchPath)?;
        let b_index = index_of(&b_snapshot, b_anchor)?;
        if b_snapshot.closed || !is_endpoint(&b_snapshot, b_index) {
            return Err(PathEditError::NotJoinable);
        }

        self.join_two_objects(a_path, &a_snapshot, a_index, b_path, &b_snapshot, b_index)
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
        let b_tree_id = tree_id_of(b_path);
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
        let snapshot = self.path(path).ok_or(PathEditError::NoSuchPath)?;
        let index = index_of(&snapshot, anchor)?;
        if !snapshot.closed && is_endpoint(&snapshot, index) {
            return Err(PathEditError::NotSplittable);
        }

        if snapshot.closed {
            self.split_closed_path(path, &snapshot, index, new_id)
        } else {
            self.split_open_path(path, &snapshot, index, new_id)
        }
    }

    /// The open-path half of [`Document::split_at_anchor`] (acceptance
    /// criterion 13). The original object keeps `anchors[0..=index]`,
    /// its own `NodeId`, and every register — only the split anchor's
    /// outgoing handle is retracted and its kind set to `Corner`. A new
    /// object (a fresh `tree.create`, same as [`Document::create_path`],
    /// copying `a_path`'s own stroke/width rather than resetting to the
    /// placeholder default) gets a second copy of the split anchor
    /// (incoming handle retracted) followed by copies of
    /// `anchors[index + 1..]`, keeping their own `AnchorId`s.
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
        );

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

    /// [`Document::create_path`]'s own body, minus the commit — shared
    /// with [`Document::split_open_path`], which needs the new object's
    /// creation folded into Split's own single commit
    /// (`specs/0002-path-node-editing/adrs.md`'s "each mutating method
    /// ends in exactly one Loro commit") rather than a second, separate
    /// one. Also takes an explicit stroke width/color rather than always
    /// writing the placeholder default, so Split can copy the original
    /// path's own style instead of resetting it.
    ///
    /// # Panics
    /// Does not panic in practice — see [`Document::create_path`]'s own
    /// doc comment.
    fn create_path_uncommitted(
        &self,
        anchors: &[NewAnchor],
        closed: bool,
        stroke_width_mm: f64,
        stroke: Color,
    ) -> NodeId {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: creating a root-level node on a freshly obtained
        // tree handle cannot fail.
        #[allow(clippy::unwrap_used)]
        let tree_id = tree.create(TreeParentId::Root).unwrap();
        // invariant: reading the meta map of a node this call just
        // created cannot fail.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id).unwrap();
        path_codec::write_path_style(&meta, closed, stroke_width_mm, stroke);
        let anchor_list = insert_anchors_container(&meta);
        for anchor in anchors {
            push_anchor(&anchor_list, anchor);
        }
        NodeId::from_parts(tree_id.peer, tree_id.counter)
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

    fn anchor(id: u64, x: f64, y: f64) -> NewAnchor {
        NewAnchor::corner(AnchorId::new(1, id), Point::new(x, y))
    }

    fn smooth_anchor(id: u64, x: f64, y: f64, handle_out: Vec2) -> NewAnchor {
        NewAnchor {
            id: AnchorId::new(1, id),
            point: Point::new(x, y),
            handle_in: handle_out.negated(),
            handle_out,
            kind: AnchorKind::Symmetric,
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

    /// The shared rule itself, independent of any document: a `Smooth`
    /// anchor mirrors, regardless of which slot was set.
    #[test]
    fn resolve_handle_pair_mirrors_for_a_smooth_anchor_either_slot() {
        assert_eq!(
            resolve_handle_pair(
                AnchorKind::Symmetric,
                HandleSlot::Out,
                Vec2::new(3.0, 4.0),
                Vec2::ZERO,
                Vec2::ZERO,
            ),
            (Vec2::new(-3.0, -4.0), Vec2::new(3.0, 4.0)),
        );
        assert_eq!(
            resolve_handle_pair(
                AnchorKind::Symmetric,
                HandleSlot::In,
                Vec2::new(3.0, 4.0),
                Vec2::ZERO,
                Vec2::ZERO,
            ),
            (Vec2::new(3.0, 4.0), Vec2::new(-3.0, -4.0)),
        );
    }

    /// A `Corner` anchor leaves the other handle exactly as given, for
    /// either slot.
    #[test]
    fn resolve_handle_pair_leaves_the_other_handle_untouched_for_a_corner_anchor() {
        let existing_in = Vec2::new(-5.0, 0.0);
        let existing_out = Vec2::new(5.0, 0.0);
        assert_eq!(
            resolve_handle_pair(
                AnchorKind::Corner,
                HandleSlot::Out,
                Vec2::new(3.0, 4.0),
                existing_in,
                existing_out,
            ),
            (existing_in, Vec2::new(3.0, 4.0)),
            "handle_in passed through unchanged"
        );
        assert_eq!(
            resolve_handle_pair(
                AnchorKind::Corner,
                HandleSlot::In,
                Vec2::new(3.0, 4.0),
                existing_in,
                existing_out,
            ),
            (Vec2::new(3.0, 4.0), existing_out),
            "handle_out passed through unchanged"
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
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Symmetric)
            .expect("convert");
        let snapshot = document.path(id).expect("path exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Symmetric);
        assert_eq!(middle.handle_in, middle.handle_out.negated());
        assert!((middle.handle_out.length() - DEFAULT_HANDLE_LENGTH_MM).abs() < 1e-9);
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
                AnchorKind::Symmetric,
            )
            .expect("convert");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Symmetric);
        assert_eq!(snapshot.anchors[2].kind, AnchorKind::Symmetric);
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
            AnchorKind::Symmetric,
        );
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(
            snapshot.anchors[0].kind,
            AnchorKind::Corner,
            "the valid id in the batch must not have been converted either"
        );
    }

    /// Acceptance criterion 16: re-applying "make symmetric" to an
    /// already-symmetric node is a no-op — no handle change, and (unlike
    /// slice 2's old behaviour) no commit at all.
    #[test]
    fn convert_symmetric_to_symmetric_is_a_no_op_and_commits_nothing() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(5.0, 0.0)),
                anchor(2, 10.0, 0.0),
            ],
            false,
        );
        let before = document.path(id).expect("exists");
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 1)], AnchorKind::Symmetric)
            .expect("convert");
        let after = document.path(id).expect("exists");
        assert_eq!(before, after, "no-op: nothing changed, not even a reset");
    }

    /// The same no-op rule for `Corner`→`Corner` and `Asymmetric`→
    /// `Asymmetric`.
    #[test]
    fn convert_corner_to_corner_and_asymmetric_to_asymmetric_are_no_ops() {
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
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Asymmetric)
            .expect("convert to asymmetric");
        let after_first = document.path(id).expect("exists");

        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Asymmetric)
            .expect("re-convert to asymmetric is a no-op");
        assert_eq!(document.path(id).expect("exists"), after_first);

        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 1)], AnchorKind::Corner)
            .expect("corner to corner is a no-op");
        assert_eq!(
            document.path(id).expect("exists").anchors[0].kind,
            AnchorKind::Corner
        );
    }

    /// Acceptance criterion 2: Corner → Asymmetric sets both handles
    /// collinear through the anchor along the tangent, keeping each
    /// side's own current length when it was already non-zero.
    #[test]
    fn convert_corner_to_asymmetric_keeps_each_sides_own_nonzero_length() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(4.0, 0.0),
                    kind: AnchorKind::Corner,
                },
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Asymmetric)
            .expect("convert");
        let snapshot = document.path(id).expect("exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Asymmetric);
        // Collinear through the anchor.
        assert_eq!(
            middle.handle_in,
            middle
                .handle_out
                .negated()
                .normalized_to(middle.handle_in.length())
        );
        // Each side keeps its own original (different) length.
        assert!((middle.handle_out.length() - 4.0).abs() < 1e-9);
        assert!((middle.handle_in.length() - 2.0).abs() < 1e-9);
    }

    /// Same conversion, but a side that started at zero length gets the
    /// slice's default length instead of staying zero.
    #[test]
    fn convert_corner_to_asymmetric_defaults_a_zero_length_side() {
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
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Asymmetric)
            .expect("convert");
        let middle = &document.path(id).expect("exists").anchors[1];
        assert!((middle.handle_out.length() - DEFAULT_HANDLE_LENGTH_MM).abs() < 1e-9);
        assert!((middle.handle_in.length() - DEFAULT_HANDLE_LENGTH_MM).abs() < 1e-9);
    }

    /// Acceptance criterion 4: Symmetric → Asymmetric and Asymmetric →
    /// Corner both leave the handles exactly where they are — shape-
    /// preserving, kind-only.
    #[test]
    fn convert_symmetric_to_asymmetric_and_asymmetric_to_corner_preserve_handles() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                smooth_anchor(1, 0.0, 0.0, Vec2::new(5.0, 0.0)),
                anchor(2, 10.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 1)], AnchorKind::Asymmetric)
            .expect("convert to asymmetric");
        let snapshot = document.path(id).expect("exists");
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Asymmetric);
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(5.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-5.0, 0.0));

        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 1)], AnchorKind::Corner)
            .expect("convert to corner");
        let snapshot = document.path(id).expect("exists");
        assert_eq!(snapshot.anchors[0].kind, AnchorKind::Corner);
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(5.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-5.0, 0.0));
    }

    /// Acceptance criterion 5: Asymmetric → Symmetric discards any
    /// independent lengths and resets both handles to the default
    /// length, collinear through the tangent.
    #[test]
    fn convert_asymmetric_to_symmetric_resets_to_default_length() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                anchor(1, 0.0, 0.0),
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(4.0, 0.0),
                    kind: AnchorKind::Asymmetric,
                },
                anchor(3, 20.0, 0.0),
            ],
            false,
        );
        document
            .convert_anchor_kind(id, &[AnchorId::new(1, 2)], AnchorKind::Symmetric)
            .expect("convert");
        let middle = &document.path(id).expect("exists").anchors[1];
        assert_eq!(middle.kind, AnchorKind::Symmetric);
        assert_eq!(middle.handle_in, middle.handle_out.negated());
        assert!((middle.handle_out.length() - DEFAULT_HANDLE_LENGTH_MM).abs() < 1e-9);
    }

    /// Acceptance criterion 3: dragging one handle of an Asymmetric
    /// anchor rotates the opposite handle to stay collinear, without
    /// changing the opposite's own length.
    #[test]
    fn asymmetric_opposite_rotates_without_changing_its_own_length() {
        assert_eq!(
            asymmetric_opposite(Vec2::new(0.0, 3.0), Vec2::new(5.0, 0.0)),
            Vec2::new(0.0, -5.0),
            "rotated to the dragged handle's new angle, opposite's own length (5) kept"
        );
    }

    /// Edge case: dragging to the zero vector leaves the opposite handle
    /// unchanged — there is no direction to follow.
    #[test]
    fn asymmetric_opposite_of_a_zero_drag_leaves_the_opposite_handle_unchanged() {
        let opposite = Vec2::new(3.0, 4.0);
        assert_eq!(asymmetric_opposite(Vec2::ZERO, opposite), opposite);
    }

    /// Edge case: an opposite handle that was already zero stays zero
    /// regardless of the dragged handle's new angle.
    #[test]
    fn asymmetric_opposite_of_a_zero_length_opposite_stays_zero() {
        assert_eq!(
            asymmetric_opposite(Vec2::new(1.0, 1.0), Vec2::ZERO),
            Vec2::ZERO
        );
    }

    #[test]
    fn set_handle_on_an_asymmetric_anchor_rotates_but_does_not_rescale_the_opposite() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let id = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Asymmetric,
                },
                anchor(2, 20.0, 0.0),
            ],
            false,
        );
        let _ = b;
        document
            .set_handle(id, a, HandleSlot::Out, Vec2::new(0.0, 7.0))
            .expect("set handle");
        let snapshot = document.path(id).expect("exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(0.0, 7.0));
        // Opposite rotated to stay collinear (opposite direction from the
        // dragged handle), but kept its own length (2).
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(0.0, -2.0));
    }

    // --- Join (acceptance criteria 8-11) ---

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

    // --- Split (acceptance criteria 12-15) ---

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
        let meta = tree.get_meta(tree_id_of(id)).expect("meta");
        meta.insert(path_codec::KEY_STROKE_WIDTH, 3.0)
            .expect("set width");
        document.commit_with_label("test setup");

        let (_, (second_path, _)) = document
            .split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(9, 1))
            .expect("split");
        let second = document.path(second_path).expect("exists");
        assert!((second.stroke_width.as_mm() - 3.0).abs() < 1e-9);
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
