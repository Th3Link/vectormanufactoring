//! The Style panel's state and edit logic (`specs/0007-stroke-and-fill-styling`,
//! criteria 1, 2, 5, 6, 13, 14, 24, 36, 37): which objects the panel edits per
//! tool, what it shows (a shared value or "Mixed"), how typed values parse,
//! and how a drag preview and its single commit behave.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    Color, DashPattern, Document, EllipseFrame, FillMode, FillModeTarget, Length, LineCap,
    LineJoin, NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StyleEdit,
};
use curvyo_ui_core::{
    BarValue, DashChoice, NodeSelection, ObjectSelection, StyleEditor, StyleField, StyleTool,
    parse_hex, parse_opacity_percent, parse_stroke_width, style_panel_state, style_scope,
};

fn square(document: &Document, x: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn circle(document: &Document, x: f64) -> NodeId {
    document.create_ellipse(EllipseFrame {
        center: Point::new(x, 5.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(5.0),
    })
}

fn line(document: &Document, n: u64, y: f64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(
                curvyo_document_core::AnchorId::new(1, n),
                Point::new(0.0, y),
            ),
            NewAnchor::corner(
                curvyo_document_core::AnchorId::new(1, n + 1),
                Point::new(10.0, y),
            ),
        ],
        false,
    )
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn selected(ids: &[NodeId]) -> ObjectSelection {
    let mut selection = ObjectSelection::new();
    selection.set(ids);
    selection
}

fn edit(document: &Document, ids: &[NodeId], edit: &StyleEdit) {
    document.edit_style(ids, edit).unwrap();
}

fn state_of(document: &Document, ids: &[NodeId]) -> curvyo_ui_core::StylePanelState {
    let objects = objects(document);
    let scope = style_scope(
        StyleTool::Other,
        &objects,
        &selected(ids),
        &NodeSelection::new(),
    );
    style_panel_state(&objects, &scope)
}

fn percent(n: u32) -> Opacity {
    Opacity::new(f64::from(n) / 100.0).unwrap()
}

// ---- scope and subject line (criterion 37) --------------------------------

#[test]
fn pen_and_an_empty_selection_disable_the_panel() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let objects = objects(&document);

    let pen = style_scope(
        StyleTool::Pen,
        &objects,
        &selected(&[a]),
        &NodeSelection::new(),
    );
    assert_eq!(pen.ids, Vec::<NodeId>::new());
    assert_eq!(pen.subject, "Pen: finish the path to style it");

    let none = style_scope(
        StyleTool::Other,
        &objects,
        &ObjectSelection::new(),
        &NodeSelection::new(),
    );
    assert_eq!(none.ids, Vec::<NodeId>::new());
    assert_eq!(none.subject, "Nothing selected");
    let node_none = style_scope(
        StyleTool::Node,
        &objects,
        &ObjectSelection::new(),
        &NodeSelection::new(),
    );
    assert_eq!(node_none.subject, "Nothing selected");
}

#[test]
fn the_subject_line_names_the_kind_or_the_count() {
    let document = Document::new(1);
    let (r1, r2, r3) = (
        square(&document, 0.0),
        square(&document, 20.0),
        square(&document, 40.0),
    );
    let e = circle(&document, 60.0);
    let p = line(&document, 1, 30.0);
    let objects = objects(&document);
    let subject = |ids: &[NodeId]| {
        style_scope(
            StyleTool::Other,
            &objects,
            &selected(ids),
            &NodeSelection::new(),
        )
        .subject
    };
    assert_eq!(subject(&[r1]), "Rectangle");
    assert_eq!(subject(&[e]), "Ellipse");
    assert_eq!(subject(&[p]), "Path");
    assert_eq!(subject(&[r1, r2, r3]), "3 rectangles");
    assert_eq!(subject(&[r1, e]), "2 objects");
    assert_eq!(subject(&[r1, e, p, r2]), "4 objects");
}

#[test]
fn a_creation_tool_with_a_selection_edits_it() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let objects = objects(&document);
    let scope = style_scope(
        StyleTool::Other,
        &objects,
        &selected(&[a]),
        &NodeSelection::new(),
    );
    assert_eq!(scope.ids, vec![a]);
}

