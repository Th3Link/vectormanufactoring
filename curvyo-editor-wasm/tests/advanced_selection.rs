//! `Session`-level tests of `advanced-selection`
//! (`specs/advanced-selection/specification.md`): the 8 px Select-tool hit
//! area next to the Node tool's 4 px, the marquee and lasso through the
//! session's modifiers, the legend, the overlay, the cursor, the minus badge
//! and the Escape cascade. The gesture rules themselves are tested in
//! `curvyo-ui-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use curvyo_document_core::{AnchorId, Document, Length, NewAnchor, Point, RectBounds, pack};
use curvyo_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Three 10 mm squares in a row at x = 0, 20 and 40, and a horizontal line
/// far below them.
fn document() -> Document {
    let document = Document::new(1);
    for x in [0.0, 20.0, 40.0] {
        let _ = document.create_rect(RectBounds {
            origin: pt(x, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
    }
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 100.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 100.0)),
        ],
        false,
    );
    document
}

fn session() -> Session {
    let mut session = Session::open(2, &pack(&document(), "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

/// Screen pixels per millimetre.
fn k(session: &Session) -> f64 {
    session.view().scale()
}

/// The host's reaction to a key change: cached modifiers, then the re-sent
/// hover at `at`.
fn hold(session: &mut Session, at: Point, shift: bool, ctrl: bool, alt: bool) {
    session.modifiers_changed(shift, ctrl, alt);
    session.pointer_hover(at, shift, ctrl);
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

/// A press at `from` with the modifiers, a drag through the middle to `to`,
/// the release there with `release` modifiers.
fn drag(
    session: &mut Session,
    from: Point,
    to: Point,
    press: (bool, bool, bool),
    release: (bool, bool, bool),
) {
    hold(session, from, press.0, press.1, press.2);
    session.pointer_down(from, press.0);
    let middle = pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y));
    hold(session, middle, release.0, release.1, release.2);
    hold(session, to, release.0, release.1, release.2);
    session.pointer_up(to, release.0, release.1);
    hold(session, to, false, false, false);
}

const NONE: (bool, bool, bool) = (false, false, false);
const SHIFT: (bool, bool, bool) = (true, false, false);
const CTRL: (bool, bool, bool) = (false, true, false);
const ALT: (bool, bool, bool) = (false, false, true);

// ---- criteria 1 and 2: the hit area ----

/// Criterion 1: the Select tool hits an outline from 8 screen pixels.
#[test]
fn the_select_tool_hits_within_eight_pixels_of_an_outline() {
    for (px, hit) in [
        (1.0, true),
        (4.5, true),
        (7.5, true),
        (9.0, false),
        (14.0, false),
    ] {
        let mut s = session();
        let at = pt(5.0, -px / k(&s));
        click(&mut s, at);
        assert_eq!(s.selected_object_count(), usize::from(hit), "{px} px");
    }
}

/// Criterion 1: the hover box appears from the same radius.
#[test]
fn hover_uses_the_same_radius() {
    let mut s = session();
    s.pointer_hover(pt(300.0, 300.0), false, false);
    let idle = s.draw_list().triangle_count();
    s.pointer_hover(pt(25.0, -7.0 / k(&s)), false, false);
    assert!(s.draw_list().triangle_count() > idle, "7 px away lights it");
    s.pointer_hover(pt(25.0, -10.0 / k(&s)), false, false);
    assert_eq!(s.draw_list().triangle_count(), idle, "10 px away does not");
    // With Alt held a press would arm a lasso: nothing lights.
    let near = pt(25.0, -7.0 / k(&s));
    hold(&mut s, near, false, false, true);
    assert_eq!(s.draw_list().triangle_count(), idle, "Alt lights nothing");
}

/// Criterion 2: the Node tool's segment pick stays at 4 px.
#[test]
fn the_node_tool_keeps_four_pixels() {
    for (px, picked) in [(2.0, true), (3.5, true), (5.0, false), (7.5, false)] {
        let mut s = session();
        s.set_tool(Tool::Node);
        // Select the path first, then click the segment `px` from it.
        click_node_tool_on_path(&mut s);
        let at = pt(50.0, 100.0 + px / k(&s));
        s.pointer_hover(at, false, false);
        s.pointer_down(at, false);
        s.pointer_up(at, false, false);
        assert_eq!(s.node_toolbar_state().can_insert, picked, "{px} px");
    }
}

/// The Node tool picks a path by its segment; clicking a node of it first
/// makes the path the one being edited.
fn click_node_tool_on_path(s: &mut Session) {
    let at = pt(0.0, 100.0);
    s.pointer_hover(at, false, false);
    s.pointer_down(at, false);
    s.pointer_up(at, false, false);
}

// ---- the marquee ----

/// Criteria 9 to 11: direction and Alt choose the mode.
#[test]
fn direction_and_alt_choose_touch_or_contain() {
    let mut s = session();
    // Rightward: only the first square is fully inside.
    drag(&mut s, pt(-5.0, -5.0), pt(25.0, 15.0), NONE, NONE);
    assert_eq!(s.selected_object_count(), 1);
    // Leftward over the same area: touch takes the second as well.
    drag(&mut s, pt(25.0, 15.0), pt(-5.0, -5.0), NONE, NONE);
    assert_eq!(s.selected_object_count(), 2);
    // Alt at release inverts a rightward drag to touch.
    drag(&mut s, pt(-5.0, -5.0), pt(25.0, 15.0), NONE, ALT);
    assert_eq!(s.selected_object_count(), 2);
}

/// Criteria 12 and 13: Shift adds and Ctrl removes, at the release.
#[test]
fn shift_adds_and_ctrl_removes() {
    let mut s = session();
    drag(&mut s, pt(-5.0, -5.0), pt(15.0, 15.0), NONE, NONE);
    assert_eq!(s.selected_object_count(), 1);
    // Start well clear of the lone selected square's handles.
    drag(&mut s, pt(15.0, -20.0), pt(55.0, 15.0), SHIFT, SHIFT);
    assert_eq!(s.selected_object_count(), 3);
    drag(&mut s, pt(15.0, -20.0), pt(35.0, 15.0), CTRL, CTRL);
    assert_eq!(s.selected_object_count(), 2, "the second square left");
}

/// Criterion 8: a click on empty canvas clears, Shift and Ctrl keep.
#[test]
fn clicks_on_empty_canvas_clear_unless_shift_or_ctrl() {
    for (modifiers, left) in [(NONE, 0), (SHIFT, 1), (CTRL, 1)] {
        let mut s = session();
        drag(&mut s, pt(-5.0, -5.0), pt(15.0, 15.0), NONE, NONE);
        let at = pt(300.0, 300.0);
        hold(&mut s, at, modifiers.0, modifiers.1, modifiers.2);
        s.pointer_down(at, modifiers.0);
        s.pointer_up(at, modifiers.0, modifiers.1);
        assert_eq!(s.selected_object_count(), left, "{modifiers:?}");
    }
}

/// Criterion 14: the legend names mode and combine and follows the keys with
/// the pointer at rest.
#[test]
fn the_legend_follows_the_modifiers_live() {
    let mut s = session();
    let from = pt(-5.0, -5.0);
    let to = pt(60.0, 40.0);
    hold(&mut s, from, false, false, false);
    s.pointer_down(from, false);
    assert!(s.live_readout().is_none(), "inside the dead zone: a click");
    hold(&mut s, to, false, false, false);
    let legend = |s: &Session| s.live_readout().expect("a marquee runs").text;
    assert_eq!(legend(&s), "Contain \u{b7} Replace");
    assert_eq!(s.live_readout().unwrap().anchor, to, "at the pointer");
    hold(&mut s, to, true, false, false);
    assert_eq!(legend(&s), "Contain \u{b7} +Add");
    hold(&mut s, to, false, true, false);
    assert_eq!(legend(&s), "Contain \u{b7} \u{2212}Remove");
    hold(&mut s, to, true, true, false);
    assert_eq!(legend(&s), "Contain \u{b7} \u{2212}Remove", "Ctrl wins");
    hold(&mut s, to, false, false, true);
    assert_eq!(legend(&s), "Touch \u{b7} Replace", "Alt inverts the mode");
    hold(&mut s, pt(-30.0, 40.0), false, false, false);
    assert_eq!(legend(&s), "Touch \u{b7} Replace", "leftward is touch");
    s.pointer_up(pt(-30.0, 40.0), false, false);
    assert!(s.live_readout().is_none(), "gone after the release");
}

/// The lasso's legend says "line".
#[test]
fn the_lasso_legend_says_line() {
    let mut s = session();
    hold(&mut s, pt(100.0, 100.0), false, false, true);
    s.pointer_down(pt(100.0, 100.0), false);
    hold(&mut s, pt(130.0, 120.0), true, false, true);
    assert_eq!(s.live_readout().unwrap().text, "Touch (line) \u{b7} +Add");
    hold(&mut s, pt(130.0, 120.0), false, true, true);
    assert_eq!(
        s.live_readout().unwrap().text,
        "Touch (line) \u{b7} \u{2212}Remove"
    );
    s.pointer_up(pt(130.0, 120.0), false, true);
}

/// The box and the line are in the draw list while the drag runs: green for
/// touch, red for contain; nothing after the release.
#[test]
fn the_overlay_is_drawn_during_the_drag_only() {
    let count = |s: &Session, rgb: (u8, u8, u8)| {
        s.draw_list()
            .triangles
            .iter()
            .filter(|v| (v.color.r, v.color.g, v.color.b) == rgb)
            .count()
    };
    let green = (0x2F, 0xAE, 0x57);
    let red = (0xE5, 0x48, 0x4D);
    let mut s = session();
    assert_eq!((count(&s, green), count(&s, red)), (0, 0));
    hold(&mut s, pt(60.0, 40.0), false, false, false);
    s.pointer_down(pt(60.0, 40.0), false);
    hold(&mut s, pt(-5.0, -5.0), false, false, false);
    assert!(
        count(&s, green) > 0 && count(&s, red) == 0,
        "leftward: green"
    );
    hold(&mut s, pt(120.0, 80.0), false, false, false);
    assert!(
        count(&s, red) > 0 && count(&s, green) == 0,
        "rightward: red"
    );
    hold(&mut s, pt(120.0, 80.0), false, false, true);
    assert!(count(&s, green) > 0 && count(&s, red) == 0, "Alt: inverted");
    s.pointer_up(pt(120.0, 80.0), false, false);
    assert_eq!((count(&s, green), count(&s, red)), (0, 0));
}

// ---- the lasso ----

/// Criteria 16 to 18: an Alt press draws a line and selects what it
/// touches; Shift or Ctrl at the release combine.
#[test]
fn a_lasso_selects_what_the_line_crosses() {
    let mut s = session();
    drag(&mut s, pt(-5.0, 5.0), pt(25.0, 5.0), ALT, ALT);
    assert_eq!(s.selected_object_count(), 2);
    drag(
        &mut s,
        pt(35.0, 5.0),
        pt(45.0, 5.0),
        (true, false, true),
        (true, false, true),
    );
    assert_eq!(s.selected_object_count(), 3);
    drag(
        &mut s,
        pt(-5.0, 5.0),
        pt(5.0, 5.0),
        (false, true, true),
        (false, true, true),
    );
    assert_eq!(s.selected_object_count(), 2);
}

/// Criterion 17: an Alt press on an object's outline draws, never moves.
#[test]
fn an_alt_press_on_an_object_never_moves_it() {
    let mut s = session();
    let before = s.pack("0.1.0").unwrap();
    drag(&mut s, pt(0.0, 5.0), pt(0.0, 60.0), ALT, ALT);
    assert_eq!(s.pack("0.1.0").unwrap(), before);
}

// ---- the Alt-click cycle ----

/// Criteria 3 to 5 through the session: a rectangle and an ellipse whose
/// outlines both lie within tolerance of one point; a click takes the nearer
/// (the rectangle), the Alt-clicks alternate.
#[test]
fn alt_clicks_alternate_between_overlapping_objects() {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(20.0),
    });
    let _ = document.create_ellipse(curvyo_document_core::EllipseFrame {
        center: pt(10.0, 10.2),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(10.0),
    });
    let mut s = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    s.resize_viewport(1200.0, 800.0);
    let is_rect = |s: &Session| {
        assert_eq!(s.selected_object_count(), 1);
        s.select_bar_state().remove_rounding_shown
    };
    let at = pt(10.0, 0.0);
    click(&mut s, at);
    assert!(is_rect(&s), "the nearer outline");
    for expected_rect in [false, true, false] {
        hold(&mut s, at, false, false, true);
        s.pointer_down(at, false);
        s.pointer_up(at, false, false);
        hold(&mut s, at, false, false, false);
        assert_eq!(is_rect(&s), expected_rect);
    }
}

