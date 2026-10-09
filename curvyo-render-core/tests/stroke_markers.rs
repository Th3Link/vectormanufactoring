//! Placement and drawing of the stroke markers (`specs/0018-stroke-markers`
//! criteria 3 to 5, 7 to 12, 14 to 17, 21): where they sit, which way they face,
//! how big they are, and that they share the stroke's layer.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::similar_names
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, DashPattern, Document, Length, MarkerCount, MarkerPlace,
    MarkerShape, NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit, Vec2,
    ViewTransform,
};
use curvyo_render_core::{DrawList, Vertex, build_artwork};

fn view() -> ViewTransform {
    ViewTransform::new(4.0, Point::new(0.0, 0.0))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn polyline(document: &Document, points: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    document.create_path(&anchors, closed)
}

fn edit(document: &Document, id: NodeId, edit: &StyleEdit) {
    document.edit_style(&[id], edit).unwrap();
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .map(|id| document.object(id).unwrap())
        .collect()
}

fn artwork(document: &Document) -> DrawList {
    build_artwork(&objects(document), view())
}

/// The vertices the markers add: the layer of the same document with every
/// marker slot None is the stroke alone, and markers are appended after it.
fn marker_vertices(document: &Document, id: NodeId) -> Vec<Vertex> {
    let with = artwork(document);
    let saved = document.path(id).unwrap().style.stroke.markers;
    for edit_none in [
        StyleEdit::MarkerStart(MarkerShape::None),
        StyleEdit::MarkerMid(MarkerShape::None),
        StyleEdit::MarkerEnd(MarkerShape::None),
    ] {
        document.edit_style(&[id], &edit_none).unwrap();
    }
    let without = artwork(document);
    document
        .edit_style(&[id], &StyleEdit::MarkerStart(saved.start))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::MarkerMid(saved.mid))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::MarkerEnd(saved.end))
        .unwrap();
    with.triangles[without.triangles.len()..].to_vec()
}

/// An arrow's three vertices as `(anchor, direction)`: the anchor is the middle
/// of its length, the direction runs from the base to the tip.
fn arrows(vertices: &[Vertex]) -> Vec<(Point, (f64, f64))> {
    vertices
        .chunks(3)
        .map(|t| {
            let tip = t[0].position;
            let base = pt(
                f64::midpoint(t[1].position.x, t[2].position.x),
                f64::midpoint(t[1].position.y, t[2].position.y),
            );
            let anchor = pt(f64::midpoint(tip.x, base.x), f64::midpoint(tip.y, base.y));
            let length = (tip.x - base.x).hypot(tip.y - base.y);
            (
                anchor,
                ((tip.x - base.x) / length, (tip.y - base.y) / length),
            )
        })
        .collect()
}

fn near(a: f64, b: f64, tolerance: f64) {
    assert!((a - b).abs() <= tolerance, "{a} vs {b}");
}

fn set(document: &Document, id: NodeId, edits: &[StyleEdit]) {
    for e in edits {
        edit(document, id, e);
    }
}

#[test]
fn an_end_and_a_start_arrow_on_a_line_are_sized_from_the_width_and_point_outward() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::StrokeWidth(mm(0.5)),
            StyleEdit::MarkerStart(MarkerShape::Arrow),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
        ],
    );
    let vertices = marker_vertices(&document, id);
    assert_eq!(vertices.len(), 6);
    let placed = arrows(&vertices);
    // Start first (the order of the slots), then End.
    let (start, end) = (&placed[0], &placed[1]);
    // End: tip at x = 101, base at x = 99 (criterion 15).
    near(end.0.x, 100.0, 1e-6);
    near(end.1.0, 1.0, 1e-9);
    let end_tip = vertices[3].position;
    near(end_tip.x, 101.0, 1e-6);
    // Start: outward, tip at x = -1.
    near(start.1.0, -1.0, 1e-9);
    near(vertices[0].position.x, -1.0, 1e-6);
    // Length 4 w = 2 mm, base 3 w = 1.5 mm (criterion 4).
    let base_width = (vertices[4].position.y - vertices[5].position.y).abs();
    near(base_width, 1.5, 1e-6);
}

#[test]
fn changing_the_width_resizes_every_marker() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::StrokeWidth(mm(2.0)),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
        ],
    );
    let vertices = marker_vertices(&document, id);
    near(vertices[0].position.x - 100.0, 4.0, 1e-6);
}

