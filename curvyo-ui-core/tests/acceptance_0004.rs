//! Additional black-box / white-box tests for `specs/0004-canvas-
//! navigation-and-selection/specification.md`, independent of the
//! implementer's own `curvyo-ui-core` unit tests (`viewport.rs`,
//! `select_tool.rs`, `hit_test_object.rs`, `object_bounds.rs` each carry
//! their own `#[cfg(test)]` modules already) — these use different
//! numbers, directions and object shapes, written against the crate's
//! public API.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, EllipseFrame, Length, NewAnchor, Point, RectBounds, Tolerance,
    Vec2,
};
use curvyo_ui_core::{Viewport, hit_test_object, object_bounds};

// ---------------------------------------------------------------------
// AC 6/8: zoom-toward-cursor stays fixed through the clamp, re-verified
// independently (different cursor, different starting pan, zooming out
// first then back in across the boundary).
// ---------------------------------------------------------------------

#[test]
fn zoom_clamp_at_the_minimum_from_an_already_panned_and_zoomed_state() {
    let mut viewport = Viewport::new();
    // Start from a non-trivial state: panned and already zoomed in once.
    viewport.pan_by_screen_delta(-220.0, 95.0);
    viewport.zoom_about(50.0, 50.0, 3.0);

    let cursor = (17.0, 640.0);
    let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);

    // Zoom out far enough that the clamp must bite at the 2% floor.
    viewport.zoom_about(cursor.0, cursor.1, 1e-6);
    assert_eq!(viewport.zoom_percent(), 2);

    let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);
    assert!((anchor_before.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor_before.y - anchor_after.y).abs() < 1e-9);
}

#[test]
fn zoom_clamp_round_trip_in_then_out_leaves_no_residual_drift() {
    let mut viewport = Viewport::new();
    let cursor = (123.0, 456.0);
    let anchor = viewport.screen_to_document(cursor.0, cursor.1);

    // Overshoot past the maximum, then straight back out past the
    // minimum, both anchored at the same cursor pixel.
    viewport.zoom_about(cursor.0, cursor.1, 1e9);
    assert_eq!(viewport.zoom_percent(), 8000);
    let after_max = viewport.screen_to_document(cursor.0, cursor.1);
    assert!((anchor.x - after_max.x).abs() < 1e-9);
    assert!((anchor.y - after_max.y).abs() < 1e-9);

    viewport.zoom_about(cursor.0, cursor.1, 1e-9);
    assert_eq!(viewport.zoom_percent(), 2);
    let after_min = viewport.screen_to_document(cursor.0, cursor.1);
    assert!((anchor.x - after_min.x).abs() < 1e-9);
    assert!((anchor.y - after_min.y).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// AC 14: a path's selection box is tight to the curve's own extrema, not
// its (looser) control-point hull.
// ---------------------------------------------------------------------

#[test]
fn object_bounds_of_a_curved_path_is_tighter_than_its_control_point_hull() {
    let document = Document::new(1);
    // A single cubic segment from (0, 0) to (100, 0) with handles that
    // pull the control polygon's own hull far outside the curve's real
    // extent — handles straight up/down by 200mm, control-hull y range
    // would be [-200, 200], but the curve itself (a symmetric S-shape)
    // peaks far short of that.
    let path = document.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(1, 1),
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(0.0, 200.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor {
                id: AnchorId::new(1, 2),
                point: Point::new(100.0, 0.0),
                handle_in: Vec2::new(0.0, -200.0),
                handle_out: Vec2::ZERO,
                kind: AnchorKind::Corner,
            },
        ],
        false,
    );
    let object = document.object(path).expect("exists");
    let (min, max) = object_bounds(&object);

    // The control-point hull's own y-range would be exactly [-200, 200].
    // The true cubic extrema are strictly inside that (a standard
    // cubic Bezier with these symmetric handles peaks at 75mm for a unit
    // chord scaled here, well under 200).
    assert!(
        max.y < 150.0 && min.y > -150.0,
        "bounds must hug the curve's own extrema, not the 200mm control-handle hull: \
         got min={min:?} max={max:?}"
    );
    // Sanity: the box is not degenerate either.
    assert!(
        max.y > 10.0 && min.y < -10.0,
        "the curve does bulge noticeably: {min:?} {max:?}"
    );
}

// ---------------------------------------------------------------------
// hit_test_object: edge cases beyond the implementer's own tie-break test.
// ---------------------------------------------------------------------

#[test]
fn hit_test_object_on_an_empty_object_list_is_always_a_miss() {
    let hit = hit_test_object(&[], Point::new(0.0, 0.0), Tolerance::from_mm(1000.0));
    assert_eq!(
        hit, None,
        "no objects, however generous the tolerance, is always a miss"
    );
}

#[test]
fn hit_test_object_at_exactly_zero_tolerance_still_hits_a_point_on_the_outline() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let objects = vec![document.object(rect).expect("exists")];
    // Exactly on the outline, zero tolerance: distance is exactly 0,
    // which must still satisfy `distance <= tolerance` (0 <= 0).
    let hit = hit_test_object(&objects, Point::new(5.0, 0.0), Tolerance::from_mm(0.0));
    assert_eq!(hit, Some(rect));
}

#[test]
fn hit_test_object_a_tiny_fraction_outside_zero_tolerance_misses() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let objects = vec![document.object(rect).expect("exists")];
    let hit = hit_test_object(&objects, Point::new(5.0, 0.001), Tolerance::from_mm(0.0));
    assert_eq!(hit, None);
}

/// Two overlapping *paths* (not the implementer's own rect/rect tie
/// test) in the same position: the later one in z-order wins an exact
/// tie, re-verified for a different object kind.
#[test]
fn an_exact_tie_between_two_identical_paths_favors_the_later_one() {
    let document = Document::new(1);
    let make = |peer| {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(peer, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(peer, 2), Point::new(20.0, 0.0)),
            ],
            false,
        )
    };
    let bottom = make(1);
    let top = make(2);
    let objects = vec![
        document.object(bottom).expect("exists"),
        document.object(top).expect("exists"),
    ];
    let hit = hit_test_object(&objects, Point::new(10.0, 0.0), Tolerance::from_mm(1.0));
    assert_eq!(hit, Some(top));
}

// ---------------------------------------------------------------------
// A primitive's hit test is unaffected by its kind — an ellipse behaves
// like the rect/path cases already covered elsewhere.
// ---------------------------------------------------------------------

#[test]
fn hit_test_object_hits_an_ellipse_outline_but_not_its_interior() {
    let document = Document::new(1);
    let ellipse = document.create_ellipse(EllipseFrame {
        center: Point::new(0.0, 0.0),
        rx: Length::from_mm(20.0),
        ry: Length::from_mm(10.0),
    });
    let objects = vec![document.object(ellipse).expect("exists")];
    let on_outline = hit_test_object(&objects, Point::new(20.0, 0.0), Tolerance::from_mm(0.5));
    assert_eq!(on_outline, Some(ellipse));
    let interior = hit_test_object(&objects, Point::new(0.0, 0.0), Tolerance::from_mm(0.5));
    assert_eq!(interior, None);
}
