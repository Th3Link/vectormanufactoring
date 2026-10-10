//! Segment drag bending in the editor session (`specs/0031-segment-drag-bending`): the press,
//! the 3 px threshold and the 4 px tolerance, the shape rule through the node kinds, one commit
//! per bend, Shift, Escape, the hover band, and the blue preview with its readout and badge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::similar_names
)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, Length, NewAnchor, NodeId, Point, RectBounds, Vec2, pack,
    unpack,
};
use curvyo_editor_wasm::{KeyInput, Session, Tool};
use curvyo_render_core::{DrawList, RgbaColor};
use curvyo_ui_core::Axis;

const ACCENT: (u8, u8, u8) = (0x2F, 0x6F, 0xEE);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn id(n: u64) -> AnchorId {
    AnchorId::new(1, n)
}

fn anchor(n: u64, x: f64, y: f64, kind: AnchorKind) -> NewAnchor {
    NewAnchor {
        id: id(n),
        point: pt(x, y),
        handle_in: Vec2::ZERO,
        handle_out: Vec2::ZERO,
        kind,
    }
}

/// A node-tool session on `document`, the first path selected as the segment under test later.
fn node_session(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session.set_tool(Tool::Node);
    session
}

/// The straight segment A (0, 0) to B (200, 0) of an open path, with a third node C (400, 0) so
/// B can carry a kind.
fn line_path(kinds: [AnchorKind; 3]) -> (Document, NodeId) {
    let document = Document::new(1);
    let path = document.create_path(
        &[
            anchor(1, 0.0, 0.0, kinds[0]),
            anchor(2, 200.0, 0.0, kinds[1]),
            anchor(3, 400.0, 0.0, kinds[2]),
        ],
        false,
    );
    (document, path)
}

fn px(session: &Session, pixels: f64) -> f64 {
    pixels / session.view().scale()
}

fn press(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
}

fn drag_to(session: &mut Session, to: Point, shift: bool) {
    session.pointer_hover(to, shift, false);
}

fn release(session: &mut Session, at: Point, shift: bool) {
    session.pointer_hover(at, shift, false);
    session.pointer_up(at, shift, false);
}

fn bend(session: &mut Session, from: Point, to: Point) {
    press(session, from);
    drag_to(session, to, false);
    release(session, to, false);
}

fn reread(session: &Session) -> Document {
    unpack(9, &session.pack("0.1.0").unwrap()).unwrap()
}

