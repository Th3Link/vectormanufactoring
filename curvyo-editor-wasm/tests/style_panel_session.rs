//! `Session`-level checks of the Style panel (`specs/0007-stroke-and-fill-styling`
//! PR 3, criteria 2, 5, 13, 24, 36, 37): the panel edits what the tool's scope
//! says, a drag previews without writing and commits once on release, and the
//! preview is drawn in place of the stored style.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Document, FillMode, Length, LineCap, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::DrawList;
use curvyo_ui_core::{BarValue, DashChoice, StyleEntryError, StyleField};

/// A session with `n` 10 mm rectangles in a row, the last one selected.
fn session_with_rectangles(n: u32) -> Session {
    let mut session = Session::new(1);
    for k in 0..n {
        session.set_tool(Tool::Rectangle);
        let x = f64::from(k) * 30.0;
        session.pointer_down(Point::new(x, 0.0), false);
        session.pointer_up(Point::new(x + 10.0, 10.0), false, false);
    }
    session
}

/// Selects the first `n` rectangles of `session_with_rectangles`: a click on
/// the edge of the first, Shift-clicks on the others.
fn select_first(session: &mut Session, n: u32) {
    session.set_tool(Tool::Select);
    for k in 0..n {
        let edge = Point::new(f64::from(k) * 30.0, 5.0);
        session.pointer_hover(edge, k > 0, false);
        session.pointer_down(edge, k > 0);
        session.pointer_up(edge, k > 0, false);
    }
}

/// The styles of every object as the document holds them now.
fn stored_styles(session: &Session) -> Vec<Style> {
    let bytes = session.pack("0.1.0").unwrap();
    let document: Document = unpack(9, &bytes).unwrap();
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .map(|object| match object {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn has_alpha(list: &DrawList, alpha: u8) -> bool {
    list.triangles.iter().any(|v| v.color.a == alpha)
}

fn percent(n: u32) -> Opacity {
    Opacity::new(f64::from(n) / 100.0).unwrap()
}

#[test]
fn a_drawn_rectangle_is_selected_and_editable() {
    let session = session_with_rectangles(1);
    let state = session.style_panel_state();
    assert!(state.enabled);
    assert_eq!(state.subject, "Rectangle");
}

#[test]
fn the_pen_and_an_empty_selection_disable_the_panel() {
    let mut session = session_with_rectangles(1);
    session.set_tool(Tool::Pen);
    let state = session.style_panel_state();
    assert!(!state.enabled);
    assert_eq!(state.subject, "Pen: finish the path to style it");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "2"),
        Ok(false)
    );
    assert_eq!(
        stored_styles(&session)[0].stroke.width,
        Length::from_mm(0.25)
    );
}

#[test]
fn a_typed_value_is_one_commit_for_the_whole_selection() {
    let mut session = session_with_rectangles(3);
    select_first(&mut session, 3);
    assert_eq!(session.style_panel_state().subject, "3 rectangles");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "2,5"),
        Ok(true)
    );
    for style in stored_styles(&session) {
        assert_eq!(style.stroke.width, Length::from_mm(2.5));
    }
    assert_eq!(
        session.style_panel_state().stroke.width,
        BarValue::Uniform(Length::from_mm(2.5))
    );
}

#[test]
fn an_invalid_value_writes_nothing_and_says_why() {
    let mut session = session_with_rectangles(1);
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "#12345678"),
        Err(StyleEntryError::HexEightDigits)
    );
    assert_eq!(
        session.set_style_text(StyleField::FillOpacity, "101"),
        Err(StyleEntryError::Percent)
    );
    assert_eq!(stored_styles(&session)[0], Style::default());
}

#[test]
fn a_style_change_keeps_the_primitive_a_primitive() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_cap(LineCap::Round);
    session.set_fill_mode(FillMode::Solid);
    let bytes = session.pack("0.1.0").unwrap();
    let document = unpack(9, &bytes).unwrap();
    let id = document.object_ids()[0];
    assert!(document.primitive(id).is_some(), "criterion 2");
}

#[test]
fn the_stroke_switch_dash_join_and_fill_row_commit_discretely() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_paint(false);
    assert!(session.style_panel_state().stroke.all_off);
    assert!(!stored_styles(&session)[0].stroke.enabled);
    session.set_stroke_dash(DashChoice::Dash);
    session.set_stroke_dash(DashChoice::Custom);
    assert_eq!(
        stored_styles(&session)[0].stroke.dash.as_slice(),
        &[6.0, 4.0],
        "Custom is not a choice and writes nothing"
    );
    session.set_fill_mode(FillMode::Solid);
    assert_eq!(
        session.style_panel_state().fill.mode,
        BarValue::Uniform(FillMode::Solid)
    );
    session.set_fill_mode(FillMode::None);
    assert_eq!(
        session.style_panel_state().fill.mode,
        BarValue::Uniform(FillMode::None)
    );
}

#[test]
fn a_drag_previews_in_the_draw_list_and_writes_once_on_release() {
    let mut session = session_with_rectangles(1);
    let before = session.draw_list();
    assert!(!has_alpha(&before, 102));

    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.preview_style_opacity(StyleField::StrokeOpacity, 60.0);
    assert!(has_alpha(&session.draw_list(), 153), "drawn at 60%");
    assert_eq!(
        stored_styles(&session)[0].stroke.opacity,
        Opacity::OPAQUE,
        "nothing stored while dragging"
    );
    assert_eq!(
        session.style_panel_state().stroke.opacity,
        BarValue::Uniform(percent(60)),
        "the field follows the drag"
    );

    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0].stroke.opacity, percent(60));
    assert!(has_alpha(&session.draw_list(), 153));
}

#[test]
fn escape_during_a_drag_reverts_and_the_release_writes_nothing() {
    let mut session = session_with_rectangles(1);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.cancel_style_preview();
    session.preview_style_opacity(StyleField::StrokeOpacity, 30.0);
    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0], Style::default());
    assert!(!has_alpha(&session.draw_list(), 77));
}

#[test]
fn a_pending_drag_commits_to_the_objects_it_started_on() {
    let mut session = session_with_rectangles(2);
    // The second rectangle is selected; start a drag on it, then the tool
    // changes before the release.
    session.preview_style_opacity(StyleField::StrokeOpacity, 50.0);
    session.set_tool(Tool::Select);
    let styles = stored_styles(&session);
    assert_eq!(styles[0].stroke.opacity, Opacity::OPAQUE);
    assert_eq!(styles[1].stroke.opacity, percent(50));
}

#[test]
fn the_node_tool_edits_the_selected_path() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    session.pointer_down(Point::new(50.0, 0.0), false);
    session.pointer_up(Point::new(50.0, 0.0), false, false);
    session.finish_pen();
    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    assert_eq!(session.style_panel_state().subject, "Path");
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "#F80"),
        Ok(true)
    );
    let style = &stored_styles(&session)[0];
    assert_eq!((style.stroke.color.r, style.stroke.color.g), (0xFF, 0x88));
}

#[test]
fn a_panel_toggle_does_not_move_the_document_on_screen() {
    let mut session = session_with_rectangles(1);
    session.resize_viewport(1000.0, 600.0);
    let corner = session.view().screen_to_document(0.0, 0.0);
    session.keep_view_origin_for_width_change(-280.0);
    session.resize_viewport(720.0, 600.0);
    assert_eq!(session.view().screen_to_document(0.0, 0.0), corner);
    session.keep_view_origin_for_width_change(280.0);
    session.resize_viewport(1000.0, 600.0);
    assert_eq!(session.view().screen_to_document(0.0, 0.0), corner);
}
