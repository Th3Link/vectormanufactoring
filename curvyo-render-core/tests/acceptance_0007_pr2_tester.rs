//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 2 (render
//! side): draw order (26), stroke on/off (5), colour and alpha (6), dash
//! (7 to 9), join and miter limit (10, 11), cap (12), fill (13 to 15), the
//! single-coverage layers and the robustness guards. Written from
//! `specification.md` before the PR 2 implementation was read.
//!
//! The pictures are judged on a CPU compositor built here from the
//! [`DrawList`] contract alone: a layer paints each pixel at most once (the
//! colour of any triangle of the layer that covers it), layers composite in
//! order with the "over" operator. Expected values come from the spec, never
//! from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss, clippy::similar_names, clippy::doc_markdown)]
#![allow(clippy::needless_range_loop, clippy::type_complexity)]
#![allow(
    missing_docs,
    clippy::assert_is_empty,
    clippy::semicolon_if_nothing_returned
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, DashPattern, Document, EllipseFrame, Length, LineCap, LineJoin,
    NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit, Vec2, ViewTransform,
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

// ------------------------------------------------------- AC 26: draw order

#[test]
fn ac26_a_path_above_a_filled_rectangle_draws_over_it_and_below_it_when_listed_first() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 40.0, 40.0);
    fill_solid(&doc, r, rgb(255, 0, 0), 1.0);
    let p = polyline(&doc, &[(5.0, 20.0), (35.0, 20.0)], false);
    style(
        &doc,
        p,
        &[
            StyleEdit::StrokeWidth(mm(6.0)),
            StyleEdit::StrokeColor(rgb(0, 0, 255)),
        ],
    );
    let on_path = pt(20.0, 20.0);
    // The path is above the rectangle in the tree: blue shows.
    let above = art(&[obj(&doc, r), obj(&doc, p)], 4.0);
    assert!(near(composite_at(&above, on_path), 0.0, 0.0, 255.0));
    // The rectangle is above the path: its fill covers the path.
    let below = art(&[obj(&doc, p), obj(&doc, r)], 4.0);
    assert!(near(composite_at(&below, on_path), 255.0, 0.0, 0.0));
}

#[test]
fn ac26_primitives_and_paths_interleave_in_the_order_given() {
    let doc = Document::new(1);
    let bottom = rect(&doc, 0.0, 0.0, 50.0, 50.0);
    fill_solid(&doc, bottom, rgb(255, 0, 0), 1.0);
    let middle = polyline(
        &doc,
        &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
        true,
    );
    fill_solid(&doc, middle, rgb(0, 255, 0), 1.0);
    let top = doc.create_ellipse(EllipseFrame {
        center: pt(25.0, 25.0),
        rx: mm(10.0),
        ry: mm(10.0),
    });
    fill_solid(&doc, top, rgb(0, 0, 255), 1.0);
    let list = art(&[obj(&doc, bottom), obj(&doc, middle), obj(&doc, top)], 4.0);
    assert!(near(composite_at(&list, pt(25.0, 25.0)), 0.0, 0.0, 255.0));
    assert!(near(composite_at(&list, pt(5.0, 5.0)), 0.0, 255.0, 0.0));
    let list = art(&[obj(&doc, top), obj(&doc, middle), obj(&doc, bottom)], 4.0);
    assert!(near(composite_at(&list, pt(25.0, 25.0)), 255.0, 0.0, 0.0));
    let list = art(&[obj(&doc, top), obj(&doc, bottom), obj(&doc, middle)], 4.0);
    assert!(near(composite_at(&list, pt(25.0, 25.0)), 0.0, 255.0, 0.0));
}

#[test]
fn ac26_an_objects_stroke_is_over_its_own_fill_and_a_higher_fill_covers_a_lower_stroke() {
    let doc = Document::new(1);
    let lower = rect(&doc, 0.0, 0.0, 30.0, 30.0);
    style(
        &doc,
        lower,
        &[
            StyleEdit::StrokeWidth(mm(6.0)),
            StyleEdit::StrokeColor(rgb(0, 0, 0)),
        ],
    );
    fill_solid(&doc, lower, rgb(255, 0, 0), 1.0);
    // Inside the stroke band, on the inner half: stroke over fill.
    let inner_half = pt(1.5, 15.0);
    let list = art(&[obj(&doc, lower)], 4.0);
    assert!(near(composite_at(&list, inner_half), 0.0, 0.0, 0.0));
    // A filled rectangle above that covers the lower one's right edge.
    let upper = rect(&doc, 20.0, -10.0, 40.0, 50.0);
    fill_solid(&doc, upper, rgb(0, 0, 255), 1.0);
    style(&doc, upper, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, lower), obj(&doc, upper)], 4.0);
    assert!(near(composite_at(&list, pt(29.0, 15.0)), 0.0, 0.0, 255.0));
    // A stroke-only object above does not cover what it merely encloses.
    let frame = rect(&doc, -5.0, -5.0, 40.0, 40.0);
    let list = art(&[obj(&doc, lower), obj(&doc, frame)], 4.0);
    assert!(near(composite_at(&list, pt(15.0, 15.0)), 255.0, 0.0, 0.0));
}

#[test]
fn ac26_the_draw_list_is_one_fill_layer_then_one_stroke_layer_per_object_in_order() {
    let doc = Document::new(1);
    let a = rect(&doc, 0.0, 0.0, 10.0, 10.0);
    fill_solid(&doc, a, rgb(255, 0, 0), 1.0);
    let b = rect(&doc, 20.0, 0.0, 10.0, 10.0);
    style(&doc, b, &[StyleEdit::StrokeColor(rgb(0, 255, 0))]);
    let c = rect(&doc, 40.0, 0.0, 10.0, 10.0);
    fill_solid(&doc, c, rgb(0, 0, 255), 0.5);
    let list = art(&[obj(&doc, a), obj(&doc, b), obj(&doc, c)], 4.0);
    assert_eq!(list.overlay_start(), list.triangles.len(), "all artwork");
    let ranges = layer_ranges(&list);
    // a: fill + stroke, b: stroke only (no fill), c: fill + stroke.
    assert_eq!(ranges.len(), 5, "layers: {ranges:?}");
    let colors: Vec<[u8; 4]> = ranges
        .iter()
        .map(|r| {
            let c = list.triangles[r.0].color;
            [c.r, c.g, c.b, c.a]
        })
        .collect();
    assert_eq!(colors[0], [255, 0, 0, 255], "a fill first");
    assert_eq!(colors[1], [0, 0, 0, 255], "then a's stroke");
    assert_eq!(colors[2], [0, 255, 0, 255], "b has only a stroke");
    assert_eq!(
        colors[3],
        [0, 0, 255, 128],
        "c fill at 50 percent (128/255)"
    );
    assert_eq!(colors[4], [0, 0, 0, 255]);
    // Every layer is one colour.
    for r in &ranges {
        let first = list.triangles[r.0].color;
        assert!(list.triangles[r.0..r.1].iter().all(|v| v.color == first));
    }
}

