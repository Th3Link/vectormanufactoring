//! Independent tester cases for `0018-stroke-markers`, render side: shapes and
//! sizes, anchors (open, closed, spaced, at nodes), direction, one layer with
//! the stroke (no darkening), draw order, dashes, budgets, degenerate paths.
//! Written from `specification.md` and `adrs.md` before the implementation was
//! read. The pictures are judged on a CPU compositor built from the
//! [`DrawList`] contract alone.
//!
//! Criteria: 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 21, 29.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss, clippy::similar_names, clippy::doc_markdown)]
#![allow(clippy::needless_range_loop, clippy::type_complexity, dead_code)]
#![allow(
    missing_docs,
    clippy::assert_is_empty,
    clippy::semicolon_if_nothing_returned,
    clippy::suboptimal_flops,
    clippy::manual_midpoint,
    clippy::manual_range_contains
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, DashPattern, Document, Length, LineCap, MarkerCount, MarkerPlace,
    MarkerShape, Markers, NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit,
    Vec2, ViewTransform, unpack,
};
use curvyo_render_core::{DrawList, build_artwork};

// ---------------------------------------------------------------- helpers

type Shape = MarkerShape;

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn view() -> ViewTransform {
    ViewTransform::new(10.0, pt(0.0, 0.0))
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn make_path(doc: &Document, points: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| anchor(i as u64 + 1, *x, *y))
        .collect();
    doc.create_path(&anchors, closed)
}

fn obj(doc: &Document, id: NodeId) -> ObjectSnapshot {
    doc.object(id).unwrap()
}

fn edit(doc: &Document, id: NodeId, edits: &[StyleEdit]) {
    for e in edits {
        doc.edit_style(&[id], e).unwrap();
    }
}

#[derive(Clone, Copy)]
struct M {
    start: Shape,
    mid: Shape,
    end: Shape,
    place: MarkerPlace,
    count: u32,
}

const NONE: M = M {
    start: Shape::None,
    mid: Shape::None,
    end: Shape::None,
    place: MarkerPlace::Spaced,
    count: 1,
};

fn apply(doc: &Document, id: NodeId, w: f64, m: M) {
    edit(
        doc,
        id,
        &[
            StyleEdit::StrokeWidth(mm(w)),
            StyleEdit::MarkerStart(m.start),
            StyleEdit::MarkerMid(m.mid),
            StyleEdit::MarkerEnd(m.end),
            StyleEdit::MarkerPlace(m.place),
            StyleEdit::MarkerCount(MarkerCount::new(m.count).unwrap()),
        ],
    );
}

/// One path, drawn alone, with stroke width `w` and the markers `m`.
fn scene(points: &[(f64, f64)], closed: bool, w: f64, m: M) -> (Document, NodeId, DrawList) {
    let doc = Document::new(1);
    let p = make_path(&doc, points, closed);
    apply(&doc, p, w, m);
    let list = build_artwork(&[obj(&doc, p)], view());
    (doc, p, list)
}

fn art(doc: &Document, ids: &[NodeId]) -> DrawList {
    let objs: Vec<ObjectSnapshot> = ids.iter().map(|i| obj(doc, *i)).collect();
    build_artwork(&objs, view())
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

fn layer_ranges(list: &DrawList) -> Vec<(usize, usize)> {
    let mut start = 0;
    let mut out = Vec::new();
    for &end in list.layers() {
        out.push((start, end));
        start = end;
    }
    out
}

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

fn covered(list: &DrawList, p: Point) -> bool {
    !is_white(composite_at(list, p))
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= 1.01)
}

fn all_finite(list: &DrawList) -> bool {
    list.triangles
        .iter()
        .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
}

fn tri(list: &DrawList) -> usize {
    list.triangle_count()
}

/// Number of triangles the markers add: `with` minus the same path without.
fn marker_tris(points: &[(f64, f64)], closed: bool, w: f64, m: M) -> usize {
    let (_, _, with) = scene(points, closed, w, m);
    let (_, _, without) = scene(points, closed, w, NONE);
    tri(&with) - tri(&without)
}

/// Triangles of one dot of width `w`: calibrated on a 3-node open path with
/// Middle Dot At nodes, which has exactly one marker (criterion 9).
fn one_dot(w: f64) -> usize {
    marker_tris(
        &[(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)],
        false,
        w,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    )
}

/// Covered runs along a horizontal scan at height `y` (document mm), step
/// 0.005 mm: the centres of the runs.
fn run_centres(list: &DrawList, y: f64, x0: f64, x1: f64) -> Vec<f64> {
    let mut centres = Vec::new();
    let mut start: Option<f64> = None;
    let mut x = x0;
    let step = 0.005;
    while x <= x1 {
        let c = covered(list, pt(x, y));
        match (c, start) {
            (true, None) => start = Some(x),
            (false, Some(s)) => {
                centres.push((s + x - step) / 2.0);
                start = None;
            }
            _ => {}
        }
        x += step;
    }
    if let Some(s) = start {
        centres.push((s + x1) / 2.0);
    }
    centres
}

/// Is a dot of a width-1 stroke (radius 1.5, stroke half width 0.5) centred
/// within 0.5 mm of `centre` along the tangent `dir`? The test points sit 1.0
/// mm to the side, outside the stroke.
fn dot_near(list: &DrawList, centre: Point, dir: Vec2) -> bool {
    let len = dir.x.hypot(dir.y);
    let (tx, ty) = (dir.x / len, dir.y / len);
    let (nx, ny) = (-ty, tx);
    let at = |s: f64, side: f64| pt(centre.x + tx * s + nx * side, centre.y + ty * s + ny * side);
    [-1.0, 1.0].iter().all(|side| {
        covered(list, at(0.0, *side))
            && covered(list, at(0.4, *side))
            && covered(list, at(-0.4, *side))
            && !covered(list, at(2.8, *side))
            && !covered(list, at(-2.8, *side))
    })
}

// -------------------------------------------- AC 4/7/15: arrow geometry

const LINE: [(f64, f64); 2] = [(0.0, 0.0), (100.0, 0.0)];

#[test]
fn an_end_arrow_is_2_mm_long_and_1_5_wide_at_half_a_mm_width() {
    let (_, _, l) = scene(
        &LINE,
        false,
        0.5,
        M {
            end: Shape::Arrow,
            ..NONE
        },
    );
    // Tip at x = 101, base at x = 99 (criterion 15).
    assert!(covered(&l, pt(100.95, 0.0)));
    assert!(!covered(&l, pt(101.05, 0.0)));
    // Past the butt end of the stroke only the arrow draws; half width at
    // x is 0.75 * (101 - x) / 2.
    for (x, half) in [(100.2, 0.75 * 0.8 / 2.0), (100.5, 0.75 * 0.5 / 2.0)] {
        assert!(covered(&l, pt(x, half - 0.03)), "x={x}");
        assert!(covered(&l, pt(x, -(half - 0.03))), "x={x}");
        assert!(!covered(&l, pt(x, half + 0.03)), "x={x}");
        assert!(!covered(&l, pt(x, -(half + 0.03))), "x={x}");
    }
    // Base corners reach 0.75 either side at x = 99.
    assert!(covered(&l, pt(99.03, 0.71)));
    assert!(covered(&l, pt(99.03, -0.71)));
    assert!(!covered(&l, pt(99.03, 0.79)));
    assert!(!covered(&l, pt(98.9, 0.6)), "nothing behind the base");
    // Nothing else along the line.
    assert!(!covered(&l, pt(50.0, 0.6)));
    assert!(!covered(&l, pt(1.0, 0.6)));
}

