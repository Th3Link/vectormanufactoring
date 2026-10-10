//! `Session`-level checks of the eyedropper (`specs/0017-style-panel-rework`
//! criteria 22 to 27): a press takes the stored colour of the topmost painted
//! object and writes it to the style scope in one commit, a press where nothing
//! is painted keeps picking, and every cancel writes nothing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{Color, Document, ObjectSnapshot, Point, Style, unpack};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{PaintTarget, StyleField};

/// Two 20 mm squares: the first at x 0..20, the second at x 60..80, the second
/// drawn last (and selected by the draw).
fn two_squares() -> Session {
    let mut session = Session::new(1);
    for k in 0..2 {
        session.set_tool(Tool::Rectangle);
        let x = f64::from(k) * 60.0;
        session.pointer_down(Point::new(x, 0.0), false);
        session.pointer_up(Point::new(x + 20.0, 20.0), false, false);
    }
    // A None background: the empty page is a miss for the eyedropper, as it was before `0040`
    // (a Solid one is picked: see `document_background_session.rs`).
    session.set_background_paint(curvyo_document_core::BackgroundPaint::None);
    session
}

fn styles(session: &Session) -> Vec<Style> {
    let document: Document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .map(|object| match object {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn select(session: &mut Session, x: f64) {
    session.set_tool(Tool::Select);
    let edge = Point::new(x, 10.0);
    session.pointer_hover(edge, false, false);
    session.pointer_down(edge, false);
    session.pointer_up(edge, false, false);
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

/// The first square: a translucent blue fill and a red stroke.
fn paint_the_first(session: &mut Session) {
    select(session, 0.0);
    session.set_fill_paint(true);
    session
        .set_style_text(StyleField::FillColor, "#0000FF80")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#FF0000")
        .unwrap();
}

#[test]
fn a_click_on_a_fill_writes_its_colour_with_alpha_to_the_selection_in_one_commit() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    assert_eq!(session.colour_pick_target(), Some(PaintTarget::Stroke));
    click(&mut session, Point::new(10.0, 10.0));
    let second = &styles(&session)[1];
    assert_eq!(second.stroke.color, Color { r: 0, g: 0, b: 255 });
    assert_eq!(second.stroke.opacity.get(), 128.0 / 255.0);
    assert_eq!(session.colour_pick_target(), None, "picking ends");
    // The press selected nothing and moved nothing.
    assert_eq!(session.selected_object_count(), 1);
}

#[test]
fn a_click_on_a_stroke_picks_the_stroke_colour() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Fill);
    click(&mut session, Point::new(0.1, 10.0));
    let second = &styles(&session)[1];
    assert_eq!(second.fill.color, Color { r: 255, g: 0, b: 0 });
    assert!(!second.fill.enabled, "a fill pick never turns the fill on");
}

#[test]
fn a_stroke_pick_turns_an_off_stroke_on() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    session.set_stroke_paint(false);
    session.begin_colour_pick(PaintTarget::Stroke);
    click(&mut session, Point::new(10.0, 10.0));
    assert!(styles(&session)[1].stroke.enabled);
}

#[test]
fn a_click_where_nothing_is_painted_writes_nothing_and_keeps_picking() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    let before = styles(&session);
    session.begin_colour_pick(PaintTarget::Stroke);
    click(&mut session, Point::new(40.0, 10.0));
    assert_eq!(styles(&session), before);
    assert_eq!(session.colour_pick_target(), Some(PaintTarget::Stroke));
    // The unfilled inside of the second square has no paint either.
    click(&mut session, Point::new(70.0, 10.0));
    assert_eq!(styles(&session), before);
    assert_eq!(session.colour_pick_target(), Some(PaintTarget::Stroke));
}

#[test]
fn the_selected_object_can_be_picked_itself() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 0.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    click(&mut session, Point::new(10.0, 10.0));
    let first = &styles(&session)[0];
    assert_eq!(first.stroke.color, Color { r: 0, g: 0, b: 255 });
}

