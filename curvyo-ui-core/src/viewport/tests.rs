use super::*;

#[test]
fn zoom_clamps_to_the_documented_range() {
    assert!((Zoom::new(1000.0).factor() - Zoom::MAX).abs() < f64::EPSILON);
    assert!((Zoom::new(0.0).factor() - Zoom::MIN).abs() < f64::EPSILON);
}

#[test]
fn zoom_percent_reads_exactly_at_the_limits() {
    assert_eq!(Zoom::new(Zoom::MIN).percent(), 2);
    assert_eq!(Zoom::new(Zoom::MAX).percent(), 8000);
    assert_eq!(Zoom::default().percent(), 100);
}

#[test]
fn default_viewport_is_100_percent_at_the_document_origin() {
    let viewport = Viewport::new();
    assert_eq!(viewport.zoom_percent(), 100);
    assert_eq!(viewport.screen_to_document(0.0, 0.0), Point::new(0.0, 0.0));
}

/// Criterion 11a: the document's corner is 72 px in from the canvas's
/// corner on both axes, at 100 %.
#[test]
fn a_new_view_puts_the_document_corner_72_px_in() {
    let viewport = Viewport::with_document_inset();
    assert_eq!(viewport.zoom_percent(), 100);
    let (x, y) = viewport.view().document_to_screen(Point::new(0.0, 0.0));
    assert!(
        (x - 72.0).abs() < 1e-9 && (y - 72.0).abs() < 1e-9,
        "{x} {y}"
    );
}

/// The inset survives size reports and window resizes until the maker pans or
/// zooms; then a resize keeps the centre again (`0004` criterion 10).
#[test]
fn the_inset_survives_resizes_until_the_first_pan_or_zoom() {
    let mut viewport = Viewport::with_document_inset();
    viewport.resize(1.0, 1.0);
    viewport.resize(700.0, 500.0);
    viewport.resize(650.0, 480.0);
    let (x, y) = viewport.view().document_to_screen(Point::new(0.0, 0.0));
    assert!(
        (x - 72.0).abs() < 1e-9 && (y - 72.0).abs() < 1e-9,
        "{x} {y}"
    );

    viewport.pan_by_screen_delta(0.0, 0.0);
    viewport.resize(850.0, 480.0);
    let (x, _) = viewport.view().document_to_screen(Point::new(0.0, 0.0));
    assert!((x - 172.0).abs() < 1e-9, "centre kept after a pan: {x}");
}

#[test]
fn pan_by_screen_delta_moves_the_origin_proportionally() {
    let mut viewport = Viewport::new();
    let before = viewport.screen_to_document(0.0, 0.0);
    viewport.pan_by_screen_delta(10.0, 20.0);
    let after = viewport.screen_to_document(0.0, 0.0);
    let moved = before.vector_to(after);
    assert!(moved.x > 0.0 && moved.y > 0.0, "content moved: {moved:?}");
    // Double the delta, double the pan (proportional, AC 1/2).
    let mut doubled = Viewport::new();
    doubled.pan_by_screen_delta(20.0, 40.0);
    let doubled_after = doubled.screen_to_document(0.0, 0.0);
    let doubled_moved = before.vector_to(doubled_after);
    assert!((doubled_moved.x - 2.0 * moved.x).abs() < 1e-9);
    assert!((doubled_moved.y - 2.0 * moved.y).abs() < 1e-9);
}

#[test]
fn pan_never_changes_the_zoom() {
    let mut viewport = Viewport::new();
    viewport.pan_by_screen_delta(500.0, -300.0);
    assert_eq!(viewport.zoom_percent(), 100);
}

#[test]
fn zoom_about_a_point_keeps_that_point_under_the_cursor() {
    let mut viewport = Viewport::new();
    // Pan somewhere away from the origin first, so this isn't a
    // trivially-true identity-view case.
    viewport.pan_by_screen_delta(137.0, -42.0);

    let cursor = (300.0, 150.0);
    let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);
    viewport.zoom_about(cursor.0, cursor.1, 2.0);
    let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);

    assert!((anchor_before.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor_before.y - anchor_after.y).abs() < 1e-9);
    assert_eq!(viewport.zoom_percent(), 200);
}

