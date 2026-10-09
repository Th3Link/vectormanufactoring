//! Independent tester checks of `specs/0007-stroke-and-fill-styling` PR 3
//! (Style panel), written from the specification before the implementation was
//! read: typed-value parsing (criteria 6, 14, 4, 5), the panel's scope per tool
//! (37) and the preview/commit gesture (36).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use curvyo_document_core::{
    AnchorId, Color, Document, Length, NewAnchor, NodeId, Opacity, Point, RectBounds, StyleEdit,
};
use curvyo_ui_core::{
    BarValue, DashChoice, NodeSelection, ObjectSelection, StyleEditor, StyleEntryError, StyleField,
    StyleTool, parse_hex, parse_opacity_percent, parse_stroke_width, style_panel_state,
    style_scope,
};
use proptest::prelude::*;

fn square(document: &Document, x: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

/// An id that no object of the test's own document carries.
fn foreign_id() -> NodeId {
    square(&Document::new(2), 500.0)
}

fn open_path(document: &Document, n: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(7, n), Point::new(0.0, 20.0)),
            NewAnchor::corner(AnchorId::new(7, n + 1), Point::new(10.0, 20.0)),
        ],
        false,
    )
}

fn objects(document: &Document) -> Vec<curvyo_document_core::ObjectSnapshot> {
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

/// The colour a typed hex gives, whatever alpha it carried.
fn hex_rgb(text: &str) -> Result<Color, StyleEntryError> {
    parse_hex(text).map(|typed| typed.color)
}

fn style_of(document: &Document, id: NodeId) -> curvyo_document_core::Style {
    match document.object(id).unwrap() {
        curvyo_document_core::ObjectSnapshot::Path(p) => p.style,
        curvyo_document_core::ObjectSnapshot::Primitive(p) => p.style,
    }
}

// ---------------------------------------------------------------------
// Hex parsing (criteria 6 and 14)
// ---------------------------------------------------------------------

#[test]
fn hex_short_and_long_forms_agree_for_every_case_and_prefix() {
    let want = Color {
        r: 0xFF,
        g: 0x88,
        b: 0x00,
    };
    for text in [
        "#F80", "f80", "F80", "#f80", "#FF8800", "ff8800", "#ff8800", "FF8800", "#Ff8800",
    ] {
        assert_eq!(hex_rgb(text), Ok(want), "{text:?}");
    }
    assert_eq!(hex_rgb("#000"), Ok(Color::BLACK));
    assert_eq!(
        hex_rgb("fff"),
        Ok(Color {
            r: 255,
            g: 255,
            b: 255
        })
    );
}

#[test]
fn hex_refuses_every_wrong_length_with_the_hex_error() {
    for text in [
        "",
        "#",
        "1",
        "12",
        "#12",
        "12345",
        "#12345",
        "1234567",
        "#1234567",
        "123456789",
        "#123456789",
    ] {
        assert_eq!(parse_hex(text), Err(StyleEntryError::Hex), "{text:?}");
    }
}

#[test]
fn hex_takes_four_and_eight_digits_as_rgb_and_alpha() {
    for text in [
        "#12345678",
        "12345678",
        "#FF880080",
        "ff880080",
        "#F808",
        "f808",
    ] {
        let typed = parse_hex(text).unwrap_or_else(|_| panic!("{text:?}"));
        assert!(typed.opacity.is_some(), "{text:?} carries an alpha");
    }
}

#[test]
fn hex_refuses_non_hex_characters() {
    for text in [
        "#GGG", "#12G", "xyzxyz", "#gg8800", "0xFF8800", "##F80", "# F80", "F 80", "#F,0", "#F.0",
        "#-F8", "-F8800", "+F8800", "#+F8", "+f8", "#+f8800",
    ] {
        assert_eq!(parse_hex(text), Err(StyleEntryError::Hex), "{text:?}");
    }
}

#[test]
fn hex_does_not_panic_on_multibyte_input_of_matching_byte_length() {
    // 3 and 6 BYTES but fewer characters, and 3 / 6 characters with more
    // bytes: slicing by byte index would panic or mis-parse.
    for text in [
        "\u{e9}\u{e9}\u{e9}",
        "#\u{e9}\u{e9}\u{e9}",
        "\u{e9}1234",
        "#\u{e9}1234",
        "\u{e9}\u{e9}",
        "#\u{e9}\u{e9}\u{e9}\u{e9}",
        "\u{ff26}\u{ff26}\u{18}",
        "\u{ff26}\u{ff26}\u{ff10}\u{ff10}\u{ff10}\u{ff10}",
        "\u{1f600}ab",
        "#\u{1f600}",
        "a\u{1f600}",
    ] {
        assert_eq!(
            parse_hex(text),
            Err(StyleEntryError::Hex),
            "{text:?} must be refused"
        );
    }
}

proptest! {
    #[test]
    fn hex_never_panics_and_round_trips_valid_input(text in "\\PC{0,12}") {
        {
            if let Ok(typed) = parse_hex(&text) {
                let color = typed.color;
                // Anything accepted must be 3, 4, 6 or 8 hex digits after an optional #
                // (surrounding spaces are ignored).
                let trimmed = text.trim();
                let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
                prop_assert!([3, 4, 6, 8].contains(&digits.len()), "{text:?}");
                prop_assert!(digits.chars().all(|c| c.is_ascii_hexdigit()), "{text:?}");
                let long = if digits.len() <= 4 {
                    digits.chars().flat_map(|c| [c, c]).collect::<String>()
                } else {
                    digits.to_string()
                };
                let n = u32::from_str_radix(&long[..6], 16).unwrap();
                prop_assert_eq!(
                    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b),
                    n
                );
            }
        }
    }

    #[test]
    fn six_digit_hex_round_trips_every_colour(rgb in 0u32..=0xFF_FFFF, upper in any::<bool>(), hash in any::<bool>()) {
        let mut text = format!("{rgb:06x}");
        if upper { text = text.to_uppercase(); }
        if hash { text.insert(0, '#'); }
        let c = parse_hex(&text).unwrap().color;
        prop_assert_eq!(
            (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b),
            rgb
        );
    }
}

