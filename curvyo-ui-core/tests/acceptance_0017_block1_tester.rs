//! Independent tester cases for `0017-style-panel-rework`, block 1 (`ui-core`
//! side): the 8-digit hex field, the picker's colour model, the dash text line,
//! the value-field scales, the eyedropper's pick and the panel state. Written
//! from `specification.md` and `adrs.md` before the implementation was read.
//!
//! Criteria covered here: 5 to 10, 11 to 15, 17 to 21 (model side), 24 and 25,
//! 28 to 33, 36 to 38 (arithmetic), 42, 43, 44 to 47, 61 (defaults).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Color, DashPattern, Document, EllipseFrame, Length, NewAnchor, NodeId,
    ObjectSnapshot, Opacity, Point, RectBounds, Style, StyleEdit, Tolerance, Vec2,
};
use curvyo_ui_core::{
    BarValue, DashChoice, DashShown, Grid, MAX_DASH_NUMBER, MAX_DASH_NUMBERS, PaintTarget,
    StyleEntryError, StyleField, StyleScope, ValueField, ValueScale, dash_text, hex_text,
    hsv_to_rgb, opacity_from_percent, parse_dash_text, parse_hex, parse_opacity_percent,
    parse_stroke_width, pick_colour, rgb_to_hsv, style_panel_state,
};
use proptest::prelude::*;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn op(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (tol {tol})");
}

// ============================================ AC 11 to 14: 8-digit hex

#[test]
fn hex_shows_eight_upper_case_digits_with_alpha_rounded_half_up() {
    assert_eq!(hex_text(rgb(0x2F, 0x6F, 0xEE), op(1.0)), "#2F6FEEFF");
    assert_eq!(hex_text(rgb(0x2F, 0x6F, 0xEE), op(0.5)), "#2F6FEE80");
    assert_eq!(hex_text(rgb(0, 0, 0), op(0.0)), "#00000000");
    assert_eq!(hex_text(rgb(255, 255, 255), op(1.0)), "#FFFFFFFF");
    assert_eq!(hex_text(rgb(0xab, 0xcd, 0xef), op(1.0)), "#ABCDEFFF");
}

#[test]
fn every_alpha_byte_survives_the_trip_through_its_stored_fraction() {
    for aa in 0u32..=255 {
        let text = hex_text(rgb(1, 2, 3), op(f64::from(aa) / 255.0));
        assert_eq!(text, format!("#010203{aa:02X}"), "AA = {aa}");
        // Typing the 8-digit form back stores AA / 255 exactly.
        let parsed = parse_hex(&text).unwrap();
        assert_eq!(parsed.opacity.unwrap().get(), f64::from(aa) / 255.0);
    }
}

#[test]
fn every_whole_percent_shows_as_round_half_up_of_255_n_over_100() {
    for n in 0u32..=100 {
        // round(255 x n / 100), .5 rounded up, in exact integer arithmetic.
        let expected = (255 * n + 50) / 100;
        let stored = opacity_from_percent(f64::from(n));
        assert_eq!(stored.get(), f64::from(n) / 100.0, "N/100 exactly for {n}");
        assert_eq!(
            hex_text(rgb(0, 0, 0), stored),
            format!("#000000{expected:02X}"),
            "N = {n}"
        );
    }
}

#[test]
fn hex_accepts_the_four_lengths_with_or_without_hash_in_any_case_with_spaces() {
    for text in [
        "F80", "f80", "#F80", "#f80", "  #F80  ", "ff8800", "FF8800", "#Ff8800", " ff8800 ",
    ] {
        let hex = parse_hex(text).unwrap_or_else(|_| panic!("{text:?}"));
        assert_eq!(hex.color, rgb(0xFF, 0x88, 0x00), "{text:?}");
        assert!(
            hex.opacity.is_none(),
            "{text:?}: 3 and 6 digits keep the alpha"
        );
    }
    for text in [
        "F80C",
        "f80c",
        "#F80C",
        " #f80c ",
        "FF8800CC",
        "ff8800cc",
        "#FF8800cc",
    ] {
        let hex = parse_hex(text).unwrap_or_else(|_| panic!("{text:?}"));
        assert_eq!(hex.color, rgb(0xFF, 0x88, 0x00), "{text:?}");
        assert_eq!(
            hex.opacity.unwrap().get(),
            f64::from(0xCCu8) / 255.0,
            "{text:?}"
        );
    }
}

#[test]
fn hex_refuses_every_other_length_and_any_non_hex_character() {
    for text in [
        "",
        " ",
        "#",
        "##",
        "F",
        "F8",
        "F8000",
        "FF880",
        "FF88000",
        "FF8800C",
        "FF8800CCD",
        "FF8800CCDD",
        "GGG",
        "#GG0000",
        "FF88ZZ",
        "0xFF8800",
        "FF 88 00",
        "# FF8800",
        "FF8800 CC",
        "ÄÄÄ",
        "ÄÄÄÄÄÄ",
        "１２３",
        "FF-800",
        "FF8800;",
        "rgb(1,2,3)",
        "#FF8800CC\u{0}",
        "FF\n8800",
        "-F80",
        "+F80",
        "F80.",
        "𝟏𝟐𝟑",
    ] {
        assert_eq!(
            parse_hex(text).err(),
            Some(StyleEntryError::Hex),
            "{text:?}"
        );
    }
    // A very long input is refused, not parsed.
    assert!(parse_hex(&"F".repeat(100_000)).is_err());
}

#[test]
fn typing_8_digits_then_percent_then_reading_back_matches_the_spec_example() {
    // 2F6FEE80 -> alpha 128/255; the Opacity field shows 50; typing 50 stores
    // 0.5 and the hex still reads 80.
    let typed = parse_hex("2F6FEE80").unwrap();
    let alpha = typed.opacity.unwrap();
    assert_eq!(alpha.get(), 128.0 / 255.0);
    assert_eq!((alpha.get() * 100.0).round(), 50.0);
    let fifty = parse_opacity_percent("50").unwrap();
    assert_eq!(fifty.get(), 0.5);
    assert_eq!(hex_text(typed.color, fifty), "#2F6FEE80");
    assert_ne!(
        alpha.get(),
        fifty.get(),
        "the two grids differ and both are kept"
    );
}

