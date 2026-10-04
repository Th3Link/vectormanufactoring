//! A flat, alpha-capable color for decoration and stroke geometry.
//!
//! Distinct from [`vecmanf_document_core::Color`]: that type is this
//! slice's one placeholder *document* stroke color (acceptance criterion
//! 6, always opaque black) — editing-UI decorations need alpha (the
//! hover ring's "faint outer ring", `specs/0002-path-node-editing/
//! specification.md`'s UX notes) and theme colors document-core has no
//! reason to know about.

/// An RGBA color, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaColor {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; `255` is fully opaque.
    pub a: u8,
}

impl RgbaColor {
    /// A fully opaque color from its RGB channels.
    #[must_use]
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// This color with a different alpha channel.
    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Solid black — this slice's one stroke color (acceptance criterion
    /// 6).
    pub const BLACK: Self = Self::opaque(0, 0, 0);

    /// White — the idle fill of an unselected node or handle glyph
    /// (`specs/0002-path-node-editing/specification.md`'s UX notes).
    pub const WHITE: Self = Self::opaque(255, 255, 255);
}

impl From<vecmanf_document_core::Color> for RgbaColor {
    fn from(color: vecmanf_document_core::Color) -> Self {
        Self::opaque(color.r, color.g, color.b)
    }
}
