//! Independent session-level acceptance tests for `0031-segment-drag-bending`, written from the
//! specification before the implementation was read. Pointer coordinates are document millimetres;
//! screen pixels are converted with the live view scale. The document is read back through
//! `pack`/`unpack`, the commits through the Loro change log.

#![allow(
    unused_must_use,
    clippy::manual_midpoint,
    clippy::ref_option,
    clippy::bool_assert_comparison,
    clippy::match_same_arms,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::used_underscore_binding,
    clippy::needless_pass_by_value,
    clippy::manual_assert,
    clippy::trivially_copy_pass_by_ref
)]

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, EllipseFrame, Length, NewAnchor, ObjectSnapshot, PathSnapshot,
    Point, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, Session, Tool};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(1);

fn aid() -> AnchorId {
    AnchorId::new(31, NEXT.fetch_add(1, Ordering::SeqCst))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn anchor(x: f64, y: f64, kind: AnchorKind, hin: Vec2, hout: Vec2) -> NewAnchor {
    NewAnchor {
        id: aid(),
        point: pt(x, y),
        handle_in: hin,
        handle_out: hout,
        kind,
    }
}

fn corner(x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(aid(), pt(x, y))
}

fn open_session(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.resize_viewport(1600.0, 1000.0);
    s.set_tool(Tool::Node);
    s
}

fn reread(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn json(s: &Session) -> Vec<u8> {
    reread(s).export_json().unwrap()
}

fn first_path(s: &Session) -> PathSnapshot {
    let d = reread(s);
    let id = d.object_ids()[0];
    d.path(id).expect("a path")
}

/// One millimetre expressed in screen pixels at the current view.
fn px_to_mm(s: &Session, px: f64) -> f64 {
    px / s.view().scale()
}

fn zoom_to(s: &mut Session, factor: f64) {
    s.wheel(0.0, -400.0 * factor.log2(), 0.0, 0.0, false, true);
}

fn hover(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
}

fn press(s: &mut Session, p: Point, shift: bool) {
    hover(s, p, shift);
    s.pointer_down(p, shift);
}

fn release(s: &mut Session, p: Point, shift: bool) {
    s.pointer_up(p, shift, false);
}

/// A full drag from `from` to `to` through a few intermediate moves.
fn drag(s: &mut Session, from: Point, to: Point, shift: bool) {
    press(s, from, shift);
    for i in 1..=4 {
        let f = f64::from(i) / 4.0;
        hover(
            s,
            pt(from.x + (to.x - from.x) * f, from.y + (to.y - from.y) * f),
            shift,
        );
    }
    release(s, to, shift);
}

fn click(s: &mut Session, p: Point) {
    press(s, p, false);
    release(s, p, false);
}

fn labels(d: &Document) -> Vec<String> {
    let loro = loro::LoroDoc::new();
    loro.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let ids: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|s| s.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, s)| s).collect()
}

fn close(a: Vec2, b: Vec2, tol: f64) -> bool {
    (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol
}

fn assert_vec(label: &str, got: Vec2, want: Vec2, tol: f64) {
    assert!(close(got, want, tol), "{label}: got {got:?}, want {want:?}");
}

/// An open straight path with the given kinds, 90 mm per segment, all handles zero.
fn straight_path(kinds: &[AnchorKind]) -> Document {
    let d = Document::new(1);
    let anchors: Vec<NewAnchor> = kinds
        .iter()
        .enumerate()
        .map(|(i, &k)| anchor(90.0 * i as f64, 0.0, k, Vec2::ZERO, Vec2::ZERO))
        .collect();
    d.create_path(&anchors, false);
    d
}

fn bend_labels(s: &Session) -> usize {
    labels(&reread(s))
        .iter()
        .filter(|l| l.as_str() == "bend_segment")
        .count()
}

// ---------------------------------------------------------------------------------------------
// Criteria 1, 2, 3, 8, 18: the basic bend
// ---------------------------------------------------------------------------------------------

#[test]
fn c8_c18_line_bend_writes_the_documented_handles_in_one_commit() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = labels(&reread(&s)).len();
    // Press on the segment middle, drag to (45, -30): the first move is more than 3 px, the
    // displacement is measured from the press point and not from where the dead zone ended.
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -1.0), false);
    hover(&mut s, pt(45.0, -15.0), false);
    hover(&mut s, pt(45.0, -30.0), false);
    release(&mut s, pt(45.0, -30.0), false);
    let p = first_path(&s);
    assert_eq!(p.anchors.len(), 2);
    assert_eq!(p.anchors[0].point, pt(0.0, 0.0), "no node moves");
    assert_eq!(p.anchors[1].point, pt(90.0, 0.0), "no node moves");
    assert_vec("A out", p.anchors[0].handle_out, v(30.0, -40.0), 1e-3);
    assert_vec("B in", p.anchors[1].handle_in, v(-30.0, -40.0), 1e-3);
    let l = labels(&reread(&s));
    assert_eq!(l.len(), before + 1, "exactly one commit: {l:?}");
    assert_eq!(l.last().unwrap(), "bend_segment");
    // The segment stays selected, no node is: the bar offers Make line, not Make curve.
    let bar = s.node_toolbar_state();
    assert!(bar.can_make_line, "bent segment is a curve now");
    assert!(!bar.can_make_curve);
    assert!(!bar.can_delete, "no node is selected after a bend");
}

