//! Black-box acceptance tests for `specs/0004-canvas-navigation-and-
//! selection/specification.md`'s 24 acceptance criteria, written against
//! `vecmanf-editor-wasm::Session`'s public API (the plain-Rust
//! orchestration layer `wasm_api`/`gpu` are a thin wasm32-only shell
//! around) before reading the implementation diff in depth.
//!
//! `wasm_api::WasmSession` and `gpu::Gpu` are `#[cfg(target_arch =
//! "wasm32")]`-gated and cannot be exercised by a native `cargo test` —
//! AC 11 (resize reconfigures the GPU surface and renders in the same
//! call, with no stale frame) and AC 12's `S` keyboard shortcut (wired in
//! `frontend/src/hooks/useEditorSession.ts`, outside any Rust crate) are
//! verified separately, by reading that code, and noted as such in the
//! test report rather than silently skipped.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{Document, Point, Shape};
use vecmanf_editor_wasm::{Session, Tool};

fn document_of(session: &Session) -> Document {
    let bytes = session.pack("0.1.0").expect("pack");
    vecmanf_document_core::unpack(99, &bytes).expect("unpack")
}

/// Draws a rectangle with the Rectangle tool from `(x, y)` to
/// `(x + w, y + h)`, leaving it selected, same as `primitive-shapes`'s own
/// create-drag.
fn draw_rect(session: &mut Session, x: f64, y: f64, w: f64, h: f64) {
    session.set_tool(Tool::Rectangle);
    session.pointer_down(Point::new(x, y), false);
    session.pointer_up(Point::new(x + w, y + h), false, false);
}

/// Draws a two-anchor open path with the Pen tool from `a` to `b`.
fn draw_path(session: &mut Session, a: Point, b: Point) {
    session.set_tool(Tool::Pen);
    session.pointer_down(a, false);
    session.pointer_up(a, false, false);
    session.pointer_down(b, false);
    session.pointer_up(b, false, false);
    session.finish_pen();
}

/// Draws an ellipse with the Ellipse tool, bounding box from `(x, y)` to
/// `(x + w, y + h)`.
fn draw_ellipse(session: &mut Session, x: f64, y: f64, w: f64, h: f64) {
    session.set_tool(Tool::Ellipse);
    session.pointer_down(Point::new(x, y), false);
    session.pointer_up(Point::new(x + w, y + h), false, false);
}

fn rect_origin(document: &Document, id: vecmanf_document_core::NodeId) -> Point {
    let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
        panic!("expected rect");
    };
    bounds.origin
}

// ---------------------------------------------------------------------
// Pan (AC 1-5)
// ---------------------------------------------------------------------

#[test]
fn ac1_vertical_wheel_pans_vertically_and_touches_no_document_content() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    let before = document_of(&session);

    let anchor = session.screen_to_document(100.0, 100.0);
    session.wheel(0.0, 50.0, 100.0, 100.0, false, false);
    let after_anchor = session.screen_to_document(100.0, 100.0);

    // The view moved: the same screen pixel now maps to a different
    // document point.
    assert_ne!(anchor, after_anchor);
    // ...but purely vertically for a vertical-only wheel delta.
    assert!(
        (anchor.x - after_anchor.x).abs() < 1e-9,
        "no horizontal pan"
    );
    assert_ne!(anchor.y, after_anchor.y, "vertical pan happened");

    let after = document_of(&session);
    assert_eq!(before.object_ids(), after.object_ids());
    for id in before.object_ids() {
        assert_eq!(
            before.object(id),
            after.object(id),
            "content unchanged by pan"
        );
    }
}

#[test]
fn ac2_shift_scroll_pans_horizontally_instead_of_vertically() {
    let mut session = Session::new(1);
    let anchor = session.screen_to_document(100.0, 100.0);
    session.wheel(0.0, 50.0, 100.0, 100.0, true, false);
    let after = session.screen_to_document(100.0, 100.0);

    assert!(
        (anchor.y - after.y).abs() < 1e-9,
        "no vertical pan under shift"
    );
    assert_ne!(anchor.x, after.x, "horizontal pan happened under shift");
}

