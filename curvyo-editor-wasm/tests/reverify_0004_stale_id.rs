//! Re-verification scenario for the architect's stale-id finding on
//! `specs/0004-canvas-navigation-and-selection`: select a path with the
//! Select tool, switch to Node and delete it down to nothing, switch
//! back to Select, shift-click a different, still-live object, and drag
//! it. Before the `retain_existing` fix this refused the whole
//! `translate_objects` batch (the survivor snapped back); confirms it
//! now moves.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{Document, Point, Shape};
use curvyo_editor_wasm::{Session, Tool};

fn rect_origin(document: &Document, id: curvyo_document_core::NodeId) -> Point {
    let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
        panic!("expected rect");
    };
    bounds.origin
}

fn document_of(session: &Session) -> Document {
    let bytes = session.pack("0.1.0").expect("pack");
    curvyo_document_core::unpack(99, &bytes).expect("unpack")
}

#[test]
fn stale_selected_id_from_a_node_tool_delete_no_longer_blocks_moving_the_survivor() {
    let mut session = Session::new(1);

    // A two-anchor open path (object A) and a rect elsewhere (object B).
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    session.pointer_down(Point::new(10.0, 0.0), false);
    session.pointer_up(Point::new(10.0, 0.0), false, false);
    session.finish_pen();

    session.set_tool(Tool::Rectangle);
    session.pointer_down(Point::new(100.0, 100.0), false);
    session.pointer_up(Point::new(110.0, 110.0), false, false);

    let document = document_of(&session);
    let ids = document.object_ids();
    assert_eq!(ids.len(), 2, "path + rect");
    let rect_id = ids
        .iter()
        .copied()
        .find(|&id| document.primitive(id).is_some())
        .expect("the rect object");

    // Select the path with the Select tool.
    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(5.0, 0.0), false); // somewhere on the path's segment
    session.pointer_up(Point::new(5.0, 0.0), false, false);

    // Switch to Node, select one of the path's two anchors, delete it —
    // `delete_anchors` drops the whole object once fewer than 2 anchors
    // remain, so the path disappears entirely.
    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.delete_selected();

    let document_after_delete = document_of(&session);
    assert_eq!(
        document_after_delete.object_ids().len(),
        1,
        "the path must be gone, only the rect remains"
    );

    // Back to Select. The Select tool's own selection still names the
    // now-deleted path (Node tool's delete never touched it). Shift-
    // click the still-live rect, then drag it.
    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(100.0, 100.0), true); // shift-click the rect
    session.pointer_hover(Point::new(103.0, 104.0), false, false);
    session.pointer_up(Point::new(103.0, 104.0), false, false);

    let document_final = document_of(&session);
    let after = rect_origin(&document_final, rect_id);
    assert_eq!(
        after,
        Point::new(103.0, 104.0),
        "the rect must have moved by the drag offset instead of the whole batch being refused"
    );
}