// ---------------------------------------------------------------------
// Opacity percent (criteria 6 and 14)
// ---------------------------------------------------------------------

proptest! {
    #[test]
    fn a_whole_percent_is_stored_as_exactly_n_over_100_and_reads_back(n in 0u32..=100) {
        let parsed = parse_opacity_percent(&n.to_string()).unwrap();
        prop_assert_eq!(parsed.get().to_bits(), (f64::from(n) / 100.0).to_bits());
        prop_assert_eq!((parsed.get() * 100.0).round() as u32, n);
        let with_sign = parse_opacity_percent(&format!("{n}%")).unwrap();
        prop_assert_eq!(with_sign, parsed);
        let preview = curvyo_ui_core::opacity_from_percent(f64::from(n));
        prop_assert_eq!(preview, parsed, "slider and typed value store the same number");
    }
}

#[test]
fn a_typed_decimal_is_rounded_to_the_nearest_whole_percent() {
    let pct = |t: &str| parse_opacity_percent(t).unwrap();
    assert_eq!(pct("49.4"), Opacity::new(0.49).unwrap());
    assert_eq!(pct("49.6"), Opacity::new(0.50).unwrap());
    assert_eq!(pct("0.4"), Opacity::new(0.0).unwrap());
    assert_eq!(pct("99.6"), Opacity::new(1.0).unwrap());
    assert_eq!(pct("100.4"), Opacity::new(1.0).unwrap());
}

#[test]
fn opacity_text_outside_the_range_or_not_a_number_is_refused() {
    for text in [
        "", " ", "abc", "101", "100.6", "-1", "-0.6", "1000", "NaN", "nan", "inf", "-inf",
        "Infinity", "1e999", "%", "5 0", "5%%",
    ] {
        assert_eq!(
            parse_opacity_percent(text),
            Err(StyleEntryError::Percent),
            "{text:?}"
        );
    }
}