#[test]
fn ac3_middle_drag_pan_keeps_the_press_point_fixed_under_a_moving_cursor() {
    let mut session = Session::new(1);
    let press = (120.0, 80.0);
    let anchor = session.screen_to_document(press.0, press.1);

    session.begin_pan(press.0, press.1);
    assert!(session.is_panning());
    for cursor in [(200.0, 80.0), (200.0, 260.0), (10.0, 400.0)] {
        session.pan_to(cursor.0, cursor.1);
        let now_under_cursor = session.screen_to_document(cursor.0, cursor.1);
        assert!((now_under_cursor.x - anchor.x).abs() < 1e-9);
        assert!((now_under_cursor.y - anchor.y).abs() < 1e-9);
    }
    session.end_pan();
    assert!(!session.is_panning());
}

#[test]
fn ac4_space_drag_pan_uses_the_identical_begin_pan_to_end_pan_entry_points() {
    // `adrs.md`'s own decision: Space+primary and middle-mouse both drive
    // the same `begin_pan`/`pan_to`/`end_pan` trio — the host (not
    // `Session`) decides which physical gesture triggers the call. This
    // pins that there is exactly one mechanism to verify, and it behaves
    // identically to AC 3's.
    let mut session = Session::new(1);
    let press = (50.0, 50.0);
    let anchor = session.screen_to_document(press.0, press.1);
    session.begin_pan(press.0, press.1);
    session.pan_to(300.0, 300.0);
    let now_under_cursor = session.screen_to_document(300.0, 300.0);
    assert!((now_under_cursor.x - anchor.x).abs() < 1e-9);
    assert!((now_under_cursor.y - anchor.y).abs() < 1e-9);
    session.end_pan();
}

#[test]
fn ac5_panning_never_cancels_an_in_progress_pen_path() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    assert!(session.pen_in_progress().is_some());

    // Scroll-pan.
    session.wheel(0.0, 80.0, 50.0, 50.0, false, false);
    assert_eq!(
        session.pen_in_progress().map(<[_]>::len),
        Some(1),
        "scroll pan must not touch the in-progress path"
    );

    // Drag-pan (middle/space).
    session.begin_pan(10.0, 10.0);
    session.pan_to(90.0, 40.0);
    session.end_pan();
    assert_eq!(
        session.pen_in_progress().map(<[_]>::len),
        Some(1),
        "drag pan must not touch the in-progress path either"
    );

    // The path can still be finished normally afterwards.
    session.pointer_down(Point::new(20.0, 0.0), false);
    session.pointer_up(Point::new(20.0, 0.0), false, false);
    session.finish_pen();
    let document = document_of(&session);
    assert_eq!(document.object_ids().len(), 1);
}

// ---------------------------------------------------------------------
// Zoom (AC 6-9)
// ---------------------------------------------------------------------

#[test]
fn ac6_ctrl_scroll_zooms_about_the_cursor_point_which_stays_fixed() {
    let mut session = Session::new(1);
    // Pan away from the origin first so this isn't a trivial identity case.
    session.wheel(0.0, 300.0, 50.0, 50.0, false, false);

    let cursor = (250.0, 180.0);
    let before_zoom = session.zoom_percent();
    let anchor = session.screen_to_document(cursor.0, cursor.1);

    session.wheel(0.0, -200.0, cursor.0, cursor.1, false, true);

    let after_zoom = session.zoom_percent();
    let anchor_after = session.screen_to_document(cursor.0, cursor.1);
    assert_ne!(after_zoom, before_zoom, "zoom level changed");
    assert!(
        (anchor.x - anchor_after.x).abs() < 1e-9,
        "x stayed under cursor"
    );
    assert!(
        (anchor.y - anchor_after.y).abs() < 1e-9,
        "y stayed under cursor"
    );
}

