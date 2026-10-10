//! The dash pattern as a line of text and back
//! (`specs/0017-style-panel-rework` criteria 29 to 31): numbers separated by
//! spaces, in multiples of the stroke width, on then off.

use curvyo_document_core::DashPattern;

use crate::style_entry::StyleEntryError;

/// The most numbers a typed pattern may hold.
pub const MAX_DASH_NUMBERS: usize = 16;

/// The largest single number a typed pattern may hold.
pub const MAX_DASH_NUMBER: f64 = 1000.0;

/// Reads the text of the pattern line. Numbers are separated by runs of
/// spaces; the decimal mark is the point, so a comma is never part of a
/// number and never a separator. The empty text is the solid pattern.
///
/// # Errors
/// [`StyleEntryError::Dash`] for more than [`MAX_DASH_NUMBERS`] numbers, a
/// number that is not a plain decimal number from 0 to [`MAX_DASH_NUMBER`],
/// or a sum of zero.
pub fn parse_dash_text(text: &str) -> Result<DashPattern, StyleEntryError> {
    let mut lengths = Vec::new();
    for word in text.split(' ').filter(|word| !word.is_empty()) {
        lengths.push(parse_number(word)?);
        if lengths.len() > MAX_DASH_NUMBERS {
            return Err(StyleEntryError::Dash);
        }
    }
    DashPattern::new(lengths).map_err(|_| StyleEntryError::Dash)
}

/// One number: digits with at most one point, no sign, no exponent, no
/// separator other than the point.
fn parse_number(word: &str) -> Result<f64, StyleEntryError> {
    let plain = word.chars().all(|c| c.is_ascii_digit() || c == '.')
        && word.chars().filter(|c| *c == '.').count() <= 1
        && word.chars().any(|c| c.is_ascii_digit());
    let value = if plain {
        word.parse::<f64>().ok()
    } else {
        None
    };
    match value {
        Some(n) if n.is_finite() && (0.0..=MAX_DASH_NUMBER).contains(&n) => Ok(n),
        _ => Err(StyleEntryError::Dash),
    }
}

/// The pattern as the line shows it: the numbers separated by one space, in
/// Rust's shortest form (`0.5`, `6`). The solid pattern is the empty text.
#[must_use]
pub fn dash_text(pattern: &DashPattern) -> String {
    pattern
        .as_slice()
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lengths(text: &str) -> Vec<f64> {
        parse_dash_text(text).unwrap().as_slice().to_vec()
    }

    #[test]
    fn numbers_are_separated_by_runs_of_spaces() {
        assert_eq!(lengths("1 2 4 2"), [1.0, 2.0, 4.0, 2.0]);
        assert_eq!(lengths("  1   2 4  2 "), [1.0, 2.0, 4.0, 2.0]);
        assert_eq!(lengths("6"), [6.0], "an odd list is stored as typed");
        assert_eq!(lengths("1 2 4"), [1.0, 2.0, 4.0]);
        assert_eq!(lengths("0.5 .5 3."), [0.5, 0.5, 3.0]);
        assert_eq!(lengths("0 3"), [0.0, 3.0], "a zero on entry is allowed");
    }

    #[test]
    fn empty_text_is_solid() {
        assert!(parse_dash_text("").unwrap().is_solid());
        assert!(parse_dash_text("   ").unwrap().is_solid());
    }

    #[test]
    fn a_comma_is_neither_separator_nor_decimal_mark() {
        for text in ["1,2,4,2", "1, 2", "1,5", "1 2,5"] {
            assert_eq!(parse_dash_text(text), Err(StyleEntryError::Dash), "{text}");
        }
    }

    #[test]
    fn counts_ranges_and_sums_are_limited() {
        let sixteen = ["1"; 16].join(" ");
        assert_eq!(lengths(&sixteen).len(), 16);
        let seventeen = ["1"; 17].join(" ");
        assert_eq!(parse_dash_text(&seventeen), Err(StyleEntryError::Dash));
        assert!(parse_dash_text("1000 1000").is_ok());
        for text in [
            "1001 1", "0 0", "0", "-1 2", "+1 2", "1e2 3", "a b", ". 1", "1..2 3", "1.2.3 4",
        ] {
            assert_eq!(parse_dash_text(text), Err(StyleEntryError::Dash), "{text}");
        }
    }

    #[test]
    fn display_uses_one_space_and_the_shortest_form() {
        assert_eq!(dash_text(&DashPattern::solid()), "");
        let pattern = parse_dash_text("1   2 4  2").unwrap();
        assert_eq!(dash_text(&pattern), "1 2 4 2");
        assert_eq!(dash_text(&parse_dash_text("0.50 6.0").unwrap()), "0.5 6");
        assert_eq!(parse_dash_text(&dash_text(&pattern)).unwrap(), pattern);
    }
}
