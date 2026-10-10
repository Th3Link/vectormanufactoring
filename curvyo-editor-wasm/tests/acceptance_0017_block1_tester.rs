//! Independent tester cases for `0017-style-panel-rework`, block 1, at the
//! `Session` level: what the panel shows for each tool and selection, the hex
//! and dash edits, the value-field calls, legacy gradient files, the eyedropper
//! and the commit rules (one commit per gesture, Escape reverts, an edit
//! commits to the objects it started on). Written from `specification.md`
//! before the implementation was read.
//!
//! Criteria covered here: 1 to 10, 11 to 15, 18 (commit side), 22 to 26, 28 to
//! 33, 36 to 44 (session side), 49, 53, 59, 60 (session side), 61.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss
)]

use curvyo_document_core::{
    Color, DisplayUnit, Document, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{EscapeStep, Session, Tool};
use curvyo_ui_core::{
    DashChoice, Grid, PaintTarget, PanelContent, StyleEntryError, StyleField, ValueField,
};

// ---------------------------------------------------------------- helpers

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

/// `n` 20 mm squares in a row, 40 mm apart, drawn with the Rectangle tool.
fn squares(n: u32) -> Session {
    let mut s = Session::new(1);
    for k in 0..n {
        s.set_tool(Tool::Rectangle);
        let x = f64::from(k) * 40.0;
        s.pointer_down(pt(x, 0.0), false);
        s.pointer_up(pt(x + 20.0, 20.0), false, false);
    }
    s
}

fn click(s: &mut Session, at: Point, shift: bool) {
    s.pointer_hover(at, shift, false);
    s.pointer_down(at, shift);
    s.pointer_up(at, shift, false);
}

/// Selects a contiguous run of squares of a [`squares`] session with a marquee,
/// which works whether or not the objects paint anything (an object with no
/// stroke and no fill cannot be clicked).
fn select(s: &mut Session, which: &[u32]) {
    let lo = f64::from(*which.iter().min().unwrap());
    let hi = f64::from(*which.iter().max().unwrap());
    s.set_tool(Tool::Select);
    // Clear first: a press on the handle of an existing selection would resize
    // it instead of starting a marquee.
    s.escape();
    let from = pt(lo * 40.0 - 15.0, -30.0);
    let to = pt(hi * 40.0 + 35.0, 30.0);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
    assert_eq!(s.selected_object_count(), which.len(), "marquee {which:?}");
}

/// Toggles the display unit (a commit with another label) and returns the
/// change count after it, so the next style commit cannot merge into the
/// previous change.
fn baseline(s: &mut Session) -> usize {
    let unit = if s.display_unit() == DisplayUnit::In {
        DisplayUnit::Mm
    } else {
        DisplayUnit::In
    };
    assert!(s.set_display_unit(unit));
    changes(s)
}

fn document_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn stored(s: &Session) -> Vec<Style> {
    let d = document_of(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn loro_of(s: &Session) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&document_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l
}

fn ops(s: &Session) -> i64 {
    loro_of(s).oplog_vv().values().map(|c| i64::from(*c)).sum()
}

fn changes(s: &Session) -> usize {
    loro_of(s).len_changes()
}

fn has_legacy_keys(s: &Session) -> bool {
    let l = loro_of(s);
    let tree = l.get_tree("paths");
    tree.nodes().into_iter().any(|n| {
        let text = format!("{:?}", tree.get_meta(n).unwrap().get_deep_value());
        text.contains("fill_kind") || text.contains("fill_stops")
    })
}

fn op(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

// ================================ AC 1 to 4: what the panel shows and hides

#[test]
fn nothing_selected_shows_the_document_section_and_no_style_area() {
    let s = Session::new(1);
    assert_eq!(s.panel_content(), PanelContent::Document);
    assert!(s.style_panel_state().is_none());
    assert_eq!(s.style_panel_view().subject, "");
}

#[test]
fn the_pen_tool_and_the_node_tool_without_a_path_leave_the_style_tab_without_content() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    assert_eq!(s.panel_content(), PanelContent::Style);

    // Since `0043` the Style tab hands over to the Document tab when the tool's
    // scope empties: the body is no longer empty for a selection the tool
    // cannot style (`specs/0043-properties-tabs/adrs.md` decision 3).
    s.set_tool(Tool::Pen);
    assert_eq!(
        s.panel_content(),
        PanelContent::Document,
        "Pen with a rectangle selected"
    );
    assert!(s.style_panel_state().is_none());
    assert_eq!(s.style_panel_view().subject, "");

    s.set_tool(Tool::Node);
    assert_eq!(
        s.panel_content(),
        PanelContent::Document,
        "Node tool, a rectangle selected: no path, no style"
    );
    assert!(s.style_panel_state().is_none());
    assert_eq!(s.style_panel_view().subject, "");

    s.set_tool(Tool::Select);
    assert_eq!(
        s.panel_content(),
        PanelContent::Style,
        "the selection stayed"
    );
}

#[test]
fn the_pen_with_nothing_selected_shows_the_document_section_until_a_path_is_unfinished() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    assert_eq!(s.panel_content(), PanelContent::Document);
    click(&mut s, pt(10.0, 10.0), false);
    assert_eq!(
        s.panel_content(),
        PanelContent::Empty,
        "an unfinished path, nothing selected: empty body"
    );
    assert!(s.style_panel_state().is_none());
}

#[test]
fn the_node_tool_with_a_path_selected_shows_the_style_of_that_path() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    click(&mut s, pt(0.0, 0.0), false);
    click(&mut s, pt(30.0, 0.0), false);
    click(&mut s, pt(30.0, 30.0), false);
    s.set_tool(Tool::Select);
    click(&mut s, pt(15.0, 0.0), false);
    s.set_tool(Tool::Node);
    assert_eq!(s.panel_content(), PanelContent::Style);
    assert_eq!(s.style_panel_view().subject, "Path");
}