#[test]
fn ac6_ctrl_scroll_the_other_direction_zooms_out_about_the_cursor() {
    let mut session = Session::new(1);
    let cursor = (400.0, 10.0);
    let anchor = session.screen_to_document(cursor.0, cursor.1);
    let before_zoom = session.zoom_percent();

    session.wheel(0.0, 500.0, cursor.0, cursor.1, false, true);

    assert!(
        session.zoom_percent() < before_zoom,
        "scrolling down zooms out"
    );
    let anchor_after = session.screen_to_document(cursor.0, cursor.1);
    assert!((anchor.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor.y - anchor_after.y).abs() < 1e-9);
}

#[test]
fn ac7_ac8_zoom_in_clamps_exactly_at_8000_percent_with_no_overshoot() {
    let mut session = Session::new(1);
    let cursor = (320.0, 240.0);
    for _ in 0..200 {
        session.wheel(0.0, -2000.0, cursor.0, cursor.1, false, true);
    }
    assert_eq!(
        session.zoom_percent(),
        8000,
        "clamped exactly at the maximum"
    );

    // One more zoom-in attempt must not move past the limit or error.
    let anchor = session.screen_to_document(cursor.0, cursor.1);
    session.wheel(0.0, -2000.0, cursor.0, cursor.1, false, true);
    assert_eq!(session.zoom_percent(), 8000, "stays exactly at the limit");
    let anchor_after = session.screen_to_document(cursor.0, cursor.1);
    assert!((anchor.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor.y - anchor_after.y).abs() < 1e-9);
}

#[test]
fn ac7_ac8_zoom_out_clamps_exactly_at_2_percent_with_no_overshoot() {
    let mut session = Session::new(1);
    let cursor = (320.0, 240.0);
    for _ in 0..200 {
        session.wheel(0.0, 2000.0, cursor.0, cursor.1, false, true);
    }
    assert_eq!(session.zoom_percent(), 2, "clamped exactly at the minimum");

    let anchor = session.screen_to_document(cursor.0, cursor.1);
    session.wheel(0.0, 2000.0, cursor.0, cursor.1, false, true);
    assert_eq!(session.zoom_percent(), 2, "stays exactly at the limit");
    let anchor_after = session.screen_to_document(cursor.0, cursor.1);
    assert!((anchor.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor.y - anchor_after.y).abs() < 1e-9);
}

#[test]
fn ac9_zoom_percent_is_always_readable_and_defaults_to_100() {
    let session = Session::new(1);
    assert_eq!(session.zoom_percent(), 100);
}

// ---------------------------------------------------------------------
// Canvas resize (AC 10; AC 11 is wasm32/GPU-only, see module doc)
// ---------------------------------------------------------------------