#[test]
fn a_start_arrow_points_outward_with_its_tip_at_minus_one() {
    let (_, _, l) = scene(
        &LINE,
        false,
        0.5,
        M {
            start: Shape::Arrow,
            ..NONE
        },
    );
    assert!(covered(&l, pt(-0.95, 0.0)));
    assert!(!covered(&l, pt(-1.05, 0.0)));
    assert!(covered(&l, pt(0.97, 0.7)), "base at x = +1");
    assert!(!covered(&l, pt(1.1, 0.5)), "behind the base nothing");
    // Narrow near the tip, wide near the base (outward).
    assert!(!covered(&l, pt(-0.8, 0.3)));
    assert!(covered(&l, pt(-0.2, 0.3)));
    assert!(!covered(&l, pt(100.5, 0.0)), "no End marker");
}

#[test]
fn start_and_end_arrows_make_a_double_headed_arrow() {
    let (_, _, l) = scene(
        &LINE,
        false,
        0.5,
        M {
            start: Shape::Arrow,
            end: Shape::Arrow,
            ..NONE
        },
    );
    assert!(covered(&l, pt(-0.95, 0.0)) && covered(&l, pt(100.95, 0.0)));
    // Mirror image about x = 50: every sampled point has the same coverage as
    // its mirror.
    for ix in 0..60 {
        for iy in 0..40 {
            let x = -2.0 + f64::from(ix) * 0.05 + 0.0137;
            let y = -1.0 + f64::from(iy) * 0.05 + 0.0071;
            assert_eq!(
                covered(&l, pt(x, y)),
                covered(&l, pt(100.0 - x, y)),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn a_dot_is_a_circle_of_three_times_the_width_centred_on_the_anchor() {
    let (_, _, l) = scene(
        &LINE,
        false,
        0.5,
        M {
            start: Shape::Dot,
            end: Shape::Dot,
            ..NONE
        },
    );
    // Radius 0.75 around (0,0) and (100,0).
    for (cx, sign) in [(0.0, -1.0), (100.0, 1.0)] {
        assert!(covered(&l, pt(cx + sign * 0.72, 0.0)));
        assert!(!covered(&l, pt(cx + sign * 0.79, 0.0)));
        assert!(covered(&l, pt(cx, 0.72)));
        assert!(covered(&l, pt(cx, -0.72)));
        assert!(!covered(&l, pt(cx, 0.79)));
        assert!(!covered(&l, pt(cx, -0.79)));
        let d = 0.74 / 2.0_f64.sqrt();
        assert!(covered(&l, pt(cx + d, d)) && covered(&l, pt(cx - d, -d)));
        let d = 0.78 / 2.0_f64.sqrt();
        assert!(!covered(&l, pt(cx + d, d)) && !covered(&l, pt(cx - d, -d)));
    }
}

#[test]
fn changing_the_width_resizes_every_marker() {
    for w in [0.2, 1.0, 2.0, 5.0] {
        let (_, _, l) = scene(
            &LINE,
            false,
            w,
            M {
                start: Shape::Dot,
                end: Shape::Arrow,
                ..NONE
            },
        );
        let r = 1.5 * w;
        assert!(covered(&l, pt(-(r - 0.03 * w), 0.0)), "w={w}");
        assert!(!covered(&l, pt(-(r + 0.03 * w), 0.0)), "w={w}");
        // End arrow: tip at 100 + 2w.
        assert!(covered(&l, pt(100.0 + 2.0 * w - 0.03 * w, 0.0)), "w={w}");
        assert!(!covered(&l, pt(100.0 + 2.0 * w + 0.03 * w, 0.0)), "w={w}");
    }
}

#[test]
fn slots_are_independent_start_arrow_middle_dot_end_arrow() {
    let (_, _, l) = scene(
        &LINE,
        false,
        1.0,
        M {
            start: Shape::Arrow,
            mid: Shape::Dot,
            end: Shape::Arrow,
            count: 1,
            place: MarkerPlace::Spaced,
        },
    );
    assert!(covered(&l, pt(-1.9, 0.0)) && covered(&l, pt(101.9, 0.0)));
    assert!(covered(&l, pt(50.0, 1.4)), "middle dot (radius 1.5)");
    assert!(!covered(&l, pt(50.0, 1.7)));
    // The middle marker is a dot, not an arrow: round, so symmetric in x.
    assert_eq!(covered(&l, pt(51.2, 1.0)), covered(&l, pt(48.8, 1.0)));
    assert!(covered(&l, pt(51.0, 0.9)) && covered(&l, pt(49.0, 0.9)));
    let (_, _, a) = scene(
        &LINE,
        false,
        1.0,
        M {
            mid: Shape::Arrow,
            ..NONE
        },
    );
    // Arrow along travel: tip at +2 from 50 => (51.9, 0) is out of the stroke? it is
    // inside the stroke; use above the line.
    assert!(
        covered(&a, pt(49.0, 1.0)),
        "wide base side at u = -1: half width 1.125"
    );
    assert!(
        !covered(&a, pt(51.0, 1.0)),
        "narrow near the tip: half width 0.375"
    );
}

#[test]
fn markers_are_translucent_exactly_as_the_stroke_and_use_its_colour() {
    let doc = Document::new(1);
    let p = make_path(&doc, &LINE, false);
    apply(
        &doc,
        p,
        1.0,
        M {
            start: Shape::Arrow,
            mid: Shape::Dot,
            end: Shape::Dot,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[
            StyleEdit::StrokeColor(Color {
                r: 200,
                g: 30,
                b: 90,
            }),
            StyleEdit::StrokeOpacity(Opacity::new(0.4).unwrap()),
        ],
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    let stroke_only = composite_at(&l, pt(20.0, 0.0));
    let arrow_only = composite_at(&l, pt(-1.2, 0.0));
    let dot_only = composite_at(&l, pt(50.0, 1.2));
    let overlap_dot = composite_at(&l, pt(100.0, 0.0));
    let overlap_dot2 = composite_at(&l, pt(99.5, 0.0));
    let overlap_arrow = composite_at(&l, pt(0.7, 0.0));
    assert!(!is_white(stroke_only));
    for (name, px) in [
        ("arrow only", arrow_only),
        ("dot only", dot_only),
        ("dot overlap", overlap_dot),
        ("dot overlap 2", overlap_dot2),
        ("arrow overlap", overlap_arrow),
    ] {
        assert!(near(px, stroke_only), "{name}: {px:?} vs {stroke_only:?}");
    }
    // The vertices of the single layer all carry the stroke colour and alpha.
    let a = (0.4_f64 * 255.0).round() as i32;
    for v in &l.triangles {
        assert_eq!((v.color.r, v.color.g, v.color.b), (200, 30, 90));
        assert!((i32::from(v.color.a) - a).abs() <= 1, "alpha {}", v.color.a);
    }
}

#[test]
fn a_half_transparent_black_stroke_has_the_same_pixel_in_overlap_and_in_marker_alone() {
    // The spec's own example (criterion 5).
    let doc = Document::new(1);
    let p = make_path(&doc, &LINE, false);
    apply(
        &doc,
        p,
        1.0,
        M {
            end: Shape::Arrow,
            mid: Shape::Dot,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap())],
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    let overlap = composite_at(&l, pt(99.0, 0.0));
    let alone = composite_at(&l, pt(100.8, 0.0));
    let stroke = composite_at(&l, pt(10.0, 0.0));
    let mid_overlap = composite_at(&l, pt(50.0, 0.0));
    for px in [overlap, alone, stroke, mid_overlap] {
        assert!((px[0] - 127.5).abs() <= 1.01, "{px:?}");
    }
}

#[test]
fn overlapping_markers_of_one_object_do_not_darken_each_other() {
    // 40 dots of radius 1.5 on a 20 mm line overlap one another heavily.
    let doc = Document::new(1);
    let p = make_path(&doc, &[(0.0, 0.0), (20.0, 0.0)], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Dot,
            count: 40,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap())],
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    let reference = composite_at(&l, pt(10.0, 0.0));
    assert!((reference[0] - 127.5).abs() <= 1.01);
    let mut checked = 0;
    let mut x = 0.3;
    while x < 19.7 {
        for y in [-1.2, -0.8, 0.0, 0.8, 1.2] {
            let px = composite_at(&l, pt(x, y));
            if !is_white(px) {
                assert!(near(px, reference), "({x}, {y}) {px:?}");
                checked += 1;
            }
        }
        x += 0.173;
    }
    assert!(checked > 200);
}

#[test]
fn markers_do_not_add_a_layer() {
    let (_, _, plain) = scene(&LINE, false, 1.0, NONE);
    let (_, _, marked) = scene(
        &LINE,
        false,
        1.0,
        M {
            start: Shape::Arrow,
            mid: Shape::Dot,
            end: Shape::Dot,
            count: 7,
            ..NONE
        },
    );
    assert_eq!(plain.layers().len(), marked.layers().len());
    assert!(tri(&marked) > tri(&plain));
}

// -------------------------------------------- AC 8: spaced on open paths

#[test]
fn spaced_markers_on_a_straight_100_mm_path_sit_at_k_over_n_plus_one() {
    // A width-0.5 line; dots have radius 0.75. Scan above the line.
    for n in [1_u32, 2, 3, 4, 7] {
        let (_, _, l) = scene(
            &LINE,
            false,
            0.5,
            M {
                mid: Shape::Dot,
                count: n,
                ..NONE
            },
        );
        let centres = run_centres(&l, 0.6, -1.0, 101.0);
        assert_eq!(centres.len(), n as usize, "n={n}: {centres:?}");
        for (k, c) in centres.iter().enumerate() {
            let want = 100.0 * (k as f64 + 1.0) / (f64::from(n) + 1.0);
            assert!((c - want).abs() <= 0.05, "n={n} k={}: {c} vs {want}", k + 1);
        }
    }
}

#[test]
fn the_count_is_for_the_whole_path_not_per_segment() {
    // Nodes at 0, 30 and 100: three markers still at 25, 50, 75.
    let (_, _, l) = scene(
        &[(0.0, 0.0), (30.0, 0.0), (100.0, 0.0)],
        false,
        0.5,
        M {
            mid: Shape::Dot,
            count: 3,
            ..NONE
        },
    );
    let centres = run_centres(&l, 0.6, -1.0, 101.0);
    assert_eq!(centres.len(), 3, "{centres:?}");
    for (c, want) in centres.iter().zip([25.0, 50.0, 75.0]) {
        assert!((c - want).abs() <= 0.05, "{c} vs {want}");
    }
}

#[test]
fn spaced_markers_follow_arc_length_on_an_unequal_polyline() {
    // 10 mm then 90 mm then 100 mm turning back along y: length 200, N = 1 at
    // arc length 100: 10 + 90 = 100 -> exactly the second node.
    let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 90.0), (110.0, 90.0)];
    let (_, _, l) = scene(
        &pts,
        false,
        1.0,
        M {
            mid: Shape::Dot,
            count: 1,
            ..NONE
        },
    );
    // Total length 10 + 90 + 100 = 200; centre at arc length 100 = (10,90) the
    // corner... that is length 10+90 = 100. Centre of the dot at the corner;
    // sample points 1.0 away from the corner, outside the stroke's miter area.
    let corner = pt(10.0, 90.0);
    assert!(
        covered(&l, pt(corner.x + 1.0, corner.y - 1.0)),
        "dot at the corner"
    );
    assert!(covered(&l, pt(corner.x - 1.0, corner.y + 1.0)));
    assert!(!covered(&l, pt(corner.x + 1.0, corner.y - 2.2)));
}

#[test]
fn a_curve_of_known_length_puts_its_single_marker_at_half_the_arc_length() {
    // Cubic from (0,0) to (100,0) with vertical handles of 60: symmetric, so
    // the mid arc length is t = 0.5, the point (50, 45).
    let doc = Document::new(1);
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
    let p = doc.create_path(&[a, b], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Dot,
            count: 1,
            ..NONE
        },
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    assert!(dot_near(&l, pt(50.0, 45.0), Vec2::new(1.0, 0.0)));
    // Numeric arc length oracle, N = 3.
    let bez = |t: f64| {
        let u = 1.0 - t;
        let x = 3.0 * u * t * t * 100.0 + t * t * t * 100.0;
        let y = 3.0 * u * u * t * 60.0 + 3.0 * u * t * t * 60.0;
        (x, y)
    };
    let steps = 200_000;
    let mut cum = vec![0.0_f64; steps + 1];
    let mut prev = bez(0.0);
    for i in 1..=steps {
        let c = bez(i as f64 / steps as f64);
        cum[i] = cum[i - 1] + (c.0 - prev.0).hypot(c.1 - prev.1);
        prev = c;
    }
    let total = cum[steps];
    assert!(total > 100.0);
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Dot,
            count: 3,
            ..NONE
        },
    );
    let l3 = build_artwork(&[obj(&doc, p)], view());
    for k in 1..=3 {
        let target = total * f64::from(k) / 4.0;
        let i = cum.partition_point(|v| *v < target);
        let t = i as f64 / steps as f64;
        let c = bez(t);
        let t2 = bez((t + 1e-4).min(1.0));
        let t1 = bez((t - 1e-4).max(0.0));
        assert!(
            dot_near(&l3, pt(c.0, c.1), Vec2::new(t2.0 - t1.0, t2.1 - t1.1)),
            "k={k} expected near ({:.2}, {:.2})",
            c.0,
            c.1
        );
    }
}

// --------------------------------------------- AC 9: at nodes (open path)

#[test]
fn at_nodes_marks_every_inner_node_and_never_the_ends() {
    let pts = [
        (0.0, 0.0),
        (30.0, 0.0),
        (60.0, 0.0),
        (90.0, 0.0),
        (120.0, 0.0),
    ];
    let (_, _, l) = scene(
        &pts,
        false,
        0.5,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            count: 9,
            ..NONE
        },
    );
    let centres = run_centres(&l, 0.6, -2.0, 122.0);
    assert_eq!(centres.len(), 3, "{centres:?}");
    for (c, want) in centres.iter().zip([30.0, 60.0, 90.0]) {
        assert!((c - want).abs() <= 0.05);
    }
}

