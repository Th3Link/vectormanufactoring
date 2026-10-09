//! The node tool's state machine (acceptance criteria 7-14).
//!
//! Selection ([`NodeSelection`]) persists across calls; a drag's
//! in-flight geometry is ephemeral (ADR 0009 §2) and collapses into one
//! [`curvyo_document_core::Document::move_anchors`] or
//! `set_handle` commit on release.

use curvyo_document_core::{
    AnchorKind, Document, HandleSlot, NodeId, PathSnapshot, Point, Tolerance, Vec2,
    resolve_handle_pair,
};
use curvyo_geometry_core::{nearest_point_on_segment, subdivide_at_parameter};

use crate::hit_test::{Hit, hit_test};
use crate::{AnchorIdMinter, NodeSelection};

/// The three hit-test tolerances the node tool needs — 8px node, 16px
/// handle, 4px segment per `docs/design-system.md` (the handle radius
/// doubled from the node's own 2026-10-05, alongside the handle glyph's
/// doubled visual size — matching the customer's "hard to hit" report:
/// a visual-only size change would look right but still feel exactly as
/// hard to hit), converted to document millimetres by the caller before
/// any of these methods are called (ADR 0002 §3: every geometric
/// comparison takes an explicit `Tolerance`; this crate never reads a
/// screen pixel itself).
#[derive(Debug, Clone, Copy)]
pub struct HitTolerances {
    /// Bounds node hits.
    pub point: Tolerance,
    /// Bounds handle hits — wider than `point`, see this type's own doc
    /// comment.
    pub handle: Tolerance,
    /// Bounds segment hits.
    pub segment: Tolerance,
}

/// Which of the node-tool's contextual-toolbar actions apply right now
/// (`specification.md`'s UX notes: "Buttons disable (not hide) when
/// nothing selected/applicable"). Computed fresh from the current
/// selection by [`NodeTool::toolbar_state`], never stored.
///
/// Independent `bool`s rather than an enum: these map 1:1 to the toolbar
/// buttons `specification.md` names, each disabled on its own condition
/// (e.g. make-line and make-curve are each other's negation *given* a
/// segment is selected, but become simultaneously `false` together when
/// it isn't) — collapsing them into one flags enum would just re-derive
/// the same booleans at every call site.
///
/// `can_convert_to_smooth` is renamed `can_convert_to_symmetric`, and
/// `can_convert_to_asymmetric`/`can_join`/`can_split` are new
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`; acceptance
/// criteria 1, 2, 8, 12).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct NodeToolbarState {
    /// Insert: a segment is selected (splits it at its midpoint — the
    /// toolbar button has no pointer position to hit-test against,
    /// unlike acceptance criterion 12's double-click).
    pub can_insert: bool,
    /// Delete: at least one node is selected.
    pub can_delete: bool,
    /// Make corner: at least one node is selected. Stays enabled
    /// regardless of the selected node(s)' current kind — a multi-
    /// selection can mix kinds, and converting a node to the kind it
    /// already has is a no-op (acceptance criterion 16), not a state to
    /// guard against.
    pub can_convert_to_corner: bool,
    /// Make symmetric: see `can_convert_to_corner`. Renamed from
    /// `can_convert_to_smooth` (criterion 1's label-only rename).
    pub can_convert_to_symmetric: bool,
    /// Make asymmetric: see `can_convert_to_corner` (criterion 2).
    pub can_convert_to_asymmetric: bool,
    /// Make line: a segment is selected and it is not already a line
    /// (acceptance criterion 14's explicit disable example).
    pub can_make_line: bool,
    /// Make curve: a segment is selected and it is already a line.
    pub can_make_curve: bool,
    /// The Node tool is active and the only selected object is a compound path: it shows its
    /// outline but no node, and the host says why (`0016-boolean-operations` criterion 38).
    /// Set by the session, which sees the object selection; the node tool does not.
    pub compound_only: bool,
    /// Join: the current selection is exactly two endpoint nodes of
    /// open paths (same path or two different objects), not the two
    /// ends of a 2-anchor open path (acceptance criterion 8).
    pub can_join: bool,
    /// Split: the current selection is exactly one node, either an
    /// interior node of an open path or any node of a closed path
    /// (acceptance criterion 12).
    pub can_split: bool,
}

/// The node tool's state: its persistent [`NodeSelection`] plus whatever
/// drag is currently in flight.
#[derive(Debug, Default)]
pub struct NodeTool {
    selection: NodeSelection,
    drag: Drag,
}

#[derive(Debug, Default)]
enum Drag {
    #[default]
    None,
    /// Dragging one or more selected nodes, possibly across several path
    /// objects at once (acceptance criteria 8, 10;
    /// `specs/0006-path-merge-split-and-node-types/adrs.md`'s architect
    /// review: "Drag stays multi-path... a plain press on any contained
    /// node keeps the selection, whether it spans one path or several").
    Nodes {
        down_at: Point,
        starts: Vec<(NodeId, curvyo_document_core::AnchorId, Point)>,
    },
    /// Dragging one handle (acceptance criterion 9). `down_at` and
    /// `start_value` (the dragged slot's own value at press time) are
    /// recorded so the commit on release writes `start_value` moved by
    /// the press→release displacement — never the absolute pointer
    /// position (architect review: a press within hit tolerance but off
    /// the handle's exact tip would otherwise relocate it to wherever
    /// the click landed, even with no drag at all). `kind` and *both*
    /// starting handle values are cached too (an anchor's kind and its
    /// handles cannot change while one continuous drag gesture holds it,
    /// so caching them here is exact, not stale) so
    /// [`NodeTool::live_drag`] can call
    /// [`curvyo_document_core::resolve_handle_pair`] — the exact
    /// function [`curvyo_document_core::Document::set_handle`] itself
    /// calls to commit — rather than recomputing the mirror rule
    /// independently.
    Handle {
        path: NodeId,
        anchor: curvyo_document_core::AnchorId,
        slot: HandleSlot,
        down_at: Point,
        start_value: Vec2,
        kind: AnchorKind,
        start_handle_in: Vec2,
        start_handle_out: Vec2,
    },
}

/// What [`NodeTool::live_drag`] resolves a drag in flight to — mirrors
/// the two shapes [`NodeTool::pointer_up`] can commit (acceptance
/// criteria 8, 9, 10), built by the exact same resolution helpers
/// (`NodeTool::resolve_node_positions`/`resolve_handle_value`, and —
/// for a handle drag — `curvyo_document_core::resolve_handle_pair`
/// itself) so a live preview and the eventual commit can never disagree
/// — the same discipline `curvyo_ui_core::PenTool::pending_anchor` uses
/// for the pen tool's own live preview.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveNodeDrag {
    /// One or more selected nodes, each at its live (not yet committed)
    /// position — acceptance criteria 8, 10, possibly spanning several
    /// path objects at once (`specs/0006-.../adrs.md`'s architect
    /// review). Handles are not listed: they are stored relative to
    /// their own anchor (`specs/0002-path-node-editing/adrs.md` decision
    /// 2), so moving the anchor's `point` alone already keeps them
    /// correct, with no separate value to resolve.
    Nodes {
        /// Each selected node's path, id and live position.
        positions: Vec<(NodeId, curvyo_document_core::AnchorId, Point)>,
    },
    /// One anchor's live (not yet committed) `(handle_in, handle_out)`
    /// pair — acceptance criterion 9. Already fully resolved by
    /// [`curvyo_document_core::resolve_handle_pair`] (mirrored for a
    /// `Smooth` anchor, the other side passed through unchanged for a
    /// `Corner` one): the caller assigns both fields directly, with no
    /// slot/mirror logic of its own to get wrong.
    Handle {
        /// The handle's path.
        path: NodeId,
        /// The handle's own anchor.
        anchor: curvyo_document_core::AnchorId,
        /// The anchor's live `handle_in`.
        handle_in: Vec2,
        /// The anchor's live `handle_out`.
        handle_out: Vec2,
    },
}

/// What [`NodeTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerDownOutcome {
    /// Nothing was hit; the selection was cleared (unless `shift` was
    /// held, in which case it is left as-is).
    Missed,
    /// A node (or the start of a multi-node drag) was hit.
    Node,
    /// A handle was hit; a handle drag began.
    Handle,
    /// A segment was hit and is now the selection.
    Segment,
}

