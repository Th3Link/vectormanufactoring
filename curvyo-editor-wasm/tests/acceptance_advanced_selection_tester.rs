//! Independent tester acceptance tests for `specs/0014-advanced-selection/` at the
//! `Session` level: hit area in screen pixels at several zooms, the cycle
//! through the session, marquee and lasso through the cached modifiers, the
//! legend, the overlay colours, the cursor, the minus badge, Escape, tool
//! switches, hostile input and cost. Selection identity is read through
//! `delete_selected` on a fresh session per observation (the session has no
//! selection accessor), so expected values never come from the code under
//! test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_sign_loss
)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::type_complexity)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::manual_let_else, missing_docs)]
#![allow(clippy::needless_range_loop)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, Point, RectBounds, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Mod flags as (shift, ctrl, alt).
type M = (bool, bool, bool);
const NONE: M = (false, false, false);
const SHIFT: M = (true, false, false);
const CTRL: M = (false, true, false);
const ALT: M = (false, false, true);
const SHIFT_CTRL: M = (true, true, false);
const ALT_SHIFT: M = (true, false, true);
const ALT_CTRL: M = (false, true, true);

/// Millimetre layout (scale independent): squares A, B, C of 100 mm at
/// x = 0, 300, 600 (y 0..100), and the horizontal line L at y = 500.
fn base_doc() -> Document {
    let d = Document::new(1);
    for x in [0.0, 300.0, 600.0] {
        let _ = d.create_rect(RectBounds {
            origin: pt(x, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(100.0),
        });
    }
    let _ = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 500.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(1000.0, 500.0)),
        ],
        false,
    );
    d
}

const NAMES: [&str; 4] = ["A", "B", "C", "L"];

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1200.0, 800.0);
    s
}

fn session() -> Session {
    open(&base_doc())
}

fn k(s: &Session) -> f64 {
    s.view().scale()
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn hold(s: &mut Session, at: Point, m: M) {
    s.modifiers_changed(m.0, m.1, m.2);
    s.pointer_hover(at, m.0, m.1);
}

fn click_m(s: &mut Session, at: Point, m: M) {
    hold(s, at, m);
    s.pointer_down(at, m.0);
    s.pointer_up(at, m.0, m.1);
    hold(s, at, NONE);
}

fn click(s: &mut Session, at: Point) {
    click_m(s, at, NONE);
}

/// Press with `press`, move through the middle and the end with `release`,
/// release with `release`.
fn drag(s: &mut Session, from: Point, to: Point, press: M, release: M) {
    hold(s, from, press);
    s.pointer_down(from, press.0);
    hold(
        s,
        pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y)),
        release,
    );
    hold(s, to, release);
    s.pointer_up(to, release.0, release.1);
    hold(s, to, NONE);
}

/// Names of the objects of the base document that the session has selected,
/// read by deleting them.
fn selected(s: &mut Session) -> Vec<&'static str> {
    let before = doc_of(s).object_ids();
    s.delete_selected();
    let after = doc_of(s).object_ids();
    before
        .iter()
        .enumerate()
        .filter(|(_, id)| !after.contains(id))
        .map(|(i, _)| NAMES[i])
        .collect()
}

/// Replays `script` on a fresh session and reads what it selected.
fn run(script: impl Fn(&mut Session)) -> Vec<&'static str> {
    let mut s = session();
    script(&mut s);
    selected(&mut s)
}

fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("did not finish in time (hang or pathological cost)")
}

fn legend(s: &Session) -> Option<String> {
    s.live_readout().map(|r| r.text)
}

const MINUS: char = '\u{2212}';
const DOT: char = '\u{b7}';

fn count_colour(s: &Session, rgb: (u8, u8, u8), alpha: Option<u8>) -> usize {
    s.draw_list()
        .triangles
        .iter()
        .filter(|v| {
            (v.color.r, v.color.g, v.color.b) == rgb && alpha.is_none_or(|a| v.color.a == a)
        })
        .count()
}

const GREEN: (u8, u8, u8) = (0x1C, 0x93, 0x47);
const RED: (u8, u8, u8) = (0xE5, 0x48, 0x4D);

// ---------------------------------------------------------------------
// AC 1 and 2: the hit area in screen pixels, at several zooms
// ---------------------------------------------------------------------

#[test]
fn ac1_select_hits_within_8_px_at_every_zoom() {
    for zoom_ticks in [0.0, -250.0, 250.0, 600.0] {
        let mut probe = session();
        if zoom_ticks != 0.0 {
            probe.wheel(0.0, zoom_ticks, 300.0, 300.0, false, true);
        }
        let scale = k(&probe);
        for (px, hit) in [
            (0.0, true),
            (2.0, true),
            (7.0, true),
            (7.9, true),
            (8.1, false),
            (9.0, false),
            (20.0, false),
        ] {
            let mut s = session();
            if zoom_ticks != 0.0 {
                s.wheel(0.0, zoom_ticks, 300.0, 300.0, false, true);
            }
            // Left of A's left edge (x = 0), mid-height, `px` screen pixels away.
            click(&mut s, pt(-px / scale, 50.0));
            assert_eq!(
                s.selected_object_count(),
                usize::from(hit),
                "scale {scale}, {px} px outside"
            );
            // Inside the edge as well: an unfilled interior is hit only by its outline.
            let mut s = session();
            if zoom_ticks != 0.0 {
                s.wheel(0.0, zoom_ticks, 300.0, 300.0, false, true);
            }
            click(&mut s, pt(px / scale, 50.0));
            assert_eq!(
                s.selected_object_count(),
                usize::from(hit),
                "scale {scale}, {px} px inside"
            );
        }
    }
}