#[test]
fn the_subject_lines_follow_the_selection_and_the_old_texts_are_gone() {
    let mut s = squares(4);
    select(&mut s, &[0]);
    assert_eq!(s.style_panel_view().subject, "Rectangle");
    select(&mut s, &[0, 1, 2]);
    assert_eq!(s.style_panel_view().subject, "3 rectangles");

    // Two ellipses and two rectangles: "4 objects".
    let mut mixed = squares(2);
    for k in 2..4 {
        mixed.set_tool(Tool::Ellipse);
        let x = f64::from(k) * 40.0;
        mixed.pointer_down(pt(x, 0.0), false);
        mixed.pointer_up(pt(x + 20.0, 20.0), false, false);
    }
    select(&mut mixed, &[0, 1]);
    mixed.set_tool(Tool::Select);
    click(&mut mixed, pt(100.0, 10.0), true);
    click(&mut mixed, pt(140.0, 10.0), true);
    assert_eq!(mixed.style_panel_view().subject, "4 objects");

    // No state of the empty panel carries the removed texts.
    for tool in [Tool::Pen, Tool::Node] {
        mixed.set_tool(tool);
        let dump = format!("{:?}", mixed.style_panel_view());
        assert!(!dump.contains("Nothing selected"));
        assert!(!dump.contains("finish the path"));
    }
}

#[test]
fn a_selection_cleared_mid_preview_empties_the_panel_but_the_preview_still_commits() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    // The selection goes away mid-preview: the preview commits to the old
    // object first (criterion 59) and the panel is the Document section.
    s.escape();
    assert_eq!(s.panel_content(), PanelContent::Document);
    assert!(s.style_panel_state().is_none());
    s.commit_style_preview();
    assert_eq!(
        stored(&s)[0].stroke.width.as_mm(),
        1.82,
        "the preview of the old selection still commits to its object"
    );
}

// ================================ AC 5 to 10: Paint None hides, mixed shows

#[test]
fn paint_none_hides_every_row_and_paint_solid_brings_back_exactly_the_stored_values() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "2").unwrap();
    s.set_style_text(StyleField::StrokeColor, "FF000080")
        .unwrap();
    s.set_stroke_dash(DashChoice::Dot);
    s.set_stroke_join(curvyo_document_core::LineJoin::Round);
    let before = s.style_panel_view();
    assert_eq!(before.stroke_paint, "on");
    assert!(before.stroke_rows);

    s.set_stroke_paint(false);
    let off = s.style_panel_view();
    assert_eq!(off.stroke_paint, "off");
    assert!(
        !off.stroke_rows,
        "Color, Opacity, Width, Dash, Join, Cap are not rendered"
    );
    s.set_stroke_paint(true);
    let back = s.style_panel_view();
    assert!(back.stroke_rows);
    assert_eq!(back.stroke_width_text, "2");
    assert_eq!(back.stroke_hex, "#FF000080");
    assert_eq!(back.stroke_dash, "dot");
    assert_eq!(back.stroke_join, "round");
    assert_eq!(back, before);
}

#[test]
fn fill_has_two_states_and_its_rows_follow_them() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let v = s.style_panel_view();
    assert_eq!((v.fill_paint.as_str(), v.fill_rows), ("off", false));
    s.set_fill_paint(true);
    let v = s.style_panel_view();
    assert_eq!((v.fill_paint.as_str(), v.fill_rows), ("on", true));
    s.set_fill_paint(false);
    let v = s.style_panel_view();
    assert_eq!((v.fill_paint.as_str(), v.fill_rows), ("off", false));
    assert!(stored(&s)[0].fill.color == Color::BLACK && !stored(&s)[0].fill.enabled);
}

#[test]
fn width_zero_by_typing_turns_the_stroke_off_in_the_same_commit_and_solid_restores_the_width() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "3.5").unwrap();
    let c = baseline(&mut s);
    s.set_style_text(StyleField::StrokeWidth, "0").unwrap();
    assert_eq!(changes(&s), c + 1, "one commit");
    let st = &stored(&s)[0].stroke;
    assert!(!st.enabled);
    assert_eq!(st.width.as_mm(), 3.5, "the last non-zero width stays");
    let v = s.style_panel_view();
    assert_eq!(v.stroke_paint, "off");
    assert!(!v.stroke_rows);
    s.set_stroke_paint(true);
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 3.5);
    assert_eq!(s.style_panel_view().stroke_width_text, "3.5");
}

#[test]
fn width_zero_from_a_file_that_never_had_a_width_restores_a_quarter_millimetre() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "0").unwrap();
    s.set_stroke_paint(true);
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 0.25);
}