#[test]
fn every_cancel_writes_nothing() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    let before = styles(&session);

    session.begin_colour_pick(PaintTarget::Stroke);
    session.escape();
    assert_eq!(session.colour_pick_target(), None, "Escape");

    session.begin_colour_pick(PaintTarget::Stroke);
    session.set_tool(Tool::Rectangle);
    assert_eq!(session.colour_pick_target(), None, "a tool change");

    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    session.begin_colour_pick(PaintTarget::Stroke);
    assert_eq!(session.colour_pick_target(), None, "the button again");

    session.begin_colour_pick(PaintTarget::Fill);
    session.end_colour_pick();
    assert_eq!(session.colour_pick_target(), None, "the host ends it");
    assert_eq!(styles(&session), before);
}

#[test]
fn escape_while_picking_does_not_clear_the_selection() {
    let mut session = two_squares();
    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    session.escape();
    assert_eq!(session.selected_object_count(), 1);
}

#[test]
fn the_hover_reports_what_a_click_would_take() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    session.pointer_hover(Point::new(10.0, 10.0), false, false);
    assert_eq!(
        session.colour_pick_hover(),
        Some(("#0000FF80".to_string(), PaintTarget::Fill))
    );
    session.pointer_hover(Point::new(40.0, 10.0), false, false);
    assert_eq!(session.colour_pick_hover(), None, "no paint here");
    assert_eq!(session.cursor_hint(), "eyedropper");
    session.end_colour_pick();
    assert_eq!(session.colour_pick_hover(), None);
    assert_ne!(session.cursor_hint(), "eyedropper");
}

/// A pan moves the document under a still pointer: what a click would take is
/// read again for the point the pointer is now over (UX review D5).
#[test]
fn a_pan_or_a_drag_pan_refreshes_the_hover_under_a_still_pointer() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    session.begin_colour_pick(PaintTarget::Stroke);
    let over_first = (0..160)
        .flat_map(|y| (0..160).map(move |x| (f64::from(x) * 5.0, f64::from(y) * 5.0)))
        .find(|&(x, y)| {
            let p = session.screen_to_document(x, y);
            (2.0..18.0).contains(&p.x) && (2.0..18.0).contains(&p.y)
        })
        .expect("the first square is on screen");
    let point = session.screen_to_document(over_first.0, over_first.1);
    session.pointer_hover(point, false, false);
    assert!(session.colour_pick_hover().is_some());
    session.wheel(0.0, 5000.0, over_first.0, over_first.1, false, false);
    assert_eq!(session.colour_pick_hover(), None, "the square moved away");
    session.wheel(0.0, -5000.0, over_first.0, over_first.1, false, false);
    assert!(session.colour_pick_hover().is_some(), "and came back");
    // A drag pan keeps its anchor under the cursor: the pointer is then over
    // the anchor's document point, which here holds no paint.
    let (ex, ey) = (over_first.0 + 200.0, over_first.1);
    assert!(
        session.screen_to_document(ex, ey).x > 20.0,
        "empty ground to the right"
    );
    session.begin_pan(ex, ey);
    session.pan_to(ex + 7.0, ey + 3.0);
    assert_eq!(
        session.colour_pick_hover(),
        None,
        "a drag pan refreshes too"
    );
    session.end_pan();
}

#[test]
fn picking_shows_in_the_panel_view() {
    let mut session = two_squares();
    select(&mut session, 60.0);
    assert_eq!(session.style_panel_view().pick_target, "");
    session.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(session.style_panel_view().pick_target, "fill");
}

#[test]
fn a_pick_writes_one_commit() {
    let mut session = two_squares();
    paint_the_first(&mut session);
    select(&mut session, 60.0);
    let loro = |session: &Session| {
        let document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
        let doc = loro::LoroDoc::new();
        doc.import(&document.export_loro_snapshot().unwrap())
            .unwrap();
        doc.oplog_vv().values().copied().sum::<i32>()
    };
    let ops = loro(&session);
    session.begin_colour_pick(PaintTarget::Stroke);
    click(&mut session, Point::new(10.0, 10.0));
    // StrokeRgba writes colour, opacity and the on state, each at most once.
    assert!(loro(&session) - ops <= 3);
    assert!(loro(&session) > ops);
}
