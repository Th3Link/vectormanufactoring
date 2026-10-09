//! Independent tester cases for `0017-style-panel-rework`, block 1, render
//! side: odd dash lists repeat as SVG defines (criterion 32), a zero "on" entry
//! draws a dot with round and square caps, a gradient object from an old file
//! draws no fill (criteria 51, 53), and solid fills still draw as before.
//! Written from `specification.md` before the implementation was read. The
//! pictures are judged on a CPU compositor built from the [`DrawList`] contract
//! alone (same oracle as the 0007 tester file).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss, clippy::similar_names, clippy::doc_markdown)]
#![allow(clippy::needless_range_loop, clippy::type_complexity, dead_code)]
#![allow(
    missing_docs,
    clippy::assert_is_empty,
    clippy::semicolon_if_nothing_returned
)]

use curvyo_document_core::{
    AnchorId, Color, DashPattern, Document, EllipseFrame, Length, LineCap, NewAnchor, NodeId,
    ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit, ViewTransform, unpack,
};
use curvyo_render_core::{DrawList, build_artwork};

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

fn view(scale: f64) -> ViewTransform {
    ViewTransform::new(scale, pt(0.0, 0.0))
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn polyline(doc: &Document, points: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| anchor(i as u64 + 1, *x, *y))
        .collect();
    doc.create_path(&anchors, closed)
}

fn rect(doc: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    doc.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn obj(doc: &Document, id: NodeId) -> ObjectSnapshot {
    doc.object(id).unwrap()
}

fn style(doc: &Document, id: NodeId, edits: &[StyleEdit]) {
    for e in edits {
        doc.edit_style(&[id], e).unwrap();
    }
}

fn fill_solid(doc: &Document, id: NodeId, color: Color, opacity: f64) {
    style(
        doc,
        id,
        &[
            StyleEdit::FillColor(color),
            StyleEdit::FillOpacity(op(opacity)),
        ],
    );
    doc.edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
}

fn dash(list: &[f64]) -> StyleEdit {
    StyleEdit::StrokeDash(DashPattern::new(list.to_vec()).unwrap())
}

// ----------------------------------------------------------- compositing

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if d.abs() < 1e-18 {
        return false;
    }
    let l1 = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d;
    let l2 = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
    let l3 = 1.0 - l1 - l2;
    let e = -1e-9;
    l1 >= e && l2 >= e && l3 >= e
}

/// The layer ranges of the artwork part of `list`.
fn layer_ranges(list: &DrawList) -> Vec<(usize, usize)> {
    let mut start = 0;
    let mut out = Vec::new();
    for &end in list.layers() {
        out.push((start, end));
        start = end;
    }
    out
}

/// The colour a layer paints at `p`, if any triangle of it covers `p`.
fn layer_color_at(list: &DrawList, range: (usize, usize), p: Point) -> Option<[u8; 4]> {
    let t = &list.triangles[range.0..range.1];
    let mut found = None;
    for tri in t.as_chunks::<3>().0 {
        if in_triangle(p, tri[0].position, tri[1].position, tri[2].position) {
            let c = tri[0].color;
            found = Some([c.r, c.g, c.b, c.a]);
        }
    }
    found
}

/// White canvas, then every artwork layer composited once with "over".
fn composite_at(list: &DrawList, p: Point) -> [f64; 3] {
    let mut px = [255.0_f64; 3];
    for range in layer_ranges(list) {
        if let Some(c) = layer_color_at(list, range, p) {
            let a = f64::from(c[3]) / 255.0;
            for k in 0..3 {
                px[k] = f64::from(c[k]) * a + px[k] * (1.0 - a);
            }
        }
    }
    px
}

fn is_white(px: [f64; 3]) -> bool {
    px.iter().all(|v| (v - 255.0).abs() < 0.6)
}

fn near(px: [f64; 3], r: f64, g: f64, b: f64) -> bool {
    (px[0] - r).abs() < 2.0 && (px[1] - g).abs() < 2.0 && (px[2] - b).abs() < 2.0
}

fn covered(list: &DrawList, p: Point) -> bool {
    !is_white(composite_at(list, p))
}

fn art(objects: &[ObjectSnapshot], scale: f64) -> DrawList {
    build_artwork(objects, view(scale))
}

