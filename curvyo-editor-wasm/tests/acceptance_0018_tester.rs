//! Independent tester cases for `0018-stroke-markers` at the `Session` level:
//! the Markers group of the panel (when it shows, Place and Count, mixed,
//! closed-path note), the commit rules, `NotAPath` routing, the Count field
//! (typed, dragged, Escape, reset), and markers outside hit-testing, selection,
//! move preview, eyedropper; copy, object to path, file round trip. Written
//! from `specification.md` and `adrs.md` before the implementation was read.
//!
//! Criteria: 1, 2, 3, 18, 19, 20, 21, 22, 23, 24, 25, 28, 30, 31, 32.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    missing_docs
)]

use curvyo_document_core::{
    Document, MarkerCount, MarkerPlace, MarkerShape, Markers, ObjectSnapshot, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{
    BarValue, Grid, MarkerSlot, PaintTarget, PanelContent, StyleEntryError, StyleField,
    StylePanelState, ValueField,
};

// ---------------------------------------------------------------- helpers

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn click(s: &mut Session, at: Point) {
    s.pointer_hover(at, false, false);
    s.pointer_down(at, false);
    s.pointer_up(at, false, false);
}

/// Draws a path with the Pen through `points` and finishes it (open) or closes
/// it on the first point.
fn pen(s: &mut Session, points: &[(f64, f64)], close: bool) {
    s.set_tool(Tool::Pen);
    for (x, y) in points {
        click(s, pt(*x, *y));
    }
    if close {
        click(s, pt(points[0].0, points[0].1));
    } else {
        s.finish_pen();
    }
    s.set_tool(Tool::Select);
}

fn rectangle(s: &mut Session, x0: f64, y0: f64, x1: f64, y1: f64) {
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(x0, y0), false);
    s.pointer_up(pt(x1, y1), false, false);
    s.set_tool(Tool::Select);
}

/// Selects everything whose bounds a marquee from `(x0,y0)` to `(x1,y1)`
/// encloses.
fn marquee(s: &mut Session, x0: f64, y0: f64, x1: f64, y1: f64) {
    s.set_tool(Tool::Select);
    s.escape();
    s.pointer_hover(pt(x0, y0), false, false);
    s.pointer_down(pt(x0, y0), false);
    s.pointer_hover(pt(x1, y1), false, false);
    s.pointer_up(pt(x1, y1), false, false);
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn styles(s: &Session) -> Vec<Style> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn markers(s: &Session) -> Vec<Markers> {
    styles(s).into_iter().map(|st| st.stroke.markers).collect()
}

fn changes(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn ops(s: &Session) -> i64 {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.oplog_vv().values().map(|c| i64::from(*c)).sum()
}

/// A commit with another label, so the next style commit cannot merge into the
/// previous change.
fn baseline(s: &mut Session) -> usize {
    let unit = if s.display_unit() == curvyo_document_core::DisplayUnit::In {
        curvyo_document_core::DisplayUnit::Mm
    } else {
        curvyo_document_core::DisplayUnit::In
    };
    assert!(s.set_display_unit(unit));
    changes(s)
}

fn state(s: &Session) -> StylePanelState {
    s.style_panel_state().expect("a style panel")
}

fn mk(s: &Session) -> curvyo_ui_core::MarkersPanel {
    state(s).stroke.markers.expect("the Markers group shows")
}

/// A session with one open 3-node path `(20,100) (80,100) (140,100)` selected.
fn one_path() -> Session {
    let mut s = Session::new(1);
    pen(
        &mut s,
        &[(20.0, 100.0), (80.0, 100.0), (140.0, 100.0)],
        false,
    );
    marquee(&mut s, 10.0, 90.0, 150.0, 110.0);
    assert_eq!(s.selected_object_count(), 1);
    s
}

// ----------------------------------------------- compositing (CPU oracle)

fn in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if d.abs() < 1e-18 {
        return false;
    }
    let l1 = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d;
    let l2 = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
    let l3 = 1.0 - l1 - l2;
    l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9
}

/// Is any artwork triangle (not the overlay) covering `p`?
fn artwork_covers(list: &curvyo_render_core::DrawList, p: Point) -> bool {
    let end = list.overlay_start();
    list.triangles[..end]
        .as_chunks::<3>()
        .0
        .iter()
        .any(|t| in_triangle(p, t[0].position, t[1].position, t[2].position))
}

// ============================================== AC 1, 3, 21: when it shows

#[test]
fn nothing_selected_shows_no_style_panel_and_a_path_shows_the_markers_with_defaults() {
    let s = Session::new(1);
    assert!(s.style_panel_state().is_none());
    let s = one_path();
    let m = mk(&s);
    assert_eq!(m.start, BarValue::Uniform(MarkerShape::None));
    assert_eq!(m.mid, BarValue::Uniform(MarkerShape::None));
    assert_eq!(m.end, BarValue::Uniform(MarkerShape::None));
    assert!(!m.place_shown);
    assert!(!m.count_shown);
    assert!(!m.closed_note);
    assert_eq!(s.panel_content(), PanelContent::Style);
}

#[test]
fn a_selection_of_only_primitives_has_no_markers_group() {
    let mut s = Session::new(1);
    rectangle(&mut s, 20.0, 20.0, 60.0, 60.0);
    marquee(&mut s, 10.0, 10.0, 70.0, 70.0);
    assert_eq!(s.selected_object_count(), 1);
    assert!(state(&s).stroke.markers.is_none());
    // A call anyway writes nothing.
    let before = (ops(&s), markers(&s));
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    s.set_marker_place(MarkerPlace::AtNodes);
    let _ = s.set_style_text(StyleField::MarkerCount, "4");
    assert_eq!((ops(&s), markers(&s)), before);
}

#[test]
fn stroke_off_or_width_zero_removes_the_group_and_keeps_the_settings() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let _ = s.set_style_text(StyleField::MarkerCount, "6").unwrap();
    let want = markers(&s);
    s.set_stroke_paint(false);
    assert!(state(&s).stroke.markers.is_none(), "Paint None");
    assert_eq!(markers(&s), want);
    s.set_stroke_paint(true);
    assert!(state(&s).stroke.markers.is_some());
    s.set_style_text(StyleField::StrokeWidth, "0").unwrap();
    assert!(state(&s).stroke.markers.is_none(), "width 0");
    assert_eq!(markers(&s), want);
    s.set_style_text(StyleField::StrokeWidth, "1").unwrap();
    let m = mk(&s);
    assert_eq!(m.start, BarValue::Uniform(MarkerShape::Arrow));
    assert_eq!(m.mid, BarValue::Uniform(MarkerShape::Dot));
    assert_eq!(markers(&s), want);
}

