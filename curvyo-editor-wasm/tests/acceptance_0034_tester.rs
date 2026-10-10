//! Independent session-level acceptance tests for `0034-pen-path-extension`, written from the
//! specification before the implementation was read: continue an open path, connect two paths,
//! the closing join (Sharp / Smooth), the Close path command, hit priorities, Escape, the
//! `Document::version()` cache and the hover budget.

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
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::manual_assert
)]

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, Document, EllipseFrame, Length, NewAnchor, NodeId, PathSnapshot,
    Point, StyleEdit, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, Session, Tool};
use curvyo_ui_core::{JoinType, PenTarget};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(1);

fn aid() -> AnchorId {
    AnchorId::new(34, NEXT.fetch_add(1, Ordering::SeqCst))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn corner(x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(aid(), pt(x, y))
}

fn open_pen(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.resize_viewport(1600.0, 1000.0);
    s.set_tool(Tool::Pen);
    s
}

fn reread(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn json(s: &Session) -> Vec<u8> {
    reread(s).export_json().unwrap()
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

fn paths(s: &Session) -> Vec<PathSnapshot> {
    let d = reread(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.path(id))
        .collect()
}

fn path_by_id(s: &Session, id: NodeId) -> Option<PathSnapshot> {
    reread(s).path(id)
}

fn xs(p: &PathSnapshot) -> Vec<(f64, f64)> {
    p.anchors.iter().map(|a| (a.point.x, a.point.y)).collect()
}

fn hover(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
}

fn click(s: &mut Session, p: Point) {
    hover(s, p, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn click_shift(s: &mut Session, p: Point) {
    hover(s, p, true);
    s.pointer_down(p, true);
    s.pointer_up(p, true, false);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    hover(s, from, false);
    s.pointer_down(from, false);
    hover(s, pt((from.x + to.x) / 2.0, (from.y + to.y) / 2.0), false);
    hover(s, to, false);
    s.pointer_up(to, false, false);
}

fn doc_with(paths: &[(&[(f64, f64)], bool)]) -> (Document, Vec<NodeId>) {
    let d = Document::new(1);
    let ids = paths
        .iter()
        .map(|(pts, closed)| {
            let a: Vec<NewAnchor> = pts.iter().map(|&(x, y)| corner(x, y)).collect();
            d.create_path(&a, *closed)
        })
        .collect();
    (d, ids)
}

fn kind_of(a: &NewAnchor) -> AnchorKind {
    a.kind
}

fn is_continue(t: &Option<PenTarget>) -> bool {
    matches!(t, Some(PenTarget::Continue(_)))
}

fn is_join(t: &Option<PenTarget>) -> bool {
    matches!(t, Some(PenTarget::Join(_)))
}

fn is_close(t: &Option<PenTarget>) -> bool {
    matches!(t, Some(PenTarget::Close { .. }))
}

fn one_px_mm(s: &Session) -> f64 {
    1.0 / s.view().scale()
}

fn new_commits(before: &[String], after: &[String]) -> Vec<String> {
    after[before.len()..].to_vec()
}

// ---------------------------------------------------------------------------------------------
// Part A: continue an open path
// ---------------------------------------------------------------------------------------------

#[test]
fn a1_the_cue_shows_over_end_nodes_only() {
    let (d, _) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], false),
        (&[(100.0, 0.0), (120.0, 0.0), (120.0, 20.0)], true),
    ]);
    let mut s = open_pen(&d);
    for p in [pt(0.0, 0.0), pt(20.0, 0.0)] {
        hover(&mut s, p, false);
        assert!(is_continue(&s.pen_target()), "end node at {p:?}");
    }
    hover(&mut s, pt(10.0, 0.0), false);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::Place)),
        "interior node shows nothing"
    );
    hover(&mut s, pt(100.0, 0.0), false);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::Place)),
        "a closed path has no end node"
    );
    // Within 16 px of the end, not beyond.
    let r = one_px_mm(&s);
    hover(&mut s, pt(20.0 + 15.0 * r, 0.0), false);
    assert!(is_continue(&s.pen_target()), "15 px away");
    hover(&mut s, pt(20.0 + 17.0 * r, 0.0), false);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::Place)),
        "17 px away"
    );
}

#[test]
fn a1_shift_over_an_end_starts_a_new_path_instead_and_follows_live() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    hover(&mut s, pt(10.0, 0.0), false);
    assert!(is_continue(&s.pen_target()));
    hover(&mut s, pt(10.0, 0.0), true);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::NewPathAt)),
        "{:?}",
        s.pen_target()
    );
    hover(&mut s, pt(10.0, 0.0), false);
    assert!(is_continue(&s.pen_target()));
}

#[test]
fn a1_compound_paths_and_primitives_are_never_targets() {
    let d = Document::new(1);
    d.create_ellipse(EllipseFrame {
        center: pt(100.0, 100.0),
        rx: Length::from_mm(20.0),
        ry: Length::from_mm(20.0),
    });
    let mut s = open_pen(&d);
    for p in [pt(120.0, 100.0), pt(100.0, 80.0), pt(100.0, 120.0)] {
        hover(&mut s, p, false);
        assert!(
            matches!(s.pen_target(), Some(PenTarget::Place)),
            "primitive {p:?}"
        );
    }
}