#[test]
fn opacity_from_percent_clamps_and_survives_non_finite_values() {
    for p in [
        150.0,
        -5.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
    ] {
        let o = curvyo_ui_core::opacity_from_percent(p);
        assert!(
            o.get().is_finite() && (0.0..=1.0).contains(&o.get()),
            "{p}: {o:?}"
        );
    }
    assert_eq!(curvyo_ui_core::opacity_from_percent(150.0), Opacity::OPAQUE);
    assert_eq!(
        curvyo_ui_core::opacity_from_percent(-5.0).get(),
        0.0,
        "negative clamps to 0"
    );
}

// ---------------------------------------------------------------------
// Stroke width (criteria 4 and 5)
// ---------------------------------------------------------------------

#[test]
fn width_accepts_zero_to_a_thousand_with_either_decimal_mark() {
    for (text, mm) in [
        ("0", 0.0),
        ("0.0", 0.0),
        ("0,0", 0.0),
        ("0.25", 0.25),
        ("0,25", 0.25),
        ("1000", 1000.0),
        ("1000.0", 1000.0),
        ("999.999", 999.999),
        ("0.123456789", 0.123_456_789),
        (".5", 0.5),
    ] {
        assert_eq!(
            parse_stroke_width(text),
            Ok(Length::from_mm(mm)),
            "{text:?} (stored without rounding)"
        );
    }
}

#[test]
fn width_refuses_negative_too_large_and_malformed_text() {
    for text in [
        "",
        " ",
        "-1",
        "-0.001",
        "1000.001",
        "1001",
        "1e9999",
        "NaN",
        "nan",
        "inf",
        "-inf",
        "Infinity",
        "abc",
        "1.2.3",
        "1,2,3",
        "1,2.3",
        "1..2",
        ",",
        ".",
        "1 2",
        "--1",
        "1mm x",
        "99999999999999999999999999999999",
    ] {
        assert_eq!(
            parse_stroke_width(text),
            Err(StyleEntryError::Width),
            "{text:?}"
        );
    }
}

proptest! {
    #[test]
    fn width_text_never_panics_and_accepts_only_finite_values_in_range(text in "\\PC{0,14}") {
        if let Ok(len) = parse_stroke_width(&text) {
            let mm = len.as_mm();
            prop_assert!(mm.is_finite() && (0.0..=1000.0).contains(&mm), "{text:?} -> {mm}");
        }
    }
}

#[test]
fn every_style_field_refuses_the_other_fields_kind_of_value() {
    assert!(StyleField::StrokeColor.parse_text("5").is_err());
    assert!(StyleField::FillColor.parse_text("10000").is_err());
    assert!(StyleField::StrokeWidth.parse_text("#F80").is_err());
    assert!(StyleField::StrokeOpacity.parse_text("#F80").is_err());
    assert!(StyleField::FillOpacity.parse_text("1001").is_err());
    assert_eq!(StyleField::from_name("stroke-dash"), None);
    assert_eq!(StyleField::from_name(""), None);
    assert_eq!(StyleField::from_name("Stroke-Width"), None);
    assert!(StyleField::StrokeWidth.color_edit(Color::BLACK).is_none());
    assert!(StyleField::StrokeColor.opacity_edit(50.0).is_none());
}

// ---------------------------------------------------------------------
// Scope per tool (criterion 37)
// ---------------------------------------------------------------------

#[test]
fn ids_the_document_no_longer_holds_are_dropped_from_the_scope() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let b = square(&document, 30.0);
    let objs = objects(&document);
    let gone = foreign_id();
    let scope = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[a, gone, b]),
        &NodeSelection::new(),
    );
    assert_eq!(scope.ids.len(), 2, "{scope:?}");
    assert_eq!(scope.subject, "2 rectangles");

    let only_gone = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[gone]),
        &NodeSelection::new(),
    );
    assert_eq!(only_gone.ids, []);
    assert_eq!(
        style_panel_state(&objs, &objs, &only_gone),
        None,
        "a stale-only selection leaves nothing to edit"
    );
}

