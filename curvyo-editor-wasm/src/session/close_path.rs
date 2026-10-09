//! `Session`'s Close path command (`specs/0034-pen-path-extension` criteria 18 to 21): which paths
//! the Node bar's buttons act on, what they would do, and the one commit that does it.

use curvyo_document_core::{NodeId, PathSnapshot};
use curvyo_ui_core::{JoinType, closable_counts, plan_close_paths};

use super::{Session, Tool};

/// What the Close path buttons show: how many open paths they would close, and how many open
/// paths they would skip for having fewer than three nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosePathState {
    /// Open ordinary paths with three or more nodes.
    pub closable: usize,
    /// Open ordinary paths with fewer than three nodes.
    pub skipped: usize,
}

/// What a press of a Close path button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosePathOutcome {
    /// How many paths were closed.
    pub closed: usize,
    /// How many open paths were skipped.
    pub skipped: usize,
}

impl Session {
    /// The paths the buttons act on: the paths the maker works on, which are those of the node
    /// selection (nodes or a segment) and the selected objects. Which nodes are selected within
    /// them does not matter (`0034` criterion 18). Compound paths never appear: the Node tool has
    /// no editable paths among them.
    fn close_path_set(&self) -> Vec<PathSnapshot> {
        let selection = self.node.selection();
        let mut wanted: Vec<NodeId> = selection
            .node_pairs()
            .iter()
            .map(|&(path, _)| path)
            .collect();
        if let Some((path, _, _)) = selection.segment_with_path() {
            wanted.push(path);
        }
        wanted.extend(self.selection.ids().iter().copied());
        self.paths()
            .into_iter()
            .filter(|path| wanted.contains(&path.id))
            .collect()
    }

    /// What the Close path buttons would do right now; zero outside the Node tool.
    #[must_use]
    pub fn close_path_state(&self) -> ClosePathState {
        if self.tool != Tool::Node {
            return ClosePathState::default();
        }
        let set = self.close_path_set();
        let refs: Vec<&PathSnapshot> = set.iter().collect();
        let (closable, skipped) = closable_counts(&refs);
        ClosePathState { closable, skipped }
    }

    /// Closes every open ordinary path of the set with three or more nodes, the first node
    /// becoming the closing node with `join` applied, as one commit (`0034` criteria 19 to 21).
    /// Does nothing outside the Node tool.
    pub fn close_paths(&mut self, join: JoinType) -> ClosePathOutcome {
        if self.tool != Tool::Node {
            return ClosePathOutcome::default();
        }
        let set = self.close_path_set();
        let refs: Vec<&PathSnapshot> = set.iter().collect();
        let plan = plan_close_paths(&refs, join);
        if plan.growths.is_empty() {
            return ClosePathOutcome {
                closed: 0,
                skipped: plan.skipped,
            };
        }
        match self.document.close_paths(&plan.growths) {
            Ok(()) => ClosePathOutcome {
                closed: plan.closed,
                skipped: plan.skipped,
            },
            Err(_) => ClosePathOutcome::default(),
        }
    }
}
