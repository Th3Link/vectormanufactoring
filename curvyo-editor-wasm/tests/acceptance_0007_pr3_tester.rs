//! Independent tester checks of `specs/0007-stroke-and-fill-styling` PR 3 at
//! the `Session` level, written from the specification before the
//! implementation was read: stroke off/on interplay (criterion 5), colour and
//! opacity entry (6, 14), fill modes (13), multi-selection (24), the panel's
//! scope per tool (37), the gesture rules (36) and the view origin when the
//! panel toggles (39).
//!
//! "Writes nothing" is checked on the Loro op log (`oplog_vv`), so a no-op that
//! still records an operation fails.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Color, Document, Length, LineCap, LineJoin, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::DrawList;
use curvyo_ui_core::{BarValue, DashChoice, StyleEntryError, StyleField};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// `n` 10 mm rectangles in a row (x = 0, 30, 60, ...), the last one selected,
/// Select tool active afterwards.
fn rectangles(n: u32) -> Session {
    let mut session = Session::new(1);
    for k in 0..n {
        session.set_tool(Tool::Rectangle);
        let x = f64::from(k) * 30.0;
        session.pointer_down(Point::new(x, 0.0), false);
        session.pointer_up(Point::new(x + 10.0, 10.0), false, false);
    }
    session.set_tool(Tool::Select);
    session
}

fn click_edge(session: &mut Session, k: u32, shift: bool) {
    session.set_tool(Tool::Select);
    let edge = Point::new(f64::from(k) * 30.0, 5.0);
    session.pointer_hover(edge, shift, false);
    session.pointer_down(edge, shift);
    session.pointer_up(edge, shift, false);
}

/// A press on an already selected object keeps a multi-selection, so empty
/// canvas is pressed first.
fn select_only(session: &mut Session, k: u32) {
    session.set_tool(Tool::Select);
    let empty = Point::new(900.0, 900.0);
    session.pointer_hover(empty, false, false);
    session.pointer_down(empty, false);
    session.pointer_up(empty, false, false);
    click_edge(session, k, false);
}

fn select_all(session: &mut Session, n: u32) {
    for k in 0..n {
        click_edge(session, k, k > 0);
    }
}

fn document_of(session: &Session) -> Document {
    unpack(9, &session.pack("0.1.0").unwrap()).unwrap()
}

fn styles(session: &Session) -> Vec<Style> {
    let document = document_of(session);
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn loro_of(session: &Session) -> loro::LoroDoc {
    let d = document_of(session);
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn version(session: &Session) -> loro::VersionVector {
    loro_of(session).oplog_vv()
}

fn has_alpha(list: &DrawList, alpha: u8) -> bool {
    list.triangles.iter().any(|v| v.color.a == alpha)
}

fn pct(n: u32) -> Opacity {
    Opacity::new(f64::from(n) / 100.0).unwrap()
}

fn red() -> Color {
    Color { r: 255, g: 0, b: 0 }
}

/// An open two-node path from (x, y) to (x + 50, y), drawn with the Pen and
/// left selected.
fn draw_path(session: &mut Session, x: f64, y: f64) {
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(x, y), false);
    session.pointer_up(Point::new(x, y), false, false);
    session.pointer_down(Point::new(x + 50.0, y), false);
    session.pointer_up(Point::new(x + 50.0, y), false, false);
    session.finish_pen();
}

// ---------------------------------------------------------------------
// Criterion 5: stroke off / on
// ---------------------------------------------------------------------

#[test]
fn width_zero_is_no_stroke_and_keeps_every_stored_value() {
    let mut session = rectangles(1);
    session
        .set_style_text(StyleField::StrokeWidth, "3")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#F00")
        .unwrap();
    session.set_stroke_dash(DashChoice::Dot);
    session.set_stroke_join(LineJoin::Round);
    session.set_stroke_cap(LineCap::Square);
    let before = styles(&session)[0].clone();

    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "0"),
        Ok(true)
    );
    let off = styles(&session)[0].clone();
    assert!(!off.stroke.enabled, "width 0 is no stroke");
    assert_eq!(off.stroke.width, before.stroke.width, "width kept");
    assert_eq!(off.stroke.color, red());
    assert_eq!(off.stroke.dash.as_slice(), &[1.0, 3.0]);
    assert_eq!(off.stroke.join, LineJoin::Round);
    assert_eq!(off.stroke.cap, LineCap::Square);
    let state = session.style_panel_state();
    assert!(state.stroke.all_off);
    assert_eq!(state.stroke.paint, BarValue::Uniform(false));

    // Typing 0 again changes nothing.
    let v = version(&session);
    session
        .set_style_text(StyleField::StrokeWidth, "0")
        .unwrap();
    assert_eq!(version(&session), v, "0 on an off stroke writes nothing");

    // Paint back on restores everything unchanged.
    session.set_stroke_paint(true);
    assert_eq!(styles(&session)[0], before);
}