// =========================================== AC 2, 6, 22, 24, 30: choices

#[test]
fn place_and_count_show_only_for_a_middle_shape_and_spaced() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let m = mk(&s);
    assert!(m.place_shown && m.count_shown);
    assert_eq!(m.place, BarValue::Uniform(MarkerPlace::Spaced));
    assert_eq!(m.count, BarValue::Uniform(MarkerCount::new(1).unwrap()));
    s.set_marker_place(MarkerPlace::AtNodes);
    let m = mk(&s);
    assert!(m.place_shown && !m.count_shown, "At nodes hides Count");
    s.set_marker_place(MarkerPlace::Spaced);
    assert!(mk(&s).count_shown);
    let _ = s.set_style_text(StyleField::MarkerCount, "9").unwrap();
    s.set_marker_place(MarkerPlace::AtNodes);
    s.set_marker_place(MarkerPlace::Spaced);
    assert_eq!(
        mk(&s).count,
        BarValue::Uniform(MarkerCount::new(9).unwrap()),
        "Count kept while hidden"
    );
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::None);
    let m = mk(&s);
    assert!(!m.place_shown && !m.count_shown);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Arrow);
    assert_eq!(
        mk(&s).count,
        BarValue::Uniform(MarkerCount::new(9).unwrap())
    );
}

#[test]
fn a_choice_is_one_commit_and_pressing_the_pressed_item_writes_nothing() {
    let mut s = one_path();
    let c = baseline(&mut s);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    assert_eq!(changes(&s), c + 1);
    assert_eq!(markers(&s)[0].end, MarkerShape::Arrow);
    let ops_before = ops(&s);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    assert_eq!(ops(&s), ops_before, "pressed item pressed again");
    // Other slots independent.
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Dot);
    let m = markers(&s)[0];
    assert_eq!(
        (m.start, m.mid, m.end),
        (MarkerShape::Dot, MarkerShape::None, MarkerShape::Arrow)
    );
}