#[test]
fn a_mixed_kind_selection_is_counted_as_objects() {
    let document = Document::new(1);
    let r = square(&document, 0.0);
    let p = open_path(&document, 1);
    let objs = objects(&document);
    let scope = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[r, p]),
        &NodeSelection::new(),
    );
    assert_eq!(scope.subject, "2 objects");
    assert_eq!(scope.ids.len(), 2);
}

#[test]
fn the_pen_ignores_a_selection_and_the_node_tool_with_nothing_is_disabled() {
    let document = Document::new(1);
    let r = square(&document, 0.0);
    let objs = objects(&document);
    let pen = style_scope(
        StyleTool::Pen,
        &objs,
        &selected(&[r]),
        &NodeSelection::new(),
    );
    assert_eq!(pen.ids, []);
    assert_eq!(pen.subject, "", "no text for the Pen (criterion 4)");

    let node_none = style_scope(
        StyleTool::Node,
        &objs,
        &ObjectSelection::new(),
        &NodeSelection::new(),
    );
    assert_eq!(node_none.ids, []);
    assert_eq!(node_none.subject, "", "no text when nothing is selected");
    assert_eq!(style_panel_state(&objs, &objs, &node_none), None);

    // Node tool, only a rectangle in the object selection: it is not a path,
    // so the panel has nothing to edit (the Node tool edits paths only).
    let node_rect = style_scope(
        StyleTool::Node,
        &objs,
        &selected(&[r]),
        &NodeSelection::new(),
    );
    assert_eq!(node_rect.ids, [], "{node_rect:?}");
}

#[test]
fn an_empty_scope_has_no_panel_state() {
    let document = Document::new(1);
    let _ = square(&document, 0.0);
    let objs = objects(&document);
    let scope = style_scope(
        StyleTool::Other,
        &objs,
        &ObjectSelection::new(),
        &NodeSelection::new(),
    );
    assert_eq!(scope.subject, "");
    assert_eq!(style_panel_state(&objs, &objs, &scope), None);
}

// ---------------------------------------------------------------------
// Mixed state and stroke-off interplay (criteria 5 and 24)
// ---------------------------------------------------------------------

#[test]
fn mixed_values_show_mixed_only_for_the_property_that_differs() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let b = square(&document, 30.0);
    document
        .edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(3.0)))
        .unwrap();
    let objs = objects(&document);
    let scope = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[a, b]),
        &NodeSelection::new(),
    );
    let state = style_panel_state(&objs, &objs, &scope).unwrap();
    assert_eq!(state.stroke.width, BarValue::Mixed);
    assert_eq!(state.stroke.color, BarValue::Uniform(Color::BLACK));
    assert_eq!(state.stroke.paint, BarValue::Uniform(true));
    assert_eq!(state.subject, "2 rectangles");
}

#[test]
fn some_on_and_some_off_shows_the_rows_and_paint_is_mixed() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let b = square(&document, 30.0);
    document
        .edit_style(&[a], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let objs = objects(&document);
    let both = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[a, b]),
        &NodeSelection::new(),
    );
    let state = style_panel_state(&objs, &objs, &both).unwrap();
    assert_eq!(state.stroke.paint, BarValue::Mixed);
    assert!(state.stroke.rows_shown, "rows stay shown when mixed");

    let off_only = style_scope(
        StyleTool::Other,
        &objs,
        &selected(&[a]),
        &NodeSelection::new(),
    );
    let state = style_panel_state(&objs, &objs, &off_only).unwrap();
    assert!(!state.stroke.rows_shown);
    assert_eq!(state.stroke.paint, BarValue::Uniform(false));
    // Colour and Width stay readable while the stroke is off.
    assert_eq!(state.stroke.width, BarValue::Uniform(Length::from_mm(0.25)));
}

