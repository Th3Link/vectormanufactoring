//! Where a selected primitive's parameter handles sit, and which handle sets
//! a box of a given on-screen size draws
//! (`specs/unified-object-editing/specification.md`, criteria 1, 2, 4, 6-8
//! and the UX notes, section 1; `adrs.md`, second-pass note).
//!
//! A parameter handle changes a primitive's own parameter: the four corner
//! radius handles of a rectangle (each sets its own radius, or all four when
//! the corners are linked) and a star's
//! inner-radius handle. They are laid out in the primitive's local frame and
//! mapped to document space through its [`OrientedBox`], like every other
//! handle; `specs/ellipse-arcs-and-shaping/` adds its own variants here.

pub use curvyo_document_core::Corner;
use curvyo_document_core::{
    CornerRadii, InnerRatio, ObjectSnapshot, Point, PointCount, Shape, StarFrame, Vec2,
    effective_corner_radii,
};

use crate::oriented_box::OrientedBox;
use crate::transform_handle_layout::{TransformHandleTolerances, at_least};

/// A parameter handle's glyph, the "knob": a circle of this diameter
/// (`docs/design-system.md`, "Parameter handle"), screen pixels.
pub const KNOB_DIAMETER_PX: f64 = 10.0;

/// The empty gap two drawn glyphs keep between them, screen pixels
/// (criterion 8).
pub(crate) const MIN_GLYPH_GAP_PX: f64 = 4.0;

/// The centre distance of two radius handles at the largest radius:
/// the knob diameter plus the gap, screen pixels.
pub const KNOB_PITCH_PX: f64 = KNOB_DIAMETER_PX + MIN_GLYPH_GAP_PX;

/// A radius handle's distance from its corner at radius 0, along the
/// corner's inward diagonal, screen pixels. Leaves a 4.3 px gap to the
/// corner resize glyph.
pub const KNOB_INSET_PX: f64 = 15.0;

/// A box's shorter side from which the parameter handles are drawn: the
/// parameter threshold `T` of criterion 7, screen pixels.
pub const PARAM_MIN_SIDE_PX: f64 = 72.0;

/// The hit radius of a parameter handle, screen pixels (criterion 5).
pub const PARAM_HIT_PX: f64 = 12.0;

/// The centre handle is not drawn while a parameter handle centre is within
/// this distance of the box centre, screen pixels (criterion 8).
pub(crate) const CENTRE_YIELD_PX: f64 = 20.0;

/// The corner's position in `box_`'s local frame.
#[must_use]
pub fn corner_local_position(corner: Corner, box_: &OrientedBox) -> Point {
    match corner {
        Corner::Tl => Point::new(box_.min.x, box_.min.y),
        Corner::Tr => Point::new(box_.max.x, box_.min.y),
        Corner::Br => Point::new(box_.max.x, box_.max.y),
        Corner::Bl => Point::new(box_.min.x, box_.max.y),
    }
}

/// One parameter handle: it changes a primitive's own parameter. The four
/// corner radius handles are four hit targets, one per corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamHandle {
    /// A rectangle's corner radius handle at `Corner`.
    CornerRadius(Corner),
    /// A star's inner-radius handle at its first inner vertex.
    InnerRadius,
}

/// Which handle families a box of a given shorter side draws (criterion 7).
#[allow(clippy::struct_excessive_bools)] // one flag per tier, read by name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleTiers {
    /// Corner resize and corner rotate handles: always.
    pub corner: bool,
    /// Edge resize handles: from 24 px.
    pub edge: bool,
    /// The centre move handle: from 48 px.
    pub centre: bool,
    /// The parameter handles: from 72 px.
    pub param: bool,
}

/// The tiers of a box whose shorter side is `shorter_side_mm` (document
/// millimetres), with the shared rounding slack so a box of exactly 24,
/// 48 or 72 px keeps its tier under `px / scale * scale` rounding. There is
/// no hysteresis.
#[must_use]
pub fn handle_tiers(shorter_side_mm: f64, tolerances: &TransformHandleTolerances) -> HandleTiers {
    HandleTiers {
        corner: true,
        edge: at_least(shorter_side_mm, tolerances.edge_handle_min_side_mm),
        centre: at_least(shorter_side_mm, tolerances.center_min_side_mm),
        param: at_least(shorter_side_mm, tolerances.param_min_side_mm),
    }
}