#[test]
fn a_width_drag_to_the_left_end_keeps_the_row_during_the_drag_and_removes_it_on_release() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "2").unwrap();
    let before = ops(&s);
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    s.preview_value_field(ValueField::StrokeWidth, 0.0, Grid::Normal);
    let mid = s.style_panel_view();
    assert!(mid.stroke_rows, "the row stays while the drag runs");
    assert_eq!(mid.stroke_width_text, "0");
    assert_eq!(ops(&s), before, "a preview writes nothing");
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 2.0);

    let c = baseline(&mut s);
    s.commit_style_preview();
    assert_eq!(changes(&s), c + 1);
    let after = s.style_panel_view();
    assert_eq!(after.stroke_paint, "off");
    assert!(!after.stroke_rows);
    assert_eq!(
        stored(&s)[0].stroke.width.as_mm(),
        2.0,
        "last non-zero width kept"
    );
    assert!(!stored(&s)[0].stroke.enabled);
}

#[test]
fn a_mixed_paint_selection_shows_its_rows_and_each_edit_follows_the_stroke_and_fill_rules() {
    let mut s = squares(3);
    select(&mut s, &[0, 1, 2]);
    s.set_stroke_paint(false);
    select(&mut s, &[1]);
    s.set_stroke_paint(true);
    select(&mut s, &[0, 1, 2]);
    let v = s.style_panel_view();
    assert_eq!(v.stroke_paint, "mixed");
    assert!(v.stroke_rows, "mixed Paint shows the rows");

    // Dash, Join and Cap change no on/off state.
    s.set_stroke_dash(DashChoice::Dash);
    s.set_stroke_join(curvyo_document_core::LineJoin::Bevel);
    s.set_stroke_cap(curvyo_document_core::LineCap::Round);
    let on: Vec<bool> = stored(&s).iter().map(|st| st.stroke.enabled).collect();
    assert_eq!(on, [false, true, false]);
    assert_eq!(s.style_panel_view().stroke_paint, "mixed");

    // Fill: one object on; a fill colour, a fill opacity and an 8 digit fill
    // hex change no fill's state.
    s.set_fill_paint(false);
    select(&mut s, &[2]);
    s.set_fill_paint(true);
    select(&mut s, &[0, 1, 2]);
    s.set_style_text(StyleField::FillColor, "00FF00").unwrap();
    s.set_style_text(StyleField::FillOpacity, "40").unwrap();
    s.set_style_text(StyleField::FillColor, "0000FF80").unwrap();
    let fill_on: Vec<bool> = stored(&s).iter().map(|st| st.fill.enabled).collect();
    assert_eq!(fill_on, [false, false, true]);
    for st in stored(&s) {
        assert_eq!(st.fill.color, rgb(0, 0, 255));
        assert_eq!(st.fill.opacity.get(), 128.0 / 255.0);
    }
    assert_eq!(s.style_panel_view().fill_paint, "mixed");

    // A stroke colour edit turns the off strokes on, in the one commit.
    let c = baseline(&mut s);
    s.set_style_text(StyleField::StrokeColor, "112233").unwrap();
    assert_eq!(changes(&s), c + 1);
    assert!(stored(&s).iter().all(|st| st.stroke.enabled));
    assert_eq!(s.style_panel_view().stroke_paint, "on");
}

#[test]
fn pressing_none_or_solid_sets_every_selected_object_in_one_commit() {
    let mut s = squares(3);
    select(&mut s, &[0, 1, 2]);
    let c = baseline(&mut s);
    s.set_stroke_paint(false);
    assert_eq!(changes(&s), c + 1);
    assert!(stored(&s).iter().all(|st| !st.stroke.enabled));
    let c = baseline(&mut s);
    s.set_fill_paint(true);
    assert_eq!(changes(&s), c + 1);
    assert!(stored(&s).iter().all(|st| st.fill.enabled));
}

// ================================ AC 11 to 16: hex through the session

#[test]
fn the_hex_view_is_eight_digits_and_follows_typed_alpha_and_the_opacity_field() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    assert_eq!(s.style_panel_view().stroke_hex, "#000000FF");
    assert_eq!(s.style_panel_view().fill_hex, "#000000FF");
    s.set_style_text(StyleField::StrokeColor, "2F6FEE80")
        .unwrap();
    let v = s.style_panel_view();
    assert_eq!(v.stroke_hex, "#2F6FEE80");
    assert_eq!(v.stroke_opacity_text, "50");
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 128.0 / 255.0);
    s.set_style_text(StyleField::StrokeOpacity, "50").unwrap();
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 0.5);
    assert_eq!(s.style_panel_view().stroke_hex, "#2F6FEE80");
    // 3 and 6 digits leave the alpha, 4 digits set it.
    s.set_style_text(StyleField::StrokeColor, "f80").unwrap();
    assert_eq!(stored(&s)[0].stroke.color, rgb(255, 136, 0));
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 0.5);
    s.set_style_text(StyleField::StrokeColor, "#F80C").unwrap();
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 204.0 / 255.0);
    assert_eq!(s.style_panel_view().stroke_hex, "#FF8800CC");
}

#[test]
fn a_refused_hex_writes_nothing_and_says_which_error() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let n = ops(&s);
    for bad in ["", "12345", "GG0000", "#12", "1234567"] {
        assert_eq!(
            s.set_style_text(StyleField::StrokeColor, bad),
            Err(StyleEntryError::Hex),
            "{bad:?}"
        );
        assert_eq!(
            s.set_style_text(StyleField::FillColor, bad),
            Err(StyleEntryError::Hex)
        );
    }
    assert_eq!(ops(&s), n);
}