#[test]
fn a2_pressing_an_end_continues_and_the_path_is_untouched() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = json(&s);
    click(&mut s, pt(20.0, 0.0));
    assert_eq!(json(&s), before, "P is unchanged in the document");
    let nodes = s.pen_in_progress().expect("the Pen is continuing");
    assert_eq!(nodes[0].point, pt(20.0, 0.0), "E is the first pen node");
    click(&mut s, pt(30.0, 0.0));
    click(&mut s, pt(40.0, 5.0));
    assert_eq!(json(&s), before, "nothing is written until the finish");
    s.finish_pen();
    let p = path_by_id(&s, ids[0]).expect("same object");
    assert_eq!(
        xs(&p),
        vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (20.0, 0.0),
            (30.0, 0.0),
            (40.0, 5.0)
        ]
    );
}

#[test]
fn a2_shift_at_the_press_starts_a_new_path_as_today() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    click_shift(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(40.0, 40.0));
    s.finish_pen();
    let ps = paths(&s);
    assert_eq!(ps.len(), 2, "a new path started");
    assert_eq!(ps[0].anchors.len(), 2, "the old path is untouched");
}

#[test]
fn a4_new_nodes_before_the_first_node_come_in_reverse_order() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    click(&mut s, pt(0.0, 0.0));
    click(&mut s, pt(0.0, 10.0));
    click(&mut s, pt(0.0, 20.0));
    s.finish_pen();
    let p = path_by_id(&s, ids[0]).unwrap();
    assert_eq!(
        xs(&p),
        vec![
            (0.0, 20.0),
            (0.0, 10.0),
            (0.0, 0.0),
            (10.0, 0.0),
            (20.0, 0.0)
        ]
    );
}

#[test]
fn a3_a4_the_first_segment_uses_the_end_nodes_stored_open_side_handle() {
    // P's first node has an incoming handle (-0, ...). Continue from it, draw one node. The
    // original anchors keep their data exactly.
    let d = Document::new(1);
    let first = NewAnchor {
        id: aid(),
        point: pt(0.0, 0.0),
        handle_in: v(0.0, -15.0),
        handle_out: v(10.0, 0.0),
        kind: AnchorKind::Corner,
    };
    let last = NewAnchor {
        id: aid(),
        point: pt(50.0, 0.0),
        handle_in: v(-10.0, 0.0),
        handle_out: v(12.0, 7.0),
        kind: AnchorKind::Corner,
    };
    let id = d.create_path(&[first, last], false);
    let mut s = open_pen(&d);
    click(&mut s, pt(50.0, 0.0));
    click(&mut s, pt(90.0, 0.0));
    s.finish_pen();
    let p = path_by_id(&s, id).unwrap();
    assert_eq!(p.anchors.len(), 3);
    assert_eq!(p.anchors[0], first, "old anchors unchanged, ids kept");
    assert_eq!(p.anchors[1], last, "old anchors unchanged, ids kept");
    assert_eq!(p.anchors[2].point, pt(90.0, 0.0));
    assert_ne!(p.anchors[2].id, p.anchors[1].id);
    // Continue from the first node instead, with a click-drag for the new node.
    let d = Document::new(1);
    let id = d.create_path(&[first, last], false);
    let mut s = open_pen(&d);
    click(&mut s, pt(0.0, 0.0));
    // A drag at (0, 40) towards (0, 60): drawn handle_out = (0, 20), handle_in = (0, -20).
    drag(&mut s, pt(0.0, 40.0), pt(0.0, 60.0));
    s.finish_pen();
    let p = path_by_id(&s, id).unwrap();
    assert_eq!(p.anchors.len(), 3);
    assert_eq!(p.anchors[1], first);
    assert_eq!(p.anchors[2], last);
    let n = p.anchors[0];
    assert_eq!(n.point, pt(0.0, 40.0));
    // The curve is as drawn: stored in/out swapped by the reversal.
    assert!((n.handle_in.y - 20.0).abs() < 1e-6, "{n:?}");
    assert!((n.handle_out.y + 20.0).abs() < 1e-6, "{n:?}");
}

#[test]
fn a5_finish_is_one_extend_path_commit_and_keeps_identity_style_place_and_ids() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(0.0, 80.0), (10.0, 80.0)], false),
    ]);
    d.edit_style(
        &[ids[0]],
        &StyleEdit::StrokeColor(Color {
            r: 200,
            g: 10,
            b: 10,
        }),
    );
    d.edit_style(&[ids[0]], &StyleEdit::StrokeWidth(Length::from_mm(2.0)));
    let style = d.path(ids[0]).unwrap().style.clone();
    let old_ids: Vec<AnchorId> = d
        .path(ids[0])
        .unwrap()
        .anchors
        .iter()
        .map(|a| a.id)
        .collect();
    let mut s = open_pen(&d);
    let before = labels(&reread(&s));
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    click(&mut s, pt(40.0, 20.0));
    s.finish_pen();
    let after = labels(&reread(&s));
    assert_eq!(
        new_commits(&before, &after),
        vec!["extend_path".to_string()]
    );
    let doc = reread(&s);
    assert_eq!(doc.object_ids(), ids, "same object, same z place");
    let p = doc.path(ids[0]).unwrap();
    assert_eq!(p.style, style);
    assert_eq!(
        p.anchors.iter().take(2).map(|a| a.id).collect::<Vec<_>>(),
        old_ids
    );
    assert_eq!(p.anchors.len(), 4);
}

