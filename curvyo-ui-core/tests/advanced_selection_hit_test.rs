//! The candidate list and the lasso test of `advanced-selection`
//! (`specs/advanced-selection/specification.md`, criteria 3 to 7 and 18):
//! `hit_test_objects` orders every object under a point, nearest first, and
//! always starts with the answer of a plain click; `hit_test_objects_along`
//! tests an outline against a drawn line.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    AnchorId, Document, EllipseFrame, FillMode, FillModeTarget, Length, NewAnchor, NodeId,
    ObjectSnapshot, Point, RectBounds, Tolerance,
};
use curvyo_ui_core::{hit_test_object, hit_test_objects, hit_test_objects_along};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tol() -> Tolerance {
    Tolerance::from_mm(1.0)
}

fn line_path(document: &Document, n: u64, a: Point, b: Point) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 2 * n), a),
            NewAnchor::corner(AnchorId::new(1, 2 * n + 1), b),
        ],
        false,
    )
}

fn square(document: &Document, x: f64, y: f64, size: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(size),
        height: Length::from_mm(size),
    })
}

fn circle(document: &Document, x: f64, y: f64, r: f64) -> NodeId {
    document.create_ellipse(EllipseFrame {
        center: pt(x, y),
        rx: Length::from_mm(r),
        ry: Length::from_mm(r),
    })
}

