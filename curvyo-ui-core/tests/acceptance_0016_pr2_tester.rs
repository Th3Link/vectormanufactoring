//! Independent acceptance tests for `0016-boolean-operations` PR 2 in
//! `curvyo-ui-core`: hit-testing (33), selection box and bounds (34),
//! marquee and lasso picking, and the geometry helper behind them.
//! Written from the specification before the implementation was read.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::assert_is_empty,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::type_complexity
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, Color, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point,
    RectBounds, StyleEdit, Tolerance, Vec2,
};
use curvyo_geometry_core::{Outline, contains_point_in_outlines};
use curvyo_ui_core::{
    MarqueeMode, content_bounds, hit_test_object, hit_test_objects, hit_test_objects_along,
    object_bounds, object_outline_bounds, objects_in_marquee, oriented_bounds,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

const TOL: Tolerance = Tolerance::from_mm(0.5);

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
            .map(|(i, p)| NewAnchor::corner(AnchorId::new(30, start + i as u64), p))
            .collect(),
        true,
    )
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn solid_fill(d: &Document, id: NodeId, on: bool) {
    d.edit_style(&[id], &StyleEdit::FillEnabled(on)).unwrap();
    if on {
        d.edit_style(&[id], &StyleEdit::FillColor(Color { r: 9, g: 9, b: 9 }))
            .unwrap();
    }
}

fn compound(d: &Document, outlines: &[(Vec<NewAnchor>, bool)], fill: bool) -> NodeId {
    let seed = rect(d, 900.0, 900.0, 1.0, 1.0);
    let id = d
        .replace_with_path(&[seed], seed, outlines, "boolean_union")
        .unwrap();
    solid_fill(d, id, fill);
    id
}

fn ring_outlines() -> Vec<(Vec<NewAnchor>, bool)> {
    vec![
        square(0, 0.0, 0.0, 40.0, true),
        square(10, 15.0, 15.0, 10.0, false),
    ]
}

fn objects(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .map(|i| d.object(i).unwrap())
        .collect()
}

// ---- 33: hit testing -------------------------------------------------

#[test]
fn ac33_inside_a_hole_the_object_is_not_picked_what_is_behind_is() {
    let d = Document::new(1);
    let behind = rect(&d, 10.0, 10.0, 20.0, 20.0);
    solid_fill(&d, behind, true);
    let ring = compound(&d, &ring_outlines(), true);
    let objs = objects(&d);
    // Hole centre: the rectangle behind wins.
    assert_eq!(hit_test_object(&objs, pt(20.0, 20.0), TOL), Some(behind));
    // On the ring body, the ring (above) wins.
    assert_eq!(hit_test_object(&objs, pt(7.0, 20.0), TOL), Some(ring));
    // hit_test_objects (cycling list) must not list the ring first in the hole.
    let list = hit_test_objects(&objs, pt(20.0, 20.0), TOL);
    assert_eq!(list.first(), Some(&behind));
}

#[test]
fn ac33_inside_a_hole_with_nothing_behind_nothing_is_picked() {
    let d = Document::new(1);
    compound(&d, &ring_outlines(), true);
    let objs = objects(&d);
    assert_eq!(hit_test_object(&objs, pt(20.0, 20.0), TOL), None);
    assert_eq!(
        hit_test_objects(&objs, pt(20.0, 20.0), TOL),
        Vec::<NodeId>::new()
    );
    // Just inside the hole, farther than tolerance from its edge.
    assert_eq!(hit_test_object(&objs, pt(17.0, 17.0), TOL), None);
}

#[test]
fn ac33_fill_between_the_outlines_picks_the_object() {
    let d = Document::new(1);
    let ring = compound(&d, &ring_outlines(), true);
    let objs = objects(&d);
    for p in [
        pt(5.0, 5.0),
        pt(7.0, 20.0),
        pt(20.0, 33.0),
        pt(32.0, 32.0),
        pt(1.0, 39.0),
    ] {
        assert_eq!(hit_test_object(&objs, p, TOL), Some(ring), "{p:?}");
    }
    assert_eq!(hit_test_object(&objs, pt(-5.0, 20.0), TOL), None);
    assert_eq!(hit_test_object(&objs, pt(45.0, 20.0), TOL), None);
}

