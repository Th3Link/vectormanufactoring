//! The pen tool's state machine (acceptance criteria 1-5; `specs/0034-pen-path-extension`).
//!
//! `specs/0002-path-node-editing/adrs.md`, "a pen session is one commit": from
//! the first click to the double-click, close-path or Escape, the
//! in-progress path lives *only* as this type's own ephemeral state
//! (ADR 0009 §2) — [`curvyo_document_core::Document`] gains exactly one
//! commit, at [`PenTool::finish`] or a closing, joining
//! [`PenTool::pointer_up`]; [`PenTool::escape`] drops this state and
//! writes nothing.
//!
//! A press has a [`PressAction`] decided at the press: place a node, continue an open path from
//! one of its ends, join the path in progress to another path's end, or close it with a join
//! type. The path in progress is kept in drawing direction; a continuation from a path's first
//! node is put in stored order only at commit.
//!
//! Double-click detection itself is the frontend's job (a real DOM
//! `dblclick` event, or platform equivalent) — this crate has no clock
//! (`CLAUDE.md` §6) and cannot time one itself — so [`PenTool::finish`]
//! is a direct action the caller invokes, not something this state
//! machine infers from two `pointer_up` calls.

use curvyo_document_core::{
    AnchorId, AnchorKind, AnchorSnapshot, Document, Length, NewAnchor, NodeId, PathEnd, PathGrowth,
    Point, reversed_anchors,
};

use crate::AnchorIdMinter;
use crate::closing_join::JoinType;
use crate::pen_target::EndNode;

mod commit;

/// One node placed in the current pen session, in the shape
/// [`Document::create_path`] needs.
type PlacedAnchor = NewAnchor;

/// A path being continued: the path read at the press that started it. It stays in the document,
/// untouched, until the maker finishes.
#[derive(Debug, Clone, PartialEq)]
pub struct Continuation {
    /// The continued path.
    pub path: NodeId,
    /// The end the maker pressed: new nodes attach there.
    pub end: PathEnd,
    /// The path's anchors as they were at the press, in stored order.
    pub original: Vec<AnchorSnapshot>,
}

/// What a press does, decided at the press and run at the release (a drag is ignored for every
/// action but [`PressAction::Place`]).
#[derive(Debug, Clone, PartialEq)]
pub enum PressAction {
    /// Place a node (or start a new path).
    Place,
    /// Continue an open path from one of its end nodes.
    Continue(Continuation),
    /// Join the path in progress to the end of another open path.
    Join(EndNode),
    /// Close the path in progress onto its closing node with this join type.
    Close(JoinType),
}

/// The pen tool's state (idle, or placing a path).
#[derive(Debug, Default)]
pub struct PenTool {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    Placing {
        /// The nodes in drawing direction. When continuing, the first is the end node pressed.
        nodes: Vec<PlacedAnchor>,
        down: Option<PointerDown>,
        /// The path being continued, if any.
        base: Option<Continuation>,
    },
}

#[derive(Debug, Clone)]
struct PointerDown {
    point: Point,
    /// Decided at `pointer_down` time (a close press that wanders away from its target before the
    /// release still closes): the press-and-release is one gesture on one target.
    action: PressAction,
}

/// What [`PenTool::pointer_up`] actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerUpOutcome {
    /// No node was placed because no matching `pointer_down` was pending
    /// (e.g. the pen tool was idle, or `escape`/`finish` already consumed
    /// it), or a commit was refused and the state dropped.
    Ignored,
    /// A node was placed; the path is still being drawn.
    Placed,
    /// The press landed on an end node of an open path: that path is being continued.
    Continued,
    /// Acceptance criterion 5: the press landed on the closing node, so the path closed and
    /// committed as this [`NodeId`] (a new path object, or the continued one).
    Closed(NodeId),
    /// The press landed on another path's end: the two became one path, this [`NodeId`].
    Joined(NodeId),
}

/// The closing segment as it will be committed, in stored orientation: from the last stored node
/// to the first, with the closing node resolved by the join (`0034` criterion 17).
#[derive(Debug, Clone, PartialEq)]
pub struct ClosingPreview {
    /// The node the closing segment leaves (its outgoing handle faces the segment).
    pub from: AnchorSnapshot,
    /// The node the closing segment arrives at (its incoming handle faces the segment).
    pub to: AnchorSnapshot,
    /// The closing node with the join applied: `from` or `to` as drawn above.
    pub closing_node: AnchorSnapshot,
}

impl PenTool {
    /// A tool with no path in progress.
    #[must_use]
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    /// Whether a path is currently being drawn.
    #[must_use]
    pub const fn is_placing(&self) -> bool {
        matches!(self.state, State::Placing { .. })
    }

