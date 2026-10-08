//! How the host's input to the Style panel (typed text and choice words)
//! becomes the style edit it commits (`specs/0007-stroke-and-fill-styling`
//! criteria 5, 6, 13, 14).

use curvyo_document_core::{Color, FillMode, Length, LineCap, LineJoin, Opacity, StyleEdit};

use crate::transform_entry::parse_entry_number;

/// The widest stroke the field accepts, millimetres.
pub const MAX_STROKE_WIDTH_MM: f64 = 1000.0;

/// Why the text of a style field was refused. The field keeps focus, selects
/// its text and shows the matching message; nothing is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleEntryError {
    /// Not 3 or 6 hex digits ("Enter 3 or 6 hex digits").
    Hex,
    /// 8 hex digits: colour and opacity are independent ("Use 6 digits; set
    /// opacity separately").
    HexEightDigits,
    /// Not a number from 0 to 100 ("Enter a number from 0 to 100").
    Percent,
    /// Not a number from 0 to 1000 ("Enter a number from 0 to 1000").
    Width,
}

impl StyleEntryError {
    /// A short stable code for the host, which owns the message text.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::HexEightDigits => "hex8",
            Self::Percent => "percent",
            Self::Width => "width",
        }
    }
}

/// Parses a hex colour: 3 or 6 digits, with or without a leading `#`, in any
/// case (`#F80` is `#FF8800`).
///
/// # Errors
/// [`StyleEntryError::HexEightDigits`] for 8 digits, [`StyleEntryError::Hex`]
/// for anything else that is not 3 or 6 hex digits.
pub fn parse_hex(text: &str) -> Result<Color, StyleEntryError> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(StyleEntryError::Hex);
    }
    let value = |hex: &str| u8::from_str_radix(hex, 16).map_err(|_| StyleEntryError::Hex);
    match digits.len() {
        3 => {
            let channel = |n: usize| value(&digits[n..=n].repeat(2));
            Ok(Color {
                r: channel(0)?,
                g: channel(1)?,
                b: channel(2)?,
            })
        }
        6 => Ok(Color {
            r: value(&digits[0..2])?,
            g: value(&digits[2..4])?,
            b: value(&digits[4..6])?,
        }),
        8 => Err(StyleEntryError::HexEightDigits),
        _ => Err(StyleEntryError::Hex),
    }
}

/// An opacity from a whole percent `n` (0 to 100), stored as exactly `n / 100`
/// so it reads back as `n`. A value outside the range is clamped.
#[must_use]
pub fn opacity_from_percent(percent: f64) -> Opacity {
    let whole = if percent.is_finite() {
        percent.round().clamp(0.0, 100.0)
    } else {
        100.0
    };
    // `whole` is within 0..=100, so the fraction is a valid opacity; `+ 0.0`
    // turns a negative zero into zero.
    Opacity::new(whole / 100.0 + 0.0).unwrap_or(Opacity::OPAQUE)
}

/// Parses an opacity field: an integer percent, a typed decimal rounded to the
/// nearest integer, an optional trailing `%`.
///
/// # Errors
/// [`StyleEntryError::Percent`] for text that is not a number or rounds
/// outside 0 to 100.
pub fn parse_opacity_percent(text: &str) -> Result<Opacity, StyleEntryError> {
    let trimmed = text.trim();
    let number = trimmed.strip_suffix('%').unwrap_or(trimmed);
    let value = parse_entry_number(number, false).ok_or(StyleEntryError::Percent)?;
    let rounded = value.round();
    if (0.0..=100.0).contains(&rounded) {
        Ok(opacity_from_percent(rounded))
    } else {
        Err(StyleEntryError::Percent)
    }
}

/// Parses the stroke width field, millimetres, with `.` or `,` as the decimal
/// mark. Zero is allowed: it switches the stroke off.
///
/// # Errors
/// [`StyleEntryError::Width`] for text that is not a number from 0 to 1000.
pub fn parse_stroke_width(text: &str) -> Result<Length, StyleEntryError> {
    match parse_entry_number(text, false) {
        Some(mm) if (0.0..=MAX_STROKE_WIDTH_MM).contains(&mm) => Ok(Length::from_mm(mm)),
        _ => Err(StyleEntryError::Width),
    }
}

/// A style property edited with a typed value or a colour area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleField {
    /// The stroke width.
    StrokeWidth,
    /// The stroke colour.
    StrokeColor,
    /// The stroke opacity.
    StrokeOpacity,
    /// The solid fill colour.
    FillColor,
    /// The solid fill opacity.
    FillOpacity,
}

impl StyleField {
    /// The field named by the host (`"stroke-width"`, `"fill-color"`, ...).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "stroke-width" => Self::StrokeWidth,
            "stroke-color" => Self::StrokeColor,
            "stroke-opacity" => Self::StrokeOpacity,
            "fill-color" => Self::FillColor,
            "fill-opacity" => Self::FillOpacity,
            _ => return None,
        })
    }

    /// The edit that typing `text` commits.
    ///
    /// # Errors
    /// The parse error of the field's own kind of value.
    pub fn parse_text(self, text: &str) -> Result<StyleEdit, StyleEntryError> {
        Ok(match self {
            Self::StrokeWidth => StyleEdit::StrokeWidth(parse_stroke_width(text)?),
            Self::StrokeColor => StyleEdit::StrokeColor(parse_hex(text)?),
            Self::StrokeOpacity => StyleEdit::StrokeOpacity(parse_opacity_percent(text)?),
            Self::FillColor => StyleEdit::FillColor(parse_hex(text)?),
            Self::FillOpacity => StyleEdit::FillOpacity(parse_opacity_percent(text)?),
        })
    }

    /// The edit that sets a colour area's value, `None` for a field that is
    /// not a colour.
    #[must_use]
    pub const fn color_edit(self, color: Color) -> Option<StyleEdit> {
        match self {
            Self::StrokeColor => Some(StyleEdit::StrokeColor(color)),
            Self::FillColor => Some(StyleEdit::FillColor(color)),
            _ => None,
        }
    }

    /// The edit that sets an opacity slider's value (a whole percent), `None`
    /// for a field that is not an opacity.
    #[must_use]
    pub fn opacity_edit(self, percent: f64) -> Option<StyleEdit> {
        match self {
            Self::StrokeOpacity => Some(StyleEdit::StrokeOpacity(opacity_from_percent(percent))),
            Self::FillOpacity => Some(StyleEdit::FillOpacity(opacity_from_percent(percent))),
            _ => None,
        }
    }
}

/// Parses the host's join word.
#[must_use]
pub fn join_from_name(name: &str) -> Option<LineJoin> {
    match name {
        "miter" => Some(LineJoin::Miter),
        "round" => Some(LineJoin::Round),
        "bevel" => Some(LineJoin::Bevel),
        _ => None,
    }
}

/// Parses the host's cap word.
#[must_use]
pub fn cap_from_name(name: &str) -> Option<LineCap> {
    match name {
        "butt" => Some(LineCap::Butt),
        "round" => Some(LineCap::Round),
        "square" => Some(LineCap::Square),
        _ => None,
    }
}

/// Parses the host's fill-mode word.
#[must_use]
pub fn fill_mode_from_name(name: &str) -> Option<FillMode> {
    match name {
        "none" => Some(FillMode::None),
        "solid" => Some(FillMode::Solid),
        "linear" => Some(FillMode::Linear),
        "radial" => Some(FillMode::Radial),
        _ => None,
    }
}
