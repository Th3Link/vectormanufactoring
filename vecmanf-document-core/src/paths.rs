//! Loro CRDT wiring for the path/anchor schema
//! (`specs/path-node-editing/adrs.md`, "the anchor schema, and the three
//! merge choices inside it"). [`crate::path_model`] defines the pure data
//! types a caller builds or reads; this module is the only place that
//! translates them to and from Loro containers, so no Loro type crosses
//! out of this crate's public API (ADR 0004 §3).
//!
//! On-disk/in-CRDT shape, one Loro tree node per path (ADR 0002 §5):
//!
//! ```text
//! meta map:
//!   closed       : bool                     LWW register
//!   stroke_width : f64 (mm)                  LWW register
//!   stroke       : [r, g, b] (i64 each)      LWW register
//!   anchors      : movable list (ADR 0009 §3), each element a map:
//!     id         : hex string of the AnchorId's u128   written once
//!     point      : [x, y] (f64)              ONE LWW register
//!     handle_in  : [x, y] (f64), relative     ONE LWW register
//!     handle_out : [x, y] (f64), relative     ONE LWW register
//!     kind       : "corner" | "smooth"        LWW register
//! ```
//!
//! `fill` is not stored: this slice's fill is always `None` (acceptance
//! criterion 6), so there is nothing to persist yet — `PathSnapshot::fill`
//! reads back as `None` unconditionally until `stroke-and-fill-styling`
//! (slice 4) gives it a value to write.

use loro::{
    Container, LoroMap, LoroMovableList, LoroTree, LoroValue, TreeID, TreeParentId,
    ValueOrContainer,
};

use crate::document::{Document, PATHS_TREE};
use crate::path_model::{
    AnchorId, AnchorKind, AnchorSnapshot, Color, HandleSlot, NewAnchor, NodeId, PathEditError,
    PathSnapshot,
};
use crate::units::{Length, Point, Vec2};

const KEY_CLOSED: &str = "closed";
const KEY_STROKE_WIDTH: &str = "stroke_width";
const KEY_STROKE: &str = "stroke";
const KEY_ANCHORS: &str = "anchors";
const KEY_ID: &str = "id";
const KEY_POINT: &str = "point";
const KEY_HANDLE_IN: &str = "handle_in";
const KEY_HANDLE_OUT: &str = "handle_out";
const KEY_KIND: &str = "kind";

const KIND_CORNER: &str = "corner";
const KIND_SMOOTH: &str = "smooth";

