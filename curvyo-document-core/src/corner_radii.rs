//! A rectangle's four corner radii and how they are evaluated against its size
//! (`specs/rectangle-corner-radii/adrs.md`, decisions 1 and 5): pure data and
//! one clamping rule, no Loro.
//!
//! Radii are stored raw, one per corner, and clamped only here, where they are
//! evaluated, by the CSS `border-radius` rule: when two radii on one side would
//! overlap, all four shrink by the same factor.

use serde::{Deserialize, Serialize};

use crate::primitive_model::RectBounds;
use crate::units::{Length, Vec2};

/// An effective radius of at most this many millimetres is a sharp corner
/// (`specs/rectangle-corner-radii/specification.md`, criterion 11).
pub const SHARP_CORNER_EPSILON_MM: f64 = 1e-9;

/// The corner of a rectangle a radius belongs to, in the rectangle's own
/// (possibly rotated) local frame, in outline order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// Local minimum x and y.
    Tl,
    /// Local maximum x, minimum y.
    Tr,
    /// Local maximum x and y.
    Br,
    /// Local minimum x, maximum y.
    Bl,
}

impl Corner {
    /// Every corner, in outline order: top-left, top-right, bottom-right,
    /// bottom-left.
    pub const ALL: [Self; 4] = [Self::Tl, Self::Tr, Self::Br, Self::Bl];

    /// The signs `(x, y)` of the corner's inward diagonal in the local frame.
    const fn inward(self) -> (f64, f64) {
        match self {
            Self::Tl => (1.0, 1.0),
            Self::Tr => (-1.0, 1.0),
            Self::Br => (-1.0, -1.0),
            Self::Bl => (1.0, -1.0),
        }
    }

    /// The corner on the other end of the box's diagonal (top-left and
    /// bottom-right, top-right and bottom-left).
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Tl => Self::Br,
            Self::Tr => Self::Bl,
            Self::Br => Self::Tl,
            Self::Bl => Self::Tr,
        }
    }

    /// The corner that shares this corner's horizontal side (top-left and
    /// top-right, bottom-right and bottom-left): its radius and this one's
    /// add up along the box's width.
    #[must_use]
    pub const fn horizontal_neighbour(self) -> Self {
        match self {
            Self::Tl => Self::Tr,
            Self::Tr => Self::Tl,
            Self::Br => Self::Bl,
            Self::Bl => Self::Br,
        }
    }

    /// The corner that shares this corner's vertical side (top-left and
    /// bottom-left, top-right and bottom-right): its radius and this one's
    /// add up along the box's height.
    #[must_use]
    pub const fn vertical_neighbour(self) -> Self {
        match self {
            Self::Tl => Self::Bl,
            Self::Bl => Self::Tl,
            Self::Tr => Self::Br,
            Self::Br => Self::Tr,
        }
    }

    /// The unit vector along the corner's inward diagonal, in the local frame.
    #[must_use]
    pub fn inward_diagonal(self) -> Vec2 {
        let (x, y) = self.inward();
        Vec2::new(x, y).normalized_to(1.0)
    }
}

/// The four stored corner radii of a rectangle: one circular radius per
/// corner, stored as entered (not clamped).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CornerRadii {
    /// Top-left.
    pub tl: Length,
    /// Top-right.
    pub tr: Length,
    /// Bottom-right.
    pub br: Length,
    /// Bottom-left.
    pub bl: Length,
}

impl CornerRadii {
    /// The same radius at all four corners.
    #[must_use]
    pub const fn uniform(radius: Length) -> Self {
        Self {
            tl: radius,
            tr: radius,
            br: radius,
            bl: radius,
        }
    }

    /// The radius at `corner`.
    #[must_use]
    pub const fn get(self, corner: Corner) -> Length {
        match corner {
            Corner::Tl => self.tl,
            Corner::Tr => self.tr,
            Corner::Br => self.br,
            Corner::Bl => self.bl,
        }
    }

    /// These radii with the radius at `corner` replaced by `radius`.
    #[must_use]
    pub const fn with(self, corner: Corner, radius: Length) -> Self {
        let mut radii = self;
        match corner {
            Corner::Tl => radii.tl = radius,
            Corner::Tr => radii.tr = radius,
            Corner::Br => radii.br = radius,
            Corner::Bl => radii.bl = radius,
        }
        radii
    }
}

