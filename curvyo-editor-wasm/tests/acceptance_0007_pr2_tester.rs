//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 2, through
//! `Session`: hit-testing with fills (23, 27), the press order with option B
//! (28, 29), hover following the press, double-click and cursor on a filled
//! interior, selection and marquee with fills, and the draw order of the
//! session's own frame (26). Written from `specification.md` before the PR 2
//! implementation was read. Expected geometry is computed here.
//!
//! Objects are told apart by what the select bar offers for them (a
//! rectangle shows "Remove rounding", a star a ratio, a polygon points, an
//! ellipse and a path neither), or by which one a drag moved.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::similar_names, clippy::doc_markdown, missing_docs)]
#![allow(clippy::type_complexity, clippy::needless_pass_by_value)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color,
    Document, EllipseFrame, FillMode, FillModeTarget, GradientStop, InnerRatio, Length, NewAnchor,
    NodeId, ObjectSnapshot, Opacity, Point, PointCount, RectBounds, Shape, StarFrame, StopId,
    StopPosition, StyleEdit, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::DrawList;
use loro::{LoroDoc, LoroMap};

// ---------------------------------------------------------------- helpers

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn op(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn ellipse(d: &Document, cx: f64, cy: f64, rx: f64, ry: f64) -> NodeId {
    d.create_ellipse(EllipseFrame {
        center: pt(cx, cy),
        rx: mm(rx),
        ry: mm(ry),
    })
}

fn star(d: &Document, cx: f64, cy: f64, r: f64, ratio: f64) -> NodeId {
    d.create_star(
        StarFrame {
            center: pt(cx, cy),
            radius: mm(r),
            angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(ratio).unwrap(),
    )
}

fn polygon(d: &Document, cx: f64, cy: f64, r: f64) -> NodeId {
    d.create_polygon(
        StarFrame {
            center: pt(cx, cy),
            radius: mm(r),
            angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
        },
        PointCount::new(6).unwrap(),
    )
}

fn path(d: &Document, pts: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    d.create_path(&anchors, closed)
}

fn fill(d: &Document, id: NodeId) {
    fill_with(d, id, rgb(255, 0, 0), 1.0);
}

fn fill_with(d: &Document, id: NodeId, c: Color, a: f64) {
    d.edit_style(&[id], &StyleEdit::FillColor(c)).unwrap();
    d.edit_style(&[id], &StyleEdit::FillOpacity(op(a))).unwrap();
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id,
            seed_stops: vec![],
        }],
    )
    .unwrap();
}

fn stop(n: u64, p: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(9, n),
        position: StopPosition::new(p).unwrap(),
        color: rgb(0, 0, 255),
        opacity: op(1.0),
    }
}

fn gradient(d: &Document, id: NodeId, mode: FillMode) {
    d.set_fill_mode(
        mode,
        &[FillModeTarget {
            id,
            seed_stops: vec![stop(1, 0.0), stop(2, 1.0)],
        }],
    )
    .unwrap();
}

fn no_stroke(d: &Document, id: NodeId) {
    d.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
}

fn click_mod(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, false);
}

fn click(s: &mut Session, p: Point) {
    click_mod(s, p, false);
}

/// A click on an empty spot first, so no selection decides the press.
fn pick(s: &mut Session, p: Point) {
    click(s, pt(900.0, 900.0));
    click(s, p);
}

fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool) {
    s.pointer_hover(from, shift, false);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, false);
    s.pointer_up(to, shift, false);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    drag_mod(s, from, to, false);
}

/// What is selected, as a word.
fn selected(s: &Session) -> String {
    match s.selected_object_count() {
        0 => "none".to_owned(),
        1 => {
            let bar = s.select_bar_state();
            if bar.remove_rounding_shown {
                "rect".to_owned()
            } else if bar.ratio.is_some() {
                "star".to_owned()
            } else if bar.points.is_some() {
                "polygon".to_owned()
            } else if bar.object_to_path {
                "ellipse".to_owned()
            } else {
                "path".to_owned()
            }
        }
        n => format!("{n} objects"),
    }
}

/// The objects of the session's document in tree order.
fn objects(s: &Session) -> Vec<ObjectSnapshot> {
    let d = unpack(9, &s.pack("0.1.0").unwrap()).unwrap();
    d.object_ids()
        .into_iter()
        .map(|id| d.object(id).unwrap())
        .collect()
}

fn centre_of(o: &ObjectSnapshot) -> Point {
    match o {
        ObjectSnapshot::Primitive(p) => match &p.shape {
            Shape::Rect { bounds, .. } => pt(
                bounds.origin.x + bounds.width.as_mm() / 2.0,
                bounds.origin.y + bounds.height.as_mm() / 2.0,
            ),
            Shape::Ellipse { frame } => frame.center,
            other => panic!("centre_of: unsupported {other:?}"),
        },
        ObjectSnapshot::Path(p) => {
            let n = p.anchors.len() as f64;
            pt(
                p.anchors.iter().map(|a| a.point.x).sum::<f64>() / n,
                p.anchors.iter().map(|a| a.point.y).sum::<f64>() / n,
            )
        }
    }
}

/// Indices (tree order) of the objects whose centre moved against `before`.
fn moved(before: &[ObjectSnapshot], after: &[ObjectSnapshot]) -> Vec<usize> {
    (0..before.len())
        .filter(|&i| {
            let (a, b) = (centre_of(&before[i]), centre_of(&after[i]));
            (a.x - b.x).abs() > 1e-6 || (a.y - b.y).abs() > 1e-6
        })
        .collect()
}

