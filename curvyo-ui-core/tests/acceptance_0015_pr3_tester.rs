//! Independent tester tests for PR 3 (panel) of
//! `specs/0015-document-size-and-rulers`, `ui-core` side: what a typed size
//! parses to (criteria 15, 16, 35), the validation and too-large messages
//! (16, 27a), the field and status formats (21, 35), the panel's content for
//! every selection and Pen state (14a) and the view following a resize or fit
//! (20). Expected values come from the specification's arithmetic, never from
//! the implementation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]

use curvyo_document_core::{
    DisplayUnit, Document, DocumentSize, Length, Point, RectBounds, Vec2, ViewTransform,
};
use curvyo_ui_core::{
    ObjectSelection, PX_PER_MM_AT_100, PanelContent, Viewport, content_too_large_message,
    document_side_message, format_cursor, format_field_length, format_size, panel_content,
    parse_document_side,
};
use proptest::prelude::*;

const UNITS: [DisplayUnit; 3] = [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::In];

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn parsed_mm(text: &str, unit: DisplayUnit) -> Option<f64> {
    parse_document_side(text, unit).map(Length::as_mm)
}

// ---- criterion 15: what typing means ----

#[test]
fn ac15_decimal_point_comma_and_surrounding_spaces() {
    for (text, want) in [
        ("300", 300.0),
        ("  300  ", 300.0),
        ("300.5", 300.5),
        ("300,5", 300.5),
        (" 300,5 ", 300.5),
        ("000000300", 300.0),
        ("1", 1.0),
        ("100000", 100_000.0),
    ] {
        let got = parsed_mm(text, DisplayUnit::Mm);
        assert_eq!(got, Some(want), "{text:?}");
    }
}

#[test]
fn ac15_text_that_is_not_one_number_is_refused() {
    for text in [
        "", "   ", "abc", "21cm", "21 cm", "21mm", "8.5in", "inf", "-inf", "+inf", "NaN", "nan",
        "Infinity", "1e999", "1,5,3", "1.5.2", "1.000,5", "1 000", "--5", "+", ".", ",", "0x10",
        "300;", "\u{221e}",
    ] {
        for unit in UNITS {
            assert_eq!(parsed_mm(text, unit), None, "{text:?} in {unit:?}");
        }
    }
}

// ---- criterion 16: limits ----

#[test]
fn ac16_limits_in_mm() {
    assert_eq!(parsed_mm("1", DisplayUnit::Mm), Some(1.0));
    assert_eq!(parsed_mm("100000", DisplayUnit::Mm), Some(100_000.0));
    for text in [
        "0",
        "0.99",
        "0.99999999",
        "100001",
        "100000.00000001",
        "-5",
        "-0",
    ] {
        assert_eq!(parsed_mm(text, DisplayUnit::Mm), None, "{text:?}");
    }
}

#[test]
fn ac16_a_value_within_1e_minus_9_of_a_limit_is_accepted_and_clamped() {
    assert_eq!(parsed_mm("0.9999999999", DisplayUnit::Mm), Some(1.0));
    assert_eq!(
        parsed_mm("100000.0000000001", DisplayUnit::Mm),
        Some(100_000.0)
    );
}

#[test]
fn ac16_limits_in_cm() {
    assert_eq!(parsed_mm("0.1", DisplayUnit::Cm), Some(1.0));
    assert_eq!(parsed_mm("10000", DisplayUnit::Cm), Some(100_000.0));
    assert_eq!(parsed_mm("21", DisplayUnit::Cm), Some(210.0));
    assert_eq!(parsed_mm("29,7", DisplayUnit::Cm), Some(297.0));
    for text in ["0.09", "0.0999", "10000.01", "10001", "0"] {
        assert_eq!(parsed_mm(text, DisplayUnit::Cm), None, "{text:?}");
    }
}

#[test]
fn ac16_limits_in_inches() {
    let low = parsed_mm("0.04", DisplayUnit::In).unwrap();
    assert!((low - 1.016).abs() < 1e-12, "{low}");
    let high = parsed_mm("3937", DisplayUnit::In).unwrap();
    assert!((high - 99_999.8).abs() < 1e-9, "{high}");
    // 0.03937 in = 0.999998 mm: below the 1 mm limit by far more than 1e-9.
    assert_eq!(parsed_mm("0.03937", DisplayUnit::In), None);
    assert_eq!(parsed_mm("0.03", DisplayUnit::In), None);
    // 3937.01 in = 100000.254 mm.
    assert_eq!(parsed_mm("3937.01", DisplayUnit::In), None);
    assert_eq!(parsed_mm("4000", DisplayUnit::In), None);
}

