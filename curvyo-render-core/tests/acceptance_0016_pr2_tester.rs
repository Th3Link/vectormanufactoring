//! Independent acceptance tests for `0016-boolean-operations` PR 2, rendering
//! of compound paths: criteria 31 (fill with holes, nonzero), 31a (dash restarts
//! per outline), 32 (stroke on every outline), 38b (no marker exists to draw).
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
    AnchorId, Color, DashPattern, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point,
    RectBounds, StyleEdit, ViewTransform,
};
use curvyo_render_core::{DrawList, build_artwork, build_live_edit_preview};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn view() -> ViewTransform {
    ViewTransform::new(4.0, pt(0.0, 0.0))
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
            .map(|(i, p)| NewAnchor::corner(AnchorId::new(40, start + i as u64), p))
            .collect(),
        true,
    )
}

/// A compound path of the given outlines; fill red and/or stroke as asked.
fn compound(
    document: &Document,
    outlines: &[(Vec<NewAnchor>, bool)],
    fill: bool,
    stroke_width: f64,
) -> NodeId {
    let seed = document.create_rect(RectBounds {
        origin: pt(500.0, 500.0),
        width: mm(1.0),
        height: mm(1.0),
    });
    let id = document
        .replace_with_path(&[seed], seed, outlines, "boolean_union")
        .unwrap();
    if fill {
        document
            .edit_style(&[id], &StyleEdit::FillColor(Color { r: 255, g: 0, b: 0 }))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::FillEnabled(true))
            .unwrap();
    } else {
        document
            .edit_style(&[id], &StyleEdit::FillEnabled(false))
            .unwrap();
    }
    if stroke_width > 0.0 {
        document
            .edit_style(&[id], &StyleEdit::StrokeWidth(mm(stroke_width)))
            .unwrap();
    } else {
        document
            .edit_style(&[id], &StyleEdit::StrokeEnabled(false))
            .unwrap();
    }
    id
}

fn artwork(document: &Document) -> DrawList {
    let objects: Vec<ObjectSnapshot> = document
        .object_ids()
        .into_iter()
        .map(|id| document.object(id).unwrap())
        .collect();
    build_artwork(&objects, view())
}

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let s = |p1: Point, p2: Point, p3: Point| {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = s(p, a, b);
    let d2 = s(p, b, c);
    let d3 = s(p, c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn covered(list: &DrawList, p: Point) -> bool {
    list.triangles
        .chunks(3)
        .any(|t| t.len() == 3 && in_triangle(p, t[0].position, t[1].position, t[2].position))
}

fn ring(d: &Document, fill: bool, stroke: f64) -> NodeId {
    compound(
        d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 15.0, 15.0, 10.0, false),
        ],
        fill,
        stroke,
    )
}

#[test]
fn ac31_a_filled_ring_paints_the_ring_and_not_the_hole() {
    let d = Document::new(1);
    ring(&d, true, 0.0);
    let list = artwork(&d);
    assert!(list.triangle_count() > 0);
    assert!(!covered(&list, pt(20.0, 20.0)), "hole centre is painted");
    assert!(
        !covered(&list, pt(16.0, 16.0)),
        "hole corner area is painted"
    );
    assert!(!covered(&list, pt(24.0, 24.0)));
    assert!(covered(&list, pt(5.0, 5.0)), "ring body is not painted");
    assert!(covered(&list, pt(20.0, 8.0)));
    assert!(covered(&list, pt(35.0, 20.0)));
    assert!(covered(&list, pt(20.0, 30.0)));
    assert!(!covered(&list, pt(-1.0, 20.0)), "paint outside the outer");
    assert!(!covered(&list, pt(41.0, 20.0)));
}

#[test]
fn ac31_nonzero_rule_a_hole_wound_the_same_way_as_its_outer_is_filled() {
    let d = Document::new(1);
    compound(
        &d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 15.0, 15.0, 10.0, true),
        ],
        true,
        0.0,
    );
    let list = artwork(&d);
    assert!(
        covered(&list, pt(20.0, 20.0)),
        "same winding means no hole under nonzero (winding 2)"
    );
}

#[test]
fn ac31_island_inside_a_hole_is_painted_and_two_separate_pieces_both_paint() {
    let d = Document::new(1);
    compound(
        &d,
        &[
            square(0, 0.0, 0.0, 40.0, true),
            square(10, 10.0, 10.0, 20.0, false),
            square(20, 18.0, 18.0, 4.0, true),
            square(30, 100.0, 0.0, 10.0, true),
        ],
        true,
        0.0,
    );
    let list = artwork(&d);
    assert!(covered(&list, pt(20.0, 20.0)), "island in the hole");
    assert!(
        !covered(&list, pt(12.0, 12.0)),
        "hole between ring and island"
    );
    assert!(covered(&list, pt(3.0, 3.0)));
    assert!(covered(&list, pt(105.0, 5.0)), "second piece");
    assert!(!covered(&list, pt(70.0, 5.0)), "gap between pieces");
}

