//! The Loro value shapes and keys for the path/anchor schema
//! (`specs/path-node-editing/adrs.md`, "the anchor schema, and the three
//! merge choices inside it"). [`crate::paths`] is the only caller: this
//! module owns the on-disk/in-CRDT shape and every read/write against it,
//! so [`crate::paths`]'s `Document` methods stay command-shaped logic with
//! no Loro value-shape details of their own (`CLAUDE.md` §5, "one
//! responsibility per module").
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

use loro::{Container, LoroDoc, LoroMap, LoroMovableList, LoroTree, LoroValue, ValueOrContainer};

use crate::path_model::{
    AnchorId, AnchorKind, AnchorSnapshot, Color, NewAnchor, NodeId, PathSnapshot,
};
use crate::units::{Length, Point, Vec2};

pub(crate) const KEY_CLOSED: &str = "closed";
pub(crate) const KEY_STROKE_WIDTH: &str = "stroke_width";
pub(crate) const KEY_STROKE: &str = "stroke";
pub(crate) const KEY_ANCHORS: &str = "anchors";
pub(crate) const KEY_ID: &str = "id";
pub(crate) const KEY_POINT: &str = "point";
pub(crate) const KEY_HANDLE_IN: &str = "handle_in";
pub(crate) const KEY_HANDLE_OUT: &str = "handle_out";
pub(crate) const KEY_KIND: &str = "kind";

const KIND_CORNER: &str = "corner";
const KIND_SMOOTH: &str = "smooth";

/// This slice's one placeholder stroke width (acceptance criterion 6).
pub(crate) const DEFAULT_STROKE_WIDTH_MM: f64 = 0.25;

/// Whether a path's tree node is live — never created, or created and then
/// deleted, both read as "does not exist" here.
///
/// `LoroTree::contains` alone is not enough: it answers "was this id ever a
/// real node", which stays `true` for a node this crate has since deleted
/// (Loro keeps a tombstone rather than purging it, for the CRDT merge to
/// stay well-defined against a concurrent remote edit of the same node).
/// `is_node_deleted` answers the question this crate actually needs.
pub(crate) fn node_exists(tree: &LoroTree, id: loro::TreeID) -> bool {
    tree.contains(id) && matches!(tree.is_node_deleted(&id), Ok(false))
}

