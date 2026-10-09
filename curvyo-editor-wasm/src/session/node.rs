//! `Session`'s Node-tool glue (`specs/0002-path-node-editing`,
//! `specs/0006-path-merge-split-and-node-types`): the contextual-toolbar
//! actions, the live node/handle drag substituted into the snapshot the
//! renderer draws, and the Node tool's decoration input. Split out of
//! `session/mod.rs` the same way `session/select.rs` and `session/shapes.rs`
//! are (`docs/technical-debt.md`, "`Session` is one module past the size
//! limit").

use curvyo_document_core::{AnchorKind, ObjectSnapshot, PathSnapshot, Point};
use curvyo_render_core::{DecorationInput, Hovered as RenderHovered};
use curvyo_ui_core::{BendResolution, Hit, LiveNodeDrag, NodeToolbarState, segment_is_bendable};

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
        let Some(live) = self.node.live_drag(cursor, self.held.shift) else {
            return;
        };
        match live {
            LiveNodeDrag::Bend(bend) => {
                // Only the two end anchors' handles move, and only the decorations read this
                // path: the artwork stays as committed (black old, `0031` criterion 16).
                for end in bend.ends {
                    if let Some(snapshot) = paths.iter_mut().find(|p| p.id == bend.path)
                        && let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == end.id)
                    {
                        anchor.handle_in = end.handle_in;
                        anchor.handle_out = end.handle_out;
                    }
                }
            }
            LiveNodeDrag::Nodes { positions } => {
                for (path, id, point) in positions {
                    if let Some(snapshot) = paths.iter_mut().find(|p| p.id == path)
                        && let Some(anchor) = snapshot.anchors.iter_mut().find(|a| a.id == id)
                    {
                        anchor.point = point;
                    }
                }
            }
            LiveNodeDrag::Handle {
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
        let mut state = self.node.toolbar_state(&self.document);
        state.compound_only = matches!(self.selection.ids(), [only]
            if self.document.path(*only).is_some_and(|path| path.is_compound()));
        state
    }

    /// The segment bend in flight past the drag threshold, as the pointer now would resolve it.
    pub(super) fn live_bend(&self) -> Option<BendResolution> {
        if self.tool != Tool::Node {
            return None;
        }
        let cursor = self.pointer_position?;
        match self.node.live_drag(cursor, self.held.shift)? {
            LiveNodeDrag::Bend(bend) => Some(bend),
            LiveNodeDrag::Nodes { .. } | LiveNodeDrag::Handle { .. } => None,
        }
    }

    /// The blue half of a bend (`0031` criterion 16): every segment the bend changes as a
    /// two-anchor open path with the bent path's own style, for the live preview outline. Empty
    /// when no bend runs. The cost is the number of changed segments (at most three), not the
    /// path's node count.
    pub(super) fn bend_preview_objects(&self, objects: &[ObjectSnapshot]) -> Vec<ObjectSnapshot> {
        let Some(bend) = self.live_bend() else {
            return Vec::new();
        };
        let Some(ObjectSnapshot::Path(path)) = objects.iter().find(|o| o.id() == bend.path) else {
            return Vec::new();
        };
        bend.changed_segments
            .iter()
            .map(|segment| {
                ObjectSnapshot::Path(PathSnapshot {
                    id: path.id,
                    closed: false,
                    style: path.style.clone(),
                    anchors: segment.to_vec(),
                    extra_subpaths: Vec::new(),
                    rotation: path.rotation,
                })
            })
            .collect()
    }

    /// The segment under the pointer if a press there would bend it: the Node tool's hit test
    /// found a segment (so the pointer is outside every node and handle radius) and it has a
    /// grab point. `None` during any drag.
    pub(super) fn hovered_segment_hit(&self, paths: &[PathSnapshot], point: Point) -> Option<Hit> {
        if self.node.drag_in_flight() {
            return None;
        }
        let hit = curvyo_ui_core::hit_test(
            paths,
            self.node.selection(),
            point,
            self.point_tolerance(),
            self.handle_tolerance(),
            self.segment_tolerance(),
        );
        match hit {
            Some(Hit::Segment { path, start, end })
                if !paths
                    .iter()
                    .find(|p| p.id == path)
                    .is_some_and(|p| segment_is_bendable(p, start, end)) =>
            {
                None
            }
            other => other,
        }
    }

    pub(super) fn decoration_input(&self) -> DecorationInput {
        if self.tool != Tool::Node {
            return DecorationInput::default();
        }
        let bend = self.live_bend();
        let selection = self.node.selection();
        // `node_pairs` directly, not `nodes()` zipped with `path()`: the
        // selection can now genuinely span several path objects
        // (`specs/0006-path-merge-split-and-node-types/specification.md`
        // acceptance criteria 6, 7, 15), and `path()` reports `None` for
        // that case — zipping against it would silently render none of
        // the selected nodes as selected instead of all of them.
        let selected_nodes = selection.node_pairs().to_vec();
        // The selected-segment overlay of the dragged segment is not drawn while the bend runs
        // (`0031` criterion 16); it returns on release and on cancel.
        let selected_segment = if bend.is_some() {
            None
        } else {
            selection.segment_with_path()
        };
        let hovered = self.hovered.map(|hit| match hit {
            Hit::Node { path, anchor } => RenderHovered::Node(path, anchor),
            Hit::Handle { path, anchor, slot } => RenderHovered::Handle(path, anchor, slot),
            Hit::Segment { path, start, end } => RenderHovered::Segment(path, start, end),
        });
        let handle_nodes = bend
            .iter()
            .flat_map(|bend| bend.ends.iter().map(|end| (bend.path, end.id)))
            .collect();
        DecorationInput {
            show_nodes: true,
            selected_nodes,
            selected_segment,
            hovered,
            handle_nodes,
        }
    }
}
