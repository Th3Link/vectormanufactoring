//! The vocabulary of a document format's size (`specs/0030-document-size-
//! presets/`): portrait or landscape, the unit a size is written in, and the size
//! as written. The numbers are lengths; a pixel exists only here.

use crate::display_unit::DisplayUnit;
use crate::document_presets::PRESET_MATCH_TOLERANCE;
use crate::units::{DocumentSize, Length};

/// Portrait or landscape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Taller than wide.
    Portrait,
    /// Wider than tall.
    Landscape,
}

impl Orientation {
    /// The orientation of `size`: `None` for a square within
    /// [`PRESET_MATCH_TOLERANCE`].
    #[must_use]
    pub fn of(size: DocumentSize) -> Option<Self> {
        let (width, height) = (size.width.as_mm(), size.height.as_mm());
        if (width - height).abs() <= PRESET_MATCH_TOLERANCE.as_mm() {
            None
        } else if width < height {
            Some(Self::Portrait)
        } else {
            Some(Self::Landscape)
        }
    }
}

/// The unit a preset's size is written in. A pixel exists only here: it is
/// 1/96 inch, and the document stores millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetUnit {
    /// Millimetres.
    Mm,
    /// Centimetres.
    Cm,
    /// Inches.
    In,
    /// Pixels at 96 per inch.
    Px,
}

impl PresetUnit {
    /// The unit's symbol as written in the file.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::In => "in",
            Self::Px => "px",
        }
    }

    /// The unit written `symbol` (`mm`, `cm`, `in` or `px`).
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "mm" => Some(Self::Mm),
            "cm" => Some(Self::Cm),
            "in" => Some(Self::In),
            "px" => Some(Self::Px),
            _ => None,
        }
    }

    /// `value` in this unit, in millimetres: `mm` as is, `cm` as `value * 10`,
    /// `in` as `value * 254 / 10`, `px` as `value * 254 / 960`.
    #[must_use]
    pub fn to_length(self, value: f64) -> Length {
        match self {
            Self::Mm => Length::from_mm(value),
            Self::Cm => Length::from_mm(value * 10.0),
            Self::In => Length::from_unit(value, DisplayUnit::In),
            Self::Px => Length::from_mm(value * 254.0 / 960.0),
        }
    }
}

/// A preset's size as written in the file, kept for the tooltip ("1920 × 1080
/// px"). Every comparison and every write uses the `Length` sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredSize {
    /// The shorter side, in `unit`.
    pub short: f64,
    /// The longer side, in `unit`.
    pub long: f64,
    /// The unit the numbers are in.
    pub unit: PresetUnit,
}