#[test]
fn layers_are_ordered_cover_the_artwork_and_get_strictly_nearer_depths() {
    let doc = Document::new(1);
    let mut snaps = Vec::new();
    for i in 0..5 {
        let r = rect(&doc, f64::from(i) * 12.0, 0.0, 10.0, 10.0);
        fill_solid(&doc, r, rgb(10 * i as u8, 0, 0), 0.5);
        snaps.push(obj(&doc, r));
    }
    let list = art(&snaps, 4.0);
    let layers = list.layers();
    assert_eq!(layers.len(), 10);
    assert!(layers.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(*layers.last().unwrap(), list.triangles.len());
    assert_eq!(list.triangles.len() % 3, 0);
    let depths = list.vertex_depths();
    assert_eq!(depths.len(), list.triangles.len());
    let mut last = f32::INFINITY;
    for (start, end) in layer_ranges(&list) {
        let d = depths[start];
        assert!(
            depths[start..end].iter().all(|x| *x == d),
            "one depth per layer"
        );
        assert!(d < last, "later layers are nearer");
        assert!((0.0..1.0).contains(&d) && d > 0.0, "inside (0, 1): {d}");
        last = d;
    }
}

// ---------------------------------------------------- AC 5: stroke on/off

#[test]
fn ac5_stroke_off_and_width_zero_draw_no_stroke_and_turning_it_back_on_restores_it() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
    style(
        &doc,
        r,
        &[
            StyleEdit::StrokeWidth(mm(3.0)),
            StyleEdit::StrokeColor(rgb(0, 0, 200)),
            dash(&[6.0, 4.0]),
            StyleEdit::StrokeJoin(LineJoin::Round),
            StyleEdit::StrokeCap(LineCap::Square),
        ],
    );
    let on = art(&[obj(&doc, r)], 4.0);
    assert!(covered(&on, pt(0.0, 10.0)));
    style(&doc, r, &[StyleEdit::StrokeEnabled(false)]);
    let off = art(&[obj(&doc, r)], 4.0);
    assert_eq!(off.triangles.len(), 0, "no fill, no stroke: nothing at all");
    assert!(off.layers().is_empty());
    style(&doc, r, &[StyleEdit::StrokeEnabled(true)]);
    let again = art(&[obj(&doc, r)], 4.0);
    assert_eq!(on, again, "every stored value comes back unchanged");
    // Width exactly 0 is "no stroke" too.
    style(&doc, r, &[StyleEdit::StrokeWidth(mm(0.0))]);
    let zero = art(&[obj(&doc, r)], 4.0);
    assert_eq!(zero.triangles.len(), 0);
}

#[test]
fn ac5_with_a_fill_the_object_still_paints_its_fill_only() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
    fill_solid(&doc, r, rgb(255, 0, 0), 1.0);
    style(&doc, r, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, r)], 4.0);
    assert_eq!(list.layers().len(), 1);
    assert!(near(composite_at(&list, pt(10.0, 10.0)), 255.0, 0.0, 0.0));
    // The fill reaches the geometric edge and not beyond: no stroke band.
    assert!(near(composite_at(&list, pt(0.5, 10.0)), 255.0, 0.0, 0.0));
    assert!(is_white(composite_at(&list, pt(-0.5, 10.0))));
}

// --------------------------------------------- AC 4: stroke width is exact

#[test]
fn ac4_a_stroke_renders_at_the_stored_width_for_path_and_primitive_alike() {
    let doc = Document::new(1);
    let line = polyline(&doc, &[(10.0, 20.0), (60.0, 20.0)], false);
    style(&doc, line, &[StyleEdit::StrokeWidth(mm(3.0))]);
    let list = art(&[obj(&doc, line)], 20.0);
    assert!(covered(&list, pt(30.0, 20.0 + 1.4)));
    assert!(covered(&list, pt(30.0, 20.0 - 1.4)));
    assert!(!covered(&list, pt(30.0, 20.0 + 1.6)));
    assert!(!covered(&list, pt(30.0, 20.0 - 1.6)));
    let doc = Document::new(1);
    let r = rect(&doc, 10.0, 10.0, 40.0, 40.0);
    style(&doc, r, &[StyleEdit::StrokeWidth(mm(3.0))]);
    let list = art(&[obj(&doc, r)], 20.0);
    for dx in [-1.4, 1.4] {
        assert!(covered(&list, pt(10.0 + dx, 30.0)));
    }
    for dx in [-1.6, 1.6] {
        assert!(!covered(&list, pt(10.0 + dx, 30.0)));
    }
}

#[test]
fn ac3_the_default_style_draws_a_quarter_millimetre_opaque_black_outline_and_no_fill() {
    let doc = Document::new(1);
    let r = rect(&doc, 10.0, 10.0, 40.0, 40.0);
    let list = art(&[obj(&doc, r)], 40.0);
    assert_eq!(list.layers().len(), 1, "stroke only");
    assert!(
        list.triangles
            .iter()
            .all(|v| (v.color.r, v.color.g, v.color.b, v.color.a) == (0, 0, 0, 255))
    );
    assert!(covered(&list, pt(10.1, 30.0)));
    assert!(covered(&list, pt(9.9, 30.0)));
    assert!(!covered(&list, pt(10.2, 30.0)));
    assert!(!covered(&list, pt(9.8, 30.0)));
    assert!(is_white(composite_at(&list, pt(30.0, 30.0))), "no fill");
}