#[test]
fn ac1_the_zoom_actually_changed_in_that_test() {
    let mut s = session();
    let before = k(&s);
    s.wheel(0.0, -250.0, 300.0, 300.0, false, true);
    assert!(
        (k(&s) - before).abs() > 1e-9,
        "wheel zoom must change the scale"
    );
}

#[test]
fn ac1_hover_lights_the_hit_object_from_the_same_radius() {
    let mut s = session();
    s.pointer_hover(pt(900.0, 300.0), false, false);
    let idle = s.draw_list().triangle_count();
    s.pointer_hover(pt(-7.0 / k(&s), 50.0), false, false);
    let near = s.draw_list().triangle_count();
    s.pointer_hover(pt(-9.0 / k(&s), 50.0), false, false);
    let far = s.draw_list().triangle_count();
    assert!(near > idle, "7 px: the hover box shows");
    assert_eq!(far, idle, "9 px: it does not");
}

fn node_tool_pick(px: f64) -> bool {
    let mut s = session();
    s.set_tool(Tool::Node);
    click(&mut s, pt(0.0, 500.0)); // a node of L makes it the edited path
    let at = pt(500.0, 500.0 + px / k(&s));
    click(&mut s, at);
    s.node_toolbar_state().can_insert
}

#[test]
fn ac2_the_node_tool_segment_pick_stays_at_4_px() {
    assert!(node_tool_pick(0.0));
    assert!(node_tool_pick(3.5));
    assert!(
        !node_tool_pick(4.6),
        "5 px from a segment is not a pick in the Node tool"
    );
    assert!(
        !node_tool_pick(7.0),
        "the Select 8 px must not leak into the Node tool"
    );
}

#[test]
fn ac2_the_select_tool_still_hits_at_7_px_where_the_node_tool_does_not() {
    let mut s = session();
    let at = pt(500.0, 500.0 + 7.0 / k(&s));
    click(&mut s, at);
    assert_eq!(selected(&mut s), vec!["L"]);
}

// ---------------------------------------------------------------------
// AC 3 to 7: Alt-click cycle through the session
// ---------------------------------------------------------------------

/// Two vertical lines 3 px apart at x = 1100 and 1100 + 3/k, plus a third
/// 6 px from the first, all named by index into the document's objects.
fn cycle_doc(scale: f64) -> Document {
    let d = base_doc();
    for dx in [0.0, 3.0, 6.0] {
        let x = 1100.0 + dx / scale;
        let n = (dx as u64) * 2 + 10;
        let _ = d.create_path(
            &[
                NewAnchor::corner(AnchorId::new(3, n), pt(x, 0.0)),
                NewAnchor::corner(AnchorId::new(3, n + 1), pt(x, 100.0)),
            ],
            false,
        );
    }
    d
}

#[test]
fn ac3_to_7_cycle_through_the_session() {
    let scale = k(&session());
    // The point sits 1 px right of the first line: distances 1, 2 and 5 px.
    let p = pt(1100.0 + 1.0 / scale, 50.0);
    let order = ["L1", "L2", "L3"];
    let names = |d: &Document| -> Vec<curvyo_document_core::NodeId> { d.object_ids() };
    let ids = names(&cycle_doc(scale));
    let sel = |steps: &[M]| -> Vec<usize> {
        let mut s = open(&cycle_doc(scale));
        for (n, m) in steps.iter().enumerate() {
            let _ = n;
            click_m(&mut s, p, *m);
        }
        let before = doc_of(&s).object_ids();
        s.delete_selected();
        let after = doc_of(&s).object_ids();
        before
            .iter()
            .enumerate()
            .filter(|(_, id)| !after.contains(id))
            .map(|(i, _)| i)
            .collect()
    };
    let _ = (order, ids);
    // Document order: A B C L, then L1 L2 L3 at indices 4, 5, 6.
    assert_eq!(sel(&[NONE]), vec![4], "plain: nearest");
    assert_eq!(sel(&[NONE, ALT]), vec![5], "next-nearest");
    assert_eq!(sel(&[NONE, ALT, ALT]), vec![6]);
    assert_eq!(
        sel(&[NONE, ALT, ALT, ALT]),
        vec![4],
        "wraps after as many clicks as candidates"
    );
    assert_eq!(sel(&[NONE, ALT, ALT, ALT, ALT]), vec![5]);
}

#[test]
fn ac4_a_session_alt_click_with_a_few_pixels_of_jitter_still_cycles() {
    let scale = k(&session());
    let p = pt(1100.0 + 1.0 / scale, 50.0);
    let mut s = open(&cycle_doc(scale));
    click(&mut s, p);
    // Alt press, 2 px of drift, release: a click, not a lasso.
    hold(&mut s, p, ALT);
    s.pointer_down(p, false);
    let drift = pt(p.x + 2.0 / scale, p.y);
    hold(&mut s, drift, ALT);
    assert!(
        legend(&s).is_none(),
        "inside the dead zone nothing is drawn"
    );
    s.pointer_up(drift, false, false);
    hold(&mut s, drift, NONE);
    assert_eq!(
        s.selected_object_count(),
        1,
        "one candidate, not every line along the drift"
    );
}

