//! Property tests of the interior (winding) test behind fill hit-testing,
//! `0007-stroke-and-fill-styling` criterion 23: derived from the rule (the
//! area a non-zero fill paints; an open outline is closed by a chord), with
//! expectations computed here.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::{OutlineTriple, contains_point};
use proptest::prelude::*;

fn corner(x: f64, y: f64) -> OutlineTriple {
    (Point::new(x, y), Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0))
}

proptest! {
    #[test]
    fn a_rectangle_contains_exactly_its_area_whatever_the_winding_direction(
        x in -100.0..100.0_f64, y in -100.0..100.0_f64,
        w in 1.0..100.0_f64, h in 1.0..100.0_f64,
        px in -250.0..250.0_f64, py in -250.0..250.0_f64,
    ) {
        // Keep clear of the edges: boundary points are the caller's tolerance.
        prop_assume!(((px - x).abs() > 1e-6) && ((px - x - w).abs() > 1e-6));
        prop_assume!(((py - y).abs() > 1e-6) && ((py - y - h).abs() > 1e-6));
        let inside = px > x && px < x + w && py > y && py < y + h;
        let cw = vec![corner(x, y), corner(x + w, y), corner(x + w, y + h), corner(x, y + h)];
        let ccw: Vec<OutlineTriple> = cw.iter().rev().copied().collect();
        let q = Point::new(px, py);
        prop_assert_eq!(contains_point(&cw, true, q), inside);
        prop_assert_eq!(contains_point(&ccw, true, q), inside);
        // Open: the chord from the last corner back to the first is a side.
        prop_assert_eq!(contains_point(&cw, false, q), inside);
    }

    #[test]
    fn an_open_triangle_is_the_same_area_as_the_closed_one(
        ax in -50.0..50.0_f64, ay in -50.0..50.0_f64,
        bx in -50.0..50.0_f64, by in -50.0..50.0_f64,
        cx in -50.0..50.0_f64, cy in -50.0..50.0_f64,
        px in -60.0..60.0_f64, py in -60.0..60.0_f64,
    ) {
        let tri = vec![corner(ax, ay), corner(bx, by), corner(cx, cy)];
        let q = Point::new(px, py);
        prop_assert_eq!(contains_point(&tri, false, q), contains_point(&tri, true, q));
    }

    #[test]
    fn any_input_is_answered_without_panicking(
        coords in proptest::collection::vec((-1e12..1e12_f64, -1e12..1e12_f64, -1e12..1e12_f64, -1e12..1e12_f64), 0..8),
        closed in any::<bool>(),
        px in -1e12..1e12_f64, py in -1e12..1e12_f64,
    ) {
        let outline: Vec<OutlineTriple> = coords
            .iter()
            .map(|(x, y, hx, hy)| (Point::new(*x, *y), Vec2::new(*hx, *hy), Vec2::new(-*hx, -*hy)))
            .collect();
        let _ = contains_point(&outline, closed, Point::new(px, py));
    }
}

#[test]
fn a_translated_outline_moves_its_interior_with_it() {
    let sq = vec![
        corner(0.0, 0.0),
        corner(10.0, 0.0),
        corner(10.0, 10.0),
        corner(0.0, 10.0),
    ];
    let moved: Vec<OutlineTriple> = sq
        .iter()
        .map(|(p, a, b)| (Point::new(p.x + 1e6, p.y - 1e6), *a, *b))
        .collect();
    assert!(contains_point(
        &moved,
        true,
        Point::new(1e6 + 5.0, -1e6 + 5.0)
    ));
    assert!(!contains_point(&moved, true, Point::new(5.0, 5.0)));
}
