//! Millimetre-based unit newtypes for the document model (ADR 0002 §2, §3).
//!
//! The canonical unit for the whole document is the millimetre, stored as
//! `f64`. This module defines [`Length`] as a newtype over that value,
//! [`DocumentSize`] as the page size the document root carries, and
//! [`Point`]/[`Vec2`] — an absolute document-space position and a relative
//! offset, respectively (ADR 0002 §3) — with the elementary arithmetic
//! `specs/0002-path-node-editing/adrs.md`'s crate-boundary decision assigns to
//! this crate: add, subtract, scale, negate, normalize, length. None of it
//! evaluates a curve; that is `curvyo-geometry-core`'s job.

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
/// `curvyo-geometry-core`'s flatten/nearest-point/subdivide and
/// `curvyo-ui-core`'s hit-testing each take one of these explicitly
/// rather than assuming a shared constant —
/// `curvyo-render-core`'s own, coarser display tolerance (ADR 0003 §7)
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

/// An angle in radians (ADR 0002 §3; `specs/0003-primitive-shapes/adrs.md`: "the
/// first use of §3's `Angle`"). A newtype rather than a bare `f64` for the
/// same reason [`Length`] is one (`CLAUDE.md` §5): a polygon/star's
/// rotation can never be confused with a length or a plain scalar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Angle(f64);

impl Angle {
    /// Builds an [`Angle`] from a value already in radians.
    #[must_use]
    pub const fn from_radians(radians: f64) -> Self {
        Self(radians)
    }

    /// Returns the angle as a plain radian value.
    #[must_use]
    pub const fn as_radians(self) -> f64 {
        self.0
    }

    /// This angle wrapped to `(-π, π]` — `object-transform`'s own
    /// "written normalized" rule (`specs/0005-object-transform/adrs.md`:
    /// "rotation... written normalized to (-π, π]"), so a `rotation`
    /// register never drifts to an ever-growing winding count across
    /// repeated rotate drags.
    #[must_use]
    pub fn normalized(self) -> Self {
        let tau = std::f64::consts::TAU;
        // Wrap into [0, tau), then shift the (-tau/2, 0] sliver up to
        // (0, tau] instead so the result lands in (-π, π] rather than
        // [-π, π).
        let wrapped = self.0.rem_euclid(tau);
        if wrapped > std::f64::consts::PI {
            Self(wrapped - tau)
        } else {
            Self(wrapped)
        }
    }
}

/// A document's page size in millimetres (ADR 0002 §2; this slice's minimal
/// root record, `specs/0001-project-file-foundation/adrs.md`).
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
/// relative to its anchor, `specs/0002-path-node-editing/adrs.md`) is a `Vec2`
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

    /// `self` rotated by `angle` about `pivot`, in Y-down document space
    /// (`specs/0005-object-transform/adrs.md`: "points about the pivot").
    /// Identity when `self == pivot`, whatever `angle` is.
    #[must_use]
    pub fn rotated_around(self, pivot: Self, angle: Angle) -> Self {
        pivot.translated(pivot.vector_to(self).rotated(angle))
    }
}

/// A relative offset in document space: millimetres, Y-down. Used for a
/// node's handle (stored relative to its own anchor,
/// `specs/0002-path-node-editing/adrs.md` decision 2) and for drag deltas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    /// The X component, in millimetres.
    pub x: f64,
    /// The Y component, in millimetres.
    pub y: f64,
}

impl Vec2 {
    /// The zero vector — a retracted handle, i.e. "this segment is a line"
    /// (`specs/0002-path-node-editing/adrs.md` decision 2: a derived property of
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

    /// `self` rotated by `angle`, in Y-down document space — a plain
    /// vector rotation, no pivot needed (`specs/0005-object-transform/
    /// adrs.md`: "handle vectors rotate by the angle, needing no pivot
    /// since it is already relative to its anchor").
    #[must_use]
    pub fn rotated(self, angle: Angle) -> Self {
        let (sin, cos) = angle.as_radians().sin_cos();
        Self::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
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
    /// (`specs/0001-project-file-foundation/adrs.md`, feature-local decision).
    fn default() -> Self {
        Self::new(Length::from_mm(210.0), Length::from_mm(297.0))
    }
}

/// Maps between document space (millimetres) and screen space (pixels):
/// pan and uniform zoom (ADR 0011 §3's "the view transform is the affine
/// transform type from `document-core`, passed to both" — `render-core`
/// for screen-space-constant decoration sizing, and, via the host that
/// owns pointer input, for turning a raw pointer event into the document
/// point `curvyo-ui-core`'s tools take).
///
/// Carries no rotation or skew: nothing in `path-node-editing` needs a
/// canvas that rotates, and a per-node affine transform (ADR 0002 §5) is
/// a separate, not-yet-implemented later concern — this is specifically
/// the *view*'s transform, not a node's. A future slice that needs one is
/// free to generalize this type rather than invent a second one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// Screen pixels per document millimetre.
    scale: f64,
    /// The document-space point that currently maps to screen pixel
    /// `(0, 0)`.
    origin: Point,
}

impl ViewTransform {
    /// Builds a [`ViewTransform`] from its zoom factor (screen pixels per
    /// document millimetre) and the document point currently at the
    /// screen's top-left corner.
    #[must_use]
    pub const fn new(scale: f64, origin: Point) -> Self {
        Self { scale, origin }
    }