// ---- cursor, badge and keys ----

/// The cursor: crosshair from the press of a marquee, lasso from the press
/// of a lasso and while Alt is held with no button down; locked at the press.
#[test]
fn the_cursor_follows_the_gesture_locked_at_the_press() {
    let mut s = session();
    let far = pt(300.0, 300.0);
    hold(&mut s, far, false, false, false);
    assert_eq!(s.cursor_hint(), "default");
    hold(&mut s, far, false, false, true);
    assert_eq!(s.cursor_hint(), "lasso", "Alt held, no button");
    hold(&mut s, far, false, false, false);
    assert_eq!(s.cursor_hint(), "default", "Alt released");

    s.pointer_down(far, false);
    assert_eq!(s.cursor_hint(), "crosshair", "armed at the press");
    hold(&mut s, pt(330.0, 320.0), false, false, true);
    assert_eq!(
        s.cursor_hint(),
        "crosshair",
        "Alt mid-box does not change it"
    );
    s.pointer_up(pt(330.0, 320.0), false, false);

    hold(&mut s, far, false, false, true);
    s.pointer_down(far, false);
    assert_eq!(s.cursor_hint(), "lasso");
    hold(&mut s, pt(330.0, 320.0), false, false, false);
    assert_eq!(
        s.cursor_hint(),
        "lasso",
        "Alt released mid-lasso: still a lasso"
    );
    s.pointer_up(pt(330.0, 320.0), false, false);
}