/// What [`NodeTool::pointer_up`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerUpOutcome {
    /// No drag was in flight.
    NoOp,
    /// Acceptance criteria 8, 10: one or more nodes were moved, one
    /// commit for the whole drag.
    NodesMoved,
    /// Acceptance criterion 9: one handle was moved.
    HandleMoved,
}

impl NodeTool {
    /// A tool with nothing selected and no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current selection.
    #[must_use]
    pub const fn selection(&self) -> &NodeSelection {
        &self.selection
    }

    /// Selects every anchor of `path` as one multi-node selection —
    /// `primitive-shapes`' "object to path" (acceptance criterion 17)
    /// uses this so a freshly converted primitive is immediately
    /// editable with the node tool, exactly as if the maker had drawn
    /// it with the pen tool and then selected every node by hand.
    pub fn select_all_anchors(&mut self, path: &PathSnapshot) {
        self.selection.clear();
        let Some((first, rest)) = path.anchors.split_first() else {
            return;
        };
        self.selection.select_single_node(path.id, first.id);
        for anchor in rest {
            self.selection.toggle_node(path.id, anchor.id);
        }
    }

    /// Escape, step 2 of `specs/0010-edit-interaction-polish/` criterion 42: cancels
    /// the node or handle drag in flight, writing nothing, and keeps the
    /// selection (criterion 45: one step per key). Returns whether there was a
    /// drag.
    pub fn cancel_drag(&mut self) -> bool {
        !matches!(std::mem::take(&mut self.drag), Drag::None)
    }