#[test]
fn ac6_escape_ends_the_cycle() {
    let scale = k(&session());
    let p = pt(1100.0 + 1.0 / scale, 50.0);
    let mut s = open(&cycle_doc(scale));
    click(&mut s, p);
    click_m(&mut s, p, ALT);
    let _ = s.escape();
    click_m(&mut s, p, ALT);
    let before = doc_of(&s).object_ids();
    s.delete_selected();
    let after = doc_of(&s).object_ids();
    let gone: Vec<usize> = before
        .iter()
        .enumerate()
        .filter(|(_, id)| !after.contains(id))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(gone.len(), 1);
    // After Escape cleared the selection the cycle must restart at the
    // nearest candidate (L1), not carry on to L3.
    assert_eq!(gone, vec![4]);
}

// ---------------------------------------------------------------------
// AC 8 to 15: marquee through the session
// ---------------------------------------------------------------------

// Boxes in mm: contains A only (-50,-50)..(250,150); crosses B at the same
// right edge only if it reaches x = 350: (-50,-50)..(350,150) contains A and
// crosses B.
const FROM: Point = Point { x: -50.0, y: -50.0 };

fn pt_to(x: f64, y: f64) -> Point {
    pt(x, y)
}

#[test]
fn ac9_10_directions_select_by_touch_and_contain() {
    // Rightward: contain. Box (-50,-50)..(350,150) contains A, only touches B.
    assert_eq!(
        run(|s| drag(s, FROM, pt_to(350.0, 150.0), NONE, NONE)),
        vec!["A"]
    );
    // Leftward over the same rectangle: touch -> A and B.
    assert_eq!(
        run(|s| drag(s, pt_to(350.0, 150.0), FROM, NONE, NONE)),
        vec!["A", "B"]
    );
}

#[test]
fn ac11_alt_at_release_inverts_and_alt_at_press_is_a_lasso_instead() {
    assert_eq!(
        run(|s| drag(s, FROM, pt_to(350.0, 150.0), NONE, ALT)),
        vec!["A", "B"],
        "Alt pressed mid-drag: the rightward box becomes touch"
    );
    assert_eq!(
        run(|s| drag(s, pt_to(350.0, 150.0), FROM, NONE, ALT)),
        vec!["A"],
        "leftward + Alt: contain"
    );
    // Alt at the press: a lasso along the same diagonal touches the outlines it crosses.
    assert_eq!(
        run(|s| drag(s, pt_to(-50.0, 50.0), pt_to(350.0, 50.0), ALT, ALT)),
        vec!["A", "B"],
        "the line crosses A's two side edges and B's left edge"
    );
}

#[test]
fn ac12_13_combine_modifiers_at_release() {
    // Start with B and C selected via clicks on their left edges.
    let prep = |s: &mut Session| {
        click(s, pt(300.0, 50.0));
        click_m(s, pt(600.0, 50.0), SHIFT);
    };
    assert_eq!(
        run(|s| {
            prep(s);
            drag(s, FROM, pt_to(350.0, 150.0), NONE, NONE);
        }),
        vec!["A"],
        "replace"
    );
    assert_eq!(
        run(|s| {
            prep(s);
            drag(s, FROM, pt_to(350.0, 150.0), NONE, SHIFT);
        }),
        vec!["A", "B", "C"],
        "Shift at release adds"
    );
    assert_eq!(
        run(|s| {
            prep(s);
            drag(s, pt_to(350.0, 150.0), FROM, NONE, CTRL);
        }),
        vec!["C"],
        "Ctrl at release removes B (touched) and does not add A"
    );
    assert_eq!(
        run(|s| {
            prep(s);
            drag(s, pt_to(350.0, 150.0), FROM, NONE, SHIFT_CTRL);
        }),
        vec!["C"],
        "Shift+Ctrl: Ctrl wins"
    );
    assert_eq!(
        run(|s| {
            prep(s);
            drag(s, FROM, pt_to(350.0, 150.0), SHIFT, NONE);
        }),
        vec!["A"],
        "Shift only at press: replace"
    );
    assert_eq!(
        run(|s| {
            prep(s);
            drag(
                s,
                pt_to(250.0, 50.0),
                pt_to(350.0, 50.0),
                ALT_CTRL,
                ALT_CTRL,
            );
        }),
        vec!["C"],
        "Alt+Ctrl lasso crossing B's left edge removes B"
    );
}

#[test]
fn ac13_ctrl_marquee_over_unselected_objects_changes_nothing() {
    assert_eq!(
        run(|s| {
            click(s, pt(600.0, 50.0));
            drag(s, pt_to(350.0, 150.0), FROM, NONE, CTRL);
        }),
        vec!["C"]
    );
}

#[test]
fn ac15_a_marquee_moves_nothing() {
    let mut s = session();
    let ids_before = doc_of(&s).object_ids();
    drag(&mut s, FROM, pt_to(900.0, 700.0), NONE, NONE);
    assert_eq!(doc_of(&s).object_ids(), ids_before);
    for (i, x) in [0.0, 300.0, 600.0].into_iter().enumerate() {
        assert_eq!(origin_of(&s, i), (x, 0.0));
    }
    assert_eq!(selected(&mut s), vec!["A", "B", "C"]);
}

#[test]
fn ac8_click_on_empty_canvas_clears_at_release_and_ctrl_shift_keep() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    assert_eq!(s.selected_object_count(), 1);
    hold(&mut s, pt(450.0, 300.0), NONE);
    s.pointer_down(pt(450.0, 300.0), false);
    assert_eq!(
        s.selected_object_count(),
        1,
        "between press and release the selection is still there (a drag may follow)"
    );
    s.pointer_up(pt(450.0, 300.0), false, false);
    assert_eq!(s.selected_object_count(), 0);
    for m in [CTRL, SHIFT, SHIFT_CTRL] {
        let mut s = session();
        click(&mut s, pt(300.0, 50.0));
        click_m(&mut s, pt(450.0, 300.0), m);
        assert_eq!(s.selected_object_count(), 1, "{m:?}");
    }
}

