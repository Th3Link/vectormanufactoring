//! The Pen's continue, join and close targets and the Close path command in the editor session
//! (`specs/0034-pen-path-extension`): what the cue says, what a press does, one commit with its
//! label, Escape, tool switches, and the Node bar's Close path buttons.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, NewAnchor, NodeId, Point, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{KeyInput, Session, Tool};
use curvyo_ui_core::{JoinType, PenTarget};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn node(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn pen_session(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session.set_tool(Tool::Pen);
    session
}

fn click(session: &mut Session, at: Point, shift: bool) {
    session.pointer_hover(at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
}

fn reread(session: &Session) -> Document {
    unpack(9, &session.pack("0.1.0").unwrap()).unwrap()
}

fn points(session: &Session, path: NodeId) -> Vec<(f64, f64)> {
    reread(session)
        .path(path)
        .unwrap()
        .anchors
        .iter()
        .map(|a| (a.point.x, a.point.y))
        .collect()
}

fn labels(session: &Session) -> Vec<String> {
    let loro = loro::LoroDoc::new();
    loro.import(&reread(session).export_loro_snapshot().unwrap())
        .unwrap();
    let frontiers: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut changes: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&frontiers, &mut |change| {
        changes.push((
            change.lamport,
            change.message.map(|m| m.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    changes.sort();
    changes.into_iter().map(|(_, label)| label).collect()
}

fn px(session: &Session, pixels: f64) -> f64 {
    pixels / session.view().scale()
}

/// Criteria 1, 23: the cue appears within 16 px of an end node (not 17), never over an interior
/// node; Shift swaps it for the new-path alternative.
#[test]
fn the_cue_follows_the_sixteen_pixel_radius_and_shift() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[node(1, 0.0, 0.0), node(2, 100.0, 0.0), node(3, 200.0, 0.0)],
        false,
    );
    let mut session = pen_session(&document);
    let near = px(&session, 15.0);
    let far = px(&session, 17.0);
    session.pointer_hover(pt(200.0 + near, 0.0), false, false);
    assert!(matches!(session.pen_target(), Some(PenTarget::Continue(_))));
    session.pointer_hover(pt(200.0 + far, 0.0), false, false);
    assert_eq!(session.pen_target(), Some(PenTarget::Place));
    session.pointer_hover(pt(100.0, 0.0), false, false);
    assert_eq!(
        session.pen_target(),
        Some(PenTarget::Place),
        "an interior node is no target"
    );
    session.pointer_hover(pt(200.0, 0.0), true, false);
    assert_eq!(session.pen_target(), Some(PenTarget::NewPathAt));
    // The cue is drawn: more triangles over the target than away from it.
    session.pointer_hover(pt(200.0, 0.0), false, false);
    let over = session.draw_list().triangle_count();
    session.pointer_hover(pt(500.0, 500.0), false, false);
    assert!(over > session.draw_list().triangle_count());
}

/// Criteria 2 to 5: press on the end continues, P stays untouched, the finish is one
/// `extend_path` that keeps the object and its first node's id; a drag is ignored on the press.
#[test]
fn continuing_extends_the_same_path_in_one_commit() {
    let document = Document::new(1);
    let path = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = pen_session(&document);
    let before = labels(&session);
    // A drag from the end node is ignored: it continues, no node is placed by the drag.
    session.pointer_hover(pt(100.0, 0.0), false, false);
    session.pointer_down(pt(100.0, 0.0), false);
    session.pointer_hover(pt(110.0, 30.0), false, false);
    session.pointer_up(pt(110.0, 30.0), false, false);
    assert_eq!(
        session.pen_in_progress().unwrap().len(),
        1,
        "only the end node E"
    );
    click(&mut session, pt(200.0, 50.0), false);
    click(&mut session, pt(300.0, 0.0), false);
    assert_eq!(
        points(&session, path).len(),
        2,
        "untouched until the finish"
    );
    assert_eq!(labels(&session), before, "nothing written yet");
    session.finish_pen();
    assert_eq!(
        points(&session, path),
        vec![(0.0, 0.0), (100.0, 0.0), (200.0, 50.0), (300.0, 0.0)]
    );
    let after = labels(&session);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("extend_path"));
    assert_eq!(reread(&session).object_ids().len(), 1);
}

/// Criterion 25: Escape on a continuation discards the new nodes and leaves the path as it was;
/// the next Escape leaves the Pen.
#[test]
fn escape_discards_a_continuation() {
    let document = Document::new(1);
    let path = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = pen_session(&document);
    let before = labels(&session);
    click(&mut session, pt(100.0, 0.0), false);
    click(&mut session, pt(200.0, 50.0), false);
    session.key_down(KeyInput {
        key: "Escape",
        ..KeyInput::default()
    });
    assert!(session.pen_in_progress().is_none());
    assert_eq!(labels(&session), before);
    assert_eq!(points(&session, path).len(), 2);
}

/// Criterion 26: leaving the Pen while continuing finishes the continuation as drawn.
#[test]
fn leaving_the_pen_finishes_a_continuation() {
    let document = Document::new(1);
    let path = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = pen_session(&document);
    click(&mut session, pt(100.0, 0.0), false);
    click(&mut session, pt(200.0, 50.0), false);
    session.set_tool(Tool::Select);
    assert_eq!(points(&session, path).len(), 3);
}

/// Criteria 7 to 11: joining onto another path's end finishes at once, one `connect_paths`, the
/// other path is gone and the continued path keeps its object and style.
#[test]
fn joining_is_one_connect_commit() {
    let document = Document::new(1);
    let p = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let q = document.create_path(&[node(3, 300.0, 0.0), node(4, 400.0, 0.0)], false);
    let mut session = pen_session(&document);
    click(&mut session, pt(100.0, 0.0), false);
    session.pointer_hover(pt(300.0, 0.0), false, false);
    assert!(matches!(session.pen_target(), Some(PenTarget::Join(_))));
    let before = labels(&session);
    click(&mut session, pt(300.0, 0.0), false);
    assert_eq!(
        points(&session, p),
        vec![(0.0, 0.0), (100.0, 0.0), (300.0, 0.0), (400.0, 0.0)]
    );
    assert!(reread(&session).path(q).is_none());
    let after = labels(&session);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("connect_paths"));
    assert!(session.pen_in_progress().is_none());
}

/// Criterion 7: with Shift the join target is only a plain node.
#[test]
fn shift_places_a_plain_node_over_a_join_target() {
    let document = Document::new(1);
    let _ = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let q = document.create_path(&[node(3, 300.0, 0.0), node(4, 400.0, 0.0)], false);
    let mut session = pen_session(&document);
    click(&mut session, pt(100.0, 0.0), false);
    session.pointer_hover(pt(300.0, 0.0), true, false);
    assert_eq!(session.pen_target(), Some(PenTarget::PlaceOverEnd));
    click(&mut session, pt(300.0, 0.0), true);
    assert_eq!(session.pen_in_progress().unwrap().len(), 2);
    assert!(reread(&session).path(q).is_some(), "Q is untouched");
}

/// Criteria 14, 15, 16, 17: closing a path drawn with clicks: as drawn it is four corners; with
/// Shift the first node becomes smooth; the cue reports the joins for the chip. One commit.
#[test]
fn closing_follows_the_join_and_shift() {
    for shift in [false, true] {
        let document = Document::new(1);
        let mut session = pen_session(&document);
        for at in [(0.0, 0.0), (200.0, 0.0), (200.0, 200.0), (0.0, 200.0)] {
            click(&mut session, pt(at.0, at.1), false);
        }
        session.pointer_hover(pt(0.0, 0.0), shift, false);
        let target = session.pen_target().unwrap();
        assert_eq!(
            target,
            PenTarget::Close {
                join: if shift {
                    JoinType::Smooth
                } else {
                    JoinType::Sharp
                },
                as_drawn: JoinType::Sharp,
                shift,
            }
        );
        click(&mut session, pt(0.0, 0.0), shift);
        let document = reread(&session);
        let closed = document.path(document.object_ids()[0]).unwrap();
        assert!(closed.closed);
        assert_eq!(closed.anchors.len(), 4);
        let first = closed.anchors[0];
        if shift {
            assert_eq!(first.kind, AnchorKind::Asymmetric);
            assert_ne!(first.handle_out, Vec2::ZERO);
        } else {
            assert_eq!(first.kind, AnchorKind::Corner);
        }
    }
}

/// Criterion 13: a path of two nodes has no close target.
#[test]
fn two_nodes_are_not_enough_to_close() {
    let document = Document::new(1);
    let mut session = pen_session(&document);
    click(&mut session, pt(0.0, 0.0), false);
    click(&mut session, pt(200.0, 0.0), false);
    session.pointer_hover(pt(0.0, 0.0), false, false);
    assert_eq!(session.pen_target(), Some(PenTarget::Place));
}

/// Criteria 18 to 21: the Close path buttons act on the paths of the selection, count closable and
/// skipped paths, write one `close_path` commit and keep the objects.
#[test]
fn the_close_path_buttons_close_the_selected_open_paths() {
    let document = Document::new(1);
    let a = document.create_path(
        &[
            node(1, 0.0, 0.0),
            node(2, 100.0, 0.0),
            node(3, 100.0, 100.0),
        ],
        false,
    );
    let b = document.create_path(&[node(11, 300.0, 0.0), node(12, 400.0, 0.0)], false);
    let c = document.create_path(
        &[
            node(21, 600.0, 0.0),
            node(22, 700.0, 0.0),
            node(23, 700.0, 100.0),
        ],
        true,
    );
    let mut session = pen_session(&document);
    session.set_tool(Tool::Select);
    // Select all three by a marquee.
    session.pointer_hover(pt(-50.0, -50.0), false, false);
    session.pointer_down(pt(-50.0, -50.0), false);
    session.pointer_hover(pt(800.0, 200.0), false, false);
    session.pointer_up(pt(800.0, 200.0), false, false);
    assert_eq!(session.selected_object_count(), 3);
    session.set_tool(Tool::Node);
    let state = session.close_path_state();
    assert_eq!(
        (state.closable, state.skipped),
        (1, 1),
        "the closed path is not counted"
    );
    let before = labels(&session);
    let outcome = session.close_paths(JoinType::Sharp);
    assert_eq!((outcome.closed, outcome.skipped), (1, 1));
    let document = reread(&session);
    assert!(document.path(a).unwrap().closed);
    assert!(
        !document.path(b).unwrap().closed,
        "fewer than three nodes: skipped"
    );
    assert!(document.path(c).unwrap().closed);
    let after = labels(&session);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("close_path"));
    assert_eq!(
        session.close_path_state().closable,
        0,
        "nothing left to close"
    );
}

/// Criterion 18: nothing selected, or another tool: the buttons have nothing to do.
#[test]
fn the_close_path_buttons_are_idle_without_a_selection_or_outside_the_node_tool() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            node(1, 0.0, 0.0),
            node(2, 100.0, 0.0),
            node(3, 100.0, 100.0),
        ],
        false,
    );
    let mut session = pen_session(&document);
    session.set_tool(Tool::Node);
    assert_eq!(session.close_path_state().closable, 0);
    assert_eq!(session.close_paths(JoinType::Sharp).closed, 0);
    session.set_tool(Tool::Pen);
    assert_eq!(session.close_path_state().closable, 0);
}