/// A rectangle's effective corner radii: the stored radii evaluated against
/// `bounds` (acceptance criterion 9). Every consumer (outline, rendering,
/// hit-testing, handle placement, conversion) calls this rather than reading
/// the stored radii directly.
///
/// With `f = min(1, W/(tl+tr), W/(bl+br), H/(tl+bl), H/(tr+br))`, a term being
/// skipped when its denominator is 0, the result is `f` times each radius. When
/// `f` is 1 the stored radii come back unchanged (no multiplication), so a
/// rectangle within its limits is evaluated bit for bit as stored. A negative
/// radius (which no writer produces and open refuses) counts as 0. With four
/// equal radii `r` this is `min(r, W/2, H/2)`.
#[must_use]
pub fn effective_corner_radii(bounds: RectBounds, radii: CornerRadii) -> CornerRadii {
    let floored = CornerRadii {
        tl: floor_at_zero(radii.tl),
        tr: floor_at_zero(radii.tr),
        br: floor_at_zero(radii.br),
        bl: floor_at_zero(radii.bl),
    };
    let (tl, tr, br, bl) = (
        floored.tl.as_mm(),
        floored.tr.as_mm(),
        floored.br.as_mm(),
        floored.bl.as_mm(),
    );
    let width = bounds.width.as_mm();
    let height = bounds.height.as_mm();
    let factor = [
        (tl + tr, width),
        (bl + br, width),
        (tl + bl, height),
        (tr + br, height),
    ]
    .into_iter()
    .filter(|&(sum, _)| sum > 0.0)
    .fold(1.0_f64, |f, (sum, side)| f.min(side.max(0.0) / sum));
    if factor >= 1.0 {
        return floored;
    }
    CornerRadii {
        tl: Length::from_mm(factor * tl),
        tr: Length::from_mm(factor * tr),
        br: Length::from_mm(factor * br),
        bl: Length::from_mm(factor * bl),
    }
}