/// `L(s) = (s - 14)/√2 - 15`: how far a radius handle travels along its
/// diagonal between radius 0 and the largest radius, for a box whose shorter
/// side is `shorter_side_mm`. Positive from `s` of about 35 px, so from the
/// 72 px threshold up.
#[must_use]
pub fn radius_travel(shorter_side_mm: f64, tolerances: &TransformHandleTolerances) -> f64 {
    (shorter_side_mm - tolerances.param_pitch_mm) / std::f64::consts::SQRT_2
        - tolerances.param_inset_mm
}

/// `G(s) = (s/2) / L(s)`: how many millimetres of radius one millimetre of
/// pointer travel along the diagonal buys, which keeps the handle under the
/// pointer (1.38 at 72 px, 1.09 at 100, 0.86 at 200, 0.71 for a large box).
/// `1` for a box too small to have a positive travel.
#[must_use]
pub fn radius_gain(shorter_side_mm: f64, tolerances: &TransformHandleTolerances) -> f64 {
    let travel = radius_travel(shorter_side_mm, tolerances);
    if travel > 0.0 {
        shorter_side_mm / 2.0 / travel
    } else {
        1.0
    }
}

/// Where a corner's knob is drawn, as a fraction `ρ` of the travel (`ρ = 1` at
/// half the shorter side, up to 2 for a corner that alone takes the whole
/// shorter side): its own effective radius over `s/2`, except that a knob above
/// half the shorter side whose diagonal partner is also large is drawn nearer
/// its corner, `ρ' = min(ρ, max(1, Σ − ρ_partner))` with
/// `Σ = 2 + √2·(S − s)/L(s)` (`s` and `S` the shorter and the longer side of
/// the box, `L` the travel, [`radius_travel`]), so that two diagonal knobs
/// never overlap (`specs/rectangle-corner-radii/` criterion 1). Only the drawn
/// position changes, never a radius: a knob at or below `ρ = 1` is always at its
/// own radius, and one corner alone can still reach 2. On a square `Σ = 2`.
#[must_use]
pub fn knob_rho(
    effective: CornerRadii,
    shorter_side_mm: f64,
    longer_side_mm: f64,
    corner: Corner,
    tolerances: &TransformHandleTolerances,
) -> f64 {
    let half = shorter_side_mm / 2.0;
    let rho = |corner: Corner| {
        if half > 0.0 {
            effective.get(corner).as_mm() / half
        } else {
            0.0
        }
    };
    let travel = radius_travel(shorter_side_mm, tolerances);
    let sigma = if travel > 0.0 {
        2.0 + std::f64::consts::SQRT_2 * (longer_side_mm - shorter_side_mm) / travel
    } else {
        2.0
    };
    rho(corner).min((sigma - rho(corner.opposite())).max(1.0))
}

/// The star's first inner vertex in its box-local frame, where the first outer
/// vertex is at angle 0 (the box is turned by `StarFrame.angle`, so handles
/// never add it): at `π/N` from the first outer vertex, `ratio` of the way to
/// the outer radius.
#[must_use]
pub(crate) fn star_inner_vertex(
    frame: StarFrame,
    point_count: PointCount,
    ratio: InnerRatio,
) -> Point {
    let step = std::f64::consts::TAU / f64::from(point_count.get());
    let theta = step / 2.0;
    let radius = frame.radius.as_mm() * ratio.get();
    frame
        .center
        .translated(Vec2::new(radius * theta.cos(), radius * theta.sin()))
}

/// Every parameter handle `object` draws, in document space: empty below the
/// parameter threshold of its box, for a path, an ellipse and a polygon.
/// A handle that is not returned is not drawn and has no hit area
/// (criterion 6).
#[must_use]
pub fn param_handles(
    object: &ObjectSnapshot,
    box_: &OrientedBox,
    tolerances: &TransformHandleTolerances,
) -> Vec<(ParamHandle, Point)> {
    let ObjectSnapshot::Primitive(primitive) = object else {
        return Vec::new();
    };
    let shorter = box_.width().min(box_.height());
    if !handle_tiers(shorter, tolerances).param {
        return Vec::new();
    }
    match primitive.shape {
        Shape::Rect {
            bounds,
            corner_radii,
        } => {
            let effective = effective_corner_radii(bounds, corner_radii);
            let longer = box_.width().max(box_.height());
            let travel = radius_travel(shorter, tolerances);
            Corner::ALL
                .iter()
                .map(|&corner| {
                    let rho = knob_rho(effective, shorter, longer, corner, tolerances);
                    let distance = tolerances.param_inset_mm + rho * travel;
                    let at = corner_local_position(corner, box_)
                        .translated(corner.inward_diagonal().scaled(distance));
                    (ParamHandle::CornerRadius(corner), box_.to_document(at))
                })
                .collect()
        }
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => vec![(
            ParamHandle::InnerRadius,
            box_.to_document(star_inner_vertex(frame, point_count, inner_ratio)),
        )],
        Shape::Ellipse { .. } | Shape::Polygon { .. } => Vec::new(),
    }
}

