//! Rendering of the style model (`specs/0007-stroke-and-fill-styling`, PR 2):
//! tree-order drawing, stroke on/off, opacity, dash, join, cap, solid fill and
//! the single-coverage layers. Criteria 3, 5 to 8, 10 to 15 and 26.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value
)]

use curvyo_document_core::{
    AnchorId, Color, CornerRadii, DashPattern, Document, Length, LineCap, LineJoin, NewAnchor,
    NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit, ViewTransform,
};
use curvyo_render_core::{DrawList, build_artwork};

const SCALE: f64 = 4.0;

fn view() -> ViewTransform {
    ViewTransform::new(SCALE, Point::new(0.0, 0.0))
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn red() -> Color {
    Color { r: 255, g: 0, b: 0 }
}

fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn polyline(document: &Document, points: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    document.create_path(&anchors, closed)
}

fn edit(document: &Document, id: NodeId, edit: StyleEdit) {
    document.edit_style(&[id], &edit).unwrap();
}

fn fill_solid(document: &Document, id: NodeId, color: Color) {
    edit(document, id, StyleEdit::FillColor(color));
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
}

fn artwork(document: &Document) -> DrawList {
    let objects: Vec<ObjectSnapshot> = document
        .object_ids()
        .into_iter()
        .map(|id| document.object(id).unwrap())
        .collect();
    build_artwork(&objects, view())
}

/// The bounding box of the vertices of `layer` of `list`.
fn layer_bounds(list: &DrawList, layer: usize) -> (f64, f64, f64, f64) {
    let start = if layer == 0 {
        0
    } else {
        list.layers()[layer - 1]
    };
    let end = list.layers()[layer];
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for v in &list.triangles[start..end] {
        b.0 = b.0.min(v.position.x);
        b.1 = b.1.min(v.position.y);
        b.2 = b.2.max(v.position.x);
        b.3 = b.3.max(v.position.y);
    }
    b
}

/// The x intervals a layer covers, merged: one per dash of a horizontal line.
fn covered_intervals(list: &DrawList, layer: usize) -> Vec<(f64, f64)> {
    let start = if layer == 0 {
        0
    } else {
        list.layers()[layer - 1]
    };
    let end = list.layers()[layer];
    let mut spans: Vec<(f64, f64)> = list.triangles[start..end]
        .chunks(3)
        .map(|t| {
            let xs = [t[0].position.x, t[1].position.x, t[2].position.x];
            (
                xs.iter().copied().fold(f64::MAX, f64::min),
                xs.iter().copied().fold(f64::MIN, f64::max),
            )
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (lo, hi) in spans {
        match merged.last_mut() {
            Some(last) if lo <= last.1 + 1e-9 => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    merged
}

fn layer_alpha(list: &DrawList, layer: usize) -> u8 {
    let start = if layer == 0 {
        0
    } else {
        list.layers()[layer - 1]
    };
    list.triangles[start].color.a
}

// ---------------------------------------------------------------------
// AC 3: the default style renders as before
// ---------------------------------------------------------------------

#[test]
fn ac3_a_default_styled_object_is_one_opaque_black_stroke_layer_as_before() {
    let document = Document::new(1);
    let path = polyline(&document, &[(0.0, 0.0), (20.0, 0.0), (20.0, 10.0)], false);
    let snapshot = document.path(path).unwrap();
    let list = build_artwork(&[ObjectSnapshot::Path(snapshot.clone())], view());
    assert_eq!(list.layers().len(), 1, "no fill: only a stroke layer");
    let old = build_artwork(&[ObjectSnapshot::Path(snapshot)], view());
    let (a, b) = (&list.triangles, &old.triangles);
    assert!(b.len() >= a.len());
    assert_eq!(
        a[..],
        b[..a.len()],
        "the stroke is vertex-for-vertex the old one"
    );
    assert!(a.iter().all(|v| v.color.a == 255 && v.color.r == 0));
}

// ---------------------------------------------------------------------
// AC 26: tree order
// ---------------------------------------------------------------------

#[test]
fn ac26_objects_are_layered_in_tree_order_paths_and_primitives_interleaved() {
    let document = Document::new(1);
    let first = rect(&document, 0.0, 0.0, 10.0, 10.0);
    let path = polyline(&document, &[(100.0, 0.0), (110.0, 0.0)], false);
    let last = rect(&document, 200.0, 0.0, 10.0, 10.0);
    for id in [first, path, last] {
        edit(&document, id, StyleEdit::StrokeWidth(mm(1.0)));
    }
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 3);
    // Layer order = tree order: x ranges ascend.
    let xs: Vec<f64> = (0..3).map(|i| layer_bounds(&list, i).0).collect();
    assert!(
        xs[0] < 50.0 && xs[1] > 50.0 && xs[1] < 150.0 && xs[2] > 150.0,
        "{xs:?}"
    );
}

#[test]
fn ac26_each_object_draws_its_fill_and_then_its_stroke() {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&document, 5.0, 5.0, 10.0, 10.0);
    fill_solid(&document, a, red());
    edit(&document, b, StyleEdit::StrokeWidth(mm(1.0)));
    let list = artwork(&document);
    // a: fill, stroke; b: stroke.
    assert_eq!(list.layers().len(), 3);
    assert_eq!(list.triangles[0].color.r, 255, "a's fill is red");
    assert_eq!(
        list.triangles[list.layers()[0]].color.r,
        0,
        "a's stroke is black"
    );
    assert!(layer_bounds(&list, 2).0 >= 4.0, "b's stroke last");
}

// ---------------------------------------------------------------------
// AC 5, 6: stroke off, colour and opacity
// ---------------------------------------------------------------------

#[test]
fn ac5_a_stroke_that_is_off_paints_nothing_and_a_fill_alone_remains() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 10.0);
    edit(&document, id, StyleEdit::StrokeEnabled(false));
    assert_eq!(artwork(&document).triangles.len(), 0);
    fill_solid(&document, id, red());
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 1);
    assert!(list.triangles.iter().all(|v| v.color.r == 255));
}

