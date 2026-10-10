//! `Session`'s Close path command (`specs/0034-pen-path-extension` criteria 18 to 21): which paths
//! the Node bar's buttons act on, what they would do, and the one commit that does it.

use curvyo_ui_core::{JoinType, closable_counts, close_path_set, plan_close_paths};

use super::{Session, Tool};

/// What the Close path buttons show: how many open paths they would close, and how many open
/// paths they would skip for having fewer than three nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosePathState {
    /// Open ordinary paths with three or more nodes.
    pub closable: usize,
    /// Open ordinary paths with fewer than three nodes, or three whose ends coincide.
    pub skipped: usize,
    /// How many of the skipped paths are of the second kind.
    pub same_ends: usize,
}

/// What a press of a Close path button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosePathOutcome {
    /// How many paths were closed.
    pub closed: usize,
    /// How many open paths were skipped.
    pub skipped: usize,
    /// How many of the skipped paths have three nodes with coinciding ends.
    pub same_ends: usize,
}

impl Session {
    /// What the Close path buttons would do right now; zero outside the Node tool.
    #[must_use]
    pub fn close_path_state(&self) -> ClosePathState {
        if self.tool != Tool::Node {
            return ClosePathState::default();
        }
        let paths = self.paths();
        let set = close_path_set(&paths, self.node.selection(), &self.selection);
        let counts = closable_counts(&set);
        ClosePathState {
            closable: counts.closable,
            skipped: counts.skipped,
            same_ends: counts.same_ends,
        }
    }

    /// Closes every open ordinary path of the set with three or more nodes, the first node
    /// becoming the closing node with `join` applied, as one commit (`0034` criteria 19 to 21).
    /// Does nothing outside the Node tool.
    pub fn close_paths(&mut self, join: JoinType) -> ClosePathOutcome {
        if self.tool != Tool::Node {
            return ClosePathOutcome::default();
        }
        let paths = self.paths();
        let set = close_path_set(&paths, self.node.selection(), &self.selection);
        let plan = plan_close_paths(&set, join);
        if plan.growths.is_empty() {
            return ClosePathOutcome {
                closed: 0,
                skipped: plan.skipped,
                same_ends: plan.same_ends,
            };
        }
        match self.document.close_paths(&plan.growths) {
            Ok(()) => ClosePathOutcome {
                closed: plan.closed,
                skipped: plan.skipped,
                same_ends: plan.same_ends,
            },
            Err(_) => ClosePathOutcome::default(),
        }
    }
}
