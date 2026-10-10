//! `Session`-level checks of `specs/0007-stroke-and-fill-styling` PR 2: the
//! draw list paints objects in tree order, a filled interior is hovered,
//! pressed and double-clicked, and the hover follows the press (criteria 23,
//! 26 to 29, 41).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Document, EllipseFrame, Length, NewAnchor, NodeId, Point, RectBounds, StyleEdit, pack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::{DrawList, RgbaColor};

const ACCENT: RgbaColor = RgbaColor::opaque(0x2F, 0x6F, 0xEE);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn square(d: &Document, x: f64, y: f64, size: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(size),
        height: mm(size),
    })
}

fn circle(d: &Document, x: f64, y: f64, r: f64) -> NodeId {
    d.create_ellipse(EllipseFrame {
        center: pt(x, y),
        rx: mm(r),
        ry: mm(r),
    })
}

fn fill(d: &Document, id: NodeId) {
    d.edit_style(&[id], &StyleEdit::FillEnabled(true)).unwrap();
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

/// Whether the draw list holds the hover box (`--accent` at 65%).
fn hover_box_shown(list: &DrawList) -> bool {
    let hover = RgbaColor { a: 166, ..ACCENT };
    list.triangles.iter().any(|v| v.color == hover)
}

#[test]
fn ac26_a_path_above_a_rectangle_in_the_tree_is_painted_over_it() {
    let d = Document::new(1);
    let rect = square(&d, 0.0, 0.0, 50.0);
    fill(&d, rect);
    d.edit_style(
        &[rect],
        &StyleEdit::FillColor(curvyo_document_core::Color { r: 255, g: 0, b: 0 }),
    )
    .unwrap();
    let path = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 25.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 25.0)),
        ],
        false,
    );
    d.edit_style(&[path], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    let s = open(&d);
    let list = s.draw_list();
    // Fill and stroke of the rectangle, then the path's stroke.
    assert_eq!(list.layers().len(), 3);
    let red_end = list.layers()[0];
    assert_eq!(
        list.triangles[0].color.r, 255,
        "the rectangle's fill is first"
    );
    assert!(
        list.triangles[red_end..list.layers()[2]]
            .iter()
            .any(|v| (v.position.y - 25.0).abs() < 2.0 && v.position.x > 9.0),
        "the path's stroke comes after everything of the rectangle"
    );
}

#[test]
fn ac23_ac28_hover_lights_a_filled_interior_and_not_an_unfilled_one() {
    let d = Document::new(1);
    let hollow = square(&d, 0.0, 0.0, 40.0);
    let filled = square(&d, 100.0, 0.0, 40.0);
    fill(&d, filled);
    let _ = hollow;
    let mut s = open(&d);
    s.pointer_hover(pt(20.0, 20.0), false, false);
    assert!(
        !hover_box_shown(&s.draw_list()),
        "the inside of a hollow shape"
    );
    s.pointer_hover(pt(120.0, 20.0), false, false);
    assert!(hover_box_shown(&s.draw_list()), "inside a filled shape");
    s.pointer_hover(pt(300.0, 300.0), false, false);
    assert!(!hover_box_shown(&s.draw_list()), "on empty canvas");
}

/// Criterion 28: inside the sole selected box a press moves the selection, so
/// hover lights no other object, except a filled object above (criterion 29).
#[test]
fn ac28_ac29_hover_follows_the_press_inside_the_selected_box() {
    let d = Document::new(1);
    let big = square(&d, 0.0, 0.0, 100.0);
    fill(&d, big);
    let small = circle(&d, 30.0, 30.0, 5.0);
    fill(&d, small);
    let hollow = circle(&d, 70.0, 30.0, 5.0);
    let _ = hollow;
    let mut s = open(&d);
    // Select the big one by clicking away from the small ones.
    click(&mut s, pt(80.0, 80.0));
    assert_eq!(s.selected_object_count(), 1);
    s.pointer_hover(pt(60.0, 80.0), false, false);
    assert!(
        !hover_box_shown(&s.draw_list()),
        "a press here moves the selection"
    );
    s.pointer_hover(pt(30.0, 30.0), false, false);
    assert!(
        hover_box_shown(&s.draw_list()),
        "a filled object above is lit"
    );
    s.pointer_hover(pt(70.0, 30.0), false, false);
    assert!(
        !hover_box_shown(&s.draw_list()),
        "a hollow one above is not"
    );
}

#[test]
fn ac29_a_press_on_a_filled_object_above_the_selection_selects_it_and_a_drag_moves_it() {
    let d = Document::new(1);
    let big = square(&d, 0.0, 0.0, 100.0);
    fill(&d, big);
    let small = circle(&d, 30.0, 30.0, 5.0);
    fill(&d, small);
    let mut s = open(&d);
    click(&mut s, pt(80.0, 80.0));
    s.pointer_hover(pt(30.0, 30.0), false, false);
    s.pointer_down(pt(30.0, 30.0), false);
    s.pointer_hover(pt(40.0, 30.0), false, false);
    s.pointer_hover(pt(50.0, 30.0), false, false);
    s.pointer_up(pt(50.0, 30.0), false, false);
    let doc = Session::open(3, &s.pack("0.1.0").unwrap()).unwrap();
    // The small circle moved by 20 mm; the big square did not.
    let moved = doc.draw_list();
    let min_x = |list: &DrawList| {
        list.triangles
            .iter()
            .map(|v| v.position.x)
            .fold(f64::MAX, f64::min)
    };
    assert!(min_x(&moved) < 1.0, "the big square is where it was");
    let after = doc.draw_list();
    let right = after
        .triangles
        .iter()
        .filter(|v| v.position.x > 40.0 && v.position.x < 60.0 && v.position.y < 40.0)
        .count();
    assert!(right > 0, "the circle's stroke now lies around x = 50");
}

#[test]
fn ac28_a_double_click_inside_a_filled_path_hands_it_to_the_node_tool() {
    let d = Document::new(1);
    let path = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(60.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(60.0, 60.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(0.0, 60.0)),
        ],
        true,
    );
    let mut s = open(&d);
    s.pointer_hover(pt(30.0, 30.0), false, false);
    s.double_click(pt(30.0, 30.0), false, false);
    assert_eq!(s.tool(), Tool::Select, "an unfilled interior is not hit");
    fill(&d, path);
    let mut s = open(&d);
    s.pointer_hover(pt(30.0, 30.0), false, false);
    s.double_click(pt(30.0, 30.0), false, false);
    assert_eq!(s.tool(), Tool::Node, "a filled interior is");
}

#[test]
fn artwork_stays_in_layers_below_the_editor_overlay() {
    let d = Document::new(1);
    let id = square(&d, 0.0, 0.0, 40.0);
    fill(&d, id);
    let mut s = open(&d);
    click(&mut s, pt(20.0, 20.0));
    let list = s.draw_list();
    assert_eq!(list.layers().len(), 2, "fill and stroke");
    assert!(
        list.triangles.len() > list.overlay_start(),
        "the selection box and handles are the overlay"
    );
}