#[test]
fn at_nodes_on_a_two_node_path_draws_nothing() {
    let (_, _, l) = scene(
        &LINE,
        false,
        0.5,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    let (_, _, plain) = scene(&LINE, false, 0.5, NONE);
    assert_eq!(tri(&l), tri(&plain));
}

#[test]
fn start_and_end_do_not_double_up_with_at_nodes() {
    // Same dot for all three slots: exactly n dots in total.
    let pts = [(0.0, 0.0), (30.0, 0.0), (60.0, 0.0), (90.0, 0.0)];
    let d = one_dot(0.5);
    let n = marker_tris(
        &pts,
        false,
        0.5,
        M {
            start: Shape::Dot,
            mid: Shape::Dot,
            end: Shape::Dot,
            place: MarkerPlace::AtNodes,
            count: 1,
        },
    );
    assert_eq!(n, d * 4, "4 nodes -> 4 dots (2 ends + 2 inner)");
}

#[test]
fn node_counting_uses_the_dot_triangle_oracle() {
    let d = one_dot(0.5);
    assert!(d > 0);
    let pts: Vec<(f64, f64)> = (0..12)
        .map(|i| (f64::from(i) * 10.0, (f64::from(i) * 7.0).sin() * 5.0))
        .collect();
    // Open at nodes: 10 inner nodes.
    assert_eq!(
        marker_tris(
            &pts,
            false,
            0.5,
            M {
                mid: Shape::Dot,
                place: MarkerPlace::AtNodes,
                ..NONE
            }
        ),
        10 * d
    );
    // Closed at nodes: all 12.
    assert_eq!(
        marker_tris(
            &pts,
            true,
            0.5,
            M {
                mid: Shape::Dot,
                place: MarkerPlace::AtNodes,
                ..NONE
            }
        ),
        12 * d
    );
    // Spaced N on open and closed.
    for n in [1, 5, 33] {
        assert_eq!(
            marker_tris(
                &pts,
                false,
                0.5,
                M {
                    mid: Shape::Dot,
                    count: n,
                    ..NONE
                }
            ),
            n as usize * d,
            "open n={n}"
        );
        assert_eq!(
            marker_tris(
                &pts,
                true,
                0.5,
                M {
                    mid: Shape::Dot,
                    count: n,
                    ..NONE
                }
            ),
            n as usize * d,
            "closed n={n}"
        );
    }
    // Place is respected: AtNodes ignores Count.
    assert_eq!(
        marker_tris(
            &pts,
            false,
            0.5,
            M {
                mid: Shape::Dot,
                place: MarkerPlace::AtNodes,
                count: 77,
                ..NONE
            }
        ),
        10 * d
    );
}

// --------------------------------------------------- AC 10: closed paths

const SQUARE: [(f64, f64); 4] = [(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)];

fn corner_has_dot(l: &DrawList, cx: f64, cy: f64, ox: f64, oy: f64) -> bool {
    // Outward diagonal point 1.0 mm from the corner on each axis: only a dot
    // of radius 1.5 (width 1) can cover it; the mitred stroke ends at 0.5.
    covered(l, pt(cx + ox, cy + oy))
}

#[test]
fn a_closed_square_with_four_spaced_markers_has_them_on_the_four_corners() {
    let (_, _, l) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            count: 4,
            ..NONE
        },
    );
    assert!(corner_has_dot(&l, 0.0, 0.0, -1.0, -1.0));
    assert!(corner_has_dot(&l, 40.0, 0.0, 1.0, -1.0));
    assert!(corner_has_dot(&l, 40.0, 40.0, 1.0, 1.0));
    assert!(corner_has_dot(&l, 0.0, 40.0, -1.0, 1.0));
    // And none at the edge middles.
    assert!(!covered(&l, pt(20.0, -1.0)));
    assert!(!covered(&l, pt(41.0, 20.0)));
}

