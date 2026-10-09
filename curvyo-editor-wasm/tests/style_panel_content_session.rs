//! `Session`-level checks of the reworked Style panel
//! (`specs/0017-style-panel-rework`): rows follow the committed Paint state,
//! the hex field edits RGB and alpha, mixed Paint turns a stroke on only for
//! the edits that say so, the pattern line and the picker's colour preview.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Color, Document, Length, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{BarValue, DashChoice, DashShown, StyleEntryError, StyleField};

/// A session with `n` 10 mm rectangles in a row.
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

/// Selects the first `n` rectangles: a click on the edge of the first,
/// Shift-clicks on the others.
fn select_first(session: &mut Session, n: u32) {
    session.set_tool(Tool::Select);
    for k in 0..n {
        let edge = Point::new(f64::from(k) * 30.0, 5.0);
        session.pointer_hover(edge, k > 0, false);
        session.pointer_down(edge, k > 0);
        session.pointer_up(edge, k > 0, false);
    }
}

fn stored_styles(session: &Session) -> Vec<Style> {
    let document: Document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
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

fn percent(n: u32) -> Opacity {
    Opacity::new(f64::from(n) / 100.0).unwrap()
}

// ---- Paint None hides the rows (criteria 5 to 8) ---------------------------

#[test]
fn stroke_rows_leave_with_paint_none_and_come_back_with_every_value() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::StrokeWidth, "2")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#FF000080")
        .unwrap();
    session.set_stroke_dash(DashChoice::Dot);
    session.set_stroke_join(curvyo_document_core::LineJoin::Round);
    let before = stored_styles(&session)[0].clone();

    session.set_stroke_paint(false);
    assert!(!session.style_panel_state().unwrap().stroke.rows_shown);
    session.set_stroke_paint(true);
    let state = session.style_panel_state().unwrap();
    assert!(state.stroke.rows_shown);
    assert_eq!(stored_styles(&session)[0], before, "criterion 7");
}

#[test]
fn width_zero_turns_the_stroke_off_in_the_same_commit_and_solid_restores_the_width() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::StrokeWidth, "3")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeWidth, "0")
        .unwrap();
    let stored = &stored_styles(&session)[0];
    assert!(!stored.stroke.enabled);
    assert_eq!(stored.stroke.width, Length::from_mm(3.0), "last width kept");
    assert!(!session.style_panel_state().unwrap().stroke.rows_shown);
    session.set_stroke_paint(true);
    assert_eq!(
        stored_styles(&session)[0].stroke.width,
        Length::from_mm(3.0)
    );
}

// ---- 8-digit hex (criteria 11 to 15) ---------------------------------------

#[test]
fn eight_digit_hex_sets_rgb_and_alpha_and_the_view_shows_eight_digits() {
    let mut session = session_with_rectangles(1);
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "2F6FEE80"),
        Ok(true)
    );
    let stroke = &stored_styles(&session)[0].stroke;
    assert_eq!(
        stroke.color,
        Color {
            r: 0x2F,
            g: 0x6F,
            b: 0xEE
        }
    );
    assert_eq!(stroke.opacity.get(), 128.0 / 255.0, "stored as AA / 255");
    let view = session.style_panel_view();
    assert_eq!(view.stroke_hex, "#2F6FEE80");
    assert_eq!(
        view.stroke_opacity.round(),
        50.0,
        "the Opacity field shows 50"
    );

    // Typing 50 in Opacity afterwards stores exactly 0.5; the hex still reads 80.
    session
        .set_style_text(StyleField::StrokeOpacity, "50")
        .unwrap();
    assert_eq!(stored_styles(&session)[0].stroke.opacity, percent(50));
    assert_eq!(session.style_panel_view().stroke_hex, "#2F6FEE80");
}

#[test]
fn six_digit_hex_keeps_the_alpha_and_four_digit_hex_sets_it() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::StrokeOpacity, "40")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#F80")
        .unwrap();
    let stroke = &stored_styles(&session)[0].stroke;
    assert_eq!(
        stroke.color,
        Color {
            r: 0xFF,
            g: 0x88,
            b: 0x00
        }
    );
    assert_eq!(
        stroke.opacity,
        percent(40),
        "3 digits leave the alpha alone"
    );
    session
        .set_style_text(StyleField::StrokeColor, "#F80C")
        .unwrap();
    assert_eq!(
        stored_styles(&session)[0].stroke.opacity.get(),
        204.0 / 255.0
    );
}