#[test]
fn c2_a_click_selects_the_segment_and_writes_nothing() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    click(&mut s, pt(45.0, 0.0));
    assert!(s.node_toolbar_state().can_make_curve, "segment is selected");
    assert_eq!(json(&s), before);
    assert_eq!(labels(&reread(&s)).len(), n, "no commit for a click");
}

#[test]
fn c2_c3_dead_zone_is_more_than_three_pixels() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    // Exactly 3 px and 2.9 px away, then released there: still a click.
    for px in [2.9, 3.0] {
        let dy = -px_to_mm(&s, px);
        press(&mut s, pt(45.0, 0.0), false);
        hover(&mut s, pt(45.0, dy), false);
        release(&mut s, pt(45.0, dy), false);
        assert_eq!(json(&s), before, "{px} px is a click");
    }
    // 3.5 px away is a bend.
    let dy = -px_to_mm(&s, 3.5);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, dy), false);
    release(&mut s, pt(45.0, dy), false);
    let p = first_path(&s);
    assert!(
        p.anchors[0].handle_out.length() > 0.0,
        "3.5 px should bend: {:?}",
        p.anchors[0]
    );
}

#[test]
fn c2_wandering_out_and_back_inside_the_dead_zone_is_still_a_click() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    let tiny = px_to_mm(&s, 2.0);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0 + tiny, 0.0), false);
    hover(&mut s, pt(45.0, tiny), false);
    release(&mut s, pt(45.0, 0.0), false);
    assert_eq!(json(&s), before);
}

#[test]
fn c19_dragging_away_and_back_to_the_press_point_writes_nothing() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -20.0), false);
    hover(&mut s, pt(45.0, -10.0), false);
    hover(&mut s, pt(45.0, 0.0), false);
    release(&mut s, pt(45.0, 0.0), false);
    assert_eq!(json(&s), before, "a line stays a line, no stored handles");
    assert_eq!(labels(&reread(&s)).len(), n);
    assert_eq!(first_path(&s).anchors[0].handle_out, Vec2::ZERO);
}

#[test]
fn c19_shift_that_makes_d_zero_writes_nothing() {
    // d = (0, 0) after the axis lock needs |dx| == |dy| == 0 only; use a tie on an axis-locked
    // drag instead: with Shift and exactly zero displacement along the winning axis there is
    // nothing to write. A real zero is the drag back to the press point (above); here Shift is
    // held for a drag that ends on the press point.
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    press(&mut s, pt(45.0, 0.0), true);
    hover(&mut s, pt(60.0, -5.0), true);
    hover(&mut s, pt(45.0, 0.0), true);
    release(&mut s, pt(45.0, 0.0), true);
    assert_eq!(json(&s), before);
}

#[test]
fn c19_a_bent_line_stays_a_curve_even_when_dragged_to_nearly_straight() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -20.0), false);
    hover(&mut s, pt(45.0, -0.01), false);
    release(&mut s, pt(45.0, -0.01), false);
    let p = first_path(&s);
    assert!(p.anchors[0].handle_out.y < -0.0, "{:?}", p.anchors[0]);
    assert!(s.node_toolbar_state().can_make_line);
}

