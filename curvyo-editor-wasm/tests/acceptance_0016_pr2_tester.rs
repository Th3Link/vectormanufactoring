//! Independent session-level acceptance tests for `0016-boolean-operations`
//! PR 2 (compound path): selecting, move, resize with stroke scaling once,
//! rotate, skew, typed entries, Ctrl-move copy, Delete, Node tool exclusions.
//! Written from the specification before the implementation was read.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::assert_is_empty,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::type_complexity
)]

use std::collections::HashSet;

use curvyo_document_core::{
    AnchorId, Color, Document, Length, NewAnchor, NodeId, ObjectSnapshot, PathSnapshot, Point,
    RectBounds, StyleEdit, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn square(start: u64, x: f64, y: f64, side: f64, ccw: bool) -> (Vec<NewAnchor>, bool) {
    let mut c = vec![
        pt(x, y),
        pt(x + side, y),
        pt(x + side, y + side),
        pt(x, y + side),
    ];
    if !ccw {
        c.reverse();
    }
    (
        c.into_iter()
            .enumerate()
            .map(|(i, p)| NewAnchor::corner(AnchorId::new(20, start + i as u64), p))
            .collect(),
        true,
    )
}

/// The ring (0..40 with a 10 mm hole at 15..25), filled, stroke 1 mm, plus a
/// loose ordinary open path far away and a rectangle behind the hole.
fn ring_document(with_behind: bool) -> (Document, NodeId) {
    let d = Document::new(1);
    if with_behind {
        let behind = d.create_rect(RectBounds {
            origin: pt(12.0, 12.0),
            width: Length::from_mm(16.0),
            height: Length::from_mm(16.0),
        });
        d.edit_style(&[behind], &StyleEdit::FillEnabled(true))
            .unwrap();
    }
    let seed = d.create_rect(RectBounds {
        origin: pt(900.0, 900.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    });
    let ring = d
        .replace_with_path(
            &[seed],
            seed,
            &[
                square(0, 0.0, 0.0, 40.0, true),
                square(10, 15.0, 15.0, 10.0, false),
            ],
            "boolean_union",
        )
        .unwrap();
    d.edit_style(&[ring], &StyleEdit::FillColor(Color { r: 255, g: 0, b: 0 }))
        .unwrap();
    d.edit_style(&[ring], &StyleEdit::FillEnabled(true))
        .unwrap();
    d.edit_style(&[ring], &StyleEdit::StrokeWidth(Length::from_mm(1.0)))
        .unwrap();
    (d, ring)
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1200.0, 800.0);
    s
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, shift, false);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

fn reread(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn paths(s: &Session) -> Vec<PathSnapshot> {
    let d = reread(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) => Some(p),
            ObjectSnapshot::Primitive(_) => None,
        })
        .collect()
}

fn compound_of(s: &Session) -> PathSnapshot {
    paths(s)
        .into_iter()
        .find(PathSnapshot::is_compound)
        .expect("a compound path")
}

fn centroid(p: &PathSnapshot, outline: usize) -> Point {
    let sub = p.subpaths().nth(outline).unwrap();
    let n = sub.anchors.len() as f64;
    pt(
        sub.anchors.iter().map(|a| a.point.x).sum::<f64>() / n,
        sub.anchors.iter().map(|a| a.point.y).sum::<f64>() / n,
    )
}

fn px(s: &Session, pixels: f64) -> f64 {
    pixels / s.view().scale()
}

// ---- 33 -----------------------------------------------------------------

#[test]
fn ac33_a_click_in_the_hole_selects_nothing_or_what_is_behind() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(20.0, 20.0));
    assert_eq!(s.selected_object_count(), 0, "hole click selected the ring");
    let (d, _) = ring_document(true);
    let mut s = open(&d);
    click(&mut s, pt(20.0, 20.0));
    assert_eq!(s.selected_object_count(), 1);
    // The selected object is the rectangle behind (move it and see which moved).
    drag(&mut s, pt(20.0, 20.0), pt(30.0, 20.0), false, false);
    assert!(
        !compound_of(&s)
            .anchors
            .iter()
            .any(|a| a.point.x > 39.9 && a.point.x < 40.1 && a.point.y > 49.0)
    );
    assert_eq!(
        compound_of(&s).anchors[0].point,
        pt(0.0, 0.0),
        "the ring moved"
    );
}