/// Whether the centre move handle is drawn (criterion 8): its tier is
/// reached, no parameter drag runs, and no parameter handle is within the
/// yield distance of the box centre. The centre handle has no hit area, so
/// hiding it loses no function.
#[must_use]
pub fn centre_drawn(
    tiers: HandleTiers,
    params: &[(ParamHandle, Point)],
    centre: Point,
    tolerances: &TransformHandleTolerances,
    param_drag_active: bool,
) -> bool {
    tiers.centre
        && !param_drag_active
        && params
            .iter()
            .all(|(_, at)| at.vector_to(centre).length() > tolerances.param_centre_yield_mm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        Angle, CornerRadii, Document, EllipseFrame, Length, NodeId, PrimitiveSnapshot, RectBounds,
    };
    use proptest::prelude::*;

    use crate::oriented_box::oriented_bounds;
    use crate::transform_handle_layout::{
        ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, transform_handles,
    };

    /// The 8 px resize squircle, the 12 px rotate glyph and the 16 px centre
    /// glyph (`docs/design-system.md`). `curvyo-render-core` asserts in its own
    /// test that its drawn glyphs never exceed these numbers: the two crates
    /// share no code, so the duplicated number is guarded on both sides.
    const RESIZE_GLYPH_PX: f64 = 8.0;
    const ROTATE_GLYPH_PX: f64 = 12.0;
    const CENTRE_GLYPH_PX: f64 = 16.0;
    /// A circumscribed radius: half the diagonal of the square glyph.
    const RESIZE_GLYPH_RADIUS_PX: f64 = RESIZE_GLYPH_PX * std::f64::consts::FRAC_1_SQRT_2;
    const KNOB_RADIUS_PX: f64 = KNOB_DIAMETER_PX / 2.0;
    const ROTATE_GLYPH_RADIUS_PX: f64 = ROTATE_GLYPH_PX / 2.0;
    /// The 16 px centre glyph has 3 px rounded corners: its farthest point is
    /// the corner arc's centre plus the radius, 10.07 px.
    const CENTRE_GLYPH_RADIUS_PX: f64 =
        (CENTRE_GLYPH_PX / 2.0 - 3.0) * std::f64::consts::SQRT_2 + 3.0;

    fn tolerances(px_per_mm: f64) -> TransformHandleTolerances {
        TransformHandleTolerances::at_scale(px_per_mm)
    }

    fn id() -> NodeId {
        Document::new(1).create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(1.0),
            height: Length::from_mm(1.0),
        })
    }

    fn rect_object(width: f64, height: f64, radius: f64, rotation: f64) -> ObjectSnapshot {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            id: id(),
            shape: Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(10.0, 20.0),
                    width: Length::from_mm(width),
                    height: Length::from_mm(height),
                },
                corner_radii: CornerRadii::uniform(Length::from_mm(radius)),
            },
            style: curvyo_document_core::Style::default(),
            rotation: Angle::from_radians(rotation),
        })
    }

    /// `rectangle-corner-radii` criterion 1: each knob sits at the position its
    /// own effective radius implies, on its corner's diagonal.
    #[test]
    fn each_knob_is_placed_by_its_own_effective_radius() {
        let object = ObjectSnapshot::Primitive(PrimitiveSnapshot {
            id: id(),
            shape: Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(0.0, 0.0),
                    width: Length::from_mm(200.0),
                    height: Length::from_mm(100.0),
                },
                corner_radii: CornerRadii {
                    tl: Length::from_mm(0.0),
                    tr: Length::from_mm(20.0),
                    br: Length::from_mm(50.0),
                    bl: Length::from_mm(0.0),
                },
            },
            style: curvyo_document_core::Style::default(),
            rotation: Angle::from_radians(0.0),
        });
        let t = tolerances(1.0);
        let handles = param_handles(&object, &oriented_bounds(&object), &t);
        assert_eq!(handles.len(), 4);
        let travel = radius_travel(100.0, &t);
        for ((handle, at), (corner, radius)) in handles.into_iter().zip([
            (Corner::Tl, 0.0),
            (Corner::Tr, 20.0),
            (Corner::Br, 50.0),
            (Corner::Bl, 0.0),
        ]) {
            assert_eq!(handle, ParamHandle::CornerRadius(corner));
            let distance = t.param_inset_mm + radius / 50.0 * travel;
            let local = box_local_corner(corner, 200.0, 100.0);
            let diagonal = corner.inward_diagonal();
            let expected = Point::new(
                local.x + diagonal.x * distance,
                local.y + diagonal.y * distance,
            );
            assert!((at.x - expected.x).abs() < 1e-9 && (at.y - expected.y).abs() < 1e-9);
        }
    }

    fn box_local_corner(corner: Corner, width: f64, height: f64) -> Point {
        match corner {
            Corner::Tl => Point::new(0.0, 0.0),
            Corner::Tr => Point::new(width, 0.0),
            Corner::Br => Point::new(width, height),
            Corner::Bl => Point::new(0.0, height),
        }
    }

    fn star_object(
        radius: f64,
        points: u32,
        ratio: f64,
        angle: f64,
        rotation: f64,
    ) -> ObjectSnapshot {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            id: id(),
            shape: Shape::Star {
                frame: StarFrame {
                    center: Point::new(50.0, 40.0),
                    radius: Length::from_mm(radius),
                    angle: Angle::from_radians(angle),
                },
                point_count: PointCount::new(points).unwrap(),
                inner_ratio: InnerRatio::new(ratio).unwrap(),
            },
            style: curvyo_document_core::Style::default(),
            rotation: Angle::from_radians(rotation),
        })
    }

    #[test]
    fn tiers_change_at_24_48_and_72_pixels_with_rounding_slack() {
        let t = tolerances(2.0);
        let tiers = |px: f64| handle_tiers(px / 2.0, &t);
        assert_eq!(
            (tiers(23.9).edge, tiers(23.9).centre, tiers(23.9).param),
            (false, false, false)
        );
        assert!(tiers(24.0).edge && !tiers(24.0).centre);
        assert!(tiers(47.9).edge && !tiers(47.9).centre);
        assert!(tiers(48.0).centre && !tiers(48.0).param);
        assert!(tiers(71.9).centre && !tiers(71.9).param);
        assert!(tiers(72.0).param);
        // `px / scale * scale` rounds a hair under 72 and still counts.
        assert!(tiers(71.95).param, "within the rounding slack");
        assert!(tiers(0.0).corner, "corner handles are never hidden");
    }

    #[test]
    fn parameter_handles_disappear_first_then_the_centre_then_the_edges() {
        let t = tolerances(1.0);
        let order = |s: f64| {
            let tiers = handle_tiers(s, &t);
            (tiers.param, tiers.centre, tiers.edge)
        };
        assert_eq!(order(72.0), (true, true, true));
        assert_eq!(order(60.0), (false, true, true));
        assert_eq!(order(30.0), (false, false, true));
        assert_eq!(order(10.0), (false, false, false));
    }

    #[test]
    fn the_travel_is_positive_at_the_threshold_and_the_gain_matches_the_design_system() {
        let t = tolerances(1.0);
        assert!(radius_travel(PARAM_MIN_SIDE_PX, &t) > 0.0);
        for (side, gain) in [(72.0, 1.38), (100.0, 1.09), (200.0, 0.86)] {
            assert!(
                (radius_gain(side, &t) - gain).abs() < 0.01,
                "gain at {side}"
            );
        }
        assert!((radius_gain(1e6, &t) - 0.707).abs() < 0.01, "large end");
    }

    #[test]
    fn a_sharp_rectangle_shows_four_radius_handles_15px_from_their_corners() {
        let t = tolerances(1.0);
        let object = rect_object(200.0, 100.0, 0.0, 0.0);
        let box_ = oriented_bounds(&object);
        let handles = param_handles(&object, &box_, &t);
        assert_eq!(handles.len(), 4);
        let (_, tl) = handles
            .iter()
            .find(|(h, _)| *h == ParamHandle::CornerRadius(Corner::Tl))
            .unwrap();
        let along = 15.0 / std::f64::consts::SQRT_2;
        assert!((tl.x - (10.0 + along)).abs() < 1e-9 && (tl.y - (20.0 + along)).abs() < 1e-9);
    }

    #[test]
    fn the_handle_reaches_s_minus_14_over_two_from_each_side_at_the_largest_radius() {
        let t = tolerances(1.0);
        let object = rect_object(100.0, 100.0, 50.0, 0.0);
        let box_ = oriented_bounds(&object);
        let handles = param_handles(&object, &box_, &t);
        let (_, tl) = handles
            .iter()
            .find(|(h, _)| *h == ParamHandle::CornerRadius(Corner::Tl))
            .unwrap();
        // (100 - 14) / 2 = 43 from each side.
        assert!((tl.x - 53.0).abs() < 1e-9 && (tl.y - 63.0).abs() < 1e-9);
    }

    #[test]
    fn radius_handle_position_uses_the_effective_radius_of_a_shrunken_rectangle() {
        let t = tolerances(1.0);
        // Stored 200, effective 40 (half of 80).
        let stored = rect_object(100.0, 80.0, 200.0, 0.0);
        let clamped = rect_object(100.0, 80.0, 40.0, 0.0);
        let at = |o: &ObjectSnapshot| param_handles(o, &oriented_bounds(o), &t);
        assert_eq!(at(&stored), at(&clamped));
    }

    #[test]
    fn a_rotated_rectangle_places_its_handles_in_its_own_frame() {
        let t = tolerances(1.0);
        let flat = rect_object(120.0, 90.0, 10.0, 0.0);
        let turned = rect_object(120.0, 90.0, 10.0, std::f64::consts::FRAC_PI_2);
        let centre = Point::new(70.0, 65.0);
        let first = |o: &ObjectSnapshot| {
            param_handles(o, &oriented_bounds(o), &t)
                .into_iter()
                .find(|(h, _)| *h == ParamHandle::CornerRadius(Corner::Tl))
                .unwrap()
                .1
        };
        let (a, b) = (first(&flat), first(&turned));
        let (va, vb) = (centre.vector_to(a), centre.vector_to(b));
        // A quarter turn about the centre: (x, y) becomes (-y, x).
        assert!((vb.x + va.y).abs() < 1e-9 && (vb.y - va.x).abs() < 1e-9);
    }

    #[test]
    fn a_star_has_one_handle_at_its_first_inner_vertex() {
        let t = tolerances(1.0);
        let object = star_object(50.0, 5, 0.5, 0.0, 0.0);
        let handles = param_handles(&object, &oriented_bounds(&object), &t);
        let [(ParamHandle::InnerRadius, at)] = handles.as_slice() else {
            panic!("one inner-radius handle, got {handles:?}");
        };
        let theta = std::f64::consts::PI / 5.0;
        assert!((at.x - (50.0 + 25.0 * theta.cos())).abs() < 1e-9);
        assert!((at.y - (40.0 + 25.0 * theta.sin())).abs() < 1e-9);
    }

    #[test]
    fn ellipses_polygons_and_paths_show_no_parameter_handle() {
        let t = tolerances(1.0);
        let ellipse = ObjectSnapshot::Primitive(PrimitiveSnapshot {
            id: id(),
            shape: Shape::Ellipse {
                frame: EllipseFrame {
                    center: Point::new(0.0, 0.0),
                    rx: Length::from_mm(100.0),
                    ry: Length::from_mm(80.0),
                },
            },
            style: curvyo_document_core::Style::default(),
            rotation: Angle::from_radians(0.0),
        });
        assert_eq!(
            param_handles(&ellipse, &oriented_bounds(&ellipse), &t),
            Vec::<(ParamHandle, Point)>::new()
        );
    }

    #[test]
    fn below_72_px_nothing_is_drawn_and_from_72_px_everything_is() {
        let object = rect_object(36.0, 36.0, 0.0, 0.0); // 72 px at 2 px/mm
        let box_ = oriented_bounds(&object);
        assert_eq!(param_handles(&object, &box_, &tolerances(2.0)).len(), 4);
        assert_eq!(
            param_handles(&object, &box_, &tolerances(1.99)),
            Vec::<(ParamHandle, Point)>::new()
        );
    }

    #[test]
    fn the_centre_handle_yields_to_a_close_radius_handle_and_during_a_parameter_drag() {
        let t = tolerances(1.0);
        let centre = Point::new(0.0, 0.0);
        let tiers = handle_tiers(100.0, &t);
        let near = [(ParamHandle::InnerRadius, Point::new(19.0, 0.0))];
        let far = [(ParamHandle::InnerRadius, Point::new(21.0, 0.0))];
        assert!(!centre_drawn(tiers, &near, centre, &t, false));
        assert!(centre_drawn(tiers, &far, centre, &t, false));
        assert!(!centre_drawn(tiers, &far, centre, &t, true));
        assert!(!centre_drawn(
            handle_tiers(40.0, &t),
            &[],
            centre,
            &t,
            false
        ));
    }

    /// Every drawn glyph of a box as `(position, circumscribed radius px)`,
    /// the centre handle excluded (it yields, see `centre_drawn`).
    fn drawn_glyphs(
        object: &ObjectSnapshot,
        t: &TransformHandleTolerances,
        side_rotate: bool,
    ) -> Vec<(Point, f64)> {
        let box_ = oriented_bounds(object);
        let spec = HandleSpec {
            resize_directions: if matches!(
                object,
                ObjectSnapshot::Primitive(PrimitiveSnapshot {
                    shape: Shape::Star { .. } | Shape::Polygon { .. },
                    ..
                })
            ) {
                &CORNERS_FOUR
            } else {
                &ALL_EIGHT
            },
            skew: false,
            side_rotate,
        };
        let mut glyphs: Vec<(Point, f64)> = transform_handles(&box_, spec, t)
            .into_iter()
            .filter_map(|(handle, at)| match handle {
                EditHandle::Resize(_) => Some((at, RESIZE_GLYPH_RADIUS_PX)),
                EditHandle::Rotate(_) => Some((at, ROTATE_GLYPH_RADIUS_PX)),
                _ => None,
            })
            .collect();
        glyphs.extend(
            param_handles(object, &box_, t)
                .into_iter()
                .map(|(_, at)| (at, KNOB_RADIUS_PX)),
        );
        glyphs
    }

    /// The smallest gap in screen pixels between any two glyphs.
    fn smallest_gap_px(glyphs: &[(Point, f64)], px_per_mm: f64) -> f64 {
        let mut smallest = f64::INFINITY;
        for (i, (a, ra)) in glyphs.iter().enumerate() {
            for (b, rb) in &glyphs[i + 1..] {
                let distance = a.vector_to(*b).length() * px_per_mm;
                smallest = smallest.min(distance - ra - rb);
            }
        }
        smallest
    }

    proptest! {
        /// Criterion 8: for a rectangle of any aspect, radius, rotation and
        /// zoom from 72 px up, no two drawn glyphs come within 4 px.
        #[test]
        fn rectangle_glyphs_keep_four_pixels_apart(
            shorter_px in 72.0f64..600.0,
            aspect in 1.0f64..8.0,
            rho in 0.0f64..=1.0,
            rotation in 0.0f64..std::f64::consts::TAU,
            scale in 0.2f64..8.0,
            side_rotate in proptest::bool::ANY,
        ) {
            let shorter = shorter_px / scale;
            let object = rect_object(shorter * aspect, shorter, rho * shorter / 2.0, rotation);
            let t = tolerances(scale);
            let glyphs = drawn_glyphs(&object, &t, side_rotate);
            prop_assert!(glyphs.len() >= 12);
            let gap = smallest_gap_px(&glyphs, scale);
            prop_assert!(gap >= MIN_GLYPH_GAP_PX - 1e-6, "gap {gap} at s={shorter_px} rho={rho}");
        }

        /// Criterion 8, the star: from 72 px up, for every point count,
        /// ratio, orientation and rotation, the inner knob keeps 4 px from
        /// every other glyph and at least the analytic bound.
        #[test]
        fn star_glyphs_keep_four_pixels_apart(
            shorter_px in 72.0f64..400.0,
            points in 3u32..=64,
            ratio in 0.01f64..=0.99,
            angle in 0.0f64..std::f64::consts::TAU,
            rotation in 0.0f64..std::f64::consts::TAU,
            scale in 0.2f64..8.0,
        ) {
            let radius = shorter_px / scale / 2.0;
            let object = star_object(radius, points, ratio, angle, rotation);
            let t = tolerances(scale);
            let glyphs = drawn_glyphs(&object, &t, false);
            let gap = smallest_gap_px(&glyphs, scale);
            prop_assert!(gap >= MIN_GLYPH_GAP_PX - 1e-6, "gap {gap} at s={shorter_px}");
            let bound = (shorter_px / 2.0) * (std::f64::consts::SQRT_2 - ratio)
                - (KNOB_RADIUS_PX + RESIZE_GLYPH_RADIUS_PX);
            prop_assert!(gap >= bound.min(MIN_GLYPH_GAP_PX) - 1e-9);
        }
    }

    fn rect_with_radii(
        width: f64,
        height: f64,
        radii: CornerRadii,
        rotation: f64,
    ) -> ObjectSnapshot {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            id: id(),
            shape: Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(10.0, 20.0),
                    width: Length::from_mm(width),
                    height: Length::from_mm(height),
                },
                corner_radii: radii,
            },
            style: curvyo_document_core::Style::default(),
            rotation: Angle::from_radians(rotation),
        })
    }

    fn radii_of(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
        CornerRadii {
            tl: Length::from_mm(tl),
            tr: Length::from_mm(tr),
            br: Length::from_mm(br),
            bl: Length::from_mm(bl),
        }
    }

    /// Criterion 1, the fixed case: TL = BR = 0.6 s on a square at `s` = 72 px.
    /// Both diagonal knobs would overlap, so both rest at the `ρ = 1` position.
    #[test]
    fn two_large_diagonal_corners_rest_at_rho_one() {
        let t = tolerances(1.0);
        let effective = radii_of(0.6 * 72.0, 0.0, 0.6 * 72.0, 0.0);
        let rho = |corner| knob_rho(effective, 72.0, 72.0, corner, &t);
        assert!((rho(Corner::Tl) - 1.0).abs() < 1e-12);
        assert!((rho(Corner::Br) - 1.0).abs() < 1e-12);
        assert!(rho(Corner::Tr).abs() < 1e-12);
    }

    /// Criterion 1: a knob at or below `ρ = 1` is always at its own radius; a
    /// lone corner may reach 2; on a wide box the longer side buys room (Σ), so
    /// two diagonal corners of 1.2 do not lag.
    #[test]
    fn the_cap_leaves_small_knobs_lone_knobs_and_wide_boxes_alone() {
        let t = tolerances(1.0);
        let half = 36.0;
        let small_and_large = radii_of(0.9 * half, 0.0, 1.9 * half, 0.0);
        assert!((knob_rho(small_and_large, 72.0, 72.0, Corner::Tl, &t) - 0.9).abs() < 1e-12);
        let lone = radii_of(0.0, 0.0, 2.0 * half, 0.0);
        assert!((knob_rho(lone, 72.0, 72.0, Corner::Br, &t) - 2.0).abs() < 1e-12);
        // One corner at 1.6 pulls its diagonal partner in from 0.4 on.
        let pair = radii_of(1.6 * half, 0.0, 0.5 * half, 0.0);
        assert!((knob_rho(pair, 72.0, 72.0, Corner::Br, &t) - 0.5).abs() < 1e-12);
        let pair = radii_of(1.6 * half, 0.0, 0.6 * half, 0.0);
        assert!((knob_rho(pair, 72.0, 72.0, Corner::Br, &t) - 0.6).abs() < 1e-12);
        let pair = radii_of(1.6 * half, 0.0, 1.0 * half, 0.0);
        assert!((knob_rho(pair, 72.0, 72.0, Corner::Br, &t) - 1.0).abs() < 1e-12);
        let wide = radii_of(1.2 * half, 0.0, 1.2 * half, 0.0);
        assert!((knob_rho(wide, 72.0, 180.0, Corner::Tl, &t) - 1.2).abs() < 1e-12);
    }

    proptest! {
        /// Criterion 1 and 8: for any four radii the CSS rule allows (stored radii
        /// pushed through the clamp, so TL = BR large and one corner at the
        /// shorter side are in the sample), any aspect, rotation and zoom from
        /// 72 px up, no two drawn glyphs come within 4 px, and the drawn knobs
        /// stay on their own diagonal.
        #[test]
        fn rectangle_glyphs_keep_four_pixels_apart_for_any_four_radii(
            shorter_px in 72.0f64..600.0,
            aspect in 1.0f64..8.0,
            stored in proptest::array::uniform4(0.0f64..=2.0),
            rotation in 0.0f64..std::f64::consts::TAU,
            scale in 0.2f64..8.0,
            side_rotate in proptest::bool::ANY,
        ) {
            let shorter = shorter_px / scale;
            let radii = radii_of(
                stored[0] * shorter / 2.0 * 1.4,
                stored[1] * shorter / 2.0 * 1.4,
                stored[2] * shorter / 2.0 * 1.4,
                stored[3] * shorter / 2.0 * 1.4,
            );
            let object = rect_with_radii(shorter * aspect, shorter, radii, rotation);
            let t = tolerances(scale);
            let glyphs = drawn_glyphs(&object, &t, side_rotate);
            let gap = smallest_gap_px(&glyphs, scale);
            prop_assert!(gap >= MIN_GLYPH_GAP_PX - 1e-6, "gap {gap} at s={shorter_px} radii={radii:?}");
        }
    }

    /// The worst-case pairs of that property, pinned: the diagonal pair of the
    /// criterion and a lone corner at `ρ = 2`, at the 72 px threshold.
    #[test]
    fn the_pinned_worst_cases_keep_four_pixels() {
        let t = tolerances(1.0);
        for radii in [
            radii_of(0.6 * 72.0, 0.0, 0.6 * 72.0, 0.0),
            radii_of(72.0, 0.0, 0.0, 0.0),
            radii_of(0.0, 72.0, 0.0, 0.0),
            radii_of(0.0, 36.0, 36.0, 0.0),
        ] {
            for side_rotate in [false, true] {
                let object = rect_with_radii(72.0, 72.0, radii, 0.0);
                let gap = smallest_gap_px(&drawn_glyphs(&object, &t, side_rotate), 1.0);
                assert!(gap >= MIN_GLYPH_GAP_PX - 1e-6, "{radii:?}: gap {gap}");
            }
        }
    }

    /// The worst star case of the architect's note: the inner vertex on the
    /// box diagonal at ratio 0.99, `s` 72: 4.6 px to the corner glyph. The box
    /// is turned by the frame angle, so the inner vertex is at `π/N` from the
    /// box's +x axis whatever the angle: only N = 4 puts it on the diagonal, and
    /// every other count clears the corner glyph by more.
    #[test]
    fn the_worst_star_case_clears_the_corner_glyph_by_four_point_six_pixels() {
        for points in [3u32, 4, 5, 8, 12, 1024] {
            for angle in [0.0, 0.7, -2.0] {
                let object = star_object(36.0, points, 0.99, angle, 0.0);
                let t = tolerances(1.0);
                let box_ = oriented_bounds(&object);
                let handles = param_handles(&object, &box_, &t);
                let knob = handles[0].1;
                let gap = box_
                    .document_corners()
                    .iter()
                    .map(|corner| {
                        knob.vector_to(*corner).length() - KNOB_RADIUS_PX - RESIZE_GLYPH_RADIUS_PX
                    })
                    .fold(f64::INFINITY, f64::min);
                if points == 4 {
                    assert!((gap - 4.6).abs() < 0.05, "N = {points}: gap {gap}");
                } else {
                    assert!(gap > 4.6, "N = {points}: gap {gap}");
                }
            }
        }
    }

    /// The centre glyph, when drawn, is at least 4 px from every knob, measured
    /// from the glyph's rounded-square extent to the knob's circle.
    #[test]
    fn the_centre_glyph_keeps_four_pixels_from_the_knobs() {
        let t = tolerances(1.0);
        for rho in [0.0, 0.3, 0.6, 0.61, 0.8, 1.0] {
            let object = rect_object(72.0, 72.0, rho * 36.0, 0.0);
            let box_ = oriented_bounds(&object);
            let params = param_handles(&object, &box_, &t);
            let centre = box_.to_document(box_.local_center());
            if !centre_drawn(handle_tiers(72.0, &t), &params, centre, &t, false) {
                continue;
            }
            for (_, at) in &params {
                let gap = at.vector_to(centre).length() - CENTRE_GLYPH_RADIUS_PX - KNOB_RADIUS_PX;
                assert!(gap >= MIN_GLYPH_GAP_PX, "rho {rho}: gap {gap}");
            }
        }
    }
}