// ------------------------------------------------ AC 6: colour and alpha

#[test]
fn ac6_stroke_colour_and_alpha_composite_as_n_over_100() {
    for n in [0_u32, 1, 25, 50, 99, 100] {
        let doc = Document::new(1);
        let line = polyline(&doc, &[(0.0, 0.0), (40.0, 0.0)], false);
        style(
            &doc,
            line,
            &[
                StyleEdit::StrokeWidth(mm(4.0)),
                StyleEdit::StrokeColor(rgb(200, 40, 10)),
                StyleEdit::StrokeOpacity(op(f64::from(n) / 100.0)),
            ],
        );
        let list = art(&[obj(&doc, line)], 4.0);
        let a = f64::from(n) / 100.0;
        let px = composite_at(&list, pt(20.0, 0.0));
        let want = [
            200.0 * a + 255.0 * (1.0 - a),
            40.0 * a + 255.0 * (1.0 - a),
            10.0 * a + 255.0 * (1.0 - a),
        ];
        for k in 0..3 {
            assert!(
                (px[k] - want[k]).abs() <= 1.5,
                "alpha {n}%: {px:?} vs {want:?}"
            );
        }
    }
}

#[test]
fn ac6_a_translucent_stroke_is_painted_once_per_pixel_over_joins_and_self_crossings() {
    // Bow tie: the stroke crosses itself in the middle; a thick stroke with
    // round joins overlaps itself at every vertex as well.
    let doc = Document::new(1);
    let bow = polyline(
        &doc,
        &[(0.0, 0.0), (40.0, 40.0), (40.0, 0.0), (0.0, 40.0)],
        true,
    );
    style(
        &doc,
        bow,
        &[
            StyleEdit::StrokeWidth(mm(8.0)),
            StyleEdit::StrokeOpacity(op(0.5)),
            StyleEdit::StrokeJoin(LineJoin::Round),
        ],
    );
    let star = polyline(
        &doc,
        &[
            (80.0, 0.0),
            (100.0, 40.0),
            (60.0, 15.0),
            (100.0, 15.0),
            (60.0, 40.0),
        ],
        true,
    );
    style(
        &doc,
        star,
        &[
            StyleEdit::StrokeWidth(mm(5.0)),
            StyleEdit::StrokeOpacity(op(0.5)),
        ],
    );
    let list = art(&[obj(&doc, bow), obj(&doc, star)], 2.0);
    let mut seen_covered = 0;
    let mut y = -6.0;
    while y < 46.0 {
        let mut x = -6.0;
        while x < 106.0 {
            let px = composite_at(&list, pt(x + 0.013, y + 0.017));
            if !is_white(px) {
                seen_covered += 1;
                assert!(
                    (px[0] - 127.5).abs() <= 1.5,
                    "pixel ({x}, {y}) blended {px:?}: coverage is not single"
                );
            }
            x += 0.5;
        }
        y += 0.5;
    }
    assert!(seen_covered > 500);
}

#[test]
fn ac6_a_translucent_stroke_over_a_translucent_fill_of_the_same_object() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 40.0, 40.0);
    style(
        &doc,
        r,
        &[
            StyleEdit::StrokeWidth(mm(8.0)),
            StyleEdit::StrokeColor(rgb(0, 0, 0)),
            StyleEdit::StrokeOpacity(op(0.5)),
        ],
    );
    fill_solid(&doc, r, rgb(255, 0, 0), 0.5);
    let list = art(&[obj(&doc, r)], 4.0);
    // Fill alone (centre): 50 percent red over white = (255, 127.5, 127.5).
    assert!(near(
        composite_at(&list, pt(20.0, 20.0)),
        255.0,
        127.5,
        127.5
    ));
    // Outer half of the stroke: black 50 percent over white.
    assert!(near(
        composite_at(&list, pt(-2.0, 20.0)),
        127.5,
        127.5,
        127.5
    ));
    // Inner half: the stroke over the fill: 0.5 * 0 + 0.5 * (255, 127.5, 127.5).
    assert!(near(
        composite_at(&list, pt(2.0, 20.0)),
        127.5,
        63.75,
        63.75
    ));
}

// ----------------------------------------------------------- AC 13 to 15

#[test]
fn ac13_fill_none_paints_nothing_and_solid_restores_the_stored_colour() {
    let doc = Document::new(1);
    let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
    fill_solid(&doc, r, rgb(0, 200, 0), 1.0);
    let filled = art(&[obj(&doc, r)], 4.0);
    assert!(near(composite_at(&filled, pt(10.0, 10.0)), 0.0, 200.0, 0.0));
    doc.edit_style(&[r], &StyleEdit::FillEnabled(false))
        .unwrap();
    let none = art(&[obj(&doc, r)], 4.0);
    assert!(is_white(composite_at(&none, pt(10.0, 10.0))));
    assert_eq!(none.layers().len(), 1, "stroke only");
    doc.edit_style(&[r], &StyleEdit::FillEnabled(true)).unwrap();
    assert_eq!(art(&[obj(&doc, r)], 4.0), filled);
}

#[test]
fn ac14_the_fill_uses_the_nonzero_rule_so_the_pentagram_centre_is_filled() {
    let doc = Document::new(1);
    let pts: Vec<(f64, f64)> = (0..5)
        .map(|i| {
            let a =
                f64::from(i) * 2.0 * 2.0 * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2;
            (50.0 + 40.0 * a.cos(), 50.0 + 40.0 * a.sin())
        })
        .collect();
    let star = polyline(&doc, &pts, true);
    fill_solid(&doc, star, rgb(255, 0, 0), 1.0);
    style(&doc, star, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, star)], 4.0);
    assert!(
        near(composite_at(&list, pt(50.0, 50.0)), 255.0, 0.0, 0.0),
        "winding 2 is filled"
    );
    assert!(
        near(composite_at(&list, pt(50.0, 20.0)), 255.0, 0.0, 0.0),
        "a tip is filled"
    );
    assert!(is_white(composite_at(&list, pt(5.0, 5.0))));
    assert!(is_white(composite_at(&list, pt(50.0, 95.0))));
}