#[test]
fn a_closed_path_with_one_spaced_marker_has_it_on_the_first_node() {
    let (_, _, l) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            count: 1,
            ..NONE
        },
    );
    assert!(corner_has_dot(&l, 0.0, 0.0, -1.0, -1.0));
    assert!(!corner_has_dot(&l, 40.0, 0.0, 1.0, -1.0));
    assert!(!corner_has_dot(&l, 40.0, 40.0, 1.0, 1.0));
    assert!(!corner_has_dot(&l, 0.0, 40.0, -1.0, 1.0));
}

#[test]
fn closed_spaced_n_is_k_over_n_from_the_first_node() {
    // N = 2: first node and the opposite corner (length 160, half = 80).
    let (_, _, l) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            count: 2,
            ..NONE
        },
    );
    assert!(corner_has_dot(&l, 0.0, 0.0, -1.0, -1.0));
    assert!(corner_has_dot(&l, 40.0, 40.0, 1.0, 1.0));
    assert!(!corner_has_dot(&l, 40.0, 0.0, 1.0, -1.0));
    assert!(!corner_has_dot(&l, 0.0, 40.0, -1.0, 1.0));
    // N = 3: at 0, 53.33 (on the right edge at y = 13.33), 106.67 (on the
    // bottom edge at x = 13.33).
    let (_, _, l) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            count: 3,
            ..NONE
        },
    );
    assert!(corner_has_dot(&l, 0.0, 0.0, -1.0, -1.0));
    assert!(dot_near(
        &l,
        pt(40.0, 160.0 / 3.0 - 40.0),
        Vec2::new(0.0, 1.0)
    ));
    assert!(dot_near(
        &l,
        pt(40.0 - (320.0 / 3.0 - 80.0), 40.0),
        Vec2::new(-1.0, 0.0)
    ));
    assert!(!corner_has_dot(&l, 40.0, 40.0, 1.0, 1.0));
}