#[test]
fn zoom_about_a_point_keeps_it_fixed_even_when_clamped_at_the_maximum() {
    let mut viewport = Viewport::new();
    let cursor = (412.0, 88.0);
    let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);

    // A factor huge enough that the pre-clamp target scale would far
    // exceed 8000% — the clamp must bite, and the point under the
    // cursor must still be exactly where it was, computed from the
    // *clamped* scale, not the pre-clamp target.
    viewport.zoom_about(cursor.0, cursor.1, 1_000_000.0);
    assert_eq!(viewport.zoom_percent(), 8000, "the clamp must have bitten");

    let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);
    assert!(
        (anchor_before.x - anchor_after.x).abs() < 1e-9,
        "x drifted: {anchor_before:?} -> {anchor_after:?}"
    );
    assert!(
        (anchor_before.y - anchor_after.y).abs() < 1e-9,
        "y drifted: {anchor_before:?} -> {anchor_after:?}"
    );
}

#[test]
fn zoom_about_a_point_keeps_it_fixed_even_when_clamped_at_the_minimum() {
    let mut viewport = Viewport::new();
    let cursor = (60.0, 500.0);
    let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);

    viewport.zoom_about(cursor.0, cursor.1, 1e-9);
    assert_eq!(viewport.zoom_percent(), 2, "the clamp must have bitten");

    let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);
    assert!((anchor_before.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor_before.y - anchor_after.y).abs() < 1e-9);
}

#[test]
fn drag_pan_keeps_the_anchor_point_under_a_moving_cursor() {
    let mut viewport = Viewport::new();
    let press_at = (100.0, 100.0);
    let anchor = viewport.screen_to_document(press_at.0, press_at.1);
    viewport.begin_drag_pan(press_at.0, press_at.1);
    assert!(viewport.is_drag_panning());

    for cursor in [(150.0, 100.0), (150.0, 220.0), (40.0, 300.0)] {
        viewport.continue_drag_pan(cursor.0, cursor.1);
        let now_under_cursor = viewport.screen_to_document(cursor.0, cursor.1);
        assert!((now_under_cursor.x - anchor.x).abs() < 1e-9);
        assert!((now_under_cursor.y - anchor.y).abs() < 1e-9);
    }

    viewport.end_drag_pan();
    assert!(!viewport.is_drag_panning());
}

#[test]
fn continue_drag_pan_without_a_gesture_in_flight_is_a_no_op() {
    let mut viewport = Viewport::new();
    let before = viewport;
    viewport.continue_drag_pan(999.0, 999.0);
    assert_eq!(viewport, before);
}

#[test]
fn resize_keeps_the_center_point_and_the_zoom() {
    let mut viewport = Viewport::new();
    viewport.resize(800.0, 600.0);
    let center_before = viewport.screen_to_document(400.0, 300.0);

    viewport.resize(1000.0, 400.0);
    let center_after = viewport.screen_to_document(500.0, 200.0);

    assert!((center_before.x - center_after.x).abs() < 1e-9);
    assert!((center_before.y - center_after.y).abs() < 1e-9);
    assert_eq!(
        viewport.zoom_percent(),
        100,
        "resize never changes the zoom"
    );
}

#[test]
fn the_first_resize_only_records_the_size_with_no_center_to_preserve() {
    let mut viewport = Viewport::new();
    let origin_before = viewport.screen_to_document(0.0, 0.0);
    viewport.resize(800.0, 600.0);
    let origin_after = viewport.screen_to_document(0.0, 0.0);
    assert_eq!(origin_before, origin_after);
}