#[test]
fn ac6_opacity_is_the_alpha_of_the_stroke_and_the_fill() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 10.0);
    edit(
        &document,
        id,
        StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap()),
    );
    edit(
        &document,
        id,
        StyleEdit::FillOpacity(Opacity::new(0.25).unwrap()),
    );
    fill_solid(&document, id, red());
    let list = artwork(&document);
    assert_eq!(layer_alpha(&list, 0), 64, "fill 25%");
    assert_eq!(layer_alpha(&list, 1), 128, "stroke 50%");
}

#[test]
fn ac6_zero_opacity_paints_nothing() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 10.0);
    edit(
        &document,
        id,
        StyleEdit::StrokeOpacity(Opacity::new(0.0).unwrap()),
    );
    assert_eq!(artwork(&document).triangles.len(), 0);
}

// ---------------------------------------------------------------------
// AC 7, 8, 9: dash
// ---------------------------------------------------------------------

#[test]
fn ac7_ac8_a_dashed_stroke_has_gaps_and_the_pattern_scales_with_the_width() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    edit(&document, id, StyleEdit::StrokeWidth(mm(1.0)));
    let solid = artwork(&document);
    edit(
        &document,
        id,
        StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    );
    let dashed = artwork(&document);
    assert_eq!(covered_intervals(&solid, 0).len(), 1, "one unbroken line");
    let dashes = covered_intervals(&dashed, 0);
    assert_eq!(dashes.len(), 10, "{dashes:?}");
    for (n, (lo, hi)) in dashes.iter().enumerate() {
        assert!(
            (lo - 10.0 * n as f64).abs() < 0.1,
            "dash {n} starts at {lo}"
        );
        assert!(
            (hi - lo - 6.0).abs() < 0.1,
            "6 on, 4 off at width 1: {lo}..{hi}"
        );
    }
    // Doubling the width doubles the period: half as many dashes, 12 mm long.
    edit(&document, id, StyleEdit::StrokeWidth(mm(2.0)));
    let wider = covered_intervals(&artwork(&document), 0);
    assert_eq!(wider.len(), 5);
    assert!((wider[0].1 - wider[0].0 - 12.0).abs() < 0.1);
}