#[test]
fn typed_opacity_percent_is_stored_exactly_and_decimals_round() {
    for (text, want) in [
        ("0", 0.0),
        ("100", 1.0),
        ("33", 0.33),
        ("7", 0.07),
        ("50%", 0.5),
        (" 50 % ", 0.5),
        ("49.5", 0.5),
        ("49.4", 0.49),
        ("0.4", 0.0),
        ("99,6", 1.0),
        ("100.4", 1.0),
    ] {
        assert_eq!(parse_opacity_percent(text).unwrap().get(), want, "{text:?}");
    }
    for text in [
        "", "abc", "-1", "100.5", "101", "-0.6", "1e2", "NaN", "inf", "5 5",
    ] {
        assert_eq!(
            parse_opacity_percent(text).err(),
            Some(StyleEntryError::Percent),
            "{text:?}"
        );
    }
}

#[test]
fn typed_width_accepts_point_and_comma_up_to_1000_and_zero() {
    for (text, want) in [
        ("0", 0.0),
        ("0.25", 0.25),
        ("0,25", 0.25),
        ("1000", 1000.0),
        (" 2 ", 2.0),
    ] {
        assert_eq!(parse_stroke_width(text).unwrap().as_mm(), want, "{text:?}");
    }
    for text in ["", "abc", "-0.1", "1000.01", "1001", "NaN", "inf", "1,2,3"] {
        assert_eq!(
            parse_stroke_width(text).err(),
            Some(StyleEntryError::Width),
            "{text:?}"
        );
    }
}

/// Criterion 42: "a trailing unit (`mm`, `%`) and surrounding spaces are
/// ignored". The opacity field does this for `%`; the width field must do it for
/// `mm`, whatever spelling the maker types.
#[test]
fn typed_width_ignores_a_trailing_mm_unit_like_opacity_ignores_percent() {
    for text in ["2mm", "2 mm", " 2mm ", "0.25mm", "0,25 mm", "1000mm", "0mm"] {
        let want: f64 = text
            .trim()
            .trim_end_matches("mm")
            .trim()
            .replace(',', ".")
            .parse()
            .unwrap();
        assert_eq!(
            parse_stroke_width(text).map(Length::as_mm),
            Ok(want),
            "{text:?}"
        );
    }
}

#[test]
fn the_hex_field_edit_kind_follows_the_digit_count() {
    // 3 / 6 digits touch the colour only; 4 / 8 digits write colour and alpha.
    assert!(matches!(
        StyleField::StrokeColor.parse_text("F80").unwrap(),
        StyleEdit::StrokeColor(_)
    ));
    assert!(matches!(
        StyleField::StrokeColor.parse_text("FF8800").unwrap(),
        StyleEdit::StrokeColor(_)
    ));
    assert!(matches!(
        StyleField::StrokeColor.parse_text("F80C").unwrap(),
        StyleEdit::StrokeRgba(_, _)
    ));
    assert!(matches!(
        StyleField::StrokeColor.parse_text("FF8800CC").unwrap(),
        StyleEdit::StrokeRgba(_, _)
    ));
    assert!(matches!(
        StyleField::FillColor.parse_text("F80").unwrap(),
        StyleEdit::FillColor(_)
    ));
    assert!(matches!(
        StyleField::FillColor.parse_text("F80C").unwrap(),
        StyleEdit::FillRgba(_, _)
    ));
    assert!(StyleField::FillColor.parse_text("F8").is_err());
}

proptest! {
    #[test]
    fn parse_hex_never_panics_on_any_text(text in ".{0,40}") {
        let _ = parse_hex(&text);
    }

    #[test]
    fn a_six_digit_hex_round_trips_through_the_eight_digit_text(r: u8, g: u8, b: u8, aa: u8) {
        let text = format!("#{r:02x}{g:02x}{b:02x}{aa:02x}");
        let parsed = parse_hex(&text).unwrap();
        prop_assert_eq!(parsed.color, rgb(r, g, b));
        let shown = hex_text(parsed.color, parsed.opacity.unwrap());
        prop_assert_eq!(shown, text.to_uppercase());
    }
}

// ============================================ AC 17 to 21: colour model

#[test]
fn rgb_hsv_rgb_is_the_identity_for_every_sampled_colour() {
    let mut checked = 0u32;
    let mut values: Vec<u16> = (0..=255).step_by(3).collect();
    values.extend([1, 2, 253, 254, 255]);
    for &r in &values {
        for &g in &values {
            for &b in &values {
                let c = rgb(r as u8, g as u8, b as u8);
                let hsv = rgb_to_hsv(c);
                let back = hsv_to_rgb(hsv.hue.unwrap_or(0.0), hsv.saturation, hsv.value);
                assert_eq!(back, c, "{c:?} -> {hsv:?}");
                checked += 1;
            }
        }
    }
    assert!(checked > 600_000);
}

#[test]
fn all_greys_have_no_hue_and_black_and_white_too() {
    for v in 0..=255u8 {
        let hsv = rgb_to_hsv(rgb(v, v, v));
        assert_eq!(hsv.hue, None, "grey {v}");
        assert_eq!(hsv.saturation, 0.0);
        near(hsv.value, f64::from(v) / 255.0, 1e-12);
    }
    // Any hue with zero saturation is the grey of the value (the thumb can keep
    // its hue while the colour passes through grey).
    for hue in [0.0, 17.0, 120.0, 359.9, 720.0, -90.0] {
        assert_eq!(hsv_to_rgb(hue, 0.0, 0.5), rgb(128, 128, 128));
        assert_eq!(hsv_to_rgb(hue, 1.0, 0.0), rgb(0, 0, 0));
    }
}

#[test]
fn primary_colours_and_hue_wrap() {
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), rgb(255, 0, 0));
    assert_eq!(hsv_to_rgb(60.0, 1.0, 1.0), rgb(255, 255, 0));
    assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), rgb(0, 255, 0));
    assert_eq!(hsv_to_rgb(180.0, 1.0, 1.0), rgb(0, 255, 255));
    assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), rgb(0, 0, 255));
    assert_eq!(hsv_to_rgb(300.0, 1.0, 1.0), rgb(255, 0, 255));
    assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), rgb(255, 0, 0));
    assert_eq!(hsv_to_rgb(-120.0, 1.0, 1.0), rgb(0, 0, 255));
    assert_eq!(hsv_to_rgb(480.0, 1.0, 1.0), rgb(0, 255, 0));
    assert_eq!(rgb_to_hsv(rgb(255, 0, 0)).hue, Some(0.0));
    near(rgb_to_hsv(rgb(0, 255, 0)).hue.unwrap(), 120.0, 1e-9);
    near(rgb_to_hsv(rgb(0, 0, 255)).hue.unwrap(), 240.0, 1e-9);
    let almost_red = rgb_to_hsv(rgb(255, 0, 1)).hue.unwrap();
    assert!((0.0..360.0).contains(&almost_red), "{almost_red}");
    // A very dark colour still has a hue.
    assert_eq!(rgb_to_hsv(rgb(1, 0, 0)).hue, Some(0.0));
}