#[test]
fn the_node_tool_edits_the_paths_that_own_the_selected_nodes() {
    let document = Document::new(1);
    let p1 = line(&document, 1, 0.0);
    let p2 = line(&document, 3, 20.0);
    let p3 = line(&document, 5, 40.0);
    let objects = objects(&document);
    let anchors = |id| document.path(id).unwrap().anchors;
    let mut nodes = NodeSelection::new();
    nodes.select_single_node(p1, anchors(p1)[0].id);
    nodes.toggle_node(p3, anchors(p3)[1].id);
    // The object selection holds a different path; the nodes win.
    let scope = style_scope(StyleTool::Node, &objects, &selected(&[p2]), &nodes);
    assert_eq!(scope.ids, vec![p1, p3]);
    assert_eq!(scope.subject, "2 paths");
}

#[test]
fn the_node_tool_with_no_node_selected_edits_the_object_selection() {
    let document = Document::new(1);
    let p1 = line(&document, 1, 0.0);
    let objects = objects(&document);
    let scope = style_scope(
        StyleTool::Node,
        &objects,
        &selected(&[p1]),
        &NodeSelection::new(),
    );
    assert_eq!(scope.ids, vec![p1]);
    assert_eq!(scope.subject, "Path");
}

// ---- what the panel shows (criteria 5, 13, 24) -----------------------------

#[test]
fn a_disabled_panel_shows_the_frozen_defaults() {
    let document = Document::new(1);
    let state = state_of(&document, &[]);
    assert!(!state.enabled);
    assert_eq!(state.stroke.paint, BarValue::Uniform(true));
    assert_eq!(state.stroke.width, BarValue::Uniform(Length::from_mm(0.25)));
    assert_eq!(state.stroke.color, BarValue::Uniform(Color::BLACK));
    assert_eq!(state.stroke.dash, BarValue::Uniform(DashChoice::Solid));
    assert_eq!(state.stroke.join, BarValue::Uniform(LineJoin::Miter));
    assert_eq!(state.stroke.cap, BarValue::Uniform(LineCap::Butt));
    assert_eq!(state.fill.mode, BarValue::Uniform(FillMode::None));
}

#[test]
fn equal_objects_show_one_value_and_different_ones_show_mixed() {
    let document = Document::new(1);
    let (a, b) = (square(&document, 0.0), circle(&document, 30.0));
    let state = state_of(&document, &[a, b]);
    assert!(state.enabled);
    assert_eq!(state.stroke.width, BarValue::Uniform(Length::from_mm(0.25)));

    edit(
        &document,
        &[a],
        &StyleEdit::StrokeWidth(Length::from_mm(2.0)),
    );
    edit(&document, &[a], &StyleEdit::StrokeCap(LineCap::Round));
    let state = state_of(&document, &[a, b]);
    assert_eq!(state.stroke.width, BarValue::Mixed);
    assert_eq!(state.stroke.cap, BarValue::Mixed);
    // Properties that still agree stay shared: "never first selected wins".
    assert_eq!(state.stroke.join, BarValue::Uniform(LineJoin::Miter));
    assert_eq!(state.stroke.color, BarValue::Uniform(Color::BLACK));
}

#[test]
fn stroke_rows_are_off_only_when_every_object_has_its_stroke_off() {
    let document = Document::new(1);
    let (a, b) = (square(&document, 0.0), square(&document, 20.0));
    edit(&document, &[a], &StyleEdit::StrokeEnabled(false));
    let some_off = state_of(&document, &[a, b]);
    assert_eq!(some_off.stroke.paint, BarValue::Mixed);
    assert!(!some_off.stroke.all_off);

    edit(&document, &[b], &StyleEdit::StrokeEnabled(false));
    let all_off = state_of(&document, &[a, b]);
    assert_eq!(all_off.stroke.paint, BarValue::Uniform(false));
    assert!(all_off.stroke.all_off);
    // The stored values stay on show while the stroke is off (criterion 5).
    assert_eq!(
        all_off.stroke.width,
        BarValue::Uniform(Length::from_mm(0.25))
    );
}