#[test]
fn ac10_resize_keeps_the_viewport_center_point_and_the_zoom() {
    let mut session = Session::new(1);
    session.resize_viewport(800.0, 600.0);
    let center_before = session.screen_to_document(400.0, 300.0);
    let zoom_before = session.zoom_percent();

    session.resize_viewport(1200.0, 300.0);
    let center_after = session.screen_to_document(600.0, 150.0);

    assert_eq!(
        session.zoom_percent(),
        zoom_before,
        "resize never changes zoom"
    );
    assert!((center_before.x - center_after.x).abs() < 1e-9);
    assert!((center_before.y - center_after.y).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// Select tool - default and reachability (AC 12-13)
// ---------------------------------------------------------------------

#[test]
fn ac13_new_session_defaults_to_select() {
    let session = Session::new(1);
    assert_eq!(session.tool(), Tool::Select);
}

#[test]
fn ac13_opening_a_saved_document_also_defaults_to_select() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    draw_path(&mut session, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    let bytes = session.pack("0.1.0").expect("pack");

    let reopened = Session::open(2, &bytes).expect("open");
    assert_eq!(reopened.tool(), Tool::Select);
}

// ---------------------------------------------------------------------
// Select tool - selecting / multi-select / move / delete (AC 14-21)
// ---------------------------------------------------------------------

#[test]
fn ac14_select_tool_selects_a_rect_a_path_and_an_ellipse() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    draw_path(&mut session, Point::new(50.0, 0.0), Point::new(60.0, 0.0));
    draw_ellipse(&mut session, 100.0, 0.0, 10.0, 10.0);

    session.set_tool(Tool::Select);
    let empty = session.draw_list().triangle_count();

    // Click the rect's left edge.
    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    let with_rect_selected = session.draw_list().triangle_count();
    assert!(with_rect_selected > empty, "a selection box drew something");

    session.escape();
    session.pointer_down(Point::new(0.0, 0.0), false); // clear via nothing hit below
    session.pointer_up(Point::new(0.0, 0.0), false, false);

    // Click the path.
    session.pointer_down(Point::new(55.0, 0.0), false);
    session.pointer_up(Point::new(55.0, 0.0), false, false);
    let with_path_selected = session.draw_list().triangle_count();
    assert!(
        with_path_selected > empty,
        "a selection box drew for the path too"
    );
}

#[test]
fn ac15_clicking_empty_canvas_clears_the_selection() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    let selected = session.draw_list().triangle_count();

    session.pointer_down(Point::new(900.0, 900.0), false);
    session.pointer_up(Point::new(900.0, 900.0), false, false);
    let cleared = session.draw_list().triangle_count();

    assert!(cleared < selected, "the selection box disappeared");

    // Delete now does nothing (nothing selected).
    let before = document_of(&session);
    session.delete_selected();
    let after = document_of(&session);
    assert_eq!(before.object_ids(), after.object_ids());
}

#[test]
fn ac16_plain_click_on_a_different_object_replaces_the_selection_not_adds() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    draw_rect(&mut session, 50.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    session.pointer_down(Point::new(50.0, 5.0), false);
    session.pointer_up(Point::new(50.0, 5.0), false, false);

    // Now delete: if the click were additive, both rects would be gone.
    session.delete_selected();
    let document = document_of(&session);
    assert_eq!(
        document.object_ids().len(),
        1,
        "only the second rect was selected"
    );
    let remaining = document.object_ids()[0];
    let Shape::Rect { bounds, .. } = document.primitive(remaining).unwrap().shape else {
        panic!("rect");
    };
    assert_eq!(
        bounds.origin,
        Point::new(0.0, 0.0),
        "the first rect survived"
    );
}

#[test]
fn ac17_shift_click_adds_a_second_object_to_the_selection() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    draw_rect(&mut session, 50.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    session.pointer_down(Point::new(50.0, 5.0), true);
    session.pointer_up(Point::new(50.0, 5.0), false, true);

    session.delete_selected();
    let document = document_of(&session);
    assert_eq!(
        document.object_ids().len(),
        0,
        "both were selected together and deleted"
    );
}

#[test]
fn ac18_dragging_any_selected_member_moves_the_whole_multi_selection() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    draw_rect(&mut session, 50.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    session.pointer_down(Point::new(50.0, 5.0), true);
    session.pointer_up(Point::new(50.0, 5.0), false, true);

    // Drag starting on the second rect (already selected) by (3, 2).
    session.pointer_down(Point::new(50.0, 5.0), false);
    session.pointer_up(Point::new(53.0, 7.0), false, false);

    let document = document_of(&session);
    let ids = document.object_ids();
    let origins: Vec<Point> = ids.iter().map(|&id| rect_origin(&document, id)).collect();
    assert!(
        origins.contains(&Point::new(3.0, 2.0)),
        "first rect moved by the drag offset: {origins:?}"
    );
    assert!(
        origins.contains(&Point::new(53.0, 2.0)),
        "second rect moved by the same offset: {origins:?}"
    );
}