#[test]
fn ac33_stroke_of_every_outline_picks_the_object_even_without_fill() {
    let d = Document::new(1);
    let ring = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 15.0, 15.0, 10.0, false),
            square(20, 100.0, 0.0, 10.0, true),
            square(30, 200.0, 0.0, 10.0, false),
        ],
        false,
    );
    let objs = objects(&d);
    // On each outline's edge, including the third and fourth (far away).
    for p in [
        pt(20.0, 0.0),
        pt(0.0, 20.0),
        pt(20.0, 15.0),
        pt(25.0, 20.0),
        pt(105.0, 0.0),
        pt(110.0, 5.0),
        pt(205.0, 10.0),
        pt(200.0, 5.0),
    ] {
        assert_eq!(hit_test_object(&objs, p, TOL), Some(ring), "{p:?}");
    }
    // Within tolerance but off the line.
    assert_eq!(hit_test_object(&objs, pt(20.0, 15.3), TOL), Some(ring));
    // Between outlines with no fill: nothing.
    assert_eq!(hit_test_object(&objs, pt(7.0, 20.0), TOL), None);
    assert_eq!(hit_test_object(&objs, pt(20.0, 20.0), TOL), None);
    assert_eq!(hit_test_object(&objs, pt(150.0, 0.0), TOL), None);
}

#[test]
fn ac33_hole_hit_parity_with_an_ordinary_path_when_fill_is_off() {
    // An ordinary closed path with Fill None is picked on its outline only;
    // the compound path must behave identically for the ring body.
    let d = Document::new(1);
    // Anchor ids are unique over the document: the ring below uses the low ones.
    let ordinary = d.create_path(&square(5000, 0.0, 0.0, 40.0, true).0, true);
    solid_fill(&d, ordinary, false);
    let ring = compound(&d, &ring_outlines(), false);
    let objs = objects(&d);
    let a = hit_test_object(&objs[..1], pt(7.0, 20.0), TOL);
    let b = hit_test_object(&objs[1..], pt(7.0, 20.0), TOL);
    assert_eq!(a.is_some(), b.is_some());
    let _ = (ordinary, ring);
}

#[test]
fn ac33_a_point_exactly_on_the_hole_edge_is_stroke_picked_not_lost() {
    let d = Document::new(1);
    let ring = compound(&d, &ring_outlines(), true);
    let objs = objects(&d);
    for p in [
        pt(15.0, 20.0),
        pt(25.0, 20.0),
        pt(20.0, 15.0),
        pt(20.0, 25.0),
    ] {
        assert_eq!(hit_test_object(&objs, p, TOL), Some(ring), "{p:?}");
    }
    // Just inside the hole but within the pick tolerance of its edge.
    assert_eq!(hit_test_object(&objs, pt(15.3, 20.0), TOL), Some(ring));
}

#[test]
fn ac33_island_inside_a_hole_is_picked_and_the_gap_around_it_is_not() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 10.0, 10.0, 20.0, false),
            square(20, 18.0, 18.0, 4.0, true),
        ],
        true,
    );
    let objs = objects(&d);
    assert_eq!(hit_test_object(&objs, pt(20.0, 20.0), TOL), Some(c));
    assert_eq!(hit_test_object(&objs, pt(13.0, 13.0), TOL), None);
    assert_eq!(hit_test_object(&objs, pt(5.0, 5.0), TOL), Some(c));
}

#[test]
fn ac33_hole_wound_like_its_outer_is_not_a_hole_under_nonzero() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 15.0, 15.0, 10.0, true),
        ],
        true,
    );
    let objs = objects(&d);
    assert_eq!(hit_test_object(&objs, pt(20.0, 20.0), TOL), Some(c));
}

#[test]
fn ac33_geometry_helper_sums_winding_over_all_outlines() {
    let ring = ring_outlines();
    let triples: Vec<Vec<_>> = ring
        .iter()
        .map(|(a, _)| {
            a.iter()
                .map(|n| (n.point, n.handle_in, n.handle_out))
                .collect()
        })
        .collect();
    let outlines: Vec<Outline<'_>> = triples
        .iter()
        .zip(&ring)
        .map(|(t, (_, closed))| Outline::new(t, *closed))
        .collect();
    assert!(contains_point_in_outlines(&outlines, pt(5.0, 5.0)));
    assert!(!contains_point_in_outlines(&outlines, pt(20.0, 20.0)));
    assert!(!contains_point_in_outlines(&outlines, pt(50.0, 5.0)));
    assert!(!contains_point_in_outlines(&[], pt(0.0, 0.0)));
    // One outline in the list behaves as contains_point.
    assert!(contains_point_in_outlines(&outlines[..1], pt(20.0, 20.0)));
    // Winding exactly on a shared vertex of the hole must not panic.
    let _ = contains_point_in_outlines(&outlines, pt(15.0, 15.0));
}

