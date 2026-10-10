//! The document background at the `Session` level
//! (`specs/0040-document-background` criteria 8, 13, 14, 17 to 20, 22 to 24, 28 to 40,
//! 42 and 43): the Background block's commands, the preview, the eyedropper and the
//! rule that the background is not an object.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    missing_docs
)]

use curvyo_document_core::{BackgroundPaint, Color, Document, DocumentBackground, Point, unpack};
use curvyo_editor_wasm::{DocumentSide, Session, SizeOutcome, Tool};
use curvyo_render_core::{CHECKER_A, RgbaColor};
use curvyo_ui_core::{Grid, PaintTarget};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn doc_of(session: &Session) -> Document {
    unpack(9, &session.pack("0.1.0").unwrap()).unwrap()
}

fn stored(session: &Session) -> DocumentBackground {
    doc_of(session).background()
}

fn ops(session: &Session) -> i64 {
    let loro = loro::LoroDoc::new();
    loro.import(&doc_of(session).export_loro_snapshot().unwrap())
        .unwrap();
    loro.oplog_vv().values().map(|c| i64::from(*c)).sum()
}

/// A session whose background is `#FF0000FF`, with a 50 x 50 rectangle at (10, 10)
/// selected.
fn with_rect() -> Session {
    let mut s = Session::new(1);
    s.show_default_view();
    s.set_background_hex("FF0000FF").unwrap();
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(10.0, 10.0), false);
    s.pointer_up(pt(60.0, 60.0), false, false);
    s.set_tool(Tool::Select);
    s
}

// ---- criteria 2, 8: the default, New and Open ------------------------------------

#[test]
fn a_new_session_shows_the_default_background() {
    let s = Session::new(1);
    let view = s.background_view();
    assert_eq!(view.paint, "solid");
    assert_eq!(view.hex, "#E8E8EBFF");
    assert_eq!(view.color, 0x00E8_E8EB);
    assert_eq!(view.opacity_text, "100");
    assert!(!view.opacity_resettable);
    assert!(!view.picking);
    assert_eq!(stored(&s), DocumentBackground::DEFAULT);
}

#[test]
fn a_new_or_opened_project_shows_its_own_background() {
    let mut first = Session::new(1);
    first.set_background_hex("#FF0000FF").unwrap();
    let plain = Session::new(2).pack("t").unwrap();
    let reopened = Session::open(3, &plain).unwrap();
    assert_eq!(reopened.background_view().hex, "#E8E8EBFF");
    let again = Session::open(4, &first.pack("t").unwrap()).unwrap();
    assert_eq!(again.background_view().hex, "#FF0000FF");
    assert_eq!(Session::new(5).background_view().hex, "#E8E8EBFF");
}

// ---- criteria 13, 14: drawing ------------------------------------------------------

#[test]
fn the_frame_draws_the_background_under_the_artwork() {
    let mut s = with_rect();
    s.set_tool(Tool::Select);
    let frame = s.frame_draw_list();
    assert_eq!(frame.checker_end(), 0);
    let area = &frame.triangles[..6];
    assert!(area.iter().all(|v| v.color == RgbaColor::opaque(255, 0, 0)));
    // The rectangle's layers come after the area (a fill of None adds none).
    assert_eq!(frame.layers().first(), Some(&6));
}

#[test]
fn none_and_a_translucent_colour_ask_for_the_checkerboard() {
    let mut s = Session::new(1);
    assert!(s.set_background_paint(BackgroundPaint::None));
    let none = s.frame_draw_list();
    assert_eq!(none.checker_end(), 6);
    assert_eq!(none.triangles[0].color, CHECKER_A);
    assert!(s.set_background_paint(BackgroundPaint::Solid));
    s.set_background_hex("FF000080").unwrap();
    let translucent = s.frame_draw_list();
    assert_eq!(translucent.checker_end(), 6);
    assert_eq!(translucent.layers().len(), none.layers().len() + 1);
}

#[test]
fn a_drag_preview_changes_the_frame_in_the_same_call() {
    let mut s = Session::new(1);
    s.preview_background_hsv(0.0, 1.0, 1.0);
    assert!(
        s.frame_draw_list().triangles[..6]
            .iter()
            .all(|v| v.color == RgbaColor::opaque(255, 0, 0))
    );
    assert_eq!(s.background_view().hex, "#FF0000FF");
    assert_eq!(
        stored(&s),
        DocumentBackground::DEFAULT,
        "nothing is stored yet"
    );
}

// ---- criteria 17 to 20, 22: the controls ---------------------------------------------