/// The minus badge shows where a Ctrl press would arm the marquee or the
/// lasso, the plus badge where it would start a copy; never both.
#[test]
fn the_minus_badge_shows_where_the_plus_badge_does_not() {
    let mut s = session();
    let empty = pt(300.0, 300.0);
    let outline = pt(5.0, 0.0);
    hold(&mut s, empty, false, false, false);
    let none = s.move_indicators();
    assert!(!none.copy_badge && !none.remove_badge);
    hold(&mut s, empty, false, true, false);
    let on_empty = s.move_indicators();
    assert!(on_empty.remove_badge && !on_empty.copy_badge);
    hold(&mut s, outline, false, true, false);
    let on_outline = s.move_indicators();
    assert!(on_outline.copy_badge && !on_outline.remove_badge);
    hold(&mut s, outline, false, true, true);
    let with_alt = s.move_indicators();
    assert!(
        with_alt.remove_badge && !with_alt.copy_badge,
        "Ctrl+Alt arms a lasso"
    );
    hold(&mut s, empty, true, false, false);
    let shift = s.move_indicators();
    assert!(
        !shift.remove_badge && !shift.copy_badge,
        "Shift alone shows none"
    );

    // While the Ctrl marquee runs, the badge stays; releasing Ctrl drops it.
    hold(&mut s, empty, false, true, false);
    s.pointer_down(empty, false);
    hold(&mut s, pt(340.0, 330.0), false, true, false);
    assert!(s.move_indicators().remove_badge);
    hold(&mut s, pt(340.0, 330.0), false, false, false);
    assert!(!s.move_indicators().remove_badge);
    s.pointer_up(pt(340.0, 330.0), false, false);
}