/// Both lists cover the same points of the box `(x0, x1, y0, y1)`, sampled
/// every 0.25 mm. A sample within 0.06 mm of an edge of either picture is
/// skipped: flattening tolerance may move an edge by that much, a join
/// difference is a region, not a rim.
fn assert_same_coverage(a: &DrawList, b: &DrawList, bx: (f64, f64, f64, f64)) {
    let steady = |l: &DrawList, p: Point| {
        let here = covered(l, p);
        [(0.06, 0.0), (-0.06, 0.0), (0.0, 0.06), (0.0, -0.06)]
            .iter()
            .all(|(dx, dy)| covered(l, pt(p.x + dx, p.y + dy)) == here)
    };
    let mut compared = 0;
    let mut y = bx.2;
    while y < bx.3 {
        let mut x = bx.0;
        while x < bx.1 {
            let p = pt(x + 0.0113, y + 0.0171);
            if steady(a, p) && steady(b, p) {
                assert_eq!(covered(a, p), covered(b, p), "coverage differs at {p:?}");
                compared += 1;
            }
            x += 0.25;
        }
        y += 0.25;
    }
    assert!(compared > 1000);
}

fn all_finite(list: &DrawList) -> bool {
    list.triangles
        .iter()
        .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
}

fn dashed_line(length: f64, width: f64, pattern: &[f64], cap: LineCap) -> ObjectSnapshot {
    let doc = Document::new(1);
    let p = polyline(&doc, &[(0.0, 0.0), (length, 0.0)], false);
    style(
        &doc,
        p,
        &[
            StyleEdit::StrokeWidth(mm(width)),
            dash(pattern),
            StyleEdit::StrokeCap(cap),
        ],
    );
    obj(&doc, p)
}

fn doubled(list: &[f64]) -> Vec<f64> {
    let mut twice = list.to_vec();
    twice.extend_from_slice(list);
    twice
}

// ------------------------------------------- AC 32: odd lists repeat (SVG)

#[test]
fn an_odd_list_draws_as_the_list_repeated_twice() {
    // `1 2 4` draws as `1 2 4 1 2 4` (criterion 32).
    let line = dashed_line(60.0, 1.0, &[1.0, 2.0, 4.0], LineCap::Butt);
    let list = art(&[line], 10.0);
    // on 0..1, off 1..3, on 3..7, off 7..8, on 8..10, off 10..14, then again.
    for (x, on) in [
        (0.5, true),
        (2.0, false),
        (5.0, true),
        (7.5, false),
        (9.0, true),
        (12.0, false),
        (14.5, true),
        (16.0, false),
        (19.0, true),
        (21.5, false),
        (23.0, true),
        (26.0, false),
        (28.5, true),
    ] {
        assert_eq!(covered(&list, pt(x, 0.0)), on, "x = {x}");
    }
}

#[test]
fn a_single_number_is_on_then_off_by_the_same_length() {
    let line = dashed_line(30.0, 1.0, &[3.0], LineCap::Butt);
    let list = art(&[line], 10.0);
    for (x, on) in [
        (1.5, true),
        (4.5, false),
        (7.5, true),
        (10.5, false),
        (13.5, true),
    ] {
        assert_eq!(covered(&list, pt(x, 0.0)), on, "x = {x}");
    }
}

#[test]
fn odd_and_long_lists_cover_the_same_points_as_their_doubled_lists() {
    let seventeen: Vec<f64> = (1..=17).map(|n| f64::from(n % 5 + 1)).collect();
    for odd in [
        vec![1.0, 2.0, 4.0],
        vec![5.0],
        vec![0.5, 1.5, 2.5, 3.5, 4.5],
        vec![6.0, 4.0, 1.0],
        seventeen,
    ] {
        for width in [0.5, 1.0, 2.0] {
            let a = art(&[dashed_line(200.0, width, &odd, LineCap::Butt)], 6.0);
            let b = art(
                &[dashed_line(200.0, width, &doubled(&odd), LineCap::Butt)],
                6.0,
            );
            assert_same_coverage(&a, &b, (0.0, 200.0, -1.0, 1.0));
        }
    }
}

#[test]
fn an_odd_list_rescales_with_the_width() {
    let narrow = art(
        &[dashed_line(60.0, 1.0, &[1.0, 2.0, 4.0], LineCap::Butt)],
        10.0,
    );
    let wide = art(
        &[dashed_line(60.0, 2.0, &[1.0, 2.0, 4.0], LineCap::Butt)],
        10.0,
    );
    // Width 2: on 0..2, off 2..6, on 6..14, off 14..16, on 16..20, off 20..28.
    for (x, on) in [
        (1.0, true),
        (4.0, false),
        (10.0, true),
        (15.0, false),
        (18.0, true),
        (24.0, false),
    ] {
        assert_eq!(covered(&wide, pt(x, 0.0)), on, "wide x = {x}");
    }
    assert!(covered(&narrow, pt(0.5, 0.0)));
    assert!(!covered(&narrow, pt(2.0, 0.0)));
}