pub(crate) fn write_path_fields(meta: &LoroMap, closed: bool) {
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

pub(crate) fn color_to_value(color: Color) -> Vec<i64> {
    vec![i64::from(color.r), i64::from(color.g), i64::from(color.b)]
}

pub(crate) fn read_closed(meta: &LoroMap) -> bool {
    matches!(
        meta.get(KEY_CLOSED).map(|v| v.get_deep_value()),
        Some(LoroValue::Bool(true))
    )
}

pub(crate) fn read_stroke_width(meta: &LoroMap) -> Length {
    match meta.get(KEY_STROKE_WIDTH).map(|v| v.get_deep_value()) {
        Some(LoroValue::Double(mm)) => Length::from_mm(mm),
        _ => Length::from_mm(DEFAULT_STROKE_WIDTH_MM),
    }
}

pub(crate) fn read_stroke(meta: &LoroMap) -> Color {
    match meta.get(KEY_STROKE).map(|v| v.get_deep_value()) {
        Some(LoroValue::List(list)) if list.len() == 3 => Color {
            r: as_u8(&list[0]),
            g: as_u8(&list[1]),
            b: as_u8(&list[2]),
        },
        _ => Color::BLACK,
    }
}

pub(crate) fn as_u8(value: &LoroValue) -> u8 {
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
/// Does not panic in practice on a document this crate created itself:
/// `create_path` always inserts this field before the path is reachable
/// by any other method, and no method here ever removes it short of
/// deleting the whole path node. A document read back from bytes is not
/// trusted on this alone — [`validate_path_tree`] checks this exact shape
/// for every path node before a freshly opened `Document` is ever handed
/// to a caller, so by the time anything outside this module can reach a
/// path node, this invariant has already been confirmed for it.
pub(crate) fn anchors_container(meta: &LoroMap) -> LoroMovableList {
    // invariant: see above.
    #[allow(clippy::unwrap_used)]
    let value = meta.get(KEY_ANCHORS).unwrap();
    match value {
        ValueOrContainer::Container(Container::MovableList(list)) => list,
        _ => panic!("invariant violated: `anchors` field is not a movable list"),
    }
}

pub(crate) fn insert_anchors_container(meta: &LoroMap) -> LoroMovableList {
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
/// beforehand — and, for a document read back from bytes,
/// [`validate_path_tree`] has already confirmed every element in every
/// path's anchor list is a map (see [`anchors_container`]'s own doc
/// comment).
pub(crate) fn anchor_map_at(anchors: &LoroMovableList, index: usize) -> LoroMap {
    match anchors.get(index) {
        Some(ValueOrContainer::Container(Container::Map(map))) => map,
        _ => panic!("invariant violated: anchor list element is not a map"),
    }
}

pub(crate) fn anchor_index(
    anchors: &LoroMovableList,
    id: AnchorId,
) -> Result<usize, crate::path_model::PathEditError> {
    for index in 0..anchors.len() {
        if read_anchor_id(&anchor_map_at(anchors, index)) == Some(id) {
            return Ok(index);
        }
    }
    Err(crate::path_model::PathEditError::NoSuchAnchor)
}

/// Resolves two anchor ids to the `(earlier, later)` pair of indices of a
/// path-adjacent segment between them — earlier being the one whose
/// `handle_out` faces the segment, later the one whose `handle_in` does.
pub(crate) fn adjacent_segment_indices(
    anchors: &LoroMovableList,
    closed: bool,
    a: AnchorId,
    b: AnchorId,
) -> Result<(usize, usize), crate::path_model::PathEditError> {
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
    Err(crate::path_model::PathEditError::NotAnAdjacentSegment)
}

/// The normalized chord direction through the anchor at `index`, from its
/// previous neighbour to its next neighbour (wrapping when `closed`), for
/// acceptance criterion 11's corner→smooth conversion. Degenerates to the
/// one available neighbour's chord at an open path's endpoint, and to the
/// zero vector for a path with no neighbours at all.
pub(crate) fn neighbour_tangent(
    anchors: &LoroMovableList,
    closed: bool,
    index: usize,
    point: Point,
) -> Vec2 {
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

pub(crate) fn read_anchor_id(map: &LoroMap) -> Option<AnchorId> {
    match map.get(KEY_ID)?.get_deep_value() {
        LoroValue::String(s) => AnchorId::from_hex(s.as_str()),
        _ => None,
    }
}

pub(crate) fn anchor_id_to_value(id: AnchorId) -> String {
    id.to_hex()
}

pub(crate) fn read_xy(map: &LoroMap, key: &str) -> Option<(f64, f64)> {
    match map.get(key)?.get_deep_value() {
        LoroValue::List(list) if list.len() == 2 => Some((as_f64(&list[0])?, as_f64(&list[1])?)),
        _ => None,
    }
}

pub(crate) fn as_f64(value: &LoroValue) -> Option<f64> {
    match value {
        LoroValue::Double(n) => Some(*n),
        #[allow(clippy::cast_precision_loss)]
        LoroValue::I64(n) => Some(*n as f64),
        _ => None,
    }
}

pub(crate) fn read_point(map: &LoroMap, key: &str) -> Point {
    read_xy(map, key).map_or(Point::new(0.0, 0.0), |(x, y)| Point::new(x, y))
}

pub(crate) fn read_vec2(map: &LoroMap, key: &str) -> Vec2 {
    read_xy(map, key).map_or(Vec2::ZERO, |(x, y)| Vec2::new(x, y))
}

pub(crate) fn write_point(map: &LoroMap, key: &str, point: Point) {
    // invariant: inserting a plain value under a known key on an attached
    // map cannot fail.
    #[allow(clippy::unwrap_used)]
    map.insert(key, vec![point.x, point.y]).unwrap();
}

pub(crate) fn write_vec2(map: &LoroMap, key: &str, v: Vec2) {
    // invariant: see `write_point`.
    #[allow(clippy::unwrap_used)]
    map.insert(key, vec![v.x, v.y]).unwrap();
}

pub(crate) fn read_kind(map: &LoroMap) -> AnchorKind {
    match map.get(KEY_KIND).map(|v| v.get_deep_value()) {
        Some(LoroValue::String(s)) if s.as_str() == KIND_SMOOTH => AnchorKind::Smooth,
        _ => AnchorKind::Corner,
    }
}

pub(crate) fn write_kind(map: &LoroMap, kind: AnchorKind) {
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
        // map is ever reachable from `path()`, and `validate_path_tree`
        // confirms it for a document read back from bytes.
        #[allow(clippy::unwrap_used)]
        id: read_anchor_id(map).unwrap(),
        point: read_point(map, KEY_POINT),
        handle_in: read_vec2(map, KEY_HANDLE_IN),
        handle_out: read_vec2(map, KEY_HANDLE_OUT),
        kind: read_kind(map),
    }
}

pub(crate) fn write_anchor_fields(map: &LoroMap, anchor: &NewAnchor) {
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

pub(crate) fn push_anchor(list: &LoroMovableList, anchor: &NewAnchor) {
    // invariant: push_container onto a freshly obtained, attached movable
    // list cannot fail.
    #[allow(clippy::unwrap_used)]
    let map = list.push_container(LoroMap::new()).unwrap();
    write_anchor_fields(&map, anchor);
}

pub(crate) fn insert_anchor_at(list: &LoroMovableList, index: usize, anchor: &NewAnchor) {
    // invariant: `index` is always `after_index + 1` for an `after` index
    // `anchor_index` just confirmed is in `0..=len()`, so it is always a
    // valid insertion point.
    #[allow(clippy::unwrap_used)]
    let map = list.insert_container(index, LoroMap::new()).unwrap();
    write_anchor_fields(&map, anchor);
}

pub(crate) fn read_path_snapshot(id: NodeId, meta: &LoroMap) -> PathSnapshot {
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

/// Validates that every path node in the `paths` tree matches the shape
/// this module's own writers always produce, *without* relying on any of
/// the `// invariant:` comments above that a trusted document gets to
/// lean on (architect review, `specs/path-node-editing/adrs.md`'s PR
/// review: "Opening a `.vmf` validates the path tree before it returns a
/// `Document`"). Called once, right after import, before a freshly opened
/// `Document` is ever handed to a caller — every read helper in this
/// module can then keep trusting its own invariant for the rest of that
/// document's lifetime, instead of re-checking the same shape on every
/// read.
///
/// Checks only the shapes this module's own helpers `.unwrap()` or
/// `panic!()` on an unexpected value — `closed`/`stroke_width`/`stroke`/
/// `point`/`handle_in`/`handle_out`/`kind` all already degrade gracefully
/// to a default on a missing or malformed value (see `read_closed` et
/// al.), so a document with those fields merely absent or odd-shaped is
/// not "damaged", just reverting to defaults, matching slice 1's
/// forward-compatible reading stance.
pub(crate) fn validate_path_tree(loro: &LoroDoc, paths_tree_key: &str) -> bool {
    let tree = loro.get_tree(paths_tree_key);
    tree.roots()
        .into_iter()
        .all(|id| validate_object_node(&tree, id))
}

/// Dispatches one object (root tree node) to path validation when its
/// `shape` tag is absent, or to
/// [`crate::shape_codec::validate_primitive_node`] when present
/// (`specs/primitive-shapes/adrs.md`: "open-file validation dispatches
/// on `shape` alone"). Unknown extra keys on either kind of node are
/// tolerated — see that module's own doc comment for why ("object to
/// path" can leave a stray primitive key behind on a path node under a
/// concurrent edit).
fn validate_object_node(tree: &LoroTree, id: loro::TreeID) -> bool {
    let Ok(meta) = tree.get_meta(id) else {
        return false;
    };
    match crate::shape_codec::read_shape_tag(&meta) {
        Some(shape) => crate::shape_codec::validate_primitive_node(&meta, &shape),
        None => validate_path_node(&meta),
    }
}

fn validate_path_node(meta: &LoroMap) -> bool {
    let Some(ValueOrContainer::Container(Container::MovableList(anchors))) = meta.get(KEY_ANCHORS)
    else {
        return false;
    };
    (0..anchors.len()).all(|index| validate_anchor(&anchors, index))
}

fn validate_anchor(anchors: &LoroMovableList, index: usize) -> bool {
    let Some(ValueOrContainer::Container(Container::Map(map))) = anchors.get(index) else {
        return false;
    };
    read_anchor_id(&map).is_some()
}
