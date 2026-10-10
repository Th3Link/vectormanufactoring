//! Independent session-level acceptance tests for `0035-combine-and-break-apart`, written from the
//! specification before the implementation was read: availability, the winding by nesting depth
//! (checked with a nonzero-winding evaluator written here), node preservation, style and place,
//! refusals that change nothing, Break apart keeping holes with their piece, the round trip, and
//! the performance inputs of criterion 19 (release, `--ignored`).

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
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::manual_assert,
    clippy::type_complexity
)]

use std::collections::HashSet;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, Document, Length, NewAnchor, NodeId, PathSnapshot, Point,
    StyleEdit, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{BreakApartOutcome, CombineOutcome, Session, Tool};
use curvyo_ui_core::{BooleanAvailability, BreakApartRefusal, CombineRefusal};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(1);

fn aid() -> AnchorId {
    AnchorId::new(35, NEXT.fetch_add(1, Ordering::SeqCst))
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

fn square(x: f64, y: f64, side: f64) -> Vec<NewAnchor> {
    vec![
        corner(x, y),
        corner(x + side, y),
        corner(x + side, y + side),
        corner(x, y + side),
    ]
}

/// A four-node Bezier circle; `ccw` flips the traversal direction.
fn circle(cx: f64, cy: f64, r: f64, reverse: bool) -> Vec<NewAnchor> {
    let k = 0.552_284_749_830_793_4 * r;
    let mut a = vec![
        NewAnchor {
            id: aid(),
            point: pt(cx + r, cy),
            handle_in: v(0.0, -k),
            handle_out: v(0.0, k),
            kind: AnchorKind::Symmetric,
        },
        NewAnchor {
            id: aid(),
            point: pt(cx, cy + r),
            handle_in: v(k, 0.0),
            handle_out: v(-k, 0.0),
            kind: AnchorKind::Symmetric,
        },
        NewAnchor {
            id: aid(),
            point: pt(cx - r, cy),
            handle_in: v(0.0, k),
            handle_out: v(0.0, -k),
            kind: AnchorKind::Symmetric,
        },
        NewAnchor {
            id: aid(),
            point: pt(cx, cy - r),
            handle_in: v(-k, 0.0),
            handle_out: v(k, 0.0),
            kind: AnchorKind::Symmetric,
        },
    ];
    if reverse {
        a.reverse();
        for n in &mut a {
            std::mem::swap(&mut n.handle_in, &mut n.handle_out);
        }
    }
    a
}

fn session_of(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.resize_viewport(1600.0, 1000.0);
    s.set_tool(Tool::Select);
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

fn new_labels(before: &[String], s: &Session) -> Vec<String> {
    let after = labels(&reread(s));
    after[before.len()..].to_vec()
}

fn paths(s: &Session) -> Vec<PathSnapshot> {
    let d = reread(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.path(id))
        .collect()
}

fn marquee(s: &mut Session, from: Point, to: Point) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(
        pt((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
        false,
        false,
    );
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
}

fn select_all(s: &mut Session) {
    marquee(s, pt(-300.0, -300.0), pt(5000.0, 5000.0));
}

fn style_edit(d: &Document, id: NodeId, r: u8, g: u8, b: u8, width: f64) {
    d.edit_style(&[id], &StyleEdit::StrokeColor(Color { r, g, b }));
    d.edit_style(&[id], &StyleEdit::StrokeWidth(Length::from_mm(width)));
}

// A nonzero-winding evaluator for a path's outlines, written here.

fn flatten(a: &[NewAnchor], closed: bool) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let n = a.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let p = &a[i];
        let q = &a[(i + 1) % n];
        let p0 = (p.point.x, p.point.y);
        let p1 = (p.point.x + p.handle_out.x, p.point.y + p.handle_out.y);
        let p2 = (q.point.x + q.handle_in.x, q.point.y + q.handle_in.y);
        let p3 = (q.point.x, q.point.y);
        for s in 0..64 {
            let t = f64::from(s) / 64.0;
            let u = 1.0 - t;
            let (c0, c1, c2, c3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push((
                c0 * p0.0 + c1 * p1.0 + c2 * p2.0 + c3 * p3.0,
                c0 * p0.1 + c1 * p1.1 + c2 * p2.1 + c3 * p3.1,
            ));
        }
    }
    out
}

fn outlines(p: &PathSnapshot) -> Vec<Vec<(f64, f64)>> {
    p.subpaths().map(|sp| flatten(sp.anchors, true)).collect()
}

fn winding_of(poly: &[(f64, f64)], x: f64, y: f64) -> i32 {
    let mut w = 0;
    for i in 0..poly.len() {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % poly.len()];
        if y0 <= y {
            if y1 > y && (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0) > 0.0 {
                w += 1;
            }
        } else if y1 <= y && (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0) < 0.0 {
            w -= 1;
        }
    }
    w
}

fn painted(p: &PathSnapshot, x: f64, y: f64) -> bool {
    outlines(p).iter().map(|o| winding_of(o, x, y)).sum::<i32>() != 0
}

fn shoelace(poly: &[(f64, f64)]) -> f64 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % poly.len()];
        a += x0 * y1 - x1 * y0;
    }
    a / 2.0
}