#[test]
fn the_hex_field_is_mixed_when_any_rgba_value_differs() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session
        .set_style_text(StyleField::StrokeColor, "#112233")
        .unwrap();
    select_first(&mut session, 2);
    let view = session.style_panel_view();
    assert!(view.stroke_hex_mixed && view.stroke_color_mixed);
    // 6 digits set the RGB of both and each keeps its alpha; one commit.
    session
        .set_style_text(StyleField::StrokeColor, "#445566")
        .unwrap();
    for style in stored_styles(&session) {
        assert_eq!(
            style.stroke.color,
            Color {
                r: 0x44,
                g: 0x55,
                b: 0x66
            }
        );
    }
    assert!(!session.style_panel_view().stroke_hex_mixed);
}

#[test]
fn a_fill_colour_edit_never_turns_the_fill_on_and_a_stroke_colour_edit_turns_the_stroke_on() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::FillColor, "2F6FEE80")
        .unwrap();
    let style = stored_styles(&session)[0].clone();
    assert!(!style.fill.enabled, "criterion 15");
    assert_eq!(style.fill.opacity.get(), 128.0 / 255.0);

    session.set_stroke_paint(false);
    session
        .set_style_text(StyleField::StrokeColor, "#00FF00")
        .unwrap();
    assert!(stored_styles(&session)[0].stroke.enabled, "criterion 9");
}

// ---- mixed Paint (criterion 9) --------------------------------------------

#[test]
fn mixed_paint_shows_the_rows_and_colour_width_edits_turn_the_off_ones_on() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session.set_stroke_paint(false);
    select_first(&mut session, 2);
    let state = session.style_panel_state().unwrap();
    assert_eq!(state.stroke.paint, BarValue::Mixed);
    assert!(state.stroke.rows_shown);

    // Dash, Join and Cap change the stored value and no on/off state.
    session.set_stroke_dash(DashChoice::Dash);
    let styles = stored_styles(&session);
    assert!(!styles[0].stroke.enabled && styles[1].stroke.enabled);
    assert!(
        styles
            .iter()
            .all(|s| s.stroke.dash.as_slice() == [6.0, 4.0])
    );

    // A width edit turns the off one on, in the same commit.
    session
        .set_style_text(StyleField::StrokeWidth, "2")
        .unwrap();
    let styles = stored_styles(&session);
    assert!(styles.iter().all(|s| s.stroke.enabled));
}

#[test]
fn fill_edits_leave_an_off_fill_off_in_a_mixed_selection() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session.set_fill_paint(true);
    select_first(&mut session, 2);
    assert_eq!(
        session.style_panel_state().unwrap().fill.paint,
        BarValue::Mixed
    );
    session
        .set_style_text(StyleField::FillOpacity, "30")
        .unwrap();
    let styles = stored_styles(&session);
    assert!(styles[0].fill.enabled && !styles[1].fill.enabled);
    assert!(styles.iter().all(|s| s.fill.opacity == percent(30)));
}

// ---- the picker's colour (criteria 18, 21) ---------------------------------

#[test]
fn an_hsv_drag_previews_the_rgb_and_commits_once_keeping_the_alpha() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::StrokeOpacity, "50")
        .unwrap();
    session.preview_style_hsv(StyleField::StrokeColor, 0.0, 1.0, 1.0);
    session.preview_style_hsv(StyleField::StrokeColor, 120.0, 1.0, 1.0);
    assert_eq!(
        stored_styles(&session)[0].stroke.color,
        Color::BLACK,
        "nothing stored while dragging"
    );
    assert_eq!(
        session.style_panel_view().stroke_color,
        0x0000_FF00,
        "the field follows the drag"
    );
    session.commit_style_preview();
    let stroke = &stored_styles(&session)[0].stroke;
    assert_eq!(stroke.color, Color { r: 0, g: 255, b: 0 });
    assert_eq!(stroke.opacity, percent(50), "criterion 18: RGB only");
}

// ---- the pattern line (criteria 29 to 33) ----------------------------------

#[test]
fn the_pattern_line_stores_the_list_as_typed_and_the_view_shows_it_back() {
    let mut session = session_with_rectangles(1);
    assert_eq!(session.set_stroke_dash_text("1   2 4  2"), Ok(true));
    let stored = &stored_styles(&session)[0].stroke.dash;
    assert_eq!(stored.as_slice(), [1.0, 2.0, 4.0, 2.0]);
    let view = session.style_panel_view();
    assert_eq!(view.stroke_dash_text, "1 2 4 2");
    assert_eq!(view.stroke_dash, "none", "no preset equals it");

    assert_eq!(session.set_stroke_dash_text("1 2 4"), Ok(true));
    assert_eq!(
        stored_styles(&session)[0].stroke.dash.as_slice(),
        [1.0, 2.0, 4.0],
        "an odd list is stored as typed"
    );
    // A preset text presses its button.
    assert_eq!(session.set_stroke_dash_text("6 4"), Ok(true));
    assert_eq!(session.style_panel_view().stroke_dash, "dash");
    // Empty text is solid.
    assert_eq!(session.set_stroke_dash_text(""), Ok(true));
    assert_eq!(session.style_panel_view().stroke_dash, "solid");
}