#[test]
fn ac33_curved_hole_hit_follows_the_curve_not_the_chord() {
    // Outer square, hole is a circle-ish 4-segment Bezier (radius 10 around 20,20).
    let k = 0.552_284_749_8 * 10.0;
    let c = 10.0;
    let hole: Vec<NewAnchor> = vec![
        (pt(20.0, 20.0 - c), Vec2::new(-k, 0.0), Vec2::new(k, 0.0)),
        (pt(20.0 + c, 20.0), Vec2::new(0.0, -k), Vec2::new(0.0, k)),
        (pt(20.0, 20.0 + c), Vec2::new(k, 0.0), Vec2::new(-k, 0.0)),
        (pt(20.0 - c, 20.0), Vec2::new(0.0, k), Vec2::new(0.0, -k)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (point, handle_in, handle_out))| NewAnchor {
        id: AnchorId::new(31, i as u64),
        point,
        handle_in,
        handle_out,
        kind: AnchorKind::Symmetric,
    })
    .collect();
    let hole: Vec<NewAnchor> = hole
        .into_iter()
        .rev()
        .map(|a| NewAnchor {
            handle_in: a.handle_out,
            handle_out: a.handle_in,
            ..a
        })
        .collect();
    let d = Document::new(1);
    let id = compound(&d, &[square(0, 0.0, 0.0, 40.0, true), (hole, true)], true);
    let objs = objects(&d);
    // 45 degree point on the circle is at radius 10: (27.07, 27.07). A point at
    // radius 9 is in the hole (not picked); at radius 11 on the fill (picked).
    let r9 = 9.0 / 2f64.sqrt();
    let r11 = 11.0 / 2f64.sqrt();
    assert_eq!(hit_test_object(&objs, pt(20.0 + r9, 20.0 + r9), TOL), None);
    assert_eq!(
        hit_test_object(&objs, pt(20.0 + r11, 20.0 + r11), TOL),
        Some(id)
    );
}

#[test]
fn ac33_hit_testing_along_a_line_touches_a_compound_path_by_any_outline() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            square(10, 100.0, 0.0, 10.0, true),
        ],
        false,
    );
    let objs = objects(&d);
    // A line that only crosses the far outline.
    let hits = hit_test_objects_along(&objs, &[pt(105.0, -5.0), pt(105.0, 15.0)], TOL);
    assert_eq!(hits, vec![c]);
    // A line through the empty gap between pieces touches nothing.
    let hits = hit_test_objects_along(&objs, &[pt(50.0, -5.0), pt(50.0, 15.0)], TOL);
    assert_eq!(hits, Vec::<NodeId>::new());
}

// ---- 34: bounds ---------------------------------------------------------

#[test]
fn ac34_selection_box_is_the_bounding_box_of_all_outlines() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 10.0, 20.0, 5.0, true),
            square(10, -30.0, 70.0, 4.0, false),
            square(20, 200.0, -15.0, 6.0, true),
        ],
        true,
    );
    let o = d.object(c).unwrap();
    let (lo, hi) = object_bounds(&o);
    assert_eq!((lo, hi), (pt(-30.0, -15.0), pt(206.0, 74.0)));
    let (lo, hi) = object_outline_bounds(&o);
    assert_eq!((lo, hi), (pt(-30.0, -15.0), pt(206.0, 74.0)));
    let b = oriented_bounds(&o);
    assert_eq!((b.min, b.max), (pt(-30.0, -15.0), pt(206.0, 74.0)));
    assert_eq!(b.angle.as_radians(), 0.0);
    let (clo, chi) = content_bounds(&d).unwrap();
    assert!(clo.x <= -30.0 && clo.y <= -15.0 && chi.x >= 206.0 && chi.y >= 74.0);
}

#[test]
fn ac34_bounds_follow_curve_extremes_of_extra_outlines_not_just_anchors() {
    let d = Document::new(1);
    let bulge = vec![
        NewAnchor {
            id: AnchorId::new(32, 0),
            point: pt(100.0, 0.0),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(0.0, -30.0),
            kind: AnchorKind::Asymmetric,
        },
        NewAnchor {
            id: AnchorId::new(32, 1),
            point: pt(120.0, 0.0),
            handle_in: Vec2::new(0.0, -30.0),
            handle_out: Vec2::ZERO,
            kind: AnchorKind::Asymmetric,
        },
        NewAnchor::corner(AnchorId::new(32, 2), pt(110.0, 10.0)),
    ];
    let c = compound(&d, &[square(0, 0.0, 0.0, 10.0, true), (bulge, true)], true);
    let o = d.object(c).unwrap();
    let (lo, _hi) = object_outline_bounds(&o);
    // Cubic from (100,0) with controls at y=-30 reaches y = -22.5 at t = .5.
    assert!(
        lo.y < -20.0,
        "curve extreme of the extra outline missed: {lo:?}"
    );
}

