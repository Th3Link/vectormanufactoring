//! Millimetre-based unit newtypes for the document model (ADR 0002 §2, §3).
//!
//! The canonical unit for the whole document is the millimetre, stored as
//! `f64`. This module defines [`Length`] as a newtype over that value and
//! [`DocumentSize`] as the page size this slice's document root carries
//! (`specs/project-file-foundation/adrs.md`, "a minimal document root
//! record, and no more").

use serde::{Deserialize, Serialize};

/// A length in millimetres, the canonical unit for document-space
/// coordinates and sizes (ADR 0002 §2).
///
/// This is a newtype rather than a bare `f64` so that a length can never be
/// confused with an angle, a speed or any other quantity (`CLAUDE.md` §5,
/// "units are types").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Length(f64);

impl Length {
    /// Builds a [`Length`] from a value already in millimetres.
    #[must_use]
    pub const fn from_mm(millimetres: f64) -> Self {
        Self(millimetres)
    }

    /// Returns the length as a plain millimetre value.
    #[must_use]
    pub const fn as_mm(self) -> f64 {
        self.0
    }
}

/// A document's page size in millimetres (ADR 0002 §2; this slice's minimal
/// root record, `specs/project-file-foundation/adrs.md`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DocumentSize {
    /// The page width.
    pub width: Length,
    /// The page height.
    pub height: Length,
}

impl DocumentSize {
    /// Builds a [`DocumentSize`] from an explicit width and height.
    #[must_use]
    pub const fn new(width: Length, height: Length) -> Self {
        Self { width, height }
    }
}

impl Default for DocumentSize {
    /// A4 portrait, 210 × 297 mm — the default when a document has never
    /// had its size set explicitly
    /// (`specs/project-file-foundation/adrs.md`, feature-local decision).
    fn default() -> Self {
        Self::new(Length::from_mm(210.0), Length::from_mm(297.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_round_trips_through_mm() {
        let length = Length::from_mm(42.5);
        assert!((length.as_mm() - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn default_document_size_is_a4_portrait() {
        let size = DocumentSize::default();
        assert!((size.width.as_mm() - 210.0).abs() < f64::EPSILON);
        assert!((size.height.as_mm() - 297.0).abs() < f64::EPSILON);
    }
}
