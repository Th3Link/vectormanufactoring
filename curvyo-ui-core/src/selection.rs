//! The node tool's persistent selection (acceptance criteria 7, 10, 14;
//! `specs/0006-path-merge-split-and-node-types/specification.md`
//! acceptance criteria 6, 7, 15).
//!
//! Ephemeral, per ADR 0009 §2 — never written to the document, never
//! saved. Holds `(NodeId, AnchorId)` pairs and resolves them lazily
//! against whatever [`curvyo_document_core::PathSnapshot`]s the caller
//! hands in, so a selected anchor a collaborator (or this same session)
//! has since deleted simply stops resolving rather than dangling
//! (`specs/0002-path-node-editing/adrs.md`).
//!
//! At most one of "some nodes selected" and "one segment selected" holds
//! at a time — acceptance criterion 14 draws a selected segment as
//! "distinct from either endpoint node being selected" — so this is one
//! enum, not two independent flags that could disagree.
//!
//! **2026-10-05, generalized to a genuine multi-path selection**
//! (`specs/0006-path-merge-split-and-node-types/adrs.md`'s "one Node-tool
//! session over several paths": "`NodeSelection` today holds one `path:
//! Option<NodeId>` and a list of `AnchorId`s. It becomes an ordered list
//! of `(NodeId, AnchorId)`... No new selection type is invented"). This
//! was deferred one round, with a narrow `SplitPair` variant standing in
//! for Split's own two-object result only (acceptance criterion 15),
//! because `canvas-navigation-and-selection`'s `ObjectSelection` did not
//! exist yet to drive the general case (criteria 6, 7). It now does, so
//! `SplitPair` is gone — the architect's own dated note on this file's
//! history: "The follow-up that delivers AC 6 and AC 7 replaces
//! `SplitPair` with the ordered `(NodeId, AnchorId)` list decided above.
//! It does not keep both." [`SelectionKind::Nodes`] now holds that list
//! directly; a plain click/shift-click (`NodeTool::pointer_down`,
//! `toggle_node` below) can build a selection spanning any number of
//! paths, not just Split's own output.
//!
//! [`NodeSelection::path`] still reports the *common* path when every
//! selected node (or the one selected segment) shares one — `None` for
//! an empty selection or one that genuinely spans several paths. The
//! single-path-only actions (convert/delete/make-line/make-curve) key
//! off this, so they stay correctly disabled for a true multi-path
//! selection exactly as they did for `SplitPair` before — seeing more
//! than one path is not reason enough to build cross-path convert/delete
//! for this slice (no acceptance criterion here asks for it; Join is the
//! one action that already works across paths by construction, via
//! [`NodeSelection::join_pairs`]).

use curvyo_document_core::{AnchorId, NodeId};

/// The node tool's current selection: nothing, one or more nodes
/// (possibly across several path objects, acceptance criteria 6, 7), or
/// one segment (a path-adjacent pair of anchors) on one path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeSelection {
    kind: SelectionKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum SelectionKind {
    #[default]
    None,
    /// Ordered `(path, anchor)` pairs — order matters for Join's "first
    /// selected" rule (`specs/0006-.../adrs.md`).
    Nodes(Vec<(NodeId, AnchorId)>),
    Segment(NodeId, AnchorId, AnchorId),
}

impl NodeSelection {
    /// No selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The one path every currently selected node (or the selected
    /// segment) belongs to, if the selection is confined to a single
    /// path. `None` for an empty selection, or one that spans two or
    /// more different path objects (acceptance criteria 6, 7).
    #[must_use]
    pub fn path(&self) -> Option<NodeId> {
        match &self.kind {
            SelectionKind::None => None,
            SelectionKind::Segment(path, _, _) => Some(*path),
            SelectionKind::Nodes(pairs) => {
                let (first_path, _) = pairs.first()?;
                pairs
                    .iter()
                    .all(|(path, _)| path == first_path)
                    .then_some(*first_path)
            }
        }
    }

    /// The currently selected nodes as `(path, anchor)` pairs, in
    /// selection order. Empty when the selection is empty or is a
    /// segment instead. The one accessor that sees every selected node
    /// regardless of how many different paths it spans — `nodes` below
    /// is the single-path convenience view most call sites still want.
    #[must_use]
    pub fn node_pairs(&self) -> &[(NodeId, AnchorId)] {
        match &self.kind {
            SelectionKind::Nodes(pairs) => pairs,
            SelectionKind::None | SelectionKind::Segment(..) => &[],
        }
    }