#[test]
fn hsv_inputs_outside_the_range_or_not_finite_never_panic_and_stay_in_range() {
    for h in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300] {
        for s in [f64::NAN, -5.0, 5.0, f64::INFINITY] {
            for v in [f64::NAN, -5.0, 5.0, f64::NEG_INFINITY] {
                let _ = hsv_to_rgb(h, s, v);
            }
        }
    }
    assert_eq!(hsv_to_rgb(0.0, 7.0, 7.0), rgb(255, 0, 0), "clamped to 1");
    assert_eq!(
        hsv_to_rgb(0.0, -3.0, 0.5),
        rgb(128, 128, 128),
        "clamped to 0"
    );
}

proptest! {
    #[test]
    fn hsv_to_rgb_is_continuous_in_hue(h in 0.0f64..360.0, s in 0.0f64..=1.0, v in 0.0f64..=1.0) {
        let a = hsv_to_rgb(h, s, v);
        let b = hsv_to_rgb(h + 0.01, s, v);
        let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).abs();
        prop_assert!(d(a.r, b.r) <= 2 && d(a.g, b.g) <= 2 && d(a.b, b.b) <= 2, "{a:?} {b:?}");
    }
}

// ============================================ AC 28 to 33: dash line

#[test]
fn dash_text_valid_forms_from_the_spec() {
    let list = |t: &str| parse_dash_text(t).unwrap().as_slice().to_vec();
    assert_eq!(list("1 2 4 2"), vec![1.0, 2.0, 4.0, 2.0]);
    assert_eq!(list("1   2 4  2"), vec![1.0, 2.0, 4.0, 2.0]);
    assert_eq!(list("  6 4  "), vec![6.0, 4.0]);
    assert_eq!(list("1 2 4"), vec![1.0, 2.0, 4.0], "odd lists are kept odd");
    assert_eq!(list("7"), vec![7.0]);
    assert_eq!(list("0 3"), vec![0.0, 3.0], "a zero on entry is accepted");
    assert_eq!(list("3 0"), vec![3.0, 0.0]);
    assert_eq!(list("0.5 0.25"), vec![0.5, 0.25]);
    assert_eq!(list("1000 1000"), vec![1000.0, 1000.0]);
    assert_eq!(list("0000.50 1"), vec![0.5, 1.0]);
    assert!(parse_dash_text("").unwrap().is_solid(), "empty is Solid");
    assert!(
        parse_dash_text("    ").unwrap().is_solid(),
        "spaces only is empty"
    );
    // The order typed is the order stored.
    assert_eq!(list("4 1 3 2"), vec![4.0, 1.0, 3.0, 2.0]);
}

#[test]
fn dash_text_one_to_sixteen_numbers() {
    let sixteen = vec!["1"; 16].join(" ");
    assert_eq!(parse_dash_text(&sixteen).unwrap().as_slice().len(), 16);
    let seventeen = vec!["1"; 17].join(" ");
    assert_eq!(
        parse_dash_text(&seventeen).err(),
        Some(StyleEntryError::Dash)
    );
    assert_eq!(MAX_DASH_NUMBERS, 16);
    assert_eq!(MAX_DASH_NUMBER, 1000.0);
    // A giant line is refused without taking forever.
    let giant = "1 ".repeat(1_000_000);
    assert!(parse_dash_text(&giant).is_err());
}

#[test]
fn dash_text_a_comma_is_never_a_separator_or_a_decimal_mark() {
    for text in [
        "1,2,4,2", "1, 2", "1,5", "1 ,2", "1 , 2", "1;2", ",", "1,", ",1", "6,4",
    ] {
        assert_eq!(
            parse_dash_text(text).err(),
            Some(StyleEntryError::Dash),
            "{text:?}"
        );
    }
}

#[test]
fn dash_text_refuses_everything_else() {
    for text in [
        "0",
        "0 0",
        "0 0 0",
        "-1 2",
        "1 -2",
        "-0 -0",
        "1001 1",
        "1000.000001 1",
        "1 1e3",
        "1e2",
        "abc",
        "1 a",
        "NaN",
        "nan 1",
        "inf",
        "Infinity 1",
        "-inf",
        "1..2",
        "1.2.3",
        ".",
        "-",
        "+",
        "+1 2",
        "1\t2",
        "1\n2",
        "1\u{a0}2",
        "１ ２",
        "٣ ٤",
        "1/2",
        "0x10 1",
        "99999999999999999999999999999999 1",
    ] {
        assert_eq!(
            parse_dash_text(text).err(),
            Some(StyleEntryError::Dash),
            "{text:?}"
        );
    }
    // Digits so many they overflow f64 to infinity, and so tiny they underflow
    // to zero (a zero sum).
    let huge = format!("1{} 1", "0".repeat(400));
    assert!(parse_dash_text(&huge).is_err());
    let tiny = format!("0.{}1 0", "0".repeat(400));
    assert!(parse_dash_text(&tiny).is_err(), "underflow to a zero sum");
}

#[test]
fn dash_text_shows_the_numbers_one_space_apart_in_the_shortest_form() {
    let text = |l: &[f64]| dash_text(&DashPattern::new(l.to_vec()).unwrap());
    assert_eq!(text(&[]), "");
    assert_eq!(text(&[6.0, 4.0]), "6 4");
    assert_eq!(text(&[1.0, 3.0]), "1 3");
    assert_eq!(text(&[6.0, 3.0, 1.0, 3.0]), "6 3 1 3");
    assert_eq!(text(&[0.5, 0.25]), "0.5 0.25");
    assert_eq!(text(&[0.0, 3.0]), "0 3");
    assert_eq!(text(&[1.0, 2.0, 4.0]), "1 2 4");
    assert_eq!(
        text(&(1..=17).map(f64::from).collect::<Vec<_>>())
            .split(' ')
            .count(),
        17
    );
}

proptest! {
    #[test]
    fn dash_text_round_trips_any_list_exactly(
        list in prop::collection::vec(prop_oneof![Just(0.0f64), 0.0f64..1000.0, (0u32..1000).prop_map(f64::from)], 1..=16)
    ) {
        prop_assume!(list.iter().sum::<f64>() > 0.0);
        let pattern = DashPattern::new(list.clone()).unwrap();
        let shown = dash_text(&pattern);
        let back = parse_dash_text(&shown).unwrap();
        prop_assert_eq!(back.as_slice(), list.as_slice());
    }

    #[test]
    fn parse_dash_text_never_panics(text in ".{0,60}") {
        let _ = parse_dash_text(&text);
    }
}