fn applied_combine(o: &CombineOutcome) -> (usize, usize, bool) {
    match o {
        CombineOutcome::Applied {
            operands,
            holes,
            styles_differ,
        } => (*operands, *holes, *styles_differ),
        other => panic!("not applied: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Criterion 1: availability
// ---------------------------------------------------------------------------------------------

#[test]
fn c1_availability_follows_the_selection_and_the_tool() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(100.0, 0.0, 20.0), true);
    d.create_path(&[corner(200.0, 0.0), corner(240.0, 0.0)], false);
    let mut s = session_of(&d);
    let a = s.path_availability();
    assert_eq!(a.combine, BooleanAvailability::NeedsTwo);
    assert!(!a.break_apart, "nothing selected");

    marquee(&mut s, pt(-10.0, -10.0), pt(130.0, 30.0));
    let a = s.path_availability();
    assert_eq!(a.combine, BooleanAvailability::Ready);
    assert!(!a.break_apart, "no compound path selected");

    select_all(&mut s);
    let a = s.path_availability();
    assert_eq!(
        a.combine,
        BooleanAvailability::OpenPaths { open: 1, of: 3 },
        "an open path leaves Combine enabled in spirit: pressing refuses"
    );

    // Any tool other than Select: everything dimmed.
    s.set_tool(Tool::Pen);
    let a = s.path_availability();
    assert_eq!(a.combine, BooleanAvailability::NeedsTwo);
    assert!(!a.break_apart);
    s.set_tool(Tool::Node);
    assert!(!s.path_availability().break_apart);
}

#[test]
fn c1_break_apart_is_enabled_with_a_compound_path_in_the_selection() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(100.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let a = s.path_availability();
    assert!(a.break_apart, "a compound path is selected");
    assert_eq!(
        a.combine,
        BooleanAvailability::NeedsTwo,
        "only one object now"
    );
}

#[test]
fn c1_a_dimmed_press_does_nothing_and_never_changes_the_tool() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    let before = json(&s);
    for tool in [Tool::Select, Tool::Pen, Tool::Node] {
        s.set_tool(tool);
        let c = s.apply_combine();
        assert!(
            matches!(c, CombineOutcome::Ignored | CombineOutcome::Refused(_)),
            "{tool:?}: {c:?}"
        );
        let b = s.apply_break_apart();
        assert!(
            matches!(
                b,
                BreakApartOutcome::Ignored | BreakApartOutcome::Refused(_)
            ),
            "{tool:?}: {b:?}"
        );
        assert_eq!(json(&s), before);
    }
}

// ---------------------------------------------------------------------------------------------
// Criteria 3 to 8: Combine
// ---------------------------------------------------------------------------------------------

#[test]
fn c4_concentric_discs_combine_into_a_ring_in_either_direction_and_either_order() {
    for outer_reversed in [false, true] {
        for inner_reversed in [false, true] {
            for inner_first in [false, true] {
                let d = Document::new(1);
                let outer = circle(100.0, 100.0, 20.0, outer_reversed);
                let inner = circle(100.0, 100.0, 10.0, inner_reversed);
                if inner_first {
                    d.create_path(&inner, true);
                    d.create_path(&outer, true);
                } else {
                    d.create_path(&outer, true);
                    d.create_path(&inner, true);
                }
                let mut s = session_of(&d);
                select_all(&mut s);
                let out = s.apply_combine();
                let (operands, holes, _) = applied_combine(&out);
                assert_eq!((operands, holes), (2, 1));
                let ps = paths(&s);
                assert_eq!(ps.len(), 1);
                let p = &ps[0];
                assert!(p.is_compound());
                assert!(painted(p, 115.0, 100.0), "radius 15 is painted");
                assert!(!painted(p, 100.0, 100.0), "the centre is a hole");
                let area: f64 = outlines(p).iter().map(|o| shoelace(o)).sum::<f64>().abs();
                assert!(
                    (area - std::f64::consts::PI * 300.0).abs() < 1.0,
                    "area {area} (reversed {outer_reversed}/{inner_reversed}, inner first {inner_first})"
                );
                // Opposite windings: shape one way, hole the other.
                let signs: Vec<f64> = outlines(p).iter().map(|o| shoelace(o).signum()).collect();
                assert_ne!(signs[0], signs[1]);
            }
        }
    }
}