#[test]
fn a5_finishing_without_a_new_node_writes_nothing() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = labels(&reread(&s));
    let json_before = json(&s);
    click(&mut s, pt(10.0, 0.0));
    s.finish_pen();
    assert_eq!(labels(&reread(&s)), before);
    assert_eq!(json(&s), json_before);
}

#[test]
fn a6_a_selected_or_unselected_path_is_continued_alike() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    s.set_tool(Tool::Select);
    click(&mut s, pt(5.0, 0.0));
    s.set_tool(Tool::Pen);
    hover(&mut s, pt(10.0, 0.0), false);
    assert!(is_continue(&s.pen_target()), "selected");
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    s.finish_pen();
    assert_eq!(path_by_id(&s, ids[0]).unwrap().anchors.len(), 3);
}

// ---------------------------------------------------------------------------------------------
// Part E: Escape and tool switch
// ---------------------------------------------------------------------------------------------

#[test]
fn e25_escape_discards_the_addition_and_the_next_escape_leaves_the_tool() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    click(&mut s, pt(50.0, 0.0));
    assert_eq!(s.escape(), EscapeStep::ClearedState);
    assert!(s.pen_in_progress().is_none(), "the Pen is idle");
    assert_eq!(json(&s), before, "P is exactly as it was");
    assert_eq!(labels(&reread(&s)).len(), n, "no commit");
    assert_eq!(path_by_id(&s, ids[0]).unwrap().anchors.len(), 2);
    assert_eq!(s.escape(), EscapeStep::LeftTool);
}

#[test]
fn e25_escape_during_a_click_drag_of_a_new_node_discards_the_whole_addition() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = json(&s);
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    hover(&mut s, pt(60.0, 0.0), false);
    s.pointer_down(pt(60.0, 0.0), false);
    hover(&mut s, pt(60.0, 20.0), false);
    let step = s.escape();
    assert!(
        matches!(step, EscapeStep::CancelledDrag | EscapeStep::ClearedState),
        "{step:?}"
    );
    s.pointer_up(pt(60.0, 20.0), false, false);
    assert!(s.pen_in_progress().is_none(), "the whole addition is gone");
    assert_eq!(json(&s), before);
}

#[test]
fn e25_escape_on_a_new_path_about_to_join_leaves_q_alone() {
    let (d, ids) = doc_with(&[(&[(100.0, 0.0), (120.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = json(&s);
    click(&mut s, pt(0.0, 30.0));
    click(&mut s, pt(50.0, 30.0));
    hover(&mut s, pt(100.0, 0.0), false);
    assert!(is_join(&s.pen_target()));
    s.escape();
    assert!(s.pen_in_progress().is_none());
    assert_eq!(json(&s), before);
    assert_eq!(paths(&s).len(), 1);
    assert_eq!(path_by_id(&s, ids[0]).unwrap().anchors.len(), 2);
}

#[test]
fn e26_leaving_the_pen_while_continuing_ends_as_drawn_never_half_written() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    s.set_tool(Tool::Select);
    let p = path_by_id(&s, ids[0]).unwrap();
    assert_eq!(xs(&p), vec![(0.0, 0.0), (10.0, 0.0), (30.0, 0.0)]);
    assert_eq!(paths(&s).len(), 1, "no stray object");
    // With no new node, leaving the tool writes nothing.
    let mut s = open_pen(&d);
    let before = json(&s);
    click(&mut s, pt(10.0, 0.0));
    s.set_tool(Tool::Node);
    assert_eq!(json(&s), before);
}

// ---------------------------------------------------------------------------------------------
// Part B: connect two paths
// ---------------------------------------------------------------------------------------------

#[test]
fn b8_connect_order_and_commit() {
    for (press, want) in [
        (
            (30.0, 0.0),
            vec![(0.0, 0.0), (10.0, 0.0), (30.0, 0.0), (40.0, 0.0)],
        ),
        (
            (40.0, 0.0),
            vec![(0.0, 0.0), (10.0, 0.0), (40.0, 0.0), (30.0, 0.0)],
        ),
    ] {
        let (d, ids) = doc_with(&[
            (&[(0.0, 0.0), (10.0, 0.0)], false),
            (&[(30.0, 0.0), (40.0, 0.0)], false),
        ]);
        let mut s = open_pen(&d);
        let before = labels(&reread(&s));
        click(&mut s, pt(10.0, 0.0));
        hover(&mut s, pt(press.0, press.1), false);
        assert!(is_join(&s.pen_target()), "join target at {press:?}");
        click(&mut s, pt(press.0, press.1));
        assert!(s.pen_in_progress().is_none(), "finished at once");
        let ps = paths(&s);
        assert_eq!(ps.len(), 1, "Q is gone");
        assert_eq!(ps[0].id, ids[0], "the continued path survives");
        assert_eq!(xs(&ps[0]), want);
        let after = labels(&reread(&s));
        assert_eq!(
            new_commits(&before, &after),
            vec!["connect_paths".to_string()],
            "one commit"
        );
    }
}

#[test]
fn b8_connect_from_the_first_node_puts_the_other_nodes_before() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(-30.0, 0.0), (-40.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(0.0, 0.0));
    click(&mut s, pt(-30.0, 0.0));
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].id, ids[0]);
    assert_eq!(
        xs(&ps[0]),
        vec![(-40.0, 0.0), (-30.0, 0.0), (0.0, 0.0), (10.0, 0.0)]
    );

    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(-30.0, 0.0), (-40.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(0.0, 0.0));
    click(&mut s, pt(-40.0, 0.0));
    let ps = paths(&s);
    assert_eq!(ps[0].id, ids[0]);
    assert_eq!(
        xs(&ps[0]),
        vec![(-30.0, 0.0), (-40.0, 0.0), (0.0, 0.0), (10.0, 0.0)]
    );
}

#[test]
fn b8_new_nodes_between_the_two_ends_are_kept() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(30.0, 0.0), (40.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(20.0, 25.0));
    click(&mut s, pt(30.0, 0.0));
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].id, ids[0]);
    assert_eq!(
        xs(&ps[0]),
        vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (20.0, 25.0),
            (30.0, 0.0),
            (40.0, 0.0)
        ]
    );
}