// ---------------------------------------------------------------------------------------------
// Criterion 11: an existing curve is reshaped
// ---------------------------------------------------------------------------------------------

#[test]
fn c11_a_curve_is_reshaped_by_adding_k_d_to_both_stored_handles() {
    let d = Document::new(1);
    d.create_path(
        &[
            anchor(0.0, 0.0, AnchorKind::Corner, Vec2::ZERO, v(20.0, 40.0)),
            anchor(90.0, 0.0, AnchorKind::Corner, v(-20.0, 40.0), Vec2::ZERO),
        ],
        false,
    );
    let mut s = open_session(&d);
    // The curve point at t = 0.5 is (45, 30); the nearest point to it is itself.
    drag(&mut s, pt(45.0, 30.0), pt(45.0, 24.0), false);
    let p = first_path(&s);
    // d = (0, -6), k = 4/3: both handles move by (0, -8).
    assert_vec("A out", p.anchors[0].handle_out, v(20.0, 32.0), 5e-2);
    assert_vec("B in", p.anchors[1].handle_in, v(-20.0, 32.0), 5e-2);
}

// ---------------------------------------------------------------------------------------------
// Criterion 12: node types
// ---------------------------------------------------------------------------------------------

#[test]
fn c12_symmetric_neighbour_mirrors_and_the_next_segment_changes_shape() {
    let d = straight_path(&[
        AnchorKind::Corner,
        AnchorKind::Symmetric,
        AnchorKind::Corner,
    ]);
    let mut s = open_session(&d);
    drag(&mut s, pt(45.0, 0.0), pt(45.0, -30.0), false);
    let p = first_path(&s);
    assert_vec("A out", p.anchors[0].handle_out, v(30.0, -40.0), 1e-3);
    assert_vec("B in", p.anchors[1].handle_in, v(-30.0, -40.0), 1e-3);
    assert_vec(
        "B out mirrors",
        p.anchors[1].handle_out,
        v(30.0, 40.0),
        1e-3,
    );
    assert_eq!(p.anchors[2].handle_in, Vec2::ZERO, "C is untouched");
    // BC is a curve now: selecting it offers Make line.
    click(&mut s, pt(146.25, 15.0));
    assert!(
        s.node_toolbar_state().can_make_line,
        "BC changed shape with the symmetric node"
    );
}

#[test]
fn c12_corner_neighbour_leaves_its_other_handle_and_the_next_segment_a_line() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    drag(&mut s, pt(45.0, 0.0), pt(45.0, -30.0), false);
    let p = first_path(&s);
    assert_vec("B in", p.anchors[1].handle_in, v(-30.0, -40.0), 1e-3);
    assert_eq!(p.anchors[1].handle_out, Vec2::ZERO);
    click(&mut s, pt(135.0, 0.0));
    assert!(s.node_toolbar_state().can_make_curve, "BC stays a line");
}

#[test]
fn c12_asymmetric_keeps_the_other_handles_length_and_turns_it_opposite() {
    let d = Document::new(1);
    d.create_path(
        &[
            corner(0.0, 0.0),
            anchor(
                90.0,
                0.0,
                AnchorKind::Asymmetric,
                v(-10.0, 0.0),
                v(25.0, 0.0),
            ),
            corner(180.0, 0.0),
        ],
        false,
    );
    let mut s = open_session(&d);
    // AB: both handles zero at A, B.in = (-10, 0) so not a pure line. Press at the chord middle
    // is no longer on the curve; use the straight-handle curve's own midpoint instead.
    // P0=(0,0) P1=(0,0) P2=(80,0) P3=(90,0): all on y=0, so (45,0) lies on the curve.
    drag(&mut s, pt(45.0, 0.0), pt(45.0, -20.0), false);
    let p = first_path(&s);
    let hin = p.anchors[1].handle_in;
    let hout = p.anchors[1].handle_out;
    assert!(hin.y < 0.0, "B.in was bent upward: {hin:?}");
    assert!(
        (hout.length() - 25.0).abs() < 1e-6,
        "length of the other handle kept: {hout:?}"
    );
    // Opposite direction: hout is anti-parallel to hin.
    let cross = hin.x * hout.y - hin.y * hout.x;
    let dot = hin.x * hout.x + hin.y * hout.y;
    assert!(
        cross.abs() < 1e-6 * hin.length() * hout.length(),
        "collinear"
    );
    assert!(dot < 0.0, "opposite direction");
}

