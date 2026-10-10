//! What the Markers group shows (`specs/0018-stroke-markers` criteria 1 to 3,
//! 21, 22, 24, 31 and 32): present for paths only, gone with the stroke, Place
//! and Count following the Middle slot, mixed over the paths of the selection.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    AnchorId, Document, Length, MarkerCount, MarkerPlace, MarkerShape, NewAnchor, NodeId,
    ObjectSnapshot, Point, RectBounds, StyleEdit,
};
use curvyo_ui_core::{
    BarValue, MarkersPanel, NodeSelection, ObjectSelection, StyleEntryError, StyleField, StyleTool,
    parse_marker_count, style_panel_state, style_scope,
};

fn path(document: &Document, n: u64, closed: bool) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, n), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, n + 1), Point::new(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, n + 2), Point::new(10.0, 10.0)),
        ],
        closed,
    )
}

fn square(document: &Document) -> NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(30.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn panel(document: &Document, ids: &[NodeId]) -> Option<MarkersPanel> {
    let objects = objects(document);
    let mut selection = ObjectSelection::new();
    selection.set(ids);
    let scope = style_scope(
        StyleTool::Other,
        &objects,
        &selection,
        &NodeSelection::new(),
    );
    style_panel_state(&objects, &objects, &scope)
        .expect("something to edit")
        .stroke
        .markers
}

fn set(document: &Document, ids: &[NodeId], edit: &StyleEdit) {
    document.edit_style(ids, edit).unwrap();
}

#[test]
fn a_path_shows_the_group_with_every_slot_none_and_no_place_or_count() {
    let document = Document::new(1);
    let id = path(&document, 1, false);
    let markers = panel(&document, &[id]).unwrap();
    assert_eq!(markers.start, BarValue::Uniform(MarkerShape::None));
    assert_eq!(markers.mid, BarValue::Uniform(MarkerShape::None));
    assert_eq!(markers.end, BarValue::Uniform(MarkerShape::None));
    assert!(!markers.place_shown && !markers.count_shown);
    assert!(!markers.closed_note);
}

#[test]
fn primitives_alone_have_no_group_and_a_mixed_selection_shows_it_for_the_paths() {
    let document = Document::new(1);
    let p = path(&document, 1, false);
    let r = square(&document);
    assert_eq!(panel(&document, &[r]), None, "criterion 21");
    set(&document, &[p], &StyleEdit::MarkerEnd(MarkerShape::Arrow));
    let markers = panel(&document, &[p, r]).unwrap();
    assert_eq!(
        markers.end,
        BarValue::Uniform(MarkerShape::Arrow),
        "the primitive does not make it mixed (criterion 22)"
    );
}

#[test]
fn no_stroke_removes_the_group_and_the_settings_come_back() {
    let document = Document::new(1);
    let id = path(&document, 1, false);
    set(&document, &[id], &StyleEdit::MarkerStart(MarkerShape::Dot));
    set(&document, &[id], &StyleEdit::StrokeEnabled(false));
    assert_eq!(panel(&document, &[id]), None, "criterion 3");
    set(&document, &[id], &StyleEdit::StrokeEnabled(true));
    assert_eq!(
        panel(&document, &[id]).unwrap().start,
        BarValue::Uniform(MarkerShape::Dot)
    );
}

#[test]
fn place_follows_the_middle_slot_and_count_follows_place() {
    let document = Document::new(1);
    let id = path(&document, 1, false);
    set(&document, &[id], &StyleEdit::MarkerMid(MarkerShape::Arrow));
    let markers = panel(&document, &[id]).unwrap();
    assert!(markers.place_shown && markers.count_shown);
    assert_eq!(markers.place, BarValue::Uniform(MarkerPlace::Spaced));
    set(
        &document,
        &[id],
        &StyleEdit::MarkerCount(MarkerCount::new(4).unwrap()),
    );
    set(
        &document,
        &[id],
        &StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
    );
    let markers = panel(&document, &[id]).unwrap();
    assert!(
        markers.place_shown && !markers.count_shown,
        "At nodes has no Count"
    );
    assert_eq!(
        markers.count,
        BarValue::Uniform(MarkerCount::new(4).unwrap()),
        "the stored count is kept"
    );
    set(&document, &[id], &StyleEdit::MarkerMid(MarkerShape::None));
    let markers = panel(&document, &[id]).unwrap();
    assert!(!markers.place_shown && !markers.count_shown);
}

#[test]
fn different_paths_show_mixed_slots_and_a_mixed_middle_shows_place_and_count() {
    let document = Document::new(1);
    let a = path(&document, 1, false);
    let b = path(&document, 10, false);
    set(&document, &[a], &StyleEdit::MarkerStart(MarkerShape::Arrow));
    set(&document, &[a], &StyleEdit::MarkerMid(MarkerShape::Dot));
    set(
        &document,
        &[a],
        &StyleEdit::MarkerCount(MarkerCount::new(3).unwrap()),
    );
    let markers = panel(&document, &[a, b]).unwrap();
    assert_eq!(markers.start, BarValue::Mixed);
    assert_eq!(markers.mid, BarValue::Mixed);
    assert_eq!(markers.end, BarValue::Uniform(MarkerShape::None));
    assert_eq!(markers.count, BarValue::Mixed);
    assert!(markers.place_shown && markers.count_shown);
}

#[test]
fn the_closed_path_line_shows_only_when_every_path_is_closed_and_a_slot_is_set() {
    let document = Document::new(1);
    let ring = path(&document, 1, true);
    assert!(!panel(&document, &[ring]).unwrap().closed_note, "both None");
    set(
        &document,
        &[ring],
        &StyleEdit::MarkerEnd(MarkerShape::Arrow),
    );
    assert!(panel(&document, &[ring]).unwrap().closed_note);
    let open = path(&document, 10, false);
    assert!(
        !panel(&document, &[ring, open]).unwrap().closed_note,
        "a mix"
    );
    set(
        &document,
        &[open],
        &StyleEdit::MarkerStart(MarkerShape::Dot),
    );
    assert!(
        !panel(&document, &[open]).unwrap().closed_note,
        "an open path"
    );
}

#[test]
fn a_typed_count_is_a_whole_number_from_one_to_five_hundred() {
    assert_eq!(parse_marker_count("1").unwrap().get(), 1);
    assert_eq!(parse_marker_count(" 500 ").unwrap().get(), 500);
    for bad in ["0", "2.5", "501", "-1", "", "x", "1,5", "1e2", "٣"] {
        assert_eq!(
            parse_marker_count(bad),
            Err(StyleEntryError::Count),
            "{bad:?}"
        );
    }
    assert_eq!(StyleEntryError::Count.code(), "count");
    assert_eq!(
        StyleField::MarkerCount.parse_text("7"),
        Ok(StyleEdit::MarkerCount(MarkerCount::new(7).unwrap()))
    );
    assert_eq!(
        StyleField::from_name("marker-count"),
        Some(StyleField::MarkerCount)
    );
}

#[test]
fn a_marker_edit_goes_to_the_paths_of_the_scope_and_any_other_edit_to_all_of_it() {
    let document = Document::new(1);
    let line = path(&document, 1, false);
    let rect = square(&document);
    let objects = objects(&document);
    let mut selection = ObjectSelection::new();
    selection.set(&[line, rect]);
    let scope = style_scope(
        StyleTool::Other,
        &objects,
        &selection,
        &NodeSelection::new(),
    );
    let marker = StyleEdit::MarkerStart(MarkerShape::Arrow);
    assert_eq!(scope.targets(&objects, &marker), vec![line]);
    let width = StyleEdit::StrokeWidth(Length::from_mm(2.0));
    assert_eq!(scope.targets(&objects, &width), vec![line, rect]);
    selection.set(&[rect]);
    let only_rect = style_scope(
        StyleTool::Other,
        &objects,
        &selection,
        &NodeSelection::new(),
    );
    assert_eq!(only_rect.targets(&objects, &marker), Vec::<NodeId>::new());
}
