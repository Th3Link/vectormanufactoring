//! Millimetre-based unit newtypes for the document model (ADR 0002 §2, §3).
//!
//! The canonical unit for the whole document is the millimetre, stored as
//! `f64`. This module defines [`Length`] as a newtype over that value,
//! [`DocumentSize`] as the page size the document root carries, and
//! [`Point`]/[`Vec2`] — an absolute document-space position and a relative
//! offset, respectively (ADR 0002 §3) — with the elementary arithmetic
//! `specs/path-node-editing/adrs.md`'s crate-boundary decision assigns to
//! this crate: add, subtract, scale, negate, normalize, length. None of it
//! evaluates a curve; that is `vecmanf-geometry-core`'s job.

use std::ops::{Add, Sub};

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

/// An explicit tolerance for one geometric comparison, in document
/// millimetres (ADR 0002 §3: "every geometric comparison takes an
/// explicit `Tolerance`. There is no global epsilon").
///
/// `vecmanf-geometry-core`'s flatten/nearest-point/subdivide and
/// `vecmanf-ui-core`'s hit-testing each take one of these explicitly
/// rather than assuming a shared constant —
/// `vecmanf-render-core`'s own, coarser display tolerance (ADR 0003 §7)
/// is a different value entirely and must never be reused for either.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tolerance(Length);

impl Tolerance {
    /// Builds a [`Tolerance`] from a value already in millimetres.
    #[must_use]
    pub const fn from_mm(millimetres: f64) -> Self {
        Self(Length::from_mm(millimetres))
    }

    /// Returns the tolerance as a plain millimetre value, for handing to
    /// e.g. `kurbo`'s own accuracy-as-`f64` parameters.
    #[must_use]
    pub const fn as_mm(self) -> f64 {
        self.0.as_mm()
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

/// An absolute position in document space: millimetres, Y-down (ADR 0002
/// §4).
///
/// Distinct from [`Vec2`] even though both wrap two `f64`s — a point is
/// "where", a vector is "how far and which way" — so a handle (always
/// relative to its anchor, `specs/path-node-editing/adrs.md`) is a `Vec2`
/// and an anchor's own position is a `Point`, and the two can never be
/// added where a subtraction was meant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// The X coordinate, in millimetres.
    pub x: f64,
    /// The Y coordinate, in millimetres, growing downward.
    pub y: f64,
}

impl Point {
    /// Builds a [`Point`] from millimetre coordinates.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// The displacement from `self` to `other`.
    #[must_use]
    pub fn vector_to(self, other: Self) -> Vec2 {
        Vec2::new(other.x - self.x, other.y - self.y)
    }

    /// `self` displaced by `offset`.
    #[must_use]
    pub fn translated(self, offset: Vec2) -> Self {
        Self::new(self.x + offset.x, self.y + offset.y)
    }
}

/// A relative offset in document space: millimetres, Y-down. Used for a
/// node's handle (stored relative to its own anchor,
/// `specs/path-node-editing/adrs.md` decision 2) and for drag deltas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    /// The X component, in millimetres.
    pub x: f64,
    /// The Y component, in millimetres.
    pub y: f64,
}

impl Vec2 {
    /// The zero vector — a retracted handle, i.e. "this segment is a line"
    /// (`specs/path-node-editing/adrs.md` decision 2: a derived property of
    /// the stored value, not a separate flag).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Builds a [`Vec2`] from millimetre components.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Scales every component by `factor`.
    #[must_use]
    pub fn scaled(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// The opposite vector — a smooth node's mirrored handle is exactly
    /// `handle_in = handle_out.negated()` (`adrs.md` decision 1).
    #[must_use]
    pub fn negated(self) -> Self {
        Self::new(-self.x, -self.y)
    }

    /// The Euclidean length, in millimetres.
    #[must_use]
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    /// This vector scaled to `target_length`, or [`Vec2::ZERO`] if it is
    /// itself (numerically) zero-length — there is no direction to scale.
    #[must_use]
    pub fn normalized_to(self, target_length: f64) -> Self {
        let current = self.length();
        if current <= f64::EPSILON {
            Self::ZERO
        } else {
            self.scaled(target_length / current)
        }
    }
}

impl Add for Vec2 {
    type Output = Self;

    /// Component-wise sum.
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    /// Component-wise difference.
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
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
    fn tolerance_round_trips_through_mm() {
        let tolerance = Tolerance::from_mm(0.1);
        assert!((tolerance.as_mm() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn default_document_size_is_a4_portrait() {
        let size = DocumentSize::default();
        assert!((size.width.as_mm() - 210.0).abs() < f64::EPSILON);
        assert!((size.height.as_mm() - 297.0).abs() < f64::EPSILON);
    }

    #[test]
    fn vector_to_is_the_displacement_between_two_points() {
        let a = Point::new(1.0, 1.0);
        let b = Point::new(4.0, 5.0);
        assert_eq!(a.vector_to(b), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn translated_moves_a_point_by_an_offset() {
        let p = Point::new(1.0, 1.0);
        assert_eq!(p.translated(Vec2::new(2.0, -1.0)), Point::new(3.0, 0.0));
    }

    #[test]
    fn negated_mirrors_a_vector() {
        let v = Vec2::new(3.0, -2.0);
        assert_eq!(v.negated(), Vec2::new(-3.0, 2.0));
        // Mirroring twice returns the original (AC 9's round trip).
        assert_eq!(v.negated().negated(), v);
    }

    #[test]
    fn length_is_euclidean() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.length() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normalized_to_scales_along_the_same_direction() {
        let v = Vec2::new(3.0, 4.0);
        let scaled = v.normalized_to(10.0);
        assert!((scaled.length() - 10.0).abs() < 1e-9);
        // Same direction: cross product of the two is ~0.
        let cross = v.x * scaled.y - v.y * scaled.x;
        assert!(cross.abs() < 1e-9);
    }

    #[test]
    fn normalized_to_of_zero_vector_is_zero() {
        assert_eq!(Vec2::ZERO.normalized_to(5.0), Vec2::ZERO);
    }

    #[test]
    fn add_and_sub_are_inverses() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, -1.0);
        assert_eq!(a.add(b).sub(b), a);
    }
}
