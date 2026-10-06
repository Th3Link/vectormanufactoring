//! Reproduction for a live-browser observation while verifying PR #20's
//! bug 4 ("pen tool drag-to-curve preview"): driving the compiled wasm
//! module in a real Chrome tab against `vecmanf-ui-core`'s own
//! `PenTool`/`build_pen_preview` wiring showed a plain straight line from
//! the last placed node all the way to the live cursor while a
//! click-and-hold drag was in flight — never the curved B→C segment with
//! growing handle lines/endpoints `specification.md`'s acceptance
//! criterion 2 and the UX notes ("Live curve preview while dragging a
//! node's handles") call for, and never the diamond glyph the PR's own
//! `pen_preview.rs` doc comment says a drag always previews (`"a drag
//! always produces a Smooth node... so the diamond glyph previews the
//! kind it is about to commit as"`).
//!
//! This test drives the exact same sequence through `Session` (the same
//! public surface `vecmanf_editor_wasm::wasm_api::WasmSession` is a thin
//! shell over, and the same type the implementer's own
//! `draw_list_shows_the_live_curve_preview_during_a_pen_drag` test in
//! `session/mod.rs` exercises) to check, independent of any browser or
//! GPU rendering, whether the draw list `Session::draw_list()` produces
//! mid-drag is geometrically a curve (criterion 2) or still just the
//! plain straight rubber-band line (criterion 1) `cursor` alone would
//! draw once `drag_origin` is ignored.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::Point;
use vecmanf_editor_wasm::{Session, Tool};

/// Same shape as the implementer's own
/// `draw_list_shows_the_live_curve_preview_during_a_pen_drag` (B at the
/// origin, C 10mm to the right, dragged to (13,4)) but checking the
/// *geometry*, not just a triangle-count increase: a triangle count
/// increase is also exactly what the plain straight rubber-band line
/// would show as the cursor moves farther from the last placed node (its
/// own thick-line quad gets longer), so that assertion alone cannot
/// distinguish "the curve+handles preview is drawing" from "the curve
/// preview never engaged and this is still just criterion 1's plain
/// line, now longer". A genuine curve bows away from the straight B→C
/// chord; every vertex of a plain straight B→cursor line stays within
/// the axis-aligned bounding box of B and the cursor themselves (plus
/// each node glyph's own few-mm-wide outline) — so a vertex meaningfully
/// outside that box is only possible from real curve/handle geometry.
#[test]
fn pen_drag_preview_draws_curve_geometry_not_just_a_longer_straight_line() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);

    session.pointer_down(Point::new(100.0, 0.0), false);
    // Drag straight "up" (negative y) by 50mm, perpendicular to the
    // B-C chord (which lies along y = 0) so a real curve bows the
    // *opposite* way (handle_in is the negated drag vector) and would
    // be unmistakable; a plain straight B->cursor rubber band, by
    // contrast, never leaves the y in [0, 50] band (B and the cursor are
    // its only two endpoints, both at y in {0, 50}).
    session.pointer_hover(Point::new(100.0, 50.0), false, false);

    let list = session.draw_list();
    assert!(
        !list.triangles.is_empty(),
        "something must be drawn for an in-progress 2-node pen session"
    );

    let min_y = list
        .triangles
        .iter()
        .map(|v| v.position.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = list
        .triangles
        .iter()
        .map(|v| v.position.y)
        .fold(f64::NEG_INFINITY, f64::max);

    // A node/handle glyph is a handful of screen pixels wide; at
    // `ViewTransform::identity()` (`Session::new`'s default, 1 document
    // mm == 1 "pixel" for `screen_px_to_mm`) that is at most a few mm, so
    // 5mm is a generous margin for "within a glyph's own outline of a
    // node that sits at y = 0", and nowhere close to the ~22mm the
    // curve's own control point (`C + handle_in`, here `(100, -50)`)
    // would pull real stroke geometry toward.
    assert!(
        min_y < -5.0,
        "no vertex rises meaningfully above y=0 (min_y={min_y}, max_y={max_y}): nothing in \
         the draw list bows toward the handle_in control point at (100,-50) the way \
         acceptance criterion 2's live curve preview must — this looks like the plain \
         straight B-to-cursor rubber-band line (criterion 1), not the curve+handles preview"
    );
}