#[test]
fn a_non_zero_width_or_a_colour_or_opacity_edit_turns_an_off_stroke_back_on() {
    for edit in 0..3 {
        let mut session = rectangles(1);
        session
            .set_style_text(StyleField::StrokeWidth, "2")
            .unwrap();
        session.set_stroke_paint(false);
        assert!(!styles(&session)[0].stroke.enabled);
        match edit {
            0 => session
                .set_style_text(StyleField::StrokeWidth, "4")
                .unwrap(),
            1 => session
                .set_style_text(StyleField::StrokeColor, "#0F0")
                .unwrap(),
            _ => session
                .set_style_text(StyleField::StrokeOpacity, "50")
                .unwrap(),
        };
        let s = &styles(&session)[0];
        assert!(s.stroke.enabled, "edit kind {edit} turns the stroke on");
        if edit != 0 {
            assert_eq!(s.stroke.width, Length::from_mm(2.0), "other values kept");
        }
        assert!(!session.style_panel_state().stroke.all_off);
    }
}

#[test]
fn a_colour_preview_on_an_off_stroke_is_drawn_and_commits_the_stroke_on() {
    let mut session = rectangles(1);
    session.set_stroke_paint(false);
    let off = session.draw_list();
    session.preview_style_color(StyleField::StrokeColor, red());
    let previewed = session.draw_list();
    assert!(
        previewed.triangles.len() > off.triangles.len(),
        "the stroke shows during the drag"
    );
    session.commit_style_preview();
    assert!(styles(&session)[0].stroke.enabled);
    assert_eq!(styles(&session)[0].stroke.color, red());
}

#[test]
fn some_on_some_off_keeps_the_rows_enabled_and_paint_on_turns_all_on() {
    let mut session = rectangles(2);
    select_only(&mut session, 0);
    session
        .set_style_text(StyleField::StrokeWidth, "5")
        .unwrap();
    session.set_stroke_paint(false);
    select_all(&mut session, 2);
    let state = session.style_panel_state();
    assert_eq!(state.subject, "2 rectangles");
    assert_eq!(state.stroke.paint, BarValue::Mixed);
    assert!(!state.stroke.all_off);
    assert_eq!(state.stroke.width, BarValue::Mixed);

    session.set_stroke_paint(true);
    let all = styles(&session);
    assert!(all.iter().all(|s| s.stroke.enabled));
    assert_eq!(
        all[0].stroke.width,
        Length::from_mm(5.0),
        "each keeps its own"
    );
    assert_eq!(all[1].stroke.width, Length::from_mm(0.25));
}

// ---------------------------------------------------------------------
// Criteria 4, 6, 14: typed values and persistence
// ---------------------------------------------------------------------

#[test]
fn typed_values_survive_save_and_reopen_unchanged() {
    let mut session = rectangles(1);
    session
        .set_style_text(StyleField::StrokeWidth, "0,123456")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#F80")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeOpacity, "37")
        .unwrap();
    session.set_fill_paint(true);
    session
        .set_style_text(StyleField::FillColor, "12ab34")
        .unwrap();
    session
        .set_style_text(StyleField::FillOpacity, "7.4")
        .unwrap();
    let reopened = Session::open(2, &session.pack("0.1.0").unwrap()).unwrap();
    let s = &styles(&reopened)[0];
    assert_eq!(s.stroke.width, Length::from_mm(0.123_456));
    assert_eq!(
        s.stroke.color,
        Color {
            r: 0xFF,
            g: 0x88,
            b: 0
        }
    );
    assert_eq!(s.stroke.opacity, pct(37));
    assert_eq!(
        s.fill.color,
        Color {
            r: 0x12,
            g: 0xAB,
            b: 0x34
        }
    );
    assert_eq!(s.fill.opacity, pct(7), "7.4 rounds to the nearest percent");
    assert_eq!(styles(&session), styles(&reopened));
}