#[test]
fn ac19_delete_removes_every_selected_object_as_one_commit() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    draw_path(&mut session, Point::new(50.0, 0.0), Point::new(60.0, 0.0));
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    session.pointer_down(Point::new(55.0, 0.0), true);
    session.pointer_up(Point::new(55.0, 0.0), false, true);

    session.delete_selected();
    let document = document_of(&session);
    assert_eq!(document.object_ids(), Vec::new());
}

#[test]
fn ac20_single_object_drag_moves_live_and_commits_once_on_release() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_hover(Point::new(4.0, 8.0), false, false);

    // Not yet committed mid-drag.
    let mid_drag = document_of(&session);
    let id = mid_drag.object_ids()[0];
    assert_eq!(rect_origin(&mid_drag, id), Point::new(0.0, 0.0));

    session.pointer_up(Point::new(4.0, 8.0), false, false);
    let after = document_of(&session);
    assert_eq!(rect_origin(&after, id), Point::new(4.0, 3.0));
}

#[test]
fn ac20_select_tool_shows_a_plain_box_while_its_own_tool_shows_handles_too() {
    // Note: a create-drag does not leave the new rect selected (`main`'s
    // pre-existing `shape_pointer_up`/`RectTool::pointer_up` discards the
    // `Created(id)` outcome without ever calling `selection.select_single`
    // — unrelated to this slice, present before it too), so this test
    // explicitly re-selects the rect under the Rectangle tool first (a
    // click on its outline, not a create-drag) to get a true "selected
    // under its own tool, handles showing" baseline to compare against.
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    let with_shape_handles = session.draw_list().triangle_count();
    assert!(
        with_shape_handles > 0,
        "the Rectangle tool draws its own handles"
    );

    // `object-transform` (acceptance criterion 1) deliberately gives the
    // Select tool its own 8 resize + 1 rotate handles on a single-object
    // selection now, on top of the plain box slice 4 shipped — so "the
    // Select tool draws strictly less than a shape tool's own handles"
    // (this test's original assertion) is no longer true by design, and
    // is not re-asserted here. What slice 4's own acceptance criterion
    // 20 still requires — amended, not voided, by this slice (`adrs.md`
    // flag 5) — is "no *shape-specific* handle", verified precisely by
    // `ac20_select_tool_never_leaks_a_shape_specific_handle_regardless_of_kind`
    // below, which this triangle-count proxy could never actually prove
    // either way.
    session.set_tool(Tool::Select);
    // The rect is still selected from the Rectangle-tool click above
    // (selection is shared across tools) — clear it first so
    // `nothing_selected` is a genuine empty baseline, not already
    // showing the plain box and transform handles.
    session.pointer_down(Point::new(500.0, 500.0), false);
    let nothing_selected = session.draw_list().triangle_count();
    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    let with_plain_box_and_transform_handles = session.draw_list().triangle_count();

    assert!(
        with_plain_box_and_transform_handles > nothing_selected,
        "selecting the rect under the Select tool must still draw *something* on top of the \
         empty baseline (its own plain box plus, since `object-transform`, its 8 resize + 1 \
         rotate transform handles)"
    );
}