#[test]
fn a_closed_path_has_no_start_or_end_marker_and_keeps_the_settings() {
    let (doc, p, with) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            start: Shape::Arrow,
            end: Shape::Dot,
            ..NONE
        },
    );
    let (_, _, plain) = scene(&SQUARE, true, 1.0, NONE);
    assert_eq!(with, plain, "identical draw list");
    // Settings are stored.
    match obj(&doc, p) {
        ObjectSnapshot::Path(path) => {
            assert_eq!(path.style.stroke.markers.start, Shape::Arrow);
            assert_eq!(path.style.stroke.markers.end, Shape::Dot);
        }
        ObjectSnapshot::Primitive(_) => panic!(),
    }
}

#[test]
fn closed_at_nodes_includes_the_first_node() {
    let (_, _, l) = scene(
        &SQUARE,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    assert!(corner_has_dot(&l, 0.0, 0.0, -1.0, -1.0));
    assert!(corner_has_dot(&l, 40.0, 0.0, 1.0, -1.0));
    assert!(corner_has_dot(&l, 40.0, 40.0, 1.0, 1.0));
    assert!(corner_has_dot(&l, 0.0, 40.0, -1.0, 1.0));
}

// ---------------------------------------- AC 11: degenerate paths, AC 3

#[test]
fn fewer_than_two_nodes_or_zero_length_draws_no_marker_in_any_slot() {
    let all_slots = M {
        start: Shape::Arrow,
        mid: Shape::Dot,
        end: Shape::Arrow,
        count: 5,
        place: MarkerPlace::Spaced,
    };
    let nodes = M {
        place: MarkerPlace::AtNodes,
        ..all_slots
    };
    for (pts, closed) in [
        (vec![(5.0, 5.0)], false),
        (vec![(5.0, 5.0)], true),
        (vec![(5.0, 5.0), (5.0, 5.0)], false),
        (vec![(5.0, 5.0), (5.0004, 5.0)], false),
        (vec![(5.0, 5.0), (5.0, 5.0), (5.0, 5.0)], true),
    ] {
        for m in [all_slots, nodes] {
            let (_, _, with) = scene(&pts, closed, 1.0, m);
            let (_, _, plain) = scene(&pts, closed, 1.0, NONE);
            assert_eq!(tri(&with), tri(&plain), "{pts:?} closed={closed}");
            assert!(all_finite(&with));
        }
    }
}

#[test]
fn zero_nodes_draws_nothing() {
    let doc = Document::new(1);
    let p = doc.create_path(&[], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            start: Shape::Arrow,
            mid: Shape::Dot,
            end: Shape::Arrow,
            ..NONE
        },
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    assert_eq!(tri(&l), 0);
}

#[test]
fn stroke_off_alpha_zero_or_width_zero_draws_no_marker() {
    let m = M {
        start: Shape::Arrow,
        mid: Shape::Dot,
        end: Shape::Arrow,
        ..NONE
    };
    for kind in 0..3 {
        let doc = Document::new(1);
        let p = make_path(&doc, &LINE, false);
        apply(&doc, p, 1.0, m);
        match kind {
            0 => edit(&doc, p, &[StyleEdit::StrokeEnabled(false)]),
            1 => edit(&doc, p, &[StyleEdit::StrokeWidth(mm(0.0))]),
            _ => edit(
                &doc,
                p,
                &[StyleEdit::StrokeOpacity(Opacity::new(0.0).unwrap())],
            ),
        }
        let l = build_artwork(&[obj(&doc, p)], view());
        assert_eq!(tri(&l), 0, "kind {kind}");
        // The markers come back with the stroke.
        edit(
            &doc,
            p,
            &[
                StyleEdit::StrokeEnabled(true),
                StyleEdit::StrokeWidth(mm(1.0)),
                StyleEdit::StrokeOpacity(Opacity::new(1.0).unwrap()),
            ],
        );
        let l = build_artwork(&[obj(&doc, p)], view());
        assert!(covered(&l, pt(101.9, 0.0)), "kind {kind}");
    }
}

#[test]
fn a_fill_only_object_with_markers_set_shows_none() {
    let doc = Document::new(1);
    let p = make_path(&doc, &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)], true);
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[
            StyleEdit::FillEnabled(true),
            StyleEdit::StrokeEnabled(false),
        ],
    );
    let with = build_artwork(&[obj(&doc, p)], view());
    edit(&doc, p, &[StyleEdit::MarkerMid(Shape::None)]);
    let without = build_artwork(&[obj(&doc, p)], view());
    assert_eq!(with, without);
}

// ------------------------------------------------- AC 12: dashes

#[test]
fn a_marker_is_drawn_where_a_dash_pattern_has_a_gap() {
    let pts = [(0.0, 0.0), (103.0, 0.0)];
    let doc = Document::new(1);
    let p = make_path(&doc, &pts, false);
    apply(
        &doc,
        p,
        1.0,
        M {
            end: Shape::Dot,
            mid: Shape::Dot,
            count: 1,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[
            StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 1.0]).unwrap()),
            StyleEdit::StrokeCap(LineCap::Butt),
        ],
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    // The end at 103 is in a gap of [1, 1] (103 mod 2 = 1: gap starts), the
    // mid marker at 51.5 is mid-gap (51.5 mod 2 = 1.5). Both dots are there.
    assert!(covered(&l, pt(103.0, 0.0)));
    assert!(covered(&l, pt(51.5, 0.0)));
    assert!(covered(&l, pt(51.5, 1.4)));
    assert!(covered(&l, pt(104.4, 0.0)));
    // Between the dot and the dashes a gap is still a gap.
    assert!(covered(&l, pt(20.5, 0.0)), "dash");
    assert!(!covered(&l, pt(21.5, 0.0)), "gap");
}

#[test]
fn markers_do_not_move_with_the_dash_phase_or_caps() {
    // The same marker centres whatever the pattern and cap.
    let base = M {
        mid: Shape::Dot,
        count: 3,
        ..NONE
    };
    for (dash, cap) in [
        (vec![0.5, 0.5], LineCap::Round),
        (vec![3.0, 2.0, 1.0], LineCap::Square),
        (vec![1.0], LineCap::Butt),
    ] {
        let doc = Document::new(1);
        let p = make_path(&doc, &LINE, false);
        apply(&doc, p, 0.5, base);
        edit(
            &doc,
            p,
            &[
                StyleEdit::StrokeDash(DashPattern::new(dash).unwrap()),
                StyleEdit::StrokeCap(cap),
            ],
        );
        let l = build_artwork(&[obj(&doc, p)], view());
        for want in [25.0, 50.0, 75.0] {
            // Above the dashed stroke (and its square caps): a dot reaches 0.75.
            assert!(covered(&l, pt(want, 0.7)), "{want}");
            assert!(!covered(&l, pt(want, 0.9)), "{want}");
            assert!(!covered(&l, pt(want + 1.0, 0.7)), "{want}");
        }
    }
}