#[test]
fn a_dot_is_three_widths_across_and_centred_on_its_anchor() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::StrokeWidth(mm(0.5)),
            StyleEdit::MarkerEnd(MarkerShape::Dot),
        ],
    );
    let vertices = marker_vertices(&document, id);
    let xs: Vec<f64> = vertices.iter().map(|v| v.position.x).collect();
    let ys: Vec<f64> = vertices.iter().map(|v| v.position.y).collect();
    let (min_x, max_x) = (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    );
    let (min_y, max_y) = (
        ys.iter().copied().fold(f64::MAX, f64::min),
        ys.iter().copied().fold(f64::MIN, f64::max),
    );
    near(max_x - min_x, 1.5, 0.02);
    near(max_y - min_y, 1.5, 0.02);
    near(f64::midpoint(max_x, min_x), 100.0, 0.02);
    near(f64::midpoint(max_y, min_y), 0.0, 0.02);
}

#[test]
fn markers_share_the_strokes_layer_and_colour_so_a_translucent_stroke_does_not_darken() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap()),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
            StyleEdit::MarkerStart(MarkerShape::Dot),
        ],
    );
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 1, "stroke and markers are one layer");
    let alpha = list.triangles[0].color.a;
    assert!(
        list.triangles
            .iter()
            .all(|v| v.color == list.triangles[0].color)
    );
    assert_eq!(alpha, 128);
}

#[test]
fn a_fill_then_the_stroke_with_its_markers_in_tree_order() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::FillEnabled(true),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
        ],
    );
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 2, "fill, then stroke with markers");
}

#[test]
fn no_stroke_means_no_markers() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
            StyleEdit::StrokeEnabled(false),
        ],
    );
    assert_eq!(artwork(&document).triangle_count(), 0);
    edit(&document, id, &StyleEdit::StrokeEnabled(true));
    assert!(artwork(&document).triangle_count() > 0);
}

#[test]
fn spaced_markers_sit_at_k_over_n_plus_one_of_the_length() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Arrow),
            StyleEdit::MarkerCount(MarkerCount::new(3).unwrap()),
        ],
    );
    let placed = arrows(&marker_vertices(&document, id));
    assert_eq!(placed.len(), 3);
    for (marker, want) in placed.iter().zip([25.0, 50.0, 75.0]) {
        near(marker.0.x, want, 0.05);
        near(marker.1.0, 1.0, 1e-6);
    }
}

#[test]
fn spaced_markers_measure_along_a_curve() {
    // An S-curve from (0, 0) to (100, 0), symmetric about (50, 0): one marker at
    // half the arc length is at its middle.
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(1, 1),
                point: pt(0.0, 0.0),
                handle_in: Vec2::new(0.0, 0.0),
                handle_out: Vec2::new(0.0, 60.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor {
                id: AnchorId::new(1, 2),
                point: pt(100.0, 0.0),
                handle_in: Vec2::new(0.0, -60.0),
                handle_out: Vec2::new(0.0, 0.0),
                kind: AnchorKind::Corner,
            },
        ],
        false,
    );
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Arrow),
            StyleEdit::MarkerCount(MarkerCount::new(1).unwrap()),
        ],
    );
    let placed = arrows(&marker_vertices(&document, id));
    near(placed[0].0.x, 50.0, 0.5);
    near(placed[0].0.y, 0.0, 0.5);
}

#[test]
fn at_nodes_skips_the_ends_of_an_open_path_and_bisects_a_corner() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Arrow),
            StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        ],
    );
    let placed = arrows(&marker_vertices(&document, id));
    assert_eq!(placed.len(), 1);
    near(placed[0].0.x, 10.0, 1e-6);
    near(placed[0].0.y, 0.0, 1e-6);
    let along = std::f64::consts::FRAC_1_SQRT_2;
    near(placed[0].1.0, along, 1e-9);
    near(placed[0].1.1, along, 1e-9);
}

#[test]
fn at_nodes_on_a_path_of_two_nodes_draws_nothing() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (10.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Dot),
            StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        ],
    );
    assert_eq!(marker_vertices(&document, id).len(), 0);
}

#[test]
fn a_closed_path_has_no_ends_and_spaced_markers_start_on_the_first_node() {
    let document = Document::new(1);
    let id = polyline(
        &document,
        &[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)],
        true,
    );
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerStart(MarkerShape::Arrow),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
        ],
    );
    assert_eq!(marker_vertices(&document, id).len(), 0, "criterion 10");
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Arrow),
            StyleEdit::MarkerCount(MarkerCount::new(4).unwrap()),
        ],
    );
    let placed = arrows(&marker_vertices(&document, id));
    assert_eq!(placed.len(), 4);
    let corners = [(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)];
    for (marker, corner) in placed.iter().zip(corners) {
        near(marker.0.x, corner.0, 0.1);
        near(marker.0.y, corner.1, 0.1);
    }
    // At nodes on a closed path: one on every node, the first included.
    set(
        &document,
        id,
        &[StyleEdit::MarkerPlace(MarkerPlace::AtNodes)],
    );
    assert_eq!(arrows(&marker_vertices(&document, id)).len(), 4);
}