#[test]
fn refused_text_writes_nothing_for_every_field_and_selection_size() {
    let mut session = rectangles(3);
    select_all(&mut session, 3);
    let v = version(&session);
    let cases = [
        (StyleField::StrokeWidth, "-1", StyleEntryError::Width),
        (StyleField::StrokeWidth, "1000.5", StyleEntryError::Width),
        (StyleField::StrokeWidth, "NaN", StyleEntryError::Width),
        (StyleField::StrokeWidth, "", StyleEntryError::Width),
        (StyleField::StrokeColor, "#12", StyleEntryError::Hex),
        (StyleField::FillColor, "zzz", StyleEntryError::Hex),
        (
            StyleField::StrokeColor,
            "#11223344",
            StyleEntryError::HexEightDigits,
        ),
        (StyleField::StrokeOpacity, "-1", StyleEntryError::Percent),
        (StyleField::FillOpacity, "101", StyleEntryError::Percent),
        (StyleField::FillOpacity, "x", StyleEntryError::Percent),
    ];
    for (field, text, error) in cases {
        assert_eq!(
            session.set_style_text(field, text),
            Err(error),
            "{field:?} {text:?}"
        );
    }
    assert_eq!(version(&session), v, "no operation recorded");
}

#[test]
fn an_edit_that_changes_nothing_records_no_operation() {
    let mut session = rectangles(2);
    select_all(&mut session, 2);
    let v = version(&session);
    // Every one of these equals the stored default.
    session
        .set_style_text(StyleField::StrokeWidth, "0.25")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "000")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeOpacity, "100")
        .unwrap();
    session.set_stroke_paint(true);
    session.set_stroke_dash(DashChoice::Solid);
    session.set_stroke_join(LineJoin::Miter);
    session.set_stroke_cap(LineCap::Butt);
    session.set_fill_paint(false);
    assert_eq!(version(&session), v);
    // A drag that ends where it began writes nothing either.
    session.preview_style_opacity(StyleField::StrokeOpacity, 100.0);
    session.commit_style_preview();
    assert_eq!(version(&session), v);
}

// ---------------------------------------------------------------------
// Criterion 13: fill modes
// ---------------------------------------------------------------------

#[test]
fn fill_on_and_off_keep_the_stored_colour() {
    let mut session = rectangles(1);
    session.set_fill_paint(true);
    let first = styles(&session)[0].clone();
    assert!(first.fill.enabled);
    assert_eq!(first.fill.color, Color::BLACK, "black the first time");
    session
        .set_style_text(StyleField::FillColor, "#0000FF")
        .unwrap();
    session
        .set_style_text(StyleField::FillOpacity, "40")
        .unwrap();
    session.set_fill_paint(false);
    let none = styles(&session)[0].clone();
    assert!(!none.fill.enabled);
    assert_eq!(none.fill.color, Color { r: 0, g: 0, b: 255 }, "kept");
    assert_eq!(none.fill.opacity, pct(40));

    session.set_fill_paint(true);
    let back = styles(&session)[0].clone();
    assert!(back.fill.enabled);
    assert_eq!(back.fill.color, Color { r: 0, g: 0, b: 255 });
    assert_eq!(back.fill.opacity, pct(40));
}

#[test]
fn the_fill_paint_over_a_mixed_selection_changes_only_the_paint() {
    let mut session = rectangles(2);
    select_only(&mut session, 0);
    session.set_fill_paint(true);
    session
        .set_style_text(StyleField::FillColor, "#F00")
        .unwrap();
    select_all(&mut session, 2);
    assert_eq!(session.style_panel_state().fill.paint, BarValue::Mixed);
    assert_eq!(session.style_panel_state().fill.color, BarValue::Mixed);
    session.set_fill_paint(true);
    let all = styles(&session);
    assert!(all.iter().all(|s| s.fill.enabled));
    assert_eq!(all[0].fill.color, red(), "keeps its own colour");
    assert_eq!(all[1].fill.color, Color::BLACK);
    assert_eq!(
        session.style_panel_state().fill.paint,
        BarValue::Uniform(true)
    );
}