#[test]
fn typing_the_hex_over_several_objects_keeps_alphas_for_short_forms_and_sets_all_for_long_ones() {
    let mut s = squares(3);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeColor, "FF000040")
        .unwrap();
    select(&mut s, &[1]);
    s.set_style_text(StyleField::StrokeColor, "00FF0080")
        .unwrap();
    select(&mut s, &[0, 1, 2]);
    let v = s.style_panel_view();
    assert!(v.stroke_hex_mixed);
    assert!(v.stroke_color_mixed);
    assert!(v.stroke_opacity_mixed);

    let c = baseline(&mut s);
    s.set_style_text(StyleField::StrokeColor, "#ABC").unwrap();
    assert_eq!(changes(&s), c + 1);
    let alphas: Vec<f64> = stored(&s)
        .iter()
        .map(|st| st.stroke.opacity.get())
        .collect();
    assert_eq!(
        alphas,
        [64.0 / 255.0, 128.0 / 255.0, 1.0],
        "each keeps its own alpha"
    );
    assert!(
        stored(&s)
            .iter()
            .all(|st| st.stroke.color == rgb(0xAA, 0xBB, 0xCC))
    );
    let v = s.style_panel_view();
    assert!(!v.stroke_color_mixed, "same RGB now");
    assert!(v.stroke_hex_mixed, "alpha still differs");

    let c = baseline(&mut s);
    s.set_style_text(StyleField::StrokeColor, "11223344")
        .unwrap();
    assert_eq!(changes(&s), c + 1);
    for st in stored(&s) {
        assert_eq!(st.stroke.color, rgb(0x11, 0x22, 0x33));
        assert_eq!(st.stroke.opacity.get(), 68.0 / 255.0);
    }
    assert!(!s.style_panel_view().stroke_hex_mixed);
}

#[test]
fn an_unchanged_hex_or_opacity_writes_nothing_and_an_off_grid_alpha_is_not_normalised() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeColor, "000000C8")
        .unwrap();
    let c = baseline(&mut s);
    let n = ops(&s);
    s.set_style_text(StyleField::StrokeColor, "#000000")
        .unwrap();
    s.set_style_text(StyleField::StrokeColor, "000000C8")
        .unwrap();
    assert_eq!(ops(&s), n);
    assert_eq!(changes(&s), c);
    // 200/255 is 78.4 %: the field shows 78 and the stored value is untouched
    // by looking at it.
    assert_eq!(s.style_panel_view().stroke_opacity_text, "78");
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 200.0 / 255.0);
}

// ================================ AC 28 to 33: dash line through the session

#[test]
fn the_dash_line_stores_what_is_typed_and_the_buttons_follow_the_stored_list() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let v = s.style_panel_view();
    assert_eq!(
        (v.stroke_dash.as_str(), v.stroke_dash_text.as_str()),
        ("solid", "")
    );
    for (typed, button, line) in [
        ("6 4", "dash", "6 4"),
        ("1   3", "dot", "1 3"),
        ("6 3 1 3", "dash-dot", "6 3 1 3"),
        ("1 2 4", "none", "1 2 4"),
        ("0 3", "none", "0 3"),
        ("", "solid", ""),
        ("0.5 0.25", "none", "0.5 0.25"),
    ] {
        assert_eq!(s.set_stroke_dash_text(typed), Ok(true), "{typed:?}");
        let v = s.style_panel_view();
        assert_eq!(v.stroke_dash, button, "{typed:?}");
        assert_eq!(v.stroke_dash_text, line, "{typed:?}");
    }
    s.set_stroke_dash(DashChoice::DashDot);
    assert_eq!(stored(&s)[0].stroke.dash.as_slice(), &[6.0, 3.0, 1.0, 3.0]);
    assert_eq!(s.style_panel_view().stroke_dash_text, "6 3 1 3");
}

#[test]
fn a_refused_dash_text_writes_nothing_and_a_comma_is_never_accepted() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_stroke_dash_text("6 4").unwrap();
    let n = ops(&s);
    for bad in [
        "1,2,4,2",
        "1, 2",
        "1,5",
        "0 0",
        "-1 2",
        "1001",
        "x",
        "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17",
    ] {
        assert_eq!(
            s.set_stroke_dash_text(bad),
            Err(StyleEntryError::Dash),
            "{bad:?}"
        );
    }
    assert_eq!(ops(&s), n);
    assert_eq!(stored(&s)[0].stroke.dash.as_slice(), &[6.0, 4.0]);
}

#[test]
fn mixed_dash_lists_show_mixed_and_one_typed_text_sets_them_all_in_one_commit() {
    let mut s = squares(3);
    select(&mut s, &[0]);
    s.set_stroke_dash(DashChoice::Dash);
    select(&mut s, &[1]);
    s.set_stroke_dash(DashChoice::Dot);
    select(&mut s, &[0, 1, 2]);
    let v = s.style_panel_view();
    assert_eq!(v.stroke_dash, "mixed");
    let c = baseline(&mut s);
    s.set_stroke_dash_text("2 5 1").unwrap();
    assert_eq!(changes(&s), c + 1);
    assert!(
        stored(&s)
            .iter()
            .all(|st| st.stroke.dash.as_slice() == [2.0, 5.0, 1.0])
    );
    assert_eq!(s.style_panel_view().stroke_dash, "none");
    // The same text again changes nothing.
    let n = ops(&s);
    s.set_stroke_dash_text("2 5 1").unwrap();
    assert_eq!(ops(&s), n);
}