#[test]
fn ac8_plain_empty_press_then_escape_keeps_the_selection() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    hold(&mut s, pt(450.0, 300.0), NONE);
    s.pointer_down(pt(450.0, 300.0), false);
    let step = s.escape();
    s.pointer_up(pt(450.0, 300.0), false, false);
    assert_eq!(
        s.selected_object_count(),
        1,
        "a cancelled press clears nothing (Escape gave {step:?})"
    );
}

#[test]
fn ac8_dead_zone_is_3_px_at_the_session() {
    let scale = k(&session());
    for (px, marquee) in [(2.0, false), (2.9, false), (3.3, true), (6.0, true)] {
        let mut s = session();
        let from = pt(450.0, 300.0);
        hold(&mut s, from, NONE);
        s.pointer_down(from, false);
        let to = pt(450.0 - px / scale, 300.0);
        hold(&mut s, to, NONE);
        assert_eq!(legend(&s).is_some(), marquee, "{px} px");
        s.pointer_up(to, false, false);
    }
}

// ---------------------------------------------------------------------
// Legend, overlay colour, cursor
// ---------------------------------------------------------------------

fn expected_legend(mode_touch: bool, line: bool, m: M) -> String {
    let mode = match (mode_touch, line) {
        (_, true) => "Touch (line)",
        (true, false) => "Touch",
        (false, false) => "Contain",
    };
    let combine = if m.1 {
        format!("{MINUS}Remove")
    } else if m.0 {
        "+Add".to_string()
    } else {
        "Replace".to_string()
    };
    format!("{mode} {DOT} {combine}")
}

#[test]
fn ac14_the_legend_follows_direction_and_every_modifier_live() {
    let from = pt(450.0, 300.0);
    for rightward in [false, true] {
        for m in [
            NONE,
            SHIFT,
            CTRL,
            ALT,
            SHIFT_CTRL,
            ALT_SHIFT,
            ALT_CTRL,
            (true, true, true),
        ] {
            let mut s = session();
            hold(&mut s, from, NONE);
            s.pointer_down(from, false);
            let to = pt(if rightward { 700.0 } else { 200.0 }, 350.0);
            hold(&mut s, to, m);
            let touch = rightward == m.2;
            let got = legend(&s).unwrap_or_default();
            assert_eq!(
                got,
                expected_legend(touch, false, m),
                "rightward {rightward} {m:?}"
            );
            assert_eq!(
                s.live_readout().unwrap().anchor,
                to,
                "anchored at the pointer"
            );
            s.pointer_up(to, m.0, m.1);
        }
    }
}

#[test]
fn ac14_16_the_lasso_legend_is_touch_line() {
    for m in [ALT, ALT_SHIFT, ALT_CTRL, (true, true, true)] {
        let mut s = session();
        let from = pt(450.0, 300.0);
        hold(&mut s, from, m);
        s.pointer_down(from, m.0);
        let to = pt(700.0, 350.0);
        hold(&mut s, to, m);
        assert_eq!(
            legend(&s).unwrap_or_default(),
            expected_legend(true, true, m),
            "{m:?}"
        );
        s.pointer_up(to, m.0, m.1);
    }
}

#[test]
fn the_legend_is_gone_after_release_escape_and_in_other_tools() {
    let mut s = session();
    let from = pt(450.0, 300.0);
    assert!(legend(&s).is_none());
    hold(&mut s, from, NONE);
    s.pointer_down(from, false);
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert!(legend(&s).is_some());
    s.pointer_up(pt(600.0, 350.0), false, false);
    assert!(legend(&s).is_none(), "after release");
    hold(&mut s, from, NONE);
    s.pointer_down(from, false);
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert_eq!(s.escape(), EscapeStep::CancelledDrag);
    assert!(legend(&s).is_none(), "after Escape");
    s.pointer_up(pt(600.0, 350.0), false, false);
    for tool in [Tool::Node, Tool::Pen] {
        s.set_tool(tool);
        hold(&mut s, from, ALT_CTRL);
        s.pointer_down(from, false);
        hold(&mut s, pt(600.0, 350.0), ALT_CTRL);
        assert!(legend(&s).is_none(), "{tool:?}: no marquee or lasso legend");
        s.pointer_up(pt(600.0, 350.0), false, true);
        hold(&mut s, pt(600.0, 350.0), NONE);
    }
}

#[test]
fn ac9_10_11_14_the_box_colour_is_green_for_touch_and_red_for_contain() {
    let from = pt(450.0, 300.0);
    let idle = {
        let mut s = session();
        hold(&mut s, from, NONE);
        (count_colour(&s, GREEN, None), count_colour(&s, RED, None))
    };
    assert_eq!(idle, (0, 0), "no marquee colours at rest");
    let colours = |rightward: bool, m: M| {
        let mut s = session();
        hold(&mut s, from, NONE);
        s.pointer_down(from, false);
        hold(&mut s, pt(if rightward { 700.0 } else { 200.0 }, 350.0), m);
        (count_colour(&s, GREEN, None), count_colour(&s, RED, None))
    };
    let (g, r) = colours(false, NONE);
    assert!(g > 0 && r == 0, "leftward: green only ({g}, {r})");
    let (g, r) = colours(true, NONE);
    assert!(r > 0 && g == 0, "rightward: red only ({g}, {r})");
    let (g, r) = colours(false, ALT);
    assert!(r > 0 && g == 0, "leftward + Alt: red");
    let (g, r) = colours(true, ALT);
    assert!(g > 0 && r == 0, "rightward + Alt: green");
    // Shift and Ctrl do not change the colour.
    for m in [SHIFT, CTRL, SHIFT_CTRL] {
        let (g, r) = colours(true, m);
        assert!(r > 0 && g == 0, "{m:?}");
    }
}