#[test]
fn a_panel_fill_edit_never_changes_the_stroke_and_vice_versa() {
    let mut session = rectangles(1);
    session.set_fill_paint(true);
    let before = styles(&session)[0].clone();
    session
        .set_style_text(StyleField::FillColor, "#123")
        .unwrap();
    session
        .set_style_text(StyleField::FillOpacity, "55")
        .unwrap();
    let after = styles(&session)[0].clone();
    assert_eq!(after.stroke, before.stroke);
    session
        .set_style_text(StyleField::StrokeColor, "#456")
        .unwrap();
    assert_eq!(styles(&session)[0].fill, after.fill);
}

// ---------------------------------------------------------------------
// Criteria 1, 2, 24: shared model and multi-selection
// ---------------------------------------------------------------------

#[test]
fn a_path_and_a_rectangle_take_the_identical_edits() {
    let mut session = rectangles(1);
    draw_path(&mut session, 100.0, 100.0);
    session.set_tool(Tool::Select);
    // Select both: the rectangle by edge click, the path by shift-click.
    select_only(&mut session, 0);
    session.pointer_hover(Point::new(125.0, 100.0), true, false);
    session.pointer_down(Point::new(125.0, 100.0), true);
    session.pointer_up(Point::new(125.0, 100.0), true, false);
    let state = session.style_panel_state();
    assert_eq!(state.subject, "2 objects");

    session
        .set_style_text(StyleField::StrokeWidth, "2.5")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeColor, "#F80")
        .unwrap();
    session
        .set_style_text(StyleField::StrokeOpacity, "60")
        .unwrap();
    session.set_stroke_dash(DashChoice::DashDot);
    session.set_stroke_join(LineJoin::Bevel);
    session.set_stroke_cap(LineCap::Round);
    session.set_fill_paint(true);
    session
        .set_style_text(StyleField::FillColor, "#0F0")
        .unwrap();
    let all = styles(&session);
    assert_eq!(all.len(), 2);
    assert_eq!(
        all[0], all[1],
        "path and rectangle hold the identical style"
    );
    let doc = document_of(&session);
    let ids = doc.object_ids();
    let primitives = ids
        .iter()
        .filter(|id| doc.primitive(**id).is_some())
        .count();
    assert_eq!(
        primitives, 1,
        "criterion 2: the rectangle is still a primitive"
    );
}

#[test]
fn a_multi_selection_edit_changes_one_property_and_keeps_each_objects_others() {
    let mut session = rectangles(3);
    select_only(&mut session, 0);
    session
        .set_style_text(StyleField::StrokeWidth, "3")
        .unwrap();
    select_only(&mut session, 1);
    session
        .set_style_text(StyleField::StrokeColor, "#F00")
        .unwrap();
    select_only(&mut session, 2);
    session.set_stroke_cap(LineCap::Round);
    select_all(&mut session, 3);
    let state = session.style_panel_state();
    assert_eq!(state.stroke.width, BarValue::Mixed);
    assert_eq!(state.stroke.color, BarValue::Mixed);
    assert_eq!(state.stroke.cap, BarValue::Mixed);
    assert_eq!(state.stroke.join, BarValue::Uniform(LineJoin::Miter));

    session.set_stroke_dash(DashChoice::Dot);
    session.set_stroke_join(LineJoin::Round);
    session
        .set_style_text(StyleField::StrokeOpacity, "20")
        .unwrap();
    let all = styles(&session);
    assert_eq!(all[0].stroke.width, Length::from_mm(3.0));
    assert_eq!(all[1].stroke.color, red());
    assert_eq!(all[2].stroke.cap, LineCap::Round);
    assert_eq!(all[0].stroke.color, Color::BLACK);
    assert_eq!(all[1].stroke.width, Length::from_mm(0.25));
    for s in &all {
        assert_eq!(s.stroke.dash.as_slice(), &[1.0, 3.0]);
        assert_eq!(s.stroke.join, LineJoin::Round);
        assert_eq!(s.stroke.opacity, pct(20));
    }
    let state = session.style_panel_state();
    assert_eq!(state.stroke.opacity, BarValue::Uniform(pct(20)));
    assert_eq!(state.stroke.width, BarValue::Mixed, "still mixed");
}