/// Escape cancels a marquee or a lasso in flight, writes nothing and keeps
/// the selection (criteria 49 and 42 of `edit-interaction-polish`).
#[test]
fn escape_cancels_a_running_selection_gesture() {
    for alt in [false, true] {
        let mut s = session();
        drag(&mut s, pt(-5.0, -5.0), pt(15.0, 15.0), NONE, NONE);
        assert_eq!(s.selected_object_count(), 1);
        let before = s.pack("0.1.0").unwrap();
        hold(&mut s, pt(300.0, 300.0), false, false, alt);
        s.pointer_down(pt(300.0, 300.0), false);
        hold(&mut s, pt(340.0, 330.0), false, false, alt);
        assert!(s.live_readout().is_some());
        assert_eq!(s.escape(), EscapeStep::CancelledDrag, "alt {alt}");
        assert!(s.live_readout().is_none());
        s.pointer_up(pt(340.0, 330.0), false, false);
        assert_eq!(s.selected_object_count(), 1, "the selection stays");
        assert_eq!(s.pack("0.1.0").unwrap(), before);
    }
}

/// The key gate: a letter does nothing while a marquee runs (a drag is in
/// flight in the Select tool).
#[test]
fn shortcuts_are_gated_during_a_marquee() {
    let mut s = session();
    hold(&mut s, pt(300.0, 300.0), false, false, false);
    s.pointer_down(pt(300.0, 300.0), false);
    hold(&mut s, pt(340.0, 330.0), false, false, false);
    let outcome = s.key_down(KeyInput {
        key: "b",
        ..KeyInput::default()
    });
    assert_eq!(outcome, KeyOutcome::Ignored);
    assert_eq!(s.tool(), Tool::Select);
    s.pointer_up(pt(340.0, 330.0), false, false);
}