    /// The identity view: 1 screen pixel per document millimetre, with
    /// the document origin at the screen's top-left corner.
    #[must_use]
    pub const fn identity() -> Self {
        Self::new(1.0, Point::new(0.0, 0.0))
    }

    /// Screen pixels per document millimetre — what a decoration's
    /// screen-space-constant size (`docs/design-system.md`) must be
    /// divided by to get its equivalent size in document millimetres at
    /// the current zoom.
    #[must_use]
    pub const fn scale(self) -> f64 {
        self.scale
    }

    /// Converts a document-space point to screen pixel coordinates.
    #[must_use]
    pub fn document_to_screen(self, point: Point) -> (f64, f64) {
        let offset = self.origin.vector_to(point);
        (offset.x * self.scale, offset.y * self.scale)
    }

    /// Converts screen pixel coordinates to a document-space point —
    /// what the host uses to turn a pointer event into the point
    /// `curvyo-ui-core`'s tools take.
    #[must_use]
    pub fn screen_to_document(self, screen_x: f64, screen_y: f64) -> Point {
        self.origin
            .translated(Vec2::new(screen_x / self.scale, screen_y / self.scale))
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
    fn identity_view_transform_maps_mm_to_pixels_one_to_one() {
        let view = ViewTransform::identity();
        let (x, y) = view.document_to_screen(Point::new(5.0, 7.0));
        assert!((x - 5.0).abs() < f64::EPSILON);
        assert!((y - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn view_transform_scales_and_pans() {
        let view = ViewTransform::new(2.0, Point::new(10.0, 10.0));
        // The origin itself maps to screen (0, 0).
        let (x, y) = view.document_to_screen(Point::new(10.0, 10.0));
        assert!(x.abs() < f64::EPSILON && y.abs() < f64::EPSILON);
        // 1mm further in document space is 2px further on screen at 2x zoom.
        let (x, y) = view.document_to_screen(Point::new(11.0, 10.0));
        assert!((x - 2.0).abs() < f64::EPSILON && y.abs() < f64::EPSILON);
    }

    #[test]
    fn screen_to_document_is_the_inverse_of_document_to_screen() {
        let view = ViewTransform::new(3.0, Point::new(-4.0, 2.0));
        let original = Point::new(12.0, -8.0);
        let (sx, sy) = view.document_to_screen(original);
        let round_tripped = view.screen_to_document(sx, sy);
        assert!((round_tripped.x - original.x).abs() < 1e-9);
        assert!((round_tripped.y - original.y).abs() < 1e-9);
    }

    #[test]
    fn angle_round_trips_through_radians() {
        let angle = Angle::from_radians(1.25);
        assert!((angle.as_radians() - 1.25).abs() < f64::EPSILON);
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

    #[test]
    fn angle_normalized_wraps_into_minus_pi_to_pi_inclusive_upper() {
        let pi = std::f64::consts::PI;
        assert!((Angle::from_radians(pi).normalized().as_radians() - pi).abs() < 1e-9);
        let just_over = Angle::from_radians(pi + 0.1).normalized().as_radians();
        assert!((just_over - (-pi + 0.1)).abs() < 1e-9);
        let two_pi_plus_half = Angle::from_radians(std::f64::consts::TAU + 0.5)
            .normalized()
            .as_radians();
        assert!((two_pi_plus_half - 0.5).abs() < 1e-9);
        let minus_two_pi = Angle::from_radians(-std::f64::consts::TAU)
            .normalized()
            .as_radians();
        assert!(minus_two_pi.abs() < 1e-9);
    }

    #[test]
    fn vec2_rotated_by_quarter_turn_swaps_axes() {
        let v = Vec2::new(1.0, 0.0);
        let rotated = v.rotated(Angle::from_radians(std::f64::consts::FRAC_PI_2));
        assert!((rotated.x - 0.0).abs() < 1e-9);
        assert!((rotated.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn vec2_rotated_preserves_length() {
        let v = Vec2::new(3.0, 4.0);
        let rotated = v.rotated(Angle::from_radians(1.23));
        assert!((rotated.length() - v.length()).abs() < 1e-9);
    }

    #[test]
    fn point_rotated_around_itself_is_identity() {
        let p = Point::new(4.0, 5.0);
        let rotated = p.rotated_around(p, Angle::from_radians(1.0));
        assert_eq!(rotated, p);
    }

    #[test]
    fn point_rotated_around_pivot_by_quarter_turn() {
        let pivot = Point::new(0.0, 0.0);
        let p = Point::new(1.0, 0.0);
        let rotated = p.rotated_around(pivot, Angle::from_radians(std::f64::consts::FRAC_PI_2));
        assert!((rotated.x - 0.0).abs() < 1e-9);
        assert!((rotated.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn point_rotated_around_off_center_pivot() {
        let pivot = Point::new(10.0, 10.0);
        let p = Point::new(11.0, 10.0);
        let rotated = p.rotated_around(pivot, Angle::from_radians(std::f64::consts::FRAC_PI_2));
        assert!((rotated.x - 10.0).abs() < 1e-9);
        assert!((rotated.y - 11.0).abs() < 1e-9);
    }
}