/// The accent hover box of the frame: the bounding box of the vertices drawn
/// in accent at about 65 percent alpha (criterion 41, 65% since the PO's revision), if any.
fn hover_box(s: &Session) -> Option<(f64, f64, f64, f64)> {
    let list = s.draw_list();
    let mut it = list
        .triangles
        .iter()
        .filter(|v| {
            (v.color.r, v.color.g, v.color.b) == (0x2F, 0x6F, 0xEE)
                && (160..=170).contains(&v.color.a)
        })
        .map(|v| v.position)
        .peekable();
    it.peek()?;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in it {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    Some((x0, x1, y0, y1))
}

fn hover_at(s: &mut Session, p: Point) -> Option<(f64, f64, f64, f64)> {
    s.pointer_hover(p, false, false);
    hover_box(s)
}

/// The hover box surrounds the box `(x0, y0, x1, y1)` within 2 mm.
fn lights(b: Option<(f64, f64, f64, f64)>, want: (f64, f64, f64, f64)) -> bool {
    b.is_some_and(|b| {
        (b.0 - want.0).abs() < 2.0
            && (b.1 - want.2).abs() < 2.0
            && (b.2 - want.1).abs() < 2.0
            && (b.3 - want.3).abs() < 2.0
    })
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if d.abs() < 1e-18 {
        return false;
    }
    let l1 = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d;
    let l2 = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
    let l3 = 1.0 - l1 - l2;
    l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9
}

/// White canvas, artwork layers composited once each. Overlay is ignored.
fn composite_at(list: &DrawList, p: Point) -> [f64; 3] {
    let mut px = [255.0_f64; 3];
    let mut start = 0;
    for &end in list.layers() {
        let mut found = None;
        for t in list.triangles[start..end].as_chunks::<3>().0 {
            if in_triangle(p, t[0].position, t[1].position, t[2].position) {
                found = Some(t[0].color);
            }
        }
        if let Some(c) = found {
            let a = f64::from(c.a) / 255.0;
            px[0] = f64::from(c.r) * a + px[0] * (1.0 - a);
            px[1] = f64::from(c.g) * a + px[1] * (1.0 - a);
            px[2] = f64::from(c.b) * a + px[2] * (1.0 - a);
        }
        start = end;
    }
    px
}

fn near(px: [f64; 3], r: f64, g: f64, b: f64) -> bool {
    (px[0] - r).abs() < 2.0 && (px[1] - g).abs() < 2.0 && (px[2] - b).abs() < 2.0
}

// ------------------------------------------------ AC 23: fill makes a hit

#[test]
fn ac23_a_filled_interior_selects_and_an_unfilled_one_does_not() {
    let d = Document::new(1);
    let filled = rect(&d, 0.0, 0.0, 60.0, 40.0);
    fill(&d, filled);
    let hollow = rect(&d, 100.0, 0.0, 60.0, 40.0);
    let mut s = open(&d);
    pick(&mut s, pt(30.0, 20.0));
    assert_eq!(selected(&s), "rect");
    pick(&mut s, pt(300.0, 300.0));
    assert_eq!(selected(&s), "none");
    pick(&mut s, pt(130.0, 20.0));
    assert_eq!(selected(&s), "none", "a hollow interior is not clickable");
    pick(&mut s, pt(100.0, 20.0));
    assert_eq!(selected(&s), "rect", "its outline still is");
    let _ = hollow;
}

#[test]
fn ac23_a_filled_object_without_a_stroke_is_selected_by_its_fill() {
    let d = Document::new(1);
    let e = ellipse(&d, 50.0, 50.0, 30.0, 20.0);
    fill(&d, e);
    no_stroke(&d, e);
    let mut s = open(&d);
    pick(&mut s, pt(50.0, 50.0));
    assert_eq!(selected(&s), "ellipse");
    pick(&mut s, pt(300.0, 50.0));
    assert_eq!(selected(&s), "none");
    // The corner of the bounding box is outside the ellipse.
    pick(&mut s, pt(22.0, 33.0));
    assert_eq!(selected(&s), "none");
}

#[test]
fn ac23_opacity_zero_still_counts_as_filled_and_a_hollow_fill_mode_none_does_not() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 60.0, 40.0);
    fill_with(&d, r, rgb(1, 2, 3), 0.0);
    let mut s = open(&d);
    pick(&mut s, pt(30.0, 20.0));
    assert_eq!(selected(&s), "rect");
    // Fill switched to None again.
    d.set_fill_mode(
        FillMode::None,
        &[FillModeTarget {
            id: r,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let mut s = open(&d);
    pick(&mut s, pt(30.0, 20.0));
    assert_eq!(selected(&s), "none");
}

#[test]
fn ac23_linear_and_radial_fills_count_as_filled() {
    for mode in [FillMode::Linear, FillMode::Radial] {
        let d = Document::new(1);
        let r = rect(&d, 0.0, 0.0, 60.0, 40.0);
        gradient(&d, r, mode);
        let mut s = open(&d);
        pick(&mut s, pt(30.0, 20.0));
        assert_eq!(selected(&s), "rect", "{mode:?}");
    }
}

/// A container whose only rectangle is in linear gradient mode with exactly
/// these stops (built through the Loro registers, as a peer's merge could).
fn with_stops(stops: &[(u64, f64)]) -> Vec<u8> {
    let d = Document::new(1);
    let _ = rect(&d, 0.0, 0.0, 60.0, 40.0);
    let loro = LoroDoc::new();
    loro.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let tree = loro.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    meta.insert("fill_enabled", true).unwrap();
    meta.insert("fill_kind", "linear").unwrap();
    let list = meta.ensure_mergeable_movable_list("fill_stops").unwrap();
    for (n, p) in stops {
        let m = list.push_container(LoroMap::new()).unwrap();
        m.insert("id", StopId::new(5, *n).to_hex()).unwrap();
        m.insert("position", *p).unwrap();
        m.insert(
            "color",
            loro::LoroValue::from(vec![
                loro::LoroValue::from(0_i64),
                loro::LoroValue::from(0_i64),
                loro::LoroValue::from(255_i64),
            ]),
        )
        .unwrap();
        m.insert("opacity", 1.0).unwrap();
    }
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, b) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", bytes),
        ("document.json", b"{}".to_vec()),
    ] {
        w.start_file(name, o).unwrap();
        w.write_all(&b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn ac35_a_gradient_with_no_stops_is_not_clickable_and_with_one_or_many_stops_it_is() {
    for (stops, clickable) in [
        (vec![], false),
        (vec![(1, 0.5)], true),
        (vec![(1, 0.0), (2, 1.0)], true),
        ((1..=20).map(|n| (n, n as f64 / 20.0)).collect(), true),
    ] {
        let bytes = with_stops(&stops);
        let Ok(mut s) = Session::open(2, &bytes) else {
            panic!("a document with {} stops must open", stops.len());
        };
        s.set_tool(Tool::Select);
        click(&mut s, pt(30.0, 20.0));
        assert_eq!(
            selected(&s) == "rect",
            clickable,
            "{} stops: interior click",
            stops.len()
        );
        // And nothing is painted for 0 stops, a flat colour for 1 stop.
        let list = s.draw_list();
        let centre = composite_at(&list, pt(30.0, 20.0));
        if stops.is_empty() {
            assert!(near(centre, 255.0, 255.0, 255.0), "0 stops paint nothing");
        }
    }
}

#[test]
fn ac23_an_open_path_is_clickable_up_to_its_closing_chord() {
    let d = Document::new(1);
    let p = path(&d, &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], false);
    fill(&d, p);
    let mut s = open(&d);
    pick(&mut s, pt(80.0, 30.0));
    assert_eq!(selected(&s), "path");
    pick(&mut s, pt(300.0, 300.0));
    // The other side of the chord y = x is outside the painted area.
    pick(&mut s, pt(30.0, 80.0));
    assert_eq!(selected(&s), "none");
    // Unfilled: the inside of the triangle does nothing.
    let d = Document::new(1);
    let _ = path(&d, &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], false);
    let mut s = open(&d);
    pick(&mut s, pt(80.0, 30.0));
    assert_eq!(selected(&s), "none");
}

#[test]
fn ac23_a_closed_curved_path_is_bounded_by_its_real_closing_segment() {
    let d = Document::new(1);
    let a = NewAnchor {
        id: AnchorId::new(1, 1),
        point: pt(0.0, 0.0),
        handle_in: Vec2::new(0.0, 0.0),
        handle_out: Vec2::new(0.0, 60.0),
        kind: AnchorKind::Corner,
    };
    let b = NewAnchor {
        id: AnchorId::new(1, 2),
        point: pt(100.0, 0.0),
        handle_in: Vec2::new(0.0, 60.0),
        handle_out: Vec2::new(0.0, 0.0),
        kind: AnchorKind::Corner,
    };
    let p = d.create_path(&[a, b], true);
    fill(&d, p);
    let mut s = open(&d);
    // The bulge reaches y = 45 at x = 50.
    pick(&mut s, pt(50.0, 25.0));
    assert_eq!(selected(&s), "path");
    pick(&mut s, pt(300.0, 300.0));
    pick(&mut s, pt(50.0, 60.0));
    assert_eq!(selected(&s), "none", "above the arc");
    pick(&mut s, pt(50.0, -15.0));
    assert_eq!(selected(&s), "none", "below the chord");
}

#[test]
fn ac23_the_nonzero_interior_of_a_self_intersecting_path_includes_its_centre() {
    let d = Document::new(1);
    let pts: Vec<(f64, f64)> = (0..5)
        .map(|i| {
            let a = f64::from(i) * 4.0 * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2;
            (100.0 + 80.0 * a.cos(), 100.0 + 80.0 * a.sin())
        })
        .collect();
    let p = path(&d, &pts, true);
    fill(&d, p);
    let mut s = open(&d);
    pick(&mut s, pt(100.0, 100.0));
    assert_eq!(selected(&s), "path");
}

#[test]
fn ac23_the_true_shape_is_hit_not_its_bounding_box() {
    // Rounded rectangle: the cut-off corner.
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    d.set_corner_radius(&[r], mm(30.0)).unwrap();
    fill(&d, r);
    let mut s = open(&d);
    pick(&mut s, pt(2.0, 2.0));
    assert_eq!(selected(&s), "none", "outside the rounded corner");
    pick(&mut s, pt(30.0, 30.0));
    assert_eq!(selected(&s), "rect");
    // Star: the notch between two arms.
    let d = Document::new(1);
    let st = star(&d, 100.0, 100.0, 80.0, 0.4);
    fill(&d, st);
    let mut s = open(&d);
    let a = (-54.0_f64).to_radians();
    let notch = pt(100.0 + 62.0 * a.cos(), 100.0 + 62.0 * a.sin());
    pick(&mut s, notch);
    assert_eq!(selected(&s), "none", "between the arms");
    pick(&mut s, pt(100.0, 100.0));
    assert_eq!(selected(&s), "star");
    // Polygon (hexagon): outside a flat side.
    let d = Document::new(1);
    let pg = polygon(&d, 100.0, 100.0, 80.0);
    fill(&d, pg);
    let mut s = open(&d);
    pick(&mut s, pt(100.0, 100.0));
    assert_eq!(selected(&s), "polygon");
    pick(&mut s, pt(180.0, 100.0 - 60.0));
    assert_eq!(selected(&s), "none", "the circumscribed box corner area");
}

// -------------------------------------------------- AC 27: the hit order

#[test]
fn ac27_example_a_hollow_ellipse_over_a_filled_rectangle() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let _e = ellipse(&d, 50.0, 50.0, 20.0, 20.0);
    let mut s = open(&d);
    pick(&mut s, pt(70.0, 50.0));
    assert_eq!(selected(&s), "ellipse", "its outline");
    pick(&mut s, pt(50.0, 50.0));
    assert_eq!(
        selected(&s),
        "rect",
        "inside the ellipse, away from its outline"
    );
    pick(&mut s, pt(50.0, 90.0));
    assert_eq!(selected(&s), "rect");
    pick(&mut s, pt(0.0, 50.0));
    assert_eq!(selected(&s), "rect", "its own outline");
}