#[test]
fn width_zero_switches_off_and_keeps_the_stored_width_and_every_other_value() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let red = Color { r: 255, g: 0, b: 0 };
    for e in [
        StyleEdit::StrokeWidth(Length::from_mm(4.0)),
        StyleEdit::StrokeColor(red),
        StyleEdit::StrokeDash(DashChoice::Dot.pattern()),
    ] {
        document.edit_style(&[a], &e).unwrap();
    }
    document
        .edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(0.0)))
        .unwrap();
    let s = style_of(&document, a);
    assert!(!s.stroke.enabled, "criterion 5: width 0 is no stroke");
    assert_eq!(s.stroke.width, Length::from_mm(4.0), "width kept");
    assert_eq!(s.stroke.color, red);
    assert_eq!(s.stroke.dash.as_slice(), &[1.0, 3.0]);

    // A non-zero typed width turns it back on with the stored values intact.
    document
        .edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(2.0)))
        .unwrap();
    let s = style_of(&document, a);
    assert!(s.stroke.enabled);
    assert_eq!(s.stroke.width, Length::from_mm(2.0));
    assert_eq!(s.stroke.color, red);
    assert_eq!(s.stroke.dash.as_slice(), &[1.0, 3.0]);
}

#[test]
fn colour_or_opacity_edit_on_an_off_stroke_turns_it_on_and_discrete_edits_do_not() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    for edit in [
        StyleEdit::StrokeColor(Color { r: 1, g: 2, b: 3 }),
        StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap()),
    ] {
        document
            .edit_style(&[a], &StyleEdit::StrokeEnabled(false))
            .unwrap();
        document.edit_style(&[a], &edit).unwrap();
        assert!(style_of(&document, a).stroke.enabled, "{edit:?}");
    }
    // Dash, join and cap are disabled while off, so they never need to turn
    // the stroke on; a stray call must not either.
    document
        .edit_style(&[a], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    document
        .edit_style(
            &[a],
            &StyleEdit::StrokeJoin(curvyo_document_core::LineJoin::Round),
        )
        .unwrap();
    assert!(!style_of(&document, a).stroke.enabled);
}

#[test]
fn stroke_width_edit_refuses_non_finite_and_negative_widths_without_writing() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let b = square(&document, 30.0);
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        let result = document.edit_style(&[a, b], &StyleEdit::StrokeWidth(Length::from_mm(bad)));
        assert!(result.is_err(), "{bad}");
    }
    assert_eq!(
        style_of(&document, a),
        curvyo_document_core::Style::default()
    );
    assert_eq!(
        style_of(&document, b),
        curvyo_document_core::Style::default()
    );
}

#[test]
fn an_edit_naming_a_deleted_object_does_not_corrupt_the_others() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let gone = foreign_id();
    let _ = document.edit_style(
        &[a, gone],
        &StyleEdit::StrokeColor(Color { r: 9, g: 9, b: 9 }),
    );
    // Either the whole edit was refused or `a` got it; never a half state of
    // `a` (colour set but stroke flag stale).
    let s = style_of(&document, a);
    assert!(
        s == curvyo_document_core::Style::default()
            || (s.stroke.color == Color { r: 9, g: 9, b: 9 } && s.stroke.enabled)
    );
}

// ---------------------------------------------------------------------
// Preview / commit gesture (criterion 36)
// ---------------------------------------------------------------------