#[test]
fn an_odd_list_is_not_confused_with_the_even_list_that_drops_the_last_number() {
    let odd = art(
        &[dashed_line(60.0, 1.0, &[1.0, 2.0, 4.0], LineCap::Butt)],
        10.0,
    );
    let even = art(&[dashed_line(60.0, 1.0, &[1.0, 2.0], LineCap::Butt)], 10.0);
    // Position 5.0 is on in the odd list (the 4 run) and off for 1 2 1 2.
    assert!(covered(&odd, pt(5.0, 0.0)));
    assert!(!covered(&even, pt(5.0, 0.0)));
}

// ------------------------------------ AC 32: a zero "on" entry is a dot

#[test]
fn a_zero_on_entry_with_round_caps_draws_a_dot_at_every_period() {
    // Width 2 mm, `0 3`: a dot of diameter 2 every 6 mm.
    let line = dashed_line(30.0, 2.0, &[0.0, 3.0], LineCap::Round);
    let list = art(&[line], 10.0);
    for k in 1..=3 {
        let c = f64::from(k) * 6.0;
        assert!(covered(&list, pt(c, 0.0)), "dot centre {c}");
        assert!(covered(&list, pt(c - 0.8, 0.0)), "dot left of {c}");
        assert!(covered(&list, pt(c + 0.8, 0.0)), "dot right of {c}");
        assert!(covered(&list, pt(c, 0.8)), "dot above {c}");
        assert!(covered(&list, pt(c, -0.8)), "dot below {c}");
        assert!(!covered(&list, pt(c + 1.3, 0.0)), "gap after {c}");
        assert!(!covered(&list, pt(c + 3.0, 0.0)), "gap middle after {c}");
        assert!(
            !covered(&list, pt(c + 0.9, 0.9)),
            "a dot is round, not square"
        );
    }
    assert!(covered(&list, pt(0.3, 0.0)), "the first dot at the start");
}

#[test]
fn a_zero_on_entry_with_square_caps_draws_a_square() {
    let line = dashed_line(30.0, 2.0, &[0.0, 3.0], LineCap::Square);
    let list = art(&[line], 10.0);
    for k in 1..=3 {
        let c = f64::from(k) * 6.0;
        assert!(covered(&list, pt(c, 0.0)));
        assert!(covered(&list, pt(c + 0.9, 0.9)), "square corner at {c}");
        assert!(covered(&list, pt(c - 0.9, -0.9)));
        assert!(!covered(&list, pt(c + 1.4, 0.0)));
    }
}

#[test]
fn a_zero_on_entry_with_butt_caps_draws_nothing() {
    let line = dashed_line(30.0, 2.0, &[0.0, 3.0], LineCap::Butt);
    let list = art(&[line], 10.0);
    let mut x = 0.0;
    while x < 30.0 {
        assert!(!covered(&list, pt(x + 0.013, 0.0)), "ink at {x}");
        x += 0.37;
    }
    assert!(all_finite(&list));
}

#[test]
fn a_zero_off_entry_draws_a_solid_line() {
    let line = dashed_line(60.0, 1.0, &[3.0, 0.0], LineCap::Butt);
    let list = art(&[line], 10.0);
    let mut x = 0.05;
    while x < 59.0 {
        assert!(covered(&list, pt(x, 0.0)), "gap at {x}");
        x += 0.41;
    }
}

#[test]
fn dots_follow_a_curve_and_a_closed_outline_without_breaking() {
    let doc = Document::new(1);
    let e = doc.create_ellipse(EllipseFrame {
        center: pt(0.0, 0.0),
        rx: mm(20.0),
        ry: mm(20.0),
    });
    style(
        &doc,
        e,
        &[
            StyleEdit::StrokeWidth(mm(2.0)),
            dash(&[0.0, 5.0]),
            StyleEdit::StrokeCap(LineCap::Round),
        ],
    );
    let list = art(&[obj(&doc, e)], 6.0);
    assert!(all_finite(&list));
    // Circumference 125.66 mm, period 10 mm: roughly 12 or 13 dots, each a
    // disc of radius 1 on the circle. Count them by sampling the circle.
    let mut inked = 0;
    let steps = 3600;
    let mut previous = false;
    for k in 0..=steps {
        let a = f64::from(k) / f64::from(steps) * std::f64::consts::TAU;
        let here = covered(&list, pt(20.0 * a.cos(), 20.0 * a.sin()));
        if here && !previous {
            inked += 1;
        }
        previous = here;
    }
    assert!((11..=14).contains(&inked), "{inked} dots");
}

