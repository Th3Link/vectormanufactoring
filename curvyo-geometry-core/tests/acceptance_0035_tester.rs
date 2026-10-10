//! Independent kernel tests for `0035-combine-and-break-apart`: the touch test (everything within
//! 0.001 mm is reported, nothing 0.003 mm or more apart is) and the nesting depth, with random
//! inputs checked against references computed here. Written from the specification and the ADR's
//! stated guarantees before the implementation was read.

#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    missing_docs
)]

use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::{
    Outline, OutlineTriple, outline_nesting, signed_area_mm2, touching_outlines,
};
use proptest::prelude::*;

fn tri(x: f64, y: f64) -> OutlineTriple {
    (Point::new(x, y), Vec2::ZERO, Vec2::ZERO)
}

fn square(x: f64, y: f64, w: f64, h: f64) -> Vec<OutlineTriple> {
    vec![tri(x, y), tri(x + w, y), tri(x + w, y + h), tri(x, y + h)]
}

fn circle(cx: f64, cy: f64, r: f64) -> Vec<OutlineTriple> {
    let k = 0.552_284_749_830_793_4 * r;
    vec![
        (
            Point::new(cx + r, cy),
            Vec2::new(0.0, -k),
            Vec2::new(0.0, k),
        ),
        (
            Point::new(cx, cy + r),
            Vec2::new(k, 0.0),
            Vec2::new(-k, 0.0),
        ),
        (
            Point::new(cx - r, cy),
            Vec2::new(0.0, k),
            Vec2::new(0.0, -k),
        ),
        (
            Point::new(cx, cy - r),
            Vec2::new(-k, 0.0),
            Vec2::new(k, 0.0),
        ),
    ]
}

fn touches(a: &[OutlineTriple], b: &[OutlineTriple]) -> bool {
    let outlines = [Outline::new(a, true), Outline::new(b, true)];
    touching_outlines(&outlines).iter().any(|&(i, j)| i != j)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Two axis-aligned squares with a horizontal gap g: touching when g <= 0.001, not when
    /// g >= 0.003, whatever the size and the vertical offset.
    #[test]
    fn the_touch_guarantee_holds_for_squares(
        w in 1.0f64..200.0, h in 1.0f64..200.0,
        dy in -0.9f64..0.9,
        close_gap in 0.0f64..0.001,
        far_gap in 0.003f64..5.0,
    ) {
        let a = square(0.0, 0.0, w, h);
        let near = square(w + close_gap, dy * h, w, h);
        prop_assert!(touches(&a, &near), "gap {close_gap}");
        let far = square(w + far_gap, dy * h, w, h);
        prop_assert!(!touches(&a, &far), "gap {far_gap}");
    }

    /// Two circles on the x axis: tangent distance d = r1 + r2 + gap.
    #[test]
    fn the_touch_guarantee_holds_for_curves(
        r1 in 1.0f64..50.0, r2 in 1.0f64..50.0,
        close_gap in 0.0f64..0.0008,
        far_gap in 0.005f64..5.0,
    ) {
        // The Bezier circle is not exact (radius error up to 0.03 %): allow for it.
        let slack = 0.0003 * (r1 + r2);
        let a = circle(0.0, 0.0, r1);
        let near = circle(r1 + r2 + close_gap - slack, 0.0, r2);
        prop_assert!(touches(&a, &near));
        let far = circle(r1 + r2 + far_gap + slack, 0.0, r2);
        prop_assert!(!touches(&a, &far));
    }

    /// Concentric squares: the depth of each is the number of larger ones, in any list order.
    #[test]
    fn nesting_depth_counts_the_enclosing_outlines(
        n in 1usize..12,
        order in proptest::collection::vec(any::<u32>(), 12),
    ) {
        let mut sizes: Vec<usize> = (0..n).collect();
        // Shuffle by the random keys.
        sizes.sort_by_key(|&i| order[i]);
        let polys: Vec<Vec<OutlineTriple>> = sizes
            .iter()
            .map(|&i| {
                let m = i as f64 * 3.0;
                square(m, m, 100.0 - 2.0 * m, 100.0 - 2.0 * m)
            })
            .collect();
        let outlines: Vec<Outline<'_>> = polys.iter().map(|p| Outline::new(p, true)).collect();
        let nesting = outline_nesting(&outlines);
        for (pos, &i) in sizes.iter().enumerate() {
            prop_assert_eq!(nesting[pos].depth, i, "square {} is enclosed by {} others", i, i);
            match nesting[pos].parent {
                None => prop_assert_eq!(i, 0),
                Some(p) => prop_assert_eq!(sizes[p], i - 1, "the parent is the next larger"),
            }
        }
    }

    /// Disjoint discs on a grid have depth 0 and no parent; a disc placed in the middle of a
    /// bigger one has depth 1.
    #[test]
    fn disjoint_discs_are_all_depth_zero(
        cols in 1usize..8, rows in 1usize..8, r in 1.0f64..9.0,
    ) {
        let mut polys = Vec::new();
        for c in 0..cols {
            for rw in 0..rows {
                polys.push(circle(c as f64 * 25.0, rw as f64 * 25.0, r));
            }
        }
        let outlines: Vec<Outline<'_>> = polys.iter().map(|p| Outline::new(p, true)).collect();
        for n in outline_nesting(&outlines) {
            prop_assert_eq!(n.depth, 0);
            prop_assert!(n.parent.is_none());
        }
        prop_assert!(touching_outlines(&outlines).is_empty());
    }
}

