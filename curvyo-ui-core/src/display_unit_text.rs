//! Text for lengths in the display unit: parsing what the maker types into a
//! document side, and formatting sizes, positions and limits
//! (`specs/0015-document-size-and-rulers/` criteria 15, 16, 21, 34 and 35).
//!
//! A value in the display unit is a bare `f64` only inside one function here,
//! between the text and a [`Length`]. Showing a value never rewrites the
//! stored one.

use curvyo_document_core::{
    DisplayUnit, DocumentSize, Length, MAX_DOCUMENT_MM, MIN_DOCUMENT_MM, validated_document_side,
};

use crate::transform_entry::parse_entry_number;

/// Parses the text of a Width or Height field in `unit` into a document side
/// (criteria 15, 16 and 35). The text is trimmed, may use a decimal point or
/// a decimal comma, and carries no unit: "21cm" is not a number. A value
/// outside 1 mm to 100 000 mm (converted from `unit`) is refused, except
/// within half a unit of the last decimal the field shows
/// ([`format_field_length`]) of a limit, where it is clamped onto it. That
/// makes the text the app shows for the largest size, "3937.0079" in, accept
/// when it is typed back, although it is 0.0007 mm over the limit. `None` is
/// the "Enter a number from ..." case of [`document_side_message`].
#[must_use]
pub fn parse_document_side(text: &str, unit: DisplayUnit) -> Option<Length> {
    let value = parse_entry_number(text, false)?;
    let tolerance = field_precision(unit).1 * unit.mm_per_unit();
    let mm = Length::from_unit(value, unit).as_mm();
    let snapped = [MIN_DOCUMENT_MM, MAX_DOCUMENT_MM]
        .into_iter()
        .find(|limit| (mm - limit).abs() <= tolerance)
        .unwrap_or(mm);
    validated_document_side(Length::from_mm(snapped)).ok()
}

/// The validation message for a refused Width or Height, with the limits in
/// `unit` rounded inward so it never promises a value the field refuses
/// (criterion 16): mm "Enter a number from 1 to 100000", cm "Enter a number
/// from 0.1 to 10000", in "Enter a number from 0.04 to 3937".
#[must_use]
pub fn document_side_message(unit: DisplayUnit) -> String {
    let lowest = Length::from_mm(MIN_DOCUMENT_MM).in_unit(unit);
    let highest = Length::from_mm(MAX_DOCUMENT_MM).in_unit(unit);
    format!(
        "Enter a number from {} to {}",
        decimal_text(ceil_hundredths(lowest), 2),
        decimal_text(floor_hundredths(highest), 2),
    )
}

/// The message for a Fit to content whose content is larger than the largest
/// document (criterion 27a), with the limit in `unit` rounded inward.
#[must_use]
pub fn content_too_large_message(unit: DisplayUnit) -> String {
    let highest = Length::from_mm(MAX_DOCUMENT_MM).in_unit(unit);
    format!(
        "The content is larger than the largest document ({} {}). Nothing was changed.",
        decimal_text(floor_hundredths(highest), 2),
        unit.symbol(),
    )
}

/// A length for a Width or Height field: 3 decimals in mm, 4 in cm and in,
/// without trailing zeros (criterion 35). Display only.
#[must_use]
pub fn format_field_length(length: Length, unit: DisplayUnit) -> String {
    decimal_text(length.in_unit(unit), field_precision(unit).0)
}

/// What a Width or Height field shows in `unit`: the number of decimals, and
/// half a unit of the last one (the most a shown value can differ from the
/// stored one).
fn field_precision(unit: DisplayUnit) -> (usize, f64) {
    match unit {
        DisplayUnit::Mm => (3, 0.0005),
        DisplayUnit::Cm | DisplayUnit::In => (4, 0.00005),
    }
}

/// A length for the status bar with a fixed number of decimals so the text
/// does not jitter: mm 1, cm 2, in 3 (all the same 0.1 mm resolution;
/// criterion 21). No unit suffix.
#[must_use]
pub fn format_status_length(length: Length, unit: DisplayUnit) -> String {
    let decimals = match unit {
        DisplayUnit::Mm => 1,
        DisplayUnit::Cm => 2,
        DisplayUnit::In => 3,
    };
    fixed_text(length.in_unit(unit), decimals)
}

/// The status bar's cursor readout with the unit once at the end, for example
/// "x: 12.3  y: 45.6 mm" (criterion 21).
#[must_use]
pub fn format_cursor(x: Length, y: Length, unit: DisplayUnit) -> String {
    format!(
        "x: {}  y: {} {}",
        format_status_length(x, unit),
        format_status_length(y, unit),
        unit.symbol(),
    )
}