#[test]
fn several_paths_with_different_settings_show_mixed_and_a_choice_applies_to_all_in_one_commit() {
    let mut s = Session::new(1);
    pen(&mut s, &[(20.0, 40.0), (80.0, 40.0), (140.0, 40.0)], false);
    pen(
        &mut s,
        &[(20.0, 100.0), (80.0, 100.0), (140.0, 100.0)],
        false,
    );
    marquee(&mut s, 10.0, 30.0, 150.0, 50.0);
    assert_eq!(s.selected_object_count(), 1);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let _ = s.set_style_text(StyleField::MarkerCount, "7").unwrap();
    marquee(&mut s, 10.0, 90.0, 150.0, 110.0);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Arrow);
    s.set_marker_place(MarkerPlace::AtNodes);
    marquee(&mut s, 5.0, 20.0, 160.0, 120.0);
    assert_eq!(s.selected_object_count(), 2);
    let m = mk(&s);
    assert_eq!(m.start, BarValue::Mixed);
    assert_eq!(m.mid, BarValue::Mixed);
    assert_eq!(m.end, BarValue::Uniform(MarkerShape::None));
    assert_eq!(m.place, BarValue::Mixed);
    assert!(m.place_shown);
    let c = baseline(&mut s);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    assert_eq!(changes(&s), c + 1, "one commit for both paths");
    let all = markers(&s);
    assert!(all.iter().all(|m| m.end == MarkerShape::Dot));
    // Each keeps its other settings.
    assert!(all.iter().any(|m| m.start == MarkerShape::Arrow
        && m.mid_count.get() == 7
        && m.mid == MarkerShape::Dot));
    assert!(all.iter().any(|m| m.start == MarkerShape::None
        && m.mid == MarkerShape::Arrow
        && m.mid_place == MarkerPlace::AtNodes));
    // A value applies to all.
    let c = baseline(&mut s);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Dot);
    assert_eq!(changes(&s), c + 1);
    assert!(markers(&s).iter().all(|m| m.start == MarkerShape::Dot));
}

#[test]
fn a_mixed_count_shows_mixed_and_a_typed_value_applies_to_all() {
    let mut s = Session::new(1);
    pen(&mut s, &[(20.0, 40.0), (140.0, 40.0)], false);
    pen(&mut s, &[(20.0, 100.0), (140.0, 100.0)], false);
    marquee(&mut s, 10.0, 30.0, 150.0, 50.0);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let _ = s.set_style_text(StyleField::MarkerCount, "3").unwrap();
    marquee(&mut s, 10.0, 90.0, 150.0, 110.0);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let _ = s.set_style_text(StyleField::MarkerCount, "5").unwrap();
    marquee(&mut s, 5.0, 20.0, 160.0, 120.0);
    assert_eq!(s.selected_object_count(), 2);
    let m = mk(&s);
    assert_eq!(m.count, BarValue::Mixed);
    assert!(
        state(&s).value_of(ValueField::MarkerCount).is_none(),
        "the field reads Mixed"
    );
    let c = baseline(&mut s);
    assert_eq!(s.set_style_text(StyleField::MarkerCount, "12"), Ok(true));
    assert_eq!(changes(&s), c + 1);
    assert!(markers(&s).iter().all(|m| m.mid_count.get() == 12));
    // Reset applies to all and writes the default.
    assert!(s.reset_value_field(ValueField::MarkerCount));
    assert!(markers(&s).iter().all(|m| m.mid_count.get() == 1));
}