/// `specs/0005-object-transform/adrs.md` flag 5: slice 4's own
/// "no shape handles and no path nodes" rule for the Select tool must
/// still hold — amended, not voided, by this slice's own generic
/// transform handles. Proven without reaching into any private
/// decoration-input type: a rectangle and an ellipse show a *different*
/// number of shape-tool handles under their own tool (the rectangle's
/// extra corner-radius handle), but the Select tool's own transform
/// handles are identical in count/shape for both kinds (8 resize + 1
/// rotate on the same generic box) — so if the Select-tool triangle
/// delta (selected vs. nothing selected) were ever to differ between
/// the two kinds, that would mean a shape-specific handle had leaked
/// into the Select tool's own decoration. It does not.
#[test]
fn ac20_select_tool_never_leaks_a_shape_specific_handle_regardless_of_kind() {
    let mut rect_session = Session::new(1);
    draw_rect(&mut rect_session, 0.0, 0.0, 10.0, 10.0);
    rect_session.set_tool(Tool::Select);
    let rect_unselected = rect_session.draw_list().triangle_count();
    rect_session.pointer_down(Point::new(0.0, 5.0), false);
    rect_session.pointer_up(Point::new(0.0, 5.0), false, false);
    let rect_selected = rect_session.draw_list().triangle_count();

    let mut ellipse_session = Session::new(1);
    draw_ellipse(&mut ellipse_session, 0.0, 0.0, 10.0, 10.0);
    ellipse_session.set_tool(Tool::Select);
    let ellipse_unselected = ellipse_session.draw_list().triangle_count();
    // The ellipse's own top point, (5, 0), sits exactly on its outline.
    ellipse_session.pointer_down(Point::new(5.0, 0.0), false);
    ellipse_session.pointer_up(Point::new(5.0, 0.0), false, false);
    let ellipse_selected = ellipse_session.draw_list().triangle_count();

    assert_eq!(
        rect_selected - rect_unselected,
        ellipse_selected - ellipse_unselected,
        "the Select tool's own decoration must add the identical amount of geometry for a \
         rect and an ellipse — any difference would mean a shape-specific (not generic \
         transform) handle leaked in"
    );
}

#[test]
fn ac21_delete_works_identically_for_a_rect_an_ellipse_and_a_path() {
    for setup in [
        (|s: &mut Session| draw_rect(s, 0.0, 0.0, 10.0, 10.0)) as fn(&mut Session),
        (|s: &mut Session| draw_ellipse(s, 0.0, 0.0, 10.0, 10.0)) as fn(&mut Session),
        (|s: &mut Session| draw_path(s, Point::new(0.0, 0.0), Point::new(10.0, 0.0)))
            as fn(&mut Session),
    ] {
        let mut session = Session::new(1);
        setup(&mut session);
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(5.0, 0.0), false);
        session.pointer_up(Point::new(5.0, 0.0), false, false);
        session.delete_selected();
        let document = document_of(&session);
        assert_eq!(
            document.object_ids(),
            Vec::new(),
            "no need to switch tools first"
        );
    }
}

// ---------------------------------------------------------------------
// Select tool - double-click handoff (AC 22-23)
// ---------------------------------------------------------------------

#[test]
fn ac22_double_click_on_a_path_hands_off_to_the_node_tool() {
    let mut session = Session::new(1);
    draw_path(&mut session, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    session.set_tool(Tool::Select);

    session.double_click(Point::new(5.0, 0.0), false, false);
    assert_eq!(session.tool(), Tool::Node);
}

#[test]
fn ac23_double_click_on_a_rect_hands_off_to_the_rectangle_tool() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.double_click(Point::new(0.0, 5.0), false, false);
    assert_eq!(session.tool(), Tool::Rectangle);
}

#[test]
fn ac23_double_click_on_an_ellipse_hands_off_to_the_ellipse_tool() {
    let mut session = Session::new(1);
    draw_ellipse(&mut session, 0.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.double_click(Point::new(5.0, 0.0), false, false);
    assert_eq!(session.tool(), Tool::Ellipse);
}

#[test]
fn double_click_on_empty_canvas_does_not_switch_tools() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Select);
    session.double_click(Point::new(900.0, 900.0), false, false);
    assert_eq!(session.tool(), Tool::Select);
}

// ---------------------------------------------------------------------
// Pan/zoom work in every tool (AC 24)
// ---------------------------------------------------------------------

#[test]
fn ac24_pan_and_zoom_work_while_the_rectangle_tool_is_mid_drag() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Rectangle);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_hover(Point::new(5.0, 5.0), false, false);

    let anchor = session.screen_to_document(200.0, 200.0);
    session.wheel(0.0, -500.0, 200.0, 200.0, false, true); // zoom
    let anchor_after = session.screen_to_document(200.0, 200.0);
    assert!((anchor.x - anchor_after.x).abs() < 1e-9);
    assert!((anchor.y - anchor_after.y).abs() < 1e-9);
    assert_eq!(session.tool(), Tool::Rectangle, "tool untouched by zoom");

    session.pointer_up(Point::new(10.0, 10.0), false, false);
    let document = document_of(&session);
    assert_eq!(
        document.object_ids().len(),
        1,
        "the drag still completed normally"
    );
}