#[test]
fn ac32_stroke_on_every_outline_and_not_between() {
    let d = Document::new(1);
    ring(&d, false, 2.0);
    let list = artwork(&d);
    // Outer edges, hole edges.
    for p in [
        pt(20.0, 0.0),
        pt(40.0, 20.0),
        pt(20.0, 40.0),
        pt(0.0, 20.0),
        pt(20.0, 15.0),
        pt(25.0, 20.0),
        pt(20.0, 25.0),
        pt(15.0, 20.0),
    ] {
        assert!(covered(&list, p), "stroke missing at {p:?}");
    }
    assert!(!covered(&list, pt(20.0, 20.0)), "no fill, centre empty");
    assert!(!covered(&list, pt(20.0, 8.0)), "no fill, ring body empty");
    assert!(!covered(&list, pt(20.0, 10.0)));
}

#[test]
fn ac32_stroke_of_a_third_and_fourth_outline_is_drawn_too() {
    let d = Document::new(1);
    compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            square(10, 50.0, 0.0, 10.0, true),
            square(20, 100.0, 0.0, 10.0, true),
            square(30, 150.0, 0.0, 10.0, false),
        ],
        false,
        1.0,
    );
    let list = artwork(&d);
    for x in [0.0, 50.0, 100.0, 150.0] {
        assert!(covered(&list, pt(x, 5.0)), "outline at x={x} not stroked");
        assert!(covered(&list, pt(x + 10.0, 5.0)));
    }
}

fn dashed(d: &Document, id: NodeId) {
    // Width 1.5 mm; dash 3 and gap 2 widths: 4.5 mm on, 3 mm off, period 7.5.
    d.edit_style(&[id], &StyleEdit::StrokeWidth(mm(1.5)))
        .unwrap();
    d.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![3.0, 2.0]).unwrap()),
    )
    .unwrap();
}

#[test]
fn ac31a_the_dash_pattern_starts_afresh_at_the_first_node_of_each_outline() {
    let d = Document::new(1);
    let id = ring(&d, false, 1.5);
    dashed(&d, id);
    let list = artwork(&d);
    // Outer outline: first node (0,0), going right. Dash 0..4.5, gap 4.5..7.5.
    assert!(covered(&list, pt(2.0, 0.0)));
    assert!(covered(&list, pt(4.0, 0.0)));
    assert!(
        !covered(&list, pt(6.0, 0.0)),
        "gap expected on the outer outline"
    );
    assert!(covered(&list, pt(9.0, 0.0)));
    // Hole outline: first node (15,25) (corner order reversed), first segment
    // runs right along y = 25. Restarted: dash 0..4.5 mm (x 15..19.5), gap
    // x 19.5..22.5, dash from 22.5. The outer outline is 160 mm = 21 periods
    // + 2.5 mm, so a pattern that continued would be 2.5 mm into its period at
    // the hole's first node and would have a gap 2..5 mm along (x 17..20).
    assert!(covered(&list, pt(16.0, 25.0)), "dash at 1 mm");
    assert!(
        covered(&list, pt(19.0, 25.0)),
        "dash should still run 4 mm into the hole outline (pattern did not restart)"
    );
    assert!(!covered(&list, pt(21.0, 25.0)), "gap 6 mm in");
    assert!(covered(&list, pt(24.0, 25.0)), "next dash from 7.5 mm");
}

#[test]
fn ac31a_equal_outlines_get_identical_dashing() {
    let d = Document::new(1);
    let id = compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            square(10, 50.0, 0.0, 10.0, true),
        ],
        false,
        1.5,
    );
    dashed(&d, id);
    let list = artwork(&d);
    // Same shape translated by 50 mm: every probe along the first matches the second.
    let mut mismatches = 0;
    let mut y = 0.0;
    while y <= 0.0 + 1e-9 {
        let mut x = 0.2;
        while x < 10.0 {
            let a = covered(&list, pt(x, y));
            let b = covered(&list, pt(50.0 + x, y));
            if a != b {
                mismatches += 1;
            }
            x += 0.25;
        }
        y += 1.0;
    }
    assert_eq!(mismatches, 0, "second outline dashes differently");
}

#[test]
fn ac31_32_fill_and_stroke_together_do_not_fill_the_hole() {
    let d = Document::new(1);
    ring(&d, true, 1.0);
    let list = artwork(&d);
    assert!(!covered(&list, pt(20.0, 20.0)));
    assert!(covered(&list, pt(7.0, 20.0)));
    assert!(covered(&list, pt(15.0, 20.0)), "hole edge stroke");
}