/// As-built note: the text a field shows for the largest size ("3937.0079")
/// is accepted when typed back and clamped onto the limit.
#[test]
fn ac16_the_shown_text_of_the_largest_size_types_back_as_the_limit() {
    let shown = format_field_length(mm(100_000.0), DisplayUnit::In);
    assert_eq!(shown, "3937.0079");
    assert_eq!(parsed_mm(&shown, DisplayUnit::In), Some(100_000.0));
    let shown = format_field_length(mm(1.0), DisplayUnit::In);
    assert_eq!(shown, "0.0394");
    // Observed: the smallest size's shown text also types back as the limit
    // itself (0.0394 in is 1.00076 mm), so retyping shown text never moves
    // the stored value (criterion 35).
    assert_eq!(parsed_mm(&shown, DisplayUnit::In), Some(1.0));
}

#[test]
fn ac16_messages_name_the_limits_in_the_display_unit_rounded_inward() {
    assert_eq!(
        document_side_message(DisplayUnit::Mm),
        "Enter a number from 1 to 100000"
    );
    assert_eq!(
        document_side_message(DisplayUnit::Cm),
        "Enter a number from 0.1 to 10000"
    );
    assert_eq!(
        document_side_message(DisplayUnit::In),
        "Enter a number from 0.04 to 3937"
    );
}

