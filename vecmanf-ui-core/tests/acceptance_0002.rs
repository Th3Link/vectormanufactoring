//! Black-box acceptance tests for `specs/0002-path-node-editing/
//! specification.md`'s 14 acceptance criteria, written against
//! `vecmanf-ui-core`'s public API (`PenTool`, `NodeTool`, `NodeSelection`,
//! `AnchorIdMinter`, `hit_test`) and `vecmanf-document-core`'s own public
//! `Document`, independent of the implementer's inline `#[cfg(test)]`
//! modules in `src/pen_tool.rs` / `src/node_tool.rs` — different scenarios
//! and boundary values, as an integration-test-level second opinion.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{
    AnchorId, AnchorKind, Document, HandleSlot, NewAnchor, Point, Tolerance, Vec2,
};
use vecmanf_ui_core::{AnchorIdMinter, HitTolerances, NodeTool, PenPointerUpOutcome, PenTool};

const CLOSE_TOLERANCE: vecmanf_document_core::Length = vecmanf_document_core::Length::from_mm(2.0);
const DRAG_THRESHOLD: vecmanf_document_core::Length = vecmanf_document_core::Length::from_mm(1.0);
const TOLERANCES: HitTolerances = HitTolerances {
    point: Tolerance::from_mm(2.0),
    segment: Tolerance::from_mm(1.0),
};

// ---------------------------------------------------------------------
// AC1 / AC2: drawing
// ---------------------------------------------------------------------

/// AC2's drag/click boundary: a release distance exactly AT the drag
/// threshold must still count as a plain click (corner node), matching
/// `pointer_up`'s own documented "over it is a drag" — strictly greater
/// than, not greater-or-equal.
#[test]
fn drag_distance_exactly_at_threshold_is_still_a_corner_click() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();

    pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
    pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);

    pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
    // Release exactly 1.0mm away == DRAG_THRESHOLD.
    pen.pointer_up(
        &mut minter,
        &document,
        Point::new(11.0, 0.0),
        DRAG_THRESHOLD,
    );

    let nodes = pen.in_progress_nodes().expect("still placing");
    assert_eq!(
        nodes[1].kind,
        AnchorKind::Corner,
        "distance exactly at the threshold must not count as a drag"
    );
}

/// AC1: a straight segment between two corner nodes has both handles at
/// the zero vector on both ends (not just the first one).
#[test]
fn ac1_both_corner_nodes_have_both_handles_zero() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
    pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
    pen.pointer_down(Point::new(10.0, 5.0), CLOSE_TOLERANCE);
    pen.pointer_up(
        &mut minter,
        &document,
        Point::new(10.0, 5.0),
        DRAG_THRESHOLD,
    );
    let id = pen.finish(&document).expect("one segment");
    let snapshot = document.path(id).expect("exists");
    for anchor in &snapshot.anchors {
        assert_eq!(anchor.handle_in, Vec2::ZERO);
        assert_eq!(anchor.handle_out, Vec2::ZERO);
        assert_eq!(anchor.kind, AnchorKind::Corner);
    }
}

// ---------------------------------------------------------------------
// AC3 / AC4: finishing and discarding
// ---------------------------------------------------------------------

/// AC3's own precondition, boundary case: finishing with exactly zero
/// nodes placed (`pointer_down` with no matching `pointer_up` at all) is
/// a no-op, not a panic.
#[test]
fn finish_with_zero_nodes_is_a_no_op() {
    let document = Document::new(1);
    let mut pen = PenTool::new();
    pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
    // No pointer_up: the state is "Placing" with zero committed nodes.
    assert_eq!(pen.finish(&document), None);
    assert_eq!(document.object_ids(), Vec::new());
}

/// AC4: Escape discards every placed node, even a long multi-node
/// in-progress path, and leaves the document with *zero* paths and *zero*
/// other side effects — re-checking "zero commits" with more nodes than
/// the implementer's own 2-node example.
#[test]
fn ac4_escape_after_many_nodes_commits_absolutely_nothing() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    // Not just "no paths exist": the document's own Loro snapshot bytes
    // must be identical before and after, proving the whole pen
    // session — ten placed nodes, then Escape — committed literally
    // nothing, not even an empty commit (`specs/0002-path-node-editing/
    // adrs.md`'s PR review: "each mutating method ends in exactly one
    // Loro commit" — the flip side is that a *discarded* session must
    // end in zero). `object_ids()` is called once, before either snapshot,
    // so its own first-ever access to the (so far untouched) `paths`
    // tree container — which Loro registers in the document's container
    // list the first time *anything* asks for it by name, read or write
    // alike — happens before `before` is captured rather than showing up
    // as a spurious diff between the two.
    let _ = document.object_ids();
    let before = document.export_loro_snapshot().expect("snapshot");
    for i in 0..10 {
        let p = Point::new(f64::from(i) * 3.0, f64::from(i) * 2.0);
        pen.pointer_down(p, CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, p, DRAG_THRESHOLD);
    }
    assert!(pen.escape());
    assert_eq!(document.object_ids(), Vec::new());
    assert!(!pen.is_placing());
    let after = document.export_loro_snapshot().expect("snapshot");
    assert_eq!(
        before, after,
        "an escaped pen session must not change the document's own bytes at all"
    );
}

