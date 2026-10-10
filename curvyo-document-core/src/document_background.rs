//! The document's background: a paint (`None` or `Solid`) and a colour with
//! alpha, stored as two registers in the document root
//! (`specs/0040-document-background`, `adrs.md` decisions 1 to 5).
//!
//! The background is a property of the document, not an object: it is not in
//! the objects tree and no object command can see it. [`Document::background`]
//! reads the registers leniently (a merged replica never makes a read panic),
//! [`validate`] is the strict check on open, and [`Document::set_background`]
//! writes only the register that changed.

use loro::{LoroDoc, LoroValue};
use serde::Serialize;

use crate::document::{Document, ROOT_MAP};
use crate::path_codec::as_f64;
use crate::path_model::Color;
use crate::style_model::Opacity;

/// The root register holding the paint: `"none"` or `"solid"`. Absent means
/// `"solid"`.
const KEY_PAINT: &str = "background_paint";
/// The root register holding the colour: one list `[r, g, b, a]`. Absent means
/// [`DocumentBackground::DEFAULT`]'s colour.
const KEY_COLOR: &str = "background_color";

const PAINT_NONE: &str = "none";
const PAINT_SOLID: &str = "solid";

/// Whether the background paints anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundPaint {
    /// No background: the document area shows a checkerboard.
    None,
    /// The background colour, at its alpha.
    Solid,
}

impl BackgroundPaint {
    /// The word stored in the file and sent to the host: `"none"` or `"solid"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => PAINT_NONE,
            Self::Solid => PAINT_SOLID,
        }
    }

    /// The paint with this exact name, or `None` for any other text.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            PAINT_NONE => Some(Self::None),
            PAINT_SOLID => Some(Self::Solid),
            _ => None,
        }
    }
}

/// The document's background. The colour is kept while the paint is
/// [`BackgroundPaint::None`], so pressing Solid again returns it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentBackground {
    /// Whether the colour is painted.
    pub paint: BackgroundPaint,
    /// Red, green and blue, the colour type of an object's fill.
    pub color: Color,
    /// The alpha, as stored: `AA / 255` from a hex field, `N / 100` from the
    /// Opacity field.
    pub opacity: Opacity,
}

impl DocumentBackground {
    /// What a new document and a file without the registers have: solid
    /// `#E8E8EB`, fully opaque (the colour the document area always had).
    pub const DEFAULT: Self = Self {
        paint: BackgroundPaint::Solid,
        color: Color {
            r: 0xE8,
            g: 0xE8,
            b: 0xEB,
        },
        opacity: Opacity::OPAQUE,
    };
}

/// The `background` member of `document.json`.
#[derive(Serialize)]
pub(crate) struct BackgroundJson {
    paint: BackgroundPaint,
    color: (u8, u8, u8, f64),
}

impl From<DocumentBackground> for BackgroundJson {
    fn from(background: DocumentBackground) -> Self {
        Self {
            paint: background.paint,
            color: (
                background.color.r,
                background.color.g,
                background.color.b,
                background.opacity.get(),
            ),
        }
    }
}

/// The colour a well-formed `background_color` value holds, or `None`: a list
/// of exactly four entries, three whole numbers from 0 to 255 and one finite
/// number from 0 to 1.
fn colour_of(value: &LoroValue) -> Option<(Color, Opacity)> {
    let LoroValue::List(list) = value else {
        return None;
    };
    let [r, g, b, alpha] = list.as_slice() else {
        return None;
    };
    let channel = |value: &LoroValue| match value {
        LoroValue::I64(n) => u8::try_from(*n).ok(),
        _ => None,
    };
    let opacity = Opacity::new(as_f64(alpha)?).ok()?;
    Some((
        Color {
            r: channel(r)?,
            g: channel(g)?,
            b: channel(b)?,
        },
        opacity,
    ))
}

fn paint_of(value: &LoroValue) -> Option<BackgroundPaint> {
    match value {
        LoroValue::String(name) => BackgroundPaint::from_name(name),
        _ => None,
    }
}