#[test]
fn ac27_example_a_filled_rectangle_covers_the_outline_of_a_circle_below_it() {
    let d = Document::new(1);
    let _c = ellipse(&d, 50.0, 50.0, 30.0, 30.0);
    let r = rect(&d, 40.0, 0.0, 120.0, 100.0);
    fill(&d, r);
    let mut s = open(&d);
    pick(&mut s, pt(80.0, 50.0));
    assert_eq!(
        selected(&s),
        "rect",
        "the circle outline is under the rectangle"
    );
    pick(&mut s, pt(20.0, 50.0));
    assert_eq!(selected(&s), "ellipse", "the visible part of the outline");
}

#[test]
fn ac27_nothing_filled_keeps_the_old_rule_nearest_outline_a_tie_to_the_topmost() {
    // Hollow A (edge x = 0) above hollow B (edge x = 0.6): the nearer edge wins,
    // whichever is on top.
    let d = Document::new(1);
    let _b = rect(&d, 0.6, 10.0, 80.0, 80.0);
    let _a = rect(&d, 0.0, 0.0, 100.0, 100.0);
    for (x, winner) in [(0.5, vec![0]), (0.1, vec![1])] {
        let mut s = open(&d);
        let before = objects(&s);
        drag(&mut s, pt(x, 50.0), pt(x, 60.0));
        assert_eq!(moved(&before, &objects(&s)), winner, "press at x = {x}");
    }
}

