//! The same outline walked the other way: the node order reversed and each node's handles swapped.

use crate::path_model::NewAnchor;

/// `anchors` in the opposite order, each anchor's incoming and outgoing handle swapped: the same
/// outline walked the other way (`specs/0006-path-merge-split-and-node-types` criterion 9, the
/// rule of Join; `0034-pen-path-extension` criteria 4 and 8).
#[must_use]
pub fn reversed_anchors(anchors: &[NewAnchor]) -> Vec<NewAnchor> {
    anchors
        .iter()
        .rev()
        .map(|anchor| NewAnchor {
            handle_in: anchor.handle_out,
            handle_out: anchor.handle_in,
            ..*anchor
        })
        .collect()
}
