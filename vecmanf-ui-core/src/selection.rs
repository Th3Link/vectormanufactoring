//! The node tool's persistent selection (acceptance criteria 7, 10, 14).
//!
//! Ephemeral, per ADR 0009 §2 — never written to the document, never
//! saved. Holds [`AnchorId`]s and resolves them lazily against whatever
//! [`vecmanf_document_core::PathSnapshot`] the caller hands in, so a
//! selected anchor a collaborator (or this same session) has since
//! deleted simply stops resolving rather than dangling
//! (`specs/0002-path-node-editing/adrs.md`).
//!
//! At most one of "some nodes selected" and "one segment selected" holds
//! at a time — acceptance criterion 14 draws a selected segment as
//! "distinct from either endpoint node being selected" — so this is one
//! enum, not two independent flags that could disagree.
//!
//! `specs/0006-path-merge-split-and-node-types/adrs.md` adds one more
//! state, [`SelectionKind::SplitPair`], for Split's own result
//! (acceptance criterion 15): the two coincident copies a successful
//! Split produces can land on two *different* path objects (criterion
//! 13's open-path case), which this type's existing `path: Option<NodeId>`
//! field cannot express for an ordinary `Nodes` selection. This is
//! deliberately narrow rather than a full generalization of `Nodes` to
//! an arbitrary cross-path set: every ordinary interactive selection
//! (`select_single_node`, `toggle_node`, `select_segment`) stays exactly
//! as single-path as it always was — only Split's own result constructor
//! ([`NodeSelection::select_split_pair`]) can produce this state, and
//! [`NodeSelection::path`] reports `None` for it, so the existing
//! single-path-only convert/delete/make-line/make-curve actions
//! correctly stay disabled until the maker clicks one of the two nodes
//! (collapsing back to an ordinary single-path selection) — see
//! `crate::node_tool`'s own `pointer_down`. Building the general
//! multi-object selection mechanism (criteria 6-7) is explicitly
//! deferred to `canvas-navigation-and-selection`'s `ObjectSelection`;
//! this one additional, narrowly-scoped state is not that mechanism, it
//! only carries Split's own already-computed result so the maker can
//! immediately drag either node apart or re-Join them (criterion 15's
//! stated reason), without inventing a way to *select* two arbitrary
//! objects' nodes together.

use vecmanf_document_core::{AnchorId, NodeId};

/// The node tool's current selection: nothing, one or more nodes on one
/// path, one segment (a path-adjacent pair of anchors) on one path, or
/// the two coincident results of a Split that landed on two different
/// path objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeSelection {
    path: Option<NodeId>,
    kind: SelectionKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum SelectionKind {
    #[default]
    None,
    Nodes(Vec<AnchorId>),
    Segment(AnchorId, AnchorId),
    /// Split's own result when the two copies landed on different path
    /// objects (acceptance criteria 13, 15) — see this module's own doc
    /// comment for why this is a separate, narrow state rather than a
    /// general cross-path `Nodes`.
    SplitPair {
        /// The first copy's path and anchor id.
        first: (NodeId, AnchorId),
        /// The second copy's path and anchor id.
        second: (NodeId, AnchorId),
    },
}

impl NodeSelection {
    /// No selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The path the current selection belongs to, if any.
    #[must_use]
    pub const fn path(&self) -> Option<NodeId> {
        self.path
    }

    /// The currently selected nodes, in selection order. Empty when the
    /// selection is empty, is a segment, or is a [`SelectionKind::
    /// SplitPair`] instead — the latter is deliberately excluded here so
    /// the single-path-only convert/delete/make-line/make-curve actions
    /// (all gated on `has_nodes = !nodes().is_empty()`) correctly stay
    /// disabled for it; [`NodeSelection::join_pairs`] is the accessor
    /// that does see it.
    #[must_use]
    pub fn nodes(&self) -> &[AnchorId] {
        match &self.kind {
            SelectionKind::Nodes(ids) => ids,
            SelectionKind::None | SelectionKind::Segment(..) | SelectionKind::SplitPair { .. } => {
                &[]
            }
        }
    }

    /// The currently selected segment's two path-adjacent endpoint ids,
    /// if a segment (rather than nodes, a split pair, or nothing) is
    /// selected.
    #[must_use]
    pub const fn segment(&self) -> Option<(AnchorId, AnchorId)> {
        match self.kind {
            SelectionKind::Segment(start, end) => Some((start, end)),
            SelectionKind::None | SelectionKind::Nodes(_) | SelectionKind::SplitPair { .. } => None,
        }
    }

    /// The two `(path, anchor)` pairs Join should act on, if the current
    /// selection qualifies at all: either exactly two nodes selected on
    /// one path (the common case — same-path Join, acceptance criterion
    /// 10), or a `SelectionKind::SplitPair` (Split's own result,
    /// possibly on two different path objects — acceptance criterion 9's
    /// "two different open path objects" case, reachable today only by
    /// immediately re-Joining a just-split path, not by any general
    /// cross-object selection). `None` for any other selection shape;
    /// callers (`crate::node_tool::NodeTool::can_join`/`join_selected`)
    /// still check endpoint-ness and open/closed against the live
    /// document — this only resolves *which* two anchors a qualifying
    /// selection names.
    #[must_use]
    pub fn join_pairs(&self) -> Option<((NodeId, AnchorId), (NodeId, AnchorId))> {
        match &self.kind {
            SelectionKind::Nodes(ids) if ids.len() == 2 => {
                let path = self.path?;
                Some(((path, ids[0]), (path, ids[1])))
            }
            SelectionKind::SplitPair { first, second } => Some((*first, *second)),
            _ => None,
        }
    }