fn floor_at_zero(radius: Length) -> Length {
    if radius.as_mm() < 0.0 {
        Length::from_mm(0.0)
    } else {
        radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Point;

    fn bounds(width: f64, height: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(3.0, 4.0),
            width: Length::from_mm(width),
            height: Length::from_mm(height),
        }
    }

    fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
        CornerRadii {
            tl: Length::from_mm(tl),
            tr: Length::from_mm(tr),
            br: Length::from_mm(br),
            bl: Length::from_mm(bl),
        }
    }

    fn mm(radii: CornerRadii) -> [f64; 4] {
        [
            radii.tl.as_mm(),
            radii.tr.as_mm(),
            radii.br.as_mm(),
            radii.bl.as_mm(),
        ]
    }

    fn assert_close(actual: [f64; 4], expected: [f64; 4]) {
        for (a, e) in actual.into_iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{actual:?} vs {expected:?}");
        }
    }

    #[test]
    fn corners_are_in_outline_order() {
        assert_eq!(
            Corner::ALL,
            [Corner::Tl, Corner::Tr, Corner::Br, Corner::Bl]
        );
    }

    #[test]
    fn inward_diagonals_are_unit_vectors_pointing_into_the_box() {
        let expected = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        for (corner, (sx, sy)) in Corner::ALL.into_iter().zip(expected) {
            let d = corner.inward_diagonal();
            assert!((d.length() - 1.0).abs() < 1e-12);
            assert!(d.x * sx > 0.0 && d.y * sy > 0.0);
        }
    }

    #[test]
    fn opposite_and_neighbours_pair_the_corners_of_the_box() {
        for corner in Corner::ALL {
            assert_eq!(corner.opposite().opposite(), corner);
            assert_eq!(corner.horizontal_neighbour().horizontal_neighbour(), corner);
            assert_eq!(corner.vertical_neighbour().vertical_neighbour(), corner);
            // The three others are the opposite and the two neighbours.
            let mut others = [
                corner.opposite(),
                corner.horizontal_neighbour(),
                corner.vertical_neighbour(),
            ];
            others.sort_by_key(|c| Corner::ALL.iter().position(|a| a == c));
            let expected: Vec<Corner> = Corner::ALL.into_iter().filter(|c| *c != corner).collect();
            assert_eq!(others.to_vec(), expected);
        }
        assert_eq!(Corner::Tl.horizontal_neighbour(), Corner::Tr);
        assert_eq!(Corner::Tl.vertical_neighbour(), Corner::Bl);
        assert_eq!(Corner::Br.opposite(), Corner::Tl);
    }

    #[test]
    fn get_and_with_address_one_corner() {
        let r = radii(1.0, 2.0, 3.0, 4.0);
        let got: Vec<f64> = Corner::ALL.iter().map(|&c| r.get(c).as_mm()).collect();
        assert_eq!(got, [1.0, 2.0, 3.0, 4.0]);
        let changed = r.with(Corner::Br, Length::from_mm(9.0));
        assert_eq!(mm(changed), [1.0, 2.0, 9.0, 4.0]);
        assert_eq!(mm(r), [1.0, 2.0, 3.0, 4.0], "with does not mutate");
    }

    #[test]
    fn uniform_sets_all_four() {
        assert_eq!(
            mm(CornerRadii::uniform(Length::from_mm(2.5))),
            [2.5, 2.5, 2.5, 2.5]
        );
    }

    #[test]
    fn radii_within_the_limits_are_returned_unchanged() {
        let stored = radii(30.0, 30.0, 0.0, 0.0);
        let effective = effective_corner_radii(bounds(100.0, 40.0), stored);
        assert_eq!(effective, stored, "criterion 9, example 1: f = 1");
    }

    #[test]
    fn neighbours_shrink_by_one_shared_factor() {
        // Criterion 9, example 2: TL 30, BL 30, TR 30, BR 0 on 100 x 40.
        let effective = effective_corner_radii(bounds(100.0, 40.0), radii(30.0, 30.0, 0.0, 30.0));
        assert_close(mm(effective), [20.0, 20.0, 0.0, 20.0]);
    }

    #[test]
    fn four_equal_radii_clamp_to_half_the_shorter_side() {
        for (w, h, r, expected) in [
            (100.0, 40.0, 5.0, 5.0),
            (100.0, 40.0, 20.0, 20.0),
            (100.0, 40.0, 100.0, 20.0),
            (40.0, 100.0, 100.0, 20.0),
            (10.0, 10.0, 5.0, 5.0),
            (10.0, 10.0, 0.0, 0.0),
        ] {
            let effective = effective_corner_radii(bounds(w, h), radii(r, r, r, r));
            assert_close(mm(effective), [expected; 4]);
        }
    }

    #[test]
    fn a_zero_denominator_term_is_skipped() {
        let effective = effective_corner_radii(bounds(10.0, 10.0), radii(0.0, 0.0, 0.0, 0.0));
        assert_eq!(mm(effective), [0.0; 4]);
        // One radius larger than the side, its partners 0: only its own sums count.
        let effective = effective_corner_radii(bounds(10.0, 10.0), radii(25.0, 0.0, 0.0, 0.0));
        assert_close(mm(effective), [10.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_degenerate_box_has_no_radius() {
        let effective = effective_corner_radii(bounds(0.0, 40.0), radii(5.0, 5.0, 5.0, 5.0));
        assert_eq!(mm(effective), [0.0; 4]);
    }

    #[test]
    fn a_negative_radius_counts_as_zero() {
        let effective = effective_corner_radii(bounds(100.0, 40.0), radii(-3.0, 5.0, 5.0, 5.0));
        assert_eq!(mm(effective), [0.0, 5.0, 5.0, 5.0]);
    }

    #[test]
    fn evaluation_never_writes_back_so_enlarging_restores_the_stored_radii() {
        let stored = radii(30.0, 30.0, 30.0, 30.0);
        let shrunk = effective_corner_radii(bounds(40.0, 20.0), stored);
        assert_close(mm(shrunk), [10.0; 4]);
        let regrown = effective_corner_radii(bounds(100.0, 100.0), stored);
        assert_eq!(regrown, stored);
    }

    /// A small deterministic generator, so the property test needs no
    /// dependency.
    struct Lcg(u64);

    impl Lcg {
        fn next_unit(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            // Top 53 bits as a fraction in [0, 1); the cast is exact.
            #[allow(clippy::cast_precision_loss)]
            {
                (self.0 >> 11) as f64 / (1u64 << 53) as f64
            }
        }
    }

    #[test]
    fn effective_radii_never_overlap_and_keep_their_ratios() {
        let mut rng = Lcg(7);
        for _ in 0..2000 {
            let (w, h) = (rng.next_unit() * 200.0, rng.next_unit() * 200.0);
            let stored = radii(
                rng.next_unit() * 150.0,
                rng.next_unit() * 150.0,
                rng.next_unit() * 150.0,
                rng.next_unit() * 150.0,
            );
            let e = effective_corner_radii(bounds(w, h), stored);
            let tol = 1e-9;
            assert!(e.tl.as_mm() + e.tr.as_mm() <= w + tol);
            assert!(e.bl.as_mm() + e.br.as_mm() <= w + tol);
            assert!(e.tl.as_mm() + e.bl.as_mm() <= h + tol);
            assert!(e.tr.as_mm() + e.br.as_mm() <= h + tol);
            // One shared factor: every ratio effective/stored is the same.
            let ratios: Vec<f64> = mm(e)
                .into_iter()
                .zip(mm(stored))
                .filter(|&(_, s)| s > 0.0)
                .map(|(eff, s)| eff / s)
                .collect();
            for pair in ratios.windows(2) {
                assert!((pair[0] - pair[1]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn four_equal_radii_match_the_single_radius_clamp_of_slice_3() {
        let mut rng = Lcg(11);
        for _ in 0..2000 {
            let (w, h) = (rng.next_unit() * 200.0, rng.next_unit() * 200.0);
            let r = rng.next_unit() * 150.0;
            let old = r.clamp(0.0, (w.min(h) / 2.0).max(0.0));
            let e = effective_corner_radii(bounds(w, h), radii(r, r, r, r));
            assert_close(mm(e), [old; 4]);
        }
    }
}