fn fill(document: &Document, id: NodeId) {
    document
        .set_fill_mode(
            FillMode::Solid,
            &[FillModeTarget {
                id,
                seed_stops: vec![],
            }],
        )
        .unwrap();
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

/// Three horizontal lines 0.3, 0.6 and 0.9 mm above y = 0, drawn bottom to
/// top in the order `far`, `mid`, `near`: the nearest to a point at y = 0 is
/// the last drawn.
fn stacked_lines(document: &Document) -> (NodeId, NodeId, NodeId) {
    let far = line_path(document, 1, pt(0.0, -0.9), pt(20.0, -0.9));
    let mid = line_path(document, 2, pt(0.0, -0.6), pt(20.0, -0.6));
    let near = line_path(document, 3, pt(0.0, -0.3), pt(20.0, -0.3));
    (far, mid, near)
}

/// Criterion 3 and 5: nearest first, then each farther one.
#[test]
fn candidates_come_nearest_first() {
    let document = Document::new(1);
    let (far, mid, near) = stacked_lines(&document);
    let objects = objects(&document);
    assert_eq!(
        hit_test_objects(&objects, pt(10.0, 0.0), tol()),
        vec![near, mid, far]
    );
}

/// Criterion 3: an exact distance tie favours the topmost, in the list as in
/// a plain click.
#[test]
fn an_exact_tie_goes_to_the_topmost() {
    let document = Document::new(1);
    let below = line_path(&document, 1, pt(0.0, -0.5), pt(20.0, -0.5));
    let above = line_path(&document, 2, pt(0.0, 0.5), pt(20.0, 0.5));
    let objects = objects(&document);
    assert_eq!(
        hit_test_objects(&objects, pt(10.0, 0.0), tol()),
        vec![above, below]
    );
}

/// Criterion 7: one object under the point is a list of one.
#[test]
fn a_single_candidate_is_a_list_of_one_and_nothing_is_empty() {
    let document = Document::new(1);
    let only = line_path(&document, 1, pt(0.0, 0.0), pt(20.0, 0.0));
    let objects = objects(&document);
    assert_eq!(hit_test_objects(&objects, pt(10.0, 0.4), tol()), vec![only]);
    assert_eq!(
        hit_test_objects(&objects, pt(10.0, 5.0), tol()),
        Vec::<NodeId>::new()
    );
    assert_eq!(
        hit_test_objects(&[], pt(0.0, 0.0), tol()),
        Vec::<NodeId>::new()
    );
}

/// An object beyond the tolerance is not a candidate (criterion 1's radius
/// is the Session's: the list uses the tolerance it is given).
#[test]
fn only_objects_within_the_tolerance_are_candidates() {
    let document = Document::new(1);
    let (far, mid, near) = stacked_lines(&document);
    let objects = objects(&document);
    let narrow = Tolerance::from_mm(0.5);
    assert_eq!(
        hit_test_objects(&objects, pt(10.0, 0.0), narrow),
        vec![near],
        "{far:?} {mid:?}"
    );
}

/// A filled rectangle with a hollow circle above it, and a hollow circle
/// below a filled one: whatever the scene, the list starts with the plain
/// click's answer (`0007` criterion 27).
#[test]
fn the_first_candidate_is_always_the_plain_click_answer() {
    let document = Document::new(1);
    let backdrop = square(&document, 0.0, 0.0, 40.0);
    fill(&document, backdrop);
    let ring = circle(&document, 20.0, 20.0, 8.0);
    let hidden = circle(&document, 30.0, 10.0, 4.0);
    let cover = square(&document, 26.0, 6.0, 8.0);
    fill(&document, cover);
    let _ = (ring, hidden);
    // `hidden` was created above `backdrop` but the cover is above it again.
    let objects = objects(&document);
    for x in 0..=80 {
        for y in 0..=80 {
            let point = pt(f64::from(x) * 0.5, f64::from(y) * 0.5);
            let list = hit_test_objects(&objects, point, tol());
            assert_eq!(
                list.first().copied(),
                hit_test_object(&objects, point, tol()),
                "at {point:?}"
            );
            let mut sorted = list.clone();
            sorted.sort_by_key(|id| format!("{id:?}"));
            sorted.dedup();
            assert_eq!(sorted.len(), list.len(), "no id twice at {point:?}");
        }
    }
}

/// Criterion 4, with fills: a click inside two overlapping filled shapes
/// selects the upper; the Alt-click list continues with the one it covers.
#[test]
fn a_filled_shape_lists_the_one_it_covers_after_itself() {
    let document = Document::new(1);
    let lower = square(&document, 0.0, 0.0, 20.0);
    let upper = square(&document, 10.0, 10.0, 20.0);
    fill(&document, lower);
    fill(&document, upper);
    let objects = objects(&document);
    // Inside both interiors, away from every outline.
    assert_eq!(
        hit_test_objects(&objects, pt(15.0, 15.0), tol()),
        vec![upper, lower]
    );
    // Inside the lower only.
    assert_eq!(hit_test_objects(&objects, pt(5.0, 5.0), tol()), vec![lower]);
}

/// A hollow circle's outline under a filled cover: a plain click gets the
/// cover, the cycle continues to the circle.
#[test]
fn an_outline_under_a_fill_is_reachable_after_the_fill() {
    let document = Document::new(1);
    let ring = circle(&document, 10.0, 10.0, 5.0);
    let cover = square(&document, 0.0, 0.0, 20.0);
    fill(&document, cover);
    let objects = objects(&document);
    let on_ring = pt(15.0, 10.0);
    assert_eq!(hit_test_object(&objects, on_ring, tol()), Some(cover));
    assert_eq!(
        hit_test_objects(&objects, on_ring, tol()),
        vec![cover, ring]
    );
}

/// The outline candidates above the cover come before the cover.
#[test]
fn outlines_above_a_fill_come_before_it() {
    let document = Document::new(1);
    let backdrop = square(&document, 0.0, 0.0, 40.0);
    fill(&document, backdrop);
    let ring = circle(&document, 20.0, 20.0, 8.0);
    let objects = objects(&document);
    assert_eq!(
        hit_test_objects(&objects, pt(28.0, 20.0), tol()),
        vec![ring, backdrop]
    );
}

// ---- the lasso line ----

#[test]
fn a_line_across_an_outline_selects_it() {
    let document = Document::new(1);
    let target = square(&document, 0.0, 0.0, 10.0);
    let other = square(&document, 40.0, 0.0, 10.0);
    let objects = objects(&document);
    let line = [pt(-5.0, 5.0), pt(5.0, 5.0)];
    assert_eq!(hit_test_objects_along(&objects, &line, tol()), vec![target]);
    let _ = other;
}

/// Criterion 20: a line through an unfilled interior that never reaches the
/// outline selects nothing, and one that ends within tolerance does.
#[test]
fn a_line_inside_the_outline_selects_nothing_until_it_gets_close() {
    let document = Document::new(1);
    let target = square(&document, 0.0, 0.0, 10.0);
    let objects = objects(&document);
    let inside = [pt(2.0, 2.0), pt(8.0, 2.0), pt(8.0, 8.0)];
    assert_eq!(
        hit_test_objects_along(&objects, &inside, tol()),
        Vec::<NodeId>::new()
    );
    let nearly = [pt(2.0, 2.0), pt(2.0, 0.6)];
    assert_eq!(
        hit_test_objects_along(&objects, &nearly, tol()),
        vec![target]
    );
}

/// A crossing is found however the line is sampled: a long diagonal stretch
/// that crosses a thin line mid-way, between its two polyline points.
#[test]
fn a_crossing_between_two_polyline_points_is_found() {
    let document = Document::new(1);
    let thin = line_path(&document, 1, pt(50.0, -10.0), pt(50.0, 10.0));
    let objects = objects(&document);
    let line = [pt(0.0, 0.0), pt(100.0, 0.0)];
    assert_eq!(hit_test_objects_along(&objects, &line, tol()), vec![thin]);
}

#[test]
fn the_result_is_in_z_order_and_counts_each_object_once() {
    let document = Document::new(1);
    let a = line_path(&document, 1, pt(10.0, -5.0), pt(10.0, 5.0));
    let b = line_path(&document, 2, pt(20.0, -5.0), pt(20.0, 5.0));
    let objects = objects(&document);
    // Drawn right to left, crossing b before a.
    let line = [pt(30.0, 0.0), pt(0.0, 0.0)];
    assert_eq!(hit_test_objects_along(&objects, &line, tol()), vec![a, b]);
}

#[test]
fn a_single_point_line_tests_that_point() {
    let document = Document::new(1);
    let a = line_path(&document, 1, pt(0.0, 0.0), pt(10.0, 0.0));
    let objects = objects(&document);
    assert_eq!(
        hit_test_objects_along(&objects, &[pt(5.0, 0.5)], tol()),
        vec![a]
    );
    assert_eq!(
        hit_test_objects_along(&objects, &[pt(5.0, 5.0)], tol()),
        Vec::<NodeId>::new()
    );
    assert_eq!(
        hit_test_objects_along(&objects, &[], tol()),
        Vec::<NodeId>::new()
    );
}

/// A line far longer than the object, and a vertical or horizontal stretch
/// (a zero slope in the slab clip), still resolve.
#[test]
fn axis_parallel_and_very_long_lines_resolve() {
    let document = Document::new(1);
    let target = square(&document, 0.0, 0.0, 10.0);
    let objects = objects(&document);
    let vertical = [pt(5.0, -1e9), pt(5.0, 1e9)];
    assert_eq!(
        hit_test_objects_along(&objects, &vertical, tol()),
        vec![target]
    );
    let beside = [pt(50.0, -1e9), pt(50.0, 1e9)];
    assert_eq!(
        hit_test_objects_along(&objects, &beside, tol()),
        Vec::<NodeId>::new()
    );
}

/// Found by the tester's property test on CI: the line passes 1.93 mm from the
/// rectangle's corner region at a point between two coarse samples, inside the
/// 2 mm tolerance.
#[test]
fn a_grazing_line_between_two_samples_is_still_within_tolerance() {
    let document = Document::new(1);
    let target = document.create_rect(RectBounds {
        origin: pt(25.0, 23.0),
        width: Length::from_mm(11.0),
        height: Length::from_mm(2.0),
    });
    let objects = objects(&document);
    let line = [pt(31.13, 37.29), pt(72.13, -33.71)];
    assert_eq!(
        hit_test_objects_along(&objects, &line, Tolerance::from_mm(2.0)),
        vec![target]
    );
}
