//! `Session`-level checks of the Style panel (`specs/0007-stroke-and-fill-styling`
//! PR 3, criteria 2, 5, 13, 24, 36, 37): the panel edits what the tool's scope
//! says, a drag previews without writing and commits once on release, and the
//! preview is drawn in place of the stored style.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Color, Document, Length, LineCap, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::DrawList;
use curvyo_ui_core::{BarValue, DashChoice, StyleEntryError, StyleField};

/// A colour-area tick for `color`, through the same call the panel makes.
fn preview_colour(session: &mut Session, field: StyleField, color: Color) {
    let hsv = curvyo_ui_core::rgb_to_hsv(color);
    session.preview_style_hsv(field, hsv.hue.unwrap_or(0.0), hsv.saturation, hsv.value);
}

/// An opacity drag tick for `percent`, through the value-field call.
fn preview_opacity(session: &mut Session, field: StyleField, percent: f64) {
    let (value_field, scale) = match field {
        StyleField::StrokeOpacity => (
            curvyo_ui_core::ValueField::StrokeOpacity,
            curvyo_ui_core::ValueScale::Opacity,
        ),
        StyleField::FillOpacity => (
            curvyo_ui_core::ValueField::FillOpacity,
            curvyo_ui_core::ValueScale::Opacity,
        ),
        _ => return,
    };
    session.preview_value_field(
        value_field,
        scale.position_of(percent),
        curvyo_ui_core::Grid::Normal,
    );
}

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

fn loro_of(session: &Session) -> loro::LoroDoc {
    let document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
    let loro = loro::LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro
}

/// How many commits the session's document holds, counted from its Loro
/// snapshot (the way `acceptance_unified_editing.rs` counts one interaction).
/// Consecutive commits of one peer with the same label can merge into one
/// change, so a count of one is only proof where the last change before the
/// gesture has another label; `op_count` is the check that cannot merge away.
fn change_count(session: &Session) -> usize {
    loro_of(session).len_changes()
}

/// How many operations the document has recorded: a gesture that wrote once per
/// tick would record more than one that wrote once on release.
fn op_count(session: &Session) -> i32 {
    loro_of(session).oplog_vv().values().copied().sum()
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
    let state = session.style_panel_state().unwrap();
    assert_eq!(state.subject, "Rectangle");
}

#[test]
fn the_pen_and_an_empty_selection_disable_the_panel() {
    let mut session = session_with_rectangles(1);
    session.set_tool(Tool::Pen);
    assert_eq!(
        session.style_panel_state(),
        None,
        "the Pen leaves nothing to edit (criterion 1)"
    );
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
    assert_eq!(session.style_panel_state().unwrap().subject, "3 rectangles");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "2,5"),
        Ok(true)
    );
    for style in stored_styles(&session) {
        assert_eq!(style.stroke.width, Length::from_mm(2.5));
    }
    assert_eq!(
        session.style_panel_state().unwrap().stroke.width,
        BarValue::Uniform(Length::from_mm(2.5))
    );
}

#[test]
fn an_invalid_value_writes_nothing_and_says_why() {
    let mut session = session_with_rectangles(1);
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "#1234567"),
        Err(StyleEntryError::Hex)
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
    session.set_fill_paint(true);
    let bytes = session.pack("0.1.0").unwrap();
    let document = unpack(9, &bytes).unwrap();
    let id = document.object_ids()[0];
    assert!(document.primitive(id).is_some(), "criterion 2");
}

#[test]
fn the_stroke_switch_dash_join_and_fill_row_commit_discretely() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_paint(false);
    assert!(!session.style_panel_state().unwrap().stroke.rows_shown);
    assert!(!stored_styles(&session)[0].stroke.enabled);
    session.set_stroke_dash(DashChoice::Dash);
    assert_eq!(
        stored_styles(&session)[0].stroke.dash.as_slice(),
        &[6.0, 4.0]
    );
    session.set_fill_paint(true);
    assert_eq!(
        session.style_panel_state().unwrap().fill.paint,
        BarValue::Uniform(true)
    );
    session.set_fill_paint(false);
    assert_eq!(
        session.style_panel_state().unwrap().fill.paint,
        BarValue::Uniform(false)
    );
}

#[test]
fn a_drag_previews_in_the_draw_list_and_writes_once_on_release() {
    let mut session = session_with_rectangles(1);
    let before = session.draw_list();
    assert!(!has_alpha(&before, 102));

    preview_opacity(&mut session, StyleField::StrokeOpacity, 40.0);
    preview_opacity(&mut session, StyleField::StrokeOpacity, 60.0);
    assert!(has_alpha(&session.draw_list(), 153), "drawn at 60%");
    assert_eq!(
        stored_styles(&session)[0].stroke.opacity,
        Opacity::OPAQUE,
        "nothing stored while dragging"
    );
    assert_eq!(
        session.style_panel_state().unwrap().stroke.opacity,
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
    preview_opacity(&mut session, StyleField::StrokeOpacity, 40.0);
    session.cancel_style_preview();
    preview_opacity(&mut session, StyleField::StrokeOpacity, 30.0);
    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0], Style::default());
    assert!(!has_alpha(&session.draw_list(), 77));
}