#[test]
fn a_mixed_selection_of_paths_and_primitives_edits_the_paths_in_one_commit() {
    let mut s = Session::new(1);
    pen(
        &mut s,
        &[(20.0, 100.0), (80.0, 100.0), (140.0, 100.0)],
        false,
    );
    rectangle(&mut s, 20.0, 150.0, 80.0, 200.0);
    marquee(&mut s, 5.0, 80.0, 160.0, 220.0);
    assert_eq!(s.selected_object_count(), 2);
    assert!(state(&s).stroke.markers.is_some(), "shown for the paths");
    let c = baseline(&mut s);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    assert_eq!(changes(&s), c + 1, "one commit");
    let st = styles(&s);
    let d = doc_of(&s);
    for (id, style) in d.object_ids().into_iter().zip(st) {
        match d.object(id).unwrap() {
            ObjectSnapshot::Path(_) => assert_eq!(style.stroke.markers.end, MarkerShape::Arrow),
            ObjectSnapshot::Primitive(_) => {
                assert_eq!(
                    style.stroke.markers,
                    Markers::default(),
                    "primitive untouched"
                );
            }
        }
    }
    // Place and Count too.
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    s.set_marker_place(MarkerPlace::AtNodes);
    let _ = s.set_style_text(StyleField::MarkerCount, "5");
    for (id, style) in d.object_ids().into_iter().zip(styles(&s)) {
        if matches!(d.object(id).unwrap(), ObjectSnapshot::Primitive(_)) {
            assert_eq!(style.stroke.markers, Markers::default());
        }
    }
}

// ================================================ AC 30, 31: the Count field

#[test]
fn a_typed_count_is_validated_and_a_refusal_writes_nothing() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    assert_eq!(s.set_style_text(StyleField::MarkerCount, "4"), Ok(true));
    let before = (ops(&s), markers(&s));
    for bad in ["0", "2.5", "501", "text", "", "-3", "1e9"] {
        assert_eq!(
            s.set_style_text(StyleField::MarkerCount, bad),
            Err(StyleEntryError::Count),
            "{bad:?}"
        );
    }
    assert_eq!((ops(&s), markers(&s)), before);
    assert_eq!(s.set_style_text(StyleField::MarkerCount, "500"), Ok(true));
    assert_eq!(markers(&s)[0].mid_count.get(), 500);
    assert_eq!(s.set_style_text(StyleField::MarkerCount, "1"), Ok(true));
    assert_eq!(markers(&s)[0].mid_count.get(), 1);
    // Typing the shown value again writes nothing.
    let o = ops(&s);
    let _ = s.set_style_text(StyleField::MarkerCount, "1");
    assert_eq!(ops(&s), o);
}

#[test]
fn a_count_drag_previews_then_commits_once_and_escape_reverts() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let c = baseline(&mut s);
    let o = ops(&s);
    // Ticks preview without writing.
    for p in [0.1, 0.3, 0.7, 1.0] {
        s.preview_value_field(ValueField::MarkerCount, p, Grid::Normal);
    }
    assert_eq!(ops(&s), o, "nothing written while dragging");
    assert_eq!(changes(&s), c);
    s.commit_style_preview();
    assert_eq!(changes(&s), c + 1, "one commit on release");
    assert_eq!(
        markers(&s)[0].mid_count.get(),
        50,
        "the end of the range is 50"
    );

    // Escape: back to the committed value, release writes nothing.
    let o = ops(&s);
    s.preview_value_field(ValueField::MarkerCount, 0.0, Grid::Normal);
    s.cancel_style_preview();
    s.commit_style_preview();
    assert_eq!(ops(&s), o);
    assert_eq!(markers(&s)[0].mid_count.get(), 50);

    // p = 0 -> 1; coarse grid -> multiples of ten.
    s.preview_value_field(ValueField::MarkerCount, 0.0, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(markers(&s)[0].mid_count.get(), 1);
    s.preview_value_field(ValueField::MarkerCount, 0.5, Grid::Coarse);
    s.commit_style_preview();
    assert_eq!(markers(&s)[0].mid_count.get() % 10, 0);
}