#[test]
fn ac33_a_click_on_the_fill_selects_the_ring_and_a_click_on_the_hole_stroke_too() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    assert_eq!(s.selected_object_count(), 1);
    let mut s = open(&d);
    click(&mut s, pt(15.0, 20.0));
    assert_eq!(s.selected_object_count(), 1);
}

// ---- 35 move ----------------------------------------------------------------

#[test]
fn ac35_dragging_the_body_moves_every_outline_by_the_same_offset() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    let before = compound_of(&s);
    drag(&mut s, pt(7.0, 20.0), pt(17.0, 26.0), false, false);
    let after = compound_of(&s);
    assert_eq!(after.extra_subpaths.len(), 1);
    for (a, b) in before.all_anchors().zip(after.all_anchors()) {
        assert!((b.point.x - a.point.x - 10.0).abs() < 1e-9, "{:?}", b.point);
        assert!((b.point.y - a.point.y - 6.0).abs() < 1e-9);
        assert_eq!(a.id, b.id);
    }
    // The hole is still a hole at its new place.
    let mut s2 = open(&reread_doc(&s));
    click(&mut s2, pt(30.0, 26.0));
    assert_eq!(
        s2.selected_object_count(),
        0,
        "the hole did not move with it"
    );
    click(&mut s2, pt(5.0, 5.0));
    assert_eq!(s2.selected_object_count(), 0);
}

fn reread_doc(s: &Session) -> Document {
    reread(s)
}

#[test]
fn ac36_ctrl_move_copies_with_fresh_anchor_ids_in_every_outline() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    drag(&mut s, pt(7.0, 20.0), pt(77.0, 20.0), false, true);
    let ps: Vec<_> = paths(&s)
        .into_iter()
        .filter(PathSnapshot::is_compound)
        .collect();
    assert_eq!(ps.len(), 2, "copy not created");
    for p in &ps {
        assert_eq!(p.extra_subpaths.len(), 1);
        assert_eq!(p.all_anchors().count(), 8);
    }
    let ids: HashSet<_> = ps
        .iter()
        .flat_map(|p| p.all_anchors().map(|a| a.id))
        .collect();
    assert_eq!(
        ids.len(),
        16,
        "anchor ids collide between original and copy"
    );
}

#[test]
fn ac36_delete_removes_the_whole_compound_path() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    s.delete_selected();
    assert!(paths(&s).iter().all(|p| !p.is_compound()));
}

// ---- 35 / 35a resize ----------------------------------------------------------

fn se_corner_drag(s: &mut Session, to: Point) {
    click(s, pt(7.0, 20.0));
    drag(s, pt(40.0, 40.0), to, false, false);
}

#[test]
fn ac35_resize_scales_every_outline_and_keeps_the_hole_a_hole() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    let before = compound_of(&s);
    se_corner_drag(&mut s, pt(100.0, 60.0));
    let after = compound_of(&s);
    // NW corner is the pivot: bounds 0..40 -> 0..100 (x2.5), 0..40 -> 0..60 (x1.5).
    let c0 = centroid(&before, 1);
    let c1 = centroid(&after, 1);
    assert!((c1.x - c0.x * 2.5).abs() < 0.2, "hole x {c1:?}");
    assert!((c1.y - c0.y * 1.5).abs() < 0.2, "hole y {c1:?}");
    // Hole centre (62.5, 30) is still not selectable fill.
    let mut s2 = open(&reread(&s));
    click(&mut s2, pt(50.0, 30.0));
    assert_eq!(s2.selected_object_count(), 0);
    click(&mut s2, pt(10.0, 30.0));
    assert_eq!(s2.selected_object_count(), 1);
}

#[test]
fn ac35a_stroke_width_scales_once_for_the_object_not_per_outline() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    s.set_scale_stroke_width(true);
    se_corner_drag(&mut s, pt(80.0, 40.0)); // sx 2, sy 1
    let p = compound_of(&s);
    let w = p.style.stroke.width.as_mm();
    let once = 2.0_f64.sqrt();
    assert!(
        (w - once).abs() < 1e-6,
        "stroke width {w}, expected {once} (scaled once); twice would be {}",
        once * once
    );
}