#[test]
fn ac9_the_box_has_a_translucent_fill_and_an_opaque_border() {
    let mut s = session();
    let from = pt(450.0, 300.0);
    hold(&mut s, from, NONE);
    s.pointer_down(from, false);
    hold(&mut s, pt(200.0, 350.0), NONE);
    assert!(count_colour(&s, GREEN, Some(255)) > 0, "solid border");
    let fill = count_colour(&s, GREEN, Some(31));
    assert!(fill >= 6, "12 % fill ({fill} vertices)");
}

#[test]
fn ac16_the_lasso_line_is_always_green_and_dashed() {
    let mut s = session();
    let from = pt(450.0, 300.0);
    hold(&mut s, from, ALT);
    s.pointer_down(from, false);
    for i in 1..=40 {
        hold(
            &mut s,
            pt(450.0 + f64::from(i) * 5.0, 300.0 + f64::from(i) * 2.0),
            ALT,
        );
    }
    let g = count_colour(&s, GREEN, None);
    assert!(g > 0);
    assert_eq!(count_colour(&s, RED, None), 0);
    // Dashed: more than one separate piece. A 250 mm line drawn solid is a
    // few triangles per point; the dashed one has many pieces.
    let list = s.draw_list();
    let green_tris = list
        .triangles
        .chunks(3)
        .filter(|t| (t[0].color.r, t[0].color.g, t[0].color.b) == GREEN)
        .count();
    let line_len_px = 40.0 * (5.0f64.hypot(2.0)) * k(&s);
    assert!(
        green_tris as f64 > line_len_px / 7.0,
        "at least one dash (2 triangles) per 7 px: {green_tris} triangles over {line_len_px} px"
    );
    // Alt released mid-lasso: still the lasso.
    hold(&mut s, pt(700.0, 400.0), NONE);
    assert!(count_colour(&s, GREEN, None) > 0 && count_colour(&s, RED, None) == 0);
    assert!(legend(&s).unwrap().starts_with("Touch (line)"));
}

#[test]
fn cursor_states_follow_the_gesture_locked_at_press() {
    let mut s = session();
    let from = pt(450.0, 300.0);
    assert_eq!(s.cursor_hint(), "default");
    hold(&mut s, from, ALT);
    assert_eq!(s.cursor_hint(), "lasso", "Alt held, hovering");
    hold(&mut s, from, NONE);
    assert_eq!(s.cursor_hint(), "default", "Alt released while idle");
    // Shift and Ctrl change neither.
    for m in [SHIFT, CTRL, SHIFT_CTRL] {
        hold(&mut s, from, m);
        assert_eq!(s.cursor_hint(), "default", "{m:?}");
    }
    // A box locks the crosshair; Alt mid-drag does not switch it.
    hold(&mut s, from, NONE);
    s.pointer_down(from, false);
    assert_eq!(
        s.cursor_hint(),
        "crosshair",
        "armed at the press, no movement yet"
    );
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert_eq!(s.cursor_hint(), "crosshair");
    hold(&mut s, pt(600.0, 350.0), ALT);
    assert_eq!(s.cursor_hint(), "crosshair", "Alt mid-box: still crosshair");
    s.pointer_up(pt(600.0, 350.0), false, false);
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert_eq!(s.cursor_hint(), "default", "after the release");
    // A lasso locks the lasso glyph even after Alt is released.
    hold(&mut s, from, ALT);
    s.pointer_down(from, false);
    assert_eq!(s.cursor_hint(), "lasso");
    hold(&mut s, pt(600.0, 350.0), ALT);
    hold(&mut s, pt(650.0, 350.0), NONE);
    assert_eq!(s.cursor_hint(), "lasso", "Alt released mid-lasso: stays");
    s.pointer_up(pt(650.0, 350.0), false, false);
    hold(&mut s, pt(650.0, 350.0), NONE);
    assert_eq!(s.cursor_hint(), "default");
}

#[test]
fn the_lasso_cursor_belongs_to_the_select_tool_only() {
    let mut s = session();
    for tool in [Tool::Node, Tool::Pen, Tool::Rectangle, Tool::Ellipse] {
        s.set_tool(tool);
        hold(&mut s, pt(450.0, 300.0), ALT);
        assert_ne!(s.cursor_hint(), "lasso", "{tool:?}");
        assert_ne!(s.cursor_hint(), "crosshair-marquee", "{tool:?}");
        hold(&mut s, pt(450.0, 300.0), NONE);
    }
}

// ---------------------------------------------------------------------
// Minus badge
// ---------------------------------------------------------------------

