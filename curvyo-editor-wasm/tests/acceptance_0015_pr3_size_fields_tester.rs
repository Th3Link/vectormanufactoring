//! Independent tester tests for PR 3 of `specs/0015-document-size-and-rulers`:
//! the typed Width and Height path through `Session::set_document_side` and
//! `side_text` (criteria 15 to 21, 35, 39), Fit outcomes, and the refusal of a
//! resize or fit while the Pen holds a path (14a). Expected values come from
//! the specification's arithmetic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::many_single_char_names)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    Angle, DisplayUnit, Document, Length, NodeId, ObjectSnapshot, Point, RectBounds,
    outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{DocumentSide, FitOutcome, Session, SizeOutcome, Tool};

use DocumentSide::{Height, Width};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    })
}

fn rotate(d: &Document, id: NodeId, about: Point, rad: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(rad)))
        .unwrap();
}

fn session_of(d: &Document) -> Session {
    Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap()
}

fn reopened(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn snapshot(s: &Session) -> Vec<u8> {
    reopened(s).export_loro_snapshot().unwrap()
}

fn size_mm(s: &Session) -> (f64, f64) {
    let size = reopened(s).size();
    (size.width.as_mm(), size.height.as_mm())
}

fn points(d: &Document) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for id in d.object_ids() {
        match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) => out.extend(p.anchors.iter().map(|a| (a.point.x, a.point.y))),
            ObjectSnapshot::Primitive(p) => out.extend(
                outline_of_rotated(&p.shape, p.rotation)
                    .iter()
                    .map(|a| (a.point.x, a.point.y)),
            ),
        }
    }
    out
}

fn screen_points(s: &Session) -> Vec<(f64, f64)> {
    points(&reopened(s))
        .iter()
        .map(|&(x, y)| s.view().document_to_screen(pt(x, y)))
        .collect()
}

fn labels(s: &Session) -> Vec<String> {
    let l = loro::LoroDoc::new();
    l.import(&snapshot(s)).unwrap();
    let ids: Vec<loro::ID> = l.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String)> = Vec::new();
    l.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|x| x.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, m)| m).collect()
}

fn changes(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&snapshot(s)).unwrap();
    l.len_changes()
}

fn click(s: &mut Session, x: f64, y: f64) {
    s.pointer_hover(pt(x, y), false, false);
    s.pointer_down(pt(x, y), false);
    s.pointer_up(pt(x, y), false, false);
}

fn one_rect_session() -> Session {
    let d = Document::new(1);
    rect(&d, 10.0, 10.0, 50.0, 50.0);
    let mut s = session_of(&d);
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    s
}

fn populated() -> Document {
    let d = Document::new(1);
    let r = rect(&d, 10.0, 10.0, 50.0, 50.0);
    rect(&d, -80.0, 500.0, 20.0, 5.0);
    rotate(&d, r, pt(20.0, 20.0), 0.7);
    d
}

// ---- criterion 17 ----

#[test]
fn ac17_the_specs_example_300_by_400() {
    let mut s = one_rect_session();
    assert_eq!(s.set_document_side(Width, "300"), SizeOutcome::Committed);
    assert_eq!(s.set_document_side(Height, "400"), SizeOutcome::Committed);
    assert_eq!(size_mm(&s), (300.0, 400.0));
    let p = points(&reopened(&s));
    assert_eq!(p[0], (55.0, 61.5));
    assert_eq!(p[2], (105.0, 111.5));
    assert_eq!(s.size_text(), "300.0 \u{d7} 400.0 mm");
}

#[test]
fn ac17_each_side_keeps_the_other_exactly() {
    let d = Document::new(1);
    let mut s = session_of(&d);
    assert_eq!(
        s.set_document_side(Height, "297.123456"),
        SizeOutcome::Committed
    );
    let (w, h) = size_mm(&s);
    assert_eq!((w, h), (210.0, 297.123_456));
    assert_eq!(s.set_document_side(Width, "100"), SizeOutcome::Committed);
    assert_eq!(size_mm(&s), (100.0, 297.123_456));
}