/// This slice's one placeholder stroke width (acceptance criterion 6).
const DEFAULT_STROKE_WIDTH_MM: f64 = 0.25;

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
        let tree = self.loro().get_tree(PATHS_TREE);
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
        NodeId::from_parts(tree_id.peer, tree_id.counter)
    }

    /// Every path object's identity, in sibling (z-)order (ADR 0002 §5) —
    /// never an array offset a caller may rely on staying stable.
    #[must_use]
    pub fn path_ids(&self) -> Vec<NodeId> {
        let tree = self.loro().get_tree(PATHS_TREE);
        tree.roots()
            .into_iter()
            .map(|id| NodeId::from_parts(id.peer, id.counter))
            .collect()
    }

    /// Reads one path's full current data, or `None` if it no longer
    /// exists (deleted locally, or by a collaborator, since the caller
    /// last read a snapshot — ADR 0009 §2).
    #[must_use]
    pub fn path(&self, id: NodeId) -> Option<PathSnapshot> {
        let tree = self.loro().get_tree(PATHS_TREE);
        let tree_id = tree_id_of(id);
        if !node_exists(&tree, tree_id) {
            return None;
        }
        let meta = tree.get_meta(tree_id).ok()?;
        Some(read_path_snapshot(id, &meta))
    }

    /// Moves every named anchor to its new absolute position, one commit
    /// for the whole drag — handles are untouched because they are stored
    /// relative to their own anchor (`specs/path-node-editing/adrs.md`
    /// decision 2; acceptance criteria 8, 10).
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
        for &(anchor_id, point) in moves {
            let index = anchor_index(&anchors, anchor_id)?;
            write_point(&anchor_map_at(&anchors, index), KEY_POINT, point);
        }
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
        Ok(())
    }

    /// Converts one anchor between [`AnchorKind::Corner`] and
    /// [`AnchorKind::Smooth`] (acceptance criterion 11).
    ///
    /// Corner → smooth pulls out two handles of equal default length,
    /// collinear through the anchor along the chord between its
    /// neighbours. Smooth → corner leaves both handles exactly where they
    /// are and only flips `kind` — geometry cannot tell the two apart,
    /// which is exactly why `kind` is a stored field
    /// (`specs/path-node-editing/adrs.md` decision 3).
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if
    /// `path` or `anchor` no longer exists.
    pub fn convert_anchor_kind(
        &self,
        path: NodeId,
        anchor: AnchorId,
        kind: AnchorKind,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let index = anchor_index(&anchors, anchor)?;
        let map = anchor_map_at(&anchors, index);
        if kind == AnchorKind::Smooth {
            let closed = read_closed(&meta);
            let point = read_point(&map, KEY_POINT);
            let tangent = neighbour_tangent(&anchors, closed, index, point)
                .normalized_to(DEFAULT_SMOOTH_HANDLE_LENGTH_MM);
            write_vec2(&map, KEY_HANDLE_OUT, tangent);
            write_vec2(&map, KEY_HANDLE_IN, tangent.negated());
        }
        write_kind(&map, kind);
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
            let tree = self.loro().get_tree(PATHS_TREE);
            let tree_id = tree_id_of(path);
            // invariant: `path_parts` above already confirmed this node
            // exists.
            #[allow(clippy::unwrap_used)]
            tree.delete(tree_id).unwrap();
            return Ok(());
        }

        // Delete from the highest index down so earlier indices stay valid.
        for index in indices.into_iter().rev() {
            // invariant: `index` was found in this same list just above.
            #[allow(clippy::unwrap_used)]
            anchors.delete(index, 1).unwrap();
        }
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
        Ok(())
    }

    /// Looks up one path's meta map and anchors movable list together, so
    /// callers that need both (e.g. the `closed` flag for wraparound) read
    /// them from the same lookup.
    fn path_parts(&self, path: NodeId) -> Result<(LoroMap, LoroMovableList), PathEditError> {
        let tree = self.loro().get_tree(PATHS_TREE);
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

/// Whether a path's tree node is live — never created, or created and then
/// deleted, both read as "does not exist" here.
///
/// `LoroTree::contains` alone is not enough: it answers "was this id ever a
/// real node", which stays `true` for a node this crate has since deleted
/// (Loro keeps a tombstone rather than purging it, for the CRDT merge to
/// stay well-defined against a concurrent remote edit of the same node).
/// `is_node_deleted` answers the question this crate actually needs.
fn node_exists(tree: &LoroTree, id: TreeID) -> bool {
    tree.contains(id) && matches!(tree.is_node_deleted(&id), Ok(false))
}

fn write_path_fields(meta: &LoroMap, closed: bool) {
    // invariant: inserting known-valid keys into a freshly created, empty
    // meta map cannot fail.
    #[allow(clippy::unwrap_used)]
    {
        meta.insert(KEY_CLOSED, closed).unwrap();
        meta.insert(KEY_STROKE_WIDTH, DEFAULT_STROKE_WIDTH_MM)
            .unwrap();
        meta.insert(KEY_STROKE, color_to_value(Color::BLACK))
            .unwrap();
    }
}

fn color_to_value(color: Color) -> Vec<i64> {
    vec![i64::from(color.r), i64::from(color.g), i64::from(color.b)]
}

fn read_closed(meta: &LoroMap) -> bool {
    matches!(
        meta.get(KEY_CLOSED).map(|v| v.get_deep_value()),
        Some(LoroValue::Bool(true))
    )
}

fn read_stroke_width(meta: &LoroMap) -> Length {
    match meta.get(KEY_STROKE_WIDTH).map(|v| v.get_deep_value()) {
        Some(LoroValue::Double(mm)) => Length::from_mm(mm),
        _ => Length::from_mm(DEFAULT_STROKE_WIDTH_MM),
    }
}

fn read_stroke(meta: &LoroMap) -> Color {
    match meta.get(KEY_STROKE).map(|v| v.get_deep_value()) {
        Some(LoroValue::List(list)) if list.len() == 3 => Color {
            r: as_u8(&list[0]),
            g: as_u8(&list[1]),
            b: as_u8(&list[2]),
        },
        _ => Color::BLACK,
    }
}

fn as_u8(value: &LoroValue) -> u8 {
    match value {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        LoroValue::I64(n) => (*n).clamp(0, 255) as u8,
        _ => 0,
    }
}

/// Fetches the already-attached `anchors` movable list from a path's meta
/// map.
///
/// # Panics
/// Does not panic in practice: `create_path` always inserts this field
/// before the path is reachable by any other method, and no method here
/// ever removes it short of deleting the whole path node.
fn anchors_container(meta: &LoroMap) -> LoroMovableList {
    // invariant: see above.
    #[allow(clippy::unwrap_used)]
    let value = meta.get(KEY_ANCHORS).unwrap();
    match value {
        ValueOrContainer::Container(Container::MovableList(list)) => list,
        _ => panic!("invariant violated: `anchors` field is not a movable list"),
    }
}

fn insert_anchors_container(meta: &LoroMap) -> LoroMovableList {
    // invariant: inserting a brand-new container under a fresh key on a
    // freshly created meta map cannot fail.
    #[allow(clippy::unwrap_used)]
    meta.insert_container(KEY_ANCHORS, LoroMovableList::new())
        .unwrap()
}

/// Fetches the anchor map at `index`.
///
/// # Panics
/// Does not panic in practice: every call site obtains `index` from this
/// same list (via [`anchor_index`] or a `0..len()` loop) immediately
/// beforehand.
fn anchor_map_at(anchors: &LoroMovableList, index: usize) -> LoroMap {
    match anchors.get(index) {
        Some(ValueOrContainer::Container(Container::Map(map))) => map,
        _ => panic!("invariant violated: anchor list element is not a map"),
    }
}

fn anchor_index(anchors: &LoroMovableList, id: AnchorId) -> Result<usize, PathEditError> {
    for index in 0..anchors.len() {
        if read_anchor_id(&anchor_map_at(anchors, index)) == Some(id) {
            return Ok(index);
        }
    }
    Err(PathEditError::NoSuchAnchor)
}

/// Resolves two anchor ids to the `(earlier, later)` pair of indices of a
/// path-adjacent segment between them — earlier being the one whose
/// `handle_out` faces the segment, later the one whose `handle_in` does.
fn adjacent_segment_indices(
    anchors: &LoroMovableList,
    closed: bool,
    a: AnchorId,
    b: AnchorId,
) -> Result<(usize, usize), PathEditError> {
    let index_a = anchor_index(anchors, a)?;
    let index_b = anchor_index(anchors, b)?;
    let len = anchors.len();
    if index_a + 1 == index_b {
        return Ok((index_a, index_b));
    }
    if index_b + 1 == index_a {
        return Ok((index_b, index_a));
    }
    if closed && len > 2 {
        if index_a == 0 && index_b == len - 1 {
            return Ok((index_b, index_a));
        }
        if index_b == 0 && index_a == len - 1 {
            return Ok((index_a, index_b));
        }
    }
    Err(PathEditError::NotAnAdjacentSegment)
}

/// The normalized chord direction through the anchor at `index`, from its
/// previous neighbour to its next neighbour (wrapping when `closed`), for
/// acceptance criterion 11's corner→smooth conversion. Degenerates to the
/// one available neighbour's chord at an open path's endpoint, and to the
/// zero vector for a path with no neighbours at all.
fn neighbour_tangent(anchors: &LoroMovableList, closed: bool, index: usize, point: Point) -> Vec2 {
    let len = anchors.len();
    let prev_index = if index > 0 {
        Some(index - 1)
    } else if closed && len > 1 {
        Some(len - 1)
    } else {
        None
    };
    let next_index = if index + 1 < len {
        Some(index + 1)
    } else if closed && len > 1 {
        Some(0)
    } else {
        None
    };
    let prev_point = prev_index.map(|i| read_point(&anchor_map_at(anchors, i), KEY_POINT));
    let next_point = next_index.map(|i| read_point(&anchor_map_at(anchors, i), KEY_POINT));
    match (prev_point, next_point) {
        (Some(prev), Some(next)) => prev.vector_to(next),
        (Some(prev), None) => prev.vector_to(point),
        (None, Some(next)) => point.vector_to(next),
        (None, None) => Vec2::ZERO,
    }
}

fn read_anchor_id(map: &LoroMap) -> Option<AnchorId> {
    match map.get(KEY_ID)?.get_deep_value() {
        LoroValue::String(s) => u128::from_str_radix(s.as_str(), 16)
            .ok()
            .map(AnchorId::from_u128),
        _ => None,
    }
}

fn anchor_id_to_value(id: AnchorId) -> String {
    let raw = id.as_u128();
    format!("{raw:032x}")
}

fn read_xy(map: &LoroMap, key: &str) -> Option<(f64, f64)> {
    match map.get(key)?.get_deep_value() {
        LoroValue::List(list) if list.len() == 2 => Some((as_f64(&list[0])?, as_f64(&list[1])?)),
        _ => None,
    }
}

fn as_f64(value: &LoroValue) -> Option<f64> {
    match value {
        LoroValue::Double(n) => Some(*n),
        #[allow(clippy::cast_precision_loss)]
        LoroValue::I64(n) => Some(*n as f64),
        _ => None,
    }
}

fn read_point(map: &LoroMap, key: &str) -> Point {
    read_xy(map, key).map_or(Point::new(0.0, 0.0), |(x, y)| Point::new(x, y))
}

fn read_vec2(map: &LoroMap, key: &str) -> Vec2 {
    read_xy(map, key).map_or(Vec2::ZERO, |(x, y)| Vec2::new(x, y))
}

fn write_point(map: &LoroMap, key: &str, point: Point) {
    // invariant: inserting a plain value under a known key on an attached
    // map cannot fail.
    #[allow(clippy::unwrap_used)]
    map.insert(key, vec![point.x, point.y]).unwrap();
}

fn write_vec2(map: &LoroMap, key: &str, v: Vec2) {
    // invariant: see `write_point`.
    #[allow(clippy::unwrap_used)]
    map.insert(key, vec![v.x, v.y]).unwrap();
}

fn read_kind(map: &LoroMap) -> AnchorKind {
    match map.get(KEY_KIND).map(|v| v.get_deep_value()) {
        Some(LoroValue::String(s)) if s.as_str() == KIND_SMOOTH => AnchorKind::Smooth,
        _ => AnchorKind::Corner,
    }
}

fn write_kind(map: &LoroMap, kind: AnchorKind) {
    // invariant: see `write_point`.
    #[allow(clippy::unwrap_used)]
    map.insert(KEY_KIND, kind_to_str(kind)).unwrap();
}

const fn kind_to_str(kind: AnchorKind) -> &'static str {
    match kind {
        AnchorKind::Corner => KIND_CORNER,
        AnchorKind::Smooth => KIND_SMOOTH,
    }
}