#[test]
fn the_four_presets_and_only_they_press_a_button() {
    assert_eq!(
        DashChoice::from_pattern(&DashPattern::solid()),
        Some(DashChoice::Solid)
    );
    let of = |l: &[f64]| DashChoice::from_pattern(&DashPattern::new(l.to_vec()).unwrap());
    assert_eq!(of(&[6.0, 4.0]), Some(DashChoice::Dash));
    assert_eq!(of(&[1.0, 3.0]), Some(DashChoice::Dot));
    assert_eq!(of(&[6.0, 3.0, 1.0, 3.0]), Some(DashChoice::DashDot));
    // Anything else leaves every button up: no "Custom".
    assert_eq!(of(&[4.0, 6.0]), None);
    assert_eq!(
        of(&[6.0, 4.0, 6.0, 4.0]),
        None,
        "the expanded form of Dash is a different list"
    );
    assert_eq!(of(&[6.0]), None);
    assert_eq!(of(&[6.0, 4.0, 0.0]), None);
    assert_eq!(of(&[6.0, 4.0, 1.0]), None);
    assert_eq!(of(&[1.0, 3.0, 1.0]), None);
    // The stored lists of the presets are those of 0007 criterion 8.
    assert!(DashChoice::Solid.pattern().is_solid());
    assert_eq!(DashChoice::Dash.pattern().as_slice(), &[6.0, 4.0]);
    assert_eq!(DashChoice::Dot.pattern().as_slice(), &[1.0, 3.0]);
    assert_eq!(
        DashChoice::DashDot.pattern().as_slice(),
        &[6.0, 3.0, 1.0, 3.0]
    );
    // The text of a preset parses back to it.
    for choice in [
        DashChoice::Solid,
        DashChoice::Dash,
        DashChoice::Dot,
        DashChoice::DashDot,
    ] {
        let text = dash_text(&choice.pattern());
        assert_eq!(
            DashChoice::from_pattern(&parse_dash_text(&text).unwrap()),
            Some(choice)
        );
    }
    assert_eq!(DashChoice::from_name("custom"), None);
}

// ============================================ AC 36 to 47: value scales

#[test]
fn width_check_values_of_criterion_46() {
    let w = ValueScale::StrokeWidth;
    assert_eq!(w.value_at(0.0), 0.0);
    near(w.value_at(0.1749), 0.25, 0.001);
    near(w.value_at(0.5), 1.82, 0.005);
    near(w.value_at(1.0), 20.0, 1e-9);
    assert_eq!(w.round(w.value_at(0.5), Grid::Normal), 1.82);
    near(w.position_of(0.25), 0.1749, 0.0005);
    assert_eq!(w.position_of(0.0), 0.0);
    near(w.position_of(20.0), 1.0, 1e-12);
}

#[test]
fn opacity_check_values_of_criterion_46() {
    let o = ValueScale::Opacity;
    assert_eq!(o.value_at(0.0), 0.0);
    assert_eq!(o.round(o.value_at(0.5), Grid::Normal), 33.0);
    assert_eq!(o.round(o.value_at(0.9), Grid::Normal), 83.0);
    near(o.value_at(1.0), 100.0, 1e-9);
    near(o.position_of(100.0), 1.0, 1e-12);
    near(o.position_of(33.0), 0.5, 0.005);
}

#[test]
fn the_scale_ends_and_defaults_match_the_spec() {
    assert_eq!(ValueScale::StrokeWidth.scale_end(), 20.0);
    assert_eq!(ValueScale::StrokeWidth.typed_max(), 1000.0);
    assert_eq!(ValueScale::StrokeWidth.default_value(), 0.25);
    assert_eq!(ValueScale::Opacity.scale_end(), 100.0);
    assert_eq!(ValueScale::Opacity.typed_max(), 100.0);
    assert_eq!(ValueScale::Opacity.default_value(), 100.0);
}

proptest! {
    #[test]
    fn both_scales_are_strictly_monotonic_and_bounded(a in 0.0f64..1.0, b in 0.0f64..1.0) {
        for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            prop_assume!(hi - lo > 1e-9);
            prop_assert!(scale.value_at(lo) < scale.value_at(hi));
            prop_assert!(scale.value_at(a) >= 0.0 && scale.value_at(a) <= scale.scale_end() + 1e-9);
        }
    }

    #[test]
    fn position_of_inverts_value_at(p in 0.0f64..=1.0) {
        for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
            let back = scale.position_of(scale.value_at(p));
            prop_assert!((back - p).abs() < 1e-9, "{scale:?} p={p} back={back}");
        }
    }

    #[test]
    fn rounding_is_idempotent_on_grid_and_never_negative(v in -10.0f64..1500.0) {
        for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
            for grid in [Grid::Normal, Grid::Coarse, Grid::Fine] {
                let r = scale.round(v, grid);
                prop_assert!(r >= 0.0);
                prop_assert_eq!(scale.round(r, grid), r, "{:?} {:?} {}", scale, grid, v);
            }
        }
    }

    #[test]
    fn a_step_always_moves_the_right_way_unless_it_hits_a_bound(v in 0.0f64..1000.0) {
        for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
            let v = v.min(scale.typed_max());
            for grid in [Grid::Normal, Grid::Coarse, Grid::Fine] {
                let up = scale.step(v, 1, grid);
                let down = scale.step(v, -1, grid);
                prop_assert!(up > v || v >= scale.typed_max() - 1e-9, "{scale:?} {grid:?} up {v} -> {up}");
                prop_assert!(down < v || v <= 0.0, "{scale:?} {grid:?} down {v} -> {down}");
                prop_assert!(up <= scale.typed_max() && down >= 0.0);
            }
        }
    }
}

#[test]
fn p_outside_the_range_or_not_a_number_is_clamped() {
    for scale in [ValueScale::StrokeWidth, ValueScale::Opacity] {
        assert_eq!(scale.value_at(-5.0), 0.0);
        assert_eq!(scale.value_at(f64::NEG_INFINITY), 0.0);
        assert_eq!(scale.value_at(f64::NAN), 0.0);
        near(scale.value_at(5.0), scale.scale_end(), 1e-9);
        assert_eq!(scale.position_of(-1.0), 0.0);
        assert_eq!(scale.position_of(f64::NAN), 0.0);
        assert_eq!(
            scale.position_of(1e9),
            1.0,
            "above the drag maximum is a full bar"
        );
    }
}