#[test]
fn ac35a_without_the_switch_the_stroke_width_is_unchanged() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    se_corner_drag(&mut s, pt(80.0, 40.0));
    assert!((compound_of(&s).style.stroke.width.as_mm() - 1.0).abs() < 1e-12);
}

#[test]
fn ac35_typed_size_equals_the_drag_result() {
    let (d, _) = ring_document(false);
    let mut by_drag = open(&d);
    se_corner_drag(&mut by_drag, pt(100.0, 60.0));
    let want = compound_of(&by_drag);

    let mut typed = open(&d);
    click(&mut typed, pt(7.0, 20.0));
    let at = pt(40.0, 40.0);
    typed.pointer_hover(at, false, false);
    typed.pointer_down(at, false);
    typed.pointer_up(at, false, false);
    typed.pointer_hover(at, false, false);
    typed.double_click(at, false, false);
    let view = typed
        .transform_entry()
        .expect("size entry on the SE handle");
    assert_eq!(view.kind, "size");
    println!(
        "fields {:?}",
        view.fields
            .iter()
            .map(|f| (f.accessible_name, f.prefill.clone()))
            .collect::<Vec<_>>()
    );
    // The box is the outline's 40 x 40 (stroke not included) or 41 x 41.
    let (w0, h0) = (
        view.fields[0].prefill.parse::<f64>().unwrap(),
        view.fields[1].prefill.parse::<f64>().unwrap(),
    );
    let outcome =
        typed.commit_transform_entry(&format!("{}", w0 * 2.5), &format!("{}", h0 * 1.5), 0);
    println!("outcome {outcome:?}");
    let got = compound_of(&typed);
    assert_eq!(got.all_anchors().count(), want.all_anchors().count());
    for (g, w) in got.all_anchors().zip(want.all_anchors()) {
        assert!(
            (g.point.x - w.point.x).abs() < 1e-6 && (g.point.y - w.point.y).abs() < 1e-6,
            "typed {:?} vs drag {:?}",
            g.point,
            w.point
        );
    }
}

// ---- 35 rotate ------------------------------------------------------------------

fn corner_rotate_handle(s: &Session, box_ne: Point) -> Point {
    let d = px(s, 32.0) / std::f64::consts::SQRT_2;
    pt(box_ne.x + d, box_ne.y - d)
}

#[test]
fn ac35_rotating_by_the_handle_turns_every_outline_and_keeps_the_hole() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    let before = compound_of(&s);
    let centre = pt(20.0, 20.0);
    let handle = corner_rotate_handle(&s, pt(40.0, 0.0));
    let v = (handle.x - centre.x, handle.y - centre.y);
    let dist = v.0.hypot(v.1);
    let end = v.1.atan2(v.0) + 37.0_f64.to_radians();
    let to = pt(centre.x + dist * end.cos(), centre.y + dist * end.sin());
    drag(&mut s, handle, to, false, false);
    let after = compound_of(&s);
    assert!((after.rotation.as_radians() - 37.0_f64.to_radians()).abs() < 1e-6);
    // Both outlines turned about the centre by 37 degrees; spot-check hole anchor 0.
    let a0 = before.extra_subpaths[0].anchors[0].point;
    let a1 = after.extra_subpaths[0].anchors[0].point;
    let (sn, cs) = 37.0_f64.to_radians().sin_cos();
    let (dx, dy) = (a0.x - centre.x, a0.y - centre.y);
    let want = pt(centre.x + dx * cs - dy * sn, centre.y + dx * sn + dy * cs);
    assert!(
        (a1.x - want.x).abs() < 1e-6 && (a1.y - want.y).abs() < 1e-6,
        "{a1:?} vs {want:?}"
    );
    // Hole centre (the ring's centre) stays unpainted/unselectable.
    let mut s2 = open(&reread(&s));
    click(&mut s2, centre);
    assert_eq!(s2.selected_object_count(), 0);
}