/// The properties panel opening or closing resizes the canvas by its own
/// width; the document must not move on screen (`0007` criterion 39).
#[test]
fn a_panel_toggle_keeps_the_top_left_origin() {
    let mut viewport = Viewport::new();
    viewport.resize(1000.0, 600.0);
    viewport.pan_by_screen_delta(120.0, 40.0);
    let corner = viewport.screen_to_document(0.0, 0.0);
    let point = viewport.screen_to_document(300.0, 200.0);

    viewport.keep_origin_for_width_change(-280.0);
    viewport.resize(720.0, 600.0);
    assert_eq!(viewport.screen_to_document(0.0, 0.0), corner);
    assert_eq!(viewport.screen_to_document(300.0, 200.0), point);
    assert_eq!(viewport.canvas_size(), (720.0, 600.0));

    viewport.keep_origin_for_width_change(280.0);
    viewport.resize(1000.0, 600.0);
    assert_eq!(viewport.screen_to_document(0.0, 0.0), corner);
}

/// The request is for one panel-sized width change only: a window resize
/// that does not match it, or the next resize after it, keeps the centre.
#[test]
fn a_panel_toggle_request_does_not_outlive_its_resize() {
    let mut viewport = Viewport::new();
    viewport.resize(1000.0, 600.0);
    viewport.keep_origin_for_width_change(-280.0);
    // The window shrank in height too: a window resize, centre kept.
    viewport.resize(720.0, 500.0);
    let centre = viewport.screen_to_document(360.0, 250.0);
    viewport.resize(1000.0, 500.0);
    let after = viewport.screen_to_document(500.0, 250.0);
    assert!((centre.x - after.x).abs() < 1e-9);

    // Used once: the same width change later is an ordinary resize.
    viewport.keep_origin_for_width_change(-280.0);
    viewport.resize(720.0, 500.0);
    let centre = viewport.screen_to_document(360.0, 250.0);
    viewport.resize(440.0, 500.0);
    let after = viewport.screen_to_document(220.0, 250.0);
    assert!((centre.x - after.x).abs() < 1e-9);
}

/// A panel opened and closed again before any resize reported leaves no
/// request behind: a later window resize of the same width keeps the centre.
#[test]
fn two_toggles_before_a_resize_leave_nothing_pending() {
    let mut viewport = Viewport::new();
    viewport.resize(1000.0, 600.0);
    viewport.keep_origin_for_width_change(-280.0);
    viewport.keep_origin_for_width_change(280.0);
    let centre = viewport.screen_to_document(500.0, 300.0);
    viewport.resize(1280.0, 600.0);
    let after = viewport.screen_to_document(640.0, 300.0);
    assert!((centre.x - after.x).abs() < 1e-9, "an ordinary resize");
}

/// A resize or fit moves the objects by `offset` in the document: moving the
/// origin by the same offset keeps every object's screen position, within
/// 0.5 px (criterion 20), at any zoom.
#[test]
fn panning_by_a_document_offset_keeps_a_shifted_object_on_screen() {
    for percent in [2.0, 100.0, 800.0, 8000.0] {
        let mut viewport = Viewport::new();
        viewport.zoom_about(0.0, 0.0, percent / 100.0);
        let object = Point::new(55.0, 61.5);
        let before = viewport.view().document_to_screen(object);
        let shift = Vec2::new(45.0, -88.5);
        viewport.pan_by_document_offset(shift);
        let after = viewport.view().document_to_screen(object.translated(shift));
        assert!((after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5);
    }
}

/// The command does not end the untouched default view: its inset and its
/// origin-keeping resize rule stay (architect note, `adrs.md` decision 7).
#[test]
fn a_document_offset_does_not_end_the_untouched_view() {
    let mut viewport = Viewport::with_document_inset();
    viewport.resize(700.0, 500.0);
    viewport.pan_by_document_offset(Vec2::new(10.0, 10.0));
    viewport.resize(900.0, 600.0);
    let (x, _) = viewport.view().document_to_screen(Point::new(10.0, 10.0));
    assert!(
        (x - 72.0).abs() < 1e-9,
        "origin kept through the resize: {x}"
    );
}