#[test]
fn the_rounding_grids_of_criterion_47() {
    let w = ValueScale::StrokeWidth;
    assert_eq!(w.round(0.254, Grid::Normal), 0.25);
    assert_eq!(w.round(0.256, Grid::Normal), 0.26);
    assert_eq!(w.round(0.004, Grid::Normal), 0.0, "rounds to zero is zero");
    assert_eq!(w.round(0.0049, Grid::Normal), 0.0);
    assert_eq!(w.round(0.249, Grid::Coarse), 0.2);
    assert_eq!(w.round(0.26, Grid::Coarse), 0.3);
    assert_eq!(w.round(0.04, Grid::Coarse), 0.0);
    assert_eq!(w.round(0.1254, Grid::Fine), 0.125);
    assert_eq!(w.round(0.0004, Grid::Fine), 0.0);
    assert_eq!(w.round(-1.0, Grid::Normal), 0.0);
    assert_eq!(w.round(f64::NAN, Grid::Normal), 0.0);
    let o = ValueScale::Opacity;
    assert_eq!(o.round(33.4, Grid::Normal), 33.0);
    assert_eq!(o.round(33.6, Grid::Normal), 34.0);
    assert_eq!(
        o.round(33.6, Grid::Fine),
        34.0,
        "opacity Ctrl is still whole percent"
    );
    assert_eq!(o.round(0.4, Grid::Normal), 0.0);
    assert_eq!(o.round(94.0, Grid::Coarse), 90.0);
    assert_eq!(o.round(96.0, Grid::Coarse), 100.0);
}

#[test]
fn the_left_end_of_the_scale_gives_exactly_zero_and_the_right_end_the_scale_end() {
    for (scale, grid) in [
        (ValueScale::StrokeWidth, Grid::Normal),
        (ValueScale::StrokeWidth, Grid::Fine),
        (ValueScale::StrokeWidth, Grid::Coarse),
        (ValueScale::Opacity, Grid::Normal),
    ] {
        assert_eq!(scale.round(scale.value_at(0.0), grid), 0.0);
        assert_eq!(scale.round(scale.value_at(1.0), grid), scale.scale_end());
    }
    // A drag that is just right of the left end still reaches 0 on the grid.
    assert_eq!(
        ValueScale::StrokeWidth.round(ValueScale::StrokeWidth.value_at(0.0005), Grid::Normal),
        0.0
    );
}

#[test]
fn arrow_key_steps_use_the_grids_of_criterion_43() {
    let w = ValueScale::StrokeWidth;
    near(w.step(0.25, 1, Grid::Normal), 0.26, 1e-12);
    near(w.step(0.25, -1, Grid::Normal), 0.24, 1e-12);
    near(w.step(0.25, 1, Grid::Fine), 0.251, 1e-12);
    near(w.step(0.25, -1, Grid::Fine), 0.249, 1e-12);
    near(w.step(1.0, 1, Grid::Coarse), 1.1, 1e-12);
    near(w.step(1.0, -1, Grid::Coarse), 0.9, 1e-12);
    assert_eq!(w.step(0.0, -1, Grid::Normal), 0.0);
    assert_eq!(w.step(0.005, -1, Grid::Normal), 0.0);
    assert_eq!(w.step(1000.0, 1, Grid::Coarse), 1000.0, "typed maximum");
    assert_eq!(w.step(999.95, 1, Grid::Coarse), 1000.0);
    near(w.step(25.0, 1, Grid::Normal), 25.01, 1e-9);
    let o = ValueScale::Opacity;
    assert_eq!(o.step(50.0, 1, Grid::Normal), 51.0);
    assert_eq!(o.step(50.0, 1, Grid::Coarse), 60.0);
    assert_eq!(o.step(50.0, 1, Grid::Fine), 51.0);
    assert_eq!(o.step(95.0, 1, Grid::Coarse), 100.0);
    assert_eq!(o.step(100.0, 1, Grid::Normal), 100.0);
    assert_eq!(o.step(0.0, -1, Grid::Coarse), 0.0);
    assert_eq!(o.step(5.0, -1, Grid::Coarse), 0.0);
    // Several steps at once.
    assert_eq!(o.step(50.0, 3, Grid::Normal), 53.0);
    assert_eq!(o.step(50.0, -100, Grid::Normal), 0.0);
    assert_eq!(o.step(50.0, i32::MAX, Grid::Normal).min(100.0), 100.0);
}

#[test]
fn the_field_text_is_up_to_three_decimals_without_trailing_zeros() {
    let w = ValueScale::StrokeWidth;
    assert_eq!(w.text(0.25), "0.25");
    assert_eq!(w.text(1.82), "1.82");
    assert_eq!(w.text(0.125), "0.125");
    assert_eq!(w.text(2.0), "2");
    assert_eq!(w.text(0.0), "0");
    assert_eq!(w.text(1000.0), "1000");
    assert_eq!(w.text(0.1 + 0.2), "0.3");
    assert_eq!(w.text(1.820_000_000_000_000_3), "1.82");
    assert_eq!(w.text(19.999_999_999_999_996), "20");
    assert_eq!(w.text(0.3 - 0.1), "0.2");
    assert_ne!(w.text(0.0004), "0", "a positive width is never shown as 0");
    let o = ValueScale::Opacity;
    assert_eq!(o.text(100.0), "100");
    assert_eq!(o.text(50.196_078), "50");
    assert_eq!(o.text(0.0), "0");
    assert_eq!(o.text(33.0), "33");
    assert_ne!(w.text(f64::NAN), "");
}

/// The host's drag rule (criteria 36 to 38), restated as the spec writes it,
/// to pin the numbers the spec quotes against the Rust scale.
fn drag(p0: f64, dx: f64, w: f64) -> f64 {
    (p0 + dx / w).clamp(0.0, 1.0)
}

#[test]
fn the_drag_examples_of_criteria_36_and_37_come_out_as_written() {
    let width = ValueScale::StrokeWidth;
    let p0 = width.position_of(0.25);
    let at = |dx: f64| width.round(width.value_at(drag(p0, dx, 244.0)), Grid::Normal);
    near(at(24.4), 0.51, 0.01);
    assert_eq!(at(0.0), 0.25, "the value does not jump to the pointer");
    assert_eq!(at(-1000.0), 0.0);
    assert_eq!(at(1000.0), 20.0);
    // Past the end and back: no re-basing, the value is the one at dx = 200.
    let beyond = at(300.0);
    assert_eq!(beyond, 20.0);
    let back = at(300.0 - 100.0);
    assert_eq!(back, at(200.0));
    near(back, 19.5, 0.3);
    assert!(back < 20.0 + 1e-12);
    let opacity = ValueScale::Opacity;
    let p0 = opacity.position_of(100.0);
    assert_eq!(
        opacity.round(opacity.value_at(drag(p0, -24.4, 244.0)), Grid::Normal),
        83.0
    );
    // A value above the drag maximum starts at p = 1 (criterion 45).
    assert_eq!(width.position_of(500.0), 1.0);
}