#[test]
fn ac17_18_a_smaller_document_scales_nothing_and_moves_content_to_the_pasteboard() {
    let mut s = one_rect_session();
    assert_eq!(s.set_document_side(Width, "20"), SizeOutcome::Committed);
    assert_eq!(s.set_document_side(Height, "20"), SizeOutcome::Committed);
    let p = points(&reopened(&s));
    // shift (-95, -138.5)
    assert!(
        (p[0].0 + 85.0).abs() < 1e-9 && (p[0].1 + 128.5).abs() < 1e-9,
        "{p:?}"
    );
    assert!((p[2].0 - p[0].0 - 50.0).abs() < 1e-9 && (p[2].1 - p[0].1 - 50.0).abs() < 1e-9);
}

// ---- criterion 19 ----

#[test]
fn ac19_one_commit_with_the_label_and_a_no_op_writes_nothing() {
    let mut s = one_rect_session();
    let n = changes(&s);
    assert_eq!(s.set_document_side(Width, "300"), SizeOutcome::Committed);
    assert_eq!(changes(&s), n + 1);
    assert_eq!(labels(&s).last().unwrap(), "resize_document");
    let stored = snapshot(&s);
    for same in ["300", " 300 ", "300,0", "300.000", "0300"] {
        assert_eq!(
            s.set_document_side(Width, same),
            SizeOutcome::Unchanged,
            "{same:?}"
        );
        assert_eq!(snapshot(&s), stored, "{same:?} wrote");
    }
}

// ---- criteria 15, 16: refusals write nothing and move nothing ----

#[test]
fn ac16_refused_text_changes_nothing_in_any_unit() {
    for unit in [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::In] {
        let mut s = one_rect_session();
        s.set_display_unit(unit);
        let stored = snapshot(&s);
        let view = s.view();
        let mut refused = vec![
            "", "  ", "abc", "21cm", "8.5in", "NaN", "inf", "-inf", "1e999", "0", "-5", "0.01",
            "100001", "1,2,3", "12 34",
        ];
        if unit == DisplayUnit::In {
            refused.push("4000");
        }
        for text in refused {
            for side in [Width, Height] {
                assert_eq!(
                    s.set_document_side(side, text),
                    SizeOutcome::Invalid,
                    "{unit:?} {text:?}"
                );
            }
        }
        assert_eq!(snapshot(&s), stored, "{unit:?}");
        assert_eq!(s.view(), view);
    }
}

#[test]
fn ac16_limits_commit_exactly() {
    let mut s = one_rect_session();
    assert_eq!(s.set_document_side(Width, "100000"), SizeOutcome::Committed);
    assert_eq!(s.set_document_side(Height, "1"), SizeOutcome::Committed);
    assert_eq!(size_mm(&s), (100_000.0, 1.0));
    assert_eq!(s.size_text(), "100000.0 \u{d7} 1.0 mm");
    assert_eq!(
        s.set_document_side(Width, "100000.0000000001"),
        SizeOutcome::Unchanged
    );
    assert_eq!(s.set_document_side(Width, "100001"), SizeOutcome::Invalid);
    assert_eq!(s.set_document_side(Height, "0.99"), SizeOutcome::Invalid);
    assert_eq!(s.side_message(), "Enter a number from 1 to 100000");
}

#[test]
fn ac16_messages_follow_the_unit() {
    let mut s = Session::new(1);
    s.set_display_unit(DisplayUnit::Cm);
    assert_eq!(s.side_message(), "Enter a number from 0.1 to 10000");
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(s.side_message(), "Enter a number from 0.04 to 3937");
}

// ---- criterion 35: inches ----

#[test]
fn ac35_letter_in_inches_is_exact_and_shows_back_as_typed() {
    let mut s = Session::new(1);
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(s.set_document_side(Width, "8.5"), SizeOutcome::Committed);
    assert_eq!(s.set_document_side(Height, "11"), SizeOutcome::Committed);
    assert_eq!(size_mm(&s), (215.9, 279.4));
    assert_eq!(s.side_text(Width), "8.5");
    assert_eq!(s.side_text(Height), "11");
    assert_eq!(s.size_text(), "8.500 \u{d7} 11.000 in");
    assert_eq!(s.set_document_side(Width, "8.5"), SizeOutcome::Unchanged);
    assert_eq!(size_mm(&s), (215.9, 279.4));
}

