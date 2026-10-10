//! The pen tool's commit planning: how the path in progress is closed, joined to another path or
//! grown, as the one [`CloseResolution`] or [`PathGrowth`] that both the preview and the commit use
//! (`specs/0034-pen-path-extension` criteria 8 to 11, 15, 17).

use curvyo_document_core::{
    COINCIDENT_MM, Document, HandleSlot, NewAnchor, PathEnd, PathGrowth, merged_junction,
    reversed_anchors,
};

use super::{ClosingPreview, Continuation, PenTool, PlacedAnchor, PointerUpOutcome, State};
use crate::closing_join::{JoinType, resolve_closing_node};
use crate::pen_target::EndNode;

/// Everything a close needs, resolved once for the preview and for the commit.
struct CloseResolution {
    /// The stored list of the closed path, with the closing node resolved.
    stored: Vec<NewAnchor>,
    /// The index of the closing node in `stored`.
    closing_index: usize,
}

impl PenTool {
    /// The closed path with `join` applied to its closing node, resolved once for the preview and
    /// for the commit (criterion 17: they cannot disagree). `None` when there is nothing to close.
    fn resolve_close(&self, join: JoinType) -> Option<CloseResolution> {
        let State::Placing { nodes, base, .. } = &self.state else {
            return None;
        };
        self.closing_node()?;
        let (mut stored, closing_index, side): (Vec<NewAnchor>, usize, HandleSlot) = match base {
            None => (nodes.clone(), 0, HandleSlot::In),
            Some(continuation) => match continuation.end {
                PathEnd::Last => {
                    let mut stored = continuation.original.clone();
                    stored.extend_from_slice(&nodes[1..]);
                    (stored, 0, HandleSlot::In)
                }
                PathEnd::First => {
                    let mut stored = reversed_anchors(&nodes[1..]);
                    stored.extend_from_slice(&continuation.original);
                    let last = stored.len() - 1;
                    (stored, last, HandleSlot::Out)
                }
            },
        };
        let len = stored.len();
        let before = stored[(closing_index + len - 1) % len].point;
        let after = stored[(closing_index + 1) % len].point;
        stored[closing_index] =
            resolve_closing_node(&stored[closing_index], before, after, side, join);
        Some(CloseResolution {
            stored,
            closing_index,
        })
    }

    /// The closing segment as it will be committed with `join` (criterion 17), for the preview.
    #[must_use]
    pub fn closing_preview(&self, join: JoinType) -> Option<ClosingPreview> {
        let resolution = self.resolve_close(join)?;
        let stored = &resolution.stored;
        let closing_node = stored[resolution.closing_index];
        Some(ClosingPreview {
            from: stored[stored.len() - 1],
            to: stored[0],
            closing_node,
        })
    }

    /// Closes the path in progress with `join`: a new path becomes one created path, a continued
    /// one is grown and closed as the same object.
    pub(super) fn commit_close(&mut self, document: &Document, join: JoinType) -> PointerUpOutcome {
        let Some(resolution) = self.resolve_close(join) else {
            return PointerUpOutcome::Placed;
        };
        let State::Placing { nodes, base, .. } = &self.state else {
            return PointerUpOutcome::Ignored;
        };
        let outcome = match base {
            None => PointerUpOutcome::Closed(document.create_path(&resolution.stored, true)),
            Some(continuation) => {
                let closing = resolution.stored[resolution.closing_index];
                let (end, added) = match continuation.end {
                    PathEnd::Last => (PathEnd::Last, nodes[1..].to_vec()),
                    PathEnd::First => (PathEnd::First, reversed_anchors(&nodes[1..])),
                };
                let growth = PathGrowth {
                    path: continuation.path,
                    end,
                    added,
                    absorb: None,
                    replace: vec![closing],
                    drop: Vec::new(),
                };
                match document.close_paths(&[growth]) {
                    Ok(()) => PointerUpOutcome::Closed(continuation.path),
                    Err(_) => PointerUpOutcome::Ignored,
                }
            }
        };
        self.state = State::Idle;
        outcome
    }