#[test]
fn a_gesture_on_one_object_is_not_applied_to_another() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let b = square(&document, 30.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::StrokeOpacity(Opacity::new(0.4).unwrap()));
    // A later tick naming other objects (the selection changed mid-drag) must
    // not retarget the gesture.
    editor.preview(&[b], StyleEdit::StrokeOpacity(Opacity::new(0.6).unwrap()));
    let mut objs = objects(&document);
    editor.apply_to(&mut objs);
    let opacity = |o: &curvyo_document_core::ObjectSnapshot| match o {
        curvyo_document_core::ObjectSnapshot::Primitive(p) => p.style.stroke.opacity,
        curvyo_document_core::ObjectSnapshot::Path(p) => p.style.stroke.opacity,
    };
    let ids = document.object_ids();
    let by_id = |id: NodeId| {
        objs.iter()
            .zip(&ids)
            .find(|(_, i)| **i == id)
            .map(|(o, _)| opacity(o))
            .unwrap()
    };
    assert_eq!(by_id(a), Opacity::new(0.6).unwrap(), "latest value on a");
    assert_eq!(by_id(b), Opacity::OPAQUE, "b is never touched");
    editor.commit(&document).unwrap();
    assert_eq!(
        style_of(&document, a).stroke.opacity,
        Opacity::new(0.6).unwrap()
    );
    assert_eq!(style_of(&document, b).stroke.opacity, Opacity::OPAQUE);
    assert!(!editor.is_active());
}

#[test]
fn a_second_commit_and_a_commit_without_preview_write_nothing() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    assert!(editor.commit(&document).is_ok());
    editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(5.0)));
    editor.commit(&document).unwrap();
    // The user then changes the width elsewhere; a stray second release must
    // not re-apply the old gesture.
    document
        .edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(7.0)))
        .unwrap();
    editor.commit(&document).unwrap();
    assert_eq!(style_of(&document, a).stroke.width, Length::from_mm(7.0));
}

#[test]
fn cancel_then_many_ticks_then_release_writes_nothing_and_the_next_gesture_works() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(5.0)));
    editor.cancel();
    assert!(!editor.is_active());
    for w in [6.0, 7.0, 8.0] {
        editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(w)));
        assert!(!editor.is_active(), "ticks after Escape are ignored");
    }
    editor.commit(&document).unwrap();
    assert_eq!(
        style_of(&document, a),
        curvyo_document_core::Style::default()
    );

    // The release ended the cancelled gesture: the next one is live.
    editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(2.0)));
    assert!(editor.is_active());
    editor.commit(&document).unwrap();
    assert_eq!(style_of(&document, a).stroke.width, Length::from_mm(2.0));
}

#[test]
fn a_commit_to_a_deleted_object_ends_the_gesture_and_does_not_poison_the_next() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(
        &[foreign_id()],
        StyleEdit::StrokeWidth(Length::from_mm(5.0)),
    );
    let _ = editor.commit(&document);
    assert!(!editor.is_active(), "the preview is dropped either way");
    editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(3.0)));
    editor.commit(&document).unwrap();
    assert_eq!(style_of(&document, a).stroke.width, Length::from_mm(3.0));
}

#[test]
fn a_preview_with_an_empty_id_list_changes_nothing_and_commits_nothing() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[], StyleEdit::StrokeWidth(Length::from_mm(5.0)));
    let mut objs = objects(&document);
    let before = objs.clone();
    editor.apply_to(&mut objs);
    assert_eq!(objs, before);
    let _ = editor.commit(&document);
    assert_eq!(
        style_of(&document, a),
        curvyo_document_core::Style::default()
    );
}

#[test]
fn a_width_preview_of_zero_is_drawn_as_no_stroke_and_only_the_commit_stores_it() {
    let document = Document::new(1);
    let a = square(&document, 0.0);
    let mut editor = StyleEditor::default();
    editor.preview(&[a], StyleEdit::StrokeWidth(Length::from_mm(0.0)));
    let mut objs = objects(&document);
    editor.apply_to(&mut objs);
    match &objs[0] {
        curvyo_document_core::ObjectSnapshot::Primitive(p) => {
            assert!(!p.style.stroke.enabled, "preview matches the commit rule");
            assert_eq!(p.style.stroke.width, Length::from_mm(0.25));
        }
        curvyo_document_core::ObjectSnapshot::Path(_) => panic!("rectangle expected"),
    }
    assert!(style_of(&document, a).stroke.enabled, "nothing stored yet");
}