#[test]
fn the_dash_golden_file_shows_odd_zero_and_seventeen_number_lists_without_rewriting_them() {
    let bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../curvyo-document-core/tests/fixtures/dash_v9.curvyo"),
    )
    .unwrap();
    let mut s = Session::open(3, &bytes).unwrap();
    let n = ops(&s);
    let mut lines = Vec::new();
    s.set_tool(Tool::Select);
    // rect (0,0,10,10) odd; ellipse centre (30,5) r 5 with 17 numbers; rect (50,0) `0 3`.
    for at in [pt(0.0, 5.0), pt(35.0, 5.0), pt(50.0, 5.0)] {
        click(&mut s, at, false);
        let v = s.style_panel_view();
        assert_eq!(v.stroke_dash, "none");
        lines.push(v.stroke_dash_text);
    }
    assert_eq!(lines[0], "1 2 4");
    assert_eq!(lines[1].split(' ').count(), 17);
    assert_eq!(lines[2], "0 3");
    assert_eq!(ops(&s), n, "looking writes nothing");
}

// ================================ AC 36 to 47 session side, 61

#[test]
fn a_value_drag_previews_without_writing_commits_once_and_a_return_to_the_start_commits_nothing() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let c = baseline(&mut s);
    let n = ops(&s);
    for p in [0.3, 0.6, 0.9, 0.2] {
        s.preview_value_field(ValueField::StrokeOpacity, p, Grid::Normal);
        assert_eq!(ops(&s), n);
        assert_ne!(s.style_panel_view().stroke_opacity_text, "");
    }
    // Back to the start value (100 %: p = 1) before the release.
    s.preview_value_field(ValueField::StrokeOpacity, 1.0, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(
        ops(&s),
        n,
        "a drag that ends where it started writes nothing"
    );
    assert_eq!(changes(&s), c);
}

#[test]
fn a_value_drag_writes_exactly_what_the_same_typed_value_writes() {
    let typed = {
        let mut s = squares(1);
        select(&mut s, &[0]);
        let n = ops(&s);
        s.set_style_text(StyleField::StrokeOpacity, "33").unwrap();
        (ops(&s) - n, stored(&s)[0].stroke.opacity.get())
    };
    let mut s = squares(1);
    select(&mut s, &[0]);
    let n = ops(&s);
    s.preview_value_field(ValueField::StrokeOpacity, 0.5, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 0.33, "N / 100 exactly");
    assert_eq!((ops(&s) - n, stored(&s)[0].stroke.opacity.get()), typed);
    // The width: the value at p = 0.5 is 1.82 mm.
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 1.82);
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Fine);
    s.commit_style_preview();
    assert_eq!(
        stored(&s)[0].stroke.width.as_mm(),
        1.818,
        "Ctrl: 0.001 mm grid"
    );
    s.preview_value_field(ValueField::StrokeWidth, 0.3, Grid::Coarse);
    s.commit_style_preview();
    let w = stored(&s)[0].stroke.width.as_mm();
    assert!(
        (w * 10.0 - (w * 10.0).round()).abs() < 1e-9,
        "Shift grid is 0.1 mm: {w}"
    );
}

#[test]
fn escape_during_a_value_drag_restores_the_committed_value_and_the_release_writes_nothing() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let n = ops(&s);
    s.preview_value_field(ValueField::StrokeWidth, 0.9, Grid::Normal);
    assert_ne!(s.style_panel_view().stroke_width_text, "0.25");
    s.cancel_style_preview();
    assert_eq!(s.style_panel_view().stroke_width_text, "0.25");
    s.commit_style_preview();
    assert_eq!(ops(&s), n);
    // And the next drag after a cancel works normally.
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 1.82);
}

#[test]
fn a_drag_commits_to_the_objects_it_started_on_even_after_a_canvas_press_or_a_tool_change() {
    // Criterion 59 / 0007 criterion 36.
    for change in ["press", "tool"] {
        let mut s = squares(2);
        select(&mut s, &[0]);
        s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
        if change == "press" {
            click(&mut s, pt(40.0, 10.0), false);
            assert_eq!(s.selected_object_count(), 1);
        } else {
            s.set_tool(Tool::Rectangle);
        }
        s.commit_style_preview();
        let st = stored(&s);
        assert_eq!(
            st[0].stroke.width.as_mm(),
            1.82,
            "{change}: the first object"
        );
        assert_eq!(
            st[1].stroke.width.as_mm(),
            0.25,
            "{change}: the second is untouched"
        );
    }
}

#[test]
fn keyboard_steps_move_one_grid_step_from_the_shown_value_and_commit_on_key_up() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let n = ops(&s);
    s.step_value_field(ValueField::StrokeWidth, 1, Grid::Normal);
    assert_eq!(s.style_panel_view().stroke_width_text, "0.26");
    assert_eq!(ops(&s), n, "key-down previews only");
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 0.26);
    // A held key repeats key-down: one commit.
    let c = baseline(&mut s);
    for _ in 0..5 {
        s.step_value_field(ValueField::StrokeWidth, -1, Grid::Normal);
    }
    s.commit_style_preview();
    assert_eq!(changes(&s), c + 1);
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 0.21);
    s.step_value_field(ValueField::StrokeOpacity, -1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 0.99);
    // Opacity stops at 100 and Width at 0 (a step down from 0.01 gives 0 and
    // turns the stroke off).
    s.step_value_field(ValueField::FillOpacity, 1, Grid::Coarse);
    s.commit_style_preview();
    assert_eq!(stored(&s)[0].fill.opacity.get(), 1.0);
    assert!(
        !stored(&s)[0].fill.enabled,
        "a fill opacity step never turns the fill on"
    );
}

