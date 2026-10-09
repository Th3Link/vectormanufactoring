//! `Session`'s Node-tool glue (`specs/0002-path-node-editing`,
//! `specs/0006-path-merge-split-and-node-types`): the contextual-toolbar
//! actions, the live node/handle drag substituted into the snapshot the
//! renderer draws, and the Node tool's decoration input. Split out of
//! `session/mod.rs` the same way `session/select.rs` and `session/shapes.rs`
//! are (`docs/technical-debt.md`, "`Session` is one module past the size
//! limit").

use curvyo_document_core::{AnchorKind, ObjectSnapshot, Point};
use curvyo_render_core::{DecorationInput, Hovered as RenderHovered};
use curvyo_ui_core::{Hit, NodeToolbarState};

use super::{Session, Tool};

impl Session {
    /// [`Session::paths`], with the node tool's in-flight drag (if any)
    /// substituted into the relevant anchor's live, not-yet-committed
    /// position/handle values — resolved by [`NodeTool::live_drag`]
    /// itself (the same helpers [`NodeTool::pointer_up`] uses to commit),
    /// so this is a pure "apply already-resolved data" step with no
    /// geometry of its own. Falls back to the committed snapshot
    /// unmodified outside the node tool, with no drag in flight, or with
    /// the pointer off the canvas (`self.pointer_position` is `None`). The
    /// Select tool's live edit is not substituted here: its blue outline is
    /// drawn over the committed paths (`specs/0009-unified-object-editing`).
    pub(super) fn live_node_drag_paths_in(
        &self,
        objects: &[ObjectSnapshot],
    ) -> Vec<curvyo_document_core::PathSnapshot> {
        let mut paths: Vec<curvyo_document_core::PathSnapshot> = objects
            .iter()
            .filter_map(|object| match object {
                ObjectSnapshot::Path(path) => Some(path.clone()),
                ObjectSnapshot::Primitive(_) => None,
            })
            .collect();
        if self.tool == Tool::Node {
            self.apply_live_node_drag(&mut paths);
        }
        paths
    }

    pub(super) fn apply_live_node_drag(&self, paths: &mut [curvyo_document_core::PathSnapshot]) {
        let Some(cursor) = self.pointer_position else {
            return;
        };
        let Some(live) = self.node.live_drag(cursor) else {
            return;
        };
        match live {
            curvyo_ui_core::LiveNodeDrag::Nodes { positions } => {
                for (path, id, point) in positions {
                    if let Some(snapshot) = paths.iter_mut().find(|p| p.id == path)
                        && let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == id)
                    {
                        anchor.point = point;
                    }
                }
            }
            curvyo_ui_core::LiveNodeDrag::Handle {
                path,
                anchor,
                handle_in,
                handle_out,
            } => {
                // `handle_in`/`handle_out` already fully resolved by
                // `NodeTool::live_drag` (which calls the exact same
                // `curvyo_document_core::resolve_handle_pair` function
                // `Document::set_handle` itself commits with) — a plain
                // assignment, no slot/mirror logic of its own to
                // independently drift from the commit.
                if let Some(snapshot) = paths.iter_mut().find(|p| p.id == path)
                    && let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == anchor)
                {
                    anchor.handle_in = handle_in;
                    anchor.handle_out = handle_out;
                }
            }
        }
    }

    /// Acceptance criterion 11 (the contextual toolbar's convert
    /// buttons). A no-op for the pen tool.
    pub fn convert_selected(&mut self, kind: AnchorKind) {
        if self.tool == Tool::Node {
            self.node.convert_selected(&self.document, kind);
        }
    }

    /// Acceptance criterion 14's "make line" (the contextual toolbar).
    pub fn make_line(&mut self) {
        if self.tool == Tool::Node {
            self.node.make_line(&self.document);
        }
    }

    /// Acceptance criterion 14's "make curve" (the contextual toolbar).
    pub fn make_curve(&mut self) {
        if self.tool == Tool::Node {
            self.node.make_curve(&self.document);
        }
    }

    /// Acceptance criteria 8-11: Join (the contextual toolbar/context
    /// menu button). A no-op outside the node tool or when the current
    /// selection does not qualify.
    pub fn join_selected(&mut self) {
        if self.tool == Tool::Node {
            self.node.join_selected(&self.document);
        }
    }

    /// Acceptance criteria 12-15: Split (the contextual toolbar/context
    /// menu button). A no-op outside the node tool or when the current
    /// selection does not qualify.
    pub fn split_selected(&mut self) {
        if self.tool == Tool::Node {
            self.node.split_selected(&mut self.minter, &self.document);
        }
    }

    /// Acceptance criterion 12: a double-click (already recognized by
    /// the host) at `point`.
    pub fn insert_at(&mut self, point: Point) {
        if self.tool == Tool::Node {
            let paths = self.paths();
            let tolerances = self.hit_tolerances();
            self.node
                .insert_at(&mut self.minter, &self.document, &paths, point, tolerances);
        }
    }

    /// The contextual toolbar's "Insert node" action: splits the
    /// currently selected segment at its midpoint. A no-op outside the
    /// node tool or without a segment selected.
    pub fn insert_selected(&mut self) {
        if self.tool == Tool::Node {
            let paths = self.paths();
            self.node
                .insert_on_selected_segment(&mut self.minter, &self.document, &paths);
        }
    }

    /// Which contextual-toolbar actions apply right now. Everything is
    /// `false` when the node tool isn't active, since the toolbar itself
    /// is only shown then.
    #[must_use]
    pub fn node_toolbar_state(&self) -> NodeToolbarState {
        if self.tool != Tool::Node {
            return NodeToolbarState::default();
        }
        self.node.toolbar_state(&self.document)
    }

    pub(super) fn decoration_input(&self) -> DecorationInput {
        if self.tool != Tool::Node {
            return DecorationInput::default();
        }
        let selection = self.node.selection();
        // `node_pairs` directly, not `nodes()` zipped with `path()`: the
        // selection can now genuinely span several path objects
        // (`specs/0006-path-merge-split-and-node-types/specification.md`
        // acceptance criteria 6, 7, 15), and `path()` reports `None` for
        // that case — zipping against it would silently render none of
        // the selected nodes as selected instead of all of them.
        let selected_nodes = selection.node_pairs().to_vec();
        let selected_segment = selection.segment_with_path();
        let hovered = self.hovered.and_then(|hit| match hit {
            Hit::Node { path, anchor } => Some(RenderHovered::Node(path, anchor)),
            Hit::Handle { path, anchor, slot } => Some(RenderHovered::Handle(path, anchor, slot)),
            Hit::Segment { .. } => None,
        });
        DecorationInput {
            show_nodes: true,
            selected_nodes,
            selected_segment,
            hovered,
        }
    }
}