#[test]
fn c4_a_third_disc_inside_the_hole_is_painted_again() {
    let d = Document::new(1);
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    d.create_path(&circle(100.0, 100.0, 10.0, true), true);
    d.create_path(&circle(100.0, 100.0, 5.0, true), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let (n, holes, _) = applied_combine(&s.apply_combine());
    assert_eq!((n, holes), (3, 1), "depth 1 is a hole, depth 2 is a shape");
    let p = &paths(&s)[0];
    assert!(painted(p, 100.0, 100.0), "the island is painted");
    assert!(!painted(p, 107.0, 100.0), "the hole between");
    assert!(painted(p, 115.0, 100.0));
}

#[test]
fn c5_nothing_is_flattened_and_every_node_keeps_its_data() {
    let d = Document::new(1);
    let a = circle(0.0, 0.0, 10.0, false);
    let b = circle(100.0, 0.0, 10.0, false);
    d.create_path(&a, true);
    d.create_path(&b, true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let p = &paths(&s)[0];
    let nodes: Vec<&NewAnchor> = p.subpaths().flat_map(|sp| sp.anchors.iter()).collect();
    assert_eq!(nodes.len(), 8);
    for (orig, got) in a.iter().chain(b.iter()).zip(nodes.iter()) {
        assert_eq!(orig.point, got.point);
        assert_eq!(orig.kind, got.kind);
        assert_eq!(orig.handle_in, got.handle_in);
        assert_eq!(orig.handle_out, got.handle_out);
    }
}

#[test]
fn c5_a_reversed_outline_draws_the_same_curve() {
    // A nested pair where the inner one must be reversed: it keeps the same geometry.
    let d = Document::new(1);
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    let inner = circle(100.0, 100.0, 10.0, false); // same direction as the outer -> reversed
    d.create_path(&inner, true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let p = &paths(&s)[0];
    let sub: Vec<&[NewAnchor]> = p.subpaths().map(|sp| sp.anchors).collect();
    assert_eq!(sub[1].len(), 4);
    // Same set of node positions, handles swapped with the order reversed.
    let want: HashSet<(i64, i64)> = inner
        .iter()
        .map(|a| ((a.point.x * 1e6) as i64, (a.point.y * 1e6) as i64))
        .collect();
    let got: HashSet<(i64, i64)> = sub[1]
        .iter()
        .map(|a| ((a.point.x * 1e6) as i64, (a.point.y * 1e6) as i64))
        .collect();
    assert_eq!(want, got);
    for g in sub[1] {
        let o = inner.iter().find(|o| o.point == g.point).unwrap();
        assert_eq!(g.handle_in, o.handle_out, "in and out swapped");
        assert_eq!(g.handle_out, o.handle_in, "in and out swapped");
    }
}

#[test]
fn c6_c7_style_place_selection_notice_data_and_one_commit() {
    let d = Document::new(1);
    let plate = d.create_path(&square(0.0, 0.0, 200.0), true);
    style_edit(&d, plate, 200, 0, 0, 1.5);
    let mid = d.create_path(&square(300.0, 0.0, 10.0), true); // unrelated, sits above the plate
    for (x, y) in [(30.0, 30.0), (100.0, 30.0), (30.0, 100.0)] {
        let c = d.create_path(&circle(x, y, 8.0, false), true);
        style_edit(&d, c, 0, 0, 200, 0.5);
    }
    let red = d.path(plate).unwrap().style.clone();
    let mut s = session_of(&d);
    marquee(&mut s, pt(-10.0, -10.0), pt(250.0, 250.0));
    let before = labels(&reread(&s));
    let out = s.apply_combine();
    let (operands, holes, differ) = applied_combine(&out);
    assert_eq!((operands, holes, differ), (4, 3, true));
    assert_eq!(new_labels(&before, &s), vec!["combine_paths".to_string()]);
    let doc = reread(&s);
    let ids = doc.object_ids();
    assert_eq!(ids.len(), 2, "the result and the unrelated square");
    assert_eq!(ids[0], doc.object_ids()[0]);
    // The result takes the plate's place (bottom).
    let result = doc.path(ids[0]).unwrap();
    assert!(result.is_compound());
    assert_eq!(result.style, red, "the bottom-most operand's style");
    assert_eq!(doc.path(ids[1]).unwrap().anchors.len(), 4);
    assert_eq!(ids[1], mid, "the unrelated square keeps its place and id");
    assert_eq!(result.rotation.as_radians(), 0.0);
    assert_eq!(s.selected_object_count(), 1, "the result alone is selected");
    assert_eq!(s.path_availability().break_apart, true);
    assert_eq!(result.subpaths().count(), 4);
}

#[test]
fn c3_outline_order_is_the_stacking_order_from_the_bottom() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 10.0), true);
    d.create_path(&square(100.0, 0.0, 10.0), true);
    d.create_path(&square(200.0, 0.0, 10.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let p = &paths(&s)[0];
    let firsts: Vec<f64> = p.subpaths().map(|sp| sp.anchors[0].point.x).collect();
    assert_eq!(firsts, vec![0.0, 100.0, 200.0]);
}

#[test]
fn c8_the_result_does_not_depend_on_how_the_objects_were_selected() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 10.0), true);
    d.create_path(&square(100.0, 0.0, 10.0), true);
    d.create_path(&square(200.0, 0.0, 10.0), true);
    let mut a = session_of(&d);
    select_all(&mut a);
    a.apply_combine();
    let mut b = session_of(&d);
    // Shift-clicks in reverse order.
    for x in [205.0, 105.0, 5.0] {
        b.pointer_hover(pt(x, 0.0), true, false);
        b.pointer_down(pt(x, 0.0), true);
        b.pointer_up(pt(x, 0.0), true, false);
    }
    assert_eq!(b.selected_object_count(), 3);
    b.apply_combine();
    let pa = &paths(&a)[0];
    let pb = &paths(&b)[0];
    let pts = |p: &PathSnapshot| -> Vec<(f64, f64)> {
        p.subpaths()
            .flat_map(|sp| sp.anchors.iter().map(|a| (a.point.x, a.point.y)))
            .collect()
    };
    assert_eq!(pts(pa), pts(pb));
}