#[test]
fn ac35_side_text_per_unit_and_showing_never_rewrites_the_stored_value() {
    let mut s = Session::new(1);
    assert_eq!(s.side_text(Width), "210");
    assert_eq!(s.side_text(Height), "297");
    assert_eq!(
        s.set_document_side(Width, "123.4567"),
        SizeOutcome::Committed
    );
    assert_eq!(s.side_text(Width), "123.457");
    assert_eq!(size_mm(&s).0, 123.4567);
    s.set_display_unit(DisplayUnit::Cm);
    assert_eq!(s.side_text(Width), "12.3457");
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(s.side_text(Width), "4.8605");
    assert_eq!(
        size_mm(&s).0,
        123.4567,
        "a display change rewrote the value"
    );
    assert_eq!(s.side_text(Height), "11.6929");
}

#[test]
fn ac35_the_largest_size_types_back_in_inches() {
    let mut s = Session::new(1);
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(
        s.set_document_side(Width, "3937.0079"),
        SizeOutcome::Committed
    );
    assert_eq!(size_mm(&s).0, 100_000.0);
    assert_eq!(s.side_text(Width), "3937.0079");
    assert_eq!(
        s.set_document_side(Width, "3937.0079"),
        SizeOutcome::Unchanged
    );
    assert_eq!(s.set_document_side(Width, "3937.01"), SizeOutcome::Invalid);
}

#[test]
fn ac34_typed_values_are_read_in_the_display_unit() {
    let mut s = Session::new(1);
    s.set_display_unit(DisplayUnit::Cm);
    assert_eq!(s.set_document_side(Width, "21,5"), SizeOutcome::Committed);
    assert_eq!(size_mm(&s).0, 215.0);
    assert_eq!(s.side_text(Width), "21.5");
    assert_eq!(s.size_text(), "21.50 \u{d7} 29.70 cm");
}

// ---- criterion 20: nothing moves on screen ----

fn assert_still(a: &[(f64, f64)], b: &[(f64, f64)], what: &str) {
    assert_eq!(a.len(), b.len());
    for (p, q) in a.iter().zip(b) {
        assert!(
            (p.0 - q.0).abs() <= 1e-6 && (p.1 - q.1).abs() <= 1e-6,
            "{what}: {p:?} -> {q:?}"
        );
    }
}

#[test]
fn ac20_a_typed_resize_moves_no_object_on_screen() {
    for zoom_clicks in [-14, -5, 0, 4, 14i32] {
        for (w, h) in [("1234.5", "7"), ("1", "100000"), ("100000", "1")] {
            let mut s = session_of(&populated());
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
            for _ in 0..zoom_clicks.abs() {
                let dy = if zoom_clicks < 0 { 200.0 } else { -200.0 };
                s.wheel(0.0, dy, 400.0, 300.0, false, true);
            }
            let before = screen_points(&s);
            assert_eq!(s.set_document_side(Width, w), SizeOutcome::Committed);
            assert_still(&before, &screen_points(&s), "width");
            s.set_document_side(Height, h);
            assert_still(&before, &screen_points(&s), "height");
        }
    }
}

#[test]
fn ac20_refused_and_unchanged_input_leaves_the_view_alone() {
    let mut s = one_rect_session();
    let view = s.view();
    assert_eq!(s.set_document_side(Width, "210"), SizeOutcome::Unchanged);
    assert_eq!(s.set_document_side(Width, "x"), SizeOutcome::Invalid);
    assert_eq!(s.view(), view);
}

#[test]
fn ac20_a_window_resize_after_a_typed_resize_in_the_untouched_view_keeps_objects() {
    let mut s = one_rect_session();
    let before = screen_points(&s);
    s.set_document_side(Width, "500");
    s.resize_viewport(640.0, 480.0);
    s.resize_viewport(1300.0, 900.0);
    assert_still(&before, &screen_points(&s), "window resize");
}