// ------------------------------------------- robustness of odd/huge lists

#[test]
fn extreme_lists_stay_finite_and_do_not_hang() {
    for pattern in [
        vec![1e-300, 1e-300],
        vec![f64::MIN_POSITIVE],
        vec![1e300, 1.0],
        vec![1000.0; 17],
        vec![0.0, 1e-9],
        vec![0.0, 0.0, 1.0],
        vec![1.0; 99],
    ] {
        let list = art(&[dashed_line(500.0, 1.0, &pattern, LineCap::Round)], 4.0);
        assert!(all_finite(&list), "{pattern:?}");
        assert!(list.triangles.len() < 5_000_000, "{pattern:?}");
    }
    // A huge width times a huge multiple overflows to infinity: still finite.
    let doc = Document::new(1);
    let p = polyline(&doc, &[(0.0, 0.0), (100.0, 0.0)], false);
    style(
        &doc,
        p,
        &[
            StyleEdit::StrokeWidth(mm(1000.0)),
            dash(&[1000.0, 1000.0, 1000.0]),
        ],
    );
    assert!(all_finite(&art(&[obj(&doc, p)], 1.0)));
}

// ------------------------- AC 51, 53: old gradient objects draw no fill

fn legacy_objects() -> Vec<ObjectSnapshot> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../curvyo-document-core/tests/fixtures/legacy_gradient_v7.curvyo");
    let doc = unpack(5, &std::fs::read(path).unwrap()).unwrap();
    doc.object_ids()
        .into_iter()
        .map(|id| doc.object(id).unwrap())
        .collect()
}

#[test]
fn a_gradient_object_from_an_old_file_has_no_fill_and_nothing_inside() {
    let objects = legacy_objects();
    let list = art(&objects, 6.0);
    assert!(all_finite(&list));
    // Linear path (0,0)-(30,0)-(30,20), open: stroke red 1.5 mm dashed, no fill.
    assert!(
        !covered(&list, pt(25.0, 5.0)),
        "interior of the linear gradient object"
    );
    // Radial rectangle at (50,0) 40x20: stroke off and fill off: nothing at all.
    for (x, y) in [
        (70.0, 10.0),
        (51.0, 1.0),
        (50.0, 10.0),
        (90.0, 10.0),
        (70.0, 0.0),
        (70.0, 20.0),
    ] {
        assert!(!covered(&list, pt(x, y)), "ink at ({x}, {y})");
    }
}

#[test]
fn the_solid_fill_next_to_the_gradient_objects_still_draws_with_its_alpha() {
    let list = art(&legacy_objects(), 6.0);
    // Ellipse at (20, 60), blue fill at 0.2 over white.
    let px = composite_at(&list, pt(20.0, 60.0));
    assert!(near(px, 204.0, 204.0, 255.0), "{px:?}");
}

#[test]
fn a_solid_fill_of_opacity_zero_draws_nothing_visible_but_is_still_a_fill() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
    fill_solid(&doc, r, rgb(255, 0, 0), 0.0);
    style(&doc, r, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, r)], 6.0);
    assert!(is_white(composite_at(&list, pt(10.0, 10.0))));
    assert!(all_finite(&list));
}

#[test]
fn solid_fills_render_unchanged_for_every_alpha_byte_of_an_eight_digit_colour() {
    for aa in [0u32, 1, 127, 128, 254, 255] {
        let doc = Document::new(1);
        let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
        style(&doc, r, &[StyleEdit::StrokeEnabled(false)]);
        doc.edit_style(
            &[r],
            &StyleEdit::FillRgba(rgb(0, 0, 255), op(f64::from(aa) / 255.0)),
        )
        .unwrap();
        doc.edit_style(&[r], &StyleEdit::FillEnabled(true)).unwrap();
        let list = art(&[obj(&doc, r)], 6.0);
        let px = composite_at(&list, pt(10.0, 10.0));
        let a = f64::from(aa) / 255.0;
        let expected = 255.0 * (1.0 - a);
        assert!(
            (px[0] - expected).abs() < 1.5 && (px[2] - 255.0).abs() < 1.5,
            "AA {aa}: {px:?} vs {expected}"
        );
    }
}