#[test]
fn c7_equal_styles_report_no_style_difference_and_a_primitive_contributes_its_curves() {
    let d = Document::new(1);
    d.create_ellipse(curvyo_document_core::EllipseFrame {
        center: pt(50.0, 50.0),
        rx: Length::from_mm(20.0),
        ry: Length::from_mm(20.0),
    });
    d.create_path(&circle(200.0, 50.0, 20.0, false), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let out = s.apply_combine();
    let (n, holes, differ) = applied_combine(&out);
    assert_eq!((n, holes), (2, 0));
    let _ = differ;
    let p = &paths(&s)[0];
    assert_eq!(p.subpaths().count(), 2);
    assert!(p.subpaths().all(|sp| sp.anchors.len() >= 4));
    assert!(
        p.subpaths()
            .flat_map(|sp| sp.anchors.iter())
            .any(|a| a.handle_in.length() > 0.0),
        "the ellipse keeps its curves"
    );
}

// ---------------------------------------------------------------------------------------------
// Criteria 9 to 12: refusals
// ---------------------------------------------------------------------------------------------

fn refused(s: &mut Session) -> CombineRefusal {
    match s.apply_combine() {
        CombineOutcome::Refused(r) => r,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn assert_nothing_changed(s: &Session, before: &[u8], n_labels: usize, selected: usize) {
    assert_eq!(json(s), before, "the document is unchanged");
    assert_eq!(labels(&reread(s)).len(), n_labels, "no commit");
    assert_eq!(s.selected_object_count(), selected, "the same selection");
}

#[test]
fn c9_an_open_path_refuses_and_names_it() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(100.0, 0.0, 20.0), true);
    let open = d.create_path(
        &[corner(200.0, 0.0), corner(240.0, 0.0), corner(240.0, 30.0)],
        false,
    );
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    match refused(&mut s) {
        CombineRefusal::OpenPaths { offenders, of } => {
            assert_eq!(of, 3);
            assert_eq!(offenders, vec![open]);
        }
        other => panic!("{other:?}"),
    }
    assert_nothing_changed(&s, &before, n, 3);
    // A second activation gives the same result.
    assert!(matches!(refused(&mut s), CombineRefusal::OpenPaths { .. }));
    assert_nothing_changed(&s, &before, n, 3);
    assert_eq!(
        s.path_availability().combine,
        BooleanAvailability::OpenPaths { open: 1, of: 3 }
    );
}

#[test]
fn c10_overlapping_and_edge_sharing_squares_are_refused_a_small_gap_combines() {
    // Overlap.
    let d = Document::new(1);
    let a = d.create_path(&square(0.0, 0.0, 20.0), true);
    let b = d.create_path(&square(10.0, 10.0, 20.0), true);
    d.create_path(&square(200.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    match refused(&mut s) {
        CombineRefusal::Touching { offenders, of } => {
            assert_eq!(of, 3);
            let set: HashSet<NodeId> = offenders.into_iter().collect();
            assert_eq!(
                set,
                HashSet::from([a, b]),
                "both of the pair, not the third"
            );
        }
        other => panic!("{other:?}"),
    }
    assert_nothing_changed(&s, &before, n, 3);

    // Shared edge.
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(20.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    assert!(
        matches!(refused(&mut s), CombineRefusal::Touching { .. }),
        "shared edge"
    );

    // 0.01 mm apart combines.
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(20.01, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let (n, _, _) = applied_combine(&s.apply_combine());
    assert_eq!(n, 2);
}

#[test]
fn c10_the_touch_threshold_is_about_a_micrometre() {
    // 0.0005 mm: touching. 0.0035 mm: not touching (the kernel's guarantee is 0.001 / 0.003).
    for (gap, touches) in [(0.0005, true), (0.0035, false), (0.05, false)] {
        let d = Document::new(1);
        d.create_path(&square(0.0, 0.0, 20.0), true);
        d.create_path(&square(20.0 + gap, 0.0, 20.0), true);
        let mut s = session_of(&d);
        select_all(&mut s);
        let out = s.apply_combine();
        assert_eq!(
            matches!(
                out,
                CombineOutcome::Refused(CombineRefusal::Touching { .. })
            ),
            touches,
            "gap {gap}: {out:?}"
        );
    }
}

#[test]
fn c10_curved_outlines_tangent_or_crossing_are_refused() {
    for (dist, touches) in [(20.0, true), (19.0, true), (20.0005, true), (20.05, false)] {
        let d = Document::new(1);
        d.create_path(&circle(0.0, 0.0, 10.0, false), true);
        d.create_path(&circle(dist, 0.0, 10.0, false), true);
        let mut s = session_of(&d);
        select_all(&mut s);
        let out = s.apply_combine();
        assert_eq!(
            matches!(
                out,
                CombineOutcome::Refused(CombineRefusal::Touching { .. })
            ),
            touches,
            "centre distance {dist}: {out:?}"
        );
    }
}

#[test]
fn c10_a_hole_touching_its_plate_is_refused() {
    // The inner square touches the outer's edge from inside.
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 100.0), true);
    d.create_path(&square(0.0, 40.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    assert!(matches!(refused(&mut s), CombineRefusal::Touching { .. }));
}

#[test]
fn c10a_a_figure_eight_is_refused_with_its_own_sentence_and_only_it_is_named() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(300.0, 0.0, 20.0), true);
    // A bowtie: the two diagonals cross.
    let eight = d.create_path(
        &[
            corner(100.0, 0.0),
            corner(140.0, 40.0),
            corner(140.0, 0.0),
            corner(100.0, 40.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    match refused(&mut s) {
        CombineRefusal::SelfTouching { offenders, of } => {
            assert_eq!(of, 3);
            assert_eq!(offenders, vec![eight]);
        }
        other => panic!("{other:?}"),
    }
    assert_nothing_changed(&s, &before, n, 3);
}

#[test]
fn c10a_touching_wins_over_self_crossing_when_both_occur() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(10.0, 10.0, 20.0), true);
    d.create_path(
        &[
            corner(300.0, 0.0),
            corner(340.0, 40.0),
            corner(340.0, 0.0),
            corner(300.0, 40.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    select_all(&mut s);
    assert!(matches!(refused(&mut s), CombineRefusal::Touching { .. }));
}

#[test]
fn c11_a_flat_shape_and_out_of_range_coordinates_are_refused() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    let flat = d.create_path(
        &[corner(100.0, 0.0), corner(120.0, 0.0), corner(140.0, 0.0)],
        true,
    );
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    match refused(&mut s) {
        CombineRefusal::NoArea { offenders, .. } => assert_eq!(offenders, vec![flat]),
        other => panic!("{other:?}"),
    }
    assert_nothing_changed(&s, &before, n, 2);

    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    let far = d.create_path(&square(2.0e7, 0.0, 20.0), true);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-10.0, -10.0), pt(2.1e7, 100.0));
    assert_eq!(s.selected_object_count(), 2);
    match refused(&mut s) {
        CombineRefusal::OutOfRange { offenders, .. } => assert_eq!(offenders, vec![far]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn c11_non_finite_coordinates_do_not_panic() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(
        &[
            corner(100.0, 0.0),
            corner(f64::NAN, 5.0),
            corner(120.0, 20.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    let out = s.apply_combine();
    // Any outcome but a panic or a corrupted document is acceptable; nothing may be written.
    if !matches!(out, CombineOutcome::Applied { .. }) {
        assert_eq!(json(&s), before);
    }
}

// ---------------------------------------------------------------------------------------------
// Criteria 13 to 18: Break apart
// ---------------------------------------------------------------------------------------------

/// Ring (20 / 10) with an island (5) inside the hole, combined.
fn ring_island_session() -> Session {
    let d = Document::new(1);
    let ring_outer = d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    style_edit(&d, ring_outer, 10, 120, 10, 1.0);
    d.create_path(&circle(100.0, 100.0, 10.0, true), true);
    d.create_path(&circle(100.0, 100.0, 5.0, false), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    s
}

#[test]
fn c13_ring_with_island_breaks_into_the_ring_and_the_disc() {
    let mut s = ring_island_session();
    let before = labels(&reread(&s));
    let compound = paths(&s)[0].clone();
    assert_eq!(compound.subpaths().count(), 3);
    let out = s.apply_break_apart();
    match out {
        BreakApartOutcome::Applied {
            compounds,
            pieces,
            with_holes,
            left_alone,
        } => {
            assert_eq!((compounds, pieces, left_alone), (1, 2, 0));
            assert!(with_holes, "the ring keeps its hole");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(new_labels(&before, &s), vec!["break_apart".to_string()]);
    let ps = paths(&s);
    assert_eq!(ps.len(), 2);
    assert!(ps[0].is_compound(), "ring with a hole");
    assert_eq!(ps[0].subpaths().count(), 2);
    assert!(
        !ps[1].is_compound(),
        "the island is an ordinary closed path"
    );
    assert!(ps[1].closed);
    assert_eq!(ps[1].anchors.len(), 4);
    // The island is not inside the ring's painted area; the ring's hole is still a hole.
    assert!(painted(&ps[0], 115.0, 100.0));
    assert!(
        !painted(&ps[0], 100.0, 100.0),
        "the hole stayed with its piece"
    );
    assert!(painted(&ps[1], 100.0, 100.0));
    // Style is copied to each piece; ids are new; every node is exactly as before.
    assert_eq!(ps[0].style, compound.style);
    assert_eq!(ps[1].style, compound.style);
    let old_ids: HashSet<AnchorId> = compound
        .subpaths()
        .flat_map(|sp| sp.anchors.iter().map(|a| a.id))
        .collect();
    let new_ids: HashSet<AnchorId> = ps
        .iter()
        .flat_map(|p| p.subpaths().flat_map(|sp| sp.anchors.iter().map(|a| a.id)))
        .collect();
    assert!(old_ids.is_disjoint(&new_ids), "new anchor ids");
    assert!(!reread(&s).object_ids().contains(&compound.id));
    let pts = |p: &PathSnapshot| -> Vec<(Point, Vec2, Vec2, AnchorKind)> {
        p.subpaths()
            .flat_map(|sp| {
                sp.anchors
                    .iter()
                    .map(|a| (a.point, a.handle_in, a.handle_out, a.kind))
            })
            .collect()
    };
    let old: Vec<_> = pts(&compound);
    let mut new: Vec<_> = pts(&ps[0]);
    new.extend(pts(&ps[1]));
    // Same nodes in the same order: ring outer, ring hole, island.
    assert_eq!(old, new, "node for node, no reversal");
    assert_eq!(s.selected_object_count(), 2, "all pieces are selected");
}

#[test]
fn c13_two_separate_squares_become_two_closed_paths() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 10.0), true);
    d.create_path(&square(100.0, 0.0, 10.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let out = s.apply_break_apart();
    assert!(
        matches!(
            out,
            BreakApartOutcome::Applied {
                compounds: 1,
                pieces: 2,
                with_holes: false,
                left_alone: 0
            }
        ),
        "{out:?}"
    );
    let ps = paths(&s);
    assert_eq!(ps.len(), 2);
    assert!(ps.iter().all(|p| !p.is_compound() && p.closed));
    assert_eq!(
        ps[0].anchors[0].point,
        pt(0.0, 0.0),
        "in the order of the first outline"
    );
    assert_eq!(ps[1].anchors[0].point, pt(100.0, 0.0));
}

#[test]
fn c13_c16_a_one_region_compound_path_is_left_alone_and_the_press_is_refused() {
    let d = Document::new(1);
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    d.create_path(&circle(100.0, 100.0, 10.0, true), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let before = json(&s);
    let n = labels(&reread(&s)).len();
    let id = reread(&s).object_ids()[0];
    let out = s.apply_break_apart();
    match out {
        BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds }) => {
            assert_eq!(compounds, 1);
        }
        other => panic!("{other:?}"),
    }
    assert_nothing_changed(&s, &before, n, 1);
    assert_eq!(reread(&s).object_ids(), vec![id], "ids kept");
}

#[test]
fn c16_plural_one_piece_refusal_counts_the_selected_compound_paths() {
    let d = Document::new(1);
    for cx in [100.0, 400.0] {
        d.create_path(&circle(cx, 100.0, 20.0, false), true);
        d.create_path(&circle(cx, 100.0, 10.0, true), true);
    }
    let mut s = session_of(&d);
    // Combine each ring separately.
    marquee(&mut s, pt(60.0, 60.0), pt(140.0, 140.0));
    s.apply_combine();
    marquee(&mut s, pt(360.0, 60.0), pt(440.0, 140.0));
    s.apply_combine();
    select_all(&mut s);
    assert_eq!(s.selected_object_count(), 2);
    match s.apply_break_apart() {
        BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds }) => {
            assert_eq!(compounds, 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn c16_break_apart_with_no_compound_path_is_refused_defensively() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 10.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    match s.apply_break_apart() {
        BreakApartOutcome::Refused(BreakApartRefusal::NoCompound) => {}
        BreakApartOutcome::Ignored => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(json(&s), before);
}

#[test]
fn c15_c17_pieces_sit_at_the_compounds_place_and_untouched_objects_stay_selected() {
    let d = Document::new(1);
    let below = d.create_path(&square(-100.0, 0.0, 10.0), true);
    // Compound of three separate squares in the middle of the stack.
    for x in [0.0, 100.0, 200.0] {
        d.create_path(&square(x, 0.0, 10.0), true);
    }
    let above = d.create_path(&square(500.0, 0.0, 10.0), true);
    let mut s = session_of(&d);
    // Combine only the three middle squares.
    marquee(&mut s, pt(-10.0, -10.0), pt(260.0, 50.0));
    s.apply_combine();
    let compound = reread(&s).object_ids()[1];
    // Select everything and break apart.
    select_all(&mut s);
    let ids_before = reread(&s).object_ids();
    assert_eq!(ids_before.len(), 3);
    assert_eq!(ids_before[1], compound);
    let out = s.apply_break_apart();
    assert!(matches!(
        out,
        BreakApartOutcome::Applied {
            compounds: 1,
            pieces: 3,
            ..
        }
    ));
    let ids = reread(&s).object_ids();
    assert_eq!(ids.len(), 5);
    assert_eq!(ids[0], below);
    assert_eq!(ids[4], above);
    let xs: Vec<f64> = ids[1..4]
        .iter()
        .map(|&i| reread(&s).path(i).unwrap().anchors[0].point.x)
        .collect();
    assert_eq!(
        xs,
        vec![0.0, 100.0, 200.0],
        "consecutive, in order of the first outline"
    );
    assert_eq!(
        s.selected_object_count(),
        5,
        "pieces and the untouched objects"
    );
}

#[test]
fn c13_c15_mixed_selection_leaves_a_one_region_compound_untouched() {
    let d = Document::new(1);
    // A ring (one region) and two separate squares to be combined (two regions).
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    d.create_path(&circle(100.0, 100.0, 10.0, true), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine();
    let ring_id = reread(&s).object_ids()[0];
    let ring_before = reread(&s).path(ring_id).unwrap();
    // Add two squares to the document by editing through a second session is not possible; draw
    // them with the Pen instead.
    s.set_tool(Tool::Pen);
    for sq in [
        [(300.0, 0.0), (330.0, 0.0), (330.0, 30.0), (300.0, 30.0)],
        [(400.0, 0.0), (430.0, 0.0), (430.0, 30.0), (400.0, 30.0)],
    ] {
        for (x, y) in sq {
            s.pointer_hover(pt(x, y), false, false);
            s.pointer_down(pt(x, y), false);
            s.pointer_up(pt(x, y), false, false);
        }
        s.pointer_hover(pt(sq[0].0, sq[0].1), false, false);
        s.pointer_down(pt(sq[0].0, sq[0].1), false);
        s.pointer_up(pt(sq[0].0, sq[0].1), false, false);
    }
    s.set_tool(Tool::Select);
    // Combine the two squares.
    marquee(&mut s, pt(280.0, -20.0), pt(460.0, 60.0));
    assert_eq!(s.selected_object_count(), 2);
    s.apply_combine();
    select_all(&mut s);
    assert_eq!(s.selected_object_count(), 2);
    let out = s.apply_break_apart();
    match out {
        BreakApartOutcome::Applied {
            compounds,
            pieces,
            left_alone,
            ..
        } => assert_eq!((compounds, pieces, left_alone), (1, 2, 1)),
        other => panic!("{other:?}"),
    }
    let doc = reread(&s);
    assert_eq!(
        doc.path(ring_id).unwrap(),
        ring_before,
        "same object, same ids"
    );
    assert_eq!(doc.object_ids().len(), 3);
    assert_eq!(s.selected_object_count(), 3);
}

#[test]
fn c18_combine_then_break_apart_and_back_again() {
    // Plate with a hole and a separate square.
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 100.0), true);
    d.create_path(&square(30.0, 30.0, 40.0), true);
    d.create_path(&square(300.0, 0.0, 50.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let orig: Vec<Vec<Point>> = paths(&s)
        .iter()
        .flat_map(|p| {
            p.subpaths()
                .map(|sp| sp.anchors.iter().map(|a| a.point).collect())
        })
        .collect();
    s.apply_combine();
    s.apply_break_apart();
    let ps = paths(&s);
    assert_eq!(ps.len(), 2, "the plate with its hole, and the square");
    assert!(ps[0].is_compound() && ps[0].subpaths().count() == 2);
    assert!(!ps[1].is_compound());
    // Geometry node for node (as a set of point lists, windings may differ for the hole).
    let got: Vec<HashSet<(i64, i64)>> = ps
        .iter()
        .flat_map(|p| {
            p.subpaths()
                .map(|sp| {
                    sp.anchors
                        .iter()
                        .map(|a| ((a.point.x * 1e6) as i64, (a.point.y * 1e6) as i64))
                        .collect()
                })
                .collect::<Vec<HashSet<(i64, i64)>>>()
        })
        .collect();
    let want: Vec<HashSet<(i64, i64)>> = orig
        .iter()
        .map(|o| {
            o.iter()
                .map(|p| ((p.x * 1e6) as i64, (p.y * 1e6) as i64))
                .collect()
        })
        .collect();
    for w in &want {
        assert!(got.contains(w), "outline {w:?} lost");
    }
    // Break apart then Combine: again one compound with the same outlines.
    select_all(&mut s);
    let (n, holes, _) = applied_combine(&s.apply_combine());
    assert_eq!((n, holes), (2, 1));
    let p = &paths(&s)[0];
    assert_eq!(p.subpaths().count(), 3);
}

#[test]
fn golden_file_combine_ring_island_opens_and_breaks_apart_into_two_objects() {
    let bytes = include_bytes!("fixtures/combine_ring_island.curvyo");
    let mut s = Session::open(5, bytes).expect("the golden file opens");
    s.resize_viewport(1600.0, 1000.0);
    s.set_tool(Tool::Select);
    let n = reread(&s).object_ids().len();
    assert_eq!(n, 1, "the saved result of Combine is one compound path");
    select_all(&mut s);
    match s.apply_break_apart() {
        BreakApartOutcome::Applied { pieces, .. } => assert_eq!(pieces, 2),
        other => panic!("{other:?}"),
    }
    assert_eq!(reread(&s).object_ids().len(), 2);
}

#[test]
fn c14_a_compound_path_remains_a_nonzero_even_after_break_apart_for_nested_depth_three() {
    // depth 0 shape, depth 1 hole, depth 2 shape, depth 3 hole.
    let d = Document::new(1);
    d.create_path(&circle(100.0, 100.0, 40.0, false), true);
    d.create_path(&circle(100.0, 100.0, 30.0, false), true);
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    d.create_path(&circle(100.0, 100.0, 10.0, false), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let (_, holes, _) = applied_combine(&s.apply_combine());
    assert_eq!(holes, 2);
    let p = &paths(&s)[0];
    assert!(painted(p, 135.0, 100.0));
    assert!(!painted(p, 125.0, 100.0));
    assert!(painted(p, 115.0, 100.0));
    assert!(!painted(p, 100.0, 100.0));
    match s.apply_break_apart() {
        BreakApartOutcome::Applied { pieces, .. } => assert_eq!(pieces, 2, "two regions"),
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Criterion 19: performance and robustness (release, --ignored)
// ---------------------------------------------------------------------------------------------

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let r = f();
    (r, t.elapsed().as_secs_f64())
}

fn big_select_all(s: &mut Session, extent: f64) {
    marquee(s, pt(-100.0, -100.0), pt(extent, extent));
}

#[test]
#[ignore = "release performance: cargo test --release -p curvyo-editor-wasm --test acceptance_0035_tester -- --ignored"]
fn c19_a_rectangle_with_1000_circles() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 1000.0), true);
    for i in 0..1000 {
        let x = 15.0 + f64::from(i % 40) * 24.0;
        let y = 15.0 + f64::from(i / 40) * 24.0;
        d.create_path(&circle(x, y, 8.0, false), true);
    }
    let mut s = session_of(&d);
    big_select_all(&mut s, 1200.0);
    let (out, t) = timed(|| s.apply_combine());
    eprintln!("(a) combine {t:.3} s");
    assert!(t < 2.0, "{t}");
    let (n, holes, _) = applied_combine(&out);
    assert_eq!((n, holes), (1001, 1000));
    let p = &paths(&s)[0];
    assert_eq!(p.subpaths().count(), 1001);
    let (out, t) = timed(|| s.apply_break_apart());
    eprintln!("(a) break apart {t:.3} s");
    assert!(t < 2.0, "{t}");
    assert!(
        matches!(
            out,
            BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds: 1 })
        ),
        "a plate with holes is one piece: {out:?}"
    );
}

#[test]
#[ignore = "release performance"]
fn c19_2000_disjoint_squares() {
    let d = Document::new(1);
    for i in 0..2000 {
        d.create_path(
            &square(f64::from(i % 50) * 12.0, f64::from(i / 50) * 12.0, 8.0),
            true,
        );
    }
    let mut s = session_of(&d);
    big_select_all(&mut s, 900.0);
    let (out, t) = timed(|| s.apply_combine());
    eprintln!("(b) combine {t:.3} s");
    assert!(t < 2.0, "{t}");
    let (n, holes, _) = applied_combine(&out);
    assert_eq!((n, holes), (2000, 0));
    assert_eq!(paths(&s)[0].subpaths().count(), 2000);
    let (out, t) = timed(|| s.apply_break_apart());
    eprintln!("(b) break apart {t:.3} s");
    assert!(t < 2.0, "{t}");
    assert!(
        matches!(out, BreakApartOutcome::Applied { pieces: 2000, .. }),
        "{out:?}"
    );
}

#[test]
#[ignore = "release performance"]
fn c19_500_nested_squares_alternate_their_windings() {
    let d = Document::new(1);
    for i in 0..500 {
        let m = f64::from(i) * 0.9;
        d.create_path(&square(m, m, 1000.0 - 2.0 * m), true);
    }
    let mut s = session_of(&d);
    big_select_all(&mut s, 1200.0);
    let (out, t) = timed(|| s.apply_combine());
    eprintln!("(c) combine {t:.3} s");
    assert!(t < 2.0, "{t}");
    let (n, holes, _) = applied_combine(&out);
    assert_eq!((n, holes), (500, 250));
    let p = &paths(&s)[0];
    let signs: Vec<f64> = p
        .subpaths()
        .map(|sp| {
            let poly: Vec<(f64, f64)> = sp.anchors.iter().map(|a| (a.point.x, a.point.y)).collect();
            shoelace(&poly).signum()
        })
        .collect();
    assert!(
        signs.windows(2).all(|w| w[0] != w[1]),
        "alternating windings"
    );
    let (out, t) = timed(|| s.apply_break_apart());
    eprintln!("(c) break apart {t:.3} s");
    assert!(t < 2.0, "{t}");
    assert!(
        matches!(out, BreakApartOutcome::Applied { pieces: 250, .. }),
        "{out:?}"
    );
}

#[test]
#[ignore = "release performance"]
fn c19_a_compound_path_of_5000_tiny_squares_breaks_into_5000_paths() {
    let d = Document::new(1);
    for i in 0..5000 {
        d.create_path(
            &square(f64::from(i % 100) * 3.0, f64::from(i / 100) * 3.0, 1.0),
            true,
        );
    }
    let mut s = session_of(&d);
    big_select_all(&mut s, 400.0);
    let (out, t) = timed(|| s.apply_combine());
    eprintln!("(d) combine {t:.3} s");
    assert!(t < 2.0, "{t}");
    applied_combine(&out);
    let (out, t) = timed(|| s.apply_break_apart());
    eprintln!("(d) break apart {t:.3} s");
    assert!(t < 2.0, "{t}");
    assert!(
        matches!(out, BreakApartOutcome::Applied { pieces: 5000, .. }),
        "{out:?}"
    );
    assert_eq!(reread(&s).object_ids().len(), 5000);
}

// ---------------------------------------------------------------------------------------------
// More edges
// ---------------------------------------------------------------------------------------------

/// A compound path built straight in the document from the given outlines (windings as given).
fn compound(d: &Document, outlines: Vec<Vec<NewAnchor>>) -> NodeId {
    let a = d.create_path(&square(-9000.0, -9000.0, 1.0), true);
    let b = d.create_path(&square(-9100.0, -9100.0, 1.0), true);
    let list: Vec<(Vec<NewAnchor>, bool)> = outlines.into_iter().map(|o| (o, true)).collect();
    d.replace_with_path(&[a, b], a, &list, "test_compound")
        .unwrap()
}

#[test]
fn c10a_a_compound_path_whose_own_outlines_touch_is_refused_as_self_touching() {
    let d = Document::new(1);
    let c = compound(&d, vec![square(0.0, 0.0, 20.0), square(10.0, 10.0, 20.0)]);
    d.create_path(&square(200.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let before = json(&s);
    match refused(&mut s) {
        CombineRefusal::SelfTouching { offenders, of } => {
            assert_eq!(of, 2);
            assert_eq!(offenders, vec![c]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(json(&s), before);
}

#[test]
fn c4_a_compound_operand_and_a_loose_shape_nest_together_by_depth() {
    // Ring (hole r10) + a disc in the hole (island) + a tiny disc in the ring material.
    let d = Document::new(1);
    d.create_path(&circle(100.0, 100.0, 20.0, false), true);
    d.create_path(&circle(100.0, 100.0, 10.0, true), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    s.apply_combine(); // the ring, one compound path
    let ring = reread(&s).object_ids()[0];
    // Add the island and the speck with the Pen is awkward; build a second document instead.
    let d = Document::new(1);
    let outer = circle(100.0, 100.0, 20.0, false);
    let hole = circle(100.0, 100.0, 10.0, true);
    compound(&d, vec![outer, hole]);
    d.create_path(&circle(100.0, 100.0, 5.0, true), true); // island in the hole: depth 2
    d.create_path(&circle(115.0, 100.0, 2.0, false), true); // speck in the material: depth 1
    let mut s = session_of(&d);
    select_all(&mut s);
    let (n, holes, _) = applied_combine(&s.apply_combine());
    assert_eq!(n, 3);
    assert_eq!(holes, 2, "the old hole and the speck are odd depth");
    let p = &paths(&s)[0];
    assert_eq!(p.subpaths().count(), 4);
    assert!(painted(p, 100.0, 100.0), "the island is painted");
    assert!(!painted(p, 107.0, 100.0), "the old hole");
    assert!(
        !painted(p, 115.0, 100.0),
        "the speck is a hole in the material"
    );
    assert!(painted(p, 118.0, 100.0), "material around the speck");
    let _ = ring;
}

#[test]
fn c10_two_identical_shapes_on_top_of_each_other_are_refused() {
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    assert!(matches!(refused(&mut s), CombineRefusal::Touching { .. }));
}

#[test]
fn c3_combining_three_nested_squares_in_the_reverse_stacking_order_still_nests_by_depth() {
    let d = Document::new(1);
    // Smallest at the bottom, largest on top: depth does not depend on the stacking order.
    d.create_path(&square(40.0, 40.0, 20.0), true);
    d.create_path(&square(20.0, 20.0, 60.0), true);
    d.create_path(&square(0.0, 0.0, 100.0), true);
    let mut s = session_of(&d);
    select_all(&mut s);
    let (n, holes, _) = applied_combine(&s.apply_combine());
    assert_eq!((n, holes), (3, 1));
    let p = &paths(&s)[0];
    assert!(painted(p, 50.0, 50.0), "depth 2 is a shape again");
    assert!(!painted(p, 30.0, 50.0), "depth 1 is a hole");
    assert!(painted(p, 10.0, 50.0));
    // Outline order is the stacking order from the bottom: smallest first.
    let first = p.subpaths().next().unwrap().anchors[0].point;
    assert_eq!(first, pt(40.0, 40.0));
}

// ---------------------------------------------------------------------------------------------
// M1: the Boolean card move changes no availability rule
// ---------------------------------------------------------------------------------------------

#[test]
fn m1_boolean_availability_is_unchanged_by_the_card_move_and_the_boolean_ops_still_run() {
    use curvyo_ui_core::BooleanOp;
    let d = Document::new(1);
    d.create_path(&square(0.0, 0.0, 20.0), true);
    d.create_path(&square(10.0, 10.0, 20.0), true);
    d.create_path(&[corner(200.0, 0.0), corner(240.0, 0.0)], false);
    let mut s = session_of(&d);
    assert_eq!(s.boolean_availability(), BooleanAvailability::NeedsTwo);
    marquee(&mut s, pt(-10.0, -10.0), pt(60.0, 60.0));
    assert_eq!(s.boolean_availability(), BooleanAvailability::Ready);
    assert_eq!(s.path_availability().combine, s.boolean_availability());
    select_all(&mut s);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::OpenPaths { open: 1, of: 3 }
    );
    assert_eq!(s.path_availability().combine, s.boolean_availability());
    s.set_tool(Tool::Node);
    assert_eq!(s.boolean_availability(), BooleanAvailability::NeedsTwo);
    s.set_tool(Tool::Select);
    // Union of the two overlapping squares still works (and Combine would refuse them).
    marquee(&mut s, pt(-10.0, -10.0), pt(60.0, 60.0));
    assert!(matches!(
        s.apply_boolean(BooleanOp::Union),
        curvyo_editor_wasm::BooleanOutcome::Applied { .. }
    ));
}
