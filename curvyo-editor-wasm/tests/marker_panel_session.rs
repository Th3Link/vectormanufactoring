//! `Session`-level checks of the Markers group (`specs/0018-stroke-markers`
//! criteria 1 to 3, 21, 22, 24, 30 to 32): a choice commits to the paths of the
//! selection and never to a primitive, Count follows the value field rules, and
//! the view carries what the host shows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Document, MarkerCount, MarkerPlace, MarkerShape, NewAnchor, ObjectSnapshot, Point,
    RectBounds, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{Grid, MarkerSlot, StyleEntryError, StyleField, ValueField};

/// A session with an open path (selected by `select_all`), a closed path and a
/// rectangle, built through the document.
fn session() -> Session {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(50.0, 0.0)),
        ],
        false,
    );
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 3), Point::new(100.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 4), Point::new(150.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 5), Point::new(150.0, 50.0)),
        ],
        true,
    );
    let _ = document.create_rect(RectBounds {
        origin: Point::new(200.0, 0.0),
        width: curvyo_document_core::Length::from_mm(40.0),
        height: curvyo_document_core::Length::from_mm(40.0),
    });
    Session::open(2, &curvyo_document_core::pack(&document, "0.1.0").unwrap()).unwrap()
}

fn styles(session: &Session) -> Vec<Style> {
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

fn select(session: &mut Session, points: &[(f64, f64)]) {
    session.set_tool(Tool::Select);
    for (n, (x, y)) in points.iter().enumerate() {
        let at = Point::new(*x, *y);
        session.pointer_hover(at, n > 0, false);
        session.pointer_down(at, n > 0);
        session.pointer_up(at, n > 0, false);
    }
}

const OPEN: (f64, f64) = (25.0, 0.0);
const CLOSED: (f64, f64) = (125.0, 0.0);
const RECT: (f64, f64) = (200.0, 20.0);

#[test]
fn a_choice_commits_to_the_paths_only_and_the_rectangle_is_untouched() {
    let mut s = session();
    select(&mut s, &[OPEN, CLOSED, RECT]);
    assert_eq!(s.selected_object_count(), 3);
    assert!(s.style_panel_view().markers_shown);
    s.set_marker_shape(MarkerSlot::End, MarkerShape::Arrow);
    let all = styles(&s);
    assert_eq!(all[0].stroke.markers.end, MarkerShape::Arrow);
    assert_eq!(all[1].stroke.markers.end, MarkerShape::Arrow);
    assert_eq!(all[2].stroke.markers.end, MarkerShape::None, "criterion 22");
    // Value field edits of Count reach the paths and do not fail on the rectangle.
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    s.set_style_text(StyleField::MarkerCount, "4").unwrap();
    let all = styles(&s);
    assert_eq!(all[0].stroke.markers.mid_count.get(), 4);
    assert_eq!(all[2].stroke.markers.mid_count.get(), 1);
}

#[test]
fn only_a_primitive_selected_shows_no_group_and_a_choice_writes_nothing() {
    let mut s = session();
    select(&mut s, &[RECT]);
    let view = s.style_panel_view();
    assert!(!view.markers_shown);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    assert_eq!(styles(&s)[2].stroke.markers.start, MarkerShape::None);
}

#[test]
fn the_view_names_the_slots_place_count_and_the_closed_line() {
    let mut s = session();
    select(&mut s, &[OPEN]);
    let view = s.style_panel_view();
    assert_eq!(
        (
            view.marker_start.as_str(),
            view.marker_mid.as_str(),
            view.marker_end.as_str()
        ),
        ("none", "none", "none")
    );
    assert!(!view.marker_place_shown && !view.marker_count_shown);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Arrow);
    let view = s.style_panel_view();
    assert!(view.marker_place_shown && view.marker_count_shown);
    assert_eq!(view.marker_place, "spaced");
    assert_eq!(view.marker_count_text, "1");
    assert!(!view.marker_count_resettable);
    s.set_marker_place(MarkerPlace::AtNodes);
    let view = s.style_panel_view();
    assert!(view.marker_place_shown && !view.marker_count_shown);
    assert_eq!(view.marker_place, "nodes");

    select(&mut s, &[CLOSED]);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Dot);
    assert!(s.style_panel_view().marker_closed_note);
    select(&mut s, &[CLOSED, OPEN]);
    assert!(!s.style_panel_view().marker_closed_note);
    assert_eq!(s.style_panel_view().marker_start, "mixed");
}

#[test]
fn count_drags_steps_resets_and_refuses_bad_text() {
    let mut s = session();
    select(&mut s, &[OPEN]);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    // A drag to the middle of the scale: 1 + 49 x 0.5 = 25.5, rounded to 26 (grid 1).
    s.preview_value_field(ValueField::MarkerCount, 0.5, Grid::Normal);
    assert_eq!(
        styles(&s)[0].stroke.markers.mid_count.get(),
        1,
        "not written yet"
    );
    assert_eq!(s.style_panel_view().marker_count_text, "26");
    s.commit_style_preview();
    assert_eq!(styles(&s)[0].stroke.markers.mid_count.get(), 26);
    s.step_value_field(ValueField::MarkerCount, -1, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(styles(&s)[0].stroke.markers.mid_count.get(), 25);
    assert!(s.style_panel_view().marker_count_resettable);
    s.reset_value_field(ValueField::MarkerCount);
    assert_eq!(styles(&s)[0].stroke.markers.mid_count, MarkerCount::ONE);
    for bad in ["0", "2.5", "501", "x"] {
        assert_eq!(
            s.set_style_text(StyleField::MarkerCount, bad),
            Err(StyleEntryError::Count),
            "{bad}"
        );
    }
    assert_eq!(styles(&s)[0].stroke.markers.mid_count, MarkerCount::ONE);
    // Home is 1, End is 50 (a drag to the ends of the scale).
    s.preview_value_field(ValueField::MarkerCount, 1.0, Grid::Normal);
    s.commit_style_preview();
    assert_eq!(styles(&s)[0].stroke.markers.mid_count.get(), 50);
}

#[test]
fn different_counts_show_mixed_and_a_choice_applies_to_all_paths() {
    let mut s = session();
    select(&mut s, &[OPEN]);
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Dot);
    s.set_style_text(StyleField::MarkerCount, "3").unwrap();
    select(&mut s, &[OPEN, CLOSED]);
    let view = s.style_panel_view();
    assert!(view.marker_count_mixed);
    assert_eq!(view.marker_mid, "mixed");
    s.set_marker_shape(MarkerSlot::Mid, MarkerShape::Arrow);
    let all = styles(&s);
    assert_eq!(all[0].stroke.markers.mid, MarkerShape::Arrow);
    assert_eq!(all[1].stroke.markers.mid, MarkerShape::Arrow);
    assert_eq!(
        all[0].stroke.markers.mid_count.get(),
        3,
        "each keeps its other settings"
    );
}

#[test]
fn the_group_leaves_with_the_stroke_and_the_settings_stay() {
    let mut s = session();
    select(&mut s, &[OPEN]);
    s.set_marker_shape(MarkerSlot::Start, MarkerShape::Arrow);
    s.set_stroke_paint(false);
    assert!(!s.style_panel_view().markers_shown);
    s.set_stroke_paint(true);
    assert_eq!(s.style_panel_view().marker_start, "arrow");
}
