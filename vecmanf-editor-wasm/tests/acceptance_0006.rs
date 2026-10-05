//! Black-box acceptance tests for `specs/0006-path-merge-split-and-node-
//! types/specification.md`'s acceptance criteria 6, 7, and the
//! cross-object case of criterion 9 — the parts of the spec that
//! `vecmanf-document-core/tests/acceptance_0006.rs` explicitly deferred
//! ("need `canvas-navigation-and-selection`'s `ObjectSelection`, which
//! does not exist on this branch") and that landed in this later round
//! on top of `vecmanf-editor-wasm::Session` and `vecmanf-ui-core`'s
//! `NodeSelection`/`NodeTool`.
//!
//! Written against `Session`'s public API only, before reading the
//! implementation diff for this round in depth (criteria 1-5, 8, 10-16
//! and AC9's same-object/split-then-rejoin case were already verified in
//! an earlier round and are not repeated here).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use vecmanf_document_core::{AnchorKind, Document, Point};
use vecmanf_editor_wasm::{Session, Tool};

fn document_of(session: &Session) -> Document {
    let bytes = session.pack("0.1.0").expect("pack");
    vecmanf_document_core::unpack(99, &bytes).expect("unpack")
}

/// Draws an open path with the Pen tool through consecutive clicks.
fn draw_path(session: &mut Session, points: &[Point]) {
    session.set_tool(Tool::Pen);
    for &p in points {
        session.pointer_down(p, false);
        session.pointer_up(p, false);
    }
    session.finish_pen();
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn point_approx_eq(a: Point, b: Point) -> bool {
    approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
}

// ---------------------------------------------------------------------
// AC 6: two selected path objects both show nodes in one Node-tool
// session.
// ---------------------------------------------------------------------

#[test]
fn ac6_two_select_tool_selected_objects_both_get_editable_nodes_in_the_node_tool() {
    let mut session = Session::new(1);
    draw_path(&mut session, &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    draw_path(
        &mut session,
        &[Point::new(0.0, 50.0), Point::new(10.0, 50.0)],
    );

    // Select both objects with the Select tool (shift-click).
    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);

    // Switch to the Node tool via the rail/shortcut (set_tool), not a
    // double-click.
    session.set_tool(Tool::Node);

    // Path A's node is hit-testable and selectable.
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    assert!(
        session.node_toolbar_state().can_delete,
        "path A's node is selected and editable"
    );

    // Path B's node is *also* hit-testable and selectable, in the same
    // session, without re-entering the Node tool.
    session.pointer_down(Point::new(0.0, 50.0), false);
    session.pointer_up(Point::new(0.0, 50.0), false);
    assert!(
        session.node_toolbar_state().can_delete,
        "path B's node is selected and editable too, same Node-tool session"
    );
}

// ---------------------------------------------------------------------
// AC 7: shift-clicking a node on a different visible path adds to the
// selection.
// ---------------------------------------------------------------------

#[test]
fn ac7_shift_click_a_node_on_a_different_object_adds_it_to_the_selection() {
    let mut session = Session::new(1);
    draw_path(&mut session, &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    draw_path(
        &mut session,
        &[Point::new(0.0, 50.0), Point::new(10.0, 50.0)],
    );

    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);

    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);

    // Both nodes selected together: endpoints of two different open
    // paths, so Join becomes available (AC 8/9 share this rule).
    assert!(
        session.node_toolbar_state().can_join,
        "both nodes stay selected together across the two objects"
    );
}