#[test]
fn ac27_an_object_below_the_topmost_filled_one_never_wins() {
    let d = Document::new(1);
    let _low = ellipse(&d, 50.0, 50.0, 20.0, 20.0);
    let top = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, top);
    let mut s = open(&d);
    for p in [pt(70.0, 50.0), pt(50.0, 30.0), pt(50.0, 50.0)] {
        pick(&mut s, p);
        assert_eq!(selected(&s), "rect", "at {p:?}");
    }
}

#[test]
fn ac27_two_filled_objects_the_topmost_wins_inside_both_and_the_lower_one_elsewhere() {
    let d = Document::new(1);
    let lower = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, lower);
    let upper = ellipse(&d, 80.0, 80.0, 40.0, 40.0);
    fill_with(&d, upper, rgb(0, 0, 255), 0.3);
    let mut s = open(&d);
    pick(&mut s, pt(80.0, 80.0));
    assert_eq!(
        selected(&s),
        "ellipse",
        "a translucent top fill still takes the click"
    );
    pick(&mut s, pt(10.0, 10.0));
    assert_eq!(selected(&s), "rect");
}

#[test]
fn ac27_among_objects_at_or_above_the_filled_one_the_nearest_outline_wins_not_the_topmost() {
    let d = Document::new(1);
    let f = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, f);
    // A hollow rectangle above: its left edge is at x = 0.8.
    let _g = rect(&d, 0.8, 10.0, 80.0, 80.0);
    for (x, winner) in [(0.2, vec![0]), (0.65, vec![1])] {
        let mut s = open(&d);
        let before = objects(&s);
        drag(&mut s, pt(x, 50.0), pt(x, 60.0));
        assert_eq!(moved(&before, &objects(&s)), winner, "press at x = {x}");
    }
}

#[test]
fn ac27_an_exact_tie_between_stacked_identical_outlines_goes_to_the_topmost() {
    for filled in [false, true] {
        let d = Document::new(1);
        let a = rect(&d, 0.0, 0.0, 100.0, 100.0);
        let b = rect(&d, 0.0, 0.0, 100.0, 100.0);
        if filled {
            fill(&d, a);
            fill(&d, b);
        }
        let mut s = open(&d);
        let before = objects(&s);
        drag(&mut s, pt(0.0, 50.0), pt(0.0, 70.0));
        let after = objects(&s);
        assert_eq!(moved(&before, &after), vec![1], "filled = {filled}");
    }
}