#[test]
fn value_field_names_edits_and_defaults() {
    assert_eq!(
        ValueField::from_name("stroke-width"),
        Some(ValueField::StrokeWidth)
    );
    assert_eq!(
        ValueField::from_name("stroke-opacity"),
        Some(ValueField::StrokeOpacity)
    );
    assert_eq!(
        ValueField::from_name("fill-opacity"),
        Some(ValueField::FillOpacity)
    );
    assert_eq!(ValueField::from_name("fill-width"), None);
    assert_eq!(ValueField::from_name(""), None);
    assert_eq!(Grid::from_name("normal"), Some(Grid::Normal));
    assert_eq!(Grid::from_name("coarse"), Some(Grid::Coarse));
    assert_eq!(Grid::from_name("fine"), Some(Grid::Fine));
    assert_eq!(Grid::from_name("Fine"), None);

    match ValueField::StrokeOpacity.edit(33.0) {
        StyleEdit::StrokeOpacity(o) => assert_eq!(o.get(), 0.33),
        other => panic!("{other:?}"),
    }
    match ValueField::FillOpacity.edit(50.0) {
        StyleEdit::FillOpacity(o) => assert_eq!(o.get(), 0.5),
        other => panic!("{other:?}"),
    }
    match ValueField::StrokeWidth.edit(0.0) {
        StyleEdit::StrokeWidth(w) => assert_eq!(w.as_mm(), 0.0),
        other => panic!("{other:?}"),
    }
    // Resets: Width 0.25 mm, Opacity 100 % for stroke and fill.
    match ValueField::StrokeWidth.reset_edit() {
        StyleEdit::StrokeWidth(w) => assert_eq!(w.as_mm(), 0.25),
        other => panic!("{other:?}"),
    }
    for field in [ValueField::StrokeOpacity, ValueField::FillOpacity] {
        match field.reset_edit() {
            StyleEdit::StrokeOpacity(o) | StyleEdit::FillOpacity(o) => assert_eq!(o.get(), 1.0),
            other => panic!("{other:?}"),
        }
    }
    let mut style = Style::default();
    style.stroke.opacity = op(128.0 / 255.0);
    near(ValueField::StrokeOpacity.value_in(&style), 50.196_078, 1e-5);
    assert_eq!(ValueField::StrokeWidth.value_in(&style), 0.25);
    assert_eq!(ValueField::FillOpacity.value_in(&style), 100.0);
}

// ============================================ AC 24, 25: the eyedropper pick

fn square(doc: &Document, x: f64, y: f64, size: f64) -> NodeId {
    doc.create_rect(RectBounds {
        origin: Point::new(x, y),
        width: Length::from_mm(size),
        height: Length::from_mm(size),
    })
}

fn paint(doc: &Document, id: NodeId, stroke: Option<(Color, f64)>, fill: Option<(Color, f64)>) {
    match stroke {
        Some((c, a)) => doc
            .edit_style(&[id], &StyleEdit::StrokeRgba(c, op(a)))
            .unwrap(),
        None => doc
            .edit_style(&[id], &StyleEdit::StrokeEnabled(false))
            .unwrap(),
    }
    if let Some((c, a)) = fill {
        doc.edit_style(&[id], &StyleEdit::FillRgba(c, op(a)))
            .unwrap();
        doc.edit_style(&[id], &StyleEdit::FillEnabled(true))
            .unwrap();
    }
}

fn snapshots(doc: &Document) -> Vec<ObjectSnapshot> {
    doc.object_ids()
        .into_iter()
        .map(|id| doc.object(id).unwrap())
        .collect()
}

const TOL: Tolerance = Tolerance::from_mm(0.5);

fn picked(objects: &[ObjectSnapshot], x: f64, y: f64) -> Option<(Color, f64, PaintTarget)> {
    pick_colour(objects, Point::new(x, y), TOL).map(|p| (p.color, p.opacity.get(), p.paint))
}

const RED: Color = Color { r: 255, g: 0, b: 0 };
const BLUE: Color = Color { r: 0, g: 0, b: 255 };
const GREEN: Color = Color { r: 0, g: 255, b: 0 };

#[test]
fn an_empty_drawing_and_empty_canvas_pick_nothing() {
    assert_eq!(picked(&[], 5.0, 5.0), None);
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, a, Some((RED, 1.0)), Some((BLUE, 1.0)));
    assert_eq!(picked(&snapshots(&doc), 100.0, 100.0), None);
    assert_eq!(
        picked(&snapshots(&doc), -5.0, 10.0),
        None,
        "outside the tolerance"
    );
}

#[test]
fn stroke_on_the_outline_fill_inside_with_alpha_kept() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, a, Some((RED, 0.5)), Some((BLUE, 128.0 / 255.0)));
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 0.0, 10.0),
        Some((RED, 0.5, PaintTarget::Stroke))
    );
    assert_eq!(
        picked(&objs, 20.2, 10.0),
        Some((RED, 0.5, PaintTarget::Stroke)),
        "tolerance"
    );
    assert_eq!(
        picked(&objs, 10.0, 10.0),
        Some((BLUE, 128.0 / 255.0, PaintTarget::Fill))
    );
    assert_eq!(
        picked(&objs, 5.0, 5.0),
        Some((BLUE, 128.0 / 255.0, PaintTarget::Fill))
    );
}

#[test]
fn a_paint_that_is_none_is_not_pickable_by_that_paint() {
    let doc = Document::new(1);
    let stroke_only = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, stroke_only, Some((RED, 1.0)), None);
    let fill_only = square(&doc, 40.0, 0.0, 20.0);
    paint(&doc, fill_only, None, Some((GREEN, 1.0)));
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 10.0, 10.0),
        None,
        "unfilled inside, stroke far away"
    );
    assert_eq!(
        picked(&objs, 0.0, 10.0),
        Some((RED, 1.0, PaintTarget::Stroke))
    );
    assert_eq!(
        picked(&objs, 50.0, 10.0),
        Some((GREEN, 1.0, PaintTarget::Fill))
    );
    // The outline of the fill-only square has no stroke: it picks the fill when
    // the point is inside, nothing outside.
    assert_eq!(
        picked(&objs, 41.0, 10.0),
        Some((GREEN, 1.0, PaintTarget::Fill))
    );
    assert_eq!(picked(&objs, 39.0, 10.0), None);
}