/// The status bar's size readout, for example "210.0 × 297.0 mm"
/// (criteria 12 and 21).
#[must_use]
pub fn format_size(size: DocumentSize, unit: DisplayUnit) -> String {
    format!(
        "{} \u{d7} {} {}",
        format_status_length(size.width, unit),
        format_status_length(size.height, unit),
        unit.symbol(),
    )
}

/// The smallest hundredth that is not below `value`. The 1e-9 keeps a value
/// that is exactly on a hundredth, such as 0.1 cm (1 mm), from being pushed up
/// by binary rounding.
fn ceil_hundredths(value: f64) -> f64 {
    (value * 100.0 - 1e-9).ceil() / 100.0
}

/// The largest hundredth that is not above `value`, with the same nudge.
fn floor_hundredths(value: f64) -> f64 {
    (value * 100.0 + 1e-9).floor() / 100.0
}

/// `value` with exactly `decimals` decimals, and no "-0".
fn fixed_text(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| matches!(c, '0' | '.')) => rest.to_string(),
        _ => text,
    }
}

/// `value` with at most `decimals` decimals and no trailing zeros or point.
fn decimal_text(value: f64, decimals: usize) -> String {
    let text = fixed_text(value, decimals);
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed_mm(text: &str, unit: DisplayUnit) -> Option<f64> {
        parse_document_side(text, unit).map(Length::as_mm)
    }

    /// Criterion 35: 8.5 in and 11 in are exactly 215.9 mm and 279.4 mm.
    #[test]
    fn inches_parse_to_exact_millimetres() {
        assert!((parsed_mm("8.5", DisplayUnit::In).unwrap() - 215.9).abs() < 1e-9);
        assert!((parsed_mm("11", DisplayUnit::In).unwrap() - 279.4).abs() < 1e-9);
        assert!((parsed_mm("2.5", DisplayUnit::Cm).unwrap() - 25.0).abs() < 1e-9);
    }

    /// Criterion 15: a decimal point or comma, surrounding spaces.
    #[test]
    fn a_decimal_point_a_decimal_comma_and_spaces_are_accepted() {
        assert_eq!(parsed_mm("12.5", DisplayUnit::Mm), Some(12.5));
        assert_eq!(parsed_mm("12,5", DisplayUnit::Mm), Some(12.5));
        assert_eq!(parsed_mm("  300  ", DisplayUnit::Mm), Some(300.0));
    }

    /// Criterion 16: empty, text, a unit in the text, exponents and several
    /// separators are not numbers.
    #[test]
    fn text_that_is_not_a_plain_number_is_refused() {
        for text in [
            "", " ", "abc", "21cm", "21 mm", "1e3", "1,2,3", "1.2.3", ".", "NaN", "inf", "--5",
            "5 5",
        ] {
            assert_eq!(parsed_mm(text, DisplayUnit::Mm), None, "{text:?}");
        }
    }

    /// Criterion 16: 1 mm to 100 000 mm in the typed unit, inclusive, with
    /// 1e-9 mm of tolerance clamped onto the limit.
    #[test]
    fn values_outside_the_limits_are_refused_and_the_limits_are_inclusive() {
        for (text, unit, ok) in [
            ("0.999", DisplayUnit::Mm, false),
            ("1", DisplayUnit::Mm, true),
            ("100000", DisplayUnit::Mm, true),
            ("100000.1", DisplayUnit::Mm, false),
            ("0.1", DisplayUnit::Cm, true),
            ("0.09", DisplayUnit::Cm, false),
            ("10000", DisplayUnit::Cm, true),
            ("10001", DisplayUnit::Cm, false),
            ("0.04", DisplayUnit::In, true),
            ("0.039", DisplayUnit::In, false),
            ("3937", DisplayUnit::In, true),
            ("3938", DisplayUnit::In, false),
            ("0", DisplayUnit::Mm, false),
            ("-5", DisplayUnit::Mm, false),
            (
                "99999999999999999999999999999999999999999999999999",
                DisplayUnit::Mm,
                false,
            ),
        ] {
            assert_eq!(parsed_mm(text, unit).is_some(), ok, "{text:?} {unit:?}");
        }
        let near = parsed_mm("100000.0000000005", DisplayUnit::Mm).unwrap();
        assert!(
            (near - 100_000.0).abs() < f64::EPSILON,
            "clamped onto the limit"
        );
        let low = parsed_mm("0.9999999995", DisplayUnit::Mm).unwrap();
        assert!((low - 1.0).abs() < f64::EPSILON);
    }

    /// Criterion 16: the three messages, limits rounded inward.
    #[test]
    fn the_validation_message_names_the_limits_in_the_display_unit() {
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

    /// Every limit the message names is accepted by the parser, so the
    /// message never promises a value the field refuses.
    #[test]
    fn the_limits_in_the_message_are_accepted_by_the_parser() {
        for unit in DisplayUnit::ALL {
            let message = document_side_message(unit);
            let numbers: Vec<_> = message
                .split_whitespace()
                .filter(|word| word.parse::<f64>().is_ok())
                .collect();
            assert_eq!(numbers.len(), 2, "{message}");
            for number in numbers {
                assert!(
                    parse_document_side(number, unit).is_some(),
                    "{unit:?} {number}"
                );
            }
        }
    }

    #[test]
    fn the_content_too_large_message_names_the_limit() {
        assert_eq!(
            content_too_large_message(DisplayUnit::Mm),
            "The content is larger than the largest document (100000 mm). Nothing was changed."
        );
        assert!(content_too_large_message(DisplayUnit::In).contains("(3937 in)"));
    }

    /// Criterion 35: 3 decimals in mm, 4 in cm and in, no trailing zeros; and
    /// showing a value does not change it.
    #[test]
    fn field_lengths_round_and_drop_trailing_zeros() {
        let a4 = DocumentSize::default();
        assert_eq!(format_field_length(a4.width, DisplayUnit::Mm), "210");
        assert_eq!(format_field_length(a4.width, DisplayUnit::Cm), "21");
        assert_eq!(format_field_length(a4.width, DisplayUnit::In), "8.2677");
        assert_eq!(
            format_field_length(Length::from_mm(215.9), DisplayUnit::In),
            "8.5"
        );
        assert_eq!(
            format_field_length(Length::from_mm(12.345_678), DisplayUnit::Mm),
            "12.346"
        );
        assert_eq!(
            format_field_length(Length::from_mm(0.0004), DisplayUnit::Mm),
            "0"
        );
        assert_eq!(
            format_field_length(Length::from_mm(100_000.0), DisplayUnit::Mm),
            "100000"
        );
        assert!((a4.width.as_mm() - 210.0).abs() < f64::EPSILON);
    }

    /// Criterion 21: fixed decimals, the unit once.
    #[test]
    fn status_readouts_have_fixed_decimals_and_the_unit_once() {
        let size = DocumentSize::default();
        assert_eq!(format_size(size, DisplayUnit::Mm), "210.0 \u{d7} 297.0 mm");
        assert_eq!(format_size(size, DisplayUnit::Cm), "21.00 \u{d7} 29.70 cm");
        assert_eq!(format_size(size, DisplayUnit::In), "8.268 \u{d7} 11.693 in");
        assert_eq!(
            format_cursor(
                Length::from_mm(12.34),
                Length::from_mm(45.67),
                DisplayUnit::Mm
            ),
            "x: 12.3  y: 45.7 mm"
        );
        assert_eq!(
            format_cursor(
                Length::from_mm(25.4),
                Length::from_mm(-50.8),
                DisplayUnit::In
            ),
            "x: 1.000  y: -2.000 in"
        );
    }

    #[test]
    fn a_tiny_negative_value_never_prints_as_minus_zero() {
        assert_eq!(
            format_status_length(Length::from_mm(-0.01), DisplayUnit::Mm),
            "0.0"
        );
        assert_eq!(
            format_field_length(Length::from_mm(-0.0001), DisplayUnit::Mm),
            "0"
        );
    }

    /// The text a field shows for any valid size is accepted when it is typed
    /// back, at both limits and next to them, in every unit; the value it
    /// parses to is within half a displayed decimal of the stored one.
    #[test]
    fn the_text_shown_for_a_valid_size_is_accepted_when_retyped() {
        for unit in DisplayUnit::ALL {
            for mm in [
                1.0,
                1.0004,
                1.5,
                99.9,
                25.4,
                215.9,
                99_999.999_6,
                99_999.5,
                100_000.0,
            ] {
                let shown = format_field_length(Length::from_mm(mm), unit);
                let back = parse_document_side(&shown, unit)
                    .unwrap_or_else(|| panic!("{unit:?} {mm}: shown {shown:?} is refused"));
                let half_step = field_precision(unit).1 * unit.mm_per_unit();
                assert!(
                    (back.as_mm() - mm).abs() <= half_step + 1e-9,
                    "{unit:?} {mm}"
                );
            }
        }
        assert_eq!(parsed_mm("3937.0079", DisplayUnit::In), Some(100_000.0));
    }

    /// The display tolerance is half a displayed decimal, not more.
    #[test]
    fn a_value_beyond_the_display_tolerance_of_a_limit_is_still_refused() {
        assert_eq!(parsed_mm("3937.01", DisplayUnit::In), None);
        assert_eq!(parsed_mm("10000.0001", DisplayUnit::Cm), None);
        assert_eq!(parsed_mm("100000.001", DisplayUnit::Mm), None);
        assert_eq!(parsed_mm("0.9994", DisplayUnit::Mm), None);
        assert_eq!(parsed_mm("0.0393", DisplayUnit::In), None);
        assert_eq!(parsed_mm("0.9996", DisplayUnit::Mm), Some(1.0));
    }
}