#[test]
fn ac27_hover_and_double_click_follow_the_same_rule_as_the_click() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let _e = ellipse(&d, 50.0, 50.0, 20.0, 20.0);
    let mut s = open(&d);
    assert!(
        lights(hover_at(&mut s, pt(70.0, 50.0)), (30.0, 30.0, 70.0, 70.0)),
        "ellipse outline"
    );
    assert!(
        lights(hover_at(&mut s, pt(50.0, 50.0)), (0.0, 0.0, 100.0, 100.0)),
        "the rectangle"
    );
    assert!(hover_at(&mut s, pt(300.0, 300.0)).is_none());
}

// --------------------------------------------- AC 28: the press, option B

fn two_rects_selected_first() -> (Document, Session) {
    // R: 100 mm filled rectangle, selected. E: 10 mm filled ellipse above.
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let e = ellipse(&d, 35.0, 65.0, 5.0, 5.0);
    fill_with(&d, e, rgb(0, 0, 255), 1.0);
    let mut s = open(&d);
    pick(&mut s, pt(20.0, 20.0));
    assert_eq!(selected(&s), "rect");
    (d, s)
}

#[test]
fn ac29_example_pressing_inside_the_small_filled_ellipse_selects_it_and_a_drag_moves_it() {
    let (_d, mut s) = two_rects_selected_first();
    let before = objects(&s);
    drag(&mut s, pt(35.0, 65.0), pt(45.0, 70.0));
    assert_eq!(selected(&s), "ellipse");
    assert_eq!(moved(&before, &objects(&s)), vec![1]);
}

#[test]
fn ac29_pressing_on_the_big_rectangle_away_from_the_ellipse_moves_the_rectangle() {
    let (_d, mut s) = two_rects_selected_first();
    let before = objects(&s);
    drag(&mut s, pt(20.0, 20.0), pt(30.0, 25.0));
    assert_eq!(selected(&s), "rect");
    assert_eq!(moved(&before, &objects(&s)), vec![0]);
}

#[test]
fn ac29_a_filled_object_below_the_selected_one_never_takes_the_press() {
    let d = Document::new(1);
    let low = ellipse(&d, 35.0, 65.0, 5.0, 5.0);
    fill_with(&d, low, rgb(0, 0, 255), 1.0);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let mut s = open(&d);
    click(&mut s, pt(20.0, 20.0));
    let before = objects(&s);
    drag(&mut s, pt(35.0, 65.0), pt(45.0, 70.0));
    assert_eq!(moved(&before, &objects(&s)), vec![1], "the rectangle moved");
    assert_eq!(selected(&s), "rect");
}

#[test]
fn ac29_an_unfilled_object_above_or_its_outline_never_takes_a_press_inside_the_box() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let _hollow = ellipse(&d, 50.0, 50.0, 20.0, 20.0);
    let mut s = open(&d);
    click(&mut s, pt(10.0, 10.0));
    assert_eq!(selected(&s), "rect");
    for (from, to) in [
        (pt(50.0, 50.0), pt(55.0, 55.0)),
        (pt(70.0, 50.0), pt(75.0, 55.0)),
    ] {
        let before = objects(&s);
        drag(&mut s, from, to);
        assert_eq!(moved(&before, &objects(&s)), vec![0], "press at {from:?}");
        assert_eq!(selected(&s), "rect");
        // Put it back.
        drag(&mut s, pt(from.x + 5.0, from.y + 5.0), from);
    }
}

#[test]
fn ac29_with_several_candidates_above_the_selection_the_topmost_wins() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let e = ellipse(&d, 35.0, 65.0, 12.0, 12.0);
    fill_with(&d, e, rgb(0, 0, 255), 1.0);
    let p = polygon(&d, 40.0, 65.0, 4.0);
    fill_with(&d, p, rgb(0, 255, 0), 1.0);
    let mut s = open(&d);
    click(&mut s, pt(10.0, 10.0));
    assert_eq!(selected(&s), "rect");
    // Both lie above the rectangle and contain the point: the polygon is on top.
    click(&mut s, pt(42.0, 65.0));
    assert_eq!(selected(&s), "polygon");
    // Outside the polygon's box, inside the ellipse only.
    click(&mut s, pt(35.0, 74.0));
    assert_eq!(selected(&s), "ellipse");
    // With the ellipse selected, the polygon above it takes a press in its area.
    click(&mut s, pt(42.0, 65.0));
    assert_eq!(selected(&s), "polygon");
}

#[test]
fn ac29_an_unfilled_selection_gives_its_press_to_a_filled_object_above_too() {
    let d = Document::new(1);
    let _s = rect(&d, 0.0, 0.0, 100.0, 100.0);
    let t = ellipse(&d, 35.0, 65.0, 8.0, 8.0);
    fill(&d, t);
    let mut s = open(&d);
    click(&mut s, pt(0.0, 50.0));
    assert_eq!(selected(&s), "rect");
    // Away from the ellipse and from every outline: a move of the hollow one.
    let before = objects(&s);
    drag(&mut s, pt(20.0, 20.0), pt(25.0, 22.0));
    assert_eq!(moved(&before, &objects(&s)), vec![0]);
    click(&mut s, pt(0.0, 50.0));
    let before = objects(&s);
    drag(&mut s, pt(35.0, 65.0), pt(45.0, 65.0));
    assert_eq!(selected(&s), "ellipse");
    assert_eq!(moved(&before, &objects(&s)), vec![1]);
}