/// AC4 interaction with AC2: escaping while a click-drag gesture is
/// physically in flight (pointer down, dragged, not yet released) must
/// still discard cleanly once Escape fires — dropping the pending
/// `pointer_down` state along with everything already placed.
#[test]
fn escape_mid_drag_gesture_discards_cleanly() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
    pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
    // Press down for a second node and "drag" (no release yet).
    pen.pointer_down(Point::new(10.0, 0.0), CLOSE_TOLERANCE);
    assert!(pen.escape());
    assert!(!pen.is_placing());
    assert_eq!(document.object_ids(), Vec::new());
}

// ---------------------------------------------------------------------
// AC5: closing
// ---------------------------------------------------------------------

/// AC5's own precondition, different shape: exactly three nodes is the
/// minimum that can close (not just "fewer than three can't").
#[test]
fn ac5_exactly_three_nodes_can_close() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    for p in [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(5.0, 10.0),
    ] {
        pen.pointer_down(p, CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, p, DRAG_THRESHOLD);
    }
    pen.pointer_down(Point::new(0.05, 0.05), CLOSE_TOLERANCE);
    let outcome = pen.pointer_up(
        &mut minter,
        &document,
        Point::new(0.05, 0.05),
        DRAG_THRESHOLD,
    );
    assert!(matches!(outcome, PenPointerUpOutcome::Closed(_)));
}

/// AC5: after closing, the pen tool is idle and a fresh click starts an
/// entirely separate new path object (not a continuation).
#[test]
fn ac5_closing_resets_the_tool_for_a_new_path() {
    let document = Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    for p in [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(5.0, 10.0),
    ] {
        pen.pointer_down(p, CLOSE_TOLERANCE);
        pen.pointer_up(&mut minter, &document, p, DRAG_THRESHOLD);
    }
    pen.pointer_down(Point::new(0.0, 0.0), CLOSE_TOLERANCE);
    pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), DRAG_THRESHOLD);
    assert_eq!(document.object_ids().len(), 1);

    pen.pointer_down(Point::new(100.0, 100.0), CLOSE_TOLERANCE);
    pen.pointer_up(
        &mut minter,
        &document,
        Point::new(100.0, 100.0),
        DRAG_THRESHOLD,
    );
    assert!(pen.is_placing());
    assert_eq!(
        document.object_ids().len(),
        1,
        "the new click must not have touched the already-closed path"
    );
}

// ---------------------------------------------------------------------
// AC7-10: selection and movement
// ---------------------------------------------------------------------

fn three_node_path(
    document: &Document,
    a: AnchorId,
    b: AnchorId,
    c: AnchorId,
) -> vecmanf_document_core::NodeId {
    document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    )
}

/// AC10, a variant the inline tests don't cover: shift-clicking the SAME
/// already-selected node twice toggles it back out, and a subsequent drag
/// of a remaining selected node leaves the deselected one untouched.
#[test]
fn ac10_shift_click_toggle_off_then_drag_only_moves_what_remains_selected() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = three_node_path(&document, a, b, c);
    let mut tool = NodeTool::new();

    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(20.0, 0.0), TOLERANCES, true); // shift-add c
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, true); // shift-remove a
    assert_eq!(tool.selection().nodes(), &[c]);

    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(20.0, 0.0), TOLERANCES, false);
    tool.pointer_up(&document, Point::new(25.0, 3.0));

    let snapshot = document.path(path).unwrap();
    let by_id = |id: AnchorId| snapshot.anchors.iter().find(|x| x.id == id).unwrap().point;
    assert_eq!(by_id(a), Point::new(0.0, 0.0), "deselected, must not move");
    assert_eq!(by_id(c), Point::new(25.0, 3.0));
}

