//! A compound path in the editor session (`specs/0016-boolean-operations`
//! criteria 38, 38a): the Node tool shows no node, handle or segment of it,
//! cannot select, join, split or delete its nodes, and an ordinary path in the
//! same document is node-editable as before.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{AnchorId, Document, NewAnchor, Point, pack, unpack};
use curvyo_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn corners(first: u64, x: f64, y: f64, side: f64, reversed: bool) -> (Vec<NewAnchor>, bool) {
    let mut at = vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    if reversed {
        at.reverse();
    }
    (
        at.into_iter()
            .enumerate()
            .map(|(k, (cx, cy))| NewAnchor::corner(AnchorId::new(6, first + k as u64), pt(cx, cy)))
            .collect(),
        true,
    )
}

/// A ring at the origin and an ordinary open path far to the right.
fn bytes() -> Vec<u8> {
    let document = Document::new(1);
    let seed = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(9, 1), pt(500.0, 500.0)),
            NewAnchor::corner(AnchorId::new(9, 2), pt(501.0, 500.0)),
        ],
        false,
    );
    let _ = document
        .replace_with_path(
            &[seed],
            seed,
            &[
                corners(100, 0.0, 0.0, 40.0, false),
                corners(200, 10.0, 10.0, 20.0, true),
            ],
            "boolean_difference",
        )
        .unwrap();
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(8, 1), pt(100.0, 100.0)),
            NewAnchor::corner(AnchorId::new(8, 2), pt(140.0, 100.0)),
            NewAnchor::corner(AnchorId::new(8, 3), pt(140.0, 140.0)),
        ],
        false,
    );
    pack(&document, "0.1.0").unwrap()
}

fn session() -> Session {
    let mut session = Session::open(2, &bytes()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session
}

fn json(session: &Session) -> Vec<u8> {
    unpack(3, &session.pack("0.1.0").unwrap())
        .unwrap()
        .export_json()
        .unwrap()
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

/// The ring's corners are not nodes in the Node tool: a click selects nothing,
/// and Join, Split, convert and delete change nothing.
#[test]
fn the_node_tool_cannot_touch_the_nodes_of_a_compound_path() {
    let mut session = session();
    session.set_tool(Tool::Node);
    let before = json(&session);
    for corner in [pt(0.0, 0.0), pt(40.0, 40.0), pt(10.0, 30.0), pt(30.0, 10.0)] {
        click(&mut session, corner);
        let state = session.node_toolbar_state();
        assert!(!state.can_join && !state.can_split, "{corner:?}");
        session.join_selected();
        session.split_selected();
        session.delete_selected();
    }
    assert_eq!(json(&session), before, "nothing changed");
}

/// The ordinary path in the same document is node-editable as before.
#[test]
fn an_ordinary_path_next_to_a_compound_one_is_still_node_editable() {
    let mut session = session();
    session.set_tool(Tool::Node);
    let before = json(&session);
    // Drag the middle node (140, 100) of the open path.
    session.pointer_hover(pt(140.0, 100.0), false, false);
    session.pointer_down(pt(140.0, 100.0), false);
    session.pointer_up(pt(150.0, 100.0), false, false);
    assert_ne!(json(&session), before);
}

/// Criterion 38: the compound path stays in the Select tool on a double-click.
#[test]
fn a_double_click_on_a_compound_path_does_not_enter_the_node_tool() {
    let mut session = session();
    session.set_tool(Tool::Select);
    let at = pt(10.0, 15.0);
    click(&mut session, at);
    let hint = session.double_click(at, false, false);
    assert!(!hint, "the edit hint is for primitives");
    assert_eq!(session.tool(), Tool::Select, "the tool does not change");
}