#[test]
fn ac28_a_drawn_handle_wins_over_a_filled_object_lying_on_it() {
    let d = Document::new(1);
    let r = rect(&d, 20.0, 20.0, 100.0, 100.0);
    fill(&d, r);
    let t = ellipse(&d, 20.0, 20.0, 12.0, 12.0);
    fill_with(&d, t, rgb(0, 0, 255), 1.0);
    let mut s = open(&d);
    click(&mut s, pt(80.0, 80.0));
    assert_eq!(selected(&s), "rect");
    let before = objects(&s);
    drag(&mut s, pt(20.0, 20.0), pt(10.0, 10.0));
    let after = objects(&s);
    assert_eq!(
        selected(&s),
        "rect",
        "the handle drag does not select the ellipse"
    );
    // The rectangle was resized (the opposite corner stays), the ellipse stayed.
    assert_ne!(before[0], after[0]);
    assert_eq!(before[1], after[1]);
}

#[test]
fn ac28_shift_toggles_a_filled_object_in_and_out_of_the_selection() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 60.0, 60.0);
    fill(&d, a);
    let b = ellipse(&d, 120.0, 30.0, 30.0, 30.0);
    fill(&d, b);
    let mut s = open(&d);
    click(&mut s, pt(30.0, 30.0));
    assert_eq!(s.selected_object_count(), 1);
    // Shift on the filled interior of another object adds it.
    click_mod(&mut s, pt(120.0, 30.0), true);
    assert_eq!(s.selected_object_count(), 2);
    // Shift on the filled interior of a selected one removes it.
    click_mod(&mut s, pt(30.0, 30.0), true);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(selected(&s), "ellipse");
    // Sole selected, Shift inside its own filled interior: toggles it out.
    click_mod(&mut s, pt(140.0, 30.0), true);
    assert_eq!(s.selected_object_count(), 0);
}

#[test]
fn ac28_a_shift_press_inside_the_sole_selected_box_away_from_every_object_finds_nothing() {
    let d = Document::new(1);
    let _a = rect(&d, 0.0, 0.0, 100.0, 100.0);
    let mut s = open(&d);
    click(&mut s, pt(0.0, 50.0));
    assert_eq!(selected(&s), "rect");
    let before = s.pack("0.1.0").unwrap();
    click_mod(&mut s, pt(50.0, 50.0), true);
    assert_eq!(selected(&s), "rect", "the selection is unchanged");
    assert_eq!(s.pack("0.1.0").unwrap(), before);
}

#[test]
fn ac28_a_drag_starting_in_an_unselected_filled_shape_moves_it_and_in_a_hollow_one_marquees() {
    let d = Document::new(1);
    let f = rect(&d, 0.0, 0.0, 60.0, 60.0);
    fill(&d, f);
    let h = rect(&d, 100.0, 0.0, 60.0, 60.0);
    let _ = h;
    let mut s = open(&d);
    let before = objects(&s);
    drag(&mut s, pt(30.0, 30.0), pt(40.0, 35.0));
    assert_eq!(moved(&before, &objects(&s)), vec![0]);
    assert_eq!(selected(&s), "rect");
    // A drag from inside the hollow rectangle is a marquee: nothing moves and
    // the marquee, which encloses only the hollow rectangle, selects it.
    click(&mut s, pt(500.0, 500.0));
    let before = objects(&s);
    drag(&mut s, pt(105.0, 5.0), pt(170.0, 70.0));
    assert_eq!(moved(&before, &objects(&s)), Vec::<usize>::new());
    assert_eq!(selected(&s), "none");
}

#[test]
fn ac28_a_drag_from_empty_canvas_is_a_marquee_that_moves_and_writes_nothing() {
    // A press on empty canvas starts a marquee (`advanced-selection`); what
    // must hold with fills around is that nothing moves.
    let d = Document::new(1);
    let a = rect(&d, 10.0, 10.0, 30.0, 30.0);
    fill(&d, a);
    let _b = rect(&d, 60.0, 10.0, 30.0, 30.0);
    let mut s = open(&d);
    click(&mut s, pt(20.0, 20.0));
    assert_eq!(selected(&s), "rect");
    let bytes = s.pack("0.1.0").unwrap();
    drag(&mut s, pt(-10.0, -10.0), pt(120.0, 60.0));
    assert_eq!(
        selected(&s),
        "2 objects",
        "the rightward box contains both rectangles and replaces the selection"
    );
    assert_eq!(s.pack("0.1.0").unwrap(), bytes);
}

// ------------------------------------------------------ hover (AC 28, 41)

#[test]
fn ac28_hover_shows_over_a_filled_interior_and_not_over_a_hollow_one() {
    let d = Document::new(1);
    let f = rect(&d, 0.0, 0.0, 60.0, 60.0);
    fill(&d, f);
    let _h = rect(&d, 100.0, 0.0, 60.0, 60.0);
    let mut s = open(&d);
    assert!(lights(
        hover_at(&mut s, pt(30.0, 30.0)),
        (0.0, 0.0, 60.0, 60.0)
    ));
    assert!(hover_at(&mut s, pt(130.0, 30.0)).is_none());
    assert!(lights(
        hover_at(&mut s, pt(100.0, 30.0)),
        (100.0, 0.0, 160.0, 60.0)
    ));
    assert!(hover_at(&mut s, pt(400.0, 400.0)).is_none());
}