/// AC8 (as narrowed by architect review against ADR 0009 §2/§3): neither
/// AC8 nor AC9 covers a zero-length drag, and under ADR 0009 §3 `point`
/// is an LWW register — re-writing the same value is still a *new*
/// operation with a newer clock, which can beat a collaborator's real
/// concurrent move of the same node on merge. So a press-and-release at
/// the same point on a node that is *already selected* (isolating this
/// from the separate question of whether a bare selecting click should
/// commit) must commit nothing at all, not a value-preserving move.
#[test]
fn ac8_zero_delta_node_drag_is_a_no_op() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let mut tool = NodeTool::new();

    // Select node `a` with a real (non-zero) drag first, so the
    // zero-delta drag under test starts from a node that is already
    // selected, rather than itself being the selecting click.
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    tool.pointer_up(&document, Point::new(1.0, 0.0));
    assert!(tool.selection().contains_node(a), "must be selected now");

    // Sanity check, called out explicitly by the architect: confirm
    // `export_loro_snapshot` is itself deterministic for unchanged state
    // before relying on byte-equality as an assertion below.
    let snap1 = document.export_loro_snapshot().expect("snapshot");
    let snap2 = document.export_loro_snapshot().expect("snapshot");
    assert_eq!(
        snap1, snap2,
        "export_loro_snapshot must be deterministic for the same unchanged state"
    );

    let point_before = document.path(path).unwrap().anchors[0].point;
    let before = document.export_loro_snapshot().expect("snapshot");

    // Press and release at the same point on the already-selected node.
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, point_before, TOLERANCES, false);
    let outcome = tool.pointer_up(&document, point_before);

    let after = document.export_loro_snapshot().expect("snapshot");

    assert_eq!(outcome, vecmanf_ui_core::NodePointerUpOutcome::NoOp);
    assert_eq!(document.path(path).unwrap().anchors[0].point, point_before);
    assert_eq!(
        before, after,
        "a zero-movement drag on an already-selected node must commit nothing \
         (ADR 0009 §3: re-writing the same LWW value is still a newer op)"
    );
}

/// Architect review of AC8/AC9: handle drags wrote the *absolute*
/// pointer position as the handle's new value, rather than a movement
/// relative to the handle's own starting value. So pressing within hit
/// tolerance but off the handle's exact tip, then releasing at that same
/// spot (zero movement), would silently relocate the handle to wherever
/// the click landed even though the maker never dragged it anywhere.
/// That must be a no-op instead.
#[test]
fn ac9_zero_delta_handle_drag_off_tip_is_a_no_op() {
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
    let mut tool = NodeTool::new();

    // Select node `a` with a real (non-zero) drag so its handles become
    // hittable at all (hit_test_handle only considers a selected node's
    // handles) without that selecting click itself being the
    // zero-delta drag under test.
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    tool.pointer_up(&document, Point::new(1.0, 0.0));
    assert!(tool.selection().contains_node(a), "must be selected now");

    // handle_out is a relative offset, unaffected by the node's own
    // move, so its tip is now at (1,0) + (5,0) = (6,0). Press 1mm off
    // that exact tip (within the 2mm point tolerance) and release at
    // the same spot: zero movement, but not on the tip itself.
    let anchor_point = document.path(path).unwrap().anchors[0].point;
    let handle_before = document.path(path).unwrap().anchors[0].handle_out;
    let tip = anchor_point.translated(handle_before);
    let press_point = tip.translated(Vec2::new(0.0, 1.0));

    let before = document.export_loro_snapshot().expect("snapshot");

    let paths = vec![document.path(path).unwrap()];
    let down_outcome = tool.pointer_down(&paths, press_point, TOLERANCES, false);
    assert_eq!(
        down_outcome,
        vecmanf_ui_core::NodePointerDownOutcome::Handle,
        "the press must land on the handle, not the node or nothing"
    );
    let outcome = tool.pointer_up(&document, press_point);

    let after = document.export_loro_snapshot().expect("snapshot");

    assert_eq!(outcome, vecmanf_ui_core::NodePointerUpOutcome::NoOp);
    assert_eq!(
        document.path(path).unwrap().anchors[0].handle_out,
        handle_before,
        "the handle must stay exactly where it was, not jump to the click position"
    );
    assert_eq!(
        before, after,
        "a zero-movement drag off a handle's exact tip must commit nothing"
    );
}