#[test]
fn a_paint_with_opacity_zero_still_counts_as_painted() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, a, Some((RED, 0.0)), Some((BLUE, 0.0)));
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 0.0, 10.0),
        Some((RED, 0.0, PaintTarget::Stroke))
    );
    assert_eq!(
        picked(&objs, 10.0, 10.0),
        Some((BLUE, 0.0, PaintTarget::Fill))
    );
}

#[test]
fn the_topmost_painted_object_wins_even_over_a_nearer_outline_below() {
    let doc = Document::new(1);
    let below = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, below, Some((RED, 1.0)), Some((GREEN, 1.0)));
    // The upper square's fill covers the lower one's right edge.
    let above = square(&doc, 10.0, 5.0, 30.0);
    paint(&doc, above, Some((BLUE, 1.0)), Some((rgb(9, 9, 9), 1.0)));
    let objs = snapshots(&doc);
    // (20, 10): on the lower square's stroke, inside the upper square's fill.
    assert_eq!(
        picked(&objs, 20.0, 10.0),
        Some((rgb(9, 9, 9), 1.0, PaintTarget::Fill))
    );
    // (5, 10): only the lower square.
    assert_eq!(
        picked(&objs, 5.0, 10.0),
        Some((GREEN, 1.0, PaintTarget::Fill))
    );
    // (10, 10): on the upper square's left stroke: its stroke, not the lower fill.
    assert_eq!(
        picked(&objs, 10.0, 10.0),
        Some((BLUE, 1.0, PaintTarget::Stroke))
    );
}

#[test]
fn an_upper_object_with_nothing_painted_there_lets_the_pick_fall_through() {
    let doc = Document::new(1);
    let below = square(&doc, 0.0, 0.0, 40.0);
    paint(&doc, below, Some((RED, 1.0)), Some((GREEN, 0.25)));
    // An unfilled frame on top: stroke only, with a big hole in the middle.
    let above = square(&doc, 5.0, 5.0, 30.0);
    paint(&doc, above, Some((BLUE, 1.0)), None);
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 20.0, 20.0),
        Some((GREEN, 0.25, PaintTarget::Fill))
    );
    assert_eq!(
        picked(&objs, 5.0, 20.0),
        Some((BLUE, 1.0, PaintTarget::Stroke))
    );
    // The stroke of the top object is switched off: the lower stroke is next.
    doc.edit_style(&[doc.object_ids()[1]], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 5.0, 20.0),
        Some((GREEN, 0.25, PaintTarget::Fill))
    );
    assert_eq!(
        picked(&objs, 0.0, 20.0),
        Some((RED, 1.0, PaintTarget::Stroke))
    );
}

#[test]
fn an_object_picks_stroke_before_its_own_fill_at_the_outline() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, a, Some((RED, 1.0)), Some((BLUE, 1.0)));
    let objs = snapshots(&doc);
    // 0.3 mm inside the edge: inside both the fill and the stroke tolerance.
    assert_eq!(
        picked(&objs, 0.3, 10.0),
        Some((RED, 1.0, PaintTarget::Stroke))
    );
}

#[test]
fn ellipses_and_paths_pick_like_rectangles() {
    let doc = Document::new(1);
    let e = doc.create_ellipse(EllipseFrame {
        center: Point::new(0.0, 0.0),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(5.0),
    });
    paint(&doc, e, Some((RED, 1.0)), Some((BLUE, 1.0)));
    let anchor = |n: u64, x: f64, y: f64| NewAnchor {
        id: AnchorId::new(1, n),
        point: Point::new(x, y),
        handle_in: Vec2::new(0.0, 0.0),
        handle_out: Vec2::new(0.0, 0.0),
        kind: AnchorKind::Corner,
    };
    let line = doc.create_path(&[anchor(1, 50.0, 0.0), anchor(2, 70.0, 0.0)], false);
    paint(&doc, line, Some((GREEN, 1.0)), None);
    let objs = snapshots(&doc);
    assert_eq!(
        picked(&objs, 10.0, 0.0),
        Some((RED, 1.0, PaintTarget::Stroke))
    );
    assert_eq!(
        picked(&objs, 0.0, 0.0),
        Some((BLUE, 1.0, PaintTarget::Fill))
    );
    assert_eq!(picked(&objs, 0.0, 8.0), None);
    assert_eq!(
        picked(&objs, 60.0, 0.2),
        Some((GREEN, 1.0, PaintTarget::Stroke))
    );
    assert_eq!(picked(&objs, 60.0, 3.0), None);
}

#[test]
fn a_zero_tolerance_still_never_panics_and_a_huge_one_picks_the_nearest_stroke_only_in_range() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 20.0);
    paint(&doc, a, Some((RED, 1.0)), None);
    let objs = snapshots(&doc);
    let _ = pick_colour(&objs, Point::new(0.0, 10.0), Tolerance::from_mm(0.0));
    let _ = pick_colour(&objs, Point::new(f64::NAN, 10.0), TOL);
    let _ = pick_colour(&objs, Point::new(f64::INFINITY, f64::NEG_INFINITY), TOL);
    let wide = pick_colour(&objs, Point::new(25.0, 10.0), Tolerance::from_mm(6.0));
    assert!(wide.is_some());
    assert!(pick_colour(&objs, Point::new(25.0, 10.0), TOL).is_none());
}

// ============================================ AC 5 to 10: panel state

fn scope(doc: &Document) -> StyleScope {
    StyleScope {
        ids: doc.object_ids(),
        subject: String::new(),
    }
}

#[test]
fn paint_none_for_every_object_hides_the_rows_and_mixed_shows_them() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let b = square(&doc, 20.0, 0.0, 10.0);
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert!(state.stroke.rows_shown);
    assert_eq!(state.stroke.paint, BarValue::Uniform(true));
    assert!(!state.fill.rows_shown, "fill is off for new objects");
    assert_eq!(state.fill.paint, BarValue::Uniform(false));

    doc.edit_style(&[a, b], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert!(!state.stroke.rows_shown);
    assert_eq!(state.stroke.paint, BarValue::Uniform(false));

    doc.edit_style(&[a], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.stroke.paint, BarValue::Mixed, "no state pressed");
    assert!(
        state.stroke.rows_shown,
        "mixed shows the rows (criterion 9)"
    );

    doc.edit_style(&[b], &StyleEdit::FillEnabled(true)).unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.fill.paint, BarValue::Mixed);
    assert!(state.fill.rows_shown);
}