#[test]
fn ac28_hover_inside_the_sole_selected_box_lights_nothing_where_a_press_would_move() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let _hollow = ellipse(&d, 50.0, 30.0, 10.0, 10.0);
    let low = ellipse(&d, 50.0, 75.0, 8.0, 8.0);
    let _ = low;
    let top = ellipse(&d, 80.0, 75.0, 8.0, 8.0);
    fill_with(&d, top, rgb(0, 0, 255), 1.0);
    let mut s = open(&d);
    click(&mut s, pt(10.0, 10.0));
    assert_eq!(selected(&s), "rect");
    // Where a press moves the rectangle: nothing lights, even on the outline
    // of the hollow ellipse (inside the box) and on the hollow one's interior.
    assert!(hover_at(&mut s, pt(10.0, 50.0)).is_none(), "plain interior");
    assert!(
        hover_at(&mut s, pt(60.0, 30.0)).is_none(),
        "outline of an unfilled one"
    );
    assert!(
        hover_at(&mut s, pt(50.0, 30.0)).is_none(),
        "unfilled interior"
    );
    assert!(
        hover_at(&mut s, pt(50.0, 75.0)).is_none(),
        "below-selection hollow outline area"
    );
    // Where a press goes to the filled ellipse above: it lights.
    assert!(lights(
        hover_at(&mut s, pt(80.0, 75.0)),
        (72.0, 67.0, 88.0, 83.0)
    ));
}

#[test]
fn ac28_the_cursor_mirrors_the_press() {
    let (_d, mut s) = two_rects_selected_first();
    let hint = |s: &mut Session, p: Point| {
        s.pointer_hover(p, false, false);
        s.cursor_hint()
    };
    let empty = hint(&mut s, pt(400.0, 400.0));
    assert_eq!(empty, "default");
    let moving = hint(&mut s, pt(20.0, 20.0));
    assert_eq!(moving, "move", "a press would move the selection");
    // Where a press would select the ellipse above, it is not the move cursor.
    let selecting = hint(&mut s, pt(35.0, 65.0));
    assert_ne!(selecting, "move");
    // And a filled interior of an unselected object reads like its outline.
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let mut s = open(&d);
    let interior = hint(&mut s, pt(50.0, 50.0));
    let outline = hint(&mut s, pt(0.0, 50.0));
    assert_eq!(interior, outline);
}

// --------------------------------------------------- double click (AC 28)

fn dbl(s: &mut Session, p: Point) -> bool {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
    s.pointer_hover(p, false, false);
    let hint = s.double_click(p, false, false);
    s.pointer_hover(p, false, false);
    hint
}

#[test]
fn ac28_double_click_inside_a_filled_path_hands_it_to_the_node_tool() {
    let d = Document::new(1);
    let p = path(&d, &[(0.0, 0.0), (120.0, 0.0), (120.0, 80.0)], true);
    fill(&d, p);
    let mut s = open(&d);
    let _ = dbl(&mut s, pt(100.0, 20.0));
    assert_eq!(s.tool(), Tool::Node);
    // An unfilled path: the same point does nothing.
    let d = Document::new(1);
    let _ = path(&d, &[(0.0, 0.0), (120.0, 0.0), (120.0, 80.0)], true);
    let mut s = open(&d);
    let hint = dbl(&mut s, pt(100.0, 20.0));
    assert_eq!(s.tool(), Tool::Select);
    assert!(!hint);
}

#[test]
fn ac28_double_click_inside_a_filled_primitive_hints_and_in_a_hollow_one_does_not() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill(&d, r);
    let mut s = open(&d);
    assert!(dbl(&mut s, pt(50.0, 50.0)), "the edit hint");
    assert_eq!(s.tool(), Tool::Select);
    let d = Document::new(1);
    let _ = rect(&d, 0.0, 0.0, 100.0, 100.0);
    let mut s = open(&d);
    assert!(!dbl(&mut s, pt(50.0, 50.0)));
}

// ---------------------------------------- invisible objects (stroke + fill)

#[test]
fn an_object_with_no_stroke_and_no_fill_draws_nothing_and_hover_agrees_with_click() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    no_stroke(&d, r);
    let mut s = open(&d);
    assert_eq!(s.draw_list().triangles.len(), 0, "nothing is drawn");
    for p in [pt(0.0, 50.0), pt(50.0, 50.0), pt(400.0, 400.0)] {
        click(&mut s, pt(900.0, 900.0));
        let lit = hover_at(&mut s, p).is_some();
        click(&mut s, p);
        assert_eq!(
            lit,
            s.selected_object_count() == 1,
            "hover and click agree at {p:?}"
        );
    }
}

// ------------------------------------------------ AC 26: the session frame