#[test]
fn b9_b10_kinds_and_handles_of_the_joined_ends_are_unchanged_and_p_keeps_its_style() {
    let d = Document::new(1);
    let p0 = corner(0.0, 0.0);
    let p1 = NewAnchor {
        id: aid(),
        point: pt(10.0, 0.0),
        handle_in: v(-3.0, 0.0),
        handle_out: v(3.0, 4.0),
        kind: AnchorKind::Asymmetric,
    };
    let q0 = NewAnchor {
        id: aid(),
        point: pt(30.0, 0.0),
        handle_in: v(-2.0, 2.0),
        handle_out: v(5.0, 0.0),
        kind: AnchorKind::Symmetric,
    };
    let q1 = corner(40.0, 0.0);
    let p = d.create_path(&[p0, p1], false);
    let q = d.create_path(&[q0, q1], false);
    d.edit_style(&[p], &StyleEdit::StrokeColor(Color { r: 200, g: 0, b: 0 }));
    d.edit_style(&[p], &StyleEdit::StrokeWidth(Length::from_mm(2.0)));
    d.edit_style(&[q], &StyleEdit::StrokeColor(Color { r: 0, g: 0, b: 200 }));
    d.edit_style(&[q], &StyleEdit::StrokeWidth(Length::from_mm(0.5)));
    let red = d.path(p).unwrap().style.clone();
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    hover(&mut s, pt(30.0, 0.0), false);
    assert!(s.pen_join_style_differs(), "the chip's third line");
    click(&mut s, pt(30.0, 0.0));
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].style, red, "the continued path's style survives");
    assert_eq!(
        ps[0].anchors,
        vec![p0, p1, q0, q1],
        "handles and kinds as they were"
    );
}

#[test]
fn b10_a_new_path_that_ends_on_q_becomes_q() {
    for (press, want) in [
        (
            (200.0, 0.0),
            vec![(100.0, 50.0), (150.0, 50.0), (200.0, 0.0), (220.0, 0.0)],
        ),
        (
            (220.0, 0.0),
            vec![(200.0, 0.0), (220.0, 0.0), (150.0, 50.0), (100.0, 50.0)],
        ),
    ] {
        let (d, ids) = doc_with(&[(&[(200.0, 0.0), (220.0, 0.0)], false)]);
        d.edit_style(&[ids[0]], &StyleEdit::StrokeWidth(Length::from_mm(0.5)));
        let q_style = d.path(ids[0]).unwrap().style.clone();
        let mut s = open_pen(&d);
        click(&mut s, pt(100.0, 50.0));
        click(&mut s, pt(150.0, 50.0));
        hover(&mut s, pt(press.0, press.1), false);
        assert!(is_join(&s.pen_target()));
        assert!(!s.pen_join_style_differs(), "a new path has no style line");
        click(&mut s, pt(press.0, press.1));
        let ps = paths(&s);
        assert_eq!(ps.len(), 1, "one object");
        assert_eq!(ps[0].id, ids[0], "Q survives with its id");
        assert_eq!(ps[0].style, q_style);
        assert_eq!(xs(&ps[0]), want);
    }
}

#[test]
fn b9_ends_within_a_micrometre_are_merged_into_one_corner_node() {
    let d = Document::new(1);
    let a = corner(0.0, 0.0);
    let b = corner(10.0, 0.0);
    let c = corner(10.0, 0.0005);
    let e = corner(30.0, 0.0005);
    let p = d.create_path(&[a, b], false);
    d.create_path(&[c, e], false);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    // The target nearest to the pointer: Q's end is 0.5 um away from E, which is excluded.
    hover(&mut s, pt(10.0, 0.0005), false);
    assert!(is_join(&s.pen_target()), "{:?}", s.pen_target());
    click(&mut s, pt(10.0, 0.0005));
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].id, p);
    assert_eq!(ps[0].anchors.len(), 3, "merged: {:?}", xs(&ps[0]));
    assert_eq!(kind_of(&ps[0].anchors[1]), AnchorKind::Corner);
}

#[test]
fn b11_the_connect_is_atomic_in_one_commit_with_q_removed() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(30.0, 0.0), (40.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    let doc = reread(&s);
    assert!(doc.path(ids[1]).is_none());
    assert_eq!(doc.object_ids(), vec![ids[0]]);
}