fn handles(session: &Session, n: u64) -> (Vec2, Vec2) {
    let document = reread(session);
    let path = document.object_ids()[0];
    let snapshot = document.path(path).unwrap();
    let a = snapshot.anchors.iter().find(|a| a.id == id(n)).unwrap();
    (a.handle_in, a.handle_out)
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

fn labels(session: &Session) -> Vec<String> {
    let loro = loro::LoroDoc::new();
    loro.import(&reread(session).export_loro_snapshot().unwrap())
        .unwrap();
    let frontiers: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut changes: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&frontiers, &mut |change| {
        changes.push((
            change.lamport,
            change.message.map(|m| m.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    changes.sort();
    changes.into_iter().map(|(_, label)| label).collect()
}

fn count_color(list: &DrawList, color: (u8, u8, u8), alpha: u8) -> usize {
    list.triangles
        .iter()
        .filter(|vertex| {
            let RgbaColor { r, g, b, a } = vertex.color;
            (r, g, b) == color && a == alpha
        })
        .count()
}

fn key(session: &mut Session, name: &str) {
    session.key_down(KeyInput {
        key: name,
        ..KeyInput::default()
    });
}

/// Criteria 1, 2, 18, 19: a click selects the segment and writes nothing; a bend is one commit
/// labelled `bend_segment`, the segment stays selected, and "Make line" is on offer.
#[test]
fn a_click_selects_and_a_drag_bends_in_one_commit() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    let before = labels(&session);

    press(&mut session, pt(100.0, 0.0));
    release(&mut session, pt(100.0, 0.0), false);
    assert_eq!(labels(&session), before, "a click writes nothing");
    assert!(
        session.node_toolbar_state().can_insert,
        "the segment is selected"
    );
    assert_eq!(handles(&session, 1), (Vec2::ZERO, Vec2::ZERO));

    bend(&mut session, pt(100.0, 0.0), pt(100.0, -30.0));
    let after = labels(&session);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("bend_segment"));
    // 200 mm segment grabbed in the middle: k = 4/3, d = (0, -30): the handles gain (0, -40).
    assert!(
        close(handles(&session, 1).1, v(200.0 / 3.0, -40.0)),
        "{:?}",
        handles(&session, 1)
    );
    assert!(close(handles(&session, 2).0, v(-200.0 / 3.0, -40.0)));
    let state = session.node_toolbar_state();
    assert!(state.can_make_line && !state.can_make_curve, "a curve now");
    assert!(state.can_insert, "still selected");
    assert!(!state.can_delete, "no node is selected");
}

/// Criterion 19: dragged away and back to the press point writes nothing.
#[test]
fn a_drag_back_to_the_press_point_writes_nothing() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    let before = labels(&session);
    press(&mut session, pt(100.0, 0.0));
    drag_to(&mut session, pt(100.0, -30.0), false);
    release(&mut session, pt(100.0, 0.0), false);
    assert_eq!(labels(&session), before);
    assert_eq!(
        handles(&session, 1),
        (Vec2::ZERO, Vec2::ZERO),
        "a line stays a line"
    );
}

/// Criteria 3 and 19: Shift limits d to the axis of the larger part; zero on the other axis.
#[test]
fn shift_limits_the_bend_to_one_axis() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    press(&mut session, pt(100.0, 0.0));
    drag_to(&mut session, pt(100.0 + 10.0, -30.0), true);
    release(&mut session, pt(100.0 + 10.0, -30.0), true);
    // |dy| > |dx|: vertical only, the same result as the plain drag of the test above.
    assert!(
        close(handles(&session, 1).1, v(200.0 / 3.0, -40.0)),
        "{:?}",
        handles(&session, 1)
    );
}

/// Criteria 4 and 5: the press uses the Node tool's 4 px tolerance at every zoom, and a node
/// within 16 px wins over the segment.
#[test]
fn the_press_uses_the_four_pixel_tolerance_and_nodes_win_near_them() {
    for percent in [100.0_f64, 800.0] {
        let (document, _) = line_path([AnchorKind::Corner; 3]);
        let mut session = node_session(&document);
        let factor = percent / f64::from(i32::try_from(session.zoom_percent()).unwrap());
        session.wheel(0.0, -400.0 * factor.log2(), 0.0, 0.0, false, true);
        let mid = 100.0;
        let near = px(&session, 3.5);
        let far = px(&session, 4.5);
        let twenty = px(&session, 20.0);
        // 3.5 px away: bends.
        bend(&mut session, pt(mid, near), pt(mid, near - twenty));
        assert_ne!(
            handles(&session, 1).1,
            Vec2::ZERO,
            "3.5 px bends at {percent} %"
        );
        // A fresh session: 4.5 px away hits nothing.
        let (document, _) = line_path([AnchorKind::Corner; 3]);
        let mut session = node_session(&document);
        session.wheel(0.0, -400.0 * factor.log2(), 0.0, 0.0, false, true);
        let before = labels(&session);
        bend(&mut session, pt(mid, far), pt(mid, far - twenty));
        assert_eq!(
            labels(&session),
            before,
            "4.5 px bends nothing at {percent} %"
        );
    }
}

/// Criterion 5: a press 10 px from an end node on the segment moves the node.
#[test]
fn a_press_near_a_node_moves_the_node() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    let near = px(&session, 10.0);
    press(&mut session, pt(near, 0.0));
    drag_to(&mut session, pt(near, -20.0), false);
    release(&mut session, pt(near, -20.0), false);
    assert_eq!(
        handles(&session, 1),
        (Vec2::ZERO, Vec2::ZERO),
        "nothing was bent"
    );
    let snapshot = reread(&session)
        .path(reread(&session).object_ids()[0])
        .unwrap();
    assert!(
        (snapshot.anchors[0].point.y + 20.0).abs() < 1e-6,
        "the node moved"
    );
}

/// Criterion 12: the far handle of a node follows its kind, and the segment on the other side
/// of it changes shape.
#[test]
fn a_symmetric_node_carries_the_bend_into_the_next_segment() {
    let (document, _) = line_path([
        AnchorKind::Corner,
        AnchorKind::Symmetric,
        AnchorKind::Corner,
    ]);
    let mut session = node_session(&document);
    bend(&mut session, pt(100.0, 0.0), pt(100.0, -30.0));
    let (b_in, b_out) = handles(&session, 2);
    assert!(close(b_in, v(-200.0 / 3.0, -40.0)));
    assert!(
        close(b_out, v(200.0 / 3.0, 40.0)),
        "mirrored: BC is a curve now: {b_out:?}"
    );
}

/// Criterion 6a: the closing segment of a closed path of two nodes can be pressed and bent, and
/// the other segment is not touched.
#[test]
fn the_closing_segment_of_a_two_node_closed_path_is_reachable() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            anchor(1, 0.0, 0.0, AnchorKind::Corner),
            anchor(2, 200.0, 0.0, AnchorKind::Corner),
        ],
        true,
    );
    let mut session = node_session(&document);
    // Both segments run along the same line; bend the first (pressed at the middle) and check
    // that the handles of the A to B segment change: A's outgoing and B's incoming.
    bend(&mut session, pt(100.0, 0.0), pt(100.0, -30.0));
    let (a_in, a_out) = handles(&session, 1);
    let (b_in, b_out) = handles(&session, 2);
    // The press hits one of the two coincident segments; exactly one pair of handles changed.
    let first = a_out != Vec2::ZERO && b_in != Vec2::ZERO;
    let closing = b_out != Vec2::ZERO && a_in != Vec2::ZERO;
    assert!(
        first ^ closing,
        "one segment was bent: {a_in:?} {a_out:?} {b_in:?} {b_out:?}"
    );
}