#[test]
fn count_keys_step_reset_and_a_stored_count_above_the_range() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    let _ = s.set_style_text(StyleField::MarkerCount, "10").unwrap();
    s.step_value_field(ValueField::MarkerCount, 1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(markers(&s)[0].mid_count.get(), 11);
    s.step_value_field(ValueField::MarkerCount, -1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(markers(&s)[0].mid_count.get(), 10);
    assert!(s.reset_value_field(ValueField::MarkerCount));
    assert_eq!(markers(&s)[0].mid_count.get(), 1);
    let o = ops(&s);
    let _ = s.reset_value_field(ValueField::MarkerCount);
    assert_eq!(ops(&s), o, "reset at the default writes nothing");
    // Above the drag range, typed.
    let _ = s.set_style_text(StyleField::MarkerCount, "300").unwrap();
    assert_eq!(state(&s).value_of(ValueField::MarkerCount), Some(300.0));
    // Arrow key from a value above the drag range stays inside 1..=500.
    s.step_value_field(ValueField::MarkerCount, 1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(markers(&s)[0].mid_count.get(), 301);
}

// ===================================================== AC 32: the closed note

#[test]
fn the_closed_path_note_shows_only_when_every_path_is_closed_and_a_start_or_end_is_set() {
    let mut s = Session::new(1);
    pen(
        &mut s,
        &[(20.0, 40.0), (80.0, 40.0), (80.0, 80.0), (20.0, 80.0)],
        true,
    );
    marquee(&mut s, 10.0, 30.0, 90.0, 90.0);
    assert_eq!(s.selected_object_count(), 1);
    assert!(!mk(&s).closed_note, "both None: no note");
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    assert!(mk(&s).closed_note);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::None);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    assert!(mk(&s).closed_note);
    // Middle alone: no note.
    s.set_marker_shape(MarkerSlot::End, MarkerShape::None);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    assert!(!mk(&s).closed_note);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    // A mix of open and closed: no note.
    pen(&mut s, &[(20.0, 150.0), (140.0, 150.0)], false);
    marquee(&mut s, 5.0, 20.0, 160.0, 170.0);
    assert_eq!(s.selected_object_count(), 2);
    assert!(!mk(&s).closed_note, "mix of open and closed");
    // Nothing is disabled/erased by the note: the settings stay.
    assert!(markers(&s).iter().any(|m| m.end == MarkerShape::Arrow));
}

// ======================================================= AC 18, 19, 20

fn path_with_big_markers() -> Session {
    let mut s = Session::new(1);
    pen(&mut s, &[(40.0, 100.0), (120.0, 100.0)], false);
    marquee(&mut s, 30.0, 90.0, 130.0, 110.0);
    assert_eq!(s.selected_object_count(), 1);
    s.set_style_text(StyleField::StrokeWidth, "10").unwrap();
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Dot);
    s
}