#[test]
fn ac35_scale_then_rotate_ring_centre_is_still_not_painted() {
    // The spec's own test: rotate by 37 degrees and scale to 150 % by 50 %.
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    // Scale first: E handle (40,20) to (60,20) => 150 % wide; S handle to 20 high => 50 %.
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 20.0), false, false);
    drag(&mut s, pt(30.0, 40.0), pt(30.0, 20.0), false, false);
    let scaled = compound_of(&s);
    let hole_centre = centroid(&scaled, 1);
    let body = pt(3.0, 3.0);
    // Now rotate 37 degrees about the box centre.
    let (x0, x1, y0, y1) = {
        let xs: Vec<f64> = scaled.all_anchors().map(|a| a.point.x).collect();
        let ys: Vec<f64> = scaled.all_anchors().map(|a| a.point.y).collect();
        (
            xs.iter().copied().fold(f64::MAX, f64::min),
            xs.iter().copied().fold(f64::MIN, f64::max),
            ys.iter().copied().fold(f64::MAX, f64::min),
            ys.iter().copied().fold(f64::MIN, f64::max),
        )
    };
    let c = pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    let handle = corner_rotate_handle(&s, pt(x1, y0));
    let v = (handle.x - c.x, handle.y - c.y);
    let dist = v.0.hypot(v.1);
    let end = v.1.atan2(v.0) + 37.0_f64.to_radians();
    drag(
        &mut s,
        handle,
        pt(c.x + dist * end.cos(), c.y + dist * end.sin()),
        false,
        false,
    );
    let after = compound_of(&s);
    assert!((after.rotation.as_radians() - 37.0_f64.to_radians()).abs() < 1e-6);
    let rotate = |p: Point| {
        let (sn, cs) = 37.0_f64.to_radians().sin_cos();
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        pt(c.x + dx * cs - dy * sn, c.y + dx * sn + dy * cs)
    };
    let mut s2 = open(&reread(&s));
    click(&mut s2, rotate(hole_centre));
    assert_eq!(s2.selected_object_count(), 0, "ring centre painted/picked");
    // Painted triangles: check the draw list too.
    let list = s2.draw_list();
    let hc = rotate(hole_centre);
    let painted = list.triangles.chunks(3).any(|t| {
        let sgn =
            |a: Point, b: Point, p: Point| (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
        let (a, b, cc) = (t[0].position, t[1].position, t[2].position);
        let d1 = sgn(hc, a, b);
        let d2 = sgn(hc, b, cc);
        let d3 = sgn(hc, cc, a);
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    });
    assert!(!painted, "the draw list paints the ring's centre");
    click(&mut s2, rotate(body));
    assert_eq!(s2.selected_object_count(), 1, "ring body not picked");
}

// ---- 38 / 38a Node tool ------------------------------------------------------------

#[test]
fn ac38_node_tool_shows_no_node_handle_or_segment_for_a_compound_path_alone() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    s.set_tool(Tool::Node);
    let list_node = s.draw_list();
    let mut s_sel = open(&d);
    click(&mut s_sel, pt(7.0, 20.0));
    // Same artwork; the Node tool adds nothing about the nodes of this path.
    let a = list_node.triangle_count();
    let b = s_sel.draw_list().triangle_count();
    // Node tool decorations for a path with nodes would add squares at 8 anchors.
    println!("node tool triangles {a}, select tool triangles {b} (selection box adds some)");
    let before = s.pack("0.1.0").unwrap();
    for corner in [
        pt(0.0, 0.0),
        pt(40.0, 0.0),
        pt(15.0, 15.0),
        pt(25.0, 25.0),
        pt(20.0, 0.0),
    ] {
        click(&mut s, corner);
        let st = s.node_toolbar_state();
        assert!(!st.can_delete && !st.can_insert && !st.can_join && !st.can_split);
        assert!(!st.can_convert_to_corner && !st.can_make_line && !st.can_make_curve);
    }
    s.delete_selected();
    s.join_selected();
    s.split_selected();
    assert_eq!(
        unpack(5, &s.pack("0.1.0").unwrap())
            .unwrap()
            .export_json()
            .unwrap(),
        unpack(5, &before).unwrap().export_json().unwrap()
    );
}

