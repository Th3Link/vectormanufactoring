//! How the host's input to the Style panel (typed text and choice words)
//! becomes the style edit it commits (`specs/0007-stroke-and-fill-styling`
//! criteria 5, 6, 13, 14).

use curvyo_document_core::{Color, Length, LineCap, LineJoin, Opacity, StyleEdit};

use crate::transform_entry::parse_entry_number;

/// The widest stroke the field accepts, millimetres.
pub const MAX_STROKE_WIDTH_MM: f64 = 1000.0;

/// Why the text of a style field was refused. The field keeps focus, selects
/// its text and shows the matching message; nothing is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleEntryError {
    /// Not 3, 4, 6 or 8 hex digits ("Enter 3, 4, 6 or 8 hex digits").
    Hex,
    /// Not a number from 0 to 100 ("Enter a number from 0 to 100").
    Percent,
    /// Not a number from 0 to 1000 ("Enter a number from 0 to 1000").
    Width,
    /// Not 1 to 16 numbers from 0 to 1000 with a sum above 0, separated by
    /// spaces ("Enter 1 to 16 numbers from 0 to 1000, for example 1 2 4 2").
    Dash,
}

impl StyleEntryError {
    /// A short stable code for the host, which owns the message text.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Percent => "percent",
            Self::Width => "width",
            Self::Dash => "dash",
        }
    }
}

/// A colour typed as hex: the RGB, and the alpha when the form carried one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexColour {
    /// The red, green and blue.
    pub color: Color,
    /// The alpha as `AA / 255` for the 4 and 8 digit forms, `None` for the 3
    /// and 6 digit forms, which leave the alpha as it is.
    pub opacity: Option<Opacity>,
}

/// Parses a hex colour: 3 (`F80`), 4 (`F80C`), 6 (`FF8800`) or 8 (`FF8800CC`)
/// digits, with or without a leading `#`, in any case, surrounding spaces
/// ignored. A 3 or 4 digit form doubles each digit.
///
/// # Errors
/// [`StyleEntryError::Hex`] for any other length or a character that is not a
/// hex digit.
pub fn parse_hex(text: &str) -> Result<HexColour, StyleEntryError> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(StyleEntryError::Hex);
    }
    let value = |hex: &str| u8::from_str_radix(hex, 16).map_err(|_| StyleEntryError::Hex);
    let doubled = |n: usize| value(&digits[n..=n].repeat(2));
    let pair = |n: usize| value(&digits[n..n + 2]);
    let (r, g, b, alpha) = match digits.len() {
        3 => (doubled(0)?, doubled(1)?, doubled(2)?, None),
        4 => (doubled(0)?, doubled(1)?, doubled(2)?, Some(doubled(3)?)),
        6 => (pair(0)?, pair(2)?, pair(4)?, None),
        8 => (pair(0)?, pair(2)?, pair(4)?, Some(pair(6)?)),
        _ => return Err(StyleEntryError::Hex),
    };
    Ok(HexColour {
        color: Color { r, g, b },
        opacity: alpha.map(opacity_from_byte),
    })
}

/// The alpha byte `AA` as the fraction `AA / 255` it is stored as.
fn opacity_from_byte(alpha: u8) -> Opacity {
    // `alpha / 255` is within 0..=1, a valid opacity.
    Opacity::new(f64::from(alpha) / 255.0).unwrap_or(Opacity::OPAQUE)
}

/// A colour with its alpha as the field shows it: `#RRGGBBAA`, upper case,
/// `AA = round(255 x alpha)` with .5 rounded up.
#[must_use]
pub fn hex_text(color: Color, opacity: Opacity) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let alpha = (opacity.get() * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}{alpha:02X}", color.r, color.g, color.b)
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
            Self::StrokeColor => hex_edit(
                parse_hex(text)?,
                StyleEdit::StrokeColor,
                StyleEdit::StrokeRgba,
            ),
            Self::StrokeOpacity => StyleEdit::StrokeOpacity(parse_opacity_percent(text)?),
            Self::FillColor => {
                hex_edit(parse_hex(text)?, StyleEdit::FillColor, StyleEdit::FillRgba)
            }
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

/// The edit a typed hex makes: the colour alone for the 3 and 6 digit forms,
/// the colour and the alpha for the 4 and 8 digit forms.
fn hex_edit(
    typed: HexColour,
    colour: fn(Color) -> StyleEdit,
    rgba: fn(Color, Opacity) -> StyleEdit,
) -> StyleEdit {
    match typed.opacity {
        None => colour(typed.color),
        Some(opacity) => rgba(typed.color, opacity),
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