/// Architect review of the AC 6/7 follow-up
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`): "Rejected:
/// collapsing a cross-path selection on a plain press. It breaks AC 10
/// across paths." A *plain* (non-shift) click back on one of the two
/// already cross-path-selected nodes must keep the whole selection —
/// not reset it to just the clicked node — and the following drag must
/// move every selected node, on every path it spans, as one commit.
#[test]
fn plain_click_on_a_cross_path_selected_node_drags_both_paths_as_one_commit() {
    let mut session = Session::new(1);
    draw_path(&mut session, &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    draw_path(
        &mut session,
        &[Point::new(0.0, 50.0), Point::new(10.0, 50.0)],
    );

    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);

    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);
    assert!(
        session.node_toolbar_state().can_join,
        "cross-path selection built, same as AC 7's own test"
    );

    // A plain click (no shift) back on path A's own already-selected
    // node must not collapse the selection — Join must still apply.
    session.pointer_down(Point::new(0.0, 0.0), false);
    assert!(
        session.node_toolbar_state().can_join,
        "a plain press on an already-selected node keeps the whole \
         cross-path selection"
    );

    // Dragging from there moves both selected nodes, on both objects, as
    // one commit.
    session.pointer_up(Point::new(3.0, 4.0), false);

    let document = document_of(&session);
    let ids = document.object_ids();
    let snapshots: Vec<_> = ids.iter().map(|&id| document.path(id).unwrap()).collect();
    let moved_a = snapshots.iter().find_map(|p| {
        p.anchors
            .iter()
            .find(|a| point_approx_eq(a.point, Point::new(3.0, 4.0)))
    });
    let moved_b = snapshots.iter().find_map(|p| {
        p.anchors
            .iter()
            .find(|a| point_approx_eq(a.point, Point::new(3.0, 54.0)))
    });
    assert!(
        moved_a.is_some(),
        "path A's selected node moved by the drag offset: {snapshots:?}"
    );
    assert!(
        moved_b.is_some(),
        "path B's selected node moved by the same offset, even though \
         the press landed on path A's node: {snapshots:?}"
    );
}

/// Regression test for the rendering bug the implementer reports fixing:
/// `Session::decoration_input()` used to zip `selection.nodes()` against
/// `selection.path()` (which is `None` for a genuine multi-path
/// selection), so neither cross-path node ever rendered as selected.
/// Verified here purely through the public `draw_list()` surface: a
/// selected node's glyph is drawn as a single `--accent`-colored layer
/// (2 triangles for a Corner square), while an unselected node draws two
/// overlaid layers, outline + white inset (4 triangles) — so each node
/// that successfully becomes "selected" at the render level *reduces*
/// the total triangle count by exactly 2, for a plain Corner node with
/// no pulled handles and no hover ring in play.
#[test]
fn decoration_rendering_reflects_both_cross_path_selected_nodes() {
    let mut session = Session::new(1);
    draw_path(&mut session, &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    draw_path(
        &mut session,
        &[Point::new(0.0, 50.0), Point::new(10.0, 50.0)],
    );

    session.set_tool(Tool::Node);
    let baseline = session.draw_list().triangle_count();

    // Select path A's node alone.
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    let one_selected = session.draw_list().triangle_count();
    assert_eq!(
        baseline - one_selected,
        2,
        "selecting one plain Corner node with zero handles drops exactly \
         2 triangles (4 unselected -> 2 selected)"
    );

    // Shift-click path B's node: now a genuine cross-path selection.
    session.pointer_down(Point::new(0.0, 50.0), true);
    session.pointer_up(Point::new(0.0, 50.0), false);
    let two_selected = session.draw_list().triangle_count();
    assert_eq!(
        baseline - two_selected,
        4,
        "both cross-path nodes render selected — with the old zip-against-\
         path() bug this would regress all the way back to `baseline` \
         (0 selected), not even keep path A's own node selected"
    );
}

// ---------------------------------------------------------------------
// AC 9's cross-object case: two *pre-existing*, unrelated path objects
// (not reached via Split).
// ---------------------------------------------------------------------

/// First-selected is path A's last anchor; second-selected is path B's
/// first anchor — the "left unchanged" sub-case of AC9's own rule. A's
/// path should survive (its own `NodeId`, own order, unchanged), with
/// B's anchors appended unchanged (not reversed), merged at the midpoint,
/// Corner kind, selection collapsed to just the new node (AC11).
#[test]
fn ac9_cross_object_join_last_to_first_appends_second_path_unchanged() {
    let mut session = Session::new(1);
    draw_path(
        &mut session,
        &[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
        ],
    );
    draw_path(
        &mut session,
        &[Point::new(100.0, 50.0), Point::new(120.0, 50.0)],
    );

    let before = document_of(&session);
    let ids_before = before.object_ids();
    assert_eq!(ids_before.len(), 2);
    let path_a_id = before.path(ids_before[0]).expect("path").id;
    let path_b_id = before.path(ids_before[1]).expect("path").id;

    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(100.0, 50.0), true);
    session.pointer_up(Point::new(100.0, 50.0), false);

    session.set_tool(Tool::Node);
    // First-selected: path A's last anchor (20, 0).
    session.pointer_down(Point::new(20.0, 0.0), false);
    session.pointer_up(Point::new(20.0, 0.0), false);
    // Second-selected: path B's first anchor (100, 50).
    session.pointer_down(Point::new(100.0, 50.0), true);
    session.pointer_up(Point::new(100.0, 50.0), false);

    assert!(session.node_toolbar_state().can_join);
    session.join_selected();

    let after = document_of(&session);
    let ids_after = after.object_ids();
    assert_eq!(ids_after.len(), 1, "two objects merged into one (AC 9)");
    assert_eq!(
        ids_after[0], path_a_id,
        "path A survives with its own NodeId"
    );
    assert!(after.object_ids().iter().all(|&id| id != path_b_id));

    let merged = after.path(path_a_id).expect("survives");
    assert!(!merged.closed);
    assert_eq!(
        merged.anchors.len(),
        4,
        "3 + 2 anchors, minus the merged pair"
    );
    assert!(point_approx_eq(
        merged.anchors[0].point,
        Point::new(0.0, 0.0)
    ));
    assert!(point_approx_eq(
        merged.anchors[1].point,
        Point::new(10.0, 0.0)
    ));
    // Merged node at the midpoint of (20,0) and (100,50), no distance
    // limit or snapping.
    assert!(point_approx_eq(
        merged.anchors[2].point,
        Point::new(60.0, 25.0)
    ));
    assert_eq!(merged.anchors[2].kind, AnchorKind::Corner);
    // B's remaining anchor follows, unchanged order (not reversed),
    // since B's selected node was its *first* anchor.
    assert!(point_approx_eq(
        merged.anchors[3].point,
        Point::new(120.0, 50.0)
    ));

    // AC 11: only the newly merged node is selected afterward.
    assert!(
        !session.node_toolbar_state().can_join,
        "selection collapsed to one node"
    );
    assert!(
        session.node_toolbar_state().can_delete,
        "the merged node itself is selected"
    );
}

/// Same geometry, but selection order swapped: path B's node is clicked
/// first, path A's last node second. Per the spec, "first"/"second" mean
/// *selection order*, not path-creation order or any other notion of
/// first — so this must produce a different survivor (B, not A) and a
/// different resulting order than the previous test, proving the UI
/// layer actually threads click order through to
/// `Document::join_endpoints`, not just path-creation order.
#[test]
fn ac9_cross_object_join_honors_selection_order_not_path_creation_order() {
    let mut session = Session::new(1);
    draw_path(
        &mut session,
        &[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
        ],
    );
    draw_path(
        &mut session,
        &[Point::new(100.0, 50.0), Point::new(120.0, 50.0)],
    );

    let before = document_of(&session);
    let ids_before = before.object_ids();
    let path_a_id = before.path(ids_before[0]).expect("path").id;
    let path_b_id = before.path(ids_before[1]).expect("path").id;

    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(100.0, 50.0), false);
    session.pointer_up(Point::new(100.0, 50.0), false);
    session.pointer_down(Point::new(0.0, 0.0), true);
    session.pointer_up(Point::new(0.0, 0.0), false);

    session.set_tool(Tool::Node);
    // First-selected this time: path B's *first* anchor (100, 50).
    session.pointer_down(Point::new(100.0, 50.0), false);
    session.pointer_up(Point::new(100.0, 50.0), false);
    // Second-selected: path A's *last* anchor (20, 0).
    session.pointer_down(Point::new(20.0, 0.0), true);
    session.pointer_up(Point::new(20.0, 0.0), false);

    assert!(session.node_toolbar_state().can_join);
    session.join_selected();

    let after = document_of(&session);
    let ids_after = after.object_ids();
    assert_eq!(ids_after.len(), 1);
    assert_eq!(
        ids_after[0], path_b_id,
        "path B survives this time — it was first-selected, not path A"
    );
    assert!(after.object_ids().iter().all(|&id| id != path_a_id));

    let merged = after.path(path_b_id).expect("survives");
    assert_eq!(merged.anchors.len(), 4);
    // First-selected (B)'s path is *first* anchor -> result is
    // second-selected (A)'s path (reversed if A's node was A's first
    // anchor, unchanged if last) followed by B's own path unchanged.
    // A's selected node was its *last* anchor, so A's path is left
    // unchanged, then B's own two anchors follow.
    assert!(point_approx_eq(
        merged.anchors[0].point,
        Point::new(0.0, 0.0)
    ));
    assert!(point_approx_eq(
        merged.anchors[1].point,
        Point::new(10.0, 0.0)
    ));
    assert!(point_approx_eq(
        merged.anchors[2].point,
        Point::new(60.0, 25.0)
    ));
    assert_eq!(merged.anchors[2].kind, AnchorKind::Corner);
    assert!(point_approx_eq(
        merged.anchors[3].point,
        Point::new(120.0, 50.0)
    ));
}

/// AC9's kind-forcing rule holds for the cross-object case too: even
/// when both original endpoints were converted away from Corner before
/// Join, the merged node becomes Corner regardless.
#[test]
fn ac9_cross_object_join_forces_corner_kind_regardless_of_originals() {
    let mut session = Session::new(1);
    draw_path(
        &mut session,
        &[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
        ],
    );
    draw_path(
        &mut session,
        &[Point::new(100.0, 50.0), Point::new(120.0, 50.0)],
    );

    session.set_tool(Tool::Node);
    // Convert path A's last anchor to Symmetric.
    session.pointer_down(Point::new(20.0, 0.0), false);
    session.pointer_up(Point::new(20.0, 0.0), false);
    session.convert_selected(AnchorKind::Symmetric);
    // Convert path B's first anchor to Asymmetric.
    session.pointer_down(Point::new(100.0, 50.0), false);
    session.pointer_up(Point::new(100.0, 50.0), false);
    session.convert_selected(AnchorKind::Asymmetric);

    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false);
    session.pointer_down(Point::new(100.0, 50.0), true);
    session.pointer_up(Point::new(100.0, 50.0), false);

    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(20.0, 0.0), false);
    session.pointer_up(Point::new(20.0, 0.0), false);
    session.pointer_down(Point::new(100.0, 50.0), true);
    session.pointer_up(Point::new(100.0, 50.0), false);

    session.join_selected();

    let after = document_of(&session);
    let merged_path = after.path(after.object_ids()[0]).expect("exists");
    let merged = merged_path
        .anchors
        .iter()
        .find(|a| point_approx_eq(a.point, Point::new(60.0, 25.0)))
        .expect("merged anchor at midpoint");
    assert_eq!(
        merged.kind,
        AnchorKind::Corner,
        "Join always forces Corner, regardless of either original endpoint's kind"
    );
}