#[test]
fn b7_shift_over_a_join_target_places_a_plain_node() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(30.0, 0.0), (40.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    hover(&mut s, pt(30.0, 0.0), true);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::PlaceOverEnd)),
        "{:?}",
        s.pen_target()
    );
    click_shift(&mut s, pt(30.0, 0.0));
    assert!(s.pen_in_progress().is_some(), "still drawing");
    s.finish_pen();
    let ps = paths(&s);
    assert_eq!(ps.len(), 2, "no join happened");
    assert_eq!(ps[0].id, ids[0]);
    assert_eq!(
        xs(&ps[0]),
        vec![(0.0, 0.0), (10.0, 0.0), (30.0, 0.0)],
        "a plain node was placed there"
    );
    assert_eq!(ps[1].anchors.len(), 2, "Q untouched");
}

#[test]
fn b12_closed_compound_and_primitive_have_no_join_cue_and_own_other_end_is_the_close_target() {
    let (d, ids) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false),
        (&[(100.0, 0.0), (120.0, 0.0), (120.0, 20.0)], true),
    ]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 10.0));
    hover(&mut s, pt(100.0, 0.0), false);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::Place)),
        "closed Q: no cue"
    );
    // The other end of the continued path is the close target (P has 3 nodes + E).
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(is_close(&s.pen_target()), "{:?}", s.pen_target());
    let _ = ids;
}

#[test]
fn c13_a_continued_path_with_two_nodes_and_no_new_node_has_no_close_target() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(
        !is_close(&s.pen_target()),
        "a closed path of two nodes is refused"
    );
    // One new node makes three: now it closes.
    click(&mut s, pt(10.0, 30.0));
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(is_close(&s.pen_target()));
}

#[test]
fn a_continued_path_with_three_nodes_closes_with_no_new_node() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false)]);
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 10.0));
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(is_close(&s.pen_target()));
    let before = labels(&reread(&s));
    click(&mut s, pt(0.0, 0.0));
    let p = path_by_id(&s, ids[0]).unwrap();
    assert!(p.closed);
    assert_eq!(p.anchors.len(), 3, "no node added");
    assert_eq!(
        new_commits(&before, &labels(&reread(&s))),
        vec!["close_path".to_string()]
    );
}

// ---------------------------------------------------------------------------------------------
// Hit priorities (7, 13, 23)
// ---------------------------------------------------------------------------------------------

#[test]
fn priority_close_target_beats_another_paths_end_beats_a_plain_node() {
    // Q's end sits 7 px from the new path's first node.
    let (d, _) = doc_with(&[(&[(0.0, 2.0), (30.0, 2.0)], false)]);
    let mut s = open_pen(&d);
    let r = one_px_mm(&s);
    // New path n1 (0,-40), n2 (60,-40), n3 (60,60). Its first node n1 is far from Q first.
    click(&mut s, pt(0.0, -40.0));
    click(&mut s, pt(60.0, -40.0));
    click(&mut s, pt(60.0, 60.0));
    // Q's end (0,2) is a join target.
    hover(&mut s, pt(0.0, 2.0), false);
    assert!(is_join(&s.pen_target()));
    // Escape and redo with Q's end inside the radius of the close target.
    s.escape();
    let (d, _) = doc_with(&[(&[(0.0, 2.0 + 0.0 * r), (30.0, 2.0)], false)]);
    let mut s = open_pen(&d);
    // Start the new path at 8 px from Q's end, away from Q's range test (it is a click, so the
    // first click would start by CONTINUING Q if within 16 px). Use Shift to start anew.
    click_shift(&mut s, pt(0.0, 2.0 + 8.0 * r));
    click(&mut s, pt(60.0, 60.0));
    click(&mut s, pt(60.0, 100.0));
    hover(&mut s, pt(0.0, 2.0 + 4.0 * r), false);
    assert!(
        is_close(&s.pen_target()),
        "the in-progress path's own close target wins: {:?}",
        s.pen_target()
    );
}

#[test]
fn p23_hit_radius_is_16_px_for_continue_join_and_close() {
    let (d, _) = doc_with(&[
        (&[(0.0, 0.0), (10.0, 0.0)], false),
        (&[(100.0, 0.0), (140.0, 0.0)], false),
    ]);
    let mut s = open_pen(&d);
    let r = one_px_mm(&s);
    click(&mut s, pt(10.0, 0.0));
    hover(&mut s, pt(100.0, 15.0 * r), false);
    assert!(is_join(&s.pen_target()), "join at 15 px");
    hover(&mut s, pt(100.0, 17.0 * r), false);
    assert!(!is_join(&s.pen_target()), "no join at 17 px");
}

// ---------------------------------------------------------------------------------------------
// Part C: closing and the join type
// ---------------------------------------------------------------------------------------------

fn draw_square_clicks(s: &mut Session) {
    for p in [pt(0.0, 0.0), pt(40.0, 0.0), pt(40.0, 40.0), pt(0.0, 40.0)] {
        click(s, p);
    }
}

#[test]
fn c15_default_join_is_as_drawn_and_a_click_path_closes_sharp() {
    let mut s = open_pen(&Document::new(1));
    draw_square_clicks(&mut s);
    hover(&mut s, pt(0.0, 0.0), false);
    let t = s.pen_target();
    match t {
        Some(PenTarget::Close {
            join,
            as_drawn,
            shift,
        }) => {
            assert_eq!(join, JoinType::Sharp);
            assert_eq!(as_drawn, JoinType::Sharp);
            assert!(!shift);
        }
        other => panic!("not a close target: {other:?}"),
    }
    click(&mut s, pt(0.0, 0.0));
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert!(ps[0].closed);
    assert_eq!(ps[0].anchors.len(), 4);
    assert!(ps[0].anchors.iter().all(|a| a.kind == AnchorKind::Corner
        && a.handle_in == Vec2::ZERO
        && a.handle_out == Vec2::ZERO));
}