#[test]
fn a_self_crossing_outline_is_reported_as_touching_itself() {
    // A bowtie.
    let bow = vec![
        tri(0.0, 0.0),
        tri(10.0, 10.0),
        tri(10.0, 0.0),
        tri(0.0, 10.0),
    ];
    let outlines = [Outline::new(&bow, true)];
    assert_eq!(touching_outlines(&outlines), vec![(0, 0)]);
    // A plain square is not.
    let sq = square(0.0, 0.0, 10.0, 10.0);
    assert_eq!(
        touching_outlines(&[Outline::new(&sq, true)]),
        Vec::<(usize, usize)>::new()
    );
}

#[test]
fn a_nested_pair_with_a_hole_edge_exactly_on_the_plate_edge_touches() {
    let outer = square(0.0, 0.0, 100.0, 100.0);
    let inner = square(0.0, 40.0, 20.0, 20.0);
    assert!(touches(&outer, &inner));
}

#[test]
fn the_signed_area_helper_agrees_in_sign_with_the_stated_winding_convention() {
    // Canonical form: shapes have positive shoelace area in document coordinates.
    let sq = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 10.0),
    ];
    assert!(signed_area_mm2(&sq) > 0.0);
}

#[test]
#[ignore = "release performance: cargo test --release -p curvyo-geometry-core -- --ignored"]
fn criterion_19_touch_and_nesting_of_a_plate_with_1000_circles_take_under_500_ms() {
    let mut polys = vec![square(0.0, 0.0, 1000.0, 1000.0)];
    for i in 0..1000 {
        polys.push(circle(
            15.0 + f64::from(i % 40) * 24.0,
            15.0 + f64::from(i / 40) * 24.0,
            8.0,
        ));
    }
    let outlines: Vec<Outline<'_>> = polys.iter().map(|p| Outline::new(p, true)).collect();
    let t = std::time::Instant::now();
    let touching = touching_outlines(&outlines);
    let nesting = outline_nesting(&outlines);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("touch + nesting: {ms:.1} ms");
    assert_eq!(touching, Vec::<(usize, usize)>::new());
    assert_eq!(nesting.iter().filter(|n| n.depth == 1).count(), 1000);
    assert!(ms < 500.0, "{ms} ms");
}
