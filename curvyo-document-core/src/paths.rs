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
    write_kind, write_point, write_vec2,
};
use crate::path_model::{
    AnchorId, AnchorKind, Color, HandleSlot, NewAnchor, NodeId, PathEditError, PathSnapshot,
};
use crate::units::{Length, Point, Vec2};

/// Default handle length a corner→symmetric/asymmetric conversion pulls
/// out (`specs/0002-path-node-editing/specification.md` acceptance
/// criterion 11; no acceptance criterion pins an exact value). Renamed
/// from `DEFAULT_SMOOTH_HANDLE_LENGTH_MM`
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`: "the wasm
/// binding string... is not persisted and is renamed outright.
/// `DEFAULT_SMOOTH_HANDLE_LENGTH_MM` becomes `DEFAULT_HANDLE_LENGTH_MM`").
const DEFAULT_HANDLE_LENGTH_MM: f64 = 10.0;

/// Below this length, a handle counts as "no existing length to keep" for
/// acceptance criterion 2's "its own current length, if that side already
/// had a non-zero handle; otherwise the slice's existing default handle
/// length" — not a geometric [`crate::units::Tolerance`] (`CLAUDE.md` §5):
/// this is a plain zero/non-zero classification of a stored value, the
/// same kind of exact check `specs/0002-path-node-editing/adrs.md` already
/// uses for "a retracted handle is the exact zero vector".
const ZERO_HANDLE_EPSILON: f64 = f64::EPSILON;

/// Default fraction of the segment chord a "make curve" extends its two
/// adjoining handles by (acceptance criterion 14; a standard Bézier
/// approximation fraction, not a value any acceptance criterion pins).
const DEFAULT_CURVE_HANDLE_FRACTION: f64 = 1.0 / 3.0;

/// Resolves what an anchor's whole `(handle_in, handle_out)` pair becomes
/// when `slot`'s handle is set to `value` — acceptance criterion 9's one
/// rule (mirror for [`AnchorKind::Symmetric`], touch only the named handle
/// for [`AnchorKind::Corner`]), as a single pure function rather than
/// logic duplicated at each of its two callers. [`Document::set_handle`]
/// calls this to build what it writes; `curvyo-ui-core`'s live handle-
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
            crate::units::Angle::from_radians(0.0),
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
    /// for the whole drag, whether every anchor shares one path or `moves`
    /// spans several — a multi-path node drag is one commit, exactly like
    /// a single-path one (`specs/0006-path-merge-split-and-node-types/
    /// adrs.md`'s "one Node-tool session over several paths": "a
    /// multi-path node drag is one commit") — handles are untouched
    /// because they are stored relative to their own anchor
    /// (`specs/0002-path-node-editing/adrs.md` decision 2; acceptance
    /// criteria 8, 10).
    ///
    /// Resolves every `(path, anchor)` pair to its container and index
    /// *before* writing any of them, so a refused move (one stale id
    /// anywhere in `moves`, on any of the paths involved) never leaves any
    /// path half-moved.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] if any named path no longer exists;
    /// [`PathEditError::NoSuchAnchor`] if any named anchor no longer
    /// exists.
    pub fn move_anchors(&self, moves: &[(NodeId, AnchorId, Point)]) -> Result<(), PathEditError> {
        let resolved: Vec<(LoroMovableList, usize, Point)> = moves
            .iter()
            .map(|&(path, anchor_id, point)| {
                let (_, anchors) = self.path_parts(path)?;
                let index = anchor_index(&anchors, anchor_id)?;
                Ok((anchors, index, point))
            })
            .collect::<Result<_, PathEditError>>()?;
        for (anchors, index, point) in &resolved {
            write_point(&anchor_map_at(anchors, *index), KEY_POINT, *point);
        }
        self.commit_with_label("move_anchors");
        Ok(())
    }

    /// Resizes a path by writing every named anchor's new point and
    /// handle vectors, plus the stroke width, together as **one commit**
    /// (`specs/0005-object-transform/adrs.md`'s resize-writes table:
    /// "anchors, `stroke_width`" — acceptance criteria 12, 8).
    /// `curvyo-ui-core` computes every value (via
    /// [`crate::path_model::PathSnapshot::scaled`] plus its own stroke-
    /// factor arithmetic) before calling this; this method only writes
    /// what was computed, resolving every named anchor before writing
    /// any of them so one stale id refuses the whole call.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] if `path` no longer exists;
    /// [`PathEditError::NoSuchAnchor`] if any named anchor no longer
    /// exists.
    pub fn resize_path(
        &self,
        path: NodeId,
        anchors: &[(AnchorId, Point, Vec2, Vec2)],
        stroke_width: Option<Length>,
    ) -> Result<(), PathEditError> {
        let (meta, anchor_list) = self.path_parts(path)?;
        let resolved: Vec<(usize, Point, Vec2, Vec2)> = anchors
            .iter()
            .map(|&(id, point, handle_in, handle_out)| {
                let index = anchor_index(&anchor_list, id)?;
                Ok((index, point, handle_in, handle_out))
            })
            .collect::<Result<_, PathEditError>>()?;
        for (index, point, handle_in, handle_out) in resolved {
            let map = anchor_map_at(&anchor_list, index);
            write_point(&map, KEY_POINT, point);
            write_vec2(&map, KEY_HANDLE_IN, handle_in);
            write_vec2(&map, KEY_HANDLE_OUT, handle_out);
        }
        crate::shapes::write_stroke_width_if_changed(&meta, stroke_width);
        self.commit_with_label("resize_path");
        Ok(())
    }

    /// Sets one anchor's handle, mirroring the opposite handle when the
    /// anchor is [`AnchorKind::Symmetric`] (`handle_in = -handle_out`),
    /// rotating the opposite handle to stay collinear *without* changing
    /// its own length when the anchor is [`AnchorKind::Asymmetric`]
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`'s rule
    /// table; acceptance criterion 3), and touching only the named
    /// handle when it is [`AnchorKind::Corner`]
    /// (`specs/0002-path-node-editing/adrs.md` decision 1; acceptance
    /// criterion 9).
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

    /// [`Document::create_path`]'s own body, minus the commit — shared
    /// with `crate::path_topology`'s Split (`split_open_path` there),
    /// which needs the new object's creation folded into Split's own
    /// single commit (`specs/0002-path-node-editing/adrs.md`'s "each
    /// mutating method ends in exactly one Loro commit") rather than a
    /// second, separate one. Also takes an explicit stroke width/color
    /// rather than always writing the placeholder default, so Split can
    /// copy the original path's own style instead of resetting it.
    /// `pub(crate)` for `path_topology` to call.
    ///
    /// # Panics
    /// Does not panic in practice — see [`Document::create_path`]'s own
    /// doc comment.
    /// `rotation` defaults to zero at every call site except
    /// `crate::path_topology::split_open_path`, which passes the
    /// original path's own `rotation` (`specs/0005-object-transform/
    /// adrs.md`, architect note: "`Document::split_at_anchor`'s new
    /// object (open path) copies the original's rotation").
    pub(crate) fn create_path_uncommitted(
        &self,
        anchors: &[NewAnchor],
        closed: bool,
        stroke_width_mm: f64,
        stroke: Color,
        rotation: crate::units::Angle,
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
        if rotation.as_radians() != 0.0 {
            path_codec::write_rotation(&meta, rotation);
        }
        let anchor_list = insert_anchors_container(&meta);
        for anchor in anchors {
            push_anchor(&anchor_list, anchor);
        }
        NodeId::from_parts(tree_id.peer, tree_id.counter)
    }

    /// Looks up one path's meta map and anchors movable list together, so
    /// callers that need both (e.g. the `closed` flag for wraparound) read
    /// them from the same lookup. `pub(crate)` for `crate::path_topology`
    /// (Join/Split) to call too.
    pub(crate) fn path_parts(
        &self,
        path: NodeId,
    ) -> Result<(LoroMap, LoroMovableList), PathEditError> {
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

/// `pub(crate)` for `crate::path_topology` (Join/Split) to call too.
pub(crate) fn tree_id_of(id: NodeId) -> TreeID {
    TreeID::new(id.peer, id.counter)
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
            .move_anchors(&[(id, AnchorId::new(1, 1), Point::new(3.0, 4.0))])
            .expect("move");
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(3.0, 4.0));
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(5.0, 0.0));
    }

    #[test]
    fn move_anchors_on_a_stale_anchor_is_refused() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        let result = document.move_anchors(&[(id, AnchorId::new(9, 9), Point::new(0.0, 0.0))]);
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
    }

    /// A refused batch move must not be half-applied: every id is
    /// resolved before any point is written, so a valid id earlier in the
    /// list stays untouched when a later id in the same call is stale.
    #[test]
    fn move_anchors_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let result = document.move_anchors(&[
            (id, AnchorId::new(1, 1), Point::new(99.0, 99.0)),
            (id, AnchorId::new(9, 9), Point::new(0.0, 0.0)),
        ]);
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        let snapshot = document.path(id).expect("path exists");
        assert_eq!(
            snapshot.anchors[0].point,
            Point::new(0.0, 0.0),
            "the valid id earlier in the batch must not have moved either"
        );
    }

    /// The generalization this slice's architect review requires: `moves`
    /// can span two different path objects and still lands as one commit
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`'s "a
    /// multi-path node drag is one commit").
    #[test]
    fn move_anchors_moves_anchors_across_two_paths_in_one_commit() {
        let document = Document::new(1);
        let first = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let second = document.create_path(&[anchor(3, 0.0, 5.0), anchor(4, 10.0, 5.0)], false);
        let before = document.loro().len_changes();
        document
            .move_anchors(&[
                (first, AnchorId::new(1, 1), Point::new(1.0, 1.0)),
                (second, AnchorId::new(1, 3), Point::new(2.0, 2.0)),
            ])
            .expect("cross-path move");
        let after = document.loro().len_changes();
        assert_eq!(
            after - before,
            1,
            "one commit for the whole cross-path drag"
        );
        assert_eq!(
            document.path(first).expect("first path exists").anchors[0].point,
            Point::new(1.0, 1.0)
        );
        assert_eq!(
            document.path(second).expect("second path exists").anchors[0].point,
            Point::new(2.0, 2.0)
        );
    }

    /// Same half-applied guarantee as the single-path case above, but with
    /// the stale id on a *different* path than the valid one.
    #[test]
    fn move_anchors_refuses_the_whole_cross_path_batch_on_one_stale_id() {
        let document = Document::new(1);
        let first = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let second = document.create_path(&[anchor(3, 0.0, 5.0), anchor(4, 10.0, 5.0)], false);
        let result = document.move_anchors(&[
            (first, AnchorId::new(1, 1), Point::new(99.0, 99.0)),
            (second, AnchorId::new(9, 9), Point::new(0.0, 0.0)),
        ]);
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        assert_eq!(
            document.path(first).expect("first path exists").anchors[0].point,
            Point::new(0.0, 0.0),
            "the valid id on the first path must not have moved either"
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

    /// Acceptance criteria 12, 8: `resize_path` writes every named
    /// anchor's point and handles plus the stroke width, all in one
    /// commit.
    #[test]
    fn resize_path_writes_anchors_and_stroke_width_in_one_commit() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let before = document.loro().len_changes();
        document
            .resize_path(
                id,
                &[
                    (a, Point::new(0.0, 0.0), Vec2::ZERO, Vec2::new(1.0, 0.0)),
                    (b, Point::new(20.0, 0.0), Vec2::new(-1.0, 0.0), Vec2::ZERO),
                ],
                Some(Length::from_mm(0.5)),
            )
            .expect("resize");
        let after = document.loro().len_changes();
        assert_eq!(after - before, 1, "one commit");
        let snapshot = document.path(id).expect("exists");
        assert_eq!(snapshot.anchors[1].point, Point::new(20.0, 0.0));
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(1.0, 0.0));
        assert!((snapshot.stroke_width.as_mm() - 0.5).abs() < 1e-9);
    }

    /// A refused `resize_path` (one stale anchor id) must not leave the
    /// path half-resized.
    #[test]
    fn resize_path_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let id = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 10.0, 0.0)], false);
        let result = document.resize_path(
            id,
            &[
                (a, Point::new(99.0, 99.0), Vec2::ZERO, Vec2::ZERO),
                (
                    AnchorId::new(9, 9),
                    Point::new(0.0, 0.0),
                    Vec2::ZERO,
                    Vec2::ZERO,
                ),
            ],
            Some(Length::from_mm(1.0)),
        );
        assert_eq!(result, Err(PathEditError::NoSuchAnchor));
        let snapshot = document.path(id).expect("exists");
        assert_eq!(
            snapshot.anchors[0].point,
            Point::new(0.0, 0.0),
            "untouched by the refused batch"
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
            document.move_anchors(&[(unknown, AnchorId::new(1, 1), Point::new(0.0, 0.0))]),
            Err(PathEditError::NoSuchPath)
        );
    }
}