// ---- repeated resizes do not drift ----

#[test]
fn two_hundred_alternating_resizes_return_the_content_to_where_it_was() {
    let mut s = session_of(&populated());
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    let p0 = points(&reopened(&s));
    let sc0 = screen_points(&s);
    for i in 0..200 {
        let (w, h) = match i % 4 {
            0 => ("100000", "100000"),
            1 => ("1", "1"),
            2 => ("7777.777", "0.5"),
            _ => ("210", "297"),
        };
        s.set_document_side(Width, w);
        s.set_document_side(Height, h);
    }
    assert_eq!(size_mm(&s), (210.0, 297.0));
    for (a, b) in p0.iter().zip(&points(&reopened(&s))) {
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
            "{a:?} {b:?}"
        );
    }
    assert_still(&sc0, &screen_points(&s), "after 200 resizes");
}

// ---- criterion 39 ----

#[test]
fn ac39_a_typed_resize_survives_save_close_open() {
    let mut s = session_of(&populated());
    s.set_display_unit(DisplayUnit::In);
    s.set_document_side(Width, "8.5");
    s.set_document_side(Height, "11");
    let pts = points(&reopened(&s));
    let bytes = s.pack("0.1.0").unwrap();
    drop(s);
    let again = Session::open(3, &bytes).unwrap();
    assert_eq!(size_mm(&again), (215.9, 279.4));
    assert_eq!(points(&reopened(&again)), pts);
    assert_eq!(again.side_text(Width), "8.5");
}

// ---- Fit outcomes ----

#[test]
fn fit_outcomes_empty_fitted_already_fits() {
    let mut s = Session::new(1);
    assert_eq!(s.fit_document(), FitOutcome::Empty);
    let mut s = one_rect_session();
    assert_eq!(s.fit_document(), FitOutcome::Fitted);
    assert_eq!(s.fit_document(), FitOutcome::AlreadyFits);
    assert_eq!(s.side_text(Width), "50");
    assert_eq!(s.side_text(Height), "50");
    // A typed resize breaks the fit; the next Fit fits again.
    s.set_document_side(Width, "300");
    assert_eq!(s.fit_document(), FitOutcome::Fitted);
    assert_eq!(size_mm(&s), (50.0, 50.0));
}

// ---- 14a: no resize or fit while the Pen holds a path ----

#[test]
fn ac14a_no_resize_or_fit_while_a_pen_path_exists() {
    let mut s = session_of(&populated());
    s.set_tool(Tool::Pen);
    click(&mut s, 300.0, 300.0);
    assert!(s.pen_in_progress().is_some());
    let stored = snapshot(&s);
    let view = s.view();
    for side in [Width, Height] {
        assert_ne!(s.set_document_side(side, "400"), SizeOutcome::Committed);
    }
    assert!(!matches!(
        s.fit_document(),
        FitOutcome::Fitted | FitOutcome::AlreadyFits | FitOutcome::TooLarge(_)
    ));
    assert_eq!(snapshot(&s), stored);
    assert_eq!(s.view(), view);
    s.escape();
    assert_eq!(s.set_document_side(Width, "400"), SizeOutcome::Committed);
}

#[test]
fn ac14a_leaving_the_pen_ends_the_path_so_a_resize_is_allowed_and_moves_nothing_hidden() {
    for other in [Tool::Select, Tool::Node, Tool::Rectangle, Tool::Ellipse] {
        let mut s = session_of(&populated());
        s.set_tool(Tool::Pen);
        click(&mut s, 300.0, 300.0);
        click(&mut s, 330.0, 340.0);
        s.set_tool(other);
        s.set_tool(Tool::Pen);
        assert!(s.pen_in_progress().is_none(), "{other:?}");
        s.set_tool(other);
        assert_eq!(
            s.set_document_side(Width, "400"),
            SizeOutcome::Committed,
            "{other:?}"
        );
    }
}