fn read_anchor_snapshot(map: &LoroMap) -> AnchorSnapshot {
    AnchorSnapshot {
        // invariant: `write_anchor_fields` always sets `id` before this
        // map is ever reachable from `path()`.
        #[allow(clippy::unwrap_used)]
        id: read_anchor_id(map).unwrap(),
        point: read_point(map, KEY_POINT),
        handle_in: read_vec2(map, KEY_HANDLE_IN),
        handle_out: read_vec2(map, KEY_HANDLE_OUT),
        kind: read_kind(map),
    }
}

fn write_anchor_fields(map: &LoroMap, anchor: &NewAnchor) {
    // invariant: inserting plain values under known keys on an attached
    // map cannot fail.
    #[allow(clippy::unwrap_used)]
    {
        map.insert(KEY_ID, anchor_id_to_value(anchor.id)).unwrap();
        map.insert(KEY_KIND, kind_to_str(anchor.kind)).unwrap();
    }
    write_point(map, KEY_POINT, anchor.point);
    write_vec2(map, KEY_HANDLE_IN, anchor.handle_in);
    write_vec2(map, KEY_HANDLE_OUT, anchor.handle_out);
}

fn push_anchor(list: &LoroMovableList, anchor: &NewAnchor) {
    // invariant: push_container onto a freshly obtained, attached movable
    // list cannot fail.
    #[allow(clippy::unwrap_used)]
    let map = list.push_container(LoroMap::new()).unwrap();
    write_anchor_fields(&map, anchor);
}