/// Strict validation on open (criterion 7): every register that is present
/// must be well formed. An absent register is valid.
pub(crate) fn validate(loro: &LoroDoc) -> bool {
    let root = loro.get_map(ROOT_MAP);
    let present = |key: &str| root.get(key).map(|value| value.get_deep_value());
    present(KEY_PAINT).is_none_or(|value| paint_of(&value).is_some())
        && present(KEY_COLOR).is_none_or(|value| colour_of(&value).is_some())
}

fn colour_value(color: Color, opacity: Opacity) -> LoroValue {
    LoroValue::List(
        vec![
            LoroValue::I64(i64::from(color.r)),
            LoroValue::I64(i64::from(color.g)),
            LoroValue::I64(i64::from(color.b)),
            LoroValue::Double(opacity.get()),
        ]
        .into(),
    )
}

impl Document {
    /// The document's background. An absent register reads as its default; a
    /// malformed one (only a merged, never validated replica can hold one)
    /// does too, and the read writes nothing.
    #[must_use]
    pub fn background(&self) -> DocumentBackground {
        let root = self.loro().get_map(ROOT_MAP);
        let read = |key: &str| root.get(key).map(|value| value.get_deep_value());
        let default = DocumentBackground::DEFAULT;
        let paint = read(KEY_PAINT)
            .and_then(|value| paint_of(&value))
            .unwrap_or(default.paint);
        let (color, opacity) = read(KEY_COLOR)
            .and_then(|value| colour_of(&value))
            .unwrap_or((default.color, default.opacity));
        DocumentBackground {
            paint,
            color,
            opacity,
        }
    }

    /// Sets the background as one commit labelled `set_document_background`
    /// that writes only the register that differs from the stored value
    /// (`Color` and the alpha compared exactly). Returns `false` and writes
    /// nothing if the value equals the stored one.
    ///
    /// # Panics
    /// Does not panic in practice: it inserts plain values into the attached
    /// root map.
    #[must_use]
    pub fn set_background(&self, value: DocumentBackground) -> bool {
        let stored = self.background();
        let paint_changed = value.paint != stored.paint;
        let colour_changed = value.color != stored.color || value.opacity != stored.opacity;
        if !paint_changed && !colour_changed {
            return false;
        }
        let root = self.loro().get_map(ROOT_MAP);
        // invariant: the root map is attached and the values are plain.
        #[allow(clippy::unwrap_used)]
        {
            if paint_changed {
                root.insert(KEY_PAINT, value.paint.name()).unwrap();
            }
            if colour_changed {
                root.insert(KEY_COLOR, colour_value(value.color, value.opacity))
                    .unwrap();
            }
        }
        self.commit_with_label("set_document_background");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The read is lenient: a malformed value in a merged replica reads as that
    /// register's default and nothing is written (`adrs.md` decision 4).
    #[test]
    fn a_malformed_register_reads_as_its_default_and_writes_nothing() {
        let document = Document::new(1);
        let root = document.loro().get_map(ROOT_MAP);
        root.insert(KEY_PAINT, "checker").unwrap();
        root.insert(KEY_COLOR, vec![1_i64]).unwrap();
        document.loro().commit();
        let before = document.loro().oplog_vv();
        assert_eq!(document.background(), DocumentBackground::DEFAULT);
        assert_eq!(document.loro().oplog_vv(), before);
    }

    /// The built-in default is the colour of `--canvas-bg` (`docs/design-system.md`).
    #[test]
    fn the_default_is_the_canvas_colour() {
        let default = DocumentBackground::DEFAULT;
        assert_eq!(
            (default.color.r, default.color.g, default.color.b),
            (0xE8, 0xE8, 0xEB)
        );
        assert_eq!(default.opacity.get(), 1.0);
        assert_eq!(default.paint, BackgroundPaint::Solid);
    }

    #[test]
    fn paint_names_round_trip() {
        for paint in [BackgroundPaint::None, BackgroundPaint::Solid] {
            assert_eq!(BackgroundPaint::from_name(paint.name()), Some(paint));
        }
        assert_eq!(BackgroundPaint::from_name("Solid"), None);
    }
}