    /// Whether a node or handle drag is in flight.
    #[must_use]
    pub const fn drag_in_flight(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    /// Escape, step 3 of criterion 42 for the Node tool (criterion 45): clears
    /// the node and segment selection, leaving the path itself as it is.
    /// Returns whether anything was selected (`specs/0002-path-node-editing`'s
    /// "Escape with nothing selected is a no-op" still holds for this call).
    pub fn clear_selection(&mut self) -> bool {
        let had_selection = !self.selection.is_empty();
        self.selection.clear();
        had_selection
    }

    /// Acceptance criteria 7, 8, 9, 10, 14: the maker pressed the mouse
    /// button down at `point`. Updates selection and/or begins a drag.
    ///
    /// `specs/0006-path-merge-split-and-node-types/specification.md`
    /// acceptance criterion 7: shift-clicking a node on a different path
    /// than the ones already selected adds it (`toggle_node` no longer
    /// restricts this to one path). A *plain* click keeps the current
    /// selection whenever the clicked node is already part of it —
    /// regardless of which path it belongs to — rather than collapsing a
    /// cross-path selection down to just the clicked node
    /// (`specs/0006-.../adrs.md`'s architect review: "Rejected: collapsing
    /// a cross-path selection on a plain press. It breaks AC 10 across
    /// paths"). Only a click on a node that is *not* already selected
    /// replaces the selection with that one node; a click that hits
    /// nothing clears the selection unless `shift` is held.
    pub fn pointer_down(
        &mut self,
        paths: &[PathSnapshot],
        point: Point,
        tolerances: HitTolerances,
        shift: bool,
    ) -> PointerDownOutcome {
        match hit_test(
            paths,
            &self.selection,
            point,
            tolerances.point,
            tolerances.handle,
            tolerances.segment,
        ) {
            Some(Hit::Handle { path, anchor, slot }) => {
                self.begin_handle_drag(paths, path, anchor, slot, point);
                PointerDownOutcome::Handle
            }
            Some(Hit::Node { path, anchor }) => {
                if shift {
                    self.selection.toggle_node(path, anchor);
                } else if !self.selection.contains_node(anchor) {
                    self.selection.select_single_node(path, anchor);
                }
                self.begin_node_drag(paths, point);
                PointerDownOutcome::Node
            }
            Some(Hit::Segment { path, start, end }) => {
                self.selection.select_segment(path, start, end);
                PointerDownOutcome::Segment
            }
            None => {
                if !shift {
                    self.selection.clear();
                }
                PointerDownOutcome::Missed
            }
        }
    }

    /// Begins a drag of every currently selected node, resolved against
    /// whichever of `paths`' snapshots each one belongs to — the drag can
    /// span several path objects at once, exactly like the resulting
    /// commit (`specs/0006-.../adrs.md`'s architect review: "Drag stays
    /// multi-path, as decided above... a plain press on any contained
    /// node keeps the selection, whether it spans one path or several").
    /// A selected node that no longer resolves (deleted since selection,
    /// `ADR 0009 §2`) is silently excluded from `starts`, not dragged.
    fn begin_node_drag(&mut self, paths: &[PathSnapshot], down_at: Point) {
        let starts = self
            .selection
            .node_pairs()
            .iter()
            .filter_map(|&(path, anchor_id)| {
                paths
                    .iter()
                    .find(|p| p.id == path)
                    .and_then(|snapshot| snapshot.anchors.iter().find(|a| a.id == anchor_id))
                    .map(|a| (path, anchor_id, a.point))
            })
            .collect();
        self.drag = Drag::Nodes { down_at, starts };
    }

    /// Records the handle's current value (and its anchor's kind, for
    /// [`NodeTool::live_drag`]'s mirror preview) at press time, so the
    /// commit on release can move it *relative to that starting value*
    /// rather than writing wherever the pointer happens to end up (see
    /// [`Drag::Handle`]'s own doc comment for why that distinction
    /// matters). Falls back to [`Vec2::ZERO`]/[`AnchorKind::Corner`] if
    /// `path`/`anchor` cannot be resolved in `paths` — the subsequent
    /// `set_handle` call on release will refuse against the live document
    /// anyway.
    fn begin_handle_drag(
        &mut self,
        paths: &[PathSnapshot],
        path: NodeId,
        anchor: curvyo_document_core::AnchorId,
        slot: HandleSlot,
        down_at: Point,
    ) {
        let found = paths
            .iter()
            .find(|p| p.id == path)
            .and_then(|snapshot| snapshot.anchors.iter().find(|a| a.id == anchor));
        let start_handle_in = found.map_or(Vec2::ZERO, |a| a.handle_in);
        let start_handle_out = found.map_or(Vec2::ZERO, |a| a.handle_out);
        let start_value = match slot {
            HandleSlot::In => start_handle_in,
            HandleSlot::Out => start_handle_out,
        };
        let kind = found.map_or(AnchorKind::Corner, |a| a.kind);
        self.drag = Drag::Handle {
            path,
            anchor,
            slot,
            down_at,
            start_value,
            kind,
            start_handle_in,
            start_handle_out,
        };
    }

    /// Acceptance criteria 8, 10's shared resolution: `starts` (each
    /// selected node's id and position at press time), moved by the
    /// press→`release` displacement. The one place this formula lives —
    /// both [`NodeTool::pointer_up`] (the commit) and
    /// [`NodeTool::live_drag`] (the preview) call it, so they cannot
    /// independently drift apart.
    fn resolve_node_positions(
        down_at: Point,
        starts: &[(NodeId, curvyo_document_core::AnchorId, Point)],
        release: Point,
    ) -> Vec<(NodeId, curvyo_document_core::AnchorId, Point)> {
        let delta = down_at.vector_to(release);
        starts
            .iter()
            .map(|&(path, id, p)| (path, id, p.translated(delta)))
            .collect()
    }

    /// Acceptance criterion 9's shared resolution: `start_value` (the
    /// handle's value at press time), moved by the press→`release`
    /// displacement. The one place this formula lives — both
    /// [`NodeTool::pointer_up`] and [`NodeTool::live_drag`] call it.
    fn resolve_handle_value(down_at: Point, start_value: Vec2, release: Point) -> Vec2 {
        let delta = down_at.vector_to(release);
        Vec2::new(start_value.x + delta.x, start_value.y + delta.y)
    }

    /// What the node tool's drag in flight (if any) would commit if
    /// released at `cursor` right now — for the renderer's live preview
    /// (acceptance criteria 8, 9, 10's "update live during the drag").
    /// Resolved by the exact same helpers [`NodeTool::pointer_up`] itself
    /// calls (`resolve_node_positions`/`resolve_handle_value`, both
    /// private: this crate's own internal resolution, not part of its
    /// public surface), so the preview and the eventual commit can never
    /// disagree. `None` when no drag is in flight.
    #[must_use]
    pub fn live_drag(&self, cursor: Point) -> Option<LiveNodeDrag> {
        match &self.drag {
            Drag::None => None,
            Drag::Nodes { down_at, starts } => Some(LiveNodeDrag::Nodes {
                positions: Self::resolve_node_positions(*down_at, starts, cursor),
            }),
            Drag::Handle {
                path,
                anchor,
                slot,
                down_at,
                start_value,
                kind,
                start_handle_in,
                start_handle_out,
            } => {
                let value = Self::resolve_handle_value(*down_at, *start_value, cursor);
                // The exact same function `Document::set_handle` itself
                // calls to commit — not a re-derivation of its mirror
                // rule.
                let (handle_in, handle_out) =
                    resolve_handle_pair(*kind, *slot, value, *start_handle_in, *start_handle_out);
                Some(LiveNodeDrag::Handle {
                    path: *path,
                    anchor: *anchor,
                    handle_in,
                    handle_out,
                })
            }
        }
    }

    /// The maker released the mouse button at `point`, ending whatever
    /// drag [`NodeTool::pointer_down`] began. Commits the drag as exactly
    /// one command.
    pub fn pointer_up(&mut self, document: &Document, point: Point) -> PointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            Drag::None => PointerUpOutcome::NoOp,
            Drag::Nodes { down_at, starts } => {
                // A press and release at the exact same point writes
                // nothing (`specs/0002-path-node-editing/adrs.md`'s dated
                // architect-review note): under ADR 0009 §3, `point` is
                // an LWW register, so re-writing the same value is still
                // a *new* operation with a newer clock — it can beat a
                // collaborator's real concurrent move of the same node
                // on merge, and it would put an empty step into slice
                // 5's undo for every selecting click. Exact equality,
                // not a geometric `Tolerance` (CLAUDE.md §5): a pixel-
                // jitter threshold against hand tremor is a separate
                // ux-engineer decision, not part of this rule.
                if point == down_at {
                    return PointerUpOutcome::NoOp;
                }
                let moves = Self::resolve_node_positions(down_at, &starts, point);
                let _ = document.move_anchors(&moves);
                PointerUpOutcome::NodesMoved
            }
            Drag::Handle {
                path,
                anchor,
                slot,
                down_at,
                start_value,
                kind: _,
                start_handle_in: _,
                start_handle_out: _,
            } => {
                // Same zero-movement rule as the node-drag branch above,
                // and for the same reason.
                if point == down_at {
                    return PointerUpOutcome::NoOp;
                }
                let value = Self::resolve_handle_value(down_at, start_value, point);
                let _ = document.set_handle(path, anchor, slot, value);
                PointerUpOutcome::HandleMoved
            }
        }
    }

    /// Acceptance criterion 11: converts every currently selected node to
    /// `kind`, as one commit for the whole multi-selection. A no-op when
    /// the selection is not a node selection.
    pub fn convert_selected(&self, document: &Document, kind: AnchorKind) {
        let Some(path) = self.selection.path() else {
            return;
        };
        let ids = self.selection.nodes();
        if ids.is_empty() {
            return;
        }
        let _ = document.convert_anchor_kind(path, &ids, kind);
    }

    /// Acceptance criterion 13: deletes every currently selected node,
    /// then clears the selection (the deleted ids can no longer resolve).
    /// A no-op when the selection is not a node selection.
    pub fn delete_selected(&mut self, document: &Document) {
        let Some(path) = self.selection.path() else {
            return;
        };
        let ids = self.selection.nodes();
        if ids.is_empty() {
            return;
        }
        let _ = document.delete_anchors(path, &ids);
        self.selection.clear();
    }

    /// Acceptance criterion 14: switches the selected segment to a
    /// straight line. A no-op when the selection is not a segment.
    pub fn make_line(&self, document: &Document) {
        if let Some((path, start, end)) = self.selection_segment() {
            let _ = document.set_segment_line(path, start, end);
        }
    }

    /// Acceptance criterion 14: switches the selected segment to a
    /// curve. A no-op when the selection is not a segment.
    pub fn make_curve(&self, document: &Document) {
        if let Some((path, start, end)) = self.selection_segment() {
            let _ = document.set_segment_curve(path, start, end);
        }
    }

    fn selection_segment(
        &self,
    ) -> Option<(
        NodeId,
        curvyo_document_core::AnchorId,
        curvyo_document_core::AnchorId,
    )> {
        let path = self.selection.path()?;
        let (start, end) = self.selection.segment()?;
        Some((path, start, end))
    }

    /// Which contextual-toolbar actions apply right now, computed from
    /// this tool's current selection against `document`'s live state —
    /// the facade (`curvyo-editor-wasm`'s `Session`) just calls this
    /// rather than re-deriving the same booleans itself.
    #[must_use]
    pub fn toolbar_state(&self, document: &Document) -> NodeToolbarState {
        let has_nodes = !self.selection.nodes().is_empty();
        let segment_is_line = self
            .selection
            .path()
            .zip(self.selection.segment())
            .and_then(|(path, (start, end))| {
                let snapshot = document.path(path)?;
                let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
                let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;
                Some(start_anchor.handle_out == Vec2::ZERO && end_anchor.handle_in == Vec2::ZERO)
            });
        NodeToolbarState {
            can_insert: segment_is_line.is_some(),
            can_delete: has_nodes,
            can_convert_to_corner: has_nodes,
            can_convert_to_symmetric: has_nodes,
            can_convert_to_asymmetric: has_nodes,
            can_make_line: segment_is_line == Some(false),
            can_make_curve: segment_is_line == Some(true),
            compound_only: false,
            can_join: self.can_join(document),
            can_split: self.can_split(document),
        }
    }

    /// Acceptance criteria 8-11: Join. A no-op (no commit) when the
    /// current selection does not qualify — [`NodeTool::can_join`]'s own
    /// condition, which `Document::join_endpoints` independently refuses
    /// on too. On success, selects only the merged node (criterion 11),
    /// so the maker can immediately continue working at the junction.
    pub fn join_selected(&mut self, document: &Document) {
        let Some((a, b)) = self.selection.join_pairs() else {
            return;
        };
        let Ok((path, anchor)) = document.join_endpoints(a.0, a.1, b.0, b.1) else {
            return;
        };
        self.selection.select_single_node(path, anchor);
    }

    /// Whether [`NodeTool::join_selected`] would do anything right now
    /// (acceptance criterion 8): the current selection names exactly two
    /// `(path, anchor)` pairs (same-path AC 10, or Split's own two-
    /// object result AC 15 — see [`NodeSelection::join_pairs`] for why
    /// those are the only two sources), and `document.check_join` — the
    /// same refusal rule `Document::join_endpoints` itself runs,
    /// checked here rather than re-derived independently (architect
    /// review: the same "one rule, one place" principle
    /// `resolve_handle_pair` already follows) — accepts them.
    #[must_use]
    pub fn can_join(&self, document: &Document) -> bool {
        self.selection
            .join_pairs()
            .is_some_and(|(a, b)| document.check_join(a.0, a.1, b.0, b.1))
    }

    /// Acceptance criteria 12-15: Split. A no-op (no commit, and
    /// `minter` is not advanced) when the current selection is not
    /// exactly one node, or [`Document::split_at_anchor`] itself refuses.
    /// On success, selects exactly one of the two resulting coincident
    /// nodes (`specs/0010-edit-interaction-polish/` criterion 50, superseding
    /// `0006` criterion 15): the new second node, the copy that keeps the
    /// original outgoing handle (the first node of the new path object for
    /// an open path, the new first node of the opened path for a closed
    /// one), which is the one `minter` just minted an id for. A press at the
    /// shared position then hits it ([`hit_test`]'s tie rule) and a drag
    /// moves that node only.
    pub fn split_selected(&mut self, minter: &mut AnchorIdMinter, document: &Document) {
        let [(path, anchor)] = self.selection.node_pairs() else {
            return;
        };
        let (path, anchor) = (*path, *anchor);
        let new_id = minter.mint();
        let Ok((first, second)) = document.split_at_anchor(path, anchor, new_id) else {
            return;
        };
        let second_node = if second.1 == new_id { second } else { first };
        self.selection
            .select_single_node(second_node.0, second_node.1);
    }

    /// Whether [`NodeTool::split_selected`] would do anything right now
    /// (acceptance criterion 12): the current selection is exactly one
    /// node, and `document.check_split` — the same refusal rule
    /// `Document::split_at_anchor` itself runs — accepts it (see
    /// [`NodeTool::can_join`]'s own doc comment for why this checks the
    /// document-core rule rather than a local copy of it).
    #[must_use]
    pub fn can_split(&self, document: &Document) -> bool {
        let [(path, anchor)] = self.selection.node_pairs() else {
            return false;
        };
        document.check_split(*path, *anchor)
    }

    /// Acceptance criterion 12: double-clicking a point on a segment
    /// (not on an existing node) inserts a new corner node there,
    /// splitting the segment with the path's visible shape unchanged at
    /// the instant of insertion. A no-op (returns `None`) when `point`
    /// does not land on a segment.
    ///
    /// Double-click detection itself is the frontend's job
    /// (`specs/0002-path-node-editing/adrs.md`'s `PenTool` doc comment makes
    /// the same point) — this is a direct action the caller invokes once
    /// it has decided a double-click landed on a segment.
    pub fn insert_at(
        &mut self,
        minter: &mut AnchorIdMinter,
        document: &Document,
        paths: &[PathSnapshot],
        point: Point,
        tolerances: HitTolerances,
    ) -> Option<NodeId> {
        // Hit-test with an empty selection (handles never hittable) and
        // the real node tolerance, so a double-click close enough to an
        // existing node to plausibly mean "that node" is read as a node
        // hit and refused here — never silently read as "the segment
        // underneath it" just because this crate used a laxer tolerance
        // than the one the maker's click was actually judged against.
        let Hit::Segment { path, start, end } = hit_test(
            paths,
            &NodeSelection::new(),
            point,
            tolerances.point,
            tolerances.handle,
            tolerances.segment,
        )?
        else {
            return None;
        };
        let snapshot = paths.iter().find(|p| p.id == path)?;
        let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
        let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;

        let (t, _, _) = nearest_point_on_segment(
            start_anchor.point,
            start_anchor.handle_out,
            end_anchor.handle_in,
            end_anchor.point,
            point,
            tolerances.segment,
        );
        let subdivision = subdivide_at_parameter(
            start_anchor.point,
            start_anchor.handle_out,
            end_anchor.handle_in,
            end_anchor.point,
            t,
        );
        let new_anchor = curvyo_document_core::NewAnchor {
            id: minter.mint(),
            point: subdivision.new_point,
            handle_in: subdivision.new_handle_in,
            handle_out: subdivision.new_handle_out,
            kind: AnchorKind::Corner,
        };
        document
            .insert_anchor(
                path,
                start,
                new_anchor,
                subdivision.prev_out,
                subdivision.next_in,
            )
            .ok()?;
        // A prior single click (e.g. the first half of the double-click
        // that landed here) may have selected this same segment as
        // `(start, end)`; that pair is no longer adjacent once the new
        // anchor sits between them, so a selection referencing it would
        // resolve to a stale, visually wrong overlay. Clear it, matching
        // `insert_on_selected_segment`'s and `delete_selected`'s own rule
        // that a structural change invalidates whatever it touches.
        self.selection.clear();
        Some(path)
    }

    /// The contextual toolbar's "Insert node" action
    /// (`specification.md`'s UX notes list it alongside Delete/convert/
    /// make-line/make-curve). Unlike [`NodeTool::insert_at`] this has no
    /// pointer position to hit-test against — the toolbar button only
    /// knows the current selection — so it splits the selected segment at
    /// its midpoint (`t = 0.5`), the same well-defined default Inkscape's
    /// own "insert node" toolbar action uses. A no-op (returns `None`)
    /// when the selection is not a segment.
    pub fn insert_on_selected_segment(
        &mut self,
        minter: &mut AnchorIdMinter,
        document: &Document,
        paths: &[PathSnapshot],
    ) -> Option<NodeId> {
        let (path, start, end) = self.selection_segment()?;
        let snapshot = paths.iter().find(|p| p.id == path)?;
        let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
        let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;

        let subdivision = subdivide_at_parameter(
            start_anchor.point,
            start_anchor.handle_out,
            end_anchor.handle_in,
            end_anchor.point,
            0.5,
        );
        let new_anchor = curvyo_document_core::NewAnchor {
            id: minter.mint(),
            point: subdivision.new_point,
            handle_in: subdivision.new_handle_in,
            handle_out: subdivision.new_handle_out,
            kind: AnchorKind::Corner,
        };
        document
            .insert_anchor(
                path,
                start,
                new_anchor,
                subdivision.prev_out,
                subdivision.next_in,
            )
            .ok()?;
        // The old (start, end) pair is no longer adjacent, so the segment
        // selection can no longer resolve — clear it rather than leave it
        // dangling, matching `delete_selected`'s same choice.
        self.selection.clear();
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, NewAnchor, Vec2};

    use super::*;

    const TOLERANCES: HitTolerances = HitTolerances {
        point: Tolerance::from_mm(2.0),
        handle: Tolerance::from_mm(4.0),
        segment: Tolerance::from_mm(1.0),
    };

    fn open_two_node_path(document: &Document, a: AnchorId, b: AnchorId) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        )
    }

    /// AC7: clicking a node selects it.
    #[test]
    fn ac7_clicking_a_node_selects_it() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];

        let mut tool = NodeTool::new();
        let outcome = tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Node);
        assert_eq!(tool.selection().nodes(), &[a]);
    }

    /// AC8: dragging a selected node moves it (and only it) to the
    /// release point; handles stay unchanged relative to it.
    #[test]
    fn ac8_dragging_a_node_moves_it_with_its_handles_unchanged() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-3.0, 0.0),
                    handle_out: Vec2::new(3.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(5.0, 7.0));
        assert_eq!(outcome, PointerUpOutcome::NodesMoved);

        let snapshot = document.path(path).expect("exists");
        let moved = &snapshot.anchors[0];
        assert_eq!(moved.point, Point::new(5.0, 7.0));
        assert_eq!(moved.handle_in, Vec2::new(-3.0, 0.0));
        assert_eq!(moved.handle_out, Vec2::new(3.0, 0.0));
    }

    /// The bug this run fixes: AC8/10's "update live during the drag"
    /// had no accessor at all — `live_drag` is `None` with nothing
    /// pressed, and resolves to the live (not yet committed) position
    /// while a node drag is in flight, matching exactly what
    /// `pointer_up` would commit at the same release point.
    #[test]
    fn live_drag_reports_the_live_node_position_mid_drag() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        assert_eq!(
            tool.live_drag(Point::new(0.0, 0.0)),
            None,
            "nothing pressed"
        );

        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        assert_eq!(
            tool.live_drag(Point::new(5.0, 7.0)),
            Some(LiveNodeDrag::Nodes {
                positions: vec![(path, a, Point::new(5.0, 7.0))],
            }),
            "mid-drag, before release"
        );

        // The commit, at the same release point, must match.
        tool.pointer_up(&document, Point::new(5.0, 7.0));
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(5.0, 7.0));
        assert_eq!(
            tool.live_drag(Point::new(5.0, 7.0)),
            None,
            "released: no drag in flight any more"
        );
    }

    /// AC9: dragging one handle of a smooth node mirrors the opposite
    /// one; dragging one handle of a corner node leaves the other alone.
    #[test]
    fn ac9_smooth_handle_drag_mirrors_the_opposite_handle() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        // Select the node first (handles are only hittable once selected).
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        let paths = vec![document.path(path).expect("exists")];
        let outcome = tool.pointer_down(&paths, Point::new(5.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Handle);
        tool.pointer_up(&document, Point::new(3.0, 4.0));

        let snapshot = document.path(path).expect("exists");
        let anchor = &snapshot.anchors[0];
        assert_eq!(anchor.handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(anchor.handle_in, Vec2::new(-3.0, -4.0));
    }

    #[test]
    fn ac9_corner_handle_drag_leaves_the_other_handle_alone() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Corner,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(5.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(3.0, 4.0));

        let snapshot = document.path(path).expect("exists");
        let anchor = &snapshot.anchors[0];
        assert_eq!(anchor.handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(anchor.handle_in, Vec2::new(-5.0, 0.0), "untouched");
    }

    /// The bug this run fixes, for AC9's handle drag: a smooth anchor's
    /// live preview mirrors the opposite handle, matching
    /// `Document::set_handle`'s own rule — not just the eventual commit.
    #[test]
    fn live_drag_mirrors_a_smooth_anchors_opposite_handle() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(5.0, 0.0), TOLERANCES, false);
        assert_eq!(
            tool.live_drag(Point::new(3.0, 4.0)),
            Some(LiveNodeDrag::Handle {
                path,
                anchor: a,
                handle_in: Vec2::new(-3.0, -4.0),
                handle_out: Vec2::new(3.0, 4.0),
            }),
            "mid-drag: the opposite handle's live mirror must already show, not just on release"
        );

        // The commit, at the same release point, must match.
        tool.pointer_up(&document, Point::new(3.0, 4.0));
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(-3.0, -4.0));
    }

    /// Same bug, the corner-node half: no mirror is previewed either,
    /// matching `pointer_up`'s own "untouched" commit.
    #[test]
    fn live_drag_does_not_mirror_a_corner_anchors_handle() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Corner,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(5.0, 0.0), TOLERANCES, false);
        assert_eq!(
            tool.live_drag(Point::new(3.0, 4.0)),
            Some(LiveNodeDrag::Handle {
                path,
                anchor: a,
                handle_in: Vec2::new(-5.0, 0.0),
                handle_out: Vec2::new(3.0, 4.0),
            }),
        );
    }

    /// AC10: shift-click adds to the selection; dragging any one of the
    /// selected nodes moves every selected node by the same offset.
    #[test]
    fn ac10_multi_selected_nodes_move_by_the_same_offset() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(10.0, 0.0)),
                NewAnchor::corner(c, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(20.0, 0.0), TOLERANCES, true);
        assert_eq!(tool.selection().nodes(), &[a, c]);

        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(20.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(25.0, 5.0));

        let snapshot = document.path(path).expect("exists");
        let by_id = |id: AnchorId| snapshot.anchors.iter().find(|x| x.id == id).unwrap().point;
        assert_eq!(by_id(a), Point::new(5.0, 5.0));
        assert_eq!(by_id(c), Point::new(25.0, 5.0));
        assert_eq!(by_id(b), Point::new(10.0, 0.0), "not selected, untouched");
    }

    /// AC11: converting a corner to smooth pulls out mirrored handles;
    /// converting smooth to corner leaves handles exactly where they are.
    #[test]
    fn ac11_convert_corner_to_smooth_and_back() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(10.0, 0.0)),
                NewAnchor::corner(c, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);

        tool.convert_selected(&document, AnchorKind::Symmetric);
        let snapshot = document.path(path).expect("exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Symmetric);
        assert_eq!(middle.handle_in, middle.handle_out.negated());

        tool.convert_selected(&document, AnchorKind::Corner);
        let snapshot = document.path(path).expect("exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Corner);
    }

    /// AC12: double-clicking a segment point inserts a corner node there,
    /// splitting the segment without changing the visible shape.
    #[test]
    fn ac12_insert_at_splits_the_segment() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(1);

        let result = tool.insert_at(
            &mut minter,
            &document,
            &paths,
            Point::new(10.0, 0.0),
            TOLERANCES,
        );
        assert_eq!(result, Some(path));

        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        // The nearest-point search is accurate to a fixed small epsilon,
        // not bit-exact: compare with an explicit tolerance.
        let inserted = snapshot.anchors[1].point;
        assert!((inserted.x - 10.0).abs() < 1e-4 && inserted.y.abs() < 1e-4);
        assert_eq!(snapshot.anchors[1].kind, AnchorKind::Corner);
    }

    /// A segment selected (e.g. by an earlier click at the same point —
    /// the first half of a double-click) before `insert_at` splits it is
    /// no longer adjacent afterwards, so the selection is cleared rather
    /// than left dangling.
    #[test]
    fn insert_at_clears_a_stale_segment_selection_it_just_split() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(1);

        let outcome = tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Segment);
        assert_eq!(tool.selection().segment(), Some((a, b)));

        tool.insert_at(
            &mut minter,
            &document,
            &paths,
            Point::new(10.0, 0.0),
            TOLERANCES,
        );
        assert!(
            tool.selection().is_empty(),
            "inserting a node away from the path does not select it"
        );
    }

    /// The contextual toolbar's "Insert node" button: splits the selected
    /// segment at its midpoint, with no pointer position involved.
    #[test]
    fn insert_on_selected_segment_splits_at_the_midpoint() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(1);

        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        assert_eq!(tool.selection().segment(), Some((a, b)));

        let paths = vec![document.path(path).expect("exists")];
        let result = tool.insert_on_selected_segment(&mut minter, &document, &paths);
        assert_eq!(result, Some(path));
        assert!(
            tool.selection().is_empty(),
            "stale segment selection cleared"
        );

        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 3);
        assert_eq!(snapshot.anchors[1].point, Point::new(10.0, 0.0));
        assert_eq!(snapshot.anchors[1].kind, AnchorKind::Corner);
    }

    /// A no-op when nothing is selected, or a node (not a segment) is.
    #[test]
    fn insert_on_selected_segment_is_a_no_op_without_a_segment_selection() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(1);

        let result = tool.insert_on_selected_segment(&mut minter, &document, &paths);
        assert_eq!(result, None);
        assert_eq!(document.path(path).expect("exists").anchors.len(), 2);
    }

    #[test]
    fn insert_at_on_an_existing_node_is_a_no_op() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(1);

        let result = tool.insert_at(
            &mut minter,
            &document,
            &paths,
            Point::new(0.0, 0.0),
            TOLERANCES,
        );
        assert_eq!(result, None);
        assert_eq!(document.path(path).expect("exists").anchors.len(), 2);
    }

    /// AC13: deleting selected nodes joins the remaining neighbours; the
    /// selection then can no longer resolve.
    #[test]
    fn ac13_delete_selected_removes_the_nodes_and_clears_selection() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(10.0, 0.0)),
                NewAnchor::corner(c, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);

        tool.delete_selected(&document);
        assert!(
            tool.selection().is_empty(),
            "deleting the selected nodes also clears the selection"
        );
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors.len(), 2);
    }

    /// AC14: clicking a segment selects it distinctly from either node;
    /// make-line/make-curve switch it without moving the endpoints.
    #[test]
    fn ac14_select_segment_and_make_line_then_curve() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: b,
                    point: Point::new(20.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Symmetric,
                },
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let outcome = tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Segment);
        assert_eq!(tool.selection().segment(), Some((a, b)));
        assert_eq!(tool.selection().nodes(), &[]);

        tool.make_line(&document);
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors[0].handle_out, Vec2::ZERO);
        assert_eq!(snapshot.anchors[1].handle_in, Vec2::ZERO);
        assert_eq!(
            snapshot.anchors[0].point,
            Point::new(0.0, 0.0),
            "endpoint unmoved"
        );
        assert_eq!(
            snapshot.anchors[1].point,
            Point::new(20.0, 0.0),
            "endpoint unmoved"
        );

        tool.make_curve(&document);
        let snapshot = document.path(path).expect("exists");
        assert_ne!(snapshot.anchors[0].handle_out, Vec2::ZERO);
        assert_ne!(snapshot.anchors[1].handle_in, Vec2::ZERO);
    }

    // Note: a plain click (press and release at the exact same point) is
    // a zero-delta move, and `pointer_up` commits it like any other move
    // rather than special-casing it as a no-op — AC8's own acceptance
    // test (`ac8_zero_delta_drag_is_a_well_defined_no_move` in
    // `tests/acceptance_0002.rs`) pins this as "still a valid move
    // commit... not a no-op that skips writing". See this run's report
    // for an open conflict against a later review note that asked for
    // the opposite.

    /// Escape mid-drag is one step (`specs/0010-edit-interaction-polish/`
    /// criteria 42 and 45): it cancels the drag and keeps the selection; the
    /// release that would otherwise end the drag is then a no-op and the node
    /// never moves.
    #[test]
    fn escape_mid_drag_cancels_only_the_drag_and_keeps_the_selection() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();

        // Select the node first, then start a second, real drag on it.
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));
        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(tool.drag_in_flight());

        assert!(tool.cancel_drag(), "a drag was in flight to cancel");
        assert!(!tool.drag_in_flight());
        assert!(
            tool.selection().contains_node(a),
            "criterion 45: the selection is kept"
        );
        assert!(!tool.cancel_drag(), "nothing more to cancel");

        let outcome = tool.pointer_up(&document, Point::new(50.0, 50.0));
        assert_eq!(outcome, PointerUpOutcome::NoOp, "the drag was cancelled");
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0), "unmoved");
    }

    /// Criterion 45: with a node or segment selected the next step clears it
    /// (the path stays); with none selected there is nothing left to clear.
    #[test]
    fn clearing_the_selection_is_a_no_op_once_nothing_is_selected() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));

        assert!(tool.clear_selection());
        assert!(tool.selection().is_empty(), "the selection is cleared");
        assert!(!tool.clear_selection());
        assert!(document.path(path).is_some(), "the path itself stays");
    }

    /// A segment selection counts as "selected" for the same step.
    #[test]
    fn a_segment_selection_is_cleared_by_the_same_step() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let outcome = tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Segment);
        tool.pointer_up(&document, Point::new(10.0, 0.0));
        assert!(tool.clear_selection());
        assert!(tool.selection().is_empty());
    }

    #[test]
    fn clicking_empty_space_clears_the_selection() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(!tool.selection().is_empty(), "clicking the node selects it");

        let outcome = tool.pointer_down(&paths, Point::new(1000.0, 1000.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Missed);
        assert!(
            tool.selection().is_empty(),
            "clicking empty space clears the selection"
        );
    }

    /// Acceptance criterion 1: the existing two-kind toggle is renamed
    /// at the toolbar-state surface (`can_convert_to_smooth` →
    /// `can_convert_to_symmetric`), with the identical enablement rule.
    #[test]
    fn toolbar_state_exposes_symmetric_and_asymmetric_and_join_split() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);

        let state = tool.toolbar_state(&document);
        assert!(state.can_convert_to_corner);
        assert!(state.can_convert_to_symmetric);
        assert!(state.can_convert_to_asymmetric);
        // A 2-anchor open path's one endpoint: Join is disabled (would
        // need the *other* endpoint too), Split is disabled (an
        // endpoint, not an interior/closed node).
        assert!(!state.can_join);
        assert!(!state.can_split);
    }

    /// AC8/AC10/AC11: selecting the two ends of one open path enables
    /// Join; triggering it closes the path and selects only the merged
    /// node.
    #[test]
    fn join_selected_closes_one_open_path_and_selects_the_merged_node() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let c = AnchorId::new(1, 3);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(10.0, 0.0)),
                NewAnchor::corner(c, Point::new(5.0, 10.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        let paths = vec![document.path(path).expect("exists")];
        tool.pointer_down(&paths, Point::new(5.0, 10.0), TOLERANCES, true);
        assert_eq!(tool.selection().nodes(), &[a, c]);
        assert!(tool.toolbar_state(&document).can_join);

        tool.join_selected(&document);

        let snapshot = document.path(path).expect("exists");
        assert!(snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 2);
        assert_eq!(tool.selection().nodes().len(), 1, "only the merged node");
        let merged_id = tool.selection().nodes()[0];
        assert!(snapshot.anchors.iter().any(|anchor| anchor.id == merged_id));
    }

    /// `specs/0006-path-merge-split-and-node-types/specification.md`
    /// acceptance criterion 7: clicking a node on one path, then shift-
    /// clicking a node on a *different* visible path, selects both
    /// together — the behaviour `NodeSelection::toggle_node` now
    /// supports directly (it used to reset to a fresh single-node
    /// selection on any path change). Both paths are "visible" in the
    /// sense criterion 6 means: `NodeTool` hit-tests and renders every
    /// path handed to it, regardless of any outer object selection
    /// (unchanged, pre-existing behaviour this slice's own `adrs.md`
    /// relies on rather than re-implementing).
    #[test]
    fn ac7_shift_click_a_node_on_a_different_path_selects_both_together() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path_one = open_two_node_path(&document, a, b);
        let c = AnchorId::new(2, 1);
        let d = AnchorId::new(2, 2);
        let path_two = document.create_path(
            &[
                NewAnchor::corner(c, Point::new(0.0, 50.0)),
                NewAnchor::corner(d, Point::new(20.0, 50.0)),
            ],
            false,
        );

        let mut tool = NodeTool::new();
        let paths = vec![
            document.path(path_one).expect("exists"),
            document.path(path_two).expect("exists"),
        ];
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_down(&paths, Point::new(0.0, 50.0), TOLERANCES, true);

        assert_eq!(
            tool.selection().node_pairs(),
            &[(path_one, a), (path_two, c)],
            "both nodes selected together, across the two different paths (AC 7)"
        );
        assert_eq!(
            tool.selection().path(),
            None,
            "no single common path — a genuine cross-path selection"
        );

        // AC 9 (via AC 8's shared rule): both are endpoint nodes of open
        // paths, so Join applies to this selection too.
        assert!(tool.toolbar_state(&document).can_join);
    }

    /// Architect review of the AC 6/7 follow-up
    /// (`specs/0006-path-merge-split-and-node-types/adrs.md`): "Rejected:
    /// collapsing a cross-path selection on a plain press. It breaks
    /// AC 10 across paths." A plain (non-shift) click on a node that is
    /// already part of a cross-path selection must keep the whole
    /// selection — not reset it to just the clicked node — so a
    /// following drag moves every selected node, on every path it spans,
    /// as one `Document::move_anchors` commit.
    #[test]
    fn plain_click_on_a_cross_path_selected_node_keeps_the_whole_selection_and_drags_it_as_one_commit()
     {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path_one = open_two_node_path(&document, a, b);
        let c = AnchorId::new(2, 1);
        let d = AnchorId::new(2, 2);
        let path_two = document.create_path(
            &[
                NewAnchor::corner(c, Point::new(0.0, 50.0)),
                NewAnchor::corner(d, Point::new(20.0, 50.0)),
            ],
            false,
        );

        let mut tool = NodeTool::new();
        let paths = vec![
            document.path(path_one).expect("exists"),
            document.path(path_two).expect("exists"),
        ];
        // Build a cross-path selection the same way AC 7's own test does.
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_down(&paths, Point::new(0.0, 50.0), TOLERANCES, true);
        assert_eq!(
            tool.selection().node_pairs(),
            &[(path_one, a), (path_two, c)],
            "cross-path selection built as in AC 7"
        );

        // A *plain* click (no shift) back on one of the two already-
        // selected nodes must not collapse the selection to just that
        // node.
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        assert_eq!(
            tool.selection().node_pairs(),
            &[(path_one, a), (path_two, c)],
            "a plain press on an already-contained node keeps the whole \
             cross-path selection, whether it spans one path or several"
        );

        // Dragging from there moves both selected nodes, on both paths,
        // as one commit.
        let outcome = tool.pointer_up(&document, Point::new(3.0, 4.0));
        assert_eq!(outcome, PointerUpOutcome::NodesMoved);

        let moved_one = document.path(path_one).expect("exists");
        let moved_two = document.path(path_two).expect("exists");
        assert_eq!(
            moved_one.anchors[0].point,
            Point::new(3.0, 4.0),
            "path one's selected node moved by the drag offset"
        );
        assert_eq!(
            moved_two.anchors[0].point,
            Point::new(3.0, 54.0),
            "path two's selected node moved by the same offset, even \
             though the press landed on path one's node"
        );
    }

    /// A three-node open path with its middle node at (10, 0), and the Node
    /// tool with that node selected.
    fn open_path_with_the_middle_selected() -> (Document, NodeId, NodeTool) {
        let document = Document::new(1);
        let path = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(10.0, 0.0));
        (document, path, tool)
    }

    fn all_paths(document: &Document) -> Vec<curvyo_document_core::PathSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.path(id))
            .collect()
    }

    /// Criteria 50 to 52: splitting an interior node of an open path makes
    /// two objects and selects exactly one node, the first node of the new
    /// second object (the copy that keeps the outgoing handle); the other
    /// coincident node is not selected.
    #[test]
    fn split_selected_on_an_interior_node_selects_only_the_second_node() {
        let (document, path, mut tool) = open_path_with_the_middle_selected();
        let mut minter = AnchorIdMinter::new(9);
        assert!(tool.toolbar_state(&document).can_split);

        tool.split_selected(&mut minter, &document);

        let ids = document.object_ids();
        assert_eq!(ids.len(), 2, "two separate objects now");
        let second = *ids.iter().find(|id| **id != path).expect("the new object");
        let new_first = document.path(second).expect("exists").anchors[0].id;
        assert_eq!(
            tool.selection().node_pairs(),
            &[(second, new_first)],
            "exactly the new second object's first node"
        );
        assert!(
            !tool.selection().contains_node(AnchorId::new(1, 2)),
            "the first node, left on the original, is not selected"
        );
        // Criterion 52: one selected end node, so Split and Join are off.
        let state = tool.toolbar_state(&document);
        assert!(!state.can_split && !state.can_join);
        assert!(state.can_convert_to_corner && state.can_delete);
    }

    /// Criterion 50: a press at the shared position hits the selected node
    /// (the tie goes to it), keeps the selection, and a drag moves that node
    /// only: the end of one piece at (10, 8), the other still at (10, 0),
    /// without any prior click on empty canvas (criterion 51).
    #[test]
    fn after_a_split_a_drag_at_the_shared_point_moves_only_the_selected_end() {
        let (document, path, mut tool) = open_path_with_the_middle_selected();
        let mut minter = AnchorIdMinter::new(9);
        tool.split_selected(&mut minter, &document);
        let selected_before = tool.selection().node_pairs().to_vec();

        let paths = all_paths(&document);
        let outcome = tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Node);
        assert_eq!(
            tool.selection().node_pairs(),
            selected_before,
            "the press hit the selected node and kept the selection"
        );
        tool.pointer_up(&document, Point::new(10.0, 8.0));

        let original = document.path(path).expect("exists");
        assert_eq!(
            original.anchors.last().expect("anchor").point,
            Point::new(10.0, 0.0)
        );
        let other = document
            .object_ids()
            .into_iter()
            .find(|id| *id != path)
            .and_then(|id| document.path(id))
            .expect("the new object");
        assert_eq!(other.anchors[0].point, Point::new(10.0, 8.0));
        assert_eq!(
            other.anchors[1].point,
            Point::new(20.0, 0.0),
            "the rest stays"
        );
    }

    /// Criterion 51 for a closed path: the new first node of the opened path
    /// is the selected one and the drag moves it alone.
    #[test]
    fn split_selected_on_a_closed_path_node_selects_the_new_first_node_only() {
        let document = Document::new(1);
        let (a, b, c) = (
            AnchorId::new(1, 1),
            AnchorId::new(1, 2),
            AnchorId::new(1, 3),
        );
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(10.0, 0.0)),
                NewAnchor::corner(c, Point::new(5.0, 10.0)),
            ],
            true,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        let mut minter = AnchorIdMinter::new(9);
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(0.0, 0.0));
        assert!(tool.toolbar_state(&document).can_split);

        tool.split_selected(&mut minter, &document);

        let snapshot = document.path(path).expect("exists");
        assert!(!snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 4);
        let new_first = snapshot.anchors[0].id;
        assert_ne!(new_first, a, "the new copy leads the opened path");
        assert_eq!(tool.selection().node_pairs(), &[(path, new_first)]);
        assert!(
            !tool.selection().contains_node(a),
            "the old node is not selected"
        );

        let paths = vec![snapshot];
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(-3.0, 4.0));
        let moved = document.path(path).expect("exists");
        assert_eq!(moved.anchors[0].point, Point::new(-3.0, 4.0));
        assert_eq!(
            moved.anchors.last().expect("anchor").point,
            Point::new(0.0, 0.0)
        );
    }

    /// The new second node keeps the original outgoing handle; the first
    /// node's is retracted (`0006` criteria 13, 14, unchanged): the selected
    /// node is the one that carries the handle.
    #[test]
    fn the_selected_node_after_a_split_keeps_the_original_outgoing_handle() {
        let document = Document::new(1);
        let (a, b, c) = (
            AnchorId::new(1, 1),
            AnchorId::new(1, 2),
            AnchorId::new(1, 3),
        );
        let mut middle = NewAnchor::corner(b, Point::new(10.0, 0.0));
        middle.handle_out = Vec2::new(3.0, 2.0);
        for closed in [false, true] {
            let path = document.create_path(
                &[
                    NewAnchor::corner(a, Point::new(0.0, 0.0)),
                    middle,
                    NewAnchor::corner(c, Point::new(20.0, 5.0)),
                ],
                closed,
            );
            let paths = vec![document.path(path).expect("exists")];
            let mut tool = NodeTool::new();
            let mut minter = AnchorIdMinter::new(9);
            tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
            tool.pointer_up(&document, Point::new(10.0, 0.0));
            tool.split_selected(&mut minter, &document);
            let [(selected_path, selected)] = tool.selection().node_pairs() else {
                panic!("one selected node");
            };
            let snapshot = document.path(*selected_path).expect("exists");
            let node = snapshot
                .anchors
                .iter()
                .find(|n| n.id == *selected)
                .expect("node");
            assert_eq!(node.handle_out, Vec2::new(3.0, 2.0), "closed {closed}");
            assert_eq!(node.handle_in, Vec2::ZERO, "closed {closed}");
        }
    }

    /// Split then Re-Join is no longer one click (criterion 52): pull the
    /// selected end away, shift-click the other end, and Join restores one
    /// object.
    #[test]
    fn split_then_rejoin_restores_one_object() {
        let (document, _, mut tool) = open_path_with_the_middle_selected();
        let mut minter = AnchorIdMinter::new(9);
        tool.split_selected(&mut minter, &document);
        assert_eq!(document.object_ids().len(), 2);
        assert!(
            !tool.toolbar_state(&document).can_join,
            "one selected node cannot be joined"
        );

        // Pull the selected end away, then select the end left at (10, 0).
        let paths = all_paths(&document);
        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
        tool.pointer_up(&document, Point::new(10.0, 8.0));
        let paths = all_paths(&document);
        tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, true);
        tool.pointer_up(&document, Point::new(10.0, 0.0));
        assert!(tool.toolbar_state(&document).can_join, "two selected ends");

        tool.join_selected(&document);
        assert_eq!(document.object_ids().len(), 1, "back to one object");
    }
}