    /// The currently selected nodes' own anchor ids, in selection order
    /// — a plain-id convenience view of [`NodeSelection::node_pairs`]
    /// for the single-path-only callers (`convert`/`delete`/toolbar
    /// emptiness checks) that never needed each pair's path once they
    /// already have it from [`NodeSelection::path`].
    #[must_use]
    pub fn nodes(&self) -> Vec<AnchorId> {
        self.node_pairs()
            .iter()
            .map(|&(_, anchor)| anchor)
            .collect()
    }

    /// The currently selected segment's path and two path-adjacent
    /// endpoint ids, if a segment (rather than nodes or nothing) is
    /// selected.
    #[must_use]
    pub const fn segment_with_path(&self) -> Option<(NodeId, AnchorId, AnchorId)> {
        match self.kind {
            SelectionKind::Segment(path, start, end) => Some((path, start, end)),
            SelectionKind::None | SelectionKind::Nodes(_) => None,
        }
    }

    /// The currently selected segment's two path-adjacent endpoint ids,
    /// if a segment (rather than nodes or nothing) is selected.
    #[must_use]
    pub const fn segment(&self) -> Option<(AnchorId, AnchorId)> {
        match self.kind {
            SelectionKind::Segment(_, start, end) => Some((start, end)),
            SelectionKind::None | SelectionKind::Nodes(_) => None,
        }
    }

    /// The two `(path, anchor)` pairs Join should act on, if the current
    /// selection is exactly two nodes — same path (acceptance criterion
    /// 10) or two different path objects (criterion 9). `None` for any
    /// other selection shape; callers (`crate::node_tool::NodeTool::
    /// can_join`/`join_selected`) still check endpoint-ness and
    /// open/closed against the live document — this only resolves
    /// *which* two anchors a qualifying selection names.
    #[must_use]
    pub fn join_pairs(&self) -> Option<((NodeId, AnchorId), (NodeId, AnchorId))> {
        match self.node_pairs() {
            [a, b] => Some((*a, *b)),
            _ => None,
        }
    }

    /// Whether `anchor` is one of the currently selected nodes, on any
    /// of the paths it spans.
    #[must_use]
    pub fn contains_node(&self, anchor: AnchorId) -> bool {
        self.node_pairs().iter().any(|&(_, a)| a == anchor)
    }