fn insert_anchor_at(list: &LoroMovableList, index: usize, anchor: &NewAnchor) {
    // invariant: `index` is always `after_index + 1` for an `after` index
    // `anchor_index` just confirmed is in `0..=len()`, so it is always a
    // valid insertion point.
    #[allow(clippy::unwrap_used)]
    let map = list.insert_container(index, LoroMap::new()).unwrap();
    write_anchor_fields(&map, anchor);
}

fn read_path_snapshot(id: NodeId, meta: &LoroMap) -> PathSnapshot {
    let anchors_list = anchors_container(meta);
    let anchors = (0..anchors_list.len())
        .map(|index| read_anchor_snapshot(&anchor_map_at(&anchors_list, index)))
        .collect();
    PathSnapshot {
        id,
        closed: read_closed(meta),
        stroke_width: read_stroke_width(meta),
        stroke: read_stroke(meta),
        fill: None,
        anchors,
    }
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
    fn path_ids_lists_every_created_path_in_order() {
        let document = Document::new(1);
        let first = document.create_path(&[anchor(1, 0.0, 0.0), anchor(2, 1.0, 0.0)], false);
        let second = document.create_path(&[anchor(3, 0.0, 0.0), anchor(4, 1.0, 0.0)], false);
        assert_eq!(document.path_ids(), vec![first, second]);
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
            .convert_anchor_kind(id, AnchorId::new(1, 2), AnchorKind::Smooth)
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
            .convert_anchor_kind(id, AnchorId::new(1, 1), AnchorKind::Corner)
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
        assert_eq!(document.path_ids(), Vec::new());
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