#[test]
fn c12_the_hidden_handle_of_an_open_end_node_is_written_by_the_kind_rule() {
    let d = straight_path(&[AnchorKind::Symmetric, AnchorKind::Corner]);
    let mut s = open_session(&d);
    drag(&mut s, pt(45.0, 0.0), pt(45.0, -30.0), false);
    let p = first_path(&s);
    assert_vec("A out", p.anchors[0].handle_out, v(30.0, -40.0), 1e-3);
    assert_vec("A hidden in", p.anchors[0].handle_in, v(-30.0, 40.0), 1e-3);
}

#[test]
fn c12_closed_path_of_two_nodes_applies_the_rules_on_both_ends() {
    let d = Document::new(1);
    d.create_path(
        &[
            anchor(
                0.0,
                0.0,
                AnchorKind::Symmetric,
                v(-20.0, 20.0),
                v(20.0, -20.0),
            ),
            anchor(
                90.0,
                0.0,
                AnchorKind::Symmetric,
                v(-20.0, -20.0),
                v(20.0, 20.0),
            ),
        ],
        true,
    );
    let mut s = open_session(&d);
    // AB: P0 (0,0) P1 (20,-20) P2 (70,-20) P3 (90,0): the curve point at t = 0.5 is (45, -15).
    // The closing segment passes (45, 15), 30 mm away.
    drag(&mut s, pt(45.0, -15.0), pt(45.0, -27.0), false);
    let p = first_path(&s);
    assert_vec(
        "A.in mirrors A.out",
        p.anchors[0].handle_in,
        Vec2::new(-p.anchors[0].handle_out.x, -p.anchors[0].handle_out.y),
        1e-6,
    );
    assert_vec(
        "B.out mirrors B.in",
        p.anchors[1].handle_out,
        Vec2::new(-p.anchors[1].handle_in.x, -p.anchors[1].handle_in.y),
        1e-6,
    );
    assert!(
        p.anchors[0].handle_out.y < -20.0,
        "the bend pulled A.out upward: {:?}",
        p.anchors[0]
    );
    assert!(p.anchors[0].handle_in.y > 20.0, "the closing side followed");
}

// ---------------------------------------------------------------------------------------------
// Criteria 6, 6a: which segments can be bent
// ---------------------------------------------------------------------------------------------

#[test]
fn c6_the_closing_segment_of_a_closed_path_can_be_bent() {
    let d = Document::new(1);
    d.create_path(
        &[corner(0.0, 0.0), corner(90.0, 0.0), corner(90.0, 90.0)],
        true,
    );
    let mut s = open_session(&d);
    // Closing segment runs from (90, 90) to (0, 0): midpoint (45, 45).
    drag(&mut s, pt(45.0, 45.0), pt(25.0, 65.0), false);
    let p = first_path(&s);
    assert_ne!(p.anchors[2].handle_out, Vec2::ZERO, "last node's out moved");
    assert_ne!(p.anchors[0].handle_in, Vec2::ZERO, "first node's in moved");
    assert_eq!(p.anchors[0].handle_out, Vec2::ZERO);
    assert_eq!(p.anchors[1].handle_out, Vec2::ZERO);
    assert_eq!(p.anchors[2].handle_in, Vec2::ZERO);
}

/// Two nodes, both segments curved; each click selects its own segment only.
fn two_node_closed() -> Document {
    let d = Document::new(1);
    d.create_path(
        &[
            anchor(0.0, 0.0, AnchorKind::Corner, v(0.0, 30.0), v(0.0, -30.0)),
            anchor(100.0, 0.0, AnchorKind::Corner, v(0.0, -30.0), v(0.0, 30.0)),
        ],
        true,
    );
    d
}