#[test]
fn the_minus_badge_shows_where_a_ctrl_press_would_arm_a_marquee_and_never_with_the_plus() {
    let mut s = session();
    let empty = pt(450.0, 300.0);
    hold(&mut s, empty, NONE);
    let b = s.move_indicators();
    assert!(!b.remove_badge && !b.copy_badge);
    hold(&mut s, empty, CTRL);
    let b = s.move_indicators();
    assert!(
        b.remove_badge && !b.copy_badge,
        "empty canvas + Ctrl: minus"
    );
    hold(&mut s, empty, SHIFT_CTRL);
    assert!(s.move_indicators().remove_badge, "Shift+Ctrl removes too");
    hold(&mut s, empty, SHIFT);
    assert!(
        !s.move_indicators().remove_badge,
        "Shift alone adds: no minus"
    );
    // Over an object's outline Ctrl means copy.
    hold(&mut s, pt(300.0, 50.0), CTRL);
    let b = s.move_indicators();
    assert!(
        b.copy_badge && !b.remove_badge,
        "over an object: plus, not minus"
    );
    // While a Ctrl marquee runs and after.
    hold(&mut s, empty, NONE);
    s.pointer_down(empty, false);
    hold(&mut s, pt(600.0, 350.0), CTRL);
    assert!(s.move_indicators().remove_badge, "during a Ctrl marquee");
    assert!(!s.move_indicators().copy_badge);
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert!(!s.move_indicators().remove_badge, "Ctrl released mid-drag");
    hold(&mut s, pt(600.0, 350.0), CTRL);
    s.pointer_up(pt(600.0, 350.0), false, true);
    hold(&mut s, pt(600.0, 350.0), NONE);
    assert!(!s.move_indicators().remove_badge, "gone after release");
    // A Ctrl lasso.
    hold(&mut s, empty, ALT_CTRL);
    s.pointer_down(empty, false);
    hold(&mut s, pt(600.0, 350.0), ALT_CTRL);
    assert!(s.move_indicators().remove_badge, "Ctrl lasso: minus");
    assert!(!s.move_indicators().copy_badge);
    s.pointer_up(pt(600.0, 350.0), false, true);
}

#[test]
fn the_plus_and_the_minus_never_show_together_anywhere() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0)); // B selected: handles and a box exist
    let probes = [
        pt(350.0, 50.0),
        pt(300.0, 50.0),
        pt(300.0, 0.0),
        pt(450.0, 300.0),
        pt(-30.0, -30.0),
        pt(400.0, 100.0),
        pt(320.0, 20.0),
    ];
    for p in probes {
        for m in [
            NONE,
            SHIFT,
            CTRL,
            ALT,
            SHIFT_CTRL,
            ALT_SHIFT,
            ALT_CTRL,
            (true, true, true),
        ] {
            hold(&mut s, p, m);
            let b = s.move_indicators();
            assert!(!(b.copy_badge && b.remove_badge), "{p:?} {m:?}");
        }
    }
    hold(&mut s, pt(450.0, 300.0), NONE);
}

#[test]
fn the_minus_badge_is_a_select_tool_thing() {
    let mut s = session();
    for tool in [Tool::Node, Tool::Pen, Tool::Rectangle] {
        s.set_tool(tool);
        hold(&mut s, pt(450.0, 300.0), CTRL);
        assert!(!s.move_indicators().remove_badge, "{tool:?}");
        hold(&mut s, pt(450.0, 300.0), NONE);
    }
}

// ---------------------------------------------------------------------
// Escape, cancel, tool switch, keys
// ---------------------------------------------------------------------

#[test]
fn escape_cascade_cancels_a_marquee_then_clears_then_nothing() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    let from = pt(450.0, 300.0);
    hold(&mut s, from, NONE);
    s.pointer_down(from, false);
    hold(&mut s, pt(800.0, 400.0), NONE);
    assert!(legend(&s).is_some());
    assert_eq!(s.escape(), EscapeStep::CancelledDrag);
    assert!(legend(&s).is_none());
    assert_eq!(
        count_colour(&s, GREEN, None) + count_colour(&s, RED, None),
        0,
        "overlay gone"
    );
    s.pointer_up(pt(800.0, 400.0), false, false);
    assert_eq!(
        s.selected_object_count(),
        1,
        "the cancelled drag did not touch the selection"
    );
    assert_eq!(s.escape(), EscapeStep::ClearedState);
    assert_eq!(s.selected_object_count(), 0);
}

#[test]
fn escape_cancels_a_lasso_and_the_release_then_selects_nothing() {
    let mut s = session();
    hold(&mut s, pt(-100.0, 50.0), ALT);
    s.pointer_down(pt(-100.0, 50.0), false);
    hold(&mut s, pt(400.0, 50.0), ALT);
    assert_eq!(s.escape(), EscapeStep::CancelledDrag);
    s.pointer_up(pt(800.0, 50.0), false, false);
    hold(&mut s, pt(800.0, 50.0), NONE);
    assert_eq!(s.selected_object_count(), 0);
    assert!(legend(&s).is_none());
}

#[test]
fn pointer_cancelled_mid_marquee_applies_nothing() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    hold(&mut s, FROM, NONE);
    s.pointer_down(FROM, false);
    hold(&mut s, pt(900.0, 700.0), NONE);
    s.pointer_cancelled();
    assert!(legend(&s).is_none());
    assert_eq!(s.selected_object_count(), 1);
    // A later plain move and click behave normally.
    click(&mut s, pt(600.0, 50.0));
    assert_eq!(selected(&mut s), vec!["C"]);
}

#[test]
fn switching_tools_mid_marquee_leaves_no_stale_gesture() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    hold(&mut s, FROM, NONE);
    s.pointer_down(FROM, false);
    hold(&mut s, pt(900.0, 700.0), NONE);
    s.set_tool(Tool::Node);
    assert!(legend(&s).is_none(), "no legend in the Node tool");
    s.pointer_up(pt(900.0, 700.0), false, false);
    s.set_tool(Tool::Select);
    assert!(legend(&s).is_none());
    assert_eq!(
        count_colour(&s, GREEN, None) + count_colour(&s, RED, None),
        0
    );
    // And a marquee works again afterwards.
    drag(&mut s, FROM, pt_to(900.0, 700.0), NONE, NONE);
    assert_eq!(s.selected_object_count(), 3);
}