#[test]
fn a_zero_handle_node_takes_its_direction_from_the_next_control_point() {
    // A cubic whose handles are zero: still faces along the line.
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (0.0, 100.0)], false);
    set(&document, id, &[StyleEdit::MarkerEnd(MarkerShape::Arrow)]);
    let placed = arrows(&marker_vertices(&document, id));
    near(placed[0].1.0, 0.0, 1e-9);
    near(placed[0].1.1, 1.0, 1e-9);
}

#[test]
fn a_path_without_length_or_with_one_node_draws_no_marker() {
    let document = Document::new(1);
    let same = polyline(&document, &[(5.0, 5.0), (5.0, 5.0)], false);
    set(&document, same, &[StyleEdit::MarkerEnd(MarkerShape::Dot)]);
    assert_eq!(marker_vertices(&document, same).len(), 0);
    let one = polyline(&document, &[(20.0, 20.0)], false);
    set(&document, one, &[StyleEdit::MarkerEnd(MarkerShape::Dot)]);
    assert_eq!(marker_vertices(&document, one).len(), 0);
}

#[test]
fn a_dash_gap_does_not_hide_a_marker() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
            StyleEdit::MarkerStart(MarkerShape::Dot),
        ],
    );
    let solid = marker_vertices(&document, id);
    edit(
        &document,
        id,
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 30.0]).unwrap()),
    );
    let dashed = marker_vertices(&document, id);
    assert_eq!(solid, dashed);
}

#[test]
fn moving_a_node_moves_the_marker() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(&document, id, &[StyleEdit::MarkerEnd(MarkerShape::Arrow)]);
    let before = arrows(&marker_vertices(&document, id))[0].0;
    let objects = objects(&document);
    let ObjectSnapshot::Path(mut path) = objects[0].clone() else {
        panic!("a path");
    };
    path.anchors[1].point = pt(100.0, 30.0);
    let list = build_artwork(&[ObjectSnapshot::Path(path)], view());
    let tip_row = &list.triangles[list.triangles.len() - 3];
    assert!(
        tip_row.position.y > before.y + 20.0,
        "the arrow follows the node"
    );
}

#[test]
fn a_primitive_draws_no_markers_even_with_settings_in_its_snapshot() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(20.0),
        height: mm(10.0),
    });
    let plain = build_artwork(&objects(&document), view());
    let ObjectSnapshot::Primitive(mut shape) = document.object(id).unwrap() else {
        panic!("a primitive");
    };
    shape.style.stroke.markers.mid = MarkerShape::Dot;
    shape.style.stroke.markers.start = MarkerShape::Arrow;
    shape.style.stroke.markers.end = MarkerShape::Arrow;
    let marked = build_artwork(&[ObjectSnapshot::Primitive(shape)], view());
    assert_eq!(plain.triangles, marked.triangles);
}

#[test]
fn at_most_fifty_thousand_markers_are_drawn_per_frame() {
    let document = Document::new(1);
    for k in 0..120_u32 {
        let y = f64::from(k);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, u64::from(k) * 2 + 1), pt(0.0, y)),
                NewAnchor::corner(AnchorId::new(1, u64::from(k) * 2 + 2), pt(100.0, y)),
            ],
            false,
        );
        set(
            &document,
            id,
            &[
                StyleEdit::MarkerMid(MarkerShape::Arrow),
                StyleEdit::MarkerCount(MarkerCount::new(500).unwrap()),
            ],
        );
    }
    let list = artwork(&document);
    // 120 x 500 = 60 000 wanted; each arrow is one triangle, plus the strokes.
    let stroke_only = {
        for id in document.object_ids() {
            edit(&document, id, &StyleEdit::MarkerMid(MarkerShape::None));
        }
        artwork(&document).triangles.len()
    };
    assert_eq!((list.triangles.len() - stroke_only) / 3, 50_000);
}

#[test]
fn a_count_above_five_hundred_draws_five_hundred() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (1000.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::MarkerMid(MarkerShape::Arrow),
            StyleEdit::MarkerCount(MarkerCount::new(900).unwrap()),
        ],
    );
    assert_eq!(arrows(&marker_vertices(&document, id)).len(), 500);
}

#[test]
fn colour_is_the_strokes_rgb_at_its_alpha() {
    let document = Document::new(1);
    let id = polyline(&document, &[(0.0, 0.0), (100.0, 0.0)], false);
    set(
        &document,
        id,
        &[
            StyleEdit::StrokeColor(Color {
                r: 10,
                g: 20,
                b: 30,
            }),
            StyleEdit::MarkerEnd(MarkerShape::Arrow),
        ],
    );
    for vertex in marker_vertices(&document, id) {
        assert_eq!(
            (vertex.color.r, vertex.color.g, vertex.color.b),
            (10, 20, 30)
        );
    }
}