    /// The path being continued, if the pen is continuing one.
    #[must_use]
    pub fn continuation(&self) -> Option<&Continuation> {
        match &self.state {
            State::Placing { base, .. } => base.as_ref(),
            State::Idle => None,
        }
    }

    /// The in-progress path's placed nodes, for a renderer's preview —
    /// `None` when idle. Never the document's own data: this state is
    /// ephemeral by design (ADR 0009 §2). When continuing, the first node is the end node the
    /// maker pressed, with its handles in drawing direction.
    #[must_use]
    pub fn in_progress_nodes(&self) -> Option<&[PlacedAnchor]> {
        match &self.state {
            State::Idle => None,
            State::Placing { nodes, .. } => Some(nodes),
        }
    }

    /// What [`PenTool::pointer_up`] would commit right now if the maker
    /// released at `cursor` — for the renderer's live drag preview
    /// (acceptance criterion 2: "show both symmetric handle lines/
    /// endpoints growing from C in real time, and reshape the B→C
    /// segment live as a curve"). `None` when idle, between gestures (the
    /// mouse is up), or the pending press is not a node placement (a close, continue or join
    /// press commits no new node to preview).
    ///
    /// Built by `PenTool::resolve_anchor` (private: this crate's own
    /// internal helper, not part of its public surface) — the exact same
    /// resolution rule [`PenTool::pointer_up`] itself calls — so the preview and the
    /// eventual commit are structurally guaranteed to agree (`specs/0002-
    /// path-node-editing/adrs.md`: "commands carry resolved geometry,
    /// never geometric intent", applied here to the preview too).
    ///
    /// `id` should be [`crate::AnchorIdMinter::peek`]'s result, not a
    /// minted one: a preview that advanced the minter's own counter would
    /// desync it from what the gesture might still turn into (Escape
    /// discards it, a press turning out to be a close-path gesture mints
    /// nothing) — the same reason [`PenTool::pointer_up`] only mints once
    /// it has committed to actually placing a node.
    #[must_use]
    pub fn pending_anchor(
        &self,
        id: AnchorId,
        cursor: Point,
        drag_threshold: Length,
    ) -> Option<NewAnchor> {
        let State::Placing { down, .. } = &self.state else {
            return None;
        };
        let down = down.as_ref()?;
        if down.action != PressAction::Place {
            return None;
        }
        Some(Self::resolve_anchor(id, down.point, cursor, drag_threshold))
    }

    /// The one rule for what anchor a press/release pair resolves to —
    /// acceptance criteria 1 and 2's shared decision, shared verbatim
    /// between the actual commit ([`PenTool::pointer_up`]) and the live
    /// preview ([`PenTool::pending_anchor`]) so the two can never
    /// independently drift apart. `down`/`release` are document-space
    /// points; the new node always sits at `down` ("a new node is added
    /// at C", acceptance criterion 2, where C is where the press started,
    /// not where it ended) with a drag-derived handle pair when the
    /// press→release distance exceeds `drag_threshold`, or a plain corner
    /// node (no handles) otherwise.
    fn resolve_anchor(
        id: AnchorId,
        down: Point,
        release: Point,
        drag_threshold: Length,
    ) -> NewAnchor {
        let drag = down.vector_to(release);
        if drag.length() > drag_threshold.as_mm() {
            NewAnchor {
                id,
                point: down,
                handle_in: drag.negated(),
                handle_out: drag,
                kind: AnchorKind::Symmetric,
            }
        } else {
            NewAnchor::corner(id, down)
        }
    }

    /// Acceptance criteria 1, 2, 5: the maker pressed the mouse button
    /// down at `point`. Starts a new path if none is in progress.
    ///
    /// `close_tolerance` is how close `point` must be to the in-progress
    /// path's first node, with at least three nodes already placed (so
    /// closing leaves a path of at least three nodes, `0034` criterion 13),
    /// for this press to be a close-path gesture, closed as drawn, rather than a new node.
    /// The pen's full set of targets (continue, join, a chosen join type) goes through
    /// [`PenTool::pointer_down_action`].
    pub fn pointer_down(&mut self, point: Point, close_tolerance: Length) {
        let action = match &self.state {
            State::Placing {
                nodes, base: None, ..
            } if Self::is_close_target(nodes, point, close_tolerance) => {
                PressAction::Close(JoinType::as_drawn(nodes[0].kind))
            }
            _ => PressAction::Place,
        };
        self.pointer_down_action(point, action);
    }

    /// A press at `point` whose action the caller decided (`crate::pen_target`).
    pub fn pointer_down_action(&mut self, point: Point, action: PressAction) {
        match &mut self.state {
            State::Idle => {
                self.state = State::Placing {
                    nodes: Vec::new(),
                    down: Some(PointerDown { point, action }),
                    base: None,
                };
            }
            State::Placing { down, .. } => {
                *down = Some(PointerDown { point, action });
            }
        }
    }