#[test]
fn keys_cannot_delete_or_act_while_a_gesture_runs_with_focus_in_a_field() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    let del = KeyInput {
        key: "Delete",
        dom_blocked: true,
        ..KeyInput::default()
    };
    assert!(matches!(s.key_down(del), KeyOutcome::Ignored));
    assert_eq!(s.selected_object_count(), 1);
    // Alt as a lone key is not a command.
    let alt = KeyInput {
        key: "Alt",
        alt: true,
        ..KeyInput::default()
    };
    assert!(matches!(s.key_down(alt), KeyOutcome::Ignored));
    assert_eq!(s.selected_object_count(), 1);
}

// ---------------------------------------------------------------------
// Moving while modifiers are held (press order)
// ---------------------------------------------------------------------

fn origin_of(s: &Session, index: usize) -> (f64, f64) {
    let d = doc_of(s);
    match d.object(d.object_ids()[index]).unwrap() {
        curvyo_document_core::ObjectSnapshot::Primitive(p) => match p.shape {
            curvyo_document_core::Shape::Rect { bounds, .. } => (bounds.origin.x, bounds.origin.y),
            _ => panic!("rect expected"),
        },
        curvyo_document_core::ObjectSnapshot::Path(_) => panic!("rect expected"),
    }
}

#[test]
fn a_plain_press_on_an_outline_still_moves() {
    let mut s = session();
    drag(&mut s, pt(300.0, 50.0), pt(300.0, 250.0), NONE, NONE);
    let o = origin_of(&s, 1);
    assert!((o.1 - 200.0).abs() < 1e-6, "B moved 200 mm down, got {o:?}");
}

#[test]
fn an_alt_press_on_an_outline_never_moves_even_if_alt_is_released_mid_drag() {
    let mut s = session();
    drag(&mut s, pt(300.0, 50.0), pt(300.0, 250.0), ALT, NONE);
    assert_eq!(origin_of(&s, 1), (300.0, 0.0));
    let mut s = session();
    drag(&mut s, pt(300.0, 50.0), pt(300.0, 250.0), ALT, ALT);
    assert_eq!(origin_of(&s, 1), (300.0, 0.0));
    // The line it drew from B's outline down to y = 250 touched B (and no one else).
    assert_eq!(selected(&mut s), vec!["B"]);
}

#[test]
fn a_ctrl_press_on_an_outline_still_copies_it() {
    let mut s = session();
    let n = doc_of(&s).object_ids().len();
    drag(&mut s, pt(300.0, 50.0), pt(300.0, 250.0), CTRL, CTRL);
    assert_eq!(doc_of(&s).object_ids().len(), n + 1);
}

#[test]
fn a_shift_press_on_an_unselected_outline_still_joins_the_selection() {
    let mut s = session();
    click(&mut s, pt(0.0, 50.0));
    click_m(&mut s, pt(300.0, 50.0), SHIFT);
    assert_eq!(selected(&mut s), vec!["A", "B"]);
}

// ---------------------------------------------------------------------
// Hostile input and cost
// ---------------------------------------------------------------------

#[test]
fn nan_infinite_and_huge_coordinates_never_panic_or_change_the_selection() {
    // `draw` only for values the session is expected to reject: a finite 1e300
    // lasso would ask the overlay for an astronomical number of dashes (see
    // the ignored defect test below).
    for (bad, draw) in [
        (f64::NAN, true),
        (f64::INFINITY, true),
        (f64::NEG_INFINITY, true),
        (1e300, false),
        (-1e300, false),
        (f64::MAX, false),
        (f64::MIN, false),
    ] {
        for press_m in [NONE, ALT, SHIFT, CTRL, ALT_CTRL] {
            let mut s = session();
            click(&mut s, pt(300.0, 50.0));
            hold(&mut s, pt(450.0, 300.0), press_m);
            s.pointer_down(pt(450.0, 300.0), press_m.0);
            for p in [
                pt(bad, 300.0),
                pt(450.0, bad),
                pt(bad, bad),
                pt(600.0, 350.0),
            ] {
                hold(&mut s, p, press_m);
                let _ = legend(&s);
                if draw {
                    let _ = s.draw_list();
                }
                let _ = s.cursor_hint();
                let _ = s.move_indicators();
            }
            s.pointer_up(pt(bad, bad), press_m.0, press_m.1);
            hold(&mut s, pt(600.0, 350.0), NONE);
            click(&mut s, pt(600.0, 50.0));
            assert_eq!(selected(&mut s), vec!["C"], "after {bad} with {press_m:?}");
        }
    }
}

/// Defect (architect review, finding 2): `lasso_line` emits one dash per 7 px
/// of line with no cap, so a finite far-away pointer makes `draw_list` build
/// billions of triangles. Run on its own, with a modest 5e5 mm line, it
/// already allocates tens of millions of vertices.
#[test]
fn a_far_pointer_during_a_lasso_does_not_blow_up_the_draw_list() {
    let mut s = session();
    hold(&mut s, pt(450.0, 300.0), ALT);
    s.pointer_down(pt(450.0, 300.0), false);
    hold(&mut s, pt(5e5, 300.0), ALT);
    assert!(s.draw_list().triangle_count() < 1_000_000);
}