/// Direct comparison against `vecmanf_render_core::build_pen_preview`
/// called explicitly two ways from the exact same inputs
/// `Session::draw_list` itself would use at this moment: once with
/// `pending: None` (criterion 1's plain rubber band to the cursor) and
/// once with `pending: Some(&anchor)` (criterion 2's curve+handles, the
/// anchor `PenTool::pending_anchor` resolves — this crate-boundary
/// architect review: the preview and `PenTool::pointer_up`'s own eventual
/// commit must share one resolution rule, not two independent
/// re-implementations of it, so this test builds `pending` the same way
/// `Session::draw_list` itself does, via `pending_anchor`, rather than
/// constructing a point by hand). `Session::draw_list()`'s own output,
/// for a press that is genuinely still held down, must match the
/// `Some(&anchor)` call and differ from the `None` call — not the other
/// way around.
#[test]
fn pen_drag_preview_matches_the_pending_anchor_some_branch_not_the_none_branch() {
    use vecmanf_render_core::build_pen_preview;
    use vecmanf_ui_core::{AnchorIdMinter, PenTool};

    let document = vecmanf_document_core::Document::new(1);
    let mut minter = AnchorIdMinter::new(1);
    let mut pen = PenTool::new();
    let close_tolerance = vecmanf_document_core::Length::from_mm(2.0);
    let drag_threshold = vecmanf_document_core::Length::from_mm(1.0);

    pen.pointer_down(Point::new(0.0, 0.0), close_tolerance);
    pen.pointer_up(&mut minter, &document, Point::new(0.0, 0.0), drag_threshold);
    pen.pointer_down(Point::new(100.0, 0.0), close_tolerance);

    let cursor = Point::new(100.0, 50.0);
    // Press held, not released: `pending_anchor` must resolve to exactly
    // what `pointer_up` would commit if released at `cursor` right now —
    // `minter.peek()`, not `mint()`, since this is only a preview.
    let pending = pen.pending_anchor(minter.peek(), cursor, drag_threshold);
    assert!(pending.is_some(), "a held press must have a pending anchor");

    let nodes = pen.in_progress_nodes().expect("still placing").to_vec();

    // Drive the exact same sequence through `Session` (what the host
    // actually calls every frame) first, so the manual
    // `build_pen_preview` calls below use `Session`'s own current view
    // (100% zoom, `canvas-navigation-and-selection`'s `Viewport`
    // default) rather than assuming the identity view
    // `path-node-editing` built this repro against — the display
    // tolerance is screen-space now (`specs/0004-canvas-navigation-and-
    // selection/adrs.md`), so a mismatched view changes the tessellated
    // triangle count even for geometrically-identical input.
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    session.pointer_down(Point::new(100.0, 0.0), false);
    session.pointer_hover(cursor, false, false);
    let session_list = session.draw_list();
    let view = session.view();

    let with_pending = build_pen_preview(&nodes, Some(cursor), pending.as_ref(), view, false);
    let without_pending = build_pen_preview(&nodes, Some(cursor), None, view, false);

    assert_ne!(
        with_pending.triangle_count(),
        without_pending.triangle_count(),
        "the curve+handles preview (Some) and the plain rubber-band preview (None) must \
         produce different amounts of geometry for this input, or this comparison can't \
         tell them apart"
    );

    assert_eq!(
        session_list.triangle_count(),
        with_pending.triangle_count(),
        "Session::draw_list() mid-drag must match build_pen_preview's own Some(pending) branch \
         (triangle_count: with_pending={}, without_pending={}, session={})",
        with_pending.triangle_count(),
        without_pending.triangle_count(),
        session_list.triangle_count()
    );
}