#[test]
fn a_stroke_that_is_off_still_reports_its_stored_values_for_a_later_solid() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    doc.edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(2.0)))
        .unwrap();
    doc.edit_style(&[a], &StyleEdit::StrokeRgba(RED, op(0.5)))
        .unwrap();
    doc.edit_style(&[a], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert!(!state.stroke.rows_shown);
    assert_eq!(state.stroke.width, BarValue::Uniform(Length::from_mm(2.0)));
    assert_eq!(state.stroke.rgba, BarValue::Uniform((RED, op(0.5))));
}

#[test]
fn a_width_dragged_to_zero_keeps_its_row_mid_drag() {
    // Criterion 8: while the drag runs the row stays; only the commit removes it.
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let committed = snapshots(&doc);
    let mut shown = committed.clone();
    let mut style = Style::default();
    StyleEdit::StrokeWidth(Length::from_mm(0.0))
        .apply_to(&mut style)
        .unwrap();
    assert!(
        !style.stroke.enabled,
        "the preview of width 0 draws no stroke"
    );
    let _ = a;
    if let ObjectSnapshot::Primitive(p) = &mut shown[0] {
        p.style = style;
    }
    let state = style_panel_state(&committed, &shown, &scope(&doc)).unwrap();
    assert!(
        state.stroke.rows_shown,
        "rows follow the committed document"
    );
    assert_eq!(
        state.stroke.paint,
        BarValue::Uniform(false),
        "the value follows the preview"
    );
}

#[test]
fn colour_and_alpha_mixed_flags_are_independent() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let b = square(&doc, 20.0, 0.0, 10.0);
    doc.edit_style(&[a], &StyleEdit::StrokeRgba(RED, op(1.0)))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::StrokeRgba(RED, op(0.5)))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.stroke.color, BarValue::Uniform(RED));
    assert_eq!(state.stroke.opacity, BarValue::Mixed);
    assert_eq!(
        state.stroke.rgba,
        BarValue::Mixed,
        "the hex field shows Mixed"
    );
    doc.edit_style(&[b], &StyleEdit::StrokeRgba(BLUE, op(1.0)))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.stroke.color, BarValue::Mixed);
    assert_eq!(state.stroke.opacity, BarValue::Uniform(op(1.0)));
    assert_eq!(state.stroke.rgba, BarValue::Mixed);
}

#[test]
fn dash_state_presets_custom_long_and_mixed() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let b = square(&doc, 20.0, 0.0, 10.0);
    let show = |doc: &Document| {
        let objs = snapshots(doc);
        style_panel_state(&objs, &objs, &scope(doc))
            .unwrap()
            .stroke
            .dash
    };
    assert_eq!(
        show(&doc),
        DashShown::Uniform {
            preset: Some(DashChoice::Solid),
            text: String::new()
        }
    );
    for (list, preset, text) in [
        (vec![6.0, 4.0], Some(DashChoice::Dash), "6 4"),
        (vec![1.0, 3.0], Some(DashChoice::Dot), "1 3"),
        (
            vec![6.0, 3.0, 1.0, 3.0],
            Some(DashChoice::DashDot),
            "6 3 1 3",
        ),
        (vec![1.0, 2.0, 4.0], None, "1 2 4"),
        (vec![0.0, 3.0], None, "0 3"),
    ] {
        doc.edit_style(
            &[a, b],
            &StyleEdit::StrokeDash(DashPattern::new(list).unwrap()),
        )
        .unwrap();
        assert_eq!(
            show(&doc),
            DashShown::Uniform {
                preset,
                text: text.to_string()
            }
        );
    }
    // More than 16 numbers from a file are shown in full.
    let long: Vec<f64> = (1..=20).map(f64::from).collect();
    doc.edit_style(
        &[a, b],
        &StyleEdit::StrokeDash(DashPattern::new(long.clone()).unwrap()),
    )
    .unwrap();
    match show(&doc) {
        DashShown::Uniform { preset, text } => {
            assert_eq!(preset, None);
            assert_eq!(text.split(' ').count(), 20);
            // And it is not rewritten by looking: parsing it would be refused.
            assert!(parse_dash_text(&text).is_err());
        }
        DashShown::Mixed => panic!("same list"),
    }
    doc.edit_style(&[a], &StyleEdit::StrokeDash(DashChoice::Dot.pattern()))
        .unwrap();
    assert_eq!(show(&doc), DashShown::Mixed);
}

#[test]
fn a_dash_edit_does_not_change_the_paint_state_of_a_mixed_selection() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let b = square(&doc, 20.0, 0.0, 10.0);
    doc.edit_style(&[b], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::StrokeDash(DashChoice::Dash.pattern()))
        .unwrap();
    doc.edit_style(
        &[a, b],
        &StyleEdit::StrokeCap(curvyo_document_core::LineCap::Round),
    )
    .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.stroke.paint, BarValue::Mixed);
    // A colour edit turns the off ones on (stroke rule) ...
    doc.edit_style(&[a, b], &StyleEdit::StrokeColor(BLUE))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.stroke.paint, BarValue::Uniform(true));
    // ... but a fill colour edit never turns a fill on.
    doc.edit_style(&[a], &StyleEdit::FillEnabled(true)).unwrap();
    doc.edit_style(&[a, b], &StyleEdit::FillColor(GREEN))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::FillOpacity(op(0.5)))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::FillRgba(GREEN, op(0.5)))
        .unwrap();
    let objs = snapshots(&doc);
    let state = style_panel_state(&objs, &objs, &scope(&doc)).unwrap();
    assert_eq!(state.fill.paint, BarValue::Mixed);
}

#[test]
fn an_unknown_scope_id_gives_no_panel_and_a_deleted_object_is_dropped() {
    let doc = Document::new(1);
    let a = square(&doc, 0.0, 0.0, 10.0);
    let objs = snapshots(&doc);
    let empty = StyleScope {
        ids: vec![],
        subject: String::new(),
    };
    assert!(style_panel_state(&objs, &objs, &empty).is_none());
    let other = Document::new(2);
    let ghost = square(&other, 0.0, 0.0, 10.0);
    let ghost_scope = StyleScope {
        ids: vec![ghost],
        subject: "Rectangle".into(),
    };
    assert!(style_panel_state(&objs, &objs, &ghost_scope).is_none());
    let mixed_scope = StyleScope {
        ids: vec![ghost, a],
        subject: "2 objects".into(),
    };
    let state = style_panel_state(&objs, &objs, &mixed_scope).unwrap();
    assert_eq!(state.subject, "2 objects");
}