#[test]
fn a_pending_drag_commits_to_the_objects_it_started_on() {
    let mut session = session_with_rectangles(2);
    // The second rectangle is selected; start a drag on it, then the tool
    // changes before the release.
    preview_opacity(&mut session, StyleField::StrokeOpacity, 50.0);
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
    assert_eq!(session.style_panel_state().unwrap().subject, "Path");
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

#[test]
fn a_drag_of_many_ticks_records_what_one_typed_value_records() {
    let mut dragged = session_with_rectangles(1);
    let mut typed = session_with_rectangles(1);
    let (n_dragged, n_typed) = (op_count(&dragged), op_count(&typed));
    let changes = change_count(&dragged);
    for percent in [90.0, 80.0, 70.0, 60.0, 50.0] {
        preview_opacity(&mut dragged, StyleField::StrokeOpacity, percent);
    }
    assert_eq!(op_count(&dragged), n_dragged, "ticks write nothing");
    dragged.commit_style_preview();
    typed
        .set_style_text(StyleField::StrokeOpacity, "50")
        .unwrap();
    let written = op_count(&dragged) - n_dragged;
    assert!(written > 0);
    assert_eq!(
        written,
        op_count(&typed) - n_typed,
        "one write of one value"
    );
    assert_eq!(change_count(&dragged), changes + 1);
    dragged.commit_style_preview();
    assert_eq!(
        op_count(&dragged) - n_dragged,
        written,
        "a second release is a no-op"
    );
}

#[test]
fn escape_then_the_release_records_nothing() {
    let mut session = session_with_rectangles(1);
    let before = op_count(&session);
    preview_opacity(&mut session, StyleField::StrokeOpacity, 40.0);
    session.cancel_style_preview();
    preview_opacity(&mut session, StyleField::StrokeOpacity, 30.0);
    session.commit_style_preview();
    assert_eq!(op_count(&session), before);
}

#[test]
fn an_edit_over_three_objects_is_one_commit_of_three_writes() {
    let written = |n: u32, drag: bool| {
        let mut session = session_with_rectangles(n);
        select_first(&mut session, n);
        let (ops, changes) = (op_count(&session), change_count(&session));
        if drag {
            preview_colour(
                &mut session,
                StyleField::StrokeColor,
                Color { r: 1, g: 2, b: 3 },
            );
            preview_colour(
                &mut session,
                StyleField::StrokeColor,
                Color { r: 4, g: 5, b: 6 },
            );
            session.commit_style_preview();
        } else {
            session
                .set_style_text(StyleField::StrokeColor, "#040506")
                .unwrap();
        }
        assert_eq!(change_count(&session), changes + 1, "one commit");
        for style in stored_styles(&session) {
            assert_eq!(style.stroke.color, Color { r: 4, g: 5, b: 6 });
        }
        op_count(&session) - ops
    };
    let one = written(1, false);
    assert!(one > 0);
    assert_eq!(written(3, false), 3 * one, "typed");
    assert_eq!(written(3, true), 3 * one, "dragged");
}

/// Criterion 44: a drag over objects with different values sets all of them to
/// the value under the pointer. The panel stops reporting Mixed after the first
/// tick (the preview makes the objects equal), so the host must latch Mixed at
/// the press; the session itself maps each `p` to its value, whatever the
/// ticks before.
#[test]
fn a_drag_over_mixed_values_sets_every_object_to_the_value_of_the_pointer_position() {
    let mut session = session_with_rectangles(3);
    session.set_tool(Tool::Select);
    for (k, percent_value) in [(0_u32, 20.0), (1, 50.0), (2, 80.0)] {
        let edge = Point::new(f64::from(k) * 30.0, 5.0);
        session.pointer_hover(edge, false, false);
        session.pointer_down(edge, false);
        session.pointer_up(edge, false, false);
        preview_opacity(&mut session, StyleField::StrokeOpacity, percent_value);
        session.commit_style_preview();
    }
    select_first(&mut session, 3);
    assert_eq!(
        session.style_panel_state().unwrap().stroke.opacity,
        BarValue::Mixed
    );

    let scale = curvyo_ui_core::ValueScale::Opacity;
    let at = |session: &mut Session, p: f64| {
        session.preview_value_field(
            curvyo_ui_core::ValueField::StrokeOpacity,
            p,
            curvyo_ui_core::Grid::Normal,
        );
    };
    at(&mut session, 0.1);
    // One tick makes the objects equal: this is why Mixed is latched by the host.
    assert!(matches!(
        session.style_panel_state().unwrap().stroke.opacity,
        BarValue::Uniform(_)
    ));
    at(&mut session, 167.0 / 244.0);
    session.commit_style_preview();
    let wanted = scale.round(scale.value_at(167.0 / 244.0), curvyo_ui_core::Grid::Normal);
    assert_eq!(wanted, 53.0);
    for style in stored_styles(&session) {
        assert_eq!(style.stroke.opacity, percent(53));
    }
}