#[test]
fn ac14_a_translucent_fill_composites_over_what_lies_below() {
    let doc = Document::new(1);
    let below = rect(&doc, 0.0, 0.0, 40.0, 40.0);
    fill_solid(&doc, below, rgb(0, 0, 255), 1.0);
    let above = rect(&doc, 20.0, 20.0, 40.0, 40.0);
    fill_solid(&doc, above, rgb(255, 0, 0), 0.25);
    style(&doc, above, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, below), obj(&doc, above)], 4.0);
    assert!(near(
        composite_at(&list, pt(30.0, 30.0)),
        63.75,
        0.0,
        191.25
    ));
    assert!(near(
        composite_at(&list, pt(50.0, 50.0)),
        255.0,
        191.25,
        191.25
    ));
    // Opacity zero paints nothing visible and does not disturb the picture.
    fill_solid(&doc, above, rgb(255, 0, 0), 0.0);
    let list = art(&[obj(&doc, below), obj(&doc, above)], 4.0);
    assert!(near(composite_at(&list, pt(30.0, 30.0)), 0.0, 0.0, 255.0));
    assert!(is_white(composite_at(&list, pt(50.0, 50.0))));
}

#[test]
fn ac15_an_open_path_fills_up_to_the_chord_and_its_stroke_stops_at_the_real_ends() {
    let doc = Document::new(1);
    let p = polyline(&doc, &[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0)], false);
    fill_solid(&doc, p, rgb(255, 0, 0), 1.0);
    style(
        &doc,
        p,
        &[
            StyleEdit::StrokeWidth(mm(2.0)),
            StyleEdit::StrokeColor(rgb(0, 0, 0)),
        ],
    );
    let list = art(&[obj(&doc, p)], 4.0);
    // Triangle (0,0) (40,0) (40,40): below the chord y = x is outside.
    assert!(near(composite_at(&list, pt(30.0, 10.0)), 255.0, 0.0, 0.0));
    assert!(is_white(composite_at(&list, pt(10.0, 30.0))));
    // The chord y = x is not stroked: the middle of it is fill red, not black.
    assert!(near(composite_at(&list, pt(20.0, 20.0)), 255.0, 0.0, 0.0));
    // The real edges are stroked.
    assert!(near(composite_at(&list, pt(20.0, 0.0)), 0.0, 0.0, 0.0));
    assert!(near(composite_at(&list, pt(40.0, 20.0)), 0.0, 0.0, 0.0));
    // And the butt ends are at the end points: nothing beyond the start.
    assert!(is_white(composite_at(&list, pt(-0.5, 0.0))));
}

#[test]
fn ac15_an_open_path_filled_with_a_curve_is_closed_by_a_straight_chord() {
    let doc = Document::new(1);
    let a = NewAnchor {
        id: AnchorId::new(1, 1),
        point: pt(0.0, 0.0),
        handle_in: Vec2::new(0.0, 0.0),
        handle_out: Vec2::new(0.0, 30.0),
        kind: AnchorKind::Corner,
    };
    let b = NewAnchor {
        id: AnchorId::new(1, 2),
        point: pt(40.0, 0.0),
        handle_in: Vec2::new(0.0, 30.0),
        handle_out: Vec2::new(0.0, 0.0),
        kind: AnchorKind::Corner,
    };
    let p = doc.create_path(&[a, b], false);
    fill_solid(&doc, p, rgb(255, 0, 0), 1.0);
    style(&doc, p, &[StyleEdit::StrokeEnabled(false)]);
    let list = art(&[obj(&doc, p)], 4.0);
    // Inside the bulge, below the arc and above the chord y = 0.
    assert!(near(composite_at(&list, pt(20.0, 10.0)), 255.0, 0.0, 0.0));
    // Above the arc top (about y = 22.5) and below the chord: outside.
    assert!(is_white(composite_at(&list, pt(20.0, 30.0))));
    assert!(is_white(composite_at(&list, pt(20.0, -3.0))));
}

// --------------------------------------------------- AC 7 to 9: dashes