// ------------------------------------------------- AC 14: direction

#[test]
fn an_arrow_follows_the_direction_of_the_path_for_any_angle() {
    for deg in [0.0_f64, 30.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, -60.0] {
        let (c, s) = (deg.to_radians().cos(), deg.to_radians().sin());
        let pts = [(10.0, 10.0), (10.0 + 100.0 * c, 10.0 + 100.0 * s)];
        let (_, _, l) = scene(
            &pts,
            false,
            1.0,
            M {
                start: Shape::Arrow,
                end: Shape::Arrow,
                ..NONE
            },
        );
        let end = pt(pts[1].0, pts[1].1);
        let start = pt(pts[0].0, pts[0].1);
        // End arrow: tip at end + 2 dir; half width at u is 1.5 (2 - u) / 4.
        let at = |o: Point, u: f64, v: f64| pt(o.x + c * u - s * v, o.y + s * u + c * v);
        assert!(covered(&l, at(end, 1.9, 0.0)), "{deg}: tip");
        assert!(!covered(&l, at(end, 2.15, 0.0)), "{deg}: beyond tip");
        assert!(covered(&l, at(end, 0.5, 0.5)), "{deg}");
        assert!(covered(&l, at(end, 0.5, -0.5)), "{deg}");
        assert!(!covered(&l, at(end, 0.5, 0.65)), "{deg}");
        assert!(!covered(&l, at(end, 0.5, -0.65)), "{deg}");
        // Start arrow: tip at start - 2 dir (outward).
        assert!(covered(&l, at(start, -1.9, 0.0)), "{deg}: start tip");
        assert!(
            !covered(&l, at(start, -2.15, 0.0)),
            "{deg}: start beyond tip"
        );
        assert!(covered(&l, at(start, -0.5, 0.5)), "{deg}");
        assert!(!covered(&l, at(start, -0.5, 0.65)), "{deg}");
    }
}

#[test]
fn the_arrow_on_an_inner_corner_lies_on_the_bisector() {
    // (0,0) (10,0) (10,10): arrow on (10,0) points along (1,1)/sqrt2.
    let (_, _, l) = scene(
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        false,
        0.5,
        M {
            mid: Shape::Arrow,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    let d = 1.0 / 2.0_f64.sqrt();
    let tip = |u: f64| pt(10.0 + d * u, d * u);
    // Half length is 1.0: tip at 1.0 along (1,1)/sqrt2.
    assert!(covered(&l, tip(0.95)));
    assert!(!covered(&l, tip(1.07)));
    // And not along the other candidates (+x or +y only).
    assert!(!covered(&l, pt(11.0, 0.0)));
    // Beside the tip, on the outer side of the corner, the arrow is narrow.
    let n = pt(10.0 + d * 0.8 + (-d) * 0.05, d * 0.8 + d * 0.05);
    assert!(covered(&l, n));
}

#[test]
fn a_smooth_node_gets_its_arrow_along_the_common_tangent() {
    let doc = Document::new(1);
    let a = NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0));
    let b = NewAnchor {
        id: AnchorId::new(1, 2),
        point: pt(50.0, 0.0),
        handle_in: Vec2::new(-10.0, 0.0),
        handle_out: Vec2::new(10.0, 0.0),
        kind: AnchorKind::Symmetric,
    };
    let c = NewAnchor::corner(AnchorId::new(1, 3), pt(100.0, 0.0));
    let p = doc.create_path(&[a, b, c], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Arrow,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    // Along +x: tip at x = 52; use above-line points where only the arrow is.
    assert!(covered(&l, pt(49.0, 1.1)));
    assert!(!covered(&l, pt(51.5, 1.1)));
    assert!(covered(&l, pt(50.0, 0.7)) && covered(&l, pt(50.0, -0.7)));
    assert!(!covered(&l, pt(50.0, 0.8)) && !covered(&l, pt(50.0, -0.8)));
}

#[test]
fn a_reversal_uses_the_outgoing_tangent() {
    // (0,0) -> (20,0) -> (0,0.0001... ) a hairpin: bisector undefined; outgoing
    // direction (back toward -x) is used. Draw must not panic or NaN.
    let (_, _, l) = scene(
        &[(0.0, 0.0), (20.0, 0.0), (0.0, 0.0001)],
        false,
        1.0,
        M {
            mid: Shape::Arrow,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    assert!(all_finite(&l));
    // The arrow points along -x: its tip is at x = 18.0 side. Tip region
    // (x < 18.3, above the stroke) covered, past the node on +x not.
    assert!(
        covered(&l, pt(21.5, 1.2)),
        "base side of an arrow pointing -x"
    );
    assert!(
        !covered(&l, pt(18.5, 1.2)),
        "an arrow pointing +x would cover this"
    );
}

#[test]
fn a_zero_length_handle_uses_the_direction_to_the_next_control_point() {
    let doc = Document::new(1);
    let a = NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0));
    let b = NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 30.0));
    let p = doc.create_path(&[a, b], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            end: Shape::Arrow,
            start: Shape::Arrow,
            ..NONE
        },
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    let d = (
        100.0_f64.hypot(30.0),
        100.0 / 100.0_f64.hypot(30.0),
        30.0 / 100.0_f64.hypot(30.0),
    );
    let (c, s) = (d.1, d.2);
    let tip = pt(100.0 + c * 1.9, 30.0 + s * 1.9);
    assert!(covered(&l, tip));
    assert!(!covered(&l, pt(100.0 + c * 2.2, 30.0 + s * 2.2)));
    let stip = pt(-c * 1.9, -s * 1.9);
    assert!(covered(&l, stip));
}

#[test]
fn a_curved_path_end_arrow_follows_the_incoming_tangent() {
    // Cubic to (100,0) whose last handle is (0,60): arrives travelling in -y?
    // handle_in = (0, 60) -> control point (100, 60): incoming direction is
    // (0,-1) (from y=60 down to y=0). End arrow tip at (100, -2).
    let doc = Document::new(1);
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
    let p = doc.create_path(&[a, b], false);
    apply(
        &doc,
        p,
        1.0,
        M {
            end: Shape::Arrow,
            start: Shape::Arrow,
            ..NONE
        },
    );
    let l = build_artwork(&[obj(&doc, p)], view());
    assert!(
        covered(&l, pt(100.0, -1.9)),
        "End tip along the incoming tangent"
    );
    assert!(!covered(&l, pt(100.0, -2.2)));
    // Start: outgoing tangent is +y (control point (0,60)); the Start arrow
    // points opposite, tip at (0, -2).
    assert!(covered(&l, pt(0.0, -1.9)));
    assert!(!covered(&l, pt(0.0, -2.2)));
}

// ------------------------------------------- AC 17: draw order & fill