#[test]
fn a_click_hover_or_marquee_on_a_marker_alone_selects_nothing() {
    let mut s = path_with_big_markers();
    s.escape();
    assert_eq!(s.selected_object_count(), 0);
    // The end dot (radius 15) reaches 135; the path ends at x = 120.
    click(&mut s, pt(132.0, 100.0));
    assert_eq!(s.selected_object_count(), 0, "click on the end dot");
    click(&mut s, pt(120.0, 112.0));
    assert_eq!(
        s.selected_object_count(),
        0,
        "click on the dot above the line"
    );
    click(&mut s, pt(30.0, 100.0));
    assert_eq!(s.selected_object_count(), 0, "start dot");
    // A marquee that touches only the dot (outside the stroke) selects nothing.
    marquee(&mut s, 126.0, 108.0, 138.0, 112.0);
    assert_eq!(s.selected_object_count(), 0);
    // The line itself still clicks.
    click(&mut s, pt(80.0, 100.0));
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn the_selection_box_ignores_the_markers() {
    let mut s = path_with_big_markers();
    s.escape();
    click(&mut s, pt(80.0, 100.0));
    assert_eq!(s.selected_object_count(), 1);
    // The box handles sit on the outline bounds (x 40..120): a press at the
    // far right middle of the marker (x = 135) is not a handle press.
    let list = s.draw_list();
    let beyond = list
        .triangles
        .iter()
        .skip(list.overlay_start())
        .filter(|v| v.position.x > 134.0)
        .count();
    assert_eq!(beyond, 0, "no selection decoration out at the marker tip");
}

#[test]
fn the_eyedropper_does_not_pick_from_a_marker() {
    let mut s = path_with_big_markers();
    s.escape();
    // A None background: the empty page is a miss for the eyedropper (as before `0040`).
    s.set_background_paint(curvyo_document_core::BackgroundPaint::None);
    s.begin_colour_pick(PaintTarget::Stroke);
    s.pointer_hover(pt(132.0, 100.0), false, false);
    assert_eq!(s.colour_pick_hover(), None, "on the dot only");
    s.pointer_hover(pt(120.0, 113.0), false, false);
    assert_eq!(s.colour_pick_hover(), None);
    s.pointer_hover(pt(80.0, 100.0), false, false);
    assert!(s.colour_pick_hover().is_some(), "on the stroke");
    s.end_colour_pick();
}

#[test]
fn the_move_preview_draws_no_markers_and_the_committed_object_keeps_them() {
    let build = |marked: bool| {
        let mut s = Session::new(1);
        pen(&mut s, &[(40.0, 100.0), (120.0, 100.0)], false);
        marquee(&mut s, 30.0, 90.0, 130.0, 110.0);
        s.set_style_text(StyleField::StrokeWidth, "4").unwrap();
        if marked {
            s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
            s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
            s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
        }
        s.escape();
        click(&mut s, pt(60.0, 100.0));
        assert_eq!(s.selected_object_count(), 1);
        s
    };
    let marked_rest = build(true).draw_list().triangle_count();
    let plain_rest = build(false).draw_list().triangle_count();
    assert!(marked_rest > plain_rest);
    let drag = |marked: bool| {
        let mut s = build(marked);
        s.pointer_hover(pt(60.0, 100.0), false, false);
        s.pointer_down(pt(60.0, 100.0), false);
        s.pointer_hover(pt(60.0, 160.0), false, false);
        s.draw_list().triangle_count()
    };
    let marked_drag = drag(true);
    let plain_drag = drag(false);
    assert_eq!(
        marked_drag - plain_drag,
        marked_rest - plain_rest,
        "the preview adds no markers: only the committed object has them"
    );
    // After release the markers sit at the new place: the end dot of a moved path.
    let mut s = build(true);
    s.pointer_hover(pt(60.0, 100.0), false, false);
    s.pointer_down(pt(60.0, 100.0), false);
    s.pointer_hover(pt(60.0, 160.0), false, false);
    assert_eq!(
        s.live_readout().map(|r| r.text).as_deref(),
        Some("Δ 0.0, 60.0 mm")
    );
    s.pointer_up(pt(60.0, 160.0), false, false);
    let list = s.draw_list();
    assert!(
        artwork_covers(&list, pt(125.0, 160.0)),
        "dot moved with the object"
    );
    assert!(
        !artwork_covers(&list, pt(125.0, 100.0)),
        "nothing left behind"
    );
}

#[test]
fn a_node_drag_moves_the_end_marker_with_the_node() {
    let mut s = Session::new(1);
    pen(&mut s, &[(40.0, 100.0), (120.0, 100.0)], false);
    marquee(&mut s, 30.0, 90.0, 130.0, 110.0);
    s.set_style_text(StyleField::StrokeWidth, "4").unwrap();
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    s.set_tool(Tool::Node);
    s.pointer_hover(pt(120.0, 100.0), false, false);
    s.pointer_down(pt(120.0, 100.0), false);
    s.pointer_hover(pt(120.0, 160.0), false, false);
    let live = s.draw_list();
    assert!(
        artwork_covers(&live, pt(120.0, 166.0)),
        "the dot (radius 6) follows the drag"
    );
    s.pointer_up(pt(120.0, 160.0), false, false);
    assert!(artwork_covers(&s.draw_list(), pt(120.0, 166.0)));
    assert!(!artwork_covers(&s.draw_list(), pt(120.0, 106.0)));
}

// ================================================ AC 23, 25, 28: other flows

#[test]
fn object_to_path_gives_a_path_with_no_markers_and_the_group_shows_defaults() {
    let mut s = Session::new(1);
    rectangle(&mut s, 20.0, 20.0, 60.0, 60.0);
    marquee(&mut s, 10.0, 10.0, 70.0, 70.0);
    s.convert_selected_to_paths();
    assert_eq!(s.selected_object_count(), 1);
    let m = mk(&s);
    assert_eq!(m.start, BarValue::Uniform(MarkerShape::None));
    assert_eq!(m.mid, BarValue::Uniform(MarkerShape::None));
    assert_eq!(m.end, BarValue::Uniform(MarkerShape::None));
    assert_eq!(markers(&s)[0], Markers::default());
    // The converted path is closed: Start/End note when set.
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    assert!(mk(&s).closed_note);
}

#[test]
fn a_ctrl_drag_copy_keeps_the_settings_and_is_independent() {
    let mut s = Session::new(1);
    pen(
        &mut s,
        &[(40.0, 100.0), (80.0, 100.0), (120.0, 100.0)],
        false,
    );
    marquee(&mut s, 30.0, 90.0, 130.0, 110.0);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    s.set_marker_place(MarkerPlace::AtNodes);
    let want = markers(&s)[0];
    s.escape();
    click(&mut s, pt(100.0, 100.0));
    s.modifiers_changed(false, true, false);
    s.pointer_hover(pt(100.0, 100.0), false, true);
    s.pointer_down(pt(100.0, 100.0), false);
    s.pointer_hover(pt(100.0, 160.0), false, true);
    s.pointer_up(pt(100.0, 160.0), false, true);
    s.modifiers_changed(false, false, false);
    let all = markers(&s);
    assert_eq!(all.len(), 2, "a copy exists");
    assert_eq!(all[0], want);
    assert_eq!(all[1], want);
    // The selection is the copy: editing it leaves the original.
    s.set_marker_shape(MarkerSlot::End, MarkerShape::None);
    let all = markers(&s);
    assert_eq!(all[0].end, MarkerShape::Arrow);
    assert_eq!(all[1].end, MarkerShape::None);
}

#[test]
fn settings_survive_save_and_open_and_the_picture_is_the_same() {
    let mut s = one_path();
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    s.set_marker_place(MarkerPlace::AtNodes);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    let bytes = s.pack("0.1.0").unwrap();
    let mut again = Session::open(5, &bytes).unwrap();
    assert_eq!(markers(&again), markers(&s));
    let (a, b) = (again.draw_list(), s.draw_list());
    assert_eq!(
        a.triangles[..a.overlay_start()],
        b.triangles[..b.overlay_start()]
    );
    marquee(&mut again, 10.0, 90.0, 150.0, 110.0);
    let m = mk(&again);
    assert_eq!(m.mid, BarValue::Uniform(MarkerShape::Dot));
    assert_eq!(m.place, BarValue::Uniform(MarkerPlace::AtNodes));
}

#[test]
fn the_golden_marker_file_opens_in_a_session_and_draws_without_panic() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("curvyo-document-core/tests/fixtures/markers_v9.curvyo"),
    )
    .unwrap();
    let mut s = Session::open(3, &bytes).unwrap();
    let list = s.draw_list();
    assert!(list.triangle_count() > 0);
    assert!(
        list.triangles
            .iter()
            .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
    );
    // Select everything: the panel shows the group, count of 501 does not break it.
    marquee(&mut s, -5000.0, -5000.0, 5000.0, 5000.0);
    let n = s.selected_object_count();
    assert!(n >= 1);
    if let Some(m) = state(&s).stroke.markers {
        println!("golden panel: {m:?}");
    }
    // Nothing was rewritten by opening and selecting.
    let o = ops(&s);
    let _ = state(&s);
    assert_eq!(ops(&s), o);
}