/// Which of the sample abscissae `xs` are inked along the line y = 0.
fn inked(list: &DrawList, xs: &[f64]) -> Vec<bool> {
    xs.iter().map(|x| covered(list, pt(*x, 0.0))).collect()
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

#[test]
fn ac8_the_three_presets_draw_their_on_off_runs_as_multiples_of_the_width() {
    // (preset, width) -> the run lengths in mm, on first.
    for (preset, width) in [
        (vec![6.0, 4.0], 1.0),
        (vec![6.0, 4.0], 2.0),
        (vec![1.0, 3.0], 1.5),
        (vec![6.0, 3.0, 1.0, 3.0], 1.0),
        (vec![6.0, 3.0, 1.0, 3.0], 0.5),
    ] {
        let runs: Vec<f64> = preset.iter().map(|m| m * width).collect();
        let period: f64 = runs.iter().sum();
        let line = dashed_line(period * 4.0 + 1.0, width, &preset, LineCap::Butt);
        let list = art(&[line], 8.0);
        let mut at = 0.0;
        for cycle in 0..4 {
            let mut x = f64::from(cycle) * period;
            at = x;
            for (i, run) in runs.iter().enumerate() {
                let mid = x + run / 2.0;
                let on = i % 2 == 0;
                assert_eq!(
                    covered(&list, pt(mid, 0.0)),
                    on,
                    "preset {preset:?} width {width}: run {i} of cycle {cycle} at x = {mid}"
                );
                // 0.15 mm inside each edge of a run is that run's state.
                if *run > 0.5 {
                    assert_eq!(covered(&list, pt(x + 0.15, 0.0)), on, "start edge");
                    assert_eq!(covered(&list, pt(x + run - 0.15, 0.0)), on, "end edge");
                }
                x += run;
            }
        }
        let _ = at;
    }
}

#[test]
fn ac8_changing_the_width_rescales_the_same_stored_pattern() {
    let doc = Document::new(1);
    let p = polyline(&doc, &[(0.0, 0.0), (100.0, 0.0)], false);
    style(
        &doc,
        p,
        &[StyleEdit::StrokeWidth(mm(1.0)), dash(&[6.0, 4.0])],
    );
    let narrow = art(&[obj(&doc, p)], 8.0);
    assert_eq!(inked(&narrow, &[3.0, 8.0, 13.0]), [true, false, true]);
    style(&doc, p, &[StyleEdit::StrokeWidth(mm(2.0))]);
    let wide = art(&[obj(&doc, p)], 8.0);
    // on 12, off 8: 3 on, 8 on, 16 off, 25 on.
    assert_eq!(
        inked(&wide, &[3.0, 8.0, 16.0, 25.0, 36.0]),
        [true, true, false, true, false]
    );
}

#[test]
fn ac7_solid_is_one_unbroken_line() {
    let line = dashed_line(100.0, 1.0, &[], LineCap::Butt);
    let list = art(&[line], 8.0);
    let mut x = 0.05;
    while x < 99.9 {
        assert!(covered(&list, pt(x, 0.0)), "gap at {x}");
        x += 0.37;
    }
}

#[test]
fn ac12_each_dash_has_its_own_cap() {
    // Dash 1 mm wide: on [0, 6], off [6, 10].
    let probe = |cap, p: Point| {
        let list = art(&[dashed_line(100.0, 1.0, &[6.0, 4.0], cap)], 20.0);
        covered(&list, p)
    };
    // Butt: nothing beyond the dash end.
    assert!(!probe(LineCap::Butt, pt(6.1, 0.0)));
    // Round: a half disc of radius 0.5 beyond the end.
    assert!(probe(LineCap::Round, pt(6.3, 0.0)));
    assert!(!probe(LineCap::Round, pt(6.55, 0.0)));
    assert!(
        !probe(LineCap::Round, pt(6.4, 0.4)),
        "outside the arc, inside the square"
    );
    // Square: the full half-width square beyond the end.
    assert!(probe(LineCap::Square, pt(6.4, 0.4)));
    assert!(probe(LineCap::Square, pt(6.4, -0.4)));
    assert!(!probe(LineCap::Square, pt(6.6, 0.0)));
    // Caps on the start of the first dash too.
    assert!(probe(LineCap::Square, pt(-0.4, 0.0)));
    assert!(!probe(LineCap::Butt, pt(-0.1, 0.0)));
    // The gap shrinks: a round dash end and the next start are 4 - 1 = 3 apart.
    assert!(!probe(LineCap::Round, pt(8.0, 0.0)));
}

#[test]
fn ac8_a_period_under_two_screen_pixels_draws_solid_and_from_two_pixels_it_dashes() {
    // Width 10 mm, pattern [0.1, 0.1] -> on 1 mm, off 1 mm, period 2 mm.
    let line = dashed_line(200.0, 10.0, &[0.1, 0.1], LineCap::Butt);
    // 0.95 px/mm: period 1.9 px. Sample the middle of an "off" run (1.5 mm).
    let solid = art(std::slice::from_ref(&line), 0.95);
    assert!(covered(&solid, pt(1.5, 0.0)), "period 1.9 px draws solid");
    assert!(covered(&solid, pt(61.5, 0.0)));
    let dashed = art(std::slice::from_ref(&line), 1.05);
    assert!(
        !covered(&dashed, pt(1.5, 0.0)),
        "period 2.1 px keeps its gaps"
    );
    assert!(covered(&dashed, pt(0.5, 0.0)));
}

#[test]
fn ac8_more_than_two_thousand_dashes_for_one_object_draw_solid() {
    // Period 2 mm. Partial last dash included, 3999 mm -> 2000 dashes.
    let ok = dashed_line(3999.0, 1.0, &[1.0, 1.0], LineCap::Butt);
    let list = art(&[ok], 5.0);
    assert!(!covered(&list, pt(1.5, 0.0)), "2000 dashes stay dashed");
    assert!(!covered(&list, pt(3995.5, 0.0)));
    let over = dashed_line(4001.0, 1.0, &[1.0, 1.0], LineCap::Butt);
    let list = art(&[over], 5.0);
    assert!(covered(&list, pt(1.5, 0.0)), "2001 dashes draw solid");
    assert!(covered(&list, pt(3995.5, 0.0)));
    // The dash cap is per object: a short second object next to it still dashes.
    let doc = Document::new(1);
    let long = polyline(&doc, &[(0.0, 0.0), (10000.0, 0.0)], false);
    style(
        &doc,
        long,
        &[StyleEdit::StrokeWidth(mm(1.0)), dash(&[1.0, 1.0])],
    );
    let short = polyline(&doc, &[(0.0, 20.0), (10.0, 20.0)], false);
    style(
        &doc,
        short,
        &[StyleEdit::StrokeWidth(mm(1.0)), dash(&[1.0, 1.0])],
    );
    let list = art(&[obj(&doc, long), obj(&doc, short)], 5.0);
    assert!(covered(&list, pt(1.5, 0.0)));
    assert!(!covered(&list, pt(1.5, 20.0)));
}

#[test]
fn ac8_a_frame_budget_of_fifty_thousand_dashes_turns_the_excess_solid() {
    // Each line: 1900 dashes (period 2 mm, length 3800 mm).
    let doc = Document::new(1);
    let mut snaps = Vec::new();
    for i in 0..40 {
        let y = f64::from(i) * 5.0;
        let p = polyline(&doc, &[(0.0, y), (3800.0, y)], false);
        style(
            &doc,
            p,
            &[StyleEdit::StrokeWidth(mm(1.0)), dash(&[1.0, 1.0])],
        );
        snaps.push(obj(&doc, p));
    }
    let list = art(&snaps, 5.0);
    let dashed: usize = (0..40)
        .filter(|i| !covered(&list, pt(1.5, f64::from(*i as u32) * 5.0)))
        .count();
    assert!(
        dashed >= 1,
        "the budget is spent in order, the first stay dashed"
    );
    assert!(
        dashed * 1900 <= 50_000,
        "{dashed} dashed objects would be {} dashes",
        dashed * 1900
    );
    assert!(dashed < 40, "the rest draw solid");
}

// -------------------------------------------------- AC 10, 11: join

fn corner_path(join: LineJoin) -> ObjectSnapshot {
    let doc = Document::new(1);
    let p = polyline(&doc, &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)], false);
    style(
        &doc,
        p,
        &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeJoin(join)],
    );
    obj(&doc, p)
}

