//! A shape created with the Rectangle, Ellipse or Polygon/Star tool becomes
//! the selected object (`specs/0003-primitive-shapes/specification.md`,
//! acceptance criteria 1, 7, 11), so its handles and bounding box show.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{Document, Point};
use curvyo_editor_wasm::{Session, Tool};

const SHAPE_TOOLS: [Tool; 3] = [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar];

fn document_of(session: &Session) -> Document {
    let bytes = session.pack("0.1.0").expect("pack");
    curvyo_document_core::unpack(99, &bytes).expect("unpack")
}

/// The same document and tool, but with nothing selected.
fn unselected_copy(session: &Session, tool: Tool) -> Session {
    let bytes = session.pack("0.1.0").expect("pack");
    let mut copy = Session::open(1, &bytes).expect("open");
    copy.set_tool(tool);
    copy
}

fn drag(session: &mut Session, tool: Tool, from: Point, to: Point) {
    session.set_tool(tool);
    session.pointer_down(from, false);
    session.pointer_up(to, false, false);
}

#[test]
fn created_shape_is_selected_and_shows_decorations() {
    for tool in SHAPE_TOOLS {
        let mut session = Session::new(1);
        drag(
            &mut session,
            tool,
            Point::new(10.0, 10.0),
            Point::new(60.0, 40.0),
        );

        let baseline = unselected_copy(&session, tool).draw_list().triangle_count();
        assert!(
            session.draw_list().triangle_count() > baseline,
            "{tool:?}: new shape shows its selection box/handles"
        );

        session.set_tool(Tool::Select);
        session.delete_selected();
        assert!(
            document_of(&session).object_ids().is_empty(),
            "{tool:?}: the new shape was the selected object"
        );
    }
}

#[test]
fn creating_a_second_shape_moves_the_selection_to_it() {
    for tool in SHAPE_TOOLS {
        let mut session = Session::new(1);
        drag(
            &mut session,
            tool,
            Point::new(10.0, 10.0),
            Point::new(60.0, 40.0),
        );
        drag(
            &mut session,
            tool,
            Point::new(200.0, 200.0),
            Point::new(260.0, 240.0),
        );
        let second = document_of(&session).object_ids()[1];

        session.set_tool(Tool::Select);
        session.delete_selected();
        let remaining = document_of(&session).object_ids();
        assert_eq!(remaining.len(), 1, "{tool:?}: only one shape deleted");
        assert_ne!(
            remaining[0], second,
            "{tool:?}: the second shape was deleted"
        );
    }
}

#[test]
fn cancelled_or_zero_size_creates_select_nothing() {
    for tool in SHAPE_TOOLS {
        let mut session = Session::new(1);
        drag(
            &mut session,
            tool,
            Point::new(10.0, 10.0),
            Point::new(60.0, 40.0),
        );
        // Clear the selection by selecting nothing under the Select tool.
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(500.0, 500.0), false);
        session.pointer_up(Point::new(500.0, 500.0), false, false);
        session.set_tool(tool);

        // Escape-cancelled drag.
        session.pointer_down(Point::new(100.0, 100.0), false);
        session.escape();
        session.pointer_up(Point::new(150.0, 150.0), false, false);
        // Plain click with no movement.
        session.pointer_down(Point::new(300.0, 300.0), false);
        session.pointer_up(Point::new(300.0, 300.0), false, false);

        assert_eq!(document_of(&session).object_ids().len(), 1, "{tool:?}");
        session.set_tool(Tool::Select);
        session.delete_selected();
        assert_eq!(
            document_of(&session).object_ids().len(),
            1,
            "{tool:?}: nothing was selected, so nothing was deleted"
        );
    }
}
