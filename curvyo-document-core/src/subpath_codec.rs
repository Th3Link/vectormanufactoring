//! The Loro value shape of a compound path's further outlines
//! (`specs/0016-boolean-operations/adrs.md`, "compound path = option A").
//!
//! A path's first outline stays in the keys [`crate::path_codec`] owns
//! (`closed`, `anchors`); this module owns the one optional key that holds
//! the others:
//!
//! ```text
//! meta map of a path:
//!   extra_subpaths : movable list, optional       absent = one-outline path
//!     each element a map:
//!       closed  : bool                            LWW register
//!       anchors : movable list of anchor maps     same schema as `anchors`
//! ```
//!
//! A writer never writes an empty list; a reader treats an empty list like an
//! absent one. Every anchor of every outline has its own unique id, so a node
//! can be named by `(NodeId, AnchorId)` without an outline index.

use loro::{Container, LoroMap, LoroMovableList, ValueOrContainer};

use crate::path_codec::{
    KEY_ANCHORS, anchor_index, anchor_map_at, anchors_container, push_anchor, read_anchor_snapshot,
    read_closed, validate_anchor, write_closed,
};
use crate::path_model::{AnchorId, NewAnchor, SubpathSnapshot};

/// The key of the optional list of further outlines.
pub(crate) const KEY_EXTRA_SUBPATHS: &str = "extra_subpaths";

/// The movable list of further outlines, or `None` for an ordinary path.
fn extra_list(meta: &LoroMap) -> Option<LoroMovableList> {
    match meta.get(KEY_EXTRA_SUBPATHS) {
        Some(ValueOrContainer::Container(Container::MovableList(list))) => Some(list),
        _ => None,
    }
}

/// The map of the further outline at `index`.
///
/// # Panics
/// Does not panic in practice: every call site takes `index` from the same
/// list's length, and [`validate_extra_subpaths`] confirmed the element shape
/// for a document read from bytes.
fn subpath_map_at(list: &LoroMovableList, index: usize) -> LoroMap {
    match list.get(index) {
        Some(ValueOrContainer::Container(Container::Map(map))) => map,
        _ => panic!("invariant violated: extra_subpaths element is not a map"),
    }
}

fn subpath_anchors(map: &LoroMap) -> LoroMovableList {
    match map.get(KEY_ANCHORS) {
        Some(ValueOrContainer::Container(Container::MovableList(list))) => list,
        _ => panic!("invariant violated: subpath `anchors` is not a movable list"),
    }
}

/// Every outline's anchor list, the first outline's first, in storage order.
pub(crate) fn all_anchor_lists(meta: &LoroMap) -> Vec<LoroMovableList> {
    let mut lists = vec![anchors_container(meta)];
    if let Some(extra) = extra_list(meta) {
        lists.extend((0..extra.len()).map(|i| subpath_anchors(&subpath_map_at(&extra, i))));
    }
    lists
}

/// The list and index of the anchor `id`, in whichever outline it is.
pub(crate) fn find_anchor(meta: &LoroMap, id: AnchorId) -> Option<(LoroMovableList, usize)> {
    all_anchor_lists(meta)
        .into_iter()
        .find_map(|list| anchor_index(&list, id).ok().map(|index| (list, index)))
}

/// The number of anchors over all outlines.
pub(crate) fn total_anchor_count(meta: &LoroMap) -> usize {
    all_anchor_lists(meta)
        .iter()
        .map(LoroMovableList::len)
        .sum()
}

/// Writes the further outlines of a freshly created path; writes nothing for
/// an empty slice (a writer never writes an empty list).
pub(crate) fn write_extra_subpaths(meta: &LoroMap, outlines: &[(Vec<NewAnchor>, bool)]) {
    if outlines.is_empty() {
        return;
    }
    // invariant: inserting a new container under a fresh key of an attached
    // map, and pushing containers onto the lists just created, cannot fail.
    #[allow(clippy::unwrap_used)]
    {
        let list = meta
            .insert_container(KEY_EXTRA_SUBPATHS, LoroMovableList::new())
            .unwrap();
        for (anchors, closed) in outlines {
            let map = list.push_container(LoroMap::new()).unwrap();
            write_closed(&map, *closed);
            let anchor_list = map
                .insert_container(KEY_ANCHORS, LoroMovableList::new())
                .unwrap();
            for anchor in anchors {
                push_anchor(&anchor_list, anchor);
            }
        }
    }
}

/// The further outlines as a read model; empty for an ordinary path.
pub(crate) fn read_extra_subpaths(meta: &LoroMap) -> Vec<SubpathSnapshot> {
    let Some(extra) = extra_list(meta) else {
        return Vec::new();
    };
    (0..extra.len())
        .map(|i| {
            let map = subpath_map_at(&extra, i);
            let anchors = subpath_anchors(&map);
            SubpathSnapshot {
                closed: read_closed(&map),
                anchors: (0..anchors.len())
                    .map(|k| read_anchor_snapshot(&anchor_map_at(&anchors, k)))
                    .collect(),
            }
        })
        .collect()
}

/// Whether a present `extra_subpaths` has the shape this module writes: a
/// movable list of maps, each with an `anchors` movable list whose elements
/// are anchor maps with a valid id. Absent is valid. Anything else is damage
/// (`OpenError::Damaged`), checked at open so the readers above can trust the
/// shape.
pub(crate) fn validate_extra_subpaths(meta: &LoroMap) -> bool {
    match meta.get(KEY_EXTRA_SUBPATHS) {
        None => true,
        Some(ValueOrContainer::Container(Container::MovableList(list))) => {
            (0..list.len()).all(|i| match list.get(i) {
                Some(ValueOrContainer::Container(Container::Map(map))) => {
                    match map.get(KEY_ANCHORS) {
                        Some(ValueOrContainer::Container(Container::MovableList(anchors))) => {
                            (0..anchors.len()).all(|k| validate_anchor(&anchors, k))
                        }
                        _ => false,
                    }
                }
                _ => false,
            })
        }
        Some(_) => false,
    }
}