#[test]
fn ac10_miter_round_and_bevel_corners_have_their_svg_shapes() {
    // Outer corner of the right angle at (50, 0) faces +x, -y; half width 2.
    let tip = pt(51.9, -1.9); // the miter point is (52, -2)
    let mid_arc = pt(51.3, -1.3); // 1.84 from the vertex: inside the round join
    let beyond_bevel = pt(51.5, -1.5); // 2.12 from the vertex: outside round
    let m = art(&[corner_path(LineJoin::Miter)], 20.0);
    let r = art(&[corner_path(LineJoin::Round)], 20.0);
    let b = art(&[corner_path(LineJoin::Bevel)], 20.0);
    assert!(covered(&m, tip) && covered(&m, mid_arc) && covered(&m, beyond_bevel));
    assert!(!covered(&r, tip) && covered(&r, mid_arc) && !covered(&r, beyond_bevel));
    assert!(!covered(&b, tip) && !covered(&b, mid_arc) && !covered(&b, beyond_bevel));
    // Inside the bevel line (the chord between the stroke edges) all three agree.
    for list in [&m, &r, &b] {
        assert!(covered(list, pt(50.8, -0.8)), "inside the bevel chord");
        assert!(covered(list, pt(51.9, 0.5)), "plain stroke edge side");
    }
    // A round join reaches exactly half the stroke width from the vertex.
    assert!(covered(&r, pt(50.0 + 1.9 * 0.707, -1.9 * 0.707)));
    assert!(!covered(&r, pt(50.0 + 2.1 * 0.707, -2.1 * 0.707)));
}

fn apex_path(interior_degrees: f64, join: LineJoin) -> ObjectSnapshot {
    let doc = Document::new(1);
    let half = interior_degrees.to_radians() / 2.0;
    let (c, s) = (half.cos() * 100.0, half.sin() * 100.0);
    let p = polyline(&doc, &[(c, -s), (0.0, 0.0), (c, s)], false);
    style(
        &doc,
        p,
        &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeJoin(join)],
    );
    obj(&doc, p)
}

#[test]
fn ac11_a_sharp_miter_falls_back_to_a_bevel_and_a_blunt_one_keeps_its_point() {
    // 40 degrees: miter length / width = 1 / sin(20 deg) = 2.9, under 4: spike.
    // The tip is at -w / (2 sin(20 deg)) = -5.85 from the vertex.
    let blunt = art(&[apex_path(40.0, LineJoin::Miter)], 20.0);
    assert!(
        covered(&blunt, pt(-5.6, 0.0)),
        "40 degrees keeps its miter point"
    );
    // 8 degrees: ratio 14: bevel (both readings of "4" agree here).
    let sharp = art(&[apex_path(8.0, LineJoin::Miter)], 20.0);
    let bevel = art(&[apex_path(8.0, LineJoin::Bevel)], 20.0);
    assert!(!covered(&sharp, pt(-3.0, 0.0)), "no spike at 8 degrees");
    assert!(!covered(&sharp, pt(-2.5, 0.0)));
    // And a fallen-back miter is a bevel: same coverage as an explicit bevel.
    for (x, y) in [
        (-1.0, 0.0),
        (-1.9, 0.0),
        (-2.2, 0.0),
        (-1.5, 0.3),
        (-0.5, 1.0),
    ] {
        assert_eq!(
            covered(&sharp, pt(x, y)),
            covered(&bevel, pt(x, y)),
            "at ({x}, {y})"
        );
    }
}

#[test]
fn ac10_rounded_corners_ignore_the_join_and_a_sharp_corner_does_not() {
    let make = |join: LineJoin, radius: Option<f64>| {
        let doc = Document::new(1);
        let r = rect(&doc, 0.0, 0.0, 40.0, 30.0);
        if let Some(radius) = radius {
            doc.set_corner_radius(&[r], mm(radius)).unwrap();
        }
        style(
            &doc,
            r,
            &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeJoin(join)],
        );
        art(&[obj(&doc, r)], 8.0)
    };
    let m = make(LineJoin::Miter, Some(8.0));
    assert_same_coverage(
        &m,
        &make(LineJoin::Round, Some(8.0)),
        (-4.0, 44.0, -4.0, 34.0),
    );
    assert_same_coverage(
        &m,
        &make(LineJoin::Bevel, Some(8.0)),
        (-4.0, 44.0, -4.0, 34.0),
    );
    // At zero radius the corner shows the join.
    let sharp_m = make(LineJoin::Miter, None);
    let sharp_b = make(LineJoin::Bevel, None);
    assert!(covered(&sharp_m, pt(-1.9, -1.9)));
    assert!(!covered(&sharp_b, pt(-1.9, -1.9)));
    // An ellipse has no vertex either.
    let ellipse = |join: LineJoin| {
        let doc = Document::new(1);
        let e = doc.create_ellipse(EllipseFrame {
            center: pt(20.0, 15.0),
            rx: mm(20.0),
            ry: mm(10.0),
        });
        style(
            &doc,
            e,
            &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeJoin(join)],
        );
        art(&[obj(&doc, e)], 8.0)
    };
    let base = ellipse(LineJoin::Miter);
    assert_same_coverage(&base, &ellipse(LineJoin::Round), (-4.0, 44.0, -4.0, 34.0));
    assert_same_coverage(&base, &ellipse(LineJoin::Bevel), (-4.0, 44.0, -4.0, 34.0));
}

// ------------------------------------------------------------- AC 12: cap

