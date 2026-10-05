//! The pen tool's state machine (acceptance criteria 1-5).
//!
//! `specs/0002-path-node-editing/adrs.md`, "a pen session is one commit": from
//! the first click to the double-click, close-path or Escape, the
//! in-progress path lives *only* as this type's own ephemeral state
//! (ADR 0009 §2) — [`vecmanf_document_core::Document`] gains exactly one
//! commit, at [`PenTool::finish`] or a closing
//! [`PenTool::pointer_up`]; [`PenTool::escape`] drops this state and
//! writes nothing.
//!
//! Double-click detection itself is the frontend's job (a real DOM
//! `dblclick` event, or platform equivalent) — this crate has no clock
//! (`CLAUDE.md` §6) and cannot time one itself — so [`PenTool::finish`]
//! is a direct action the caller invokes, not something this state
//! machine infers from two `pointer_up` calls.

use vecmanf_document_core::{AnchorId, AnchorKind, Document, Length, NewAnchor, NodeId, Point};

use crate::AnchorIdMinter;

/// One node placed in the current pen session, in the shape
/// [`Document::create_path`] needs.
type PlacedAnchor = NewAnchor;

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
        nodes: Vec<PlacedAnchor>,
        down: Option<PointerDown>,
    },
}

#[derive(Debug, Clone, Copy)]
struct PointerDown {
    point: Point,
    /// Set at `pointer_down` time when this press landed on the
    /// in-progress path's own first node with enough nodes already placed
    /// to close (acceptance criterion 5) — decided then, not re-checked
    /// at release, so a drag that wanders away from the first node still
    /// closes: closing is a press-and-release gesture on that one target,
    /// not a final-position check.
    closing: bool,
}