/// AC9: dragging a handle back to the zero vector (retracting it fully)
/// must be representable — a maker can turn a smooth node's pulled-out
/// handle back into a corner-like retracted handle without changing
/// `kind`.
#[test]
fn ac9_dragging_a_handle_to_zero_retracts_it_without_changing_kind() {
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
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    tool.pointer_up(&document, Point::new(0.0, 0.0));

    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(5.0, 0.0), TOLERANCES, false);
    // Drag the handle all the way back onto the node itself.
    tool.pointer_up(&document, Point::new(0.0, 0.0));

    let snapshot = document.path(path).unwrap();
    let anchor = &snapshot.anchors[0];
    assert_eq!(anchor.handle_out, Vec2::ZERO);
    assert_eq!(
        anchor.handle_in,
        Vec2::ZERO,
        "smooth mirror follows to zero too"
    );
    assert_eq!(
        anchor.kind,
        AnchorKind::Smooth,
        "kind itself is untouched by geometry"
    );
}

// ---------------------------------------------------------------------
// AC11-13: convert, insert, delete
// ---------------------------------------------------------------------

/// AC11: converting every node of a multi-selection at once (not just a
/// single selected node) converts all of them.
#[test]
fn ac11_convert_selected_applies_to_every_selected_node() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = three_node_path(&document, a, b, c);
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(20.0, 0.0), TOLERANCES, true);

    tool.convert_selected(&document, AnchorKind::Smooth);
    let snapshot = document.path(path).unwrap();
    assert_eq!(snapshot.anchors[0].kind, AnchorKind::Smooth);
    assert_eq!(snapshot.anchors[2].kind, AnchorKind::Smooth);
    assert_eq!(snapshot.anchors[1].kind, AnchorKind::Corner, "untouched");
}

/// AC13: deleting nodes so that exactly one remains (not zero) must
/// still remove the whole path object, per "fewer than two nodes" — one
/// is fewer than two.
#[test]
fn ac13_deleting_down_to_exactly_one_node_removes_the_whole_path() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = three_node_path(&document, a, b, c);
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, true);

    tool.delete_selected(&document);
    assert_eq!(
        document.path(path),
        None,
        "two of three deleted -> one left -> whole path gone"
    );
    assert_eq!(document.object_ids(), Vec::new());
}

/// AC13: deleting ALL nodes of a path at once also removes the whole path
/// (zero remaining, the most extreme case of "fewer than two").
#[test]
fn ac13_deleting_every_node_at_once_removes_the_whole_path() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, true);
    assert_eq!(tool.selection().nodes(), &[a, b]);

    tool.delete_selected(&document);
    assert_eq!(document.path(path), None);
}

/// AC13: deleting a node on a *closed* triangle leaves two nodes and one
/// segment (still a path object, since two is not fewer than two) and the
/// result must no longer be closed-with-wraparound in a way that creates
/// a duplicate segment — specifically the two survivors must be joined by
/// exactly one segment.
#[test]
fn ac13_deleting_one_node_of_a_closed_triangle_leaves_a_two_node_path() {
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
        true,
    );
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
    tool.delete_selected(&document);

    let snapshot = document
        .path(path)
        .expect("two nodes remain, path survives");
    assert_eq!(snapshot.anchors.len(), 2);
}

/// AC12: inserting on the wraparound (closing) segment of a closed path
/// works the same as any other segment.
#[test]
fn ac12_insert_on_the_closing_segment_of_a_closed_path() {
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
        true,
    );
    let mut tool = NodeTool::new();
    let mut minter = AnchorIdMinter::new(1);
    let paths = vec![document.path(path).unwrap()];
    // Midpoint of the wraparound segment c -> a.
    let midpoint = Point::new(2.5, 5.0);
    let result = tool.insert_at(&mut minter, &document, &paths, midpoint, TOLERANCES);
    assert_eq!(result, Some(path));
    let snapshot = document.path(path).unwrap();
    assert_eq!(snapshot.anchors.len(), 4);
    assert!(snapshot.closed);
}

// ---------------------------------------------------------------------
// AC14: segments
// ---------------------------------------------------------------------

/// AC14: "make line" on a segment that is already a line is idempotent
/// (toolbar disables it, but the underlying command must still be safe
/// to call and must not move the endpoints or error).
#[test]
fn ac14_make_line_on_an_already_straight_segment_is_idempotent() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    tool.pointer_down(&paths, Point::new(10.0, 0.0), TOLERANCES, false);
    tool.make_line(&document);
    let snapshot = document.path(path).unwrap();
    assert_eq!(snapshot.anchors[0].handle_out, Vec2::ZERO);
    assert_eq!(snapshot.anchors[0].point, Point::new(0.0, 0.0));
    assert_eq!(snapshot.anchors[1].point, Point::new(20.0, 0.0));
}