#[test]
fn ac12_open_ends_follow_butt_round_and_square() {
    let ends = |cap: LineCap| {
        let doc = Document::new(1);
        let p = polyline(&doc, &[(0.0, 0.0), (50.0, 0.0)], false);
        style(
            &doc,
            p,
            &[StyleEdit::StrokeWidth(mm(2.0)), StyleEdit::StrokeCap(cap)],
        );
        art(&[obj(&doc, p)], 20.0)
    };
    let butt = ends(LineCap::Butt);
    let round = ends(LineCap::Round);
    let square = ends(LineCap::Square);
    for (side, sign) in [(0.0, -1.0), (50.0, 1.0)] {
        let at = |dx: f64, dy: f64| pt(side + sign * dx, dy);
        assert!(!covered(&butt, at(0.1, 0.0)));
        assert!(covered(&round, at(0.9, 0.0)) && !covered(&round, at(1.1, 0.0)));
        assert!(
            !covered(&round, at(0.85, 0.85)),
            "square corner, not in the arc"
        );
        assert!(covered(&square, at(0.9, 0.9)) && !covered(&square, at(1.1, 0.0)));
    }
}

#[test]
fn ac12_a_closed_path_has_no_ends_so_the_cap_changes_nothing() {
    let make = |cap: LineCap| {
        let doc = Document::new(1);
        let p = polyline(
            &doc,
            &[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)],
            true,
        );
        style(
            &doc,
            p,
            &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeCap(cap)],
        );
        let r = rect(&doc, 50.0, 0.0, 30.0, 30.0);
        style(
            &doc,
            r,
            &[StyleEdit::StrokeWidth(mm(4.0)), StyleEdit::StrokeCap(cap)],
        );
        art(&[obj(&doc, p), obj(&doc, r)], 8.0)
    };
    let butt = make(LineCap::Butt);
    // Same coverage on a sample grid (tessellation may differ in layout).
    for cap in [LineCap::Round, LineCap::Square] {
        let other = make(cap);
        let mut y = -5.0;
        while y < 35.0 {
            let mut x = -5.0;
            while x < 85.0 {
                let p = pt(x + 0.031, y + 0.017);
                assert_eq!(covered(&butt, p), covered(&other, p), "{cap:?} at {p:?}");
                x += 0.4;
            }
            y += 0.4;
        }
    }
}

// ------------------------------------------- AC 1: one property set

#[test]
fn ac1_a_rectangle_and_the_same_closed_path_render_identically_in_every_style() {
    type Setup = fn(&Document, NodeId);
    let setups: Vec<Setup> = vec![
        |_, _| {},
        |d, id| {
            style(
                d,
                id,
                &[
                    StyleEdit::StrokeWidth(mm(5.0)),
                    StyleEdit::StrokeJoin(LineJoin::Round),
                ],
            )
        },
        |d, id| {
            style(
                d,
                id,
                &[
                    StyleEdit::StrokeWidth(mm(5.0)),
                    StyleEdit::StrokeJoin(LineJoin::Bevel),
                    StyleEdit::StrokeOpacity(op(0.4)),
                ],
            )
        },
        |d, id| {
            style(d, id, &[StyleEdit::StrokeEnabled(false)]);
            fill_solid(d, id, rgb(10, 200, 90), 0.6);
        },
        |d, id| {
            style(
                d,
                id,
                &[
                    StyleEdit::StrokeWidth(mm(2.0)),
                    StyleEdit::StrokeColor(rgb(0, 0, 255)),
                ],
            );
            fill_solid(d, id, rgb(255, 0, 0), 1.0);
        },
    ];
    for (i, setup) in setups.iter().enumerate() {
        let doc = Document::new(1);
        let r = rect(&doc, 5.0, 5.0, 40.0, 25.0);
        let p = polyline(
            &doc,
            &[(5.0, 5.0), (45.0, 5.0), (45.0, 30.0), (5.0, 30.0)],
            true,
        );
        setup(&doc, r);
        setup(&doc, p);
        let lr = art(&[obj(&doc, r)], 4.0);
        let lp = art(&[obj(&doc, p)], 4.0);
        let mut y = -2.0;
        while y < 38.0 {
            let mut x = -2.0;
            while x < 52.0 {
                let q = pt(x + 0.0123, y + 0.0187);
                let (a, b) = (composite_at(&lr, q), composite_at(&lp, q));
                for k in 0..3 {
                    assert!(
                        (a[k] - b[k]).abs() < 1.0,
                        "setup {i}: {q:?}: {a:?} vs {b:?}"
                    );
                }
                x += 0.5;
            }
            y += 0.5;
        }
    }
}

// ------------------------------- robustness: degenerate and hostile input

#[test]
fn degenerate_objects_never_panic_and_never_emit_non_finite_vertices() {
    let doc = Document::new(1);
    let mut snaps = vec![
        polyline(&doc, &[(5.0, 5.0)], false),
        polyline(&doc, &[(15.0, 5.0)], true),
    ];
    // Coincident anchors.
    snaps.push(polyline(
        &doc,
        &[(25.0, 5.0), (25.0, 5.0), (25.0, 5.0)],
        true,
    ));
    snaps.push(polyline(&doc, &[(30.0, 5.0), (30.0, 5.0)], false));
    // Two anchors closed, collinear closed.
    snaps.push(polyline(&doc, &[(0.0, 20.0), (10.0, 20.0)], true));
    snaps.push(polyline(
        &doc,
        &[(0.0, 30.0), (5.0, 30.0), (10.0, 30.0)],
        true,
    ));
    // Zero-size and tiny primitives.
    snaps.push(rect(&doc, 40.0, 40.0, 0.0, 0.0));
    snaps.push(rect(&doc, 50.0, 40.0, 10.0, 0.0));
    snaps.push(rect(&doc, 60.0, 40.0, 1e-9, 1e-9));
    snaps.push(doc.create_ellipse(EllipseFrame {
        center: pt(70.0, 40.0),
        rx: mm(0.0),
        ry: mm(0.0),
    }));
    for (i, id) in snaps.clone().into_iter().enumerate() {
        fill_solid(&doc, id, rgb(255, 0, 0), 0.5);
        style(
            &doc,
            id,
            &[
                StyleEdit::StrokeWidth(mm(3.0)),
                StyleEdit::StrokeCap(if i % 2 == 0 {
                    LineCap::Round
                } else {
                    LineCap::Square
                }),
                StyleEdit::StrokeJoin(LineJoin::Miter),
                dash(&[6.0, 4.0]),
            ],
        );
    }
    let objects: Vec<ObjectSnapshot> = snaps.iter().map(|id| obj(&doc, *id)).collect();
    for scale in [1e-3, 0.02 * 96.0 / 25.4, 1.0, 100.0, 1e4] {
        let list = art(&objects, scale);
        assert!(all_finite(&list), "scale {scale}");
        assert_eq!(list.triangles.len() % 3, 0);
        assert_eq!(list.overlay_start(), list.triangles.len());
    }
    // Every object alone, too.
    for o in &objects {
        let list = art(std::slice::from_ref(o), 4.0);
        assert!(all_finite(&list));
    }
}