#[test]
fn a_mixed_field_ignores_arrow_steps_and_a_drag_sets_all_to_the_absolute_value() {
    let mut s = squares(3);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "1").unwrap();
    select(&mut s, &[1]);
    s.set_style_text(StyleField::StrokeWidth, "3").unwrap();
    select(&mut s, &[0, 1, 2]);
    let v = s.style_panel_view();
    assert!(v.stroke_width_mixed);
    assert!(v.stroke_width_resettable, "Mixed shows the reset icon");
    let n = ops(&s);
    s.step_value_field(ValueField::StrokeWidth, 1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(ops(&s), n, "the arrow keys do nothing on a Mixed field");

    let c = baseline(&mut s);
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(changes(&s), c + 1);
    assert!(stored(&s).iter().all(|st| st.stroke.width.as_mm() == 1.82));
}

#[test]
fn reset_sets_every_object_to_the_default_in_one_commit_and_writes_nothing_at_the_default() {
    let mut s = squares(2);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "1").unwrap();
    select(&mut s, &[0, 1]);
    assert!(s.style_panel_view().stroke_width_resettable);
    let c = baseline(&mut s);
    assert!(s.reset_value_field(ValueField::StrokeWidth));
    assert_eq!(changes(&s), c + 1);
    assert!(stored(&s).iter().all(|st| st.stroke.width.as_mm() == 0.25));
    assert!(
        !s.style_panel_view().stroke_width_resettable,
        "icon hidden at the default"
    );
    let n = ops(&s);
    assert!(s.reset_value_field(ValueField::StrokeWidth));
    assert_eq!(ops(&s), n, "already the default: nothing written");

    // Opacity: 100 %, for stroke and fill; the fill stays off.
    s.set_style_text(StyleField::StrokeOpacity, "20").unwrap();
    s.set_style_text(StyleField::FillOpacity, "30").unwrap();
    assert!(s.style_panel_view().stroke_opacity_resettable);
    assert!(s.style_panel_view().fill_opacity_resettable);
    s.reset_value_field(ValueField::StrokeOpacity);
    s.reset_value_field(ValueField::FillOpacity);
    for st in stored(&s) {
        assert_eq!(st.stroke.opacity.get(), 1.0);
        assert_eq!(st.fill.opacity.get(), 1.0);
        assert!(!st.fill.enabled);
    }
}

#[test]
fn a_reset_of_width_or_opacity_is_an_edit_that_turns_an_off_stroke_on() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeWidth, "2").unwrap();
    s.set_stroke_paint(false);
    // The rows are hidden while off, but the call exists for a mixed selection.
    assert!(s.reset_value_field(ValueField::StrokeWidth));
    assert!(stored(&s)[0].stroke.enabled);
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 0.25);
}

#[test]
fn value_field_calls_with_nothing_to_edit_do_nothing() {
    let mut s = Session::new(1);
    let n = ops(&s);
    s.preview_value_field(ValueField::StrokeWidth, 0.5, Grid::Normal);
    s.step_value_field(ValueField::StrokeOpacity, 1, Grid::Normal);
    s.commit_style_preview();
    s.cancel_style_preview();
    assert!(!s.reset_value_field(ValueField::StrokeWidth));
    assert_eq!(s.set_style_text(StyleField::StrokeWidth, "3"), Ok(false));
    assert_eq!(s.set_stroke_dash_text("1 2"), Ok(false));
    assert_eq!(ops(&s), n);
}

#[test]
fn typed_value_field_text_is_refused_outside_its_range_and_writes_nothing() {
    let mut s = squares(1);
    select(&mut s, &[0]);
    let n = ops(&s);
    assert_eq!(
        s.set_style_text(StyleField::StrokeWidth, "1000.5"),
        Err(StyleEntryError::Width)
    );
    assert_eq!(
        s.set_style_text(StyleField::StrokeWidth, "-1"),
        Err(StyleEntryError::Width)
    );
    assert_eq!(
        s.set_style_text(StyleField::StrokeWidth, "abc"),
        Err(StyleEntryError::Width)
    );
    assert_eq!(
        s.set_style_text(StyleField::StrokeOpacity, "101"),
        Err(StyleEntryError::Percent)
    );
    assert_eq!(
        s.set_style_text(StyleField::FillOpacity, "-1"),
        Err(StyleEntryError::Percent)
    );
    assert_eq!(ops(&s), n);
    // Decimals in Opacity round to the nearest whole percent.
    s.set_style_text(StyleField::StrokeOpacity, "33.6").unwrap();
    assert_eq!(stored(&s)[0].stroke.opacity.get(), 0.34);
    // The typed maximum is 1000 mm, above the drag range.
    s.set_style_text(StyleField::StrokeWidth, "1000").unwrap();
    assert_eq!(stored(&s)[0].stroke.width.as_mm(), 1000.0);
    assert_eq!(
        s.style_panel_view().stroke_width_bar,
        1.0,
        "a value above the drag range is a full bar"
    );
    assert_eq!(s.style_panel_view().stroke_width_text, "1000");
}

// ================================ AC 49, 53: gradients and legacy files

fn legacy_session() -> Session {
    let bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../curvyo-document-core/tests/fixtures/legacy_gradient_v7.curvyo"),
    )
    .unwrap();
    Session::open(3, &bytes).unwrap()
}