#[test]
fn an_object_above_covers_the_markers_below_and_a_marker_covers_its_own_fill() {
    let doc = Document::new(1);
    let p = make_path(
        &doc,
        &[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)],
        true,
    );
    apply(
        &doc,
        p,
        1.0,
        M {
            mid: Shape::Dot,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    edit(
        &doc,
        p,
        &[
            StyleEdit::FillColor(Color { r: 255, g: 0, b: 0 }),
            StyleEdit::FillEnabled(true),
            StyleEdit::StrokeColor(Color { r: 0, g: 0, b: 255 }),
        ],
    );
    // The dot at (40,40) reaches 1.5 inside the fill area as well.
    let only = art(&doc, &[p]);
    let inside_fill_and_dot = composite_at(&only, pt(39.0, 39.0));
    assert!(
        inside_fill_and_dot[2] > 200.0 && inside_fill_and_dot[0] < 60.0,
        "marker (blue) is above the object's own fill: {inside_fill_and_dot:?}"
    );
    assert!(composite_at(&only, pt(30.0, 30.0))[0] > 200.0, "fill");
    // A higher opaque rectangle covers the marker; a lower one does not.
    let top = doc.create_rect(RectBounds {
        origin: pt(35.0, 35.0),
        width: mm(10.0),
        height: mm(10.0),
    });
    edit(
        &doc,
        top,
        &[
            StyleEdit::FillColor(Color { r: 0, g: 255, b: 0 }),
            StyleEdit::FillEnabled(true),
            StyleEdit::StrokeEnabled(false),
        ],
    );
    let above = art(&doc, &[p, top]);
    let px = composite_at(&above, pt(41.0, 41.0));
    assert!(
        px[1] > 200.0 && px[2] < 60.0,
        "{px:?}: higher object covers the marker"
    );
    let below = art(&doc, &[top, p]);
    let px = composite_at(&below, pt(41.0, 41.0));
    assert!(
        px[2] > 200.0 && px[1] < 60.0,
        "{px:?}: the marker is above a lower object"
    );
}

// ------------------------------------------------- AC 21: primitives

#[test]
fn a_primitive_snapshot_with_marker_settings_draws_no_marker() {
    let doc = Document::new(1);
    let r = doc.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(40.0),
        height: mm(30.0),
    });
    let plain = build_artwork(&[obj(&doc, r)], view());
    let mut snap = obj(&doc, r);
    match &mut snap {
        ObjectSnapshot::Primitive(p) => {
            p.style.stroke.markers = Markers {
                start: Shape::Arrow,
                mid: Shape::Dot,
                end: Shape::Arrow,
                mid_place: MarkerPlace::AtNodes,
                mid_count: MarkerCount::new(5).unwrap(),
            };
        }
        ObjectSnapshot::Path(_) => panic!(),
    }
    let marked = build_artwork(&[snap], view());
    assert_eq!(plain, marked);
}

// ---------------------------------------------- AC 29: counts and budgets

#[test]
fn at_most_500_markers_per_slot_are_drawn() {
    let m = |n| M {
        mid: Shape::Dot,
        count: n,
        ..NONE
    };
    let d = one_dot(0.5);
    let n500 = marker_tris(&LINE, false, 0.5, m(500));
    assert_eq!(n500, 500 * d);
    for n in [501, 1000, 100_000, u32::MAX] {
        assert_eq!(marker_tris(&LINE, false, 0.5, m(n)), n500, "n={n}");
    }
    let n499 = marker_tris(&LINE, false, 0.5, m(499));
    assert_eq!(n499, 499 * d);
}

#[test]
fn a_frame_draws_at_most_50_000_markers_but_every_stroke() {
    let d = one_dot(0.5);
    let doc = Document::new(1);
    let mut ids = Vec::new();
    for i in 0..160 {
        let y = f64::from(i) * 5.0;
        let p = make_path(&doc, &[(0.0, y), (100.0, y)], false);
        apply(
            &doc,
            p,
            0.5,
            M {
                mid: Shape::Dot,
                count: 500,
                ..NONE
            },
        );
        ids.push(p);
    }
    let marked = art(&doc, &ids);
    for p in &ids {
        edit(&doc, *p, &[StyleEdit::MarkerMid(Shape::None)]);
    }
    let plain = art(&doc, &ids);
    let added = tri(&marked) - tri(&plain);
    // 160 * 500 = 80 000 requested.
    assert!(added <= 50_000 * d, "{} dots", added / d);
    assert!(added >= 49_000 * d, "budget reached: {} dots", added / d);
    // Every stroke is still there.
    assert_eq!(marked.layers().len(), plain.layers().len());
    for i in 0..160 {
        assert!(covered(&marked, pt(33.0, f64::from(i) * 5.0)), "stroke {i}");
    }
}

// --------------------------------------------- huge / tiny widths

#[test]
fn huge_and_tiny_widths_stay_finite() {
    for w in [0.0001, 0.001, 50.0, 500.0, 1.0e4, 1.0e6, 1.0e9] {
        for pts in [LINE.to_vec(), SQUARE.to_vec()] {
            let closed = pts.len() == 4;
            let (_, _, l) = scene(
                &pts,
                closed,
                w,
                M {
                    start: Shape::Arrow,
                    mid: Shape::Dot,
                    end: Shape::Arrow,
                    count: 20,
                    ..NONE
                },
            );
            assert!(all_finite(&l), "w={w}");
        }
    }
}

#[test]
fn a_very_short_open_path_with_a_wide_stroke_still_draws_an_arrow() {
    let (_, _, l) = scene(
        &[(0.0, 0.0), (0.5, 0.0)],
        false,
        5.0,
        M {
            end: Shape::Arrow,
            ..NONE
        },
    );
    assert!(all_finite(&l));
    assert!(covered(&l, pt(0.5 + 9.0, 0.0)), "tip at 0.5 + 10");
    assert!(!covered(&l, pt(0.5 + 10.6, 0.0)));
}

#[test]
fn a_large_coordinate_path_is_finite() {
    let (_, _, l) = scene(
        &[(-1.0e6, -1.0e6), (1.0e6, 1.0e6)],
        false,
        1.0,
        M {
            start: Shape::Arrow,
            mid: Shape::Dot,
            end: Shape::Arrow,
            count: 9,
            ..NONE
        },
    );
    assert!(all_finite(&l));
}

// ------------------------------------------------ compound paths