/// A message never promises a value the field refuses: both limits quoted in
/// the message parse, in every unit.
#[test]
fn ac16_the_limits_quoted_in_the_messages_are_accepted_by_the_parser() {
    for unit in UNITS {
        let message = document_side_message(unit);
        let numbers: Vec<&str> = message
            .split_whitespace()
            .filter(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .collect();
        assert_eq!(numbers.len(), 2, "{message}");
        for n in numbers {
            assert!(parsed_mm(n, unit).is_some(), "{n} in {unit:?}");
        }
    }
}

#[test]
fn ac27a_too_large_message_names_the_limit_in_the_display_unit() {
    assert_eq!(
        content_too_large_message(DisplayUnit::Mm),
        "The content is larger than the largest document (100000 mm). Nothing was changed."
    );
    let cm = content_too_large_message(DisplayUnit::Cm);
    assert!(
        cm.contains("10000 cm") && cm.contains("Nothing was changed"),
        "{cm}"
    );
    let inch = content_too_large_message(DisplayUnit::In);
    assert!(
        inch.contains("3937 in") && inch.contains("Nothing was changed"),
        "{inch}"
    );
}

// ---- criterion 35: inches ----

#[test]
fn ac35_eight_and_a_half_by_eleven_inches_is_exactly_letter_in_mm() {
    let w = parse_document_side("8.5", DisplayUnit::In).unwrap();
    let h = parse_document_side("11", DisplayUnit::In).unwrap();
    assert_eq!(w.as_mm(), 215.9);
    assert_eq!(h.as_mm(), 279.4);
    assert_eq!(parsed_mm("1", DisplayUnit::In), Some(25.4));
    assert_eq!(parsed_mm("8,5", DisplayUnit::In), Some(215.9));
}

#[test]
fn ac35_field_text_has_3_or_4_decimals_and_no_trailing_zeros() {
    for (v, unit, want) in [
        (210.0, DisplayUnit::Mm, "210"),
        (297.0, DisplayUnit::Mm, "297"),
        (215.9, DisplayUnit::Mm, "215.9"),
        (1.0 / 3.0, DisplayUnit::Mm, "0.333"),
        (123.4564, DisplayUnit::Mm, "123.456"),
        (100_000.0, DisplayUnit::Mm, "100000"),
        (210.0, DisplayUnit::Cm, "21"),
        (297.0, DisplayUnit::Cm, "29.7"),
        (1.0, DisplayUnit::Cm, "0.1"),
        (215.9, DisplayUnit::In, "8.5"),
        (279.4, DisplayUnit::In, "11"),
        (25.4, DisplayUnit::In, "1"),
        (210.0, DisplayUnit::In, "8.2677"),
        (297.0, DisplayUnit::In, "11.6929"),
    ] {
        assert_eq!(format_field_length(mm(v), unit), want, "{v} mm in {unit:?}");
    }
}

#[test]
fn ac35_field_text_never_shows_negative_zero_or_an_exponent() {
    for v in [0.0, -0.0, 1e-12, 12_345.678_9, 99_999.999_9] {
        for unit in UNITS {
            let t = format_field_length(mm(v), unit);
            assert!(!t.contains('e') && !t.contains("-0"), "{v} {unit:?}: {t}");
            assert!(!t.ends_with('.'), "{t}");
            if t.contains('.') {
                assert!(!t.ends_with('0'), "trailing zero: {t}");
            }
        }
    }
}

// ---- criterion 21 / 34: status formats ----

#[test]
fn ac21_size_and_cursor_formats_per_unit() {
    let a4 = DocumentSize::from_mm(210.0, 297.0);
    assert_eq!(format_size(a4, DisplayUnit::Mm), "210.0 \u{d7} 297.0 mm");
    assert_eq!(format_size(a4, DisplayUnit::Cm), "21.00 \u{d7} 29.70 cm");
    assert_eq!(format_size(a4, DisplayUnit::In), "8.268 \u{d7} 11.693 in");
    assert_eq!(
        format_cursor(mm(12.3), mm(45.6), DisplayUnit::Mm),
        "x: 12.3  y: 45.6 mm"
    );
    assert_eq!(
        format_cursor(mm(25.4), mm(50.8), DisplayUnit::In),
        "x: 1.000  y: 2.000 in"
    );
    assert_eq!(
        format_cursor(mm(120.0), mm(45.0), DisplayUnit::Cm),
        "x: 12.00  y: 4.50 cm"
    );
}

proptest! {
    /// Whatever is typed, the result is either refused or inside the limits.
    #[test]
    fn typed_text_is_refused_or_inside_the_limits(text in "\\PC{0,24}", unit in 0usize..3) {
        if let Some(l) = parse_document_side(&text, UNITS[unit]) {
            let v = l.as_mm();
            prop_assert!(v.is_finite() && (1.0..=100_000.0).contains(&v), "{text:?} -> {v}");
        }
    }

    /// Numbers typed in the usual way parse to the same value in every unit
    /// (value times mm-per-unit, within 1e-9 mm) or are refused.
    #[test]
    fn typed_numbers_parse_per_unit(v in 0.0f64..200_000.0, unit in 0usize..3) {
        let unit = UNITS[unit];
        let per = match unit { DisplayUnit::Mm => 1.0, DisplayUnit::Cm => 10.0, DisplayUnit::In => 25.4 };
        let text = format!("{v}");
        let want = v * per;
        match parse_document_side(&text, unit) {
            Some(l) => {
                prop_assert!((l.as_mm() - want).abs() < 1e-6, "{text} {unit:?} {} vs {want}", l.as_mm());
            }
            None => prop_assert!(!(1.0 + 1e-9..=100_000.0 - 1e-9).contains(&want), "{text} {unit:?}"),
        }
    }

    /// A comma and a point mean the same.
    #[test]
    fn comma_equals_point(i in 1u32..99_999, f in 0u32..1000) {
        let a = parse_document_side(&format!("{i}.{f}"), DisplayUnit::Mm);
        let b = parse_document_side(&format!("{i},{f}"), DisplayUnit::Mm);
        prop_assert_eq!(a.map(Length::as_mm), b.map(Length::as_mm));
    }
}

// ---- criterion 14a: panel content for every state ----

fn selection_of(n: usize) -> ObjectSelection {
    let document = Document::new(1);
    let ids: Vec<_> = (0..n)
        .map(|i| {
            document.create_rect(RectBounds {
                origin: Point::new(i as f64 * 10.0, 0.0),
                width: mm(5.0),
                height: mm(5.0),
            })
        })
        .collect();
    let mut selection = ObjectSelection::new();
    selection.set(&ids);
    selection
}

#[test]
fn ac14a_panel_content_for_every_selection_and_pen_state() {
    for (selected, pen, want) in [
        (0, false, PanelContent::Document),
        (0, true, PanelContent::Empty),
        (1, false, PanelContent::Style),
        (1, true, PanelContent::Style),
        (2, false, PanelContent::Style),
        (2, true, PanelContent::Style),
        (50, false, PanelContent::Style),
    ] {
        assert_eq!(
            panel_content(&selection_of(selected), pen),
            want,
            "{selected} selected, pen unfinished: {pen}"
        );
    }
}

#[test]
fn ac14a_clearing_the_selection_reveals_the_document_section() {
    let mut selection = selection_of(3);
    assert_eq!(panel_content(&selection, false), PanelContent::Style);
    selection.clear();
    assert_eq!(panel_content(&selection, false), PanelContent::Document);
}

// ---- criterion 20: the view follows a resize or a fit ----

fn screen(view: ViewTransform, x: f64, y: f64) -> (f64, f64) {
    view.document_to_screen(Point::new(x, y))
}

fn viewport_at(percent_steps: i32, with_inset: bool) -> Viewport {
    let mut v = if with_inset {
        Viewport::with_document_inset()
    } else {
        Viewport::new()
    };
    v.resize(1000.0, 700.0);
    if percent_steps != 0 {
        v.zoom_about(300.0, 200.0, 1.25_f64.powi(percent_steps));
        v.pan_by_screen_delta(17.5, -33.25);
    }
    v
}

/// An object at document point `p` before, at `p + offset` after the
/// command, has the same screen position: for every zoom, with and without
/// the inset flag, for any offset.
#[test]
fn ac20_pan_by_document_offset_keeps_moved_points_in_place() {
    let offsets = [
        Vec2::new(0.0, 0.0),
        Vec2::new(45.0, 0.0),
        Vec2::new(45.0, 51.5),
        Vec2::new(-45.0, -51.5),
        Vec2::new(-0.000_001, 1e-9),
        Vec2::new(99_999.0, -99_999.0),
        Vec2::new(12_345.678, 0.5),
    ];
    for steps in [-20, -8, -3, 0, 3, 8, 20] {
        for with_inset in [true, false] {
            for off in offsets {
                let mut v = viewport_at(steps, with_inset);
                let p = Point::new(10.0, 10.0);
                let (bx, by) = screen(v.view(), p.x, p.y);
                v.pan_by_document_offset(off);
                let (ax, ay) = screen(v.view(), p.x + off.x, p.y + off.y);
                let tol = 1e-6;
                assert!(
                    (bx - ax).abs() < tol && (by - ay).abs() < tol,
                    "zoom steps {steps} inset {with_inset} off {off:?}: ({bx}, {by}) vs ({ax}, {ay})"
                );
            }
        }
    }
}

#[test]
fn ac20_zero_offset_changes_nothing_including_the_inset() {
    let mut v = Viewport::with_document_inset();
    v.resize(900.0, 600.0);
    let before = screen(v.view(), 0.0, 0.0);
    v.pan_by_document_offset(Vec2::new(0.0, 0.0));
    assert_eq!(screen(v.view(), 0.0, 0.0), before);
    // 128 px is not exactly representable through the scale, as 72 px happened to be.
    let near = |(x, y): (f64, f64)| (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9;
    assert!(near(before), "{before:?}");
    // Still the untouched view: a window resize keeps the corner at 128 px.
    v.resize(500.0, 450.0);
    assert!(near(screen(v.view(), 0.0, 0.0)));
}

/// The command is not a navigation gesture: a window resize of a view that
/// only followed a resize keeps what was on screen where it was (as-built
/// note, criterion 20).
#[test]
fn ac20_a_window_resize_after_the_command_keeps_the_objects_in_place() {
    let mut v = Viewport::with_document_inset();
    v.resize(1000.0, 700.0);
    let off = Vec2::new(45.0, 51.5);
    let before = screen(v.view(), 10.0, 10.0);
    v.pan_by_document_offset(off);
    v.resize(640.0, 480.0);
    v.resize(1300.0, 900.0);
    let after = screen(v.view(), 10.0 + off.x, 10.0 + off.y);
    assert!(
        (before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6,
        "{before:?} vs {after:?}"
    );
}

#[test]
fn ac20_zoom_is_unchanged_by_the_command() {
    let mut v = viewport_at(5, false);
    let zoom = v.zoom_percent();
    v.pan_by_document_offset(Vec2::new(500.0, -500.0));
    assert_eq!(v.zoom_percent(), zoom);
    let expected_scale = f64::from(i32::try_from(zoom).unwrap()) / 100.0 * PX_PER_MM_AT_100;
    assert!((v.view().scale() - expected_scale).abs() / expected_scale < 0.01);
}

/// Two commands in a row equal one with the summed offset.
#[test]
fn ac20_offsets_add_up() {
    let mut a = viewport_at(4, true);
    let mut b = viewport_at(4, true);
    a.pan_by_document_offset(Vec2::new(10.0, 20.0));
    a.pan_by_document_offset(Vec2::new(-3.0, 7.5));
    b.pan_by_document_offset(Vec2::new(7.0, 27.5));
    let (ax, ay) = screen(a.view(), 0.0, 0.0);
    let (bx, by) = screen(b.view(), 0.0, 0.0);
    assert!((ax - bx).abs() < 1e-9 && (ay - by).abs() < 1e-9);
}