// ---------------------------------------------------------------------
// Criterion 37: scope per tool, empty selection, subject line
// ---------------------------------------------------------------------

#[test]
fn an_empty_selection_and_the_pen_ignore_every_panel_control() {
    let mut session = rectangles(2);
    let v = version(&session);
    // Escape clears the object selection in the Select tool.
    session.escape();
    let before = session.draw_list();
    assert_eq!(session.style_panel_state().subject, "Nothing selected");
    assert!(!session.style_panel_state().enabled);
    for round in 0..2 {
        if round == 1 {
            select_only(&mut session, 0);
            session.set_tool(Tool::Pen);
            assert!(!session.style_panel_state().enabled);
        }
        assert_eq!(
            session.set_style_text(StyleField::StrokeWidth, "9"),
            Ok(false)
        );
        assert_eq!(
            session.set_style_text(StyleField::FillColor, "#F00"),
            Ok(false)
        );
        session.set_stroke_paint(false);
        session.set_stroke_dash(DashChoice::Dash);
        session.set_stroke_join(LineJoin::Round);
        session.set_stroke_cap(LineCap::Round);
        session.set_fill_paint(true);
        session.preview_style_color(StyleField::StrokeColor, red());
        session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
        assert_eq!(session.draw_list().triangles.len(), before.triangles.len());
        assert!(!has_alpha(&session.draw_list(), 102), "no preview drawn");
        session.commit_style_preview();
    }
    assert_eq!(version(&session), v, "nothing recorded");
    assert!(styles(&session).iter().all(|s| *s == Style::default()));
    // The disabled panel still reports the frozen defaults.
    let state = session.style_panel_state();
    assert_eq!(state.stroke.width, BarValue::Uniform(Length::from_mm(0.25)));
    assert_eq!(state.fill.paint, BarValue::Uniform(false));
}

#[test]
fn subjects_name_the_kind_the_count_or_objects() {
    let mut session = Session::new(1);
    for k in 0..2 {
        session.set_tool(Tool::Ellipse);
        let x = f64::from(k) * 30.0;
        session.pointer_down(Point::new(x, 0.0), false);
        session.pointer_up(Point::new(x + 10.0, 10.0), false, false);
    }
    assert_eq!(session.style_panel_state().subject, "Ellipse");
    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    session.pointer_hover(Point::new(30.0, 5.0), true, false);
    session.pointer_down(Point::new(30.0, 5.0), true);
    session.pointer_up(Point::new(30.0, 5.0), true, false);
    assert_eq!(session.style_panel_state().subject, "2 ellipses");
}

#[test]
fn a_shape_just_drawn_with_any_creation_tool_is_editable_at_once() {
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
        let mut session = Session::new(1);
        session.set_tool(tool);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(20.0, 20.0), false, false);
        let state = session.style_panel_state();
        assert!(state.enabled, "{tool:?}");
        assert_ne!(state.subject, "Nothing selected", "{tool:?}");
        session
            .set_style_text(StyleField::StrokeColor, "#F00")
            .unwrap();
        session.set_fill_paint(true);
        let s = &styles(&session)[0];
        assert_eq!(s.stroke.color, red(), "{tool:?}");
        assert!(s.fill.enabled, "{tool:?}");
    }
}

#[test]
fn switching_to_the_select_tool_keeps_the_panel_on_the_selection() {
    let mut session = rectangles(1);
    session.set_tool(Tool::Select);
    let state = session.style_panel_state();
    assert!(state.enabled);
    assert_eq!(state.subject, "Rectangle");
}

#[test]
fn the_node_tool_edits_paths_only_and_follows_the_node_selection() {
    let mut session = rectangles(1);
    draw_path(&mut session, 100.0, 100.0);
    draw_path(&mut session, 100.0, 140.0);
    // Select tool: all three objects, then switch to the Node tool.
    session.set_tool(Tool::Select);
    select_only(&mut session, 0);
    for y in [100.0, 140.0] {
        session.pointer_hover(Point::new(125.0, y), true, false);
        session.pointer_down(Point::new(125.0, y), true);
        session.pointer_up(Point::new(125.0, y), true, false);
    }
    assert_eq!(session.style_panel_state().subject, "3 objects");
    session.set_tool(Tool::Node);
    let state = session.style_panel_state();
    assert_eq!(
        state.subject, "2 paths",
        "the Node tool without a node selected edits the paths of the object selection only"
    );
    session.set_stroke_cap(LineCap::Round);
    let all = styles(&session);
    assert_eq!(
        all[0].stroke.cap,
        LineCap::Butt,
        "the rectangle is untouched"
    );
    assert_eq!(all[1].stroke.cap, LineCap::Round);
    assert_eq!(all[2].stroke.cap, LineCap::Round);
}