#[test]
fn the_dash_choice_names_a_preset_or_custom() {
    let document = Document::new(1);
    let (a, b) = (square(&document, 0.0), square(&document, 20.0));
    for (choice, lengths) in [
        (DashChoice::Dash, vec![6.0, 4.0]),
        (DashChoice::Dot, vec![1.0, 3.0]),
        (DashChoice::DashDot, vec![6.0, 3.0, 1.0, 3.0]),
        (DashChoice::Custom, vec![2.0, 2.0]),
        (DashChoice::Solid, vec![]),
    ] {
        let pattern = DashPattern::new(lengths).unwrap();
        edit(&document, &[a], &StyleEdit::StrokeDash(pattern.clone()));
        let state = state_of(&document, &[a]);
        assert_eq!(state.stroke.dash, BarValue::Uniform(choice));
        assert_eq!(choice.pattern().is_none(), choice == DashChoice::Custom);
        if let Some(preset) = choice.pattern() {
            assert_eq!(preset, pattern);
        }
    }
    edit(
        &document,
        &[a],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    );
    assert_eq!(state_of(&document, &[a, b]).stroke.dash, BarValue::Mixed);
    assert_eq!(DashChoice::from_name("dash-dot"), Some(DashChoice::DashDot));
    assert_eq!(DashChoice::from_name("custom"), None);
}

#[test]
fn the_fill_mode_row_reads_none_and_solid_and_mixed() {
    let document = Document::new(1);
    let (a, b) = (square(&document, 0.0), square(&document, 20.0));
    assert_eq!(
        state_of(&document, &[a, b]).fill.mode,
        BarValue::Uniform(FillMode::None)
    );
    document
        .set_fill_mode(
            FillMode::Solid,
            &[FillModeTarget {
                id: a,
                seed_stops: vec![],
            }],
        )
        .unwrap();
    let state = state_of(&document, &[a, b]);
    assert_eq!(state.fill.mode, BarValue::Mixed);
    assert_eq!(
        state_of(&document, &[a]).fill.mode,
        BarValue::Uniform(FillMode::Solid)
    );
}

// ---- typed values (criteria 5, 6, 14) --------------------------------------

#[test]
fn hex_accepts_three_or_six_digits_in_any_case_with_or_without_hash() {
    let orange = Color {
        r: 0xFF,
        g: 0x88,
        b: 0x00,
    };
    for text in ["#F80", "f80", "#ff8800", "FF8800", "  #Ff8800  "] {
        assert_eq!(parse_hex(text), Ok(orange), "{text}");
    }
    assert!(parse_hex("").is_err());
    assert!(parse_hex("#12").is_err());
    assert!(parse_hex("#12345").is_err());
    assert!(parse_hex("#GGGGGG").is_err());
    // Colour and opacity are independent: eight digits are refused, with their
    // own message.
    let eight = parse_hex("#FF8800CC").unwrap_err();
    assert_ne!(eight, parse_hex("#12").unwrap_err());
    assert_eq!(eight.code(), "hex8");
    assert_eq!(parse_hex("#12").unwrap_err().code(), "hex");
}

#[test]
fn opacity_is_an_integer_percent_stored_as_n_over_100() {
    assert_eq!(parse_opacity_percent("50"), Ok(percent(50)));
    assert_eq!(parse_opacity_percent(" 50 % "), Ok(percent(50)));
    assert_eq!(parse_opacity_percent("0"), Ok(percent(0)));
    assert_eq!(parse_opacity_percent("100"), Ok(percent(100)));
    // A typed decimal rounds to the nearest integer.
    assert_eq!(parse_opacity_percent("33.4"), Ok(percent(33)));
    assert_eq!(parse_opacity_percent("33,6"), Ok(percent(34)));
    // Every N reads back as N.
    for n in 0..=100 {
        let stored = parse_opacity_percent(&n.to_string()).unwrap();
        assert!((stored.get() * 100.0 - f64::from(n)).abs() < 1e-9);
    }
    for bad in ["", "abc", "-1", "101", "100.6", "1e2"] {
        assert!(parse_opacity_percent(bad).is_err(), "{bad}");
    }
}