#[test]
fn c15_shift_makes_a_corner_node_smooth_with_collinear_handles_along_d_to_b() {
    let mut s = open_pen(&Document::new(1));
    draw_square_clicks(&mut s);
    hover(&mut s, pt(0.0, 0.0), true);
    assert_eq!(
        s.pen_close_joins(),
        Some((JoinType::Smooth, JoinType::Sharp, true)),
        "the preview and the chip read live from Shift"
    );
    click_shift(&mut s, pt(0.0, 0.0));
    let p = &paths(&s)[0];
    assert!(p.closed && p.anchors.len() == 4);
    let a = p.anchors[0];
    assert_eq!(a.kind, AnchorKind::Asymmetric);
    // Tangent from D (0,40) to B (40,0).
    let t = v(40.0, -40.0);
    let cross = a.handle_out.x * t.y - a.handle_out.y * t.x;
    assert!(cross.abs() < 1e-6, "out along the tangent: {a:?}");
    assert!(a.handle_out.x * t.x + a.handle_out.y * t.y > 0.0, "{a:?}");
    let cross_in = a.handle_in.x * t.y - a.handle_in.y * t.x;
    assert!(cross_in.abs() < 1e-6, "in along the tangent: {a:?}");
    assert!(a.handle_in.x * t.x + a.handle_in.y * t.y < 0.0, "{a:?}");
    assert!(a.handle_in.length() > 0.0 && a.handle_out.length() > 0.0);
    // No other node changed.
    for n in &p.anchors[1..] {
        assert_eq!(n.kind, AnchorKind::Corner);
        assert_eq!(n.handle_in, Vec2::ZERO);
        assert_eq!(n.handle_out, Vec2::ZERO);
    }
}

fn square_with_dragged_first(shift_close: bool) -> PathSnapshot {
    let mut s = open_pen(&Document::new(1));
    drag(&mut s, pt(0.0, 0.0), pt(10.0, 0.0));
    click(&mut s, pt(40.0, 0.0));
    click(&mut s, pt(40.0, 40.0));
    hover(&mut s, pt(0.0, 0.0), shift_close);
    assert!(is_close(&s.pen_target()));
    if shift_close {
        click_shift(&mut s, pt(0.0, 0.0));
    } else {
        click(&mut s, pt(0.0, 0.0));
    }
    let ps = paths(&s);
    assert_eq!(ps.len(), 1);
    assert!(ps[0].closed);
    assert_eq!(ps[0].anchors.len(), 3);
    ps[0].clone()
}

#[test]
fn c15_a_dragged_first_node_closes_smooth_by_default_and_sharp_with_shift() {
    let smooth = square_with_dragged_first(false);
    let a = smooth.anchors[0];
    assert_eq!(a.kind, AnchorKind::Symmetric, "unchanged");
    assert!((a.handle_out.x - 10.0).abs() < 1e-6, "{a:?}");
    assert!((a.handle_in.x + 10.0).abs() < 1e-6, "{a:?}");

    let sharp = square_with_dragged_first(true);
    let a = sharp.anchors[0];
    assert_eq!(a.kind, AnchorKind::Corner);
    assert_eq!(
        a.handle_in,
        Vec2::ZERO,
        "the closing-side handle is retracted"
    );
    assert!(
        (a.handle_out.x - 10.0).abs() < 1e-6,
        "the other handle stays: {a:?}"
    );
    assert_ne!(
        smooth.anchors[0].handle_in, sharp.anchors[0].handle_in,
        "different curves"
    );
}

#[test]
fn c15_the_preview_resolves_the_same_join_the_commit_applies() {
    // As-drawn Symmetric: join Smooth without Shift, Sharp with it.
    let mut s = open_pen(&Document::new(1));
    drag(&mut s, pt(0.0, 0.0), pt(10.0, 0.0));
    click(&mut s, pt(40.0, 0.0));
    click(&mut s, pt(40.0, 40.0));
    hover(&mut s, pt(0.0, 0.0), false);
    assert_eq!(
        s.pen_close_joins(),
        Some((JoinType::Smooth, JoinType::Smooth, false))
    );
    hover(&mut s, pt(0.0, 0.0), true);
    assert_eq!(
        s.pen_close_joins(),
        Some((JoinType::Sharp, JoinType::Smooth, true))
    );
}

#[test]
fn c14_a_drag_on_the_close_target_closes_without_shaping_a_handle() {
    let mut s = open_pen(&Document::new(1));
    click(&mut s, pt(0.0, 0.0));
    click(&mut s, pt(40.0, 0.0));
    click(&mut s, pt(40.0, 40.0));
    hover(&mut s, pt(0.0, 0.0), false);
    s.pointer_down(pt(0.0, 0.0), false);
    hover(&mut s, pt(10.0, 10.0), false);
    hover(&mut s, pt(30.0, 30.0), false);
    s.pointer_up(pt(30.0, 30.0), false, false);
    let ps = paths(&s);
    assert_eq!(ps.len(), 1, "the press closed the path on release");
    assert!(ps[0].closed);
    assert_eq!(ps[0].anchors.len(), 3, "no node added");
    for a in &ps[0].anchors {
        assert_eq!(a.handle_in, Vec2::ZERO, "the drag shaped nothing: {a:?}");
        assert_eq!(a.handle_out, Vec2::ZERO, "the drag shaped nothing: {a:?}");
    }
}