#[test]
fn ac34_rotated_compound_has_an_oriented_box_that_holds_every_anchor() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 150.0, true),
            square(10, 100.0, 40.0, 10.0, false),
        ],
        true,
    );
    let rotated = d
        .object(c)
        .unwrap()
        .rotated(pt(50.0, 20.0), Angle::from_radians(0.7));
    d.rotate_object(&rotated).unwrap();
    let o = d.object(c).unwrap();
    let b = oriented_bounds(&o);
    assert!((b.angle.as_radians() - 0.7).abs() < 1e-9);
    let ObjectSnapshot::Path(p) = &o else {
        panic!()
    };
    for a in p.all_anchors() {
        let l = b.to_local(a.point);
        assert!(
            l.x >= b.min.x - 1e-6
                && l.x <= b.max.x + 1e-6
                && l.y >= b.min.y - 1e-6
                && l.y <= b.max.y + 1e-6,
            "anchor {:?} outside the oriented box",
            a.point
        );
    }
    // The box is tight: some anchor touches each side.
    let ls: Vec<Point> = p.all_anchors().map(|a| b.to_local(a.point)).collect();
    assert!(ls.iter().any(|l| (l.x - b.min.x).abs() < 1e-6));
    assert!(ls.iter().any(|l| (l.x - b.max.x).abs() < 1e-6));
    assert!(ls.iter().any(|l| (l.y - b.min.y).abs() < 1e-6));
    assert!(ls.iter().any(|l| (l.y - b.max.y).abs() < 1e-6));
    // Hit testing still honours the hole after the rotation.
    let objs = objects(&d);
    let hole_centre = rotated_point(pt(105.0, 45.0), pt(50.0, 20.0), 0.7);
    assert_eq!(hit_test_object(&objs, hole_centre, TOL), None);
    let body = rotated_point(pt(5.0, 5.0), pt(50.0, 20.0), 0.7);
    assert_eq!(hit_test_object(&objs, body, TOL), Some(c));
}

fn rotated_point(p: Point, pivot: Point, a: f64) -> Point {
    let (s, c) = a.sin_cos();
    let (dx, dy) = (p.x - pivot.x, p.y - pivot.y);
    pt(pivot.x + dx * c - dy * s, pivot.y + dx * s + dy * c)
}

#[test]
fn ac34_marquee_uses_the_box_of_all_outlines() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            square(10, 100.0, 0.0, 10.0, true),
        ],
        false,
    );
    let objs = objects(&d);
    // Contain: a marquee around only the first outline does not contain the object.
    let contain = objects_in_marquee(
        &objs,
        pt(-1.0, -1.0),
        pt(11.0, 11.0),
        MarqueeMode::Contain,
        TOL,
    );
    assert_eq!(contain, Vec::<NodeId>::new());
    let contain = objects_in_marquee(
        &objs,
        pt(-1.0, -1.0),
        pt(111.0, 11.0),
        MarqueeMode::Contain,
        TOL,
    );
    assert_eq!(contain, vec![c]);
    // Touch: the far outline's side is enough.
    let touch = objects_in_marquee(
        &objs,
        pt(99.0, 3.0),
        pt(120.0, 8.0),
        MarqueeMode::Touch,
        TOL,
    );
    assert_eq!(touch, vec![c]);
}

#[test]
fn ac34_degenerate_compound_with_empty_first_outline_does_not_panic() {
    let d = Document::new(1);
    let c = compound(
        &d,
        &[(
            vec![NewAnchor::corner(AnchorId::new(33, 0), pt(7.0, 7.0))],
            true,
        )],
        true,
    );
    let o = d.object(c).unwrap();
    let (lo, hi) = object_bounds(&o);
    assert_eq!((lo, hi), (pt(7.0, 7.0), pt(7.0, 7.0)));
    let objs = objects(&d);
    let _ = hit_test_object(&objs, pt(7.0, 7.0), TOL);
    let _ = hit_test_object(&objs, pt(100.0, 7.0), TOL);
}

#[test]
fn ac33_a_thousand_outlines_hit_test_stays_fast_and_correct() {
    let d = Document::new(1);
    let outlines: Vec<_> = (0..1000)
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
    let c = compound(&d, &outlines, true);
    let objs = objects(&d);
    let start = std::time::Instant::now();
    for i in 0..200 {
        let x = (i % 40) as f64 * 3.0 + 1.0;
        let y = (i / 40) as f64 * 3.0 + 1.0;
        assert_eq!(hit_test_object(&objs, pt(x, y), TOL), Some(c));
    }
    // Gap between squares: further than tolerance from every edge.
    assert_eq!(
        hit_test_object(&objs, pt(2.5, 1.0), Tolerance::from_mm(0.1)),
        None
    );
    println!("200 hit tests over 1000 outlines: {:?}", start.elapsed());
}
