//! The node tool's state machine (acceptance criteria 7-14).
//!
//! Selection ([`NodeSelection`]) persists across calls; a drag's
//! in-flight geometry is ephemeral (ADR 0009 §2) and collapses into one
//! [`vecmanf_document_core::Document::move_anchors`] or
//! `set_handle` commit on release.

use vecmanf_document_core::{
    AnchorKind, Document, HandleSlot, NodeId, PathSnapshot, Point, Tolerance,
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
    /// Dragging one handle (acceptance criterion 9).
    Handle {
        path: NodeId,
        anchor: vecmanf_document_core::AnchorId,
        slot: HandleSlot,
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

    /// Acceptance criterion: Escape with a selection present clears it;
    /// with nothing selected it is a no-op
    /// (`specs/path-node-editing/specification.md`'s node-tool actions
    /// notes). Returns whether anything was cleared.
    pub fn escape(&mut self) -> bool {
        let had_selection = !self.selection.is_empty();
        self.selection.clear();
        had_selection
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
                self.drag = Drag::Handle { path, anchor, slot };
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
                let delta = down_at.vector_to(point);
                let moves: Vec<_> = starts
                    .iter()
                    .map(|&(id, p)| (id, p.translated(delta)))
                    .collect();
                let _ = document.move_anchors(path, &moves);
                PointerUpOutcome::NodesMoved
            }
            Drag::Handle { path, anchor, slot } => {
                if let Some(a) = document
                    .path(path)
                    .and_then(|snapshot| snapshot.anchors.into_iter().find(|a| a.id == anchor))
                {
                    let value = a.point.vector_to(point);
                    let _ = document.set_handle(path, anchor, slot, value);
                }
                PointerUpOutcome::HandleMoved
            }
        }
    }

    /// Acceptance criterion 11: converts every currently selected node to
    /// `kind`. A no-op when the selection is not a node selection.
    pub fn convert_selected(&self, document: &Document, kind: AnchorKind) {
        let Some(path) = self.selection.path() else {
            return;
        };
        for &anchor in self.selection.nodes() {
            let _ = document.convert_anchor_kind(path, anchor, kind);
        }
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

    /// Acceptance criterion 12: double-clicking a point on a segment
    /// (not on an existing node) inserts a new corner node there,
    /// splitting the segment with the path's visible shape unchanged at
    /// the instant of insertion. A no-op (returns `None`) when `point`
    /// does not land on a segment.
    ///
    /// Double-click detection itself is the frontend's job
    /// (`specs/path-node-editing/adrs.md`'s `PenTool` doc comment makes
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
        assert!(tool.selection().is_empty());
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
        assert!(tool.selection().is_empty());
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
        assert!(!tool.selection().is_empty());

        let outcome = tool.pointer_down(&paths, Point::new(1000.0, 1000.0), TOLERANCES, false);
        assert_eq!(outcome, PointerDownOutcome::Missed);
        assert!(tool.selection().is_empty());
    }
}