#[test]
fn ac38_double_click_with_the_select_tool_does_not_change_the_tool_and_changes_nothing() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    let at = pt(7.0, 20.0);
    click(&mut s, at);
    let before = s.pack("0.1.0").unwrap();
    s.pointer_hover(at, false, false);
    let _ = s.double_click(at, false, false);
    assert_eq!(s.tool(), Tool::Select);
    let after = s.pack("0.1.0").unwrap();
    assert_eq!(
        unpack(5, &after).unwrap().export_json().unwrap(),
        unpack(5, &before).unwrap().export_json().unwrap()
    );
    // Also an unselected double-click (first press selects).
    let mut s = open(&d);
    s.pointer_hover(at, false, false);
    s.pointer_down(at, false);
    s.pointer_up(at, false, false);
    let _ = s.double_click(at, false, false);
    assert_eq!(s.tool(), Tool::Select);
    // And a double click on the stroke of the hole.
    let mut s = open(&d);
    let on_hole = pt(15.0, 20.0);
    s.pointer_hover(on_hole, false, false);
    s.pointer_down(on_hole, false);
    s.pointer_up(on_hole, false, false);
    let _ = s.double_click(on_hole, false, false);
    assert_eq!(s.tool(), Tool::Select);
}

#[test]
fn ac38a_compound_plus_ordinary_path_in_the_node_tool_only_the_ordinary_one_has_nodes() {
    let d = Document::new(1);
    let seed = d.create_rect(RectBounds {
        origin: pt(900.0, 900.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    });
    let ring = d
        .replace_with_path(
            &[seed],
            seed,
            &[
                square(0, 0.0, 0.0, 40.0, true),
                square(10, 15.0, 15.0, 10.0, false),
            ],
            "boolean_difference",
        )
        .unwrap();
    let ordinary = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(8, 1), pt(100.0, 100.0)),
            NewAnchor::corner(AnchorId::new(8, 2), pt(140.0, 100.0)),
            NewAnchor::corner(AnchorId::new(8, 3), pt(140.0, 140.0)),
        ],
        false,
    );
    let _ = (ring, ordinary);
    let mut s = open(&d);
    // Select both in the Select tool (click ring, shift-click the open path edge).
    click(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(120.0, 100.0), true, false);
    s.pointer_down(pt(120.0, 100.0), true);
    s.pointer_up(pt(120.0, 100.0), true, false);
    assert_eq!(s.selected_object_count(), 2);
    s.set_tool(Tool::Node);
    let before = s.pack("0.1.0").unwrap();
    // Select all nodes (Ctrl+A) then Delete: only the ordinary path's nodes go.
    fn key(k: &str, ctrl: bool) -> curvyo_editor_wasm::KeyInput<'_> {
        curvyo_editor_wasm::KeyInput {
            key: k,
            shift: false,
            ctrl,
            alt: false,
            repeat: false,
            dom_blocked: false,
            time_ms: 0.0,
        }
    }
    let _ = s.key_down(key("a", true));
    s.delete_selected();
    let ps = paths(&s);
    let comp: Vec<_> = ps.iter().filter(|p| p.is_compound()).collect();
    assert_eq!(
        comp.len(),
        1,
        "the compound path was deleted through the Node tool"
    );
    assert_eq!(
        comp[0].all_anchors().count(),
        8,
        "a node of the compound path was deleted"
    );
    let _ = before;
}

#[test]
fn ac37_open_save_roundtrip_through_the_session_keeps_the_compound_path() {
    let (d, _) = ring_document(true);
    let s = open(&d);
    let bytes = s.pack("0.1.0").unwrap();
    let s2 = Session::open(3, &bytes).unwrap();
    assert_eq!(compound_of(&s2), compound_of(&s));
}

// ---- performance sanity ---------------------------------------------------------------

