//! Probe for the implementer's note of `0016-boolean-operations` PR 3: "drawing a second
//! rectangle by drag sometimes lost the first one". Draws two rectangles with the Rectangle tool
//! by many pointer sequences and asserts that the first one is still there, unchanged, after
//! the second is drawn. Uses only API that exists on `main`, so it can run there too.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines
)]

use curvyo_document_core::{
    Document, NodeId, ObjectSnapshot, Point, RectBounds, Shape, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn fresh() -> Session {
    let mut s = Session::new(2);
    s.resize_viewport(1200.0, 800.0);
    s.set_tool(Tool::Rectangle);
    s
}

fn doc(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn rects(s: &Session) -> Vec<(NodeId, RectBounds)> {
    let d = doc(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id)? {
            ObjectSnapshot::Primitive(p) => match p.shape {
                Shape::Rect { bounds, .. } => Some((id, bounds)),
                _ => None,
            },
            ObjectSnapshot::Path(_) => None,
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Style {
    /// hover, down, hover, up (what the browser sends).
    Full,
    /// down, up, no hover events at all.
    NoHover,
    /// down, one move, up at the move point.
    OneMove,
    /// down, several moves, up elsewhere than the last move.
    UpElsewhere,
    /// a press and release at one point (a click).
    Click,
}

/// Presses `R` first (the maker picks the Rectangle tool again; a created shape returns the
/// session to the Select tool, `0003`/`0011` criterion 28), then draws.
fn draw(s: &mut Session, style: Style, a: Point, b: Point, shift: bool, ctrl: bool) {
    s.set_tool(Tool::Rectangle);
    match style {
        Style::Full => {
            s.pointer_hover(a, shift, false);
            s.pointer_down(a, shift);
            s.pointer_hover(
                pt(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y)),
                shift,
                ctrl,
            );
            s.pointer_hover(b, shift, ctrl);
            s.pointer_up(b, shift, ctrl);
        }
        Style::NoHover => {
            s.pointer_down(a, shift);
            s.pointer_up(b, shift, ctrl);
        }
        Style::OneMove => {
            s.pointer_down(a, shift);
            s.pointer_hover(b, shift, ctrl);
            s.pointer_up(b, shift, ctrl);
        }
        Style::UpElsewhere => {
            s.pointer_down(a, shift);
            s.pointer_hover(pt(b.x - 5.0, b.y - 5.0), shift, ctrl);
            s.pointer_up(b, shift, ctrl);
        }
        Style::Click => {
            s.pointer_down(a, shift);
            s.pointer_up(a, shift, false);
        }
    }
}

const STYLES: [Style; 4] = [
    Style::Full,
    Style::NoHover,
    Style::OneMove,
    Style::UpElsewhere,
];

/// Every combination of pointer sequence for the first and the second rectangle, second one far
/// away from the first.
#[test]
fn two_separate_rectangles_exist_for_every_pointer_sequence() {
    for first in STYLES {
        for second in STYLES {
            for (shift1, shift2, ctrl2) in [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (false, false, true),
                (true, true, true),
            ] {
                let mut s = fresh();
                draw(&mut s, first, pt(10.0, 10.0), pt(50.0, 40.0), shift1, false);
                let before = rects(&s);
                assert_eq!(before.len(), 1, "{first:?} {shift1}: first rectangle");
                draw(
                    &mut s,
                    second,
                    pt(100.0, 100.0),
                    pt(160.0, 150.0),
                    shift2,
                    ctrl2,
                );
                let after = rects(&s);
                assert_eq!(
                    after.len(),
                    2,
                    "{first:?}/{second:?} shift {shift1}/{shift2} ctrl {ctrl2}: lost one"
                );
                assert!(
                    after
                        .iter()
                        .any(|(id, b)| *id == before[0].0 && *b == before[0].1),
                    "{first:?}/{second:?}: the first rectangle changed"
                );
            }
        }
    }
}

/// The second drag starts inside, on the edge of, on the corner of, and just outside the first
/// (the first is selected after being drawn, with its handles visible).
#[test]
fn a_second_drag_that_starts_on_the_first_rectangle_still_draws_a_new_one() {
    let starts = [
        pt(30.0, 25.0), // centre
        pt(10.0, 10.0), // corner
        pt(50.0, 40.0), // opposite corner
        pt(30.0, 10.0), // edge midpoint
        pt(10.0, 25.0), // edge midpoint
        pt(12.0, 12.0), // just inside
        pt(8.0, 8.0),   // just outside, in handle reach
        pt(52.0, 42.0), // just outside
    ];
    for style in STYLES {
        for start in starts {
            let mut s = fresh();
            draw(
                &mut s,
                Style::Full,
                pt(10.0, 10.0),
                pt(50.0, 40.0),
                false,
                false,
            );
            let before = rects(&s);
            draw(
                &mut s,
                style,
                start,
                pt(start.x + 70.0, start.y + 60.0),
                false,
                false,
            );
            let after = rects(&s);
            assert_eq!(after.len(), 2, "{style:?} from {start:?}: {after:?}");
            assert!(
                after
                    .iter()
                    .any(|(id, b)| *id == before[0].0 && *b == before[0].1),
                "{style:?} from {start:?}: the first rectangle changed: {after:?}"
            );
        }
    }
}

/// A second rectangle drawn in the opposite direction, overlapping the first.
#[test]
fn overlapping_and_reversed_drags() {
    for (a, b) in [
        (pt(40.0, 35.0), pt(0.0, 0.0)),
        (pt(100.0, 5.0), pt(20.0, 60.0)),
        (pt(5.0, 100.0), pt(60.0, 20.0)),
    ] {
        let mut s = fresh();
        draw(
            &mut s,
            Style::Full,
            pt(10.0, 10.0),
            pt(50.0, 40.0),
            false,
            false,
        );
        let before = rects(&s);
        draw(&mut s, Style::Full, a, b, false, false);
        let after = rects(&s);
        assert_eq!(after.len(), 2, "{a:?}->{b:?}: {after:?}");
        assert!(
            after
                .iter()
                .any(|(id, bb)| *id == before[0].0 && *bb == before[0].1)
        );
    }
}

/// Release outside the viewport, press without a move, a stray release, a cancelled drag and
/// Escape: none of them may remove the first rectangle.
#[test]
fn odd_pointer_sequences_never_lose_the_first_rectangle() {
    let outside = [pt(-500.0, -500.0), pt(5000.0, 5000.0), pt(-1.0, 300.0)];
    for out in outside {
        let mut s = fresh();
        draw(
            &mut s,
            Style::Full,
            pt(10.0, 10.0),
            pt(50.0, 40.0),
            false,
            false,
        );
        let before = rects(&s);
        s.pointer_hover(pt(100.0, 100.0), false, false);
        s.pointer_down(pt(100.0, 100.0), false);
        s.pointer_hover(out, false, false);
        s.pointer_up(out, false, false);
        let after = rects(&s);
        assert!(
            after
                .iter()
                .any(|(id, b)| *id == before[0].0 && *b == before[0].1),
            "release at {out:?}: {after:?}"
        );
        assert!(after.len() <= 2);
        // and a third drag still works
        draw(
            &mut s,
            Style::Full,
            pt(300.0, 300.0),
            pt(340.0, 330.0),
            false,
            false,
        );
        let third = rects(&s);
        assert!(
            third.iter().any(|(id, _)| *id == before[0].0),
            "after a third drag: {third:?}"
        );
    }

    // press without a move and release elsewhere is a drag; press and release at one place is a click
    let mut s = fresh();
    draw(
        &mut s,
        Style::Full,
        pt(10.0, 10.0),
        pt(50.0, 40.0),
        false,
        false,
    );
    let before = rects(&s);
    draw(
        &mut s,
        Style::Click,
        pt(100.0, 100.0),
        pt(100.0, 100.0),
        false,
        false,
    );
    let after = rects(&s);
    assert!(
        after
            .iter()
            .any(|(id, b)| *id == before[0].0 && *b == before[0].1)
    );
    draw(
        &mut s,
        Style::Full,
        pt(200.0, 200.0),
        pt(240.0, 230.0),
        false,
        false,
    );
    assert!(rects(&s).iter().any(|(id, _)| *id == before[0].0));

    // stray release, double release, cancelled drag, Escape mid-drag
    let mut s = fresh();
    draw(
        &mut s,
        Style::Full,
        pt(10.0, 10.0),
        pt(50.0, 40.0),
        false,
        false,
    );
    let before = rects(&s);
    s.pointer_up(pt(90.0, 90.0), false, false);
    s.pointer_up(pt(90.0, 90.0), false, false);
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_hover(pt(150.0, 150.0), false, false);
    s.pointer_cancelled();
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_hover(pt(150.0, 150.0), false, false);
    let _ = s.escape();
    s.pointer_up(pt(150.0, 150.0), false, false);
    let after = rects(&s);
    assert!(
        after
            .iter()
            .any(|(id, b)| *id == before[0].0 && *b == before[0].1),
        "{after:?}"
    );
    draw(
        &mut s,
        Style::Full,
        pt(300.0, 300.0),
        pt(340.0, 330.0),
        false,
        false,
    );
    assert!(rects(&s).iter().any(|(id, _)| *id == before[0].0));
}

/// The same drawing in a session that was opened from a saved file (as the app does), then the
/// tool changed to Select and back to Rectangle between the two drags.
#[test]
fn drawing_after_reopen_and_tool_switches() {
    let mut s = fresh();
    draw(
        &mut s,
        Style::Full,
        pt(10.0, 10.0),
        pt(50.0, 40.0),
        false,
        false,
    );
    let bytes = s.pack("0.1.0").unwrap();
    let mut s = Session::open(3, &bytes).unwrap();
    s.resize_viewport(1200.0, 800.0);
    let before = rects(&s);
    for between in [
        None,
        Some(Tool::Select),
        Some(Tool::Node),
        Some(Tool::Ellipse),
    ] {
        let mut t = Session::open(4, &bytes).unwrap();
        t.resize_viewport(1200.0, 800.0);
        t.set_tool(Tool::Rectangle);
        if let Some(tool) = between {
            t.set_tool(tool);
            t.set_tool(Tool::Rectangle);
        }
        draw(
            &mut t,
            Style::Full,
            pt(100.0, 100.0),
            pt(160.0, 150.0),
            false,
            false,
        );
        let after = rects(&t);
        assert_eq!(after.len(), 2, "{between:?}");
        assert!(
            after
                .iter()
                .any(|(id, b)| *id == before[0].0 && *b == before[0].1)
        );
    }
    let _ = pack;
}

/// What the implementer saw: a drag without picking the Rectangle tool again is a Select-tool
/// gesture. After the first rectangle the tool is Select (by design, `shape_pointer_up`); a
/// second drag starting on the first one MOVES it, so only one rectangle exists. This is the
/// documented behaviour, not a lost object, and it is identical on `main`.
#[test]
fn without_picking_the_tool_again_the_second_drag_is_a_select_gesture() {
    let mut s = fresh();
    draw(
        &mut s,
        Style::Full,
        pt(10.0, 10.0),
        pt(50.0, 40.0),
        false,
        false,
    );
    assert_eq!(s.tool(), Tool::Select, "a created shape returns to Select");
    let first = rects(&s)[0];
    s.pointer_hover(pt(30.0, 25.0), false, false);
    s.pointer_down(pt(30.0, 25.0), false);
    s.pointer_hover(pt(60.0, 55.0), false, false);
    s.pointer_up(pt(60.0, 55.0), false, false);
    let after = rects(&s);
    assert_eq!(after.len(), 1, "moved, not copied and not drawn");
    assert_eq!(after[0].0, first.0);
    assert_eq!(after[0].1.origin, pt(40.0, 40.0));
}