/// What [`PenTool::pointer_up`] actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerUpOutcome {
    /// No node was placed because no matching `pointer_down` was pending
    /// (e.g. the pen tool was idle, or `escape`/`finish` already consumed
    /// it).
    Ignored,
    /// A node was placed; the path is still being drawn.
    Placed,
    /// Acceptance criterion 5: the press landed on the path's own first
    /// node, so the path closed and committed as this [`NodeId`].
    Closed(NodeId),
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

    /// The in-progress path's placed nodes, for a renderer's preview —
    /// `None` when idle. Never the document's own data: this state is
    /// ephemeral by design (ADR 0009 §2).
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
    /// mouse is up), or the pending press is a close-path gesture
    /// (acceptance criterion 5 closes on a plain click; closing commits
    /// no new node to preview).
    ///
    /// Built by `PenTool::resolve_anchor` (private: this crate's own
    /// internal helper, not part of its public surface) — the exact same
    /// resolution rule [`PenTool::pointer_up`] itself calls — so the preview and the
    /// eventual commit are structurally guaranteed to agree (`specs/0002-
    /// path-node-editing/adrs.md`: "commands carry resolved geometry,
    /// never geometric intent", applied here to the preview too). A
    /// separate re-implementation of this rule in `vecmanf-render-core`
    /// previously ignored `drag_threshold` entirely, so a drag just under
    /// it previewed a smooth node with handles that then committed, on
    /// release, as a corner node with none — this method is what closes
    /// that gap.
    ///
    /// `id` should be [`crate::AnchorIdMinter::peek`]'s result, not a
    /// minted one: a preview that advanced the minter's own counter would
    /// desync it from what the gesture might still turn into (Escape
    /// discards it, a press turning out to be a close-path gesture mints
    /// nothing) — the same reason [`PenTool::pointer_up`] only mints once
    /// it has committed to actually placing a node.
    ///
    /// This is a *different* point from the last entry of
    /// [`PenTool::in_progress_nodes`]: that list only gains the resolved
    /// anchor once the press *releases* ([`PenTool::pointer_up`] is what
    /// actually pushes it), so a renderer asking only `in_progress_nodes`
    /// has no way to know a drag is even in flight, let alone what it
    /// would resolve to — which is exactly why the live drag preview
    /// needs this method and not just the existing one.
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
        if down.closing {
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
                kind: AnchorKind::Smooth,
            }
        } else {
            NewAnchor::corner(id, down)
        }
    }

    /// Acceptance criteria 1, 2, 5: the maker pressed the mouse button
    /// down at `point`. Starts a new path if none is in progress.
    ///
    /// `close_tolerance` is how close `point` must be to the in-progress
    /// path's first node, with at least two nodes already placed (so
    /// closing leaves at least three, acceptance criterion 5's own
    /// precondition), for this press to be a close-path gesture rather
    /// than a new node.
    pub fn pointer_down(&mut self, point: Point, close_tolerance: Length) {
        match &mut self.state {
            State::Idle => {
                self.state = State::Placing {
                    nodes: Vec::new(),
                    down: Some(PointerDown {
                        point,
                        closing: false,
                    }),
                };
            }
            State::Placing { nodes, down } => {
                let closing = Self::is_close_target(nodes, point, close_tolerance);
                *down = Some(PointerDown { point, closing });
            }
        }
    }

    /// Whether `point` is close enough to the in-progress path's own
    /// first node, with enough nodes already placed, to read as a
    /// close-path press (acceptance criterion 5) — the exact rule
    /// [`PenTool::pointer_down`] commits to, factored out so a hover-
    /// time query ([`PenTool::is_hovering_close_target`]) can use the
    /// identical condition rather than a second copy that could drift
    /// from it (a maker must never see a "this will close the path"
    /// hover cue and then have the actual click decide otherwise).
    fn is_close_target(nodes: &[PlacedAnchor], point: Point, close_tolerance: Length) -> bool {
        nodes.len() >= 2
            && nodes.first().is_some_and(|first| {
                first.point.vector_to(point).length() <= close_tolerance.as_mm()
            })
    }

    /// Acceptance criterion 5's own UX note (`specification.md`'s
    /// "Cursors"): whether `point` (the live cursor, in document space)
    /// is currently over the in-progress path's own close target, for
    /// the host to swap in the close-path cursor variant and the first
    /// node's hover ring. `false` when idle (nothing to close).
    #[must_use]
    pub fn is_hovering_close_target(&self, point: Point, close_tolerance: Length) -> bool {
        match &self.state {
            State::Idle => false,
            State::Placing { nodes, .. } => Self::is_close_target(nodes, point, close_tolerance),
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
    /// release.
    pub fn pointer_up(
        &mut self,
        minter: &mut AnchorIdMinter,
        document: &Document,
        point: Point,
        drag_threshold: Length,
    ) -> PointerUpOutcome {
        let State::Placing { nodes, down } = &mut self.state else {
            return PointerUpOutcome::Ignored;
        };
        let Some(down) = down.take() else {
            return PointerUpOutcome::Ignored;
        };

        if down.closing {
            let path_id = document.create_path(nodes, true);
            self.state = State::Idle;
            return PointerUpOutcome::Closed(path_id);
        }

        let id = minter.mint();
        let anchor = Self::resolve_anchor(id, down.point, point, drag_threshold);
        nodes.push(anchor);
        PointerUpOutcome::Placed
    }

    /// Acceptance criterion 3: commits the in-progress path as an open
    /// path object, exactly as drawn, and returns to idle ready to start
    /// a new, separate path. A no-op (returns `None`, changes nothing)
    /// when there is no path in progress or it has fewer than two nodes —
    /// "at least one segment" is this criterion's own precondition.
    pub fn finish(&mut self, document: &Document) -> Option<NodeId> {
        let State::Placing { nodes, .. } = &self.state else {
            return None;
        };
        if nodes.len() < 2 {
            return None;
        }
        let path_id = document.create_path(nodes, false);
        self.state = State::Idle;
        Some(path_id)
    }

    /// Acceptance criterion 4: discards the entire in-progress path,
    /// writing nothing — not an undo (`specs/0002-path-node-editing/adrs.md`:
    /// "AC4's Escape is not an undo and must not be built as one").
    /// Returns whether there was anything to discard.
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
        assert_eq!(nodes[0].handle_out, vecmanf_document_core::Vec2::ZERO);
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
        assert_eq!(c.kind, AnchorKind::Smooth);
        assert_eq!(c.handle_out, vecmanf_document_core::Vec2::new(3.0, 4.0));
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

    /// Acceptance criterion 5's hover cue: once enough nodes are placed,
    /// hovering near the first node reports the close target; hovering
    /// elsewhere, or being idle, does not.
    #[test]
    fn is_hovering_close_target_matches_pointer_downs_own_closing_decision() {
        let document = Document::new(1);
        let mut minter = minter();
        let mut pen = PenTool::new();
        assert!(
            !pen.is_hovering_close_target(Point::new(0.0, 0.0), CLOSE_TOLERANCE),
            "idle: nothing to close"
        );

        pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
        pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
        pen.pointer_up(
            &mut minter,
            &document,
            Point::new(10.0, 0.0),
            DRAG_THRESHOLD,
        );

        assert!(
            pen.is_hovering_close_target(Point::new(0.1, 0.1), CLOSE_TOLERANCE),
            "within tolerance of the first node, with two nodes placed"
        );
        assert!(
            !pen.is_hovering_close_target(Point::new(10.0, 0.0), CLOSE_TOLERANCE),
            "near the last node, not the first, is not a close target"
        );
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
                handle_in: vecmanf_document_core::Vec2::new(-3.0, -4.0),
                handle_out: vecmanf_document_core::Vec2::new(3.0, 4.0),
                kind: AnchorKind::Smooth,
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
                handle_in: vecmanf_document_core::Vec2::new(-3.0, -4.0),
                handle_out: vecmanf_document_core::Vec2::new(3.0, 4.0),
                kind: AnchorKind::Smooth,
            }),
            "held at C, the press position — not wherever the cursor ended up"
        );
    }

    /// The bug this test guards against: a drag just under
    /// `drag_threshold` must preview the exact same corner node (no
    /// handles) that `pointer_up` would actually commit at that cursor
    /// position — not a smooth node with handles that then disagrees with
    /// the commit on release. Before `pending_anchor` shared
    /// `resolve_anchor` with `pointer_up`, `vecmanf-render-core`'s own
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
        assert_eq!(committed.handle_in, vecmanf_document_core::Vec2::ZERO);
        assert_eq!(committed.handle_out, vecmanf_document_core::Vec2::ZERO);
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
}