#[test]
fn a2_a_drag_on_an_end_node_shapes_nothing() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    let before = json(&s);
    drag(&mut s, pt(10.0, 0.0), pt(20.0, 20.0));
    assert_eq!(json(&s), before, "P is untouched");
    s.finish_pen();
    let p = path_by_id(&s, ids[0]).unwrap();
    assert_eq!(p.anchors.len(), 2);
    assert_eq!(p.anchors[1].handle_out, Vec2::ZERO, "no handle shaped on E");
}

#[test]
fn c13_two_nodes_never_show_a_close_target() {
    let mut s = open_pen(&Document::new(1));
    click(&mut s, pt(0.0, 0.0));
    click(&mut s, pt(40.0, 0.0));
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(!is_close(&s.pen_target()));
    click(&mut s, pt(0.0, 0.0));
    assert!(
        paths(&s).is_empty(),
        "a click there is a plain node, not a close"
    );
}

#[test]
fn c14_closing_a_new_path_is_one_create_commit_and_a_continued_close_is_close_path() {
    let mut s = open_pen(&Document::new(1));
    let before = labels(&reread(&s));
    draw_square_clicks(&mut s);
    click(&mut s, pt(0.0, 0.0));
    let after = labels(&reread(&s));
    assert_eq!(after.len(), before.len() + 1, "one commit: {after:?}");
}

// ---------------------------------------------------------------------------------------------
// Part D: Close path command
// ---------------------------------------------------------------------------------------------

fn select_all_nodes_tool(s: &mut Session, from: Point, to: Point) {
    s.set_tool(Tool::Select);
    hover(s, from, false);
    s.pointer_down(from, false);
    hover(s, pt((from.x + to.x) / 2.0, (from.y + to.y) / 2.0), false);
    hover(s, to, false);
    s.pointer_up(to, false, false);
    s.set_tool(Tool::Node);
}

#[test]
fn d19_close_path_sharp_and_smooth_apply_the_same_rules_as_the_pen() {
    for join in [JoinType::Sharp, JoinType::Smooth] {
        let d = Document::new(1);
        // First node dragged-out symmetric, others corners.
        let first = NewAnchor {
            id: aid(),
            point: pt(0.0, 0.0),
            handle_in: v(-10.0, 0.0),
            handle_out: v(10.0, 0.0),
            kind: AnchorKind::Symmetric,
        };
        let p = d.create_path(&[first, corner(40.0, 0.0), corner(40.0, 40.0)], false);
        let corner_first = d.create_path(
            &[
                corner(100.0, 0.0),
                corner(140.0, 0.0),
                corner(140.0, 40.0),
                corner(100.0, 40.0),
            ],
            false,
        );
        let mut s = open_pen(&d);
        select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(200.0, 100.0));
        assert_eq!(s.close_path_state().closable, 2);
        let before = labels(&reread(&s));
        let out = s.close_paths(join);
        assert_eq!(out.closed, 2);
        assert_eq!(out.skipped, 0);
        assert_eq!(
            new_commits(&before, &labels(&reread(&s))),
            vec!["close_path".to_string()],
            "one commit for all paths"
        );
        let a = path_by_id(&s, p).unwrap();
        assert!(a.closed);
        assert_eq!(a.anchors.len(), 3);
        let b = path_by_id(&s, corner_first).unwrap();
        assert!(b.closed);
        assert_eq!(b.anchors.len(), 4);
        match join {
            JoinType::Sharp => {
                assert_eq!(a.anchors[0].kind, AnchorKind::Corner);
                assert_eq!(a.anchors[0].handle_in, Vec2::ZERO, "closing side retracted");
                assert!((a.anchors[0].handle_out.x - 10.0).abs() < 1e-9);
                assert_eq!(b.anchors[0].kind, AnchorKind::Corner);
                assert_eq!(b.anchors[0].handle_in, Vec2::ZERO);
                assert_eq!(b.anchors[0].handle_out, Vec2::ZERO);
            }
            JoinType::Smooth => {
                assert_eq!(a.anchors[0].kind, AnchorKind::Symmetric, "unchanged");
                assert_eq!(a.anchors[0].handle_in, v(-10.0, 0.0));
                assert_eq!(b.anchors[0].kind, AnchorKind::Asymmetric);
                assert!(b.anchors[0].handle_in.length() > 0.0);
                assert!(b.anchors[0].handle_out.length() > 0.0);
            }
        }
    }
}