#[test]
fn an_empty_object_list_is_an_empty_draw_list() {
    let list = art(&[], 4.0);
    assert!(list.triangles.is_empty());
    assert!(list.layers().is_empty());
    assert_eq!(list.overlay_start(), 0);
    assert!(list.vertex_depths().is_empty());
}

#[test]
fn a_huge_or_tiny_stored_width_is_displayed_bounded_and_the_stored_width_is_kept() {
    for w in [
        1e-300,
        1e-12,
        1e6,
        1e12,
        1e30,
        1e38,
        1e100,
        1e300,
        f64::MAX / 4.0,
    ] {
        let doc = Document::new(1);
        let r = rect(&doc, 0.0, 0.0, 20.0, 20.0);
        let p = polyline(&doc, &[(0.0, 50.0), (30.0, 50.0), (30.0, 80.0)], false);
        for id in [r, p] {
            style(&doc, id, &[StyleEdit::StrokeWidth(mm(w))]);
            let list = art(&[obj(&doc, id)], 4.0);
            assert!(all_finite(&list), "width {w}");
            assert!(
                list.triangles
                    .iter()
                    .all(|v| v.position.x.abs() < 1e9 && v.position.y.abs() < 1e9),
                "width {w}: geometry must stay bounded for display"
            );
            let stored = match obj(&doc, id) {
                ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
                ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
            };
            assert_eq!(stored, w, "display clamps never rewrite the document");
            // With dashes and caps on top.
            style(
                &doc,
                id,
                &[dash(&[6.0, 4.0]), StyleEdit::StrokeCap(LineCap::Round)],
            );
            let list = art(&[obj(&doc, id)], 4.0);
            assert!(all_finite(&list), "dashed width {w}");
        }
    }
}

#[test]
fn hostile_positions_do_not_poison_the_rest_of_the_frame() {
    let doc = Document::new(1);
    let good = rect(&doc, 0.0, 0.0, 10.0, 10.0);
    fill_solid(&doc, good, rgb(255, 0, 0), 1.0);
    let far = polyline(&doc, &[(0.0, 0.0), (1e9, 1e9), (-1e9, 3.0)], true);
    fill_solid(&doc, far, rgb(0, 255, 0), 1.0);
    style(&doc, far, &[dash(&[6.0, 4.0])]);
    let list = art(&[obj(&doc, far), obj(&doc, good)], 4.0);
    assert!(all_finite(&list));
    assert!(near(composite_at(&list, pt(5.0, 5.0)), 255.0, 0.0, 0.0));
}

/// A deterministic pseudo-random walk over styles and shapes: whatever the
/// style, the draw list keeps its contract.
#[test]
fn random_styled_frames_keep_the_draw_list_contract() {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for round in 0..60 {
        let doc = Document::new(1);
        let mut snaps = Vec::new();
        let count = 1 + (next() * 6.0) as usize;
        for _ in 0..count {
            let n = 2 + (next() * 5.0) as usize;
            let pts: Vec<(f64, f64)> = (0..n).map(|_| (next() * 80.0, next() * 80.0)).collect();
            let id = if next() < 0.3 {
                rect(
                    &doc,
                    next() * 50.0,
                    next() * 50.0,
                    next() * 40.0,
                    next() * 40.0,
                )
            } else {
                polyline(&doc, &pts, next() < 0.5)
            };
            let joins = [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel];
            let caps = [LineCap::Butt, LineCap::Round, LineCap::Square];
            let patterns: [&[f64]; 5] = [
                &[],
                &[6.0, 4.0],
                &[1.0, 3.0],
                &[6.0, 3.0, 1.0, 3.0],
                &[0.01, 0.01],
            ];
            style(
                &doc,
                id,
                &[
                    StyleEdit::StrokeWidth(mm(next() * 12.0)),
                    StyleEdit::StrokeColor(rgb((next() * 255.0) as u8, 0, 0)),
                    StyleEdit::StrokeOpacity(op(next())),
                    StyleEdit::StrokeJoin(joins[(next() * 3.0) as usize % 3]),
                    StyleEdit::StrokeCap(caps[(next() * 3.0) as usize % 3]),
                    dash(patterns[(next() * 5.0) as usize % 5]),
                ],
            );
            if next() < 0.6 {
                fill_solid(&doc, id, rgb(0, (next() * 255.0) as u8, 0), next());
            }
            snaps.push(obj(&doc, id));
        }
        let scale = 0.05 + next() * 30.0;
        let list = art(&snaps, scale);
        assert!(all_finite(&list), "round {round}");
        assert_eq!(list.triangles.len() % 3, 0);
        assert_eq!(list.overlay_start(), list.triangles.len(), "round {round}");
        assert!(
            list.layers().windows(2).all(|w| w[0] < w[1]),
            "round {round}"
        );
        assert_eq!(list.vertex_depths().len(), list.triangles.len());
        // Layer uniformity: one paint per layer.
        for (s, e) in layer_ranges(&list) {
            let c = list.triangles[s].color;
            assert!(
                list.triangles[s..e].iter().all(|v| v.color == c),
                "round {round}"
            );
        }
        // Idempotence: the same snapshots draw the same list.
        assert_eq!(list, art(&snaps, scale), "round {round}: deterministic");
    }
}