#[test]
fn selecting_a_node_narrows_the_node_tool_to_the_path_that_owns_it() {
    let mut session = Session::new(1);
    draw_path(&mut session, 0.0, 0.0);
    draw_path(&mut session, 0.0, 40.0);
    session.set_tool(Tool::Select);
    for y in [0.0, 40.0] {
        session.pointer_hover(Point::new(25.0, y), y > 0.0, false);
        session.pointer_down(Point::new(25.0, y), y > 0.0);
        session.pointer_up(Point::new(25.0, y), y > 0.0, false);
    }
    session.set_tool(Tool::Node);
    assert_eq!(session.style_panel_state().subject, "2 paths");
    // Click the first node of the second path.
    session.pointer_down(Point::new(0.0, 40.0), false);
    session.pointer_up(Point::new(0.0, 40.0), false, false);
    let subject = session.style_panel_state().subject;
    assert_eq!(subject, "Path", "only the owner of the selected node");
    session
        .set_style_text(StyleField::StrokeColor, "#F00")
        .unwrap();
    let all = styles(&session);
    assert_eq!(all[0].stroke.color, Color::BLACK);
    assert_eq!(all[1].stroke.color, red());
}

// ---------------------------------------------------------------------
// Criterion 36: gestures
// ---------------------------------------------------------------------

#[test]
fn a_canvas_press_that_changes_the_selection_does_not_retarget_a_pending_drag() {
    let mut session = rectangles(2);
    select_only(&mut session, 1);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    // Press and release on the other rectangle while the drag is "pending".
    click_edge(&mut session, 0, false);
    session.commit_style_preview();
    let all = styles(&session);
    assert_eq!(
        all[1].stroke.opacity,
        pct(40),
        "the objects the edit started on"
    );
    assert_eq!(
        all[0].stroke.opacity,
        Opacity::OPAQUE,
        "not the new selection"
    );
    assert!(
        !has_alpha(&session.draw_list(), 102) || all[1].stroke.opacity == pct(40),
        "no ghost"
    );
    assert_eq!(
        session.style_panel_state().stroke.opacity,
        BarValue::Uniform(Opacity::OPAQUE),
        "the panel now reads the new selection"
    );
}

#[test]
fn the_preview_never_leaks_into_the_next_selection_or_the_saved_file() {
    let mut session = rectangles(2);
    select_only(&mut session, 1);
    session.preview_style_color(StyleField::StrokeColor, red());
    assert!(
        styles(&session)
            .iter()
            .all(|s| s.stroke.color == Color::BLACK),
        "pack during a drag holds the committed style only"
    );
    session.cancel_style_preview();
    click_edge(&mut session, 0, false);
    session.commit_style_preview();
    assert!(styles(&session).iter().all(|s| *s == Style::default()));
    assert!(
        session
            .draw_list()
            .triangles
            .iter()
            .all(|v| !(v.color.r == 255 && v.color.g == 0 && v.color.b == 0)),
        "no red left on the canvas"
    );
}

#[test]
fn a_tool_switch_mid_drag_commits_once_to_the_started_objects_and_leaves_no_ghost() {
    for tool in [
        Tool::Pen,
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        let mut session = rectangles(1);
        session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
        session.set_tool(tool);
        session.commit_style_preview();
        session.cancel_style_preview();
        let stored = styles(&session)[0].stroke.opacity;
        assert_eq!(
            stored,
            pct(40),
            "{tool:?}: the drag started on this rectangle"
        );
        assert!(
            has_alpha(&session.draw_list(), 102),
            "{tool:?} draws the commit"
        );
    }
}