#[test]
fn a_refused_pattern_writes_nothing() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_dash(DashChoice::Dash);
    for text in [
        "1,2",
        "0 0",
        "1 2 x",
        "1001 1",
        "1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1",
    ] {
        assert_eq!(
            session.set_stroke_dash_text(text),
            Err(StyleEntryError::Dash),
            "{text}"
        );
    }
    assert_eq!(
        stored_styles(&session)[0].stroke.dash.as_slice(),
        [6.0, 4.0]
    );
}

#[test]
fn different_lists_show_mixed_and_a_preset_press_sets_all() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session.set_stroke_dash(DashChoice::Dot);
    select_first(&mut session, 2);
    let view = session.style_panel_view();
    assert_eq!(view.stroke_dash, "mixed");
    assert_eq!(
        session.style_panel_state().unwrap().stroke.dash,
        DashShown::Mixed
    );
    session.set_stroke_dash(DashChoice::DashDot);
    assert!(
        stored_styles(&session)
            .iter()
            .all(|s| s.stroke.dash.as_slice() == [6.0, 3.0, 1.0, 3.0])
    );
}

#[test]
fn a_list_of_more_than_sixteen_numbers_from_a_file_is_shown_in_full_and_not_rewritten() {
    // The editing limit is 16; a file may hold more (criterion 33).
    let document = Document::new(1);
    let id = document.create_rect(curvyo_document_core::RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let long: Vec<f64> = (1..=20).map(f64::from).collect();
    document
        .edit_style(
            &[id],
            &curvyo_document_core::StyleEdit::StrokeDash(
                curvyo_document_core::DashPattern::new(long).unwrap(),
            ),
        )
        .unwrap();
    let mut session =
        Session::open(2, &curvyo_document_core::pack(&document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    let edge = Point::new(0.0, 5.0);
    session.pointer_hover(edge, false, false);
    session.pointer_down(edge, false);
    session.pointer_up(edge, false, false);
    let text = session.style_panel_view().stroke_dash_text;
    assert_eq!(text.split(' ').count(), 20);
}

// ---- value fields (criteria 36 to 47, 61) ----------------------------------

use curvyo_ui_core::{Grid, ValueField, ValueScale};

/// How many operations the document has recorded: a gesture that wrote once per
/// tick would record more than one that wrote once on release.
fn op_count(session: &Session) -> i32 {
    let document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
    let loro = loro::LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro.oplog_vv().values().copied().sum()
}

#[test]
fn a_width_drag_previews_per_tick_and_commits_once_on_release() {
    let mut session = session_with_rectangles(1);
    let ops = op_count(&session);
    for p in [0.2, 0.4, 0.3] {
        session.preview_value_field(ValueField::StrokeWidth, p, Grid::Normal);
    }
    assert_eq!(op_count(&session), ops, "nothing written while dragging");
    let shown = session.style_panel_view();
    let want = ValueScale::StrokeWidth.round(ValueScale::StrokeWidth.value_at(0.3), Grid::Normal);
    assert_eq!(shown.stroke_width, want, "the field follows the drag");
    assert_eq!(shown.stroke_width_text, ValueScale::StrokeWidth.text(want));
    session.commit_style_preview();
    assert_eq!(
        stored_styles(&session)[0].stroke.width,
        Length::from_mm(want)
    );
}

#[test]
fn a_drag_that_ends_where_it_began_commits_nothing() {
    let mut session = session_with_rectangles(1);
    let ops = op_count(&session);
    session.preview_value_field(ValueField::StrokeOpacity, 0.5, Grid::Normal);
    session.preview_value_field(ValueField::StrokeOpacity, 1.0, Grid::Normal);
    session.commit_style_preview();
    assert_eq!(op_count(&session), ops);
}

#[test]
fn escape_during_a_drag_restores_the_value_and_the_release_writes_nothing() {
    let mut session = session_with_rectangles(1);
    let ops = op_count(&session);
    session.preview_value_field(ValueField::StrokeOpacity, 0.3, Grid::Normal);
    session.cancel_style_preview();
    assert_eq!(session.style_panel_view().stroke_opacity, 100.0);
    // Further ticks after Escape are ignored until the release.
    session.preview_value_field(ValueField::StrokeOpacity, 0.2, Grid::Normal);
    assert_eq!(session.style_panel_view().stroke_opacity, 100.0);
    session.commit_style_preview();
    assert_eq!(op_count(&session), ops);
}

#[test]
fn dragging_the_width_to_zero_keeps_the_row_until_the_release() {
    let mut session = session_with_rectangles(1);
    session.preview_value_field(ValueField::StrokeWidth, 0.0, Grid::Normal);
    let state = session.style_panel_state().unwrap();
    assert!(
        state.stroke.rows_shown,
        "criterion 8: the row stays mid-drag"
    );
    assert_eq!(session.style_panel_view().stroke_width_text, "0");
    session.commit_style_preview();
    let stroke = stored_styles(&session)[0].stroke.clone();
    assert!(!stroke.enabled);
    assert_eq!(
        stroke.width,
        Length::from_mm(0.25),
        "the last width is kept"
    );
    assert!(!session.style_panel_state().unwrap().stroke.rows_shown);
}

#[test]
fn the_arrow_keys_step_on_the_grid_and_a_held_key_is_one_commit() {
    let mut session = session_with_rectangles(1);
    let ops = op_count(&session);
    for _ in 0..3 {
        session.step_value_field(ValueField::StrokeWidth, 1, Grid::Normal);
    }
    assert_eq!(op_count(&session), ops);
    assert_eq!(session.style_panel_view().stroke_width_text, "0.28");
    session.commit_style_preview();
    assert_eq!(
        stored_styles(&session)[0].stroke.width,
        Length::from_mm(0.28)
    );
    session.step_value_field(ValueField::StrokeOpacity, -1, Grid::Coarse);
    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0].stroke.opacity, percent(90));
}

#[test]
fn arrow_keys_do_nothing_while_the_values_differ() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session
        .set_style_text(StyleField::StrokeWidth, "2")
        .unwrap();
    select_first(&mut session, 2);
    assert!(session.style_panel_view().stroke_width_mixed);
    session.step_value_field(ValueField::StrokeWidth, 1, Grid::Normal);
    session.commit_style_preview();
    let widths: Vec<Length> = stored_styles(&session)
        .iter()
        .map(|s| s.stroke.width)
        .collect();
    assert_eq!(widths, [Length::from_mm(2.0), Length::from_mm(0.25)]);
    // A drag then sets all of them to the value under the pointer.
    session.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    session.commit_style_preview();
    let widths: Vec<Length> = stored_styles(&session)
        .iter()
        .map(|s| s.stroke.width)
        .collect();
    assert_eq!(widths[0], widths[1]);
    assert_eq!(widths[0], Length::from_mm(1.82));
}