#[test]
fn d19_d20_d21_skipped_closed_compound_and_primitive_are_not_counted() {
    let d = Document::new(1);
    let two = d.create_path(&[corner(0.0, 0.0), corner(20.0, 0.0)], false);
    let three = d.create_path(
        &[corner(0.0, 50.0), corner(20.0, 50.0), corner(20.0, 70.0)],
        false,
    );
    let closed = d.create_path(
        &[corner(100.0, 0.0), corner(120.0, 0.0), corner(120.0, 20.0)],
        true,
    );
    d.create_ellipse(EllipseFrame {
        center: pt(200.0, 30.0),
        rx: Length::from_mm(15.0),
        ry: Length::from_mm(15.0),
    });
    let style_three = d.path(three).unwrap().style.clone();
    let mut s = open_pen(&d);
    select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(300.0, 150.0));
    let st = s.close_path_state();
    assert_eq!((st.closable, st.skipped), (1, 1));
    let out = s.close_paths(JoinType::Sharp);
    assert_eq!((out.closed, out.skipped), (1, 1));
    let doc = reread(&s);
    assert!(
        !doc.path(two).unwrap().closed,
        "fewer than 3 nodes: skipped"
    );
    assert!(doc.path(three).unwrap().closed);
    assert_eq!(doc.path(three).unwrap().style, style_three);
    assert!(doc.path(closed).unwrap().closed);
    assert_eq!(doc.object_ids().len(), 4);
}

#[test]
fn d18_buttons_are_inert_outside_the_node_tool_and_without_an_applicable_path() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (20.0, 0.0)], false)]);
    let mut s = open_pen(&d);
    select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(100.0, 100.0));
    assert_eq!(s.close_path_state().closable, 0);
    let before = json(&s);
    let out = s.close_paths(JoinType::Smooth);
    assert_eq!(out.closed, 0);
    assert_eq!(json(&s), before, "a dimmed press does nothing");
    s.set_tool(Tool::Select);
    assert_eq!(s.close_path_state().closable, 0);
    assert_eq!(s.close_paths(JoinType::Sharp).closed, 0);
}

#[test]
fn d19_first_and_last_node_within_a_micrometre_are_merged_first() {
    let (d, ids) = doc_with(&[(
        &[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 0.0004)],
        false,
    )]);
    let mut s = open_pen(&d);
    select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(100.0, 100.0));
    assert_eq!(s.close_paths(JoinType::Sharp).closed, 1);
    let p = path_by_id(&s, ids[0]).unwrap();
    assert!(p.closed);
    assert_eq!(p.anchors.len(), 3, "merged into one node: {:?}", xs(&p));
}

#[test]
fn d19_the_selection_and_the_object_ids_survive() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0)], false)]);
    let mut s = open_pen(&d);
    select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(100.0, 100.0));
    let n = s.selected_object_count();
    s.close_paths(JoinType::Smooth);
    assert_eq!(s.selected_object_count(), n);
    assert_eq!(reread(&s).object_ids(), ids);
}

// ---------------------------------------------------------------------------------------------
// Document::version() and the end-node cache (criterion 24)
// ---------------------------------------------------------------------------------------------

#[test]
fn v24_the_end_node_cache_follows_document_changes_without_a_pointer_move() {
    let (d, _) = doc_with(&[(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0)], false)]);
    let mut s = open_pen(&d);
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(
        is_continue(&s.pen_target()),
        "warm cache: a continue target"
    );
    // Change the document under the cache, not moving the pointer.
    s.set_tool(Tool::Select);
    s.set_tool(Tool::Node);
    select_all_nodes_tool(&mut s, pt(-20.0, -20.0), pt(100.0, 100.0));
    s.close_paths(JoinType::Sharp);
    s.set_tool(Tool::Pen);
    hover(&mut s, pt(0.0, 0.0), false);
    assert!(
        matches!(s.pen_target(), Some(PenTarget::Place)),
        "the path is closed now: no stale cue, got {:?}",
        s.pen_target()
    );
}

#[test]
fn v24_a_new_path_becomes_a_target_at_once() {
    let d = Document::new(1);
    let mut s = open_pen(&d);
    hover(&mut s, pt(200.0, 0.0), false);
    assert!(matches!(s.pen_target(), Some(PenTarget::Place)));
    click(&mut s, pt(100.0, 0.0));
    click(&mut s, pt(200.0, 0.0));
    s.finish_pen();
    hover(&mut s, pt(200.0, 0.0), false);
    assert!(is_continue(&s.pen_target()), "{:?}", s.pen_target());
}

#[test]
fn document_version_changes_on_every_new_command() {
    let (d, ids) = doc_with(&[(&[(0.0, 0.0), (10.0, 0.0)], false)]);
    let v0 = d.version();
    let mut s = open_pen(&d);
    click(&mut s, pt(10.0, 0.0));
    click(&mut s, pt(30.0, 0.0));
    s.finish_pen();
    let v1 = reread(&s).version();
    // A fresh read of the same state has the same version; a different state differs.
    assert_eq!(v1, reread(&s).version());
    assert_ne!(v0, v1);
    let _ = ids;
}

#[test]
#[ignore = "release benchmark: cargo test --release -- --ignored"]
fn v24_hover_over_5000_open_paths_costs_under_2_ms_warm() {
    let d = Document::new(1);
    for i in 0..5000 {
        let x = f64::from(i % 100) * 30.0;
        let y = f64::from(i / 100) * 30.0;
        d.create_path(&[corner(x, y), corner(x + 20.0, y)], false);
    }
    let mut s = open_pen(&d);
    hover(&mut s, pt(5.0, 5.0), false);
    let _ = s.pen_target();
    let t = Instant::now();
    let n = 200;
    for i in 0..n {
        hover(&mut s, pt(f64::from(i) * 7.0 % 3000.0 + 3.0, 15.0), false);
        let _ = s.pen_target();
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
    eprintln!("warm hover: {per:.3} ms");
    assert!(per < 2.0, "{per} ms");
}