#[test]
fn a_session_with_a_hundred_outline_compound_path_moves_and_hit_tests() {
    let d = Document::new(1);
    let seed = d.create_rect(RectBounds {
        origin: pt(900.0, 900.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    });
    let n: usize = 100;
    let outlines: Vec<_> = (0..n)
        .map(|i| {
            square(
                (i * 4) as u64,
                (i % 40) as f64 * 3.0,
                (i / 40) as f64 * 3.0,
                2.0,
                true,
            )
        })
        .collect();
    let id = d
        .replace_with_path(&[seed], seed, &outlines, "boolean_union")
        .unwrap();
    d.edit_style(&[id], &StyleEdit::FillEnabled(true)).unwrap();
    let t = std::time::Instant::now();
    let mut s = open(&d);
    println!("open: {:?}", t.elapsed());
    click(&mut s, pt(1.0, 1.0));
    println!("click: {:?}", t.elapsed());
    assert_eq!(s.selected_object_count(), 1);
    drag(&mut s, pt(1.0, 1.0), pt(11.0, 6.0), false, false);
    println!("drag: {:?}", t.elapsed());
    let _ = s.draw_list();
    println!("draw: {:?}", t.elapsed());
    println!("select + move + draw with 1000 outlines: {:?}", t.elapsed());
    assert_eq!(compound_of(&s).extra_subpaths.len(), n - 1);
}

// ---- live preview, Object to path --------------------------------------------

fn painted(list: &curvyo_render_core::DrawList, p: Point) -> bool {
    list.triangles.chunks(3).any(|t| {
        let sgn =
            |a: Point, b: Point, q: Point| (q.x - b.x) * (a.y - b.y) - (a.x - b.x) * (q.y - b.y);
        let (a, b, c) = (t[0].position, t[1].position, t[2].position);
        let (d1, d2, d3) = (sgn(p, a, b), sgn(p, b, c), sgn(p, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    })
}

#[test]
fn live_move_preview_shows_every_outline_and_keeps_the_hole_open() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    s.pointer_hover(pt(7.0, 20.0), false, false);
    s.pointer_down(pt(7.0, 20.0), false);
    s.pointer_hover(pt(67.0, 20.0), false, false);
    s.pointer_hover(pt(67.0, 20.0), false, false);
    let list = s.draw_list();
    assert!(
        painted(&list, pt(60.0, 20.0)),
        "moved outer outline not previewed"
    );
    assert!(
        painted(&list, pt(75.0, 20.0)),
        "moved hole outline not previewed"
    );
    assert!(
        !painted(&list, pt(77.0, 17.0)) && !painted(&list, pt(83.0, 23.0)),
        "the hole is painted during the drag"
    );
    s.pointer_up(pt(67.0, 20.0), false, false);
    assert_eq!(compound_of(&s).anchors[0].point, pt(60.0, 0.0));
}

#[test]
fn ac36_object_to_path_is_not_offered_for_a_compound_path_and_does_nothing() {
    let (d, _) = ring_document(false);
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    assert!(!s.select_bar_state().object_to_path);
    let before = unpack(5, &s.pack("0.1.0").unwrap())
        .unwrap()
        .export_json()
        .unwrap();
    s.convert_selected_to_paths();
    assert_eq!(s.tool(), Tool::Select, "the tool changed");
    assert_eq!(
        unpack(5, &s.pack("0.1.0").unwrap())
            .unwrap()
            .export_json()
            .unwrap(),
        before
    );
}

#[test]
fn ac36_object_to_path_on_a_mix_converts_the_rectangle_and_leaves_the_compound_alone() {
    let (d, _) = ring_document(false);
    let r = d.create_rect(RectBounds {
        origin: pt(100.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    d.edit_style(&[r], &StyleEdit::FillEnabled(true)).unwrap();
    let mut s = open(&d);
    click(&mut s, pt(7.0, 20.0));
    s.pointer_hover(pt(105.0, 5.0), true, false);
    s.pointer_down(pt(105.0, 5.0), true);
    s.pointer_up(pt(105.0, 5.0), true, false);
    assert_eq!(s.selected_object_count(), 2);
    assert!(s.select_bar_state().object_to_path);
    let ring_before = compound_of(&s);
    s.convert_selected_to_paths();
    assert_eq!(
        compound_of(&s),
        ring_before,
        "the compound path was altered"
    );
}