#[test]
fn c6a_both_segments_of_a_two_node_closed_path_are_selectable_and_distinct() {
    // Segment 1 (A to B): P0 (0,0) P1 (0,-30) P2 (100,-30) P3 (100,0): apex (50, -22.5).
    // Closing segment (B to A): P0 (100,0) P1 (100,30) P2 (0,30) P3 (0,0): apex (50, 22.5).
    let mut s = open_session(&two_node_closed());
    click(&mut s, pt(50.0, -22.5));
    assert!(s.node_toolbar_state().can_make_line, "AB selected");
    s.make_line();
    let p = first_path(&s);
    assert_eq!(p.anchors[0].handle_out, Vec2::ZERO, "AB is a line now");
    assert_eq!(p.anchors[1].handle_in, Vec2::ZERO);
    assert_ne!(p.anchors[1].handle_out, Vec2::ZERO, "BA untouched");
    assert_ne!(p.anchors[0].handle_in, Vec2::ZERO, "BA untouched");

    let mut s = open_session(&two_node_closed());
    click(&mut s, pt(50.0, 22.5));
    assert!(s.node_toolbar_state().can_make_line, "BA selected");
    s.make_line();
    let p = first_path(&s);
    assert_eq!(p.anchors[1].handle_out, Vec2::ZERO, "BA is a line now");
    assert_eq!(p.anchors[0].handle_in, Vec2::ZERO);
    assert_ne!(p.anchors[0].handle_out, Vec2::ZERO, "AB untouched");
    assert_ne!(p.anchors[1].handle_in, Vec2::ZERO, "AB untouched");
}

#[test]
fn c6a_the_closing_segment_of_a_two_node_closed_path_can_be_bent() {
    let mut s = open_session(&two_node_closed());
    drag(&mut s, pt(50.0, 22.5), pt(50.0, 40.0), false);
    let p = first_path(&s);
    assert_eq!(p.anchors[0].handle_out, v(0.0, -30.0), "AB untouched");
    assert_eq!(p.anchors[1].handle_in, v(0.0, -30.0), "AB untouched");
    assert_ne!(p.anchors[1].handle_out, v(0.0, 30.0), "BA bent");
    assert_ne!(p.anchors[0].handle_in, v(0.0, 30.0), "BA bent");
}

#[test]
fn c6_a_primitive_cannot_be_bent() {
    let d = Document::new(1);
    d.create_ellipse(EllipseFrame {
        center: pt(50.0, 50.0),
        rx: Length::from_mm(40.0),
        ry: Length::from_mm(40.0),
    });
    let mut s = open_session(&d);
    let before = json(&s);
    drag(&mut s, pt(90.0, 50.0), pt(110.0, 60.0), false);
    // A press on a primitive does what it does today: it never bends and never rewrites it as a
    // path with handles.
    let after = reread(&s);
    assert!(
        !labels(&after).iter().any(|l| l == "bend_segment"),
        "no bend on a primitive"
    );
    let ids = after.object_ids();
    assert_eq!(ids.len(), 1);
    assert!(
        matches!(after.object(ids[0]), Some(ObjectSnapshot::Primitive(_))),
        "still a primitive"
    );
    let _ = before;
}

// ---------------------------------------------------------------------------------------------
// Criteria 4, 5: hit tolerance and hit order
// ---------------------------------------------------------------------------------------------

#[test]
fn c4_press_within_four_pixels_bends_beyond_four_does_not_at_100_and_800_percent() {
    for factor in [1.0, 8.0] {
        let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
        let mut s = open_session(&d);
        if factor > 1.0 {
            zoom_to(&mut s, factor);
        }
        let one_px = px_to_mm(&s, 1.0);
        let mm = |px: f64| px * one_px;
        // Near hit: 3.5 px off the segment in the middle, then 20 px of movement.
        let cx = 45.0;
        // At 800 % the segment is longer than the screen; the middle is still pointable in
        // document space.
        press(&mut s, pt(cx, mm(3.5)), false);
        hover(&mut s, pt(cx, mm(3.5 - 20.0)), false);
        release(&mut s, pt(cx, mm(3.5 - 20.0)), false);
        let p = first_path(&s);
        assert!(
            p.anchors[0].handle_out.length() > 0.0,
            "zoom {factor}: 3.5 px press bends"
        );

        // Far hit: 4.5 px away hits nothing.
        let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
        let mut s = open_session(&d);
        if factor > 1.0 {
            zoom_to(&mut s, factor);
        }
        let one_px = px_to_mm(&s, 1.0);
        let mm = |px: f64| px * one_px;
        let before = json(&s);
        press(&mut s, pt(cx, mm(4.5)), false);
        hover(&mut s, pt(cx, mm(4.5 - 20.0)), false);
        release(&mut s, pt(cx, mm(4.5 - 20.0)), false);
        assert_eq!(json(&s), before, "zoom {factor}: 4.5 px press hits nothing");
        assert!(
            !s.node_toolbar_state().can_make_curve && !s.node_toolbar_state().can_make_line,
            "zoom {factor}: no segment selected"
        );
    }
}