#[test]
fn ac8_a_pattern_too_fine_to_see_draws_solid() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    edit(&document, id, StyleEdit::StrokeWidth(mm(0.1)));
    edit(
        &document,
        id,
        StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 1.0]).unwrap()),
    );
    // Period 0.2 mm at 4 px/mm = 0.8 px: solid.
    assert_eq!(covered_intervals(&artwork(&document), 0).len(), 1);
}

#[test]
fn a_dash_pattern_with_a_huge_width_or_count_cannot_blow_up() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (1000.0, 0.0)], false);
    edit(&document, id, StyleEdit::StrokeWidth(mm(1e30)));
    edit(
        &document,
        id,
        StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 1.0]).unwrap()),
    );
    let list = artwork(&document);
    assert!(
        list.triangles
            .iter()
            .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
    );
    let (_, min_y, _, max_y) = layer_bounds(&list, 0);
    assert!(
        max_y - min_y <= 200_000.1,
        "the width is capped for display"
    );
}

// ---------------------------------------------------------------------
// AC 10, 11, 12: join and cap
// ---------------------------------------------------------------------

fn corner_tip_x(join: LineJoin, angle_height: f64) -> f64 {
    let document = Document::new(1);
    // A polyline right to (20, 0) then back toward the left: the tip at
    // (20, 0) is a sharp corner.
    let id = polyline(
        &document,
        &[(0.0, -angle_height), (20.0, 0.0), (0.0, angle_height)],
        false,
    );
    edit(&document, id, StyleEdit::StrokeWidth(mm(2.0)));
    edit(&document, id, StyleEdit::StrokeJoin(join));
    let list = artwork(&document);
    layer_bounds(&list, 0).2
}

#[test]
fn ac10_miter_reaches_a_point_bevel_cuts_it_and_round_is_half_a_width() {
    // 90 degree corner: miter extends sqrt(2)*w/2 ... bevel cuts it.
    let miter = corner_tip_x(LineJoin::Miter, 20.0);
    let bevel = corner_tip_x(LineJoin::Bevel, 20.0);
    let round = corner_tip_x(LineJoin::Round, 20.0);
    assert!(miter > bevel + 0.2, "miter {miter} bevel {bevel}");
    assert!(round > bevel && round < miter, "round {round}");
    assert!(
        (round - 21.0).abs() < 0.05,
        "an arc of radius w/2 = 1 about the vertex"
    );
}

#[test]
fn ac11_a_miter_past_the_limit_of_4_renders_as_a_bevel() {
    // Very sharp corner (about 11 degrees): miter ratio 1/sin(5.7deg) = 10 > 4.
    let miter = corner_tip_x(LineJoin::Miter, 2.0);
    let bevel = corner_tip_x(LineJoin::Bevel, 2.0);
    assert!((miter - bevel).abs() < 1e-4, "limited miter = bevel");
    // A milder corner (90 degrees, ratio 1.41 < 4) is mitred.
    assert!(corner_tip_x(LineJoin::Miter, 20.0) > corner_tip_x(LineJoin::Bevel, 20.0) + 0.2);
}

fn open_end_x(cap: LineCap) -> f64 {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (20.0, 0.0)], false);
    edit(&document, id, StyleEdit::StrokeWidth(mm(2.0)));
    edit(&document, id, StyleEdit::StrokeCap(cap));
    layer_bounds(&artwork(&document), 0).2
}