#[test]
fn ac31_many_outlines_render_without_panic() {
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
    compound(&d, &outlines, true, 0.5);
    let list = artwork(&d);
    assert!(covered(&list, pt(1.0, 1.0)));
    assert!(covered(&list, pt(39.0 * 3.0 + 1.0, 24.0 * 3.0 + 1.0)));
    assert!(!covered(&list, pt(2.5, 1.0)), "gap between squares");
}

#[test]
fn live_edit_preview_outlines_every_outline() {
    let d = Document::new(1);
    let id = ring(&d, true, 1.0);
    let objects = vec![d.object(id).unwrap()];
    let list = build_live_edit_preview(&objects, view());
    assert!(covered(&list, pt(0.0, 20.0)), "outer outline missing");
    assert!(
        covered(&list, pt(15.0, 20.0)),
        "hole outline missing in the preview"
    );
    assert!(!covered(&list, pt(20.0, 20.0)), "preview must be hollow");
    assert!(!covered(&list, pt(7.0, 20.0)));
}

#[test]
fn an_open_extra_outline_does_not_break_the_render() {
    let d = Document::new(1);
    compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            (
                vec![
                    NewAnchor::corner(AnchorId::new(40, 90), pt(50.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(40, 91), pt(60.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(40, 92), pt(60.0, 10.0)),
                ],
                false,
            ),
        ],
        false,
        1.0,
    );
    let list = artwork(&d);
    assert!(covered(&list, pt(55.0, 0.0)));
    assert!(covered(&list, pt(60.0, 5.0)));
    assert!(
        !covered(&list, pt(50.0, 5.0)),
        "open outline must not be closed by the stroke"
    );
}

#[test]
fn single_anchor_and_degenerate_extra_outlines_do_not_panic() {
    let d = Document::new(1);
    compound(
        &d,
        &[
            square(0, 0.0, 0.0, 10.0, true),
            (
                vec![NewAnchor::corner(AnchorId::new(40, 90), pt(50.0, 0.0))],
                true,
            ),
            (
                vec![
                    NewAnchor::corner(AnchorId::new(40, 91), pt(70.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(40, 92), pt(70.0, 0.0)),
                ],
                true,
            ),
        ],
        true,
        1.0,
    );
    let list = artwork(&d);
    assert!(covered(&list, pt(5.0, 5.0)));
}

/// Criterion 31a says every outline is dashed on its own. The dash limits are the object's: a
/// compound path within the per-object cap (2,000 dashes) is dashed in every outline, the last
/// like the first, and one over the cap is drawn solid in every outline, the first like the last
/// (never some outlines dashed and some solid).
fn large_compound_gap_pattern(n: usize) -> impl Fn(usize) -> (bool, bool) {
    let d = Document::new(1);
    // Squares of 20 mm: 80 dashes each at width 0.5 mm.
    let outlines: Vec<_> = (0..n)
        .map(|i| {
            square(
                (i * 4) as u64,
                (i % 30) as f64 * 25.0,
                (i / 30) as f64 * 25.0,
                20.0,
                true,
            )
        })
        .collect();
    let id = compound(&d, &outlines, false, 0.5);
    d.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 1.0]).unwrap()),
    )
    .unwrap();
    let list = artwork(&d);
    // Along the top edge of a square, dash 0..0.5 mm, gap 0.5..1.0 mm: (dashed, solid).
    move |i: usize| {
        let (x, y) = ((i % 30) as f64 * 25.0, (i / 30) as f64 * 25.0);
        (
            !covered(&list, pt(x + 10.75, y)) && covered(&list, pt(x + 10.25, y)),
            covered(&list, pt(x + 10.75, y)) && covered(&list, pt(x + 10.25, y)),
        )
    }
}

#[test]
fn ac31a_a_large_compound_path_dashes_its_last_outline_like_its_first() {
    let n = 24; // 1,920 dashes: within the cap of 2,000 for one object
    let at = large_compound_gap_pattern(n);
    assert!(at(0).0, "first outline is not dashed");
    assert!(at(n / 2).0, "a middle outline is not dashed");
    assert!(at(n - 1).0, "last outline is not dashed");
}

#[test]
fn ac31a_a_compound_path_over_the_per_object_cap_is_solid_in_every_outline() {
    let n = 700; // 56,000 dashes: over the cap, so the whole object is solid
    let at = large_compound_gap_pattern(n);
    assert!(at(0).1, "first outline is solid");
    assert!(at(n / 2).1, "a middle outline is solid");
    assert!(at(n - 1).1, "last outline is solid");
}