    /// Whether `point` is close enough to the in-progress new path's own first node, with enough
    /// nodes already placed, to read as a close-path press (acceptance criterion 5).
    fn is_close_target(nodes: &[PlacedAnchor], point: Point, close_tolerance: Length) -> bool {
        nodes.len() >= 3
            && nodes.first().is_some_and(|first| {
                first.point.vector_to(point).length() <= close_tolerance.as_mm()
            })
    }

    /// The node the in-progress path would close onto, with its point and kind: the first node of
    /// a new path (three or more placed), or the other end of a continued path (three or more
    /// nodes once closed). `None` otherwise (`0034` criterion 13).
    #[must_use]
    pub fn closing_node(&self) -> Option<AnchorSnapshot> {
        let State::Placing { nodes, base, .. } = &self.state else {
            return None;
        };
        match base {
            None => (nodes.len() >= 3).then(|| nodes[0]),
            Some(continuation) => {
                let total = continuation.original.len() + nodes.len() - 1;
                (total >= 3).then(|| match continuation.end {
                    PathEnd::Last => continuation.original[0],
                    PathEnd::First => continuation.original[continuation.original.len() - 1],
                })
            }
        }
    }

    /// Acceptance criteria 1, 2, 5: the maker released the mouse button at
    /// `point`, ending the gesture [`PenTool::pointer_down`] started.
    ///
    /// Whether this is a plain click (acceptance criterion 1's corner
    /// node) or a click-drag (acceptance criterion 2's smooth node) is
    /// decided here, from the straight-line distance between the
    /// `pointer_down` point and `point`: at or under `drag_threshold` is a
    /// click, over it is a drag. The new node's own position is always
    /// the `pointer_down` point ("a new node is added at C", acceptance
    /// criterion 2, where C is where the press started, not where it
    /// ended); a drag's handle vector is the displacement from press to
    /// release. A continue, join or close press ignores the release point.
    pub fn pointer_up(
        &mut self,
        minter: &mut AnchorIdMinter,
        document: &Document,
        point: Point,
        drag_threshold: Length,
    ) -> PointerUpOutcome {
        let State::Placing { down, .. } = &mut self.state else {
            return PointerUpOutcome::Ignored;
        };
        let Some(down) = down.take() else {
            return PointerUpOutcome::Ignored;
        };
        match down.action {
            PressAction::Place => {
                let id = minter.mint();
                let anchor = Self::resolve_anchor(id, down.point, point, drag_threshold);
                if let State::Placing { nodes, .. } = &mut self.state {
                    nodes.push(anchor);
                }
                PointerUpOutcome::Placed
            }
            PressAction::Continue(continuation) => {
                let end_node = match continuation.end {
                    PathEnd::Last => continuation.original[continuation.original.len() - 1],
                    PathEnd::First => reversed_anchors(&continuation.original[..1])[0],
                };
                self.state = State::Placing {
                    nodes: vec![end_node],
                    down: None,
                    base: Some(continuation),
                };
                PointerUpOutcome::Continued
            }
            PressAction::Join(target) => self.commit_join(document, &target),
            PressAction::Close(join) => self.commit_close(document, join),
        }
    }

    /// Acceptance criterion 3: commits the in-progress path as an open
    /// path object, exactly as drawn, and returns to idle ready to start
    /// a new, separate path. A continued path is extended and keeps its object id
    /// (`0034` criterion 5). A no-op (returns `None`, changes nothing)
    /// when there is no path in progress or it has fewer than two nodes —
    /// "at least one segment" is this criterion's own precondition, and a continuation
    /// with no node added writes nothing.
    pub fn finish(&mut self, document: &Document) -> Option<NodeId> {
        let State::Placing { nodes, base, .. } = &self.state else {
            return None;
        };
        if nodes.len() < 2 {
            return None;
        }
        let path_id = match base {
            None => document.create_path(nodes, false),
            Some(continuation) => {
                let added = match continuation.end {
                    PathEnd::Last => nodes[1..].to_vec(),
                    PathEnd::First => reversed_anchors(&nodes[1..]),
                };
                let growth = PathGrowth {
                    path: continuation.path,
                    end: continuation.end,
                    added,
                    absorb: None,
                    replace: Vec::new(),
                    drop: Vec::new(),
                };
                let path = continuation.path;
                let committed = document.extend_path(&growth).is_ok();
                self.state = State::Idle;
                return committed.then_some(path);
            }
        };
        self.state = State::Idle;
        Some(path_id)
    }