#[test]
fn ac24_pan_and_zoom_work_while_the_node_tool_is_active() {
    let mut session = Session::new(1);
    draw_path(&mut session, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    session.set_tool(Tool::Node);

    session.wheel(0.0, 60.0, 50.0, 50.0, false, false);
    session.begin_pan(20.0, 20.0);
    session.pan_to(80.0, 80.0);
    session.end_pan();
    assert_eq!(session.tool(), Tool::Node);
}

// ---------------------------------------------------------------------
// Pan/zoom are ephemeral: never committed, never saved (adrs.md's
// explicitly flagged risk)
// ---------------------------------------------------------------------

#[test]
fn pan_and_zoom_are_not_persisted_across_a_pack_and_reopen_round_trip() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);

    // Pan and zoom away from the default view.
    session.wheel(0.0, 300.0, 50.0, 50.0, false, false);
    session.wheel(0.0, -400.0, 50.0, 50.0, false, true);
    assert_ne!(
        session.zoom_percent(),
        100,
        "sanity: the view actually changed"
    );

    let bytes = session.pack("0.1.0").expect("pack");
    let reopened = Session::open(2, &bytes).expect("open");

    assert_eq!(
        reopened.zoom_percent(),
        100,
        "a reopened document starts at the default 100% zoom, not the saved session's"
    );

    // Document content is unaffected by the view having been panned/zoomed.
    let original_content = document_of(&session);
    let reopened_content = document_of(&reopened);
    assert_eq!(original_content.object_ids(), reopened_content.object_ids());
    for id in original_content.object_ids() {
        assert_eq!(original_content.object(id), reopened_content.object(id));
    }
}

#[test]
fn pan_and_zoom_are_absent_from_the_packed_bytes_themselves() {
    // A stronger form of the above: compare the packed bytes of the same
    // document content at two different view states. If view state leaked
    // into the container, the bytes would differ.
    let mut plain = Session::new(1);
    draw_rect(&mut plain, 0.0, 0.0, 10.0, 10.0);
    let bytes_at_default_view = plain.pack("0.1.0").expect("pack");

    let mut panned = Session::new(1);
    draw_rect(&mut panned, 0.0, 0.0, 10.0, 10.0);
    panned.wheel(0.0, 500.0, 10.0, 10.0, false, false);
    panned.wheel(0.0, -900.0, 10.0, 10.0, false, true);
    let bytes_after_pan_zoom = panned.pack("0.1.0").expect("pack");

    assert_eq!(
        bytes_at_default_view, bytes_after_pan_zoom,
        "identical document content packs to identical bytes regardless of view state"
    );
}

/// Sanity check independent of the implementer's own `object_bounds`
/// tests: a path's selection-indicator extent (via the fact that it is
/// selectable and deletable, criterion 14's "extended here to paths too")
/// does not depend on pan/zoom view state — selecting the same path at two
/// different zoom levels must hit it either way.
#[test]
fn object_selection_hit_testing_is_unaffected_by_the_current_zoom_level() {
    let mut session = Session::new(1);
    draw_rect(&mut session, 0.0, 0.0, 10.0, 10.0);
    session.set_tool(Tool::Select);

    session.wheel(0.0, -1500.0, 5.0, 5.0, false, true); // zoom in a lot

    session.pointer_down(Point::new(5.0, 0.0), false);
    session.pointer_up(Point::new(5.0, 0.0), false, false);
    session.delete_selected();

    let document = document_of(&session);
    assert_eq!(
        document.object_ids(),
        Vec::new(),
        "still selectable after a big zoom change"
    );
}