#[test]
fn a_discrete_edit_during_a_drag_keeps_both_and_leaves_the_state_consistent() {
    let mut session = rectangles(1);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.set_stroke_cap(LineCap::Round);
    session.commit_style_preview();
    let s = &styles(&session)[0];
    assert_eq!(s.stroke.cap, LineCap::Round, "the discrete click is kept");
    // Whether the drag was flushed or dropped, the canvas must match the file.
    assert_eq!(
        has_alpha(&session.draw_list(), 102),
        s.stroke.opacity == pct(40),
        "no ghost preview after the release"
    );
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.commit_style_preview();
    assert_eq!(
        styles(&session)[0].stroke.opacity,
        pct(40),
        "next drag is live"
    );
}

#[test]
fn a_typed_value_during_a_drag_does_not_lose_the_typed_value() {
    let mut session = rectangles(1);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session
        .set_style_text(StyleField::StrokeWidth, "4")
        .unwrap();
    session.commit_style_preview();
    assert_eq!(styles(&session)[0].stroke.width, Length::from_mm(4.0));
}

#[test]
fn escape_during_a_drag_on_a_multi_selection_reverts_all_objects() {
    let mut session = rectangles(3);
    select_all(&mut session, 3);
    let before = session.draw_list();
    session.preview_style_color(StyleField::StrokeColor, red());
    session.preview_style_color(StyleField::StrokeColor, Color { r: 0, g: 0, b: 255 });
    session.cancel_style_preview();
    assert_eq!(
        session.draw_list().triangles.len(),
        before.triangles.len(),
        "back to the committed style"
    );
    assert!(
        session
            .draw_list()
            .triangles
            .iter()
            .all(|v| !(v.color.r == 0 && v.color.g == 0 && v.color.b == 255)),
        "no blue left"
    );
    session.commit_style_preview();
    assert!(styles(&session).iter().all(|s| *s == Style::default()));
    // The next drag after the release works again.
    session.preview_style_color(StyleField::StrokeColor, red());
    session.commit_style_preview();
    assert!(styles(&session).iter().all(|s| s.stroke.color == red()));
}

#[test]
fn out_of_range_and_non_finite_slider_ticks_store_a_valid_opacity() {
    for percent in [150.0, -20.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut session = rectangles(1);
        session.preview_style_opacity(StyleField::FillOpacity, percent);
        session.commit_style_preview();
        let o = styles(&session)[0].fill.opacity.get();
        assert!(o.is_finite() && (0.0..=1.0).contains(&o), "{percent}: {o}");
        // The file still saves and reopens.
        Session::open(2, &session.pack("0.1.0").unwrap()).unwrap();
    }
}

#[test]
fn a_colour_tick_in_the_opacity_field_or_the_other_way_round_is_ignored() {
    let mut session = rectangles(1);
    let v = version(&session);
    session.preview_style_color(StyleField::StrokeWidth, red());
    session.preview_style_opacity(StyleField::FillColor, 50.0);
    session.commit_style_preview();
    assert_eq!(version(&session), v);
}

#[test]
fn a_preview_for_a_deleted_object_commits_without_panic_or_damage() {
    let mut session = rectangles(2);
    select_all(&mut session, 2);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    select_only(&mut session, 0);
    session.delete_selected();
    session.commit_style_preview();
    session.commit_style_preview();
    let all = styles(&session);
    assert_eq!(all.len(), 1);
    assert!(
        all[0].stroke.opacity == pct(40) || all[0].stroke.opacity == Opacity::OPAQUE,
        "valid either way"
    );
    // The document is still editable.
    session
        .set_style_text(StyleField::StrokeWidth, "2")
        .unwrap();
}

#[test]
fn a_panel_edit_after_the_selection_was_deleted_is_a_no_op() {
    let mut session = rectangles(1);
    session.delete_selected();
    let v = version(&session);
    assert_eq!(session.style_panel_state().subject, "Nothing selected");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "3"),
        Ok(false)
    );
    assert_eq!(version(&session), v);
}

// ---------------------------------------------------------------------
// Criterion 39: the document does not move when the panel toggles
// ---------------------------------------------------------------------

fn corner(session: &Session) -> (f64, f64) {
    let p = session.view().screen_to_document(0.0, 0.0);
    (p.x, p.y)
}