    /// Joins the path in progress to `target`'s path (`0034` criteria 8 to 11): a continued path
    /// absorbs the other; a new path ends on the other and becomes it.
    pub(super) fn commit_join(
        &mut self,
        document: &Document,
        target: &EndNode,
    ) -> PointerUpOutcome {
        let State::Placing { nodes, base, .. } = &self.state else {
            return PointerUpOutcome::Ignored;
        };
        let growth = match base {
            Some(continuation) => Self::growth_continuing(continuation, nodes, target),
            None => Self::growth_new_path(nodes, target),
        };
        let outcome = growth.map_or(PointerUpOutcome::Ignored, |growth| {
            let survivor = growth.path;
            match document.connect_paths(&growth) {
                Ok(()) => PointerUpOutcome::Joined(survivor),
                Err(_) => PointerUpOutcome::Ignored,
            }
        });
        self.state = State::Idle;
        outcome
    }

    /// A continued path P grows by the nodes drawn after E and absorbs the target's path. When the
    /// last drawn node (or E) lies on the target's end, the two merge into one node.
    fn growth_continuing(
        continuation: &Continuation,
        nodes: &[PlacedAnchor],
        target: &EndNode,
    ) -> Option<PathGrowth> {
        let mut added: Vec<NewAnchor> = nodes[1..].to_vec();
        let target_is_last = target.end == PathEnd::Last;
        let mut replace = Vec::new();
        let mut drop = Vec::new();
        if let Some(last) = added.last().copied() {
            if last.point.vector_to(target.anchor.point).length() < COINCIDENT_MM {
                let merged = merged_junction(&last, true, &target.anchor, target_is_last);
                *added.last_mut()? = merged;
                drop.push(target.anchor.id);
            }
        } else {
            let drawn_end = nodes.first()?;
            if drawn_end.point.vector_to(target.anchor.point).length() < COINCIDENT_MM {
                // E is an existing node of P, in drawing direction; its stored handles are the
                // swapped ones when continuing from the first node.
                let e_stored = match continuation.end {
                    PathEnd::Last => continuation.original[continuation.original.len() - 1],
                    PathEnd::First => continuation.original[0],
                };
                let merged = merged_junction(
                    &e_stored,
                    continuation.end == PathEnd::Last,
                    &target.anchor,
                    target_is_last,
                );
                replace.push(merged);
                drop.push(target.anchor.id);
            }
        }
        if continuation.end == PathEnd::First {
            added = reversed_anchors(&added);
        }
        Some(PathGrowth {
            path: continuation.path,
            end: continuation.end,
            added,
            absorb: Some((target.path, target.end)),
            replace,
            drop,
        })
    }

    /// A new path that ends on another path's end becomes that path: it grows at the target's end
    /// by the new nodes, walked away from the target.
    fn growth_new_path(nodes: &[PlacedAnchor], target: &EndNode) -> Option<PathGrowth> {
        let mut drawn: Vec<NewAnchor> = nodes.to_vec();
        let mut replace = Vec::new();
        if let Some(last) = drawn.last().copied()
            && last.point.vector_to(target.anchor.point).length() < COINCIDENT_MM
        {
            // The target's own node keeps its id and becomes the merged junction.
            let merged = merged_junction(&target.anchor, target.end == PathEnd::Last, &last, true);
            replace.push(merged);
            drawn.pop();
        }
        if drawn.is_empty() && replace.is_empty() {
            return None;
        }
        // Stored order: at the target's last end the new nodes follow it, the node drawn last
        // first; at its first end they precede it, in drawing order.
        let added = match target.end {
            PathEnd::Last => reversed_anchors(&drawn),
            PathEnd::First => drawn,
        };
        Some(PathGrowth {
            path: target.path,
            end: target.end,
            added,
            absorb: None,
            replace,
            drop: Vec::new(),
        })
    }
}