#[test]
fn width_is_zero_to_a_thousand_millimetres_with_either_decimal_mark() {
    assert_eq!(parse_stroke_width("0.5"), Ok(Length::from_mm(0.5)));
    assert_eq!(parse_stroke_width("0,125"), Ok(Length::from_mm(0.125)));
    assert_eq!(parse_stroke_width("0"), Ok(Length::from_mm(0.0)));
    assert_eq!(parse_stroke_width("1000"), Ok(Length::from_mm(1000.0)));
    for bad in ["", "abc", "-0.1", "1000.5", "1,2,3"] {
        assert!(parse_stroke_width(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_field_turns_typed_text_into_the_edit_it_commits() {
    assert_eq!(
        StyleField::from_name("stroke-width")
            .unwrap()
            .parse_text("2")
            .unwrap(),
        StyleEdit::StrokeWidth(Length::from_mm(2.0))
    );
    assert_eq!(
        StyleField::from_name("fill-color")
            .unwrap()
            .parse_text("#000")
            .unwrap(),
        StyleEdit::FillColor(Color::BLACK)
    );
    assert_eq!(
        StyleField::from_name("stroke-opacity")
            .unwrap()
            .parse_text("25")
            .unwrap(),
        StyleEdit::StrokeOpacity(percent(25))
    );
    assert_eq!(
        StyleField::from_name("fill-opacity")
            .unwrap()
            .parse_text("25")
            .unwrap(),
        StyleEdit::FillOpacity(percent(25))
    );
    assert!(StyleField::from_name("nonsense").is_none());
}

// ---- preview and the single commit (criterion 36) ----------------------------

#[test]
fn a_preview_changes_what_is_drawn_and_nothing_stored() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::FillOpacity(percent(40)));
    editor.preview(&[a], StyleEdit::FillOpacity(percent(30)));

    let mut shown = objects(&document);
    editor.apply_to(&mut shown);
    let ObjectSnapshot::Primitive(p) = &shown[0] else {
        panic!("a primitive");
    };
    assert_eq!(p.style.fill.opacity, percent(30));
    let stored = document.primitive(a).unwrap();
    assert_eq!(stored.style.fill.opacity, Opacity::OPAQUE, "not stored");
}

#[test]
fn the_release_writes_one_commit_to_the_objects_the_edit_started_on() {
    let document = Document::new(1);
    let (a, b) = (square(&document, 0.0), square(&document, 20.0));
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::StrokeOpacity(percent(60)));
    // A later preview call names other objects (the selection moved on): the
    // edit stays with the objects it started on.
    editor.preview(&[b], StyleEdit::StrokeOpacity(percent(20)));
    editor.commit(&document).unwrap();

    assert_eq!(
        document.primitive(a).unwrap().style.stroke.opacity,
        percent(20)
    );
    assert_eq!(
        document.primitive(b).unwrap().style.stroke.opacity,
        Opacity::OPAQUE
    );
    assert!(!editor.is_active());
    // Nothing pending: a second release writes nothing.
    editor.commit(&document).unwrap();
}

#[test]
fn escape_drops_the_preview_and_the_release_then_writes_nothing() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::StrokeOpacity(percent(60)));
    editor.cancel();
    assert!(!editor.is_active());
    // The pointer is still down: further moves must not bring the preview back.
    editor.preview(&[a], StyleEdit::StrokeOpacity(percent(50)));
    assert!(!editor.is_active());
    let mut shown = objects(&document);
    let before = shown.clone();
    editor.apply_to(&mut shown);
    assert_eq!(shown, before);
    editor.commit(&document).unwrap();
    assert_eq!(
        document.primitive(a).unwrap().style.stroke.opacity,
        Opacity::OPAQUE
    );
    // The release ended the gesture: the next drag previews again.
    editor.preview(&[a], StyleEdit::StrokeOpacity(percent(50)));
    assert!(editor.is_active());
}

#[test]
fn a_cancel_with_no_drag_running_does_not_block_the_next_one() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.cancel();
    editor.preview(&[a], StyleEdit::FillOpacity(percent(10)));
    assert!(editor.is_active());
}

#[test]
fn the_host_words_parse_back() {
    use curvyo_ui_core::{cap_from_name, fill_mode_from_name, join_from_name};
    assert_eq!(join_from_name("bevel"), Some(LineJoin::Bevel));
    assert_eq!(cap_from_name("square"), Some(LineCap::Square));
    assert_eq!(fill_mode_from_name("solid"), Some(FillMode::Solid));
    assert_eq!(join_from_name("mixed"), None);
    assert_eq!(fill_mode_from_name("custom"), None);
}