#[test]
fn c5_a_press_within_sixteen_pixels_of_a_node_moves_the_node_instead() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let ten = px_to_mm(&s, 10.0);
    drag(&mut s, pt(ten, 0.0), pt(ten, -20.0), false);
    let p = first_path(&s);
    assert_ne!(p.anchors[0].point, pt(0.0, 0.0), "the node moved");
    assert_eq!(p.anchors[0].handle_out, Vec2::ZERO, "no bend");
    assert_eq!(bend_labels(&s), 0);
}

#[test]
fn c5_a_segment_shorter_than_two_node_radii_cannot_be_bent() {
    // A 5 mm segment is about 19 px long at 100 %: every point on it is within 16 px of a node.
    let d = Document::new(1);
    d.create_path(&[corner(0.0, 0.0), corner(5.0, 0.0)], false);
    let mut s = open_session(&d);
    drag(&mut s, pt(2.5, 0.0), pt(2.5, -10.0), false);
    assert_eq!(bend_labels(&s), 0, "never a bend");
    let p = first_path(&s);
    assert_eq!(p.anchors[0].handle_out, Vec2::ZERO);
    assert_eq!(p.anchors[1].handle_in, Vec2::ZERO);
}

#[test]
fn hit_order_a_handle_wins_over_the_segment_under_it() {
    // A curve whose handles lie on the chord: the selected node's handle end sits on the segment.
    let d = Document::new(1);
    d.create_path(
        &[
            anchor(0.0, 0.0, AnchorKind::Corner, Vec2::ZERO, v(30.0, 0.0)),
            anchor(120.0, 0.0, AnchorKind::Corner, v(-30.0, 0.0), Vec2::ZERO),
        ],
        false,
    );
    let mut s = open_session(&d);
    click(&mut s, pt(120.0, 0.0)); // select B so its incoming handle (at 90, 0) is shown
    drag(&mut s, pt(90.0, 0.0), pt(90.0, -20.0), false);
    let p = first_path(&s);
    assert_eq!(
        p.anchors[0].handle_out,
        v(30.0, 0.0),
        "A's handle did not move: no bend"
    );
    assert_ne!(
        p.anchors[1].handle_in,
        v(-30.0, 0.0),
        "the handle was dragged"
    );
    assert_eq!(bend_labels(&s), 0);
}

// ---------------------------------------------------------------------------------------------
// Criterion 13: Shift axis lock and the readout
// ---------------------------------------------------------------------------------------------

#[test]
fn c13_shift_limits_d_to_the_farther_axis() {
    // d = (10, -30): vertical wins.
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    drag(&mut s, pt(45.0, 0.0), pt(55.0, -30.0), true);
    let p = first_path(&s);
    assert_vec("A out", p.anchors[0].handle_out, v(30.0, -40.0), 1e-3);
    assert_vec("B in", p.anchors[1].handle_in, v(-30.0, -40.0), 1e-3);
    // d = (30, -10): horizontal wins, k d = (40, 0): the handles slide along the line.
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    drag(&mut s, pt(45.0, 0.0), pt(75.0, -10.0), true);
    let p = first_path(&s);
    assert_vec("A out", p.anchors[0].handle_out, v(30.0 + 40.0, 0.0), 1e-3);
    assert_vec("B in", p.anchors[1].handle_in, v(-30.0 + 40.0, 0.0), 1e-3);
}

#[test]
fn c13_shift_is_read_live_on_every_move_and_a_release_unlocks() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(55.0, -30.0), true); // locked vertical
    hover(&mut s, pt(55.0, -30.0), false); // released: the free d = (10, -30)
    release(&mut s, pt(55.0, -30.0), false);
    let p = first_path(&s);
    // free d = (10, -30): k d = (13.33, -40)
    assert_vec(
        "A out",
        p.anchors[0].handle_out,
        v(30.0 + 40.0 / 3.0, -40.0),
        1e-3,
    );
}