#[test]
fn compound_paths_get_markers_per_outline() {
    let doc = unpack(
        1,
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("curvyo-document-core/tests/fixtures/compound_v8.curvyo"),
        )
        .unwrap(),
    )
    .unwrap();
    let compounds: Vec<NodeId> = doc
        .object_ids()
        .into_iter()
        .filter(|id| matches!(doc.object(*id).unwrap(), ObjectSnapshot::Path(p) if p.is_compound()))
        .collect();
    assert!(!compounds.is_empty(), "the fixture has a compound path");
    let id = compounds[0];
    let (nodes_total, subpaths, first_closed) = match doc.object(id).unwrap() {
        ObjectSnapshot::Path(p) => {
            let n: usize = p.subpaths().map(|s| s.anchors.len()).sum();
            (n, p.subpaths().count(), p.closed)
        }
        ObjectSnapshot::Primitive(_) => unreachable!(),
    };
    assert!(subpaths >= 2);
    // Calibrate a dot at this stroke width.
    let w = 0.5;
    edit(&doc, id, &[StyleEdit::StrokeWidth(mm(w))]);
    let plain = art(&doc, &[id]);
    edit(
        &doc,
        id,
        &[
            StyleEdit::MarkerMid(Shape::Dot),
            StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        ],
    );
    let marked = art(&doc, &[id]);
    assert!(all_finite(&marked));
    assert!(
        tri(&marked) > tri(&plain),
        "markers drawn on a compound path"
    );
    let d = one_dot(w);
    let added = tri(&marked) - tri(&plain);
    assert_eq!(added % d, 0);
    let dots = added / d;
    println!(
        "compound: nodes_total={nodes_total} subpaths={subpaths} first_closed={first_closed} dots={dots}"
    );
    if first_closed {
        // Spec is silent for compounds; per outline means every node of every
        // closed outline carries a marker.
        assert_eq!(dots, nodes_total, "At nodes on closed outlines: every node");
    }
    // Start / End never draw on closed outlines.
    edit(
        &doc,
        id,
        &[
            StyleEdit::MarkerMid(Shape::None),
            StyleEdit::MarkerStart(Shape::Arrow),
            StyleEdit::MarkerEnd(Shape::Arrow),
        ],
    );
    let sa = art(&doc, &[id]);
    println!(
        "compound start/end: extra triangles = {}",
        tri(&sa) - tri(&plain)
    );
    if first_closed {
        assert_eq!(tri(&sa), tri(&plain));
    }
}

// ------------------------------------------------- degenerate directions

/// DEFECT (criterion 14, "a tangent with a zero-length handle uses the
/// direction to the next control point or node that differs from the
/// anchor"): when the node next to an end (or to an inner node) coincides with
/// it, the marker is dropped instead of looking further along the path. Found
/// by the tester; the test stays ignored until the implementer fixes it.
#[test]
#[ignore = "defect: a coincident neighbour node drops the Start/End/At nodes marker (criterion 14)"]
fn duplicate_nodes_take_the_direction_to_the_next_differing_point() {
    // Start on a doubled first node: outgoing tangent toward (10, 0); the
    // Start arrow points outward, tip at x = -2.
    let (_, _, l) = scene(
        &[(0.0, 0.0), (0.0, 0.0), (10.0, 0.0)],
        false,
        1.0,
        M {
            start: Shape::Arrow,
            ..NONE
        },
    );
    assert!(all_finite(&l));
    assert!(covered(&l, pt(-1.9, 0.0)), "tip left of the start");
    assert!(!covered(&l, pt(-2.2, 0.0)));
    assert!(covered(&l, pt(-1.9, 0.0)) && !covered(&l, pt(2.2, 1.2)));
    // End on a doubled last node: incoming tangent from (0, 0).
    let (_, _, l) = scene(
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.0)],
        false,
        1.0,
        M {
            end: Shape::Arrow,
            ..NONE
        },
    );
    assert!(covered(&l, pt(11.9, 0.0)), "tip right of the end");
    assert!(!covered(&l, pt(12.2, 0.0)));
    // At nodes on a doubled inner node: both arrows point along +x.
    let (_, _, l) = scene(
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (20.0, 0.0)],
        false,
        1.0,
        M {
            mid: Shape::Arrow,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    assert!(all_finite(&l));
    assert!(covered(&l, pt(8.2, 1.2)), "wide base on the left");
    assert!(!covered(&l, pt(11.8, 1.2)), "narrow tip on the right");
    // Three coincident inner nodes carry three arrows.
    let pts = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 0.0),
        (10.0, 0.0),
        (20.0, 0.0),
    ];
    let with = marker_tris(
        &pts,
        false,
        1.0,
        M {
            mid: Shape::Arrow,
            place: MarkerPlace::AtNodes,
            ..NONE
        },
    );
    assert_eq!(with, 3, "one arrow triangle per inner node");
}

#[test]
fn a_closed_path_with_a_doubled_closing_node_still_spaces_by_length() {
    let pts = [
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 40.0),
        (0.0, 40.0),
        (0.0, 0.0),
    ];
    let (_, _, l) = scene(
        &pts,
        true,
        1.0,
        M {
            mid: Shape::Dot,
            count: 4,
            ..NONE
        },
    );
    assert!(all_finite(&l));
    for (cx, cy, ox, oy) in [
        (0.0, 0.0, -1.0, -1.0),
        (40.0, 0.0, 1.0, -1.0),
        (40.0, 40.0, 1.0, 1.0),
        (0.0, 40.0, -1.0, 1.0),
    ] {
        assert!(corner_has_dot(&l, cx, cy, ox, oy), "corner ({cx}, {cy})");
    }
}

#[test]
fn a_cusp_and_a_hairpin_curve_stay_finite_for_every_slot() {
    let doc = Document::new(1);
    let a = NewAnchor {
        id: AnchorId::new(1, 1),
        point: pt(0.0, 0.0),
        handle_in: Vec2::new(0.0, 0.0),
        handle_out: Vec2::new(50.0, 50.0),
        kind: AnchorKind::Corner,
    };
    let b = NewAnchor {
        id: AnchorId::new(1, 2),
        point: pt(10.0, 0.0),
        handle_in: Vec2::new(40.0, 50.0),
        handle_out: Vec2::new(-30.0, -30.0),
        kind: AnchorKind::Corner,
    };
    let c = NewAnchor::corner(AnchorId::new(1, 3), pt(0.0, 0.0));
    let p = doc.create_path(&[a, b, c], false);
    for place in [MarkerPlace::Spaced, MarkerPlace::AtNodes] {
        apply(
            &doc,
            p,
            1.0,
            M {
                start: Shape::Arrow,
                mid: Shape::Arrow,
                end: Shape::Arrow,
                count: 25,
                place,
            },
        );
        let l = build_artwork(&[obj(&doc, p)], view());
        assert!(all_finite(&l));
        assert!(tri(&l) > 0);
    }
}

#[test]
fn the_length_threshold_for_markers_is_a_thousandth_of_a_millimetre() {
    let m = M {
        end: Shape::Arrow,
        ..NONE
    };
    // 0.0005 mm: no marker; 0.002 mm and 1 mm: a marker.
    for (len, drawn) in [(0.0005, false), (0.002, true), (1.0, true)] {
        let n = marker_tris(&[(0.0, 0.0), (len, 0.0)], false, 1.0, m);
        assert_eq!(n, usize::from(drawn), "length {len}");
    }
}