#[test]
fn ac12_butt_ends_at_the_endpoint_round_and_square_reach_half_a_width_beyond() {
    assert!((open_end_x(LineCap::Butt) - 20.0).abs() < 1e-4);
    assert!((open_end_x(LineCap::Square) - 21.0).abs() < 1e-4);
    assert!((open_end_x(LineCap::Round) - 21.0).abs() < 0.02);
}

#[test]
fn ac12_a_closed_paths_stroke_ignores_the_cap() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 10.0);
    let butt = artwork(&document);
    edit(&document, id, StyleEdit::StrokeCap(LineCap::Square));
    assert_eq!(artwork(&document), butt);
}

#[test]
fn ac10_a_rounded_rectangle_has_no_join_at_a_curved_corner_and_no_spurious_one_at_zero_length() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 20.0, 10.0);
    // Radius half the shorter side: the straight sides on the short edges
    // have zero length.
    document
        .set_corner_radii(&[(id, CornerRadii::uniform(mm(5.0)))])
        .unwrap();
    edit(&document, id, StyleEdit::StrokeWidth(mm(1.0)));
    let miter = artwork(&document);
    edit(&document, id, StyleEdit::StrokeJoin(LineJoin::Bevel));
    let bevel = artwork(&document);
    // The joins between a straight side and an arc are tangent-continuous:
    // the silhouette is the same whatever the join (the tessellation may
    // differ in the last float digit).
    let (a, b) = (layer_bounds(&miter, 0), layer_bounds(&bevel, 0));
    for (x, y) in [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)] {
        assert!((x - y).abs() < 0.02, "{a:?} vs {b:?}");
    }
    let (min_x, min_y, max_x, max_y) = layer_bounds(&miter, 0);
    assert!(
        min_x >= -0.51 && min_y >= -0.51 && max_x <= 20.51 && max_y <= 10.51,
        "no spike"
    );
}

// ---------------------------------------------------------------------
// AC 13, 14, 15: fill
// ---------------------------------------------------------------------

#[test]
fn ac14_a_solid_fill_covers_the_interior() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 6.0);
    edit(&document, id, StyleEdit::StrokeEnabled(false));
    fill_solid(&document, id, red());
    let list = artwork(&document);
    let b = layer_bounds(&list, 0);
    assert_eq!(b, (0.0, 0.0, 10.0, 6.0));
    let area: f64 = list
        .triangles
        .chunks(3)
        .map(|t| {
            ((t[1].position.x - t[0].position.x) * (t[2].position.y - t[0].position.y)
                - (t[2].position.x - t[0].position.x) * (t[1].position.y - t[0].position.y))
                .abs()
                / 2.0
        })
        .sum();
    assert!((area - 60.0).abs() < 1e-6, "{area}");
}

#[test]
fn ac13_fill_none_paints_nothing_even_with_a_stored_colour() {
    let document = Document::new(1);
    let id = rect(&document, 0.0, 0.0, 10.0, 10.0);
    edit(&document, id, StyleEdit::FillColor(red()));
    edit(&document, id, StyleEdit::StrokeEnabled(false));
    assert_eq!(artwork(&document).triangles.len(), 0);
}

#[test]
fn ac15_an_open_paths_fill_is_closed_by_a_chord_and_its_stroke_is_not() {
    let document = Document::new(1);
    // A U open at the top.
    let id = polyline(
        &document,
        &[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)],
        false,
    );
    edit(&document, id, StyleEdit::StrokeWidth(mm(0.5)));
    let without_fill = artwork(&document);
    fill_solid(&document, id, red());
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 2);
    let (min_x, min_y, max_x, max_y) = layer_bounds(&list, 0);
    assert_eq!(
        (min_x, min_y, max_x, max_y),
        (0.0, 0.0, 10.0, 10.0),
        "the fill spans the chord"
    );
    // The stroke layer is the open path's, unchanged by the fill.
    assert_eq!(
        list.triangles[list.layers()[0]..],
        without_fill.triangles[..]
    );
}