#[test]
fn c13_shift_at_the_press_has_no_effect_on_a_segment() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    press(&mut s, pt(45.0, 0.0), true);
    hover(&mut s, pt(55.0, -30.0), false);
    release(&mut s, pt(55.0, -30.0), false);
    let p = first_path(&s);
    assert_vec(
        "A out",
        p.anchors[0].handle_out,
        v(30.0 + 40.0 / 3.0, -40.0),
        1e-3,
    );
}

#[test]
fn c16_readout_shows_the_displacement_with_a_real_minus_sign() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -15.0), false);
    hover(&mut s, pt(57.5, -3.0), false);
    let r = s.live_readout().expect("a readout during the bend");
    assert_eq!(r.text, "Δ 12.5, −3.0 mm");
    // With Shift the locked axis reads 0.0.
    hover(&mut s, pt(57.5, -3.0), true);
    let r = s.live_readout().expect("readout with shift");
    assert_eq!(r.text, "Δ 12.5, 0.0 mm");
    release(&mut s, pt(57.5, -3.0), true);
    assert!(s.live_readout().is_none(), "the readout goes with the drag");
}

// ---------------------------------------------------------------------------------------------
// Criteria 14, 20: capture and cancel
// ---------------------------------------------------------------------------------------------

#[test]
fn c20_escape_cancels_the_bend_writes_nothing_and_keeps_the_segment_selected() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -15.0), false);
    hover(&mut s, pt(45.0, -30.0), false);
    assert_eq!(
        s.escape(),
        EscapeStep::CancelledDrag,
        "one Escape ends the drag"
    );
    assert!(s.live_readout().is_none(), "the readout disappears");
    release(&mut s, pt(45.0, -30.0), false);
    assert_eq!(json(&s), before, "the release writes nothing");
    assert!(
        s.node_toolbar_state().can_make_curve,
        "the segment stays selected"
    );
    assert_eq!(
        s.escape(),
        EscapeStep::ClearedState,
        "a second Escape clears"
    );
    assert!(!s.node_toolbar_state().can_make_curve);
}

#[test]
fn c20_a_system_pointer_cancel_writes_nothing() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    let before = json(&s);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -30.0), false);
    s.pointer_cancelled();
    release(&mut s, pt(45.0, -30.0), false);
    assert_eq!(json(&s), before);
    assert!(s.node_toolbar_state().can_make_curve);
}

#[test]
fn c14_the_pointer_leaving_does_not_end_the_bend_and_a_release_outside_commits() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    press(&mut s, pt(45.0, 0.0), false);
    hover(&mut s, pt(45.0, -15.0), false);
    s.pointer_leave();
    // The captured pointer keeps delivering events from far outside the canvas.
    hover(&mut s, pt(45.0, -3000.0), false);
    release(&mut s, pt(45.0, -3000.0), false);
    let p = first_path(&s);
    assert!(
        (p.anchors[0].handle_out.y - (-3000.0 * 4.0 / 3.0)).abs() < 1e-3,
        "{:?}",
        p.anchors[0]
    );
    assert_eq!(bend_labels(&s), 1);
}

// ---------------------------------------------------------------------------------------------
// Criterion 21 and compound paths
// ---------------------------------------------------------------------------------------------

#[test]
fn c21_the_select_tool_still_moves_a_path_dragged_by_its_outline() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    s.set_tool(Tool::Select);
    drag(&mut s, pt(45.0, 0.0), pt(45.0, -30.0), false);
    let p = first_path(&s);
    assert_eq!(p.anchors[0].handle_out, Vec2::ZERO, "nothing bends");
    assert!(
        (p.anchors[0].point.y - (-30.0)).abs() < 1e-6,
        "the path moved: {:?}",
        p.anchors[0].point
    );
    assert_eq!(bend_labels(&s), 0);
}

#[test]
fn c21_the_pen_still_starts_a_new_path_on_an_outline() {
    let d = straight_path(&[AnchorKind::Corner, AnchorKind::Corner]);
    let mut s = open_session(&d);
    s.set_tool(Tool::Pen);
    click(&mut s, pt(45.0, 0.0));
    click(&mut s, pt(45.0, 20.0));
    s.finish_pen();
    let doc = reread(&s);
    assert_eq!(doc.object_ids().len(), 2, "a second path was drawn");
    assert_eq!(bend_labels(&s), 0);
}

