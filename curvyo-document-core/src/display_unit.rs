//! The unit in which lengths are shown to the maker (mm, cm, in), and the only
//! conversions between it and [`Length`]'s millimetres.
//!
//! The display unit is presentation only: stored values stay in millimetres
//! and a unit change moves nothing (`specs/0015-document-size-and-rulers/`,
//! criteria 34 and 38; ADR 0002 §2).

use serde::Serialize;

use crate::units::Length;

/// A unit for showing lengths. One inch is exactly 25.4 mm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayUnit {
    /// Millimetres, the stored unit and the default.
    #[default]
    Mm,
    /// Centimetres, exactly 10 mm.
    Cm,
    /// Inches, exactly 25.4 mm.
    In,
}

impl DisplayUnit {
    /// Every unit, in the order the maker is offered them.
    pub const ALL: [Self; 3] = [Self::Mm, Self::Cm, Self::In];

    /// How many millimetres one of this unit is: 1, 10 or exactly 25.4.
    #[must_use]
    pub const fn mm_per_unit(self) -> f64 {
        match self {
            Self::Mm => 1.0,
            Self::Cm => 10.0,
            Self::In => 25.4,
        }
    }

    /// The unit's symbol as stored in the project file and shown to the
    /// maker: `"mm"`, `"cm"` or `"in"`.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::In => "in",
        }
    }

    /// The unit with this exact symbol, or `None` for any other text.
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.symbol() == symbol)
    }
}

impl Length {
    /// Builds a [`Length`] from a value in `unit`. The one conversion into
    /// millimetres.
    #[must_use]
    pub fn from_unit(value: f64, unit: DisplayUnit) -> Self {
        Self::from_mm(value * unit.mm_per_unit())
    }

    /// This length as a plain number in `unit`. The one conversion out of
    /// millimetres, for text and for nothing else.
    #[must_use]
    pub fn in_unit(self, unit: DisplayUnit) -> f64 {
        self.as_mm() / unit.mm_per_unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inch_is_exactly_25_4_mm() {
        assert!((DisplayUnit::In.mm_per_unit() - 25.4).abs() < f64::EPSILON);
        assert!((Length::from_unit(1.0, DisplayUnit::In).as_mm() - 25.4).abs() < f64::EPSILON);
        assert!((Length::from_unit(8.5, DisplayUnit::In).as_mm() - 215.9).abs() < 1e-9);
        assert!((Length::from_unit(11.0, DisplayUnit::In).as_mm() - 279.4).abs() < 1e-9);
    }

    #[test]
    fn a_centimetre_is_ten_mm_and_a_mm_is_itself() {
        assert!((Length::from_unit(2.1, DisplayUnit::Cm).as_mm() - 21.0).abs() < 1e-12);
        assert!((Length::from_unit(7.5, DisplayUnit::Mm).as_mm() - 7.5).abs() < f64::EPSILON);
    }

    #[test]
    fn in_unit_inverts_from_unit() {
        for unit in DisplayUnit::ALL {
            for value in [0.0, 0.04, 1.0, 8.5, 3937.0, -12.25] {
                let back = Length::from_unit(value, unit).in_unit(unit);
                assert!((back - value).abs() < 1e-9, "{unit:?} {value}: {back}");
            }
        }
    }

    #[test]
    fn symbols_round_trip_and_unknown_text_is_not_a_unit() {
        for unit in DisplayUnit::ALL {
            assert_eq!(DisplayUnit::from_symbol(unit.symbol()), Some(unit));
        }
        for text in ["", "MM", "m", "inch", " mm", "pt"] {
            assert_eq!(DisplayUnit::from_symbol(text), None, "{text:?}");
        }
        assert_eq!(DisplayUnit::default(), DisplayUnit::Mm);
    }
}
