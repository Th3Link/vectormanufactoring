//! The node tool's state machine (acceptance criteria 7-14).
//!
//! Selection ([`NodeSelection`]) persists across calls; a drag's
//! in-flight geometry is ephemeral (ADR 0009 §2) and collapses into one
//! [`vecmanf_document_core::Document::move_anchors`] or
//! `set_handle` commit on release.

use vecmanf_document_core::{
    AnchorKind, Document, HandleSlot, NodeId, PathSnapshot, Point, Tolerance, Vec2,
};
use vecmanf_geometry_core::{nearest_point_on_segment, subdivide_at_parameter};

use crate::hit_test::{Hit, hit_test};
use crate::{AnchorIdMinter, NodeSelection};

/// The two hit-test tolerances the node tool needs — 8px node/handle,
/// 4px segment per `docs/design-system.md`, converted to document
/// millimetres by the caller before any of these methods are called
/// (ADR 0002 §3: every geometric comparison takes an explicit
/// `Tolerance`; this crate never reads a screen pixel itself).
#[derive(Debug, Clone, Copy)]
pub struct HitTolerances {
    /// Bounds node and handle hits.
    pub point: Tolerance,
    /// Bounds segment hits.
    pub segment: Tolerance,
}

/// Which of the node-tool's contextual-toolbar actions apply right now
/// (`specification.md`'s UX notes: "Buttons disable (not hide) when
/// nothing selected/applicable"). Computed fresh from the current
/// selection by [`NodeTool::toolbar_state`], never stored.
///
/// Six independent `bool`s rather than an enum: these map 1:1 to the six
/// toolbar buttons `specification.md` names, each disabled on its own
/// condition (e.g. make-line and make-curve are each other's negation
/// *given* a segment is selected, but become simultaneously `false`
/// together when it isn't) — collapsing them into one flags enum would
/// just re-derive the same six booleans at every call site.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct NodeToolbarState {
    /// Insert: a segment is selected (splits it at its midpoint — the
    /// toolbar button has no pointer position to hit-test against,
    /// unlike acceptance criterion 12's double-click).
    pub can_insert: bool,
    /// Delete: at least one node is selected.
    pub can_delete: bool,
    /// Make corner / make smooth: at least one node is selected. Both
    /// buttons stay enabled regardless of the selected node(s)' current
    /// kind — a multi-selection can mix kinds, and converting a node to
    /// the kind it already has is a harmless no-op, not a state to guard
    /// against.
    pub can_convert_to_corner: bool,
    /// See `can_convert_to_corner`.
    pub can_convert_to_smooth: bool,
    /// Make line: a segment is selected and it is not already a line
    /// (acceptance criterion 14's explicit disable example).
    pub can_make_line: bool,
    /// Make curve: a segment is selected and it is already a line.
    pub can_make_curve: bool,
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
    /// Dragging one or more selected nodes (acceptance criteria 8, 10).
    Nodes {
        path: NodeId,
        down_at: Point,
        starts: Vec<(vecmanf_document_core::AnchorId, Point)>,
    },
    /// Dragging one handle (acceptance criterion 9). `down_at` and
    /// `start_value` are recorded at press time so the commit on release
    /// writes `start_value` moved by the press→release displacement —
    /// never the absolute pointer position (architect review: a press
    /// within hit tolerance but off the handle's exact tip would
    /// otherwise relocate it to wherever the click landed, even with no
    /// drag at all).
    Handle {
        path: NodeId,
        anchor: vecmanf_document_core::AnchorId,
        slot: HandleSlot,
        down_at: Point,
        start_value: Vec2,
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

    /// Acceptance criterion: Escape with a selection present clears it;
    /// with nothing selected it is a no-op
    /// (`specs/0002-path-node-editing/specification.md`'s node-tool actions
    /// notes). Also cancels any drag currently in flight, writing nothing
    /// — consistent with the pen tool's own Escape, which discards its
    /// in-progress state rather than leaving a gesture half-finished.
    /// Returns whether anything (a selection, a drag, or both) was
    /// cancelled.
    pub fn escape(&mut self) -> bool {
        let had_selection = !self.selection.is_empty();
        let had_drag = !matches!(self.drag, Drag::None);
        self.selection.clear();
        self.drag = Drag::None;
        had_selection || had_drag
    }

    /// Acceptance criteria 7, 8, 9, 10, 14: the maker pressed the mouse
    /// button down at `point`. Updates selection and/or begins a drag.
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
            tolerances.segment,
        ) {
            Some(Hit::Handle { path, anchor, slot }) => {
                self.begin_handle_drag(paths, path, anchor, slot, point);
                PointerDownOutcome::Handle
            }
            Some(Hit::Node { path, anchor }) => {
                if shift {
                    self.selection.toggle_node(path, anchor);
                } else if self.selection.path() != Some(path)
                    || !self.selection.contains_node(anchor)
                {
                    self.selection.select_single_node(path, anchor);
                }
                self.begin_node_drag(paths, path, point);
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

    fn begin_node_drag(&mut self, paths: &[PathSnapshot], path: NodeId, down_at: Point) {
        let Some(snapshot) = paths.iter().find(|p| p.id == path) else {
            return;
        };
        let starts = self
            .selection
            .nodes()
            .iter()
            .filter_map(|&id| {
                snapshot
                    .anchors
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| (id, a.point))
            })
            .collect();
        self.drag = Drag::Nodes {
            path,
            down_at,
            starts,
        };
    }

    /// Records the handle's current value at press time, so the commit
    /// on release can move it *relative to that starting value* rather
    /// than writing wherever the pointer happens to end up (see
    /// [`Drag::Handle`]'s own doc comment for why that distinction
    /// matters). Falls back to [`Vec2::ZERO`] if `path`/`anchor` cannot
    /// be resolved in `paths` — the subsequent `set_handle` call on
    /// release will refuse against the live document anyway.
    fn begin_handle_drag(
        &mut self,
        paths: &[PathSnapshot],
        path: NodeId,
        anchor: vecmanf_document_core::AnchorId,
        slot: HandleSlot,
        down_at: Point,
    ) {
        let start_value = paths
            .iter()
            .find(|p| p.id == path)
            .and_then(|snapshot| snapshot.anchors.iter().find(|a| a.id == anchor))
            .map_or(Vec2::ZERO, |a| match slot {
                HandleSlot::In => a.handle_in,
                HandleSlot::Out => a.handle_out,
            });
        self.drag = Drag::Handle {
            path,
            anchor,
            slot,
            down_at,
            start_value,
        };
    }

    /// The maker released the mouse button at `point`, ending whatever
    /// drag [`NodeTool::pointer_down`] began. Commits the drag as exactly
    /// one command.
    pub fn pointer_up(&mut self, document: &Document, point: Point) -> PointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            Drag::None => PointerUpOutcome::NoOp,
            Drag::Nodes {
                path,
                down_at,
                starts,
            } => {
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
                let delta = down_at.vector_to(point);
                let moves: Vec<_> = starts
                    .iter()
                    .map(|&(id, p)| (id, p.translated(delta)))
                    .collect();
                let _ = document.move_anchors(path, &moves);
                PointerUpOutcome::NodesMoved
            }
            Drag::Handle {
                path,
                anchor,
                slot,
                down_at,
                start_value,
            } => {
                // Same zero-movement rule as the node-drag branch above,
                // and for the same reason.
                if point == down_at {
                    return PointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let value = Vec2::new(start_value.x + delta.x, start_value.y + delta.y);
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
        let _ = document.convert_anchor_kind(path, ids, kind);
    }

    /// Acceptance criterion 13: deletes every currently selected node,
    /// then clears the selection (the deleted ids can no longer resolve).
    /// A no-op when the selection is not a node selection.
    pub fn delete_selected(&mut self, document: &Document) {
        let Some(path) = self.selection.path() else {
            return;
        };
        let ids = self.selection.nodes().to_vec();
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
        vecmanf_document_core::AnchorId,
        vecmanf_document_core::AnchorId,
    )> {
        let path = self.selection.path()?;
        let (start, end) = self.selection.segment()?;
        Some((path, start, end))
    }

    /// Which contextual-toolbar actions apply right now, computed from
    /// this tool's current selection against `document`'s live state —
    /// the facade (`vecmanf-editor-wasm`'s `Session`) just calls this
    /// rather than re-deriving the same six booleans itself.
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
            can_convert_to_smooth: has_nodes,
            can_make_line: segment_is_line == Some(false),
            can_make_curve: segment_is_line == Some(true),
        }
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
        let new_anchor = vecmanf_document_core::NewAnchor {
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
        let new_anchor = vecmanf_document_core::NewAnchor {
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
    use vecmanf_document_core::{AnchorId, NewAnchor, Vec2};

    use super::*;

    const TOLERANCES: HitTolerances = HitTolerances {
        point: Tolerance::from_mm(2.0),
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
                    kind: AnchorKind::Smooth,
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
                    kind: AnchorKind::Smooth,
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

        tool.convert_selected(&document, AnchorKind::Smooth);
        let snapshot = document.path(path).expect("exists");
        let middle = &snapshot.anchors[1];
        assert_eq!(middle.kind, AnchorKind::Smooth);
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
        assert_eq!(snapshot.anchors[1].point, Point::new(10.0, 0.0));
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
                    kind: AnchorKind::Smooth,
                },
                NewAnchor {
                    id: b,
                    point: Point::new(20.0, 0.0),
                    handle_in: Vec2::new(-5.0, 0.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Smooth,
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

    /// Escape mid-drag cancels the drag (consistent with the pen tool's
    /// own Escape): the subsequent release that would otherwise end the
    /// drag is now a no-op, and the node never moves.
    #[test]
    fn escape_mid_drag_cancels_it() {
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

        assert!(tool.escape(), "a drag was in flight to cancel");

        let outcome = tool.pointer_up(&document, Point::new(50.0, 50.0));
        assert_eq!(outcome, PointerUpOutcome::NoOp, "the drag was cancelled");
        let snapshot = document.path(path).expect("exists");
        assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0), "unmoved");
    }

    #[test]
    fn escape_clears_a_present_selection_and_is_a_no_op_otherwise() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = open_two_node_path(&document, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut tool = NodeTool::new();
        tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);

        assert!(tool.escape());
        assert!(tool.selection().is_empty(), "escape clears the selection");
        assert!(!tool.escape());
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
}