fn circle_anchors(cx: f64, cy: f64, r: f64) -> Vec<NewAnchor> {
    let k = 0.552_284_749_830_793_4 * r;
    vec![
        anchor(cx + r, cy, AnchorKind::Symmetric, v(0.0, -k), v(0.0, k)),
        anchor(cx, cy + r, AnchorKind::Symmetric, v(k, 0.0), v(-k, 0.0)),
        anchor(cx - r, cy, AnchorKind::Symmetric, v(0.0, k), v(0.0, -k)),
        anchor(cx, cy - r, AnchorKind::Symmetric, v(-k, 0.0), v(k, 0.0)),
    ]
}

#[test]
fn c6_a_compound_path_cannot_be_bent() {
    let d = Document::new(1);
    d.create_path(&circle_anchors(100.0, 100.0, 60.0), true);
    d.create_path(&circle_anchors(100.0, 100.0, 30.0), true);
    let mut s = open_session(&d);
    s.set_tool(Tool::Select);
    // Marquee both, then combine.
    s.pointer_hover(pt(10.0, 10.0), false, false);
    s.pointer_down(pt(10.0, 10.0), false);
    s.pointer_hover(pt(100.0, 100.0), false, false);
    s.pointer_hover(pt(200.0, 200.0), false, false);
    s.pointer_up(pt(200.0, 200.0), false, false);
    s.apply_combine();
    s.set_tool(Tool::Node);
    let before = json(&s);
    // The outer outline's rightmost point is a node (160, 100); take the outline between the
    // nodes on the upper right: the circle point at 45 degrees.
    let r = 60.0;
    let on = pt(
        100.0 + r * 0.5f64.sqrt() * 1.0,
        100.0 - r * 0.5f64.sqrt() * 1.0,
    );
    drag(&mut s, on, pt(on.x + 15.0, on.y - 15.0), false);
    assert_eq!(bend_labels(&s), 0);
    assert_eq!(
        json(&s),
        before,
        "a compound path is left alone in the Node tool"
    );
}

// ---------------------------------------------------------------------------------------------
// Criterion 17: the work per move does not grow with the node count
// ---------------------------------------------------------------------------------------------

fn long_zigzag(n: usize) -> Document {
    let d = Document::new(1);
    let anchors: Vec<NewAnchor> = (0..n)
        .map(|i| corner(i as f64 * 20.0, if i % 2 == 0 { 0.0 } else { 15.0 }))
        .collect();
    d.create_path(&anchors, false);
    d
}

fn per_move_micros(n: usize, moves: usize) -> f64 {
    let mut s = open_session(&long_zigzag(n));
    // Press on the middle of the first segment (10, 7.5).
    press(&mut s, pt(10.0, 7.5), false);
    hover(&mut s, pt(10.0, 0.0), false);
    let t = Instant::now();
    for i in 0..moves {
        hover(&mut s, pt(10.0, -(i as f64 % 30.0) - 1.0), false);
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / moves as f64;
    release(&mut s, pt(10.0, -5.0), false);
    us
}

#[test]
fn c17_work_per_pointer_move_does_not_grow_with_the_node_count() {
    let small = per_move_micros(50, 400);
    let large = per_move_micros(5000, 400);
    eprintln!("per move: 50 nodes {small:.1} us, 5000 nodes {large:.1} us");
    // The frame is not drawn here, only the tool's own state; a 100x larger path must not make
    // a move 20x dearer (a scan of the whole path would be about 100x).
    assert!(
        large < small * 20.0 + 500.0,
        "50 nodes {small:.1} us vs 5000 nodes {large:.1} us"
    );
}

#[test]
fn c17_a_bend_on_a_5000_node_path_commits_correctly() {
    let mut s = open_session(&long_zigzag(5000));
    let n = first_path(&s).anchors.len();
    drag(&mut s, pt(10.0, 7.5), pt(10.0, -20.0), false);
    let p = first_path(&s);
    assert_eq!(p.anchors.len(), n);
    assert_ne!(p.anchors[0].handle_out, Vec2::ZERO);
    assert_eq!(bend_labels(&s), 1);
}