    /// Whether `anchor` is one of the currently selected nodes.
    #[must_use]
    pub fn contains_node(&self, anchor: AnchorId) -> bool {
        self.nodes().contains(&anchor)
    }

    /// Whether there is nothing selected at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.kind, SelectionKind::None)
    }

    /// Clears the selection (acceptance criterion: Escape with a
    /// selection present, `specification.md`'s node-tool actions notes).
    pub fn clear(&mut self) {
        self.path = None;
        self.kind = SelectionKind::None;
    }

    /// Acceptance criterion 7: selects exactly one node, replacing
    /// whatever was selected before (including on a different path).
    pub fn select_single_node(&mut self, path: NodeId, anchor: AnchorId) {
        self.path = Some(path);
        self.kind = SelectionKind::Nodes(vec![anchor]);
    }

    /// Acceptance criterion 10: shift-click toggles one node's
    /// membership in a multi-selection. Switching to a different path (or
    /// away from a segment selection) starts a fresh one-node selection
    /// instead of toggling against unrelated state.
    pub fn toggle_node(&mut self, path: NodeId, anchor: AnchorId) {
        if self.path != Some(path) {
            self.select_single_node(path, anchor);
            return;
        }
        let mut ids = match std::mem::take(&mut self.kind) {
            SelectionKind::Nodes(ids) => ids,
            // `SplitPair` is unreachable here in practice: `self.path` is
            // always `None` for it, so the early return above already
            // fired. Handled anyway for exhaustiveness, the same empty-
            // start-fresh fallback as `None`/`Segment`.
            SelectionKind::None | SelectionKind::Segment(..) | SelectionKind::SplitPair { .. } => {
                Vec::new()
            }
        };
        if let Some(position) = ids.iter().position(|&id| id == anchor) {
            ids.remove(position);
        } else {
            ids.push(anchor);
        }
        if ids.is_empty() {
            self.clear();
        } else {
            self.kind = SelectionKind::Nodes(ids);
        }
    }

    /// Acceptance criterion 14: selects one segment, replacing any node
    /// selection.
    pub fn select_segment(&mut self, path: NodeId, start: AnchorId, end: AnchorId) {
        self.path = Some(path);
        self.kind = SelectionKind::Segment(start, end);
    }

    /// Acceptance criterion 15: selects Split's own two resulting
    /// coincident nodes, replacing whatever was selected before. `path()`
    /// reports `None` for this selection (see this module's own doc
    /// comment) even when `first` and `second` happen to share one path
    /// (criterion 14's closed-path case) — a subsequent plain click on
    /// either node still correctly collapses to an ordinary single-node
    /// selection on its own path, same as switching to any other path
    /// does for an ordinary `Nodes` selection.
    pub fn select_split_pair(&mut self, first: (NodeId, AnchorId), second: (NodeId, AnchorId)) {
        self.path = None;
        self.kind = SelectionKind::SplitPair { first, second };
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{Document, NewAnchor, Point};

    use super::*;

    /// `NodeId` has no public constructor outside `document-core`, so
    /// tests that only need *some* two distinct path ids mint them the
    /// one way available: creating real (throwaway) paths.
    fn two_distinct_paths() -> (NodeId, NodeId) {
        let document = Document::new(1);
        let a = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 101), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 102), Point::new(1.0, 0.0)),
            ],
            false,
        );
        let b = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 201), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 202), Point::new(1.0, 0.0)),
            ],
            false,
        );
        (a, b)
    }

    fn one_path() -> NodeId {
        two_distinct_paths().0
    }

    #[test]
    fn a_fresh_selection_is_empty() {
        let selection = NodeSelection::new();
        assert!(
            selection.is_empty(),
            "a fresh selection has nothing selected"
        );
        assert_eq!(selection.path(), None);
        assert_eq!(selection.nodes(), &[]);
        assert_eq!(selection.segment(), None);
    }

    #[test]
    fn select_single_node_replaces_any_prior_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        assert_eq!(selection.nodes(), &[a]);
        selection.select_single_node(path, b);
        assert_eq!(selection.nodes(), &[b]);
    }

    #[test]
    fn toggle_node_builds_a_multi_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        selection.toggle_node(path, b);
        assert_eq!(selection.nodes(), &[a, b]);
    }

    #[test]
    fn toggle_node_removes_an_already_selected_node() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        selection.toggle_node(path, b);
        selection.toggle_node(path, a);
        assert_eq!(selection.nodes(), &[b]);
    }

    #[test]
    fn toggle_node_on_a_different_path_starts_a_fresh_selection() {
        let (path_a, path_b) = two_distinct_paths();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path_a, a);
        selection.toggle_node(path_b, b);
        assert_eq!(selection.path(), Some(path_b));
        assert_eq!(selection.nodes(), &[b]);
    }

    #[test]
    fn select_segment_replaces_a_node_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        selection.select_segment(path, a, b);
        assert_eq!(selection.nodes(), &[]);
        assert_eq!(selection.segment(), Some((a, b)));
    }

    #[test]
    fn clear_empties_the_selection() {
        let path = one_path();
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, AnchorId::new(1, 1));
        selection.clear();
        assert!(selection.is_empty(), "clear() must empty the selection");
    }
}