/// Criterion 6: a primitive and a compound path are not bent in the Node tool.
#[test]
fn primitives_are_not_bent() {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(60.0),
    });
    let mut session = node_session(&document);
    let before = labels(&session);
    bend(&mut session, pt(50.0, 0.0), pt(50.0, -30.0));
    assert_eq!(labels(&session), before);
}

/// Criterion 20: Escape ends the bend, writes nothing and keeps the segment selected; the
/// release after it writes nothing.
#[test]
fn escape_cancels_the_bend() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    let before = labels(&session);
    press(&mut session, pt(100.0, 0.0));
    drag_to(&mut session, pt(100.0, -30.0), false);
    assert!(session.live_readout().is_some(), "a bend runs");
    key(&mut session, "Escape");
    assert!(session.live_readout().is_none(), "the readout is gone");
    release(&mut session, pt(100.0, -30.0), false);
    assert_eq!(labels(&session), before);
    assert!(
        session.node_toolbar_state().can_insert,
        "the segment stays selected"
    );
}

/// Criterion 16: while the bend runs the artwork is the committed one, the blue preview is
/// drawn, the readout says "Δ x, y mm", and the end nodes show their handles.
#[test]
fn the_preview_readout_and_badge() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    press(&mut session, pt(100.0, 0.0));
    let at_press = session.draw_list();
    let black_before = count_color(&at_press, (0, 0, 0), 255);
    assert!(
        session.live_readout().is_none(),
        "inside the threshold it is a click"
    );

    drag_to(&mut session, pt(110.0, -30.0), false);
    let during = session.draw_list();
    assert_eq!(
        count_color(&during, (0, 0, 0), 255),
        black_before,
        "black old is as committed"
    );
    // The handle of A at its live position: the segment is grabbed in the middle, so the outgoing
    // handle is (200 / 3, 0) plus k d = (4/3) * (10, -30).
    let handle_end = pt(200.0 / 3.0 + 40.0 / 3.0, -40.0);
    assert!(
        during.triangles.iter().any(|vertex| {
            let RgbaColor { r, g, b, a } = vertex.color;
            (r, g, b, a) == (ACCENT.0, ACCENT.1, ACCENT.2, 255)
                && vertex.position.vector_to(handle_end).length() < 3.0
        }),
        "the live handle of the end node is drawn"
    );
    let readout = session.live_readout().unwrap();
    assert_eq!(readout.text, "Δ 10.0, \u{2212}30.0 mm");
    assert_eq!(session.move_indicators().lock, None);

    drag_to(&mut session, pt(110.0, -30.0), true);
    assert_eq!(
        session.live_readout().unwrap().text,
        "Δ 0.0, \u{2212}30.0 mm"
    );
    assert_eq!(session.move_indicators().lock, Some(Axis::Y));

    release(&mut session, pt(110.0, -30.0), true);
    assert!(session.live_readout().is_none());
    assert_eq!(session.move_indicators().lock, None);
}

/// Criterion 15: the hover band shows over a segment a press would bend, not on the selected
/// segment, not near a node, not during a drag.
#[test]
fn the_hover_band_follows_what_a_press_would_bend() {
    let (document, _) = line_path([AnchorKind::Corner; 3]);
    let mut session = node_session(&document);
    let band = |list: &DrawList| count_color(list, ACCENT, 128);

    session.pointer_hover(pt(100.0, px(&session, 20.0)), false, false);
    assert_eq!(band(&session.draw_list()), 0, "20 px away: no band");
    session.pointer_hover(pt(100.0, px(&session, 2.0)), false, false);
    assert!(band(&session.draw_list()) > 0, "within 4 px: the band");
    session.pointer_hover(pt(px(&session, 10.0), 0.0), false, false);
    assert_eq!(
        band(&session.draw_list()),
        0,
        "within the node radius: none"
    );

    press(&mut session, pt(100.0, 0.0));
    release(&mut session, pt(100.0, 0.0), false);
    session.pointer_hover(pt(100.0, px(&session, 2.0)), false, false);
    assert_eq!(
        band(&session.draw_list()),
        0,
        "the selected segment keeps its own look"
    );

    // Another segment, then a drag: no band while it runs.
    session.pointer_hover(pt(300.0, px(&session, 2.0)), false, false);
    assert!(band(&session.draw_list()) > 0);
    press(&mut session, pt(300.0, 0.0));
    drag_to(&mut session, pt(300.0, -30.0), false);
    assert_eq!(band(&session.draw_list()), 0, "no band during a drag");
}
