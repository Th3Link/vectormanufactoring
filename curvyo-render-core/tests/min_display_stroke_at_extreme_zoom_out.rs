//! White-box edge-case test for the architect-flagged rendering risk
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`, flag 2): "at 2%,
//! the document is invisible" — a committed shape's default 0.25mm stroke
//! renders at roughly 0.25mm * (0.02 * 96/25.4) px/mm ≈ 0.019 screen px at
//! the minimum zoom, which drops out entirely without a floor. The default
//! chosen (recommendation (a)) is a 1-screen-px minimum *display* stroke
//! width, independent of the stored width.
//!
//! This is a numeric check of the actual tessellated geometry's on-screen
//! size at the 2% boundary, not just a non-zero `triangle_count` (`lyon`
//! can still emit a non-empty, near-zero-area sliver). Uses a primitive
//! (rect) rather than a path so the measurement is not confounded by
//! node-glyph geometry, which is screen-space-constant at a much larger
//! size and would dominate a naive bounding-box measurement.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{Document, Length, Point, RectBounds, ViewTransform};
use curvyo_render_core::build_primitive_strokes;

/// 2% zoom, in the same CSS-reference-pixel terms acceptance criterion 7
/// defines (`96.0 / 25.4` px/mm at 100%).
const SCALE_AT_2_PERCENT: f64 = 0.02 * (96.0 / 25.4);

#[test]
fn a_default_width_rect_stroke_does_not_vanish_at_2_percent_zoom() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let snapshot = document.primitive(rect).expect("exists");
    // The stored stroke width is this slice's 0.25mm placeholder default
    // — unmodified by this test, matching a real document.
    assert!((snapshot.style.stroke.width.as_mm() - 0.25).abs() < 1e-9);

    let view = ViewTransform::new(SCALE_AT_2_PERCENT, Point::new(0.0, 0.0));
    // No selection/hover: only the placeholder stroke itself draws, no
    // bounding box and no handles to confound the measurement.
    let list = build_primitive_strokes(&[snapshot], view);
    assert!(
        list.triangle_count() > 0,
        "the stroke must tessellate to something"
    );

    // The rect's nominal outline spans exactly 10mm each axis; the
    // stroke centerline sits on that outline, so the *total* vertex
    // bounding box grows by the stroke's own full width beyond 10mm
    // (half outward, half inward) on every side — a robust way to
    // recover the on-screen width without depending on how lyon
    // distributes vertices along a straight edge.
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for vertex in &list.triangles {
        min_x = min_x.min(vertex.position.x);
        max_x = max_x.max(vertex.position.x);
        min_y = min_y.min(vertex.position.y);
        max_y = max_y.max(vertex.position.y);
    }
    let width_mm = (max_x - min_x) - 10.0;
    let height_mm = (max_y - min_y) - 10.0;

    let raw_width_px = 0.25 * SCALE_AT_2_PERCENT;
    assert!(
        raw_width_px < 0.05,
        "sanity check: the raw (unfloored) width really would have been sub-pixel \
         ({raw_width_px} px)"
    );

    let width_px = width_mm * SCALE_AT_2_PERCENT;
    let height_px = height_mm * SCALE_AT_2_PERCENT;
    assert!(
        width_px > 0.5 && height_px > 0.5,
        "the on-screen stroke width must be floored to roughly 1 screen px on both axes, not \
         left at the raw {raw_width_px} px (measured {width_px} x {height_px} px, \
         {width_mm} x {height_mm} mm at 2% zoom)"
    );
}