#[test]
fn the_paint_group_is_one_commit_and_none_then_solid_returns_the_colour() {
    let mut s = Session::new(1);
    s.set_background_hex("2F6FEEFF").unwrap();
    let before = ops(&s);
    assert!(s.set_background_paint(BackgroundPaint::None));
    assert_eq!(ops(&s), before + 1, "one register, one write");
    assert_eq!(s.background_view().paint, "none");
    assert!(s.set_background_paint(BackgroundPaint::Solid));
    assert_eq!(s.background_view().hex, "#2F6FEEFF");
    let ops_now = ops(&s);
    assert!(
        !s.set_background_paint(BackgroundPaint::Solid),
        "already pressed"
    );
    assert_eq!(ops(&s), ops_now);
}

#[test]
fn the_hex_field_follows_the_style_rules_with_the_document_as_target() {
    let mut s = Session::new(1);
    assert_eq!(s.set_background_hex("fff"), Ok(true));
    assert_eq!(s.background_view().hex, "#FFFFFFFF");
    assert_eq!(s.set_background_hex(" f80c "), Ok(true));
    assert_eq!(s.background_view().hex, "#FF8800CC");
    // 3 and 6 digits keep the alpha.
    assert_eq!(s.set_background_hex("#123456"), Ok(true));
    assert_eq!(s.background_view().hex, "#123456CC");
    let ops_now = ops(&s);
    assert!(s.set_background_hex("12345").is_err());
    assert!(s.set_background_hex("").is_err());
    assert_eq!(
        s.set_background_hex("123456"),
        Ok(false),
        "equal writes nothing"
    );
    assert_eq!(ops(&s), ops_now);
}

#[test]
fn the_hex_and_the_opacity_field_edit_the_same_alpha_each_in_its_own_grid() {
    let mut s = Session::new(1);
    assert_eq!(s.set_background_hex("2F6FEE80"), Ok(true));
    let view = s.background_view();
    assert_eq!(view.opacity_text, "50");
    assert_eq!(stored(&s).opacity.get(), 128.0 / 255.0, "not rounded");
    assert_eq!(s.set_background_opacity_text("50"), Ok(true));
    assert_eq!(stored(&s).opacity.get(), 0.5);
    assert_eq!(s.background_view().hex, "#2F6FEE80");
    assert!(s.set_background_opacity_text("101").is_err());
}