#[test]
fn toggling_the_panel_keeps_the_origin_at_any_zoom_pan_and_pixel_ratio() {
    for dpr in [1.0, 2.0] {
        let mut session = rectangles(1);
        session.set_device_pixel_ratio(dpr);
        session.resize_viewport(1200.0, 700.0);
        session.wheel(0.0, -300.0, 400.0, 300.0, false, true);
        session.wheel(40.0, 20.0, 400.0, 300.0, false, false);
        session.begin_pan(500.0, 400.0);
        session.pan_to(350.0, 380.0);
        session.end_pan();
        let zoom = session.zoom_percent();
        let c = corner(&session);
        for _ in 0..50 {
            session.keep_view_origin_for_width_change(-280.0);
            session.resize_viewport(920.0, 700.0);
            assert_eq!(corner(&session), c, "collapsed, dpr {dpr}");
            session.keep_view_origin_for_width_change(280.0);
            session.resize_viewport(1200.0, 700.0);
            assert_eq!(corner(&session), c, "expanded, dpr {dpr}");
        }
        assert_eq!(session.zoom_percent(), zoom, "zoom is never touched");
    }
}

#[test]
fn a_non_finite_toggle_delta_cannot_corrupt_the_view() {
    let mut session = rectangles(1);
    session.resize_viewport(1000.0, 600.0);
    let c = corner(&session);
    for delta in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        session.keep_view_origin_for_width_change(delta);
        session.resize_viewport(720.0, 600.0);
        session.resize_viewport(1000.0, 600.0);
        let now = corner(&session);
        assert!(now.0.is_finite() && now.1.is_finite(), "{delta}");
        assert_eq!(now, c, "{delta}: a refused delta changes nothing");
    }
}

#[test]
fn a_zero_delta_keeps_the_origin() {
    let mut session = rectangles(1);
    session.resize_viewport(1000.0, 600.0);
    let c = corner(&session);
    session.keep_view_origin_for_width_change(0.0);
    session.resize_viewport(1000.0, 600.0);
    assert_eq!(corner(&session), c);
    session.keep_view_origin_for_width_change(-280.0);
    session.resize_viewport(720.0, 600.0);
    assert_eq!(corner(&session), c);
}

// ---------------------------------------------------------------------
// pointer_is_down (used by Shift+Ctrl+F, criterion 38)
// ---------------------------------------------------------------------

#[test]
fn the_pointer_is_down_between_press_and_release_only() {
    let mut session = rectangles(1);
    assert!(!session.is_pointer_down());
    session.pointer_down(Point::new(0.0, 5.0), false);
    assert!(session.is_pointer_down());
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    assert!(!session.is_pointer_down());

    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_cancelled();
    assert!(!session.is_pointer_down(), "a cancelled press is not held");

    session.set_tool(Tool::Rectangle);
    session.pointer_down(Point::new(100.0, 100.0), false);
    session.escape();
    session.pointer_up(Point::new(120.0, 120.0), false, false);
    assert!(!session.is_pointer_down());
}

#[test]
fn choosing_the_pressed_fill_stroke_join_or_dash_again_records_no_operation() {
    let mut session = rectangles(1);
    session.set_fill_paint(true);
    session.set_stroke_dash(DashChoice::Dash);
    session.set_stroke_join(LineJoin::Bevel);
    session.set_stroke_cap(LineCap::Square);
    session.set_stroke_paint(false);
    let v = version(&session);
    // A click on the item that is already pressed (the toggle strips send it).
    session.set_fill_paint(true);
    session.set_stroke_dash(DashChoice::Dash);
    session.set_stroke_join(LineJoin::Bevel);
    session.set_stroke_cap(LineCap::Square);
    session.set_stroke_paint(false);
    assert_eq!(version(&session), v);
}

#[test]
fn a_press_on_empty_canvas_with_a_pending_drag_commits_it_to_the_started_objects() {
    let mut session = rectangles(2);
    select_all(&mut session, 2);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    let empty = Point::new(900.0, 900.0);
    session.pointer_down(empty, false);
    session.pointer_up(empty, false, false);
    session.commit_style_preview();
    assert!(
        styles(&session).iter().all(|s| s.stroke.opacity == pct(40)),
        "both objects the drag started on, once"
    );
    assert_eq!(session.style_panel_state().subject, "Nothing selected");
}
