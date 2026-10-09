//! A compound path is drawn as one object with several outlines
//! (`specs/0016-boolean-operations` criteria 31, 31a, 32): the fill is the
//! nonzero area over all outlines (a ring has a hole), every outline is
//! stroked, and a dash pattern starts afresh at the first node of each outline.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::similar_names
)]

use curvyo_document_core::{
    AnchorId, Color, DashPattern, Document, FillMode, FillModeTarget, Length, NewAnchor, NodeId,
    ObjectSnapshot, Point, StyleEdit, ViewTransform,
};
use curvyo_render_core::{DrawList, build_artwork, build_live_edit_preview};

fn view() -> ViewTransform {
    ViewTransform::new(4.0, Point::new(0.0, 0.0))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A closed square outline with ids from `first`, reversed when `reversed`.
fn square(first: u64, x: f64, y: f64, side: f64, reversed: bool) -> (Vec<NewAnchor>, bool) {
    let mut corners = vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    if reversed {
        corners.reverse();
    }
    let anchors = corners
        .into_iter()
        .enumerate()
        .map(|(k, (cx, cy))| NewAnchor::corner(AnchorId::new(3, first + k as u64), pt(cx, cy)))
        .collect();
    (anchors, true)
}

/// A compound path of the given outlines, with a stroke of width 1 and no
/// fill unless asked for.
fn compound(document: &Document, outlines: &[(Vec<NewAnchor>, bool)], fill: bool) -> NodeId {
    let seed = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(9, 1), pt(500.0, 500.0)),
            NewAnchor::corner(AnchorId::new(9, 2), pt(501.0, 500.0)),
        ],
        false,
    );
    let id = document
        .replace_with_path(&[seed], seed, outlines, "boolean_union")
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(Length::from_mm(1.0)))
        .unwrap();
    if fill {
        document
            .edit_style(&[id], &StyleEdit::FillColor(Color { r: 255, g: 0, b: 0 }))
            .unwrap();
        document
            .set_fill_mode(
                FillMode::Solid,
                &[FillModeTarget {
                    id,
                    seed_stops: vec![],
                }],
            )
            .unwrap();
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
    build_artwork(&objects, &[], view())
}

/// Whether the triangles of `layer` cover `p`.
fn covers(list: &DrawList, layer: usize, p: (f64, f64)) -> bool {
    let start = if layer == 0 {
        0
    } else {
        list.layers()[layer - 1]
    };
    let end = list.layers()[layer];
    list.triangles[start..end].chunks(3).any(|t| {
        let [a, b, c] = [&t[0], &t[1], &t[2]].map(|v| (v.position.x, v.position.y));
        let side =
            |u: (f64, f64), v: (f64, f64)| (v.0 - u.0) * (p.1 - u.1) - (v.1 - u.1) * (p.0 - u.0);
        let (s1, s2, s3) = (side(a, b), side(b, c), side(c, a));
        (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0)
    })
}

/// Criterion 31: a ring paints the ring only; the centre shows what is behind.
#[test]
fn a_ring_fills_the_area_between_its_outlines_and_not_the_hole() {
    let document = Document::new(1);
    compound(
        &document,
        &[
            square(100, 0.0, 0.0, 40.0, false),
            square(200, 10.0, 10.0, 20.0, true),
        ],
        true,
    );
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 1, "a fill layer, the stroke is off");
    assert!(covers(&list, 0, (5.0, 20.0)), "between the outlines");
    assert!(covers(&list, 0, (35.0, 35.0)), "between the outlines");
    assert!(!covers(&list, 0, (20.0, 20.0)), "the hole is not painted");
    assert!(!covers(&list, 0, (45.0, 20.0)), "outside the ring");
}

/// An outline wound the same way as its surrounding one is a second layer of
/// fill, not a hole (nonzero rule).
#[test]
fn an_inner_outline_wound_alike_is_not_a_hole() {
    let document = Document::new(1);
    compound(
        &document,
        &[
            square(100, 0.0, 0.0, 40.0, false),
            square(200, 10.0, 10.0, 20.0, false),
        ],
        true,
    );
    assert!(covers(&artwork(&document), 0, (20.0, 20.0)));
}

/// Criterion 32: every outline is stroked, in one layer.
#[test]
fn every_outline_of_a_compound_path_is_stroked() {
    let document = Document::new(1);
    compound(
        &document,
        &[
            square(100, 0.0, 0.0, 40.0, false),
            square(200, 10.0, 10.0, 20.0, true),
            square(300, 60.0, 0.0, 10.0, false),
        ],
        false,
    );
    let list = artwork(&document);
    assert_eq!(list.layers().len(), 1, "one stroke layer for the object");
    for edge in [(0.0, 20.0), (10.0, 20.0), (60.0, 5.0), (40.0, 20.0)] {
        assert!(covers(&list, 0, edge), "the edge at {edge:?} is stroked");
    }
    assert!(
        !covers(&list, 0, (20.0, 20.0)),
        "not the middle of the hole"
    );
}

/// Criterion 31a: the dash pattern starts afresh at the first node of each
/// outline. The first outline has a perimeter of 164 mm, so a pattern that ran
/// on across outlines would start the second one in the middle of a gap.
#[test]
fn a_dash_pattern_restarts_at_the_first_node_of_each_outline() {
    let document = Document::new(1);
    let id = compound(
        &document,
        &[
            square(100, 0.0, 0.0, 41.0, false),
            square(200, 60.0, 0.0, 41.0, false),
        ],
        false,
    );
    document
        .edit_style(
            &[id],
            &StyleEdit::StrokeDash(DashPattern::new(vec![4.0, 4.0]).unwrap()),
        )
        .unwrap();
    let list = artwork(&document);
    // Both outlines start at their top-left corner and run to the right: the
    // first dash covers 0..4 mm of the top edge, the next gap 4..8.
    for x0 in [0.0, 60.0] {
        assert!(covers(&list, 0, (x0 + 2.0, 0.0)), "first dash at {x0}");
        assert!(!covers(&list, 0, (x0 + 6.0, 0.0)), "first gap at {x0}");
        assert!(covers(&list, 0, (x0 + 10.0, 0.0)), "second dash at {x0}");
    }
}

/// The blue preview of a moved or resized compound path outlines every
/// outline.
#[test]
fn the_live_preview_outlines_every_outline() {
    let document = Document::new(1);
    let id = compound(
        &document,
        &[
            square(100, 0.0, 0.0, 40.0, false),
            square(200, 10.0, 10.0, 20.0, true),
        ],
        false,
    );
    let preview = build_live_edit_preview(&[document.object(id).unwrap()], view());
    let near_edge = |p: (f64, f64)| {
        preview.triangles.chunks(3).any(|t| {
            let [a, b, c] = [&t[0], &t[1], &t[2]].map(|v| (v.position.x, v.position.y));
            let side = |u: (f64, f64), v: (f64, f64)| {
                (v.0 - u.0) * (p.1 - u.1) - (v.1 - u.1) * (p.0 - u.0)
            };
            let (s1, s2, s3) = (side(a, b), side(b, c), side(c, a));
            (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0)
        })
    };
    assert!(near_edge((0.0, 20.0)) && near_edge((10.0, 20.0)));
}