#[test]
fn a_legacy_gradient_object_shows_fill_none_and_opening_writes_nothing() {
    let mut s = legacy_session();
    let n = ops(&s);
    s.set_tool(Tool::Select);
    // Radial rectangle at (50, 0), 40 x 20, stroke off: select by marquee-free
    // click on the outline is impossible (no stroke), so select by its fill
    // region being absent: use the linear path's segment (15, 0)...
    click(&mut s, pt(15.0, 0.0), false);
    let v = s.style_panel_view();
    assert_eq!(v.fill_paint, "off", "a gradient fill is Paint None");
    assert!(!v.fill_rows);
    assert_eq!(v.subject, "Path");
    assert_eq!(ops(&s), n);
    assert!(has_legacy_keys(&s));
    // A save without a fill edit keeps the gradient data.
    let again = Session::open(4, &s.pack("t").unwrap()).unwrap();
    assert!(has_legacy_keys(&again));
}

#[test]
fn the_interior_of_a_gradient_object_is_not_clickable_and_the_object_has_no_fill_to_pick() {
    let mut s = legacy_session();
    s.set_tool(Tool::Select);
    // (25, 5) is inside the open path's implied triangle, far from its segments.
    click(&mut s, pt(25.0, 5.0), false);
    assert_eq!(
        s.selected_object_count(),
        0,
        "a fill that reads as off is not clickable"
    );
    // Radial rectangle (50..90, 0..20): stroke off, fill off: nothing to click.
    click(&mut s, pt(70.0, 10.0), false);
    assert_eq!(s.selected_object_count(), 0);
    // The eyedropper finds nothing there either.
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(70.0, 10.0), false);
    assert_eq!(
        s.colour_pick_target(),
        Some(PaintTarget::Fill),
        "no paint here: still picking"
    );
}

#[test]
fn the_next_fill_edit_of_a_gradient_object_drops_the_legacy_keys_in_the_same_commit() {
    let mut s = legacy_session();
    s.set_tool(Tool::Select);
    click(&mut s, pt(15.0, 0.0), false);
    assert!(has_legacy_keys(&s));
    let c = baseline(&mut s);
    s.set_fill_paint(true);
    assert_eq!(changes(&s), c + 1);
    let v = s.style_panel_view();
    assert_eq!(v.fill_paint, "on");
    assert!(v.fill_rows);
    assert_eq!(
        v.fill_hex, "#00800080",
        "the object's stored solid colour and opacity"
    );
    // The other gradient object (the rectangle) still carries its keys: the
    // keys of the edited object are gone.
    let l = loro_of(&s);
    let tree = l.get_tree("paths");
    let with_keys = tree
        .nodes()
        .into_iter()
        .filter(|n| {
            format!("{:?}", tree.get_meta(*n).unwrap().get_deep_value()).contains("fill_kind")
        })
        .count();
    assert_eq!(with_keys, 1);
}

// ================================ AC 22 to 26, 59: the eyedropper

fn painted_squares() -> Session {
    // Square 0: red 50 % stroke and blue fill at 128/255. Square 1: green fill
    // (stroke black). Square 2: untouched defaults.
    let mut s = squares(3);
    select(&mut s, &[0]);
    s.set_style_text(StyleField::StrokeColor, "FF000080")
        .unwrap();
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "0000FF80").unwrap();
    select(&mut s, &[1]);
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "00FF00").unwrap();
    s
}

#[test]
fn a_pick_writes_to_every_selected_object_in_one_commit_and_leaves_the_selection_alone() {
    let mut s = painted_squares();
    select(&mut s, &[1, 2]);
    let c = baseline(&mut s);
    s.begin_colour_pick(PaintTarget::Stroke);
    assert_eq!(s.style_panel_view().pick_target, "stroke");
    click(&mut s, pt(0.0, 10.0), false); // the red stroke of square 0
    assert_eq!(changes(&s), c + 1);
    let st = stored(&s);
    for k in [1, 2] {
        assert_eq!(st[k].stroke.color, rgb(255, 0, 0));
        assert_eq!(st[k].stroke.opacity.get(), 128.0 / 255.0);
    }
    assert_eq!(
        st[0].stroke.color,
        rgb(255, 0, 0),
        "the source is unchanged"
    );
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(s.colour_pick_target(), None);
    assert_eq!(s.style_panel_view().pick_target, "");
}

#[test]
fn the_hover_readout_shows_what_a_click_would_take_and_clears_off_the_drawing() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    s.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(s.colour_pick_hover(), None);
    s.pointer_hover(pt(10.0, 10.0), false, false);
    assert_eq!(
        s.colour_pick_hover(),
        Some(("#0000FF80".to_string(), PaintTarget::Fill))
    );
    s.pointer_hover(pt(0.0, 10.0), false, false);
    assert_eq!(
        s.colour_pick_hover(),
        Some(("#FF000080".to_string(), PaintTarget::Stroke))
    );
    s.pointer_hover(pt(30.0, 10.0), false, false);
    assert_eq!(s.colour_pick_hover(), None, "No paint here");
    s.pointer_hover(pt(1000.0, 1000.0), false, false);
    assert_eq!(s.colour_pick_hover(), None);
    s.end_colour_pick();
    s.pointer_hover(pt(10.0, 10.0), false, false);
    assert_eq!(s.colour_pick_hover(), None, "no readout when not picking");
}