#[test]
fn a_resize_moves_the_markers_with_the_geometry_without_stretching_them() {
    let mut s = Session::new(1);
    pen(&mut s, &[(40.0, 100.0), (120.0, 100.0)], false);
    marquee(&mut s, 30.0, 90.0, 130.0, 110.0);
    s.set_style_text(StyleField::StrokeWidth, "4").unwrap();
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Dot);
    s.escape();
    click(&mut s, pt(60.0, 100.0));
    assert_eq!(s.selected_object_count(), 1);
    // Drag the right middle handle from x = 120 to x = 200.
    s.pointer_hover(pt(120.0, 100.0), false, false);
    s.pointer_down(pt(120.0, 100.0), false);
    s.pointer_hover(pt(200.0, 100.0), false, false);
    s.pointer_up(pt(200.0, 100.0), false, false);
    let d = doc_of(&s);
    let end_x = match d.object(d.object_ids()[0]).unwrap() {
        ObjectSnapshot::Path(p) => p.anchors.last().unwrap().point.x,
        ObjectSnapshot::Primitive(_) => unreachable!(),
    };
    assert!((end_x - 200.0).abs() < 1e-6, "resized to x = {end_x}");
    let list = s.draw_list();
    // The dot is still a circle of radius 3 x width / 2 = 6 around the new end.
    assert!(artwork_covers(&list, pt(200.0, 105.0)));
    assert!(!artwork_covers(&list, pt(200.0, 107.0)));
    assert!(artwork_covers(&list, pt(205.0, 100.0)));
    assert!(!artwork_covers(&list, pt(207.0, 100.0)));
    assert!(
        !artwork_covers(&list, pt(120.0, 105.0)),
        "none left at the old end"
    );
}