    /// Whether there is nothing selected at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.kind, SelectionKind::None)
    }

    /// Clears the selection (acceptance criterion: Escape with a
    /// selection present, `specification.md`'s node-tool actions notes).
    pub fn clear(&mut self) {
        self.kind = SelectionKind::None;
    }

    /// Acceptance criterion 7: selects exactly one node, replacing
    /// whatever was selected before (including on a different path).
    pub fn select_single_node(&mut self, path: NodeId, anchor: AnchorId) {
        self.kind = SelectionKind::Nodes(vec![(path, anchor)]);
    }

    /// Acceptance criterion 10, extended by `specs/0006-.../
    /// specification.md` acceptance criterion 7: shift-click toggles one
    /// node's membership in a multi-selection — now possibly adding a
    /// node on a *different* path than the ones already selected, which
    /// criterion 7 explicitly asks for ("that criterion's wording never
    /// restricted shift-click-to-add to one path"). Switching away from
    /// a segment selection still starts a fresh one-node selection
    /// instead of toggling against unrelated state.
    pub fn toggle_node(&mut self, path: NodeId, anchor: AnchorId) {
        let mut pairs = match std::mem::take(&mut self.kind) {
            SelectionKind::Nodes(pairs) => pairs,
            SelectionKind::None | SelectionKind::Segment(..) => Vec::new(),
        };
        if let Some(position) = pairs.iter().position(|&(p, a)| p == path && a == anchor) {
            pairs.remove(position);
        } else {
            pairs.push((path, anchor));
        }
        if pairs.is_empty() {
            self.clear();
        } else {
            self.kind = SelectionKind::Nodes(pairs);
        }
    }

    /// Acceptance criterion 14: selects one segment, replacing any node
    /// selection.
    pub fn select_segment(&mut self, path: NodeId, start: AnchorId, end: AnchorId) {
        self.kind = SelectionKind::Segment(path, start, end);
    }

    /// Replaces the current selection with exactly these `(path,
    /// anchor)` pairs — Split's own result (`specs/0006-.../
    /// specification.md` acceptance criterion 15), on one path (the
    /// closed-path case, criterion 14) or two (the open-path case,
    /// criterion 13): both are now just an ordinary possibly-multi-path
    /// node selection, the same representation an interactive shift-
    /// click across paths builds (`NodeSelection::toggle_node`) — no
    /// separate variant needed any more.
    pub fn select_nodes(&mut self, pairs: Vec<(NodeId, AnchorId)>) {
        if pairs.is_empty() {
            self.clear();
        } else {
            self.kind = SelectionKind::Nodes(pairs);
        }
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{Document, NewAnchor, Point};

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
        assert_eq!(selection.nodes(), Vec::new());
        assert_eq!(selection.segment(), None);
    }

    #[test]
    fn select_single_node_replaces_any_prior_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        assert_eq!(selection.nodes(), vec![a]);
        selection.select_single_node(path, b);
        assert_eq!(selection.nodes(), vec![b]);
    }

    #[test]
    fn toggle_node_builds_a_multi_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        selection.toggle_node(path, b);
        assert_eq!(selection.nodes(), vec![a, b]);
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
        assert_eq!(selection.nodes(), vec![b]);
    }

    /// `specs/0006-path-merge-split-and-node-types/specification.md`
    /// acceptance criterion 7: shift-clicking a node on a *different*
    /// path adds it to the selection rather than starting a fresh one —
    /// the behaviour this slice changes from the single-path-only
    /// `toggle_node` `path-node-editing` originally shipped.
    #[test]
    fn toggle_node_on_a_different_path_adds_to_the_selection() {
        let (path_a, path_b) = two_distinct_paths();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path_a, a);
        selection.toggle_node(path_b, b);
        assert_eq!(
            selection.node_pairs(),
            &[(path_a, a), (path_b, b)],
            "both nodes selected together, across the two different paths"
        );
        assert_eq!(
            selection.path(),
            None,
            "no single common path once the selection spans two"
        );
    }

    /// Toggling a node already in a multi-path selection off it still
    /// works, same as the single-path case above.
    #[test]
    fn toggle_node_removes_from_a_multi_path_selection() {
        let (path_a, path_b) = two_distinct_paths();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path_a, a);
        selection.toggle_node(path_b, b);
        selection.toggle_node(path_a, a);
        assert_eq!(selection.node_pairs(), &[(path_b, b)]);
        assert_eq!(selection.path(), Some(path_b));
    }

    #[test]
    fn select_segment_replaces_a_node_selection() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        selection.select_segment(path, a, b);
        assert_eq!(selection.nodes(), Vec::new());
        assert_eq!(selection.segment(), Some((a, b)));
        assert_eq!(selection.path(), Some(path));
    }

    #[test]
    fn clear_empties_the_selection() {
        let path = one_path();
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, AnchorId::new(1, 1));
        selection.clear();
        assert!(selection.is_empty(), "clear() must empty the selection");
    }

    /// `select_nodes` replaces the selection outright, same-path or
    /// across two different ones — Split's own result (acceptance
    /// criterion 15), here exercised directly as the general setter it
    /// now is.
    #[test]
    fn select_nodes_replaces_the_selection_same_or_different_paths() {
        let (path_a, path_b) = two_distinct_paths();
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let mut selection = NodeSelection::new();
        selection.select_nodes(vec![(path_a, a), (path_b, b)]);
        assert_eq!(selection.node_pairs(), &[(path_a, a), (path_b, b)]);
        assert_eq!(selection.path(), None);
        assert_eq!(
            selection.join_pairs(),
            Some(((path_a, a), (path_b, b))),
            "immediately re-joinable, acceptance criterion 15's stated reason"
        );
    }

    #[test]
    fn join_pairs_is_none_unless_exactly_two_nodes_are_selected() {
        let path = one_path();
        let a = AnchorId::new(1, 1);
        let mut selection = NodeSelection::new();
        assert_eq!(selection.join_pairs(), None, "nothing selected");
        selection.select_single_node(path, a);
        assert_eq!(selection.join_pairs(), None, "only one node selected");
    }
}