    /// Acceptance criterion 4: discards the entire in-progress path, including a continuation's
    /// new nodes (the continued path is untouched, `0034` criterion 25), writing nothing — not an
    /// undo (`specs/0002-path-node-editing/adrs.md`: "AC4's Escape is not an undo and must not be
    /// built as one"). Returns whether there was anything to discard.
    pub fn escape(&mut self) -> bool {
        let was_placing = self.is_placing();
        self.state = State::Idle;
        was_placing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minter() -> AnchorIdMinter {
        AnchorIdMinter::new(1)
    }

    const CLOSE_TOLERANCE: Length = Length::from_mm(2.0);
    const DRAG_THRESHOLD: Length = Length::from_mm(1.0);

    /// AC1: a plain click at A then a plain click at B draws a two-node
    /// open path with a straight segment, both corner nodes.
    #[test]
    fn ac1_two_plain_clicks_make_a_two_node_open_path() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        let outcome = pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        assert_eq!(outcome, PointerUpOutcome::Placed);

        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        let outcome = pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );
        assert_eq!(outcome, PointerUpOutcome::Placed);

        let nodes = pen.in_progress_nodes().expect("still placing");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].point, Point::new(0.0, 0.0));
        assert_eq!(nodes[1].point, Point::new(10.0, 0.0));
        assert_eq!(nodes[0].kind, AnchorKind::Corner);
        assert_eq!(nodes[1].kind, AnchorKind::Corner);
        assert_eq!(nodes[0].handle_out, curvyo_document_core::Vec2::ZERO);
    }

    /// AC2: pressing down at C, dragging, then releasing adds a smooth
    /// node at C with symmetric handles along the drag direction.
    #[test]
    fn ac2_a_click_drag_makes_a_smooth_node_with_symmetric_handles() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);

        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        let outcome = pen.pointer_up(
            &mut minter,
            &document,
            Point::new(13.0, 4.0),
            DRAG_THRESHOLD,
        );
        assert_eq!(outcome, PointerUpOutcome::Placed);

        let nodes = pen.in_progress_nodes().expect("still placing");
        let c = &nodes[1];
        assert_eq!(
            c.point,
            Point::new(10.0, 0.0),
            "the node sits at C, not the release point"
        );
        assert_eq!(c.kind, AnchorKind::Symmetric);
        assert_eq!(c.handle_out, curvyo_document_core::Vec2::new(3.0, 4.0));
        assert_eq!(c.handle_in, c.handle_out.negated());
    }

    /// AC3: finishing with at least one segment commits one open path and
    /// resets the tool to start a new, separate path next.
    #[test]
    fn ac3_finish_commits_one_open_path_and_resets() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );

        let id = pen.finish(&document).expect("one segment exists");
        assert!(!pen.is_placing());

        let snapshot = document.path(id).expect("committed");
        assert!(!snapshot.closed);
        assert_eq!(snapshot.anchors.len(), 2);

        // Ready for a new, separate path.
        pen.pointer_down(Point::new(50.0, 50.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(50.0, 50.0),
            DRAG_THRESHOLD,
        );
        assert_eq!(pen.in_progress_nodes().expect("new path").len(), 1);
        assert_eq!(
            document.object_ids().len(),
            1,
            "the new click did not extend the finished path"
        );
    }

    /// AC3's own precondition: finishing with fewer than one segment
    /// (zero or one node) is a no-op.
    #[test]
    fn finish_with_no_segment_is_a_no_op() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);

        assert_eq!(pen.finish(&document), None);
        assert!(
            pen.is_placing(),
            "the single node is still there to continue from"
        );
        assert_eq!(document.object_ids(), Vec::new());
    }

    /// AC4: Escape discards everything and writes nothing.
    #[test]
    fn ac4_escape_discards_without_writing_anything() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );

        assert!(pen.escape());
        assert!(!pen.is_placing());
        assert_eq!(document.object_ids(), Vec::new());

        // Escape with nothing in progress changes nothing and says so.
        assert!(!pen.escape());
    }

    /// AC5: clicking the in-progress path's own first node, with three or
    /// more nodes, closes and commits — a closed path, drawing ends.
    #[test]
    fn ac5_clicking_the_first_node_closes_and_commits() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );
        pen.pointer_down(Point::new(5.0, 10.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(5.0, 10.0),
            DRAG_THRESHOLD,
        );

        // Press back on (near) the first node.
        pen.pointer_down(Point::new(0.1, 0.1), CLOSE_TOLERANCE);
        let outcome = pen.pointer_up(&mut minter, &document, Point::new(0.1, 0.1), DRAG_THRESHOLD);

        let PointerUpOutcome::Closed(id) = outcome else {
            panic!("expected Closed, got {outcome:?}");
        };
        assert!(!pen.is_placing());
        let snapshot = document.path(id).expect("committed");
        assert!(snapshot.closed);
        assert_eq!(
            snapshot.anchors.len(),
            3,
            "the close click does not add a fourth node"
        );
    }

    /// AC5's own precondition: fewer than three nodes placed, clicking
    /// near the first node does not close (not enough nodes for a valid
    /// closed path) — it is treated as placing a second node there
    /// instead.
    #[test]
    fn closing_needs_at_least_three_nodes() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);

        pen.pointer_down(Point::new(0.1, 0.1), CLOSE_TOLERANCE);
        let outcome = pen.pointer_up(&mut minter, &document, Point::new(0.1, 0.1), DRAG_THRESHOLD);
        assert_eq!(outcome, PointerUpOutcome::Placed);
        assert_eq!(pen.in_progress_nodes().expect("still placing").len(), 2);
    }

    #[test]
    fn pointer_up_without_a_pending_down_is_ignored() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        let outcome = pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        assert_eq!(outcome, PointerUpOutcome::Ignored);
    }

    /// Acceptance criterion 2's live drag preview needs to know a press
    /// is currently held down and what it would resolve to — idle
    /// (nothing pressed), and a press that has already released (consumed
    /// by `pointer_up`), both report `None`.
    #[test]
    fn pending_anchor_is_none_when_idle_or_between_gestures() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        let id = AnchorId::new(1, 0);
        assert_eq!(
            pen.pending_anchor(id, Point::new(0.0, 0.0), DRAG_THRESHOLD),
            None,
            "idle"
        );

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        assert_eq!(
            pen.pending_anchor(id, Point::new(0.0, 0.0), DRAG_THRESHOLD),
            None,
            "released: the drag already committed"
        );
    }

    /// While a press is held down (not yet released) and the live cursor
    /// has moved past `drag_threshold`, `pending_anchor` previews exactly
    /// what `pointer_up` would commit at that same cursor position: a
    /// smooth node at C (the press position, not the cursor) with
    /// symmetric handles along the drag.
    #[test]
    fn pending_anchor_matches_what_pointer_up_would_commit_past_the_threshold() {
        let mut pen = PenTool::new();
        let id = AnchorId::new(1, 0);
        // Even the very first press of a brand-new path previews its own
        // anchor: dragging while placing the first node pulls out that
        // node's own handles, with no B→C segment yet (nothing precedes
        // it) — the renderer is the one that decides there is no segment
        // to draw, not this method.
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        assert_eq!(
            pen.pending_anchor(id, Point::new(3.0, 4.0), DRAG_THRESHOLD),
            Some(NewAnchor {
                id,
                point: Point::new(0.0, 0.0),
                handle_in: curvyo_document_core::Vec2::new(-3.0, -4.0),
                handle_out: curvyo_document_core::Vec2::new(3.0, 4.0),
                kind: AnchorKind::Symmetric,
            })
        );

        let mut minter = minter();
        let document = Document::new(1);
        pen.pointer_up(&mut minter, &document, Point::new(3.0, 4.0), DRAG_THRESHOLD);

        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        assert_eq!(
            pen.pending_anchor(id, Point::new(13.0, 4.0), DRAG_THRESHOLD),
            Some(NewAnchor {
                id,
                point: Point::new(10.0, 0.0),
                handle_in: curvyo_document_core::Vec2::new(-3.0, -4.0),
                handle_out: curvyo_document_core::Vec2::new(3.0, 4.0),
                kind: AnchorKind::Symmetric,
            }),
            "held at C, the press position — not wherever the cursor ended up"
        );
    }

    /// The bug this test guards against: a drag just under
    /// `drag_threshold` must preview the exact same corner node (no
    /// handles) that `pointer_up` would actually commit at that cursor
    /// position — not a smooth node with handles that then disagrees with
    /// the commit on release. Before `pending_anchor` shared
    /// `resolve_anchor` with `pointer_up`, `curvyo-render-core`'s own
    /// re-implementation of this rule ignored `drag_threshold` entirely
    /// and always previewed a smooth node.
    #[test]
    fn pending_anchor_under_the_threshold_previews_a_corner_node_with_no_handles() {
        let mut pen = PenTool::new();
        let id = AnchorId::new(1, 0);
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);

        // A tiny, sub-threshold movement — DRAG_THRESHOLD is 1.0mm here.
        let cursor = Point::new(0.3, 0.0);
        assert_eq!(
            pen.pending_anchor(id, cursor, DRAG_THRESHOLD),
            Some(NewAnchor::corner(id, Point::new(0.0, 0.0))),
            "must match pointer_up's own corner-node resolution for the same sub-threshold \
             release point, not a smooth node with handles"
        );

        // Confirm it actually does match what pointer_up commits.
        let mut minter = minter();
        let document = Document::new(1);
        pen.pointer_up(&mut minter, &document, cursor, DRAG_THRESHOLD);
        let committed = pen.in_progress_nodes().expect("still placing")[0];
        assert_eq!(committed.kind, AnchorKind::Corner);
        assert_eq!(committed.handle_in, curvyo_document_core::Vec2::ZERO);
        assert_eq!(committed.handle_out, curvyo_document_core::Vec2::ZERO);
    }

    /// A press held down over the close target (acceptance criterion 5)
    /// is a plain-click gesture, not a drag-to-curve one — no live curve
    /// preview applies to it.
    #[test]
    fn pending_anchor_is_none_while_closing() {
        let mut minter = minter();
        let document = Document::new(1);
        let mut pen = PenTool::new();
        let id = AnchorId::new(1, 0);
        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );
        pen.pointer_down(Point::new(5.0, 10.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(5.0, 10.0),
            DRAG_THRESHOLD,
        );

        // Press back down near the first node: a close-path gesture.
        pen.pointer_down(Point::new(0.1, 0.1), CLOSE_TOLERANCE);
        assert_eq!(
            pen.pending_anchor(id, Point::new(0.1, 0.1), DRAG_THRESHOLD),
            None,
            "closing, not dragging"
        );
    }

    // ---- `0034-pen-path-extension` -------------------------------------------------------

    use crate::pen_target::{EndNodeIndex, PenTarget, continuation_of, pen_target};
    use curvyo_document_core::{Color, PathSnapshot, StyleEdit, Vec2};

    const HIT: Length = Length::from_mm(2.0);

    fn paths(document: &Document) -> Vec<PathSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.path(id))
            .collect()
    }

    /// One pen gesture at `at` as the session runs it: decide the target, press, release.
    fn gesture(
        pen: &mut PenTool,
        minter: &mut AnchorIdMinter,
        document: &Document,
        at: Point,
        shift: bool,
    ) -> PointerUpOutcome {
        let all = paths(document);
        let index = EndNodeIndex::from_paths(&all);
        let action = match pen_target(pen, &index, at, HIT, shift) {
            PenTarget::Continue(end) => {
                let path = all.iter().find(|p| p.id == end.path).unwrap();
                PressAction::Continue(continuation_of(&end, path))
            }
            PenTarget::Join(end) => PressAction::Join(end),
            PenTarget::Close { join, .. } => PressAction::Close(join),
            PenTarget::Place | PenTarget::NewPathAt | PenTarget::PlaceOverEnd => PressAction::Place,
        };
        pen.pointer_down_action(at, action);
        pen.pointer_up(minter, document, at, DRAG_THRESHOLD)
    }

    fn pts(document: &Document, path: NodeId) -> Vec<(f64, f64)> {
        document
            .path(path)
            .unwrap()
            .anchors
            .iter()
            .map(|a| (a.point.x, a.point.y))
            .collect()
    }

    fn line(document: &Document, base: u64, points: &[(f64, f64)]) -> NodeId {
        let anchors: Vec<NewAnchor> = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                NewAnchor::corner(AnchorId::new(1, base + i as u64), Point::new(x, y))
            })
            .collect();
        document.create_path(&anchors, false)
    }

    /// `0034` criteria 2 to 5: pressing the last node continues the path, new nodes append, the
    /// path is untouched until the finish, and the finish is one `extend_path` that keeps the id.
    #[test]
    fn continuing_from_the_last_node_appends_and_keeps_the_object() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let path = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
        let mut pen = PenTool::new();
        let outcome = gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(20.0, 0.0),
            false,
        );
        assert_eq!(outcome, PointerUpOutcome::Continued);
        assert_eq!(
            pen.in_progress_nodes().unwrap().len(),
            1,
            "E is the first drawn node"
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(30.0, 0.0),
            false,
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(40.0, 0.0),
            false,
        );
        assert_eq!(
            pts(&document, path).len(),
            3,
            "the path is untouched until the finish"
        );
        assert_eq!(pen.finish(&document), Some(path));
        assert_eq!(
            pts(&document, path),
            vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (20.0, 0.0),
                (30.0, 0.0),
                (40.0, 0.0)
            ]
        );
        assert_eq!(document.object_ids().len(), 1);
    }

    /// Criterion 4: continuing from the first node places the new nodes before it.
    #[test]
    fn continuing_from_the_first_node_prepends_the_new_nodes_in_reverse_drawing_order() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let path = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(0.0, 0.0),
            false,
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(0.0, 10.0),
            false,
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(0.0, 20.0),
            false,
        );
        pen.finish(&document).unwrap();
        assert_eq!(
            pts(&document, path),
            vec![
                (0.0, 20.0),
                (0.0, 10.0),
                (0.0, 0.0),
                (10.0, 0.0),
                (20.0, 0.0)
            ]
        );
    }

    /// Criteria 1 and 25: Shift over an end starts a new path; Escape on a continuation discards
    /// the additions and leaves the path as it was.
    #[test]
    fn shift_starts_a_new_path_and_escape_leaves_the_continued_path_alone() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let path = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            true,
        );
        assert!(pen.continuation().is_none(), "Shift: a new path");
        assert!(pen.escape());
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            false,
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(30.0, 5.0),
            false,
        );
        assert!(pen.escape());
        assert_eq!(pts(&document, path), vec![(0.0, 0.0), (10.0, 0.0)]);
        assert_eq!(pen.finish(&document), None);
    }

    /// Criterion 5: finishing with no node added writes nothing.
    #[test]
    fn finishing_a_continuation_without_a_new_node_writes_nothing() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let path = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            false,
        );
        assert_eq!(pen.finish(&document), None);
        assert_eq!(pts(&document, path).len(), 2);
    }

    /// Criterion 8: P (0,0),(10,0) continued from its last node onto Q's first node (30,0) gives
    /// (0,0),(10,0),(30,0),(40,0); onto Q's last node (40,0): (0,0),(10,0),(40,0),(30,0). Q is gone
    /// and P's style survives (criterion 10).
    #[test]
    fn joining_follows_the_connect_rule_and_keeps_the_continued_style() {
        for (press, expected) in [
            (
                (30.0, 0.0),
                vec![(0.0, 0.0), (10.0, 0.0), (30.0, 0.0), (40.0, 0.0)],
            ),
            (
                (40.0, 0.0),
                vec![(0.0, 0.0), (10.0, 0.0), (40.0, 0.0), (30.0, 0.0)],
            ),
        ] {
            let document = Document::new(1);
            let mut minter = AnchorIdMinter::new(5);
            let p = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0)]);
            let q = line(&document, 10, &[(30.0, 0.0), (40.0, 0.0)]);
            document
                .edit_style(&[p], &StyleEdit::StrokeColor(Color { r: 200, g: 0, b: 0 }))
                .unwrap();
            let red = document.path(p).unwrap().style;
            let mut pen = PenTool::new();
            gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(10.0, 0.0),
                false,
            );
            let outcome = gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(press.0, press.1),
                false,
            );
            assert_eq!(outcome, PointerUpOutcome::Joined(p));
            assert_eq!(pts(&document, p), expected);
            assert!(document.path(q).is_none());
            assert_eq!(document.path(p).unwrap().style, red);
            assert!(!pen.is_placing());
        }
    }

    /// Criterion 10: a new path that ends on Q becomes Q (its object, style and place).
    #[test]
    fn a_new_path_ending_on_another_becomes_that_path() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let q = line(&document, 10, &[(30.0, 0.0), (40.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(0.0, 0.0),
            false,
        );
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            false,
        );
        let outcome = gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(30.0, 0.0),
            false,
        );
        assert_eq!(outcome, PointerUpOutcome::Joined(q));
        assert_eq!(document.object_ids().len(), 1);
        // Q starts at its first node, so the new nodes precede it in drawing order.
        assert_eq!(
            pts(&document, q),
            vec![(0.0, 0.0), (10.0, 0.0), (30.0, 0.0), (40.0, 0.0)]
        );
    }

    /// Criterion 9: ends within 0.001 mm merge into one node (midpoint, a Corner, each side's
    /// handle kept), with no zero-length segment.
    #[test]
    fn coincident_ends_merge_into_one_node() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let p = line(&document, 1, &[(0.0, 0.0), (10.0, 0.0)]);
        let q = line(&document, 10, &[(10.0004, 0.0), (20.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            false,
        );
        let outcome = gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(10.0004, 0.0),
            false,
        );
        assert_eq!(outcome, PointerUpOutcome::Joined(p));
        let points = pts(&document, p);
        assert_eq!(points.len(), 3, "the two ends became one node: {points:?}");
        assert!((points[1].0 - 10.0002).abs() < 1e-9);
        assert!(document.path(q).is_none());
    }

    /// Criteria 14, 15, 17: a path drawn with clicks closes as it does today with no Shift (four
    /// corners) and Smooth with Shift (the first node Asymmetric, collinear handles along the
    /// direction from D to B); the preview and the commit are the same result.
    #[test]
    fn closing_with_shift_makes_the_first_node_smooth_and_the_preview_is_the_commit() {
        for shift in [false, true] {
            let document = Document::new(1);
            let mut minter = AnchorIdMinter::new(5);
            let mut pen = PenTool::new();
            for at in [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)] {
                gesture(
                    &mut pen,
                    &mut minter,
                    &document,
                    Point::new(at.0, at.1),
                    false,
                );
            }
            let join = JoinType::resolve(AnchorKind::Corner, shift);
            let preview = pen.closing_preview(join).unwrap();
            let outcome = gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(0.0, 0.0),
                shift,
            );
            let PointerUpOutcome::Closed(id) = outcome else {
                panic!("{outcome:?}")
            };
            let snapshot = document.path(id).unwrap();
            assert!(snapshot.closed);
            assert_eq!(snapshot.anchors.len(), 4);
            assert_eq!(
                snapshot.anchors[0], preview.closing_node,
                "preview == commit"
            );
            if shift {
                let first = &snapshot.anchors[0];
                assert_eq!(first.kind, AnchorKind::Asymmetric);
                let along = Vec2::new(20.0, -20.0).normalized_to(1.0);
                assert!((first.handle_out.x - along.x * 10.0).abs() < 1e-9);
                assert!((first.handle_in.y + along.y * 10.0).abs() < 1e-9);
            } else {
                assert!(
                    snapshot
                        .anchors
                        .iter()
                        .all(|a| a.kind == AnchorKind::Corner)
                );
                assert!(snapshot.anchors.iter().all(|a| a.handle_in == Vec2::ZERO));
            }
        }
    }

    /// Criterion 15: a first node drawn with a drag (Symmetric) closes Smooth as drawn; with Shift
    /// it becomes a Corner whose closing-side (incoming) handle is retracted, so the two closing
    /// segments differ.
    #[test]
    fn sharp_on_a_dragged_first_node_retracts_the_closing_side_handle() {
        let mut results = Vec::new();
        for shift in [false, true] {
            let document = Document::new(1);
            let mut minter = AnchorIdMinter::new(5);
            let mut pen = PenTool::new();
            // First node drawn with a drag: handle (3, 4).
            pen.pointer_down(Point::new(0.0, 0.0), HIT);
            pen.pointer_up(&mut minter, &document, Point::new(3.0, 4.0), DRAG_THRESHOLD);
            for at in [(20.0, 0.0), (20.0, 20.0)] {
                gesture(
                    &mut pen,
                    &mut minter,
                    &document,
                    Point::new(at.0, at.1),
                    false,
                );
            }
            let outcome = gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(0.0, 0.0),
                shift,
            );
            let PointerUpOutcome::Closed(id) = outcome else {
                panic!("{outcome:?}")
            };
            results.push(document.path(id).unwrap().anchors[0]);
        }
        assert_eq!(results[0].kind, AnchorKind::Symmetric);
        assert_eq!(results[1].kind, AnchorKind::Corner);
        assert_eq!(
            results[1].handle_in,
            Vec2::ZERO,
            "retracted on the closing side"
        );
        assert_eq!(
            results[1].handle_out,
            Vec2::new(3.0, 4.0),
            "the other handle as drawn"
        );
        assert_ne!(results[0], results[1]);
    }

    /// Criterion 14: closing a continuation closes the same object, with the other end as the
    /// closing node, from either end.
    #[test]
    fn closing_a_continuation_closes_the_same_object() {
        for first in [false, true] {
            let document = Document::new(1);
            let mut minter = AnchorIdMinter::new(5);
            let p = line(&document, 1, &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)]);
            let mut pen = PenTool::new();
            // Continue from the last node (20, 20) or the first (0, 0), draw one node, close on
            // the other end.
            let (start, via, target) = if first {
                ((0.0, 0.0), (0.0, 20.0), (20.0, 20.0))
            } else {
                ((20.0, 20.0), (0.0, 20.0), (0.0, 0.0))
            };
            gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(start.0, start.1),
                false,
            );
            gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(via.0, via.1),
                false,
            );
            let outcome = gesture(
                &mut pen,
                &mut minter,
                &document,
                Point::new(target.0, target.1),
                false,
            );
            assert_eq!(outcome, PointerUpOutcome::Closed(p));
            let snapshot = document.path(p).unwrap();
            assert!(snapshot.closed);
            assert_eq!(snapshot.anchors.len(), 4);
            assert_eq!(document.object_ids().len(), 1);
        }
    }

    /// Criterion 13: a continued path of two nodes with no new node has no close target.
    #[test]
    fn a_two_node_continuation_cannot_close() {
        let document = Document::new(1);
        let mut minter = AnchorIdMinter::new(5);
        let _ = line(&document, 1, &[(0.0, 0.0), (20.0, 0.0)]);
        let mut pen = PenTool::new();
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(20.0, 0.0),
            false,
        );
        assert!(pen.closing_node().is_none());
        gesture(
            &mut pen,
            &mut minter,
            &document,
            Point::new(20.0, 20.0),
            false,
        );
        assert!(pen.closing_node().is_some(), "three nodes once closed");
    }
}