#[test]
fn ac26_the_session_frame_draws_in_tree_order() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 80.0, 80.0);
    fill(&d, r);
    let p = path(&d, &[(10.0, 40.0), (70.0, 40.0)], false);
    d.edit_style(&[p], &StyleEdit::StrokeWidth(mm(10.0)))
        .unwrap();
    d.edit_style(&[p], &StyleEdit::StrokeColor(rgb(0, 0, 255)))
        .unwrap();
    let s = open(&d);
    let list = s.draw_list();
    assert!(
        near(composite_at(&list, pt(40.0, 40.0)), 0.0, 0.0, 255.0),
        "path above rect"
    );
    assert!(near(composite_at(&list, pt(40.0, 10.0)), 255.0, 0.0, 0.0));
    // Reverse: the rectangle is created last, so it covers the path.
    let d = Document::new(1);
    let p = path(&d, &[(10.0, 40.0), (70.0, 40.0)], false);
    d.edit_style(&[p], &StyleEdit::StrokeWidth(mm(10.0)))
        .unwrap();
    d.edit_style(&[p], &StyleEdit::StrokeColor(rgb(0, 0, 255)))
        .unwrap();
    let r = rect(&d, 0.0, 0.0, 80.0, 80.0);
    fill(&d, r);
    let s = open(&d);
    assert!(
        near(
            composite_at(&s.draw_list(), pt(40.0, 40.0)),
            255.0,
            0.0,
            0.0
        ),
        "rect above path"
    );
}

#[test]
fn ac26_a_selection_and_the_editor_lines_are_drawn_over_a_fill_never_under_it() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill_with(&d, r, rgb(0, 0, 0), 1.0);
    let mut s = open(&d);
    let plain = s.draw_list();
    click(&mut s, pt(50.0, 50.0));
    let list = s.draw_list();
    assert!(list.triangles.len() > plain.triangles.len());
    // The artwork is a prefix of the frame; the decorations are overlay.
    assert_eq!(list.layers(), plain.layers());
    assert!(list.overlay_start() < list.triangles.len());
    assert_eq!(
        &list.triangles[..list.overlay_start()],
        &plain.triangles[..]
    );
}

#[test]
fn ac41_the_hover_box_is_sixty_five_percent_accent_over_a_white_casing_and_stays_solid() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 100.0);
    fill_with(&d, r, rgb(0, 0, 0), 1.0);
    let mut s = open(&d);
    s.pointer_hover(pt(50.0, 50.0), false, false);
    let list = s.draw_list();
    let accent: Vec<_> = list
        .triangles
        .iter()
        .filter(|v| (v.color.r, v.color.g, v.color.b) == (0x2F, 0x6F, 0xEE))
        .collect();
    assert_ne!(accent.len(), 0);
    assert!(
        accent.iter().all(|v| (164..=168).contains(&v.color.a)),
        "65 percent"
    );
    let casing = list
        .triangles
        .iter()
        .skip(list.overlay_start())
        .filter(|v| {
            (v.color.r, v.color.g, v.color.b) == (255, 255, 255) && (164..=168).contains(&v.color.a)
        })
        .count();
    assert!(casing > 0, "a white casing at the same alpha");
}

// ------------------------------------------- white-box edges seen in the diff

#[test]
fn a_filled_primitive_hit_follows_its_rotation() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 100.0, 20.0);
    fill(&d, r);
    let turned = d.object(r).unwrap().rotated(
        pt(50.0, 10.0),
        Angle::from_radians(std::f64::consts::FRAC_PI_2),
    );
    d.rotate_object(&turned).unwrap();
    let mut s = open(&d);
    // Now a vertical bar x in [40, 60], y in [-40, 60].
    pick(&mut s, pt(50.0, -30.0));
    assert_eq!(selected(&s), "rect");
    pick(&mut s, pt(10.0, 10.0));
    assert_eq!(
        selected(&s),
        "none",
        "inside the old box, outside the turned shape"
    );
}

#[test]
fn degenerate_filled_paths_are_safe_to_click_and_hover() {
    let d = Document::new(1);
    let one = path(&d, &[(10.0, 10.0)], false);
    let two = path(&d, &[(30.0, 10.0), (30.0, 10.0)], true);
    let line = path(&d, &[(50.0, 10.0), (90.0, 10.0)], false);
    let flat = path(&d, &[(100.0, 10.0), (120.0, 10.0), (140.0, 10.0)], true);
    for id in [one, two, line, flat] {
        fill(&d, id);
    }
    let mut s = open(&d);
    for x in (0..160).step_by(5) {
        for y in [-5.0, 0.0, 10.0, 10.2, 15.0] {
            let p = pt(f64::from(x), y);
            let _ = hover_at(&mut s, p);
            pick(&mut s, p);
            let _ = dbl(&mut s, p);
            s.set_tool(Tool::Select);
            let _ = s.draw_list();
        }
    }
    // A zero-area fill has no interior: a point beside the line is not a hit.
    pick(&mut s, pt(70.0, 14.0));
    assert_eq!(selected(&s), "none");
    // The line's own outline still is.
    pick(&mut s, pt(70.0, 10.0));
    assert_eq!(selected(&s), "path");
}

#[test]
fn hovering_over_a_thousand_filled_objects_stays_interactive() {
    let d = Document::new(1);
    for i in 0..1000 {
        let r = rect(
            &d,
            f64::from(i % 40) * 12.0,
            f64::from(i / 40) * 12.0,
            10.0,
            10.0,
        );
        fill(&d, r);
    }
    let mut s = open(&d);
    let start = std::time::Instant::now();
    for i in 0..100 {
        s.pointer_hover(pt(f64::from(i) * 4.7, f64::from(i) * 4.3), false, false);
    }
    let per_hover = start.elapsed().as_secs_f64() * 10.0; // ms
    assert!(
        per_hover < 100.0,
        "{per_hover:.1} ms per hover with 1000 filled objects (debug build)"
    );
}
