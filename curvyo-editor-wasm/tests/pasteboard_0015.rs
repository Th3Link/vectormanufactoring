//! `specs/0015-document-size-and-rulers/` criteria 29 and 30 through `Session`:
//! every tool creates and edits an object that lies on the pasteboard, wholly
//! or partly, exactly as it does inside the document, and the object can be
//! selected and moved. No tool reads the document size.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{Document, ObjectSnapshot, Point, Shape, unpack};
use curvyo_editor_wasm::{Session, Tool};

const EPS: f64 = 1e-9;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn reopened(session: &Session) -> Document {
    unpack(99, &session.pack("0.1.0").unwrap()).unwrap()
}

fn drag(session: &mut Session, from: Point, to: Point) {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

fn rect_box(session: &Session) -> (f64, f64, f64, f64) {
    let document = reopened(session);
    let id = *document.object_ids().last().unwrap();
    let Some(ObjectSnapshot::Primitive(primitive)) = document.object(id) else {
        panic!("a primitive");
    };
    let Shape::Rect { bounds, .. } = primitive.shape else {
        panic!("a rect");
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

/// A rectangle wholly beyond the top-left corner, straddling the right edge
/// of the A4 document and wholly below it: created with the dragged corners.
#[test]
fn the_rectangle_tool_creates_on_the_pasteboard_without_clipping() {
    for (from, to, want) in [
        (
            pt(-60.0, -60.0),
            pt(-20.0, -20.0),
            (-60.0, -60.0, 40.0, 40.0),
        ),
        (pt(190.0, 40.0), pt(260.0, 90.0), (190.0, 40.0, 70.0, 50.0)),
        (pt(10.0, 400.0), pt(50.0, 500.0), (10.0, 400.0, 40.0, 100.0)),
    ] {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        drag(&mut session, from, to);
        let got = rect_box(&session);
        assert!(
            (got.0 - want.0).abs() < EPS
                && (got.1 - want.1).abs() < EPS
                && (got.2 - want.2).abs() < EPS
                && (got.3 - want.3).abs() < EPS,
            "{got:?} != {want:?}"
        );
    }
}

#[test]
fn the_ellipse_and_polygon_tools_create_on_the_pasteboard() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Ellipse);
    drag(&mut session, pt(-100.0, -100.0), pt(-40.0, -60.0));
    session.set_tool(Tool::PolygonStar);
    drag(&mut session, pt(500.0, 500.0), pt(530.0, 500.0));
    let document = reopened(&session);
    assert_eq!(document.object_ids().len(), 2);
}

#[test]
fn the_pen_places_nodes_on_the_pasteboard_and_commits_the_path_as_drawn() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    for point in [pt(-30.0, -30.0), pt(400.0, 500.0), pt(-30.0, 600.0)] {
        session.pointer_hover(point, false, false);
        session.pointer_down(point, false);
        session.pointer_up(point, false, false);
    }
    session.finish_pen();
    let document = reopened(&session);
    let id = *document.object_ids().last().unwrap();
    let path = document.path(id).unwrap();
    let points: Vec<_> = path
        .anchors
        .iter()
        .map(|a| (a.point.x, a.point.y))
        .collect();
    assert_eq!(points, [(-30.0, -30.0), (400.0, 500.0), (-30.0, 600.0)]);
}

/// Criterion 30: an object on the pasteboard is selected by a click on its
/// outline and moved by a drag, like any other.
#[test]
fn an_object_on_the_pasteboard_can_be_selected_and_moved() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Rectangle);
    drag(&mut session, pt(-60.0, -60.0), pt(-20.0, -20.0));
    session.set_tool(Tool::Select);
    // Press on the top edge, away from its handles, and drag by (+10, +5).
    session.pointer_hover(pt(-50.0, -60.0), false, false);
    session.pointer_down(pt(-50.0, -60.0), false);
    session.pointer_hover(pt(-40.0, -55.0), false, false);
    session.pointer_up(pt(-40.0, -55.0), false, false);
    let got = rect_box(&session);
    assert!(
        (got.0 - -50.0).abs() < 1e-6 && (got.1 - -55.0).abs() < 1e-6 && (got.2 - 40.0).abs() < 1e-6,
        "{got:?}"
    );
}
