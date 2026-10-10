//! `Document::bend_segment`: the one command a finished segment bend writes
//! (`specs/0031-segment-drag-bending` criteria 12, 18, 19).

use crate::document::Document;
use crate::path_codec::{
    KEY_HANDLE_IN, KEY_HANDLE_OUT, adjacent_segment_indices, anchor_map_at, read_closed, read_kind,
    read_vec2, write_vec2,
};
use crate::path_model::{AnchorId, AnchorKind, HandleSlot, NodeId, PathEditError};
use crate::paths::resolve_handle_pair;
use crate::units::Vec2;

impl Document {
    /// Sets the two handles that face one segment as one commit labelled `bend_segment`: `start`'s
    /// outgoing handle and `end`'s incoming handle, each relative to its own anchor. No node moves.
    ///
    /// Each handle is written the way [`Document::set_handle`] writes it, by the one rule
    /// [`resolve_handle_pair`]: a Corner anchor's other handle is untouched, a Symmetric one's
    /// becomes the mirror, an Asymmetric one's turns opposite at its own length. The segment on
    /// the far side of such an anchor changes shape with it. An end node of an open path has no
    /// far segment, but its hidden handle is written by the same rule, as a hand drag of the shown
    /// handle would write it.
    ///
    /// Both ids and the directed adjacency (`end` follows `start`, or wraps to it when the path is
    /// closed) are resolved before the first write, so a refusal changes nothing.
    ///
    /// # Errors
    /// [`PathEditError::NoSuchPath`] / [`PathEditError::NoSuchAnchor`] if `path`, `start` or `end`
    /// no longer exist; [`PathEditError::NotAnAdjacentSegment`] if `end` does not follow `start`.
    pub fn bend_segment(
        &self,
        path: NodeId,
        start: AnchorId,
        end: AnchorId,
        start_handle_out: Vec2,
        end_handle_in: Vec2,
    ) -> Result<(), PathEditError> {
        let (meta, anchors) = self.path_parts(path)?;
        let closed = read_closed(&meta);
        let (start_index, end_index) = adjacent_segment_indices(&anchors, closed, start, end)?;
        let start_map = anchor_map_at(&anchors, start_index);
        let end_map = anchor_map_at(&anchors, end_index);
        write_resolved(&start_map, HandleSlot::Out, start_handle_out);
        write_resolved(&end_map, HandleSlot::In, end_handle_in);
        self.commit_with_label("bend_segment");
        Ok(())
    }
}

/// Writes `value` to `slot` of the anchor in `map` and, for a Symmetric or Asymmetric anchor,
/// the opposite handle that [`resolve_handle_pair`] resolves (never for a Corner anchor: a
/// redundant write would beat a collaborator's concurrent edit of that field).
fn write_resolved(map: &loro::LoroMap, slot: HandleSlot, value: Vec2) {
    let kind = read_kind(map);
    let handle_in = read_vec2(map, KEY_HANDLE_IN);
    let handle_out = read_vec2(map, KEY_HANDLE_OUT);
    let (new_in, new_out) = resolve_handle_pair(kind, slot, value, handle_in, handle_out);
    match slot {
        HandleSlot::In => write_vec2(map, KEY_HANDLE_IN, new_in),
        HandleSlot::Out => write_vec2(map, KEY_HANDLE_OUT, new_out),
    }
    if kind != AnchorKind::Corner {
        match slot {
            HandleSlot::In => write_vec2(map, KEY_HANDLE_OUT, new_out),
            HandleSlot::Out => write_vec2(map, KEY_HANDLE_IN, new_in),
        }
    }
}