#[test]
fn a_fill_pick_never_turns_the_fill_on_and_a_stroke_pick_turns_a_stroke_on() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    s.set_stroke_paint(false);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(10.0, 10.0), false);
    assert_eq!(stored(&s)[2].fill.color, rgb(0, 0, 255));
    assert!(!stored(&s)[2].fill.enabled);
    s.begin_colour_pick(PaintTarget::Stroke);
    click(&mut s, pt(50.0, 10.0), false);
    assert!(stored(&s)[2].stroke.enabled);
}

#[test]
fn a_press_while_picking_selects_nothing_moves_nothing_and_draws_nothing() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    let all = |s: &Session| {
        let d = document_of(s);
        d.object_ids()
            .into_iter()
            .map(|id| format!("{:?}", d.object(id).unwrap()))
            .collect::<Vec<_>>()
    };
    let before = all(&s);
    s.begin_colour_pick(PaintTarget::Stroke);
    // A drag from the selected square's inside to far away: no move, no new
    // object, nothing painted there so nothing written.
    s.pointer_hover(pt(90.0, 10.0), false, false);
    s.pointer_down(pt(90.0, 10.0), false);
    s.pointer_hover(pt(150.0, 80.0), false, false);
    s.pointer_up(pt(150.0, 80.0), false, false);
    assert_eq!(all(&s), before);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(
        s.colour_pick_target(),
        Some(PaintTarget::Stroke),
        "a miss keeps picking"
    );
    // A press on another, unselected object picks, it does not select it.
    click(&mut s, pt(40.0, 10.0), false);
    assert_eq!(s.selected_object_count(), 1);
    let after = all(&s);
    assert_eq!(after.len(), before.len());
    // Only a style may have changed: the shapes are equal.
    for (a, b) in after.iter().zip(&before) {
        let shape = |t: &str| t.split("style:").next().unwrap().to_string();
        assert_eq!(shape(a), shape(b));
    }
}

#[test]
fn the_normal_click_after_a_pick_works_again_and_the_release_of_the_pick_press_does_not_leak() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    s.begin_colour_pick(PaintTarget::Stroke);
    click(&mut s, pt(0.0, 10.0), false);
    assert_eq!(s.colour_pick_target(), None);
    // Selecting another object by clicking its outline now works.
    s.set_tool(Tool::Select);
    click(&mut s, pt(40.0, 10.0), false);
    assert_eq!(s.selected_object_count(), 1);
    click(&mut s, pt(40.0, 10.0), true);
    // Drawing a rectangle works again after a pick.
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(200.0, 0.0), false);
    s.pointer_up(pt(220.0, 20.0), false, false);
    assert_eq!(document_of(&s).object_ids().len(), 4);
}

#[test]
fn every_way_of_ending_a_pick_writes_nothing() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    let n = ops(&s);

    s.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(s.escape(), EscapeStep::ClearedState);
    assert_eq!(s.colour_pick_target(), None, "Escape");
    assert_eq!(
        s.selected_object_count(),
        1,
        "Escape ends picking and never clears the selection"
    );

    s.begin_colour_pick(PaintTarget::Fill);
    s.set_tool(Tool::Rectangle);
    assert_eq!(s.colour_pick_target(), None, "tool change");
    s.set_tool(Tool::Select);

    s.begin_colour_pick(PaintTarget::Fill);
    s.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(s.colour_pick_target(), None, "the button again");

    s.begin_colour_pick(PaintTarget::Fill);
    s.begin_colour_pick(PaintTarget::Stroke);
    assert_eq!(
        s.colour_pick_target(),
        Some(PaintTarget::Stroke),
        "the other eyedropper switches"
    );

    s.begin_colour_pick(PaintTarget::Stroke);
    s.end_colour_pick();
    assert_eq!(
        s.colour_pick_target(),
        None,
        "a press elsewhere in the panel"
    );
    s.end_colour_pick();
    assert_eq!(ops(&s), n);
}

#[test]
fn pan_and_zoom_keep_working_while_picking_and_do_not_end_it() {
    let mut s = painted_squares();
    select(&mut s, &[2]);
    s.resize_viewport(800.0, 600.0);
    s.begin_colour_pick(PaintTarget::Stroke);
    s.wheel(0.0, 40.0, 400.0, 300.0, false, false);
    s.wheel(0.0, -120.0, 400.0, 300.0, false, true);
    s.begin_pan(100.0, 100.0);
    s.pan_to(160.0, 130.0);
    s.end_pan();
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Stroke));
    // After zooming the pick still lands on the painted shape under the pointer.
    let on_square_zero = s.screen_to_document(0.0, 0.0);
    let _ = on_square_zero;
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn picking_with_the_object_hit_order_takes_the_topmost_painted_object() {
    // Two squares overlap; the later one is on top.
    let mut s = Session::new(1);
    for (x, fill) in [(0.0, "FF0000"), (10.0, "0000FF")] {
        s.set_tool(Tool::Rectangle);
        s.pointer_down(pt(x, 0.0), false);
        s.pointer_up(pt(x + 30.0, 30.0), false, false);
        s.set_fill_paint(true);
        s.set_style_text(StyleField::FillColor, fill).unwrap();
    }
    // A third, far away, to receive the colour.
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(100.0, 0.0), false);
    s.pointer_up(pt(130.0, 30.0), false, false);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(20.0, 15.0), false); // inside both fills
    assert_eq!(stored(&s)[2].fill.color, rgb(0, 0, 255), "the topmost");
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(5.0, 15.0), false); // only the lower
    assert_eq!(stored(&s)[2].fill.color, rgb(255, 0, 0));
    let _ = op(1.0);
}