#[test]
fn reset_sets_every_object_to_the_default_in_one_commit_and_does_nothing_at_the_default() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session
        .set_style_text(StyleField::StrokeWidth, "3")
        .unwrap();
    select_first(&mut session, 2);
    let view = session.style_panel_view();
    assert!(view.stroke_width_resettable, "mixed shows the icon");
    assert!(!view.stroke_opacity_resettable, "at the default: hidden");
    assert!(session.reset_value_field(ValueField::StrokeWidth));
    assert!(
        stored_styles(&session)
            .iter()
            .all(|s| s.stroke.width == Length::from_mm(0.25))
    );
    assert!(!session.style_panel_view().stroke_width_resettable);
    let ops = op_count(&session);
    session.reset_value_field(ValueField::StrokeWidth);
    assert_eq!(op_count(&session), ops, "nothing written at the default");
}

#[test]
fn a_reset_of_the_width_is_an_edit_that_turns_an_off_stroke_on() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_paint(false);
    // An off stroke keeps its width 0.25: nothing differs, so nothing is written.
    assert!(session.reset_value_field(ValueField::StrokeWidth));
    assert!(stored_styles(&session)[0].stroke.enabled);
}

#[test]
fn the_view_carries_bar_text_and_a_full_bar_above_the_drag_range() {
    let mut session = session_with_rectangles(1);
    let view = session.style_panel_view();
    assert_eq!(view.stroke_width_text, "0.25");
    assert!((view.stroke_width_bar * 244.0 - 43.0).abs() < 0.6);
    assert_eq!(view.stroke_opacity_bar, 1.0);
    session
        .set_style_text(StyleField::StrokeWidth, "500")
        .unwrap();
    let view = session.style_panel_view();
    assert_eq!(view.stroke_width_bar, 1.0);
    assert_eq!(view.stroke_width_text, "500");
}