/// Defect (architect review, finding 1): per the accepted specs a Ctrl press
/// inside the sole selected object's box arms the remove marquee.
#[test]
fn ctrl_inside_the_sole_selected_box_arms_the_remove_marquee() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0)); // B selected
    // Inside B's box, away from its outline and from its centre handle (350,
    // 50), where Ctrl still copies (`edit-interaction-polish` criterion 38).
    let inside = pt(320.0, 30.0);
    hold(&mut s, inside, CTRL);
    let b = s.move_indicators();
    assert!(b.remove_badge && !b.copy_badge, "minus badge, no plus");
    let n = doc_of(&s).object_ids().len();
    s.pointer_down(inside, false);
    hold(&mut s, pt(250.0, 150.0), CTRL);
    assert!(legend(&s).unwrap_or_default().contains("Remove"));
    s.pointer_up(pt(250.0, 150.0), false, true);
    hold(&mut s, pt(250.0, 150.0), NONE);
    assert_eq!(doc_of(&s).object_ids().len(), n, "nothing copied");
    assert_eq!(origin_of(&s, 1), (300.0, 0.0), "nothing moved");
    assert_eq!(
        s.selected_object_count(),
        0,
        "B (touched by the leftward box) was removed"
    );
}

#[test]
fn a_huge_but_finite_lasso_release_finishes_and_selects_what_it_crosses() {
    let got = within(30, || {
        let mut s = session();
        hold(&mut s, pt(-1e7, 50.0), ALT);
        s.pointer_down(pt(-1e7, 50.0), false);
        hold(&mut s, pt(1e7, 50.0), ALT);
        s.pointer_up(pt(1e7, 50.0), false, false);
        selected(&mut s)
    });
    assert_eq!(got, vec!["A", "B", "C"]);
}

#[test]
fn a_zero_size_marquee_back_at_the_press_point_is_handled() {
    let mut s = session();
    click(&mut s, pt(300.0, 50.0));
    let p = pt(450.0, 300.0);
    hold(&mut s, p, NONE);
    s.pointer_down(p, false);
    hold(&mut s, pt(700.0, 700.0), NONE);
    hold(&mut s, p, NONE);
    s.pointer_up(p, false, false);
    assert!(s.selected_object_count() <= 1);
}

#[test]
fn a_long_lasso_with_many_points_keeps_every_frame_cheap() {
    let t = within(120, || {
        let mut s = session();
        let from = pt(-100.0, 700.0);
        hold(&mut s, from, ALT);
        s.pointer_down(from, false);
        let start = Instant::now();
        for i in 0..5000 {
            let x = -100.0 + f64::from(i) * 0.3;
            hold(
                &mut s,
                pt(x, 700.0 + (f64::from(i) * 0.05).sin() * 30.0),
                ALT,
            );
            if i % 250 == 0 {
                let _ = s.draw_list();
                let _ = legend(&s);
            }
        }
        let elapsed = start.elapsed();
        s.pointer_up(pt(1400.0, 700.0), false, false);
        elapsed
    });
    assert!(t < Duration::from_secs(60), "5000 moves took {t:?}");
}

fn crowded_doc(columns: u32, rows: u32) -> Document {
    let d = Document::new(1);
    for r in 0..rows {
        for c in 0..columns {
            let _ = d.create_rect(RectBounds {
                origin: pt(f64::from(c) * 12.0, f64::from(r) * 12.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            });
        }
    }
    d
}

#[test]
fn thousands_of_objects_marquee_lasso_and_alt_click_through_the_session() {
    let (marquee, contained, lasso, cycled) = within(300, || {
        let d = crowded_doc(60, 50); // 3000 squares
        let mut s = open(&d);
        let t0 = Instant::now();
        drag(&mut s, pt(-20.0, -20.0), pt(2000.0, 2000.0), NONE, NONE);
        let all = s.selected_object_count();
        let marquee = t0.elapsed();
        s.escape();
        let t1 = Instant::now();
        // A lasso along the top row of 60 squares.
        drag(&mut s, pt(-20.0, 5.0), pt(800.0, 5.0), ALT, ALT);
        let row = s.selected_object_count();
        let lasso = t1.elapsed();
        let t2 = Instant::now();
        // Alt-clicks in the 2 mm gap between two squares, many times.
        s.escape();
        click(&mut s, pt(11.0, 5.0));
        for _ in 0..20 {
            click_m(&mut s, pt(11.0, 5.0), ALT);
        }
        let cycled = t2.elapsed();
        (marquee, (all, row), lasso, cycled)
    });
    assert_eq!(contained.0, 3000, "everything is inside a 2000 mm box");
    assert_eq!(contained.1, 60, "one row");
    for (what, t) in [("marquee", marquee), ("lasso", lasso), ("cycle", cycled)] {
        assert!(t < Duration::from_secs(120), "{what} took {t:?}");
    }
}

/// What the revived marquee does: a click elsewhere after returning to the
/// Select tool is applied as the old marquee's release.
#[test]
fn after_a_tool_round_trip_a_plain_click_is_just_a_click() {
    let mut s = session();
    hold(&mut s, FROM, NONE);
    s.pointer_down(FROM, false);
    hold(&mut s, pt(900.0, 700.0), NONE);
    s.set_tool(Tool::Node);
    s.pointer_up(pt(900.0, 700.0), false, false);
    s.set_tool(Tool::Select);
    click(&mut s, pt(600.0, 50.0));
    assert_eq!(selected(&mut s), vec!["C"]);
}