/// AC14: clicking exactly on a node (not between two nodes) must select
/// the node, never the segment — disambiguating AC7 vs AC14's "between
/// two nodes rather than on either one" wording with a direct check.
#[test]
fn ac14_clicking_exactly_on_a_node_selects_the_node_not_a_segment() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    let outcome = tool.pointer_down(&paths, Point::new(0.0, 0.0), TOLERANCES, false);
    assert_eq!(outcome, vecmanf_ui_core::NodePointerDownOutcome::Node);
    assert_eq!(tool.selection().nodes(), &[a]);
    assert_eq!(tool.selection().segment(), None);
}

/// AC14 on a closed path's wraparound segment: make-curve on the
/// closing segment extends the correct pair of handles (last anchor's
/// `handle_out`, first anchor's `handle_in`), not some other pair.
#[test]
fn ac14_make_curve_on_the_wraparound_segment_touches_the_right_handles() {
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
        true,
    );
    let mut tool = NodeTool::new();
    let paths = vec![document.path(path).unwrap()];
    // Midpoint of the closing segment c -> a.
    let outcome = tool.pointer_down(&paths, Point::new(2.5, 5.0), TOLERANCES, false);
    assert_eq!(outcome, vecmanf_ui_core::NodePointerDownOutcome::Segment);
    assert_eq!(tool.selection().segment(), Some((c, a)));

    tool.make_curve(&document);
    let snapshot = document.path(path).unwrap();
    assert_ne!(
        snapshot.anchors[2].handle_out,
        Vec2::ZERO,
        "c's handle_out (toward a)"
    );
    assert_ne!(
        snapshot.anchors[0].handle_in,
        Vec2::ZERO,
        "a's handle_in (toward c)"
    );
    assert_eq!(snapshot.anchors[1].handle_in, Vec2::ZERO, "b untouched");
    assert_eq!(snapshot.anchors[1].handle_out, Vec2::ZERO, "b untouched");
}

// ---------------------------------------------------------------------
// Direct Document-level checks (AC6, AC9's corner/smooth set_handle API)
// ---------------------------------------------------------------------

/// AC6: every path, regardless of how it was drawn (straight AC1, curved
/// AC2, or closed AC5), reads back the exact 0.25mm/black/no-fill
/// placeholder style — checked directly against `PathSnapshot` here as a
/// second, independent confirmation alongside `vecmanf-document-core`'s
/// own inline tests.
#[test]
fn ac6_every_path_shape_gets_the_identical_placeholder_style() {
    let document = Document::new(1);
    let straight = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(1.0, 1.0)),
        ],
        false,
    );
    let curved = document.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(1, 3),
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(1.0, 1.0),
                kind: AnchorKind::Smooth,
            },
            NewAnchor::corner(AnchorId::new(1, 4), Point::new(5.0, 5.0)),
        ],
        false,
    );
    let closed = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 5), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 6), Point::new(1.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 7), Point::new(0.5, 1.0)),
        ],
        true,
    );
    for id in [straight, curved, closed] {
        let snapshot = document.path(id).unwrap();
        assert!((snapshot.stroke_width.as_mm() - 0.25).abs() < 1e-9);
        assert_eq!(snapshot.stroke, vecmanf_document_core::Color::BLACK);
        assert_eq!(snapshot.fill, None);
    }
}

/// `HandleSlot`/`set_handle` as used directly (not via the node tool's
/// drag path): dragging the `In` slot on a smooth node mirrors `Out`,
/// matching the `Out`-slot case already covered elsewhere — a symmetry
/// check the implementer's inline tests only exercise from one side.
#[test]
fn set_handle_in_slot_on_smooth_mirrors_out_slot_too() {
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(1, 1),
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::new(-5.0, 0.0),
                handle_out: Vec2::new(5.0, 0.0),
                kind: AnchorKind::Smooth,
            },
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
        ],
        false,
    );
    document
        .set_handle(
            id,
            AnchorId::new(1, 1),
            HandleSlot::In,
            Vec2::new(2.0, -3.0),
        )
        .unwrap();
    let snapshot = document.path(id).unwrap();
    assert_eq!(snapshot.anchors[0].handle_in, Vec2::new(2.0, -3.0));
    assert_eq!(snapshot.anchors[0].handle_out, Vec2::new(-2.0, 3.0));
}