#[test]
fn the_picker_drag_previews_then_commits_once_and_keeps_the_alpha() {
    let mut s = Session::new(1);
    s.set_background_hex("11223380").unwrap();
    let before = ops(&s);
    for step in 0..10 {
        s.preview_background_hsv(f64::from(step) * 10.0, 0.8, 0.9);
    }
    assert_eq!(ops(&s), before, "a drag writes nothing");
    s.commit_background_preview();
    assert_eq!(
        ops(&s),
        before + 1,
        "the release writes the colour register once"
    );
    let now = stored(&s);
    assert_eq!(now.opacity.get(), 128.0 / 255.0);
    assert_ne!(
        now.color,
        Color {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
    // The release again writes nothing.
    let ops_now = ops(&s);
    s.commit_background_preview();
    assert_eq!(ops(&s), ops_now);
}

#[test]
fn escape_drops_the_preview_and_the_release_writes_nothing() {
    let mut s = Session::new(1);
    let before = ops(&s);
    s.preview_background_hsv(200.0, 1.0, 1.0);
    s.cancel_background_preview();
    s.commit_background_preview();
    assert_eq!(ops(&s), before);
    assert_eq!(s.background_view().hex, "#E8E8EBFF");
}

#[test]
fn the_opacity_field_drags_steps_and_resets() {
    let mut s = Session::new(1);
    s.set_background_hex("FF0000FF").unwrap();
    s.preview_background_opacity(0.0, Grid::Normal);
    assert_eq!(s.background_view().opacity_text, "0");
    s.preview_background_opacity(1.0, Grid::Normal);
    assert_eq!(s.background_view().opacity_text, "100");
    s.preview_background_opacity(f64::NAN, Grid::Normal);
    assert_eq!(s.background_view().opacity_text, "100", "NaN is ignored");
    s.cancel_background_preview();
    // Arrow keys: 1 % each, Shift 10 %, previewed, committed on key-up.
    s.step_background_opacity(-1, Grid::Normal);
    s.step_background_opacity(-1, Grid::Normal);
    assert_eq!(s.background_view().opacity_text, "98");
    s.step_background_opacity(-1, Grid::Coarse);
    assert_eq!(
        s.background_view().opacity_text,
        "90",
        "the coarse grid rounds to tens"
    );
    assert_eq!(stored(&s).opacity.get(), 1.0);
    s.commit_background_preview();
    assert_eq!(stored(&s).opacity.get(), 0.9);
    assert!(s.background_view().opacity_resettable);
    assert!(s.reset_background_opacity());
    assert_eq!(stored(&s).opacity.get(), 1.0);
    assert!(!s.reset_background_opacity(), "already 100 %");
}

#[test]
fn every_edit_is_one_commit_and_the_paint_edit_leaves_the_colour_register_alone() {
    let mut s = Session::new(1);
    let before = ops(&s);
    s.set_background_paint(BackgroundPaint::None);
    assert_eq!(ops(&s), before + 1);
    let loro = loro::LoroDoc::new();
    loro.import(&doc_of(&s).export_loro_snapshot().unwrap())
        .unwrap();
    assert!(loro.get_map("root").get("background_color").is_none());
    assert!(loro.get_map("root").get("background_paint").is_some());
}

#[test]
fn a_drag_in_flight_still_commits_when_the_selection_changes() {
    let mut s = with_rect();
    s.preview_background_hsv(120.0, 1.0, 1.0);
    // A click on empty area clears the selection (the panel switches section).
    s.pointer_down(pt(150.0, 150.0), false);
    s.pointer_up(pt(150.0, 150.0), false, false);
    assert_eq!(
        stored(&s).color,
        Color { r: 0, g: 255, b: 0 },
        "committed by the click"
    );
}

#[test]
fn the_subject_line_is_derived_from_the_size_only() {
    let mut s = Session::new(1);
    let before = s.document_presets_view().subject;
    s.set_background_hex("FF0000FF").unwrap();
    assert_eq!(s.document_presets_view().subject, before);
}

// ---- criteria 28 to 39: the eyedropper ---------------------------------------------------

fn rect_stroke(s: &Session) -> (Color, f64, bool) {
    let doc = doc_of(s);
    let id = doc.object_ids()[0];
    let style = doc.object(id).unwrap().style().clone();
    (
        style.stroke.color,
        style.stroke.opacity.get(),
        style.stroke.enabled,
    )
}

#[test]
fn the_stroke_eyedropper_takes_the_stored_background_colour_with_its_alpha() {
    let mut s = with_rect();
    s.set_background_hex("FF000080").unwrap();
    s.set_stroke_paint(false);
    s.begin_colour_pick(PaintTarget::Stroke);
    s.pointer_hover(pt(150.0, 150.0), false, false);
    assert_eq!(
        s.colour_pick_hover(),
        Some(("#FF000080".to_string(), PaintTarget::Background))
    );
    s.pointer_down(pt(150.0, 150.0), false);
    s.pointer_up(pt(150.0, 150.0), false, false);
    let (color, opacity, enabled) = rect_stroke(&s);
    assert_eq!(color, Color { r: 255, g: 0, b: 0 });
    assert_eq!(
        opacity,
        128.0 / 255.0,
        "the stored alpha, not the composite"
    );
    assert!(enabled, "a pick for the stroke turns an off stroke on");
    assert!(s.colour_pick_target().is_none(), "picking ended");
}

#[test]
fn objects_keep_priority_over_the_background() {
    let mut s = with_rect();
    s.set_fill_paint(true);
    s.begin_colour_pick(PaintTarget::Stroke);
    s.pointer_down(pt(30.0, 30.0), false);
    s.pointer_up(pt(30.0, 30.0), false, false);
    // The rectangle's own fill is black, not the background.
    assert_eq!(rect_stroke(&s).0, Color { r: 0, g: 0, b: 0 });
}

#[test]
fn the_pasteboard_and_a_none_background_pick_nothing_and_keep_picking() {
    let mut s = with_rect();
    s.begin_colour_pick(PaintTarget::Stroke);
    let ops_now = ops(&s);
    // A4 is 210 x 297: 1 px outside the right edge.
    s.pointer_hover(pt(210.001, 100.0), false, false);
    assert_eq!(s.colour_pick_hover(), None);
    s.pointer_down(pt(210.001, 100.0), false);
    s.pointer_up(pt(210.001, 100.0), false, false);
    assert_eq!(ops(&s), ops_now);
    assert!(s.colour_pick_target().is_some());
    // The edge itself is on the document.
    s.pointer_hover(pt(210.0, 100.0), false, false);
    assert!(s.colour_pick_hover().is_some());
    s.end_colour_pick();
    s.set_background_paint(BackgroundPaint::None);
    s.begin_colour_pick(PaintTarget::Stroke);
    s.pointer_hover(pt(150.0, 150.0), false, false);
    assert_eq!(s.colour_pick_hover(), None);
    s.pointer_down(pt(150.0, 150.0), false);
    s.pointer_up(pt(150.0, 150.0), false, false);
    assert!(s.colour_pick_target().is_some(), "picking stays active");
}

#[test]
fn the_pick_does_not_depend_on_the_zoom() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.resize_viewport(800.0, 600.0);
    s.set_background_hex("12345680").unwrap();
    s.begin_colour_pick(PaintTarget::Background);
    let mut seen = Vec::new();
    for _ in 0..3 {
        s.pointer_hover(pt(100.0, 100.0), false, false);
        seen.push(s.colour_pick_hover());
        s.wheel(0.0, -300.0, 400.0, 300.0, false, true);
    }
    assert!(seen.iter().all(|hover| *hover == seen[0]));
    assert_eq!(
        seen[0],
        Some(("#12345680".to_string(), PaintTarget::Background))
    );
}

#[test]
fn the_background_eyedropper_sets_the_colour_keeps_solid_and_ends_picking() {
    let mut s = with_rect();
    s.set_background_paint(BackgroundPaint::None);
    s.begin_colour_pick(PaintTarget::Background);
    assert!(s.background_view().picking);
    // The rectangle's default stroke is black: press on its outline.
    s.pointer_down(pt(10.0, 30.0), false);
    s.pointer_up(pt(10.0, 30.0), false, false);
    let now = stored(&s);
    assert_eq!(now.paint, BackgroundPaint::Solid);
    assert_eq!(now.color, Color { r: 0, g: 0, b: 0 });
    assert!(!s.background_view().picking);
    assert_eq!(s.background_view().hex, "#000000FF");
}

#[test]
fn a_pick_is_announced_once_and_a_cancel_says_nothing() {
    let mut s = Session::new(1);
    s.set_background_hex("FF000080").unwrap();
    s.begin_colour_pick(PaintTarget::Background);
    s.escape();
    assert_eq!(s.take_colour_pick_announcement(), "");
    s.begin_colour_pick(PaintTarget::Background);
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_up(pt(100.0, 100.0), false, false);
    assert_eq!(
        s.take_colour_pick_announcement(),
        "Background color set to #FF000080"
    );
    assert_eq!(s.take_colour_pick_announcement(), "", "said once");
}

#[test]
fn a_pick_equal_to_the_stored_colour_writes_nothing_and_still_ends_picking() {
    let mut s = Session::new(1);
    s.begin_colour_pick(PaintTarget::Background);
    let ops_now = ops(&s);
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_up(pt(100.0, 100.0), false, false);
    assert_eq!(ops(&s), ops_now);
    assert!(s.colour_pick_target().is_none());
}

#[test]
fn a_pick_changes_nothing_in_the_selection_and_the_second_press_of_the_button_ends_it() {
    let mut s = with_rect();
    let count = s.selected_object_count();
    s.begin_colour_pick(PaintTarget::Background);
    s.pointer_down(pt(150.0, 250.0), false);
    s.pointer_up(pt(150.0, 250.0), false, false);
    assert_eq!(s.selected_object_count(), count);
    s.begin_colour_pick(PaintTarget::Background);
    s.begin_colour_pick(PaintTarget::Background);
    assert!(s.colour_pick_target().is_none());
}

// ---- criteria 40 to 43: the background is not an object -----------------------------------

#[test]
fn the_background_is_not_selectable_and_has_no_bounds() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.set_background_hex("FF0000FF").unwrap();
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_up(pt(100.0, 100.0), false, false);
    assert_eq!(s.selected_object_count(), 0);
    assert!(!s.has_objects(), "Fit to content stays absent");
    assert_eq!(s.fit_document(), curvyo_editor_wasm::FitOutcome::Empty);
}

#[test]
fn resizing_keeps_the_background_and_writes_no_background_register() {
    let mut s = with_rect();
    s.set_background_paint(BackgroundPaint::None);
    let want = stored(&s);
    let loro_registers = |s: &Session| {
        let loro = loro::LoroDoc::new();
        loro.import(&doc_of(s).export_loro_snapshot().unwrap())
            .unwrap();
        let root = loro.get_map("root");
        (
            root.get("background_paint").map(|v| v.get_deep_value()),
            root.get("background_color").map(|v| v.get_deep_value()),
        )
    };
    let registers = loro_registers(&s);
    assert_eq!(
        s.set_document_side(DocumentSide::Width, "297"),
        SizeOutcome::Committed
    );
    assert_eq!(
        s.set_document_side(DocumentSide::Height, "420"),
        SizeOutcome::Committed
    );
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Unchanged);
    assert_eq!(stored(&s), want);
    assert_eq!(loro_registers(&s), registers);
    s.set_display_unit(curvyo_document_core::DisplayUnit::In);
    s.fit_document();
    assert_eq!(stored(&s), want);
    assert_eq!(loro_registers(&s), registers);
}
