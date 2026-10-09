//! The display unit: how lengths are shown, converted and stored as a document
//! setting.
//!
//! It is presentation only: stored values stay in millimetres and a unit
//! change moves nothing (`specs/0015-document-size-and-rulers/`, criteria 34,
//! 36 and 38; ADR 0002 §2). [`DisplayUnit`] holds the exact conversions;
//! [`Document::display_unit`] and [`Document::set_display_unit`] read and
//! write the root register.

use loro::LoroValue;
use serde::Serialize;

use crate::document::{Document, KEY_DISPLAY_UNIT, ROOT_MAP};
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
        // An inch is 254 / 10 mm: multiplying by the integer 254 first and
        // dividing once rounds the result once, so 8.5 in is the double
        // nearest to 215.9 mm and not a neighbour of it.
        Self::from_mm(match unit {
            DisplayUnit::Mm => value,
            DisplayUnit::Cm => value * 10.0,
            DisplayUnit::In => value * 254.0 / 10.0,
        })
    }

    /// This length as a plain number in `unit`. The one conversion out of
    /// millimetres, for text and for nothing else.
    #[must_use]
    pub fn in_unit(self, unit: DisplayUnit) -> f64 {
        match unit {
            DisplayUnit::Mm => self.as_mm(),
            DisplayUnit::Cm => self.as_mm() / 10.0,
            DisplayUnit::In => self.as_mm() * 10.0 / 254.0,
        }
    }
}

impl Document {
    /// The unit lengths are shown in. Absent or unknown reads as mm
    /// (`specs/0015-document-size-and-rulers/` criteria 36 and 38).
    #[must_use]
    pub fn display_unit(&self) -> DisplayUnit {
        let root = self.loro().get_map(ROOT_MAP);
        match root.get(KEY_DISPLAY_UNIT).map(|v| v.get_deep_value()) {
            Some(LoroValue::String(symbol)) => {
                DisplayUnit::from_symbol(&symbol).unwrap_or_default()
            }
            _ => DisplayUnit::default(),
        }
    }

    /// Sets the unit lengths are shown in, as one commit labelled
    /// `set_display_unit` that moves nothing (criteria 36 and 38). Returns
    /// `false` and writes nothing if `unit` is already the display unit.
    ///
    /// # Panics
    /// Does not panic in practice: it inserts a plain string into the
    /// attached root map.
    #[must_use]
    pub fn set_display_unit(&self, unit: DisplayUnit) -> bool {
        if self.display_unit() == unit {
            return false;
        }
        let root = self.loro().get_map(ROOT_MAP);
        // invariant: the root map is attached and the value is a plain string.
        #[allow(clippy::unwrap_used)]
        root.insert(KEY_DISPLAY_UNIT, unit.symbol()).unwrap();
        self.commit_with_label("set_display_unit");
        true
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

    /// Typing 8.5 and 11 inches stores exactly the doubles of 215.9 and 279.4
    /// mm (criterion 35), not neighbours of them.
    #[test]
    fn typed_inches_are_the_nearest_double_of_the_decimal_millimetres() {
        assert_eq!(Length::from_unit(8.5, DisplayUnit::In).as_mm(), 215.9);
        assert_eq!(Length::from_unit(11.0, DisplayUnit::In).as_mm(), 279.4);
        assert_eq!(Length::from_unit(1.0, DisplayUnit::In).as_mm(), 25.4);
        assert_eq!(Length::from_mm(215.9).in_unit(DisplayUnit::In), 8.5);
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

    #[test]
    fn display_unit_defaults_to_mm_and_changing_it_is_one_commit_that_moves_nothing() {
        use crate::primitive_model::RectBounds;
        use crate::units::Point;

        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(1.0, 2.0),
            width: Length::from_mm(3.0),
            height: Length::from_mm(4.0),
        });
        let object = document.object(id);
        let size = document.size();
        assert_eq!(document.display_unit(), DisplayUnit::Mm);
        let before = document.loro().len_changes();

        assert!(document.set_display_unit(DisplayUnit::In));

        assert_eq!(document.display_unit(), DisplayUnit::In);
        assert_eq!(document.loro().len_changes(), before + 1);
        let vv = document.loro().oplog_vv();
        let peer = document.loro().peer_id();
        let end = vv.get(&peer).copied().expect("the peer has written");
        let change = document
            .loro()
            .get_change(loro::ID::new(peer, end - 1))
            .expect("the newest change exists");
        assert_eq!(change.message(), "set_display_unit");
        assert_eq!(document.object(id), object);
        assert_eq!(document.size(), size);

        assert!(!document.set_display_unit(DisplayUnit::In), "same unit");
        assert_eq!(document.loro().len_changes(), before + 1);
    }

    #[test]
    fn an_unknown_stored_unit_reads_as_mm() {
        let document = Document::new(1);
        let root = document.loro().get_map(ROOT_MAP);
        root.insert(KEY_DISPLAY_UNIT, "furlong").unwrap();
        assert_eq!(document.display_unit(), DisplayUnit::Mm);
        root.insert(KEY_DISPLAY_UNIT, 3.0).unwrap();
        assert_eq!(document.display_unit(), DisplayUnit::Mm);
    }
}
