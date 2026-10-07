//! A primitive's closed-form outline (`specs/0003-primitive-shapes/adrs.md`,
//! "outline construction lives in `curvyo-document-core`"): one pure
//! function per shape, parameters in, [`OutlineAnchor`]s out, no curve
//! ever evaluated (flattened, projected, subdivided or intersected) —
//! only trigonometry and the fixed Bézier-circle constant `KAPPA`. Three
//! consumers reach these functions over edges that already exist:
//! `curvyo-render-core` (stroking, acceptance criterion 16),
//! `curvyo-ui-core` (hit-testing and "object to path", acceptance
//! criteria 17-20), and a later SVG exporter.
//!
//! Every outline here is closed, and direction is increasing angle in
//! Y-down document space — clockwise on screen, matching Inkscape's own
//! rect/ellipse/polygon → path conversion (`adrs.md`).

use crate::corner_radii::{Corner, CornerRadii, SHARP_CORNER_EPSILON_MM, effective_corner_radii};
use crate::path_model::AnchorKind;
use crate::primitive_model::{
    EllipseFrame, PointCount, RectBounds, Shape, StarFrame, shape_center,
};
use crate::units::{Angle, Point, Vec2};

/// The standard cubic-Bézier approximation of a quarter circle:
/// `4 * (√2 - 1) / 3`. Used for ellipse quadrants (acceptance criterion
/// 19) and rounded-rectangle corners (acceptance criterion 18) alike.
#[allow(clippy::excessive_precision)]
pub const KAPPA: f64 = 0.552_284_749_830_793_4;

/// One outline anchor, structurally identical to
/// [`crate::path_model::NewAnchor`] minus the identity: an outline is a
/// pure function of a shape's parameters and has no [`crate::AnchorId`]
/// of its own until a caller (`curvyo-ui-core`, converting one to a
/// real path) mints one (`adrs.md`: "a primitive's outline is closed-
/// form... parameters → its outline anchors without ids").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineAnchor {
    /// The anchor's absolute document-space position.
    pub point: Point,
    /// Its handle toward the previous anchor, relative to `point`.
    pub handle_in: Vec2,
    /// Its handle toward the next anchor, relative to `point`.
    pub handle_out: Vec2,
    /// Corner or smooth.
    pub kind: AnchorKind,
}

impl OutlineAnchor {
    /// A corner node with live handles, the tangent point of an arc.
    const fn tangent(point: Point, handle_in: Vec2, handle_out: Vec2) -> Self {
        Self {
            point,
            handle_in,
            handle_out,
            kind: AnchorKind::Corner,
        }
    }

    const fn corner(point: Point) -> Self {
        Self {
            point,
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind: AnchorKind::Corner,
        }
    }
}

/// A rectangle's outline (`specs/0003-primitive-shapes/` criteria 1-6, 18;
/// `specs/rectangle-corner-radii/` criteria 11, 16): clockwise on screen from
/// the end of the top-left corner's arc (or the top-left corner point if it is
/// sharp). Each corner with a positive effective radius contributes two
/// tangent nodes joined by one cubic arc, each sharp corner one node, so the
/// list holds 4 to 8 nodes, all [`AnchorKind::Corner`] (a line-to-arc join
/// cannot be [`AnchorKind::Symmetric`], see `adrs.md`). The radii are
/// evaluated by [`effective_corner_radii`] first.
#[must_use]
pub fn rect_outline(bounds: RectBounds, radii: CornerRadii) -> Vec<OutlineAnchor> {
    let effective = effective_corner_radii(bounds, radii);
    let [tl, tr, br, bl] = Corner::ALL.map(|corner| effective.get(corner).as_mm());
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let right = left + bounds.width.as_mm();
    let bottom = top + bounds.height.as_mm();
    let sharp = |radius: f64| radius <= SHARP_CORNER_EPSILON_MM;
    let at = OutlineAnchor::tangent;

    let mut anchors = Vec::with_capacity(8);
    // Top edge, left end: the end of the top-left arc (its start closes the loop).
    anchors.push(if sharp(tl) {
        OutlineAnchor::corner(Point::new(left, top))
    } else {
        at(
            Point::new(left + tl, top),
            Vec2::new(-KAPPA * tl, 0.0),
            Vec2::ZERO,
        )
    });
    if sharp(tr) {
        anchors.push(OutlineAnchor::corner(Point::new(right, top)));
    } else {
        let handle = KAPPA * tr;
        anchors.push(at(
            Point::new(right - tr, top),
            Vec2::ZERO,
            Vec2::new(handle, 0.0),
        ));
        anchors.push(at(
            Point::new(right, top + tr),
            Vec2::new(0.0, -handle),
            Vec2::ZERO,
        ));
    }
    if sharp(br) {
        anchors.push(OutlineAnchor::corner(Point::new(right, bottom)));
    } else {
        let handle = KAPPA * br;
        anchors.push(at(
            Point::new(right, bottom - br),
            Vec2::ZERO,
            Vec2::new(0.0, handle),
        ));
        anchors.push(at(
            Point::new(right - br, bottom),
            Vec2::new(handle, 0.0),
            Vec2::ZERO,
        ));
    }
    if sharp(bl) {
        anchors.push(OutlineAnchor::corner(Point::new(left, bottom)));
    } else {
        let handle = KAPPA * bl;
        anchors.push(at(
            Point::new(left + bl, bottom),
            Vec2::ZERO,
            Vec2::new(-handle, 0.0),
        ));
        anchors.push(at(
            Point::new(left, bottom - bl),
            Vec2::new(0.0, handle),
            Vec2::ZERO,
        ));
    }
    if !sharp(tl) {
        anchors.push(at(
            Point::new(left, top + tl),
            Vec2::ZERO,
            Vec2::new(0.0, -KAPPA * tl),
        ));
    }
    anchors
}

/// An ellipse's outline (acceptance criteria 7-9, 19): 4 smooth nodes,
/// one per quadrant extreme, starting at `(cx + rx, cy)`.
#[must_use]
pub fn ellipse_outline(frame: EllipseFrame) -> Vec<OutlineAnchor> {
    let EllipseFrame { center, rx, ry } = frame;
    let rx = rx.as_mm();
    let ry = ry.as_mm();
    let kx = KAPPA * rx;
    let ky = KAPPA * ry;
    let smooth = AnchorKind::Symmetric;
    vec![
        // East (the rx extreme): vertical handles, length κ·ry.
        OutlineAnchor {
            point: center.translated(Vec2::new(rx, 0.0)),
            handle_in: Vec2::new(0.0, -ky),
            handle_out: Vec2::new(0.0, ky),
            kind: smooth,
        },
        // South (the ry extreme): horizontal handles, length κ·rx.
        OutlineAnchor {
            point: center.translated(Vec2::new(0.0, ry)),
            handle_in: Vec2::new(kx, 0.0),
            handle_out: Vec2::new(-kx, 0.0),
            kind: smooth,
        },
        // West.
        OutlineAnchor {
            point: center.translated(Vec2::new(-rx, 0.0)),
            handle_in: Vec2::new(0.0, ky),
            handle_out: Vec2::new(0.0, -ky),
            kind: smooth,
        },
        // North.
        OutlineAnchor {
            point: center.translated(Vec2::new(0.0, -ry)),
            handle_in: Vec2::new(-kx, 0.0),
            handle_out: Vec2::new(kx, 0.0),
            kind: smooth,
        },
    ]
}

/// The N vertex positions of a regular polygon/star frame, in traversal
/// order, at a given radius — shared by [`polygon_outline`] and
/// [`star_outline`]'s outer/inner vertex rings.
fn vertex(center: Point, radius: f64, angle_radians: f64) -> Point {
    center.translated(Vec2::new(
        radius * angle_radians.cos(),
        radius * angle_radians.sin(),
    ))
}

/// A regular polygon's outline (acceptance criteria 10, 11, 13, 15, 20):
/// `point_count` corner nodes, evenly spaced, joined by straight
/// segments.
#[must_use]
pub fn polygon_outline(frame: StarFrame, point_count: PointCount) -> Vec<OutlineAnchor> {
    let n = f64::from(point_count.get());
    let step = std::f64::consts::TAU / n;
    #[allow(clippy::cast_precision_loss)] // point_count is capped at 1024
    (0..point_count.get())
        .map(|k| {
            let theta = frame.angle.as_radians() + step * f64::from(k);
            OutlineAnchor::corner(vertex(frame.center, frame.radius.as_mm(), theta))
        })
        .collect()
}

/// A star's outline (acceptance criteria 10, 12, 13, 14, 15, 20):
/// `2 * point_count` corner nodes alternating outer/inner, joined by
/// straight segments. Inner vertices sit at the angular midpoints
/// between outer ones, at `inner_ratio * radius`.
#[must_use]
pub fn star_outline(
    frame: StarFrame,
    point_count: PointCount,
    inner_ratio: crate::primitive_model::InnerRatio,
) -> Vec<OutlineAnchor> {
    let n = f64::from(point_count.get());
    let step = std::f64::consts::TAU / n;
    let outer_r = frame.radius.as_mm();
    let inner_r = outer_r * inner_ratio.get();
    let mut anchors = Vec::with_capacity(2 * point_count.get() as usize);
    for k in 0..point_count.get() {
        let outer_theta = frame.angle.as_radians() + step * f64::from(k);
        anchors.push(OutlineAnchor::corner(vertex(
            frame.center,
            outer_r,
            outer_theta,
        )));
        let inner_theta = outer_theta + step / 2.0;
        anchors.push(OutlineAnchor::corner(vertex(
            frame.center,
            inner_r,
            inner_theta,
        )));
    }
    anchors
}

/// Dispatches to the right outline function for `shape` — the one call
/// every consumer actually needs (acceptance criterion 17's conversion,
/// rendering, hit-testing).
#[must_use]
pub fn outline_of(shape: &Shape) -> Vec<OutlineAnchor> {
    match *shape {
        Shape::Rect {
            bounds,
            corner_radii,
        } => rect_outline(bounds, corner_radii),
        Shape::Ellipse { frame } => ellipse_outline(frame),
        Shape::Polygon { frame, point_count } => polygon_outline(frame, point_count),
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => star_outline(frame, point_count, inner_ratio),
    }
}

/// `shape`'s outline, rotated about its own frame center by `rotation` —
/// the one place a primitive's rotation is ever applied
/// (`specs/0005-object-transform/adrs.md`: "`document-core`'s outline
/// function returns the rotated outline... Rendering, `hit_test_object`,
/// object to path and later export all read that outline, so none of
/// them changes."). Identical to [`outline_of`] when `rotation` is zero.
#[must_use]
pub fn outline_of_rotated(shape: &Shape, rotation: Angle) -> Vec<OutlineAnchor> {
    let anchors = outline_of(shape);
    if rotation.as_radians() == 0.0 {
        return anchors;
    }
    let center = shape_center(shape);
    anchors
        .into_iter()
        .map(|anchor| OutlineAnchor {
            point: anchor.point.rotated_around(center, rotation),
            handle_in: anchor.handle_in.rotated(rotation),
            handle_out: anchor.handle_out.rotated(rotation),
            kind: anchor.kind,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive_model::InnerRatio;
    use crate::units::Length;

    /// Zero rotation returns exactly the plain outline.
    #[test]
    fn outline_of_rotated_at_zero_angle_matches_outline_of() {
        let shape = Shape::Rect {
            bounds: bounds(0.0, 0.0, 10.0, 10.0),
            corner_radii: CornerRadii::uniform(Length::from_mm(0.0)),
        };
        assert_eq!(
            outline_of_rotated(&shape, Angle::from_radians(0.0)),
            outline_of(&shape)
        );
    }

    /// A rotated rectangle's outline anchors are the unrotated outline's
    /// anchors rotated about the frame's own center (acceptance
    /// criterion 25's own rule, reused here for rendering/hit-testing).
    #[test]
    fn outline_of_rotated_rotates_every_anchor_about_the_frame_center() {
        let shape = Shape::Rect {
            bounds: bounds(0.0, 0.0, 10.0, 10.0),
            corner_radii: CornerRadii::uniform(Length::from_mm(0.0)),
        };
        let rotation = Angle::from_radians(std::f64::consts::FRAC_PI_2);
        let rotated = outline_of_rotated(&shape, rotation);
        let center = shape_center(&shape);
        let plain = outline_of(&shape);
        for (plain_anchor, rotated_anchor) in plain.iter().zip(rotated.iter()) {
            let expected = plain_anchor.point.rotated_around(center, rotation);
            assert!((rotated_anchor.point.x - expected.x).abs() < 1e-9);
            assert!((rotated_anchor.point.y - expected.y).abs() < 1e-9);
        }
    }

    fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        }
    }

    /// AC18: zero radius is exactly 4 corner nodes, straight segments.
    #[test]
    fn rect_outline_at_zero_radius_has_four_corner_nodes() {
        let outline = rect_outline(
            bounds(0.0, 0.0, 10.0, 10.0),
            CornerRadii::uniform(Length::from_mm(0.0)),
        );
        assert_eq!(outline.len(), 4);
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
        assert!(
            outline
                .iter()
                .all(|a| a.handle_in == Vec2::ZERO && a.handle_out == Vec2::ZERO)
        );
    }

    /// AC18: a non-zero radius is exactly 8 corner nodes (never
    /// smooth), 4 pairs joined by a curve, the rest by straight lines.
    #[test]
    fn rect_outline_at_nonzero_radius_has_eight_corner_nodes() {
        let outline = rect_outline(
            bounds(0.0, 0.0, 20.0, 10.0),
            CornerRadii::uniform(Length::from_mm(2.0)),
        );
        assert_eq!(outline.len(), 8);
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
        let curved = outline
            .iter()
            .filter(|a| a.handle_in != Vec2::ZERO || a.handle_out != Vec2::ZERO)
            .count();
        assert_eq!(curved, 8, "every tangent point has exactly one live handle");
    }

    /// AC19: 4 smooth nodes, 4 segments, within 0.1% of the larger
    /// radius for a true ellipse (kappa's own ≈0.027% deviation).
    #[test]
    fn ellipse_outline_has_four_smooth_nodes_within_tolerance() {
        let frame = EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(6.0),
        };
        let outline = ellipse_outline(frame);
        assert_eq!(outline.len(), 4);
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Symmetric));
        for anchor in &outline {
            assert_eq!(anchor.handle_in, anchor.handle_out.negated());
        }
        // Sample the mid-parameter point of the first (East->South)
        // Bézier segment and compare it against the true ellipse radius
        // at that angle — well within AC19's 0.1% of the larger radius.
        let a = outline[0];
        let b = outline[1];
        let c1 = a.point.translated(a.handle_out);
        let c2 = b.point.translated(b.handle_in);
        let t = 0.5_f64;
        let mt = 1.0 - t;
        let bx = mt.powi(3) * a.point.x
            + 3.0 * mt.powi(2) * t * c1.x
            + 3.0 * mt * t.powi(2) * c2.x
            + t.powi(3) * b.point.x;
        let by = mt.powi(3) * a.point.y
            + 3.0 * mt.powi(2) * t * c1.y
            + 3.0 * mt * t.powi(2) * c2.y
            + t.powi(3) * b.point.y;
        // True ellipse point at the 45-degree parameter between East and
        // South.
        let theta = std::f64::consts::FRAC_PI_4;
        let true_x = 10.0 * theta.cos();
        let true_y = 6.0 * theta.sin();
        let deviation = ((bx - true_x).powi(2) + (by - true_y).powi(2)).sqrt();
        assert!(deviation / 10.0 < 0.001, "deviation {deviation} too large");
    }

    /// AC20: an N-point polygon has exactly N corner nodes.
    #[test]
    fn polygon_outline_has_n_corner_nodes() {
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: crate::units::Angle::from_radians(0.0),
        };
        let outline = polygon_outline(frame, PointCount::new(6).unwrap());
        assert_eq!(outline.len(), 6);
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
        assert!(
            outline
                .iter()
                .all(|a| a.handle_in == Vec2::ZERO && a.handle_out == Vec2::ZERO)
        );
        // First vertex sits at the frame's own angle/radius (AC11).
        assert!((outline[0].point.x - 10.0).abs() < 1e-9);
        assert!(outline[0].point.y.abs() < 1e-9);
    }

    /// AC20: an N-point star has exactly 2N corner nodes, alternating
    /// outer/inner radius.
    #[test]
    fn star_outline_has_2n_corner_nodes_alternating_radius() {
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: crate::units::Angle::from_radians(0.0),
        };
        let outline = star_outline(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        assert_eq!(outline.len(), 10);
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
        let distance = |p: Point| p.vector_to(Point::new(0.0, 0.0)).length();
        for (i, anchor) in outline.iter().enumerate() {
            let expected = if i % 2 == 0 { 10.0 } else { 5.0 };
            assert!(
                (distance(anchor.point) - expected).abs() < 1e-9,
                "anchor {i} at distance {}",
                distance(anchor.point)
            );
        }
    }

    /// The slice-3 single-radius outline, kept verbatim (its own clamp
    /// included) as the reference the per-corner function must reproduce for
    /// four equal radii.
    fn legacy_rect_outline(bounds: RectBounds, corner_radius: f64) -> Vec<OutlineAnchor> {
        let half_shorter_side = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
        let radius = corner_radius.clamp(0.0, half_shorter_side.max(0.0));
        let left = bounds.origin.x;
        let top = bounds.origin.y;
        let width = bounds.width.as_mm();
        let height = bounds.height.as_mm();
        if radius <= 0.0 {
            return vec![
                OutlineAnchor::corner(Point::new(left, top)),
                OutlineAnchor::corner(Point::new(left + width, top)),
                OutlineAnchor::corner(Point::new(left + width, top + height)),
                OutlineAnchor::corner(Point::new(left, top + height)),
            ];
        }
        let handle = KAPPA * radius;
        let anchor = |point, handle_in, handle_out| OutlineAnchor {
            point,
            handle_in,
            handle_out,
            kind: AnchorKind::Corner,
        };
        vec![
            anchor(
                Point::new(left + radius, top),
                Vec2::new(-handle, 0.0),
                Vec2::ZERO,
            ),
            anchor(
                Point::new(left + width - radius, top),
                Vec2::ZERO,
                Vec2::new(handle, 0.0),
            ),
            anchor(
                Point::new(left + width, top + radius),
                Vec2::new(0.0, -handle),
                Vec2::ZERO,
            ),
            anchor(
                Point::new(left + width, top + height - radius),
                Vec2::ZERO,
                Vec2::new(0.0, handle),
            ),
            anchor(
                Point::new(left + width - radius, top + height),
                Vec2::new(handle, 0.0),
                Vec2::ZERO,
            ),
            anchor(
                Point::new(left + radius, top + height),
                Vec2::ZERO,
                Vec2::new(-handle, 0.0),
            ),
            anchor(
                Point::new(left, top + height - radius),
                Vec2::new(0.0, handle),
                Vec2::ZERO,
            ),
            anchor(
                Point::new(left, top + radius),
                Vec2::ZERO,
                Vec2::new(0.0, -handle),
            ),
        ]
    }

    fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
        CornerRadii {
            tl: Length::from_mm(tl),
            tr: Length::from_mm(tr),
            br: Length::from_mm(br),
            bl: Length::from_mm(bl),
        }
    }

    fn assert_anchor_close(actual: &OutlineAnchor, expected: &OutlineAnchor) {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(
            close(actual.point.x, expected.point.x)
                && close(actual.point.y, expected.point.y)
                && close(actual.handle_in.x, expected.handle_in.x)
                && close(actual.handle_in.y, expected.handle_in.y)
                && close(actual.handle_out.x, expected.handle_out.x)
                && close(actual.handle_out.y, expected.handle_out.y)
                && actual.kind == expected.kind,
            "{actual:?} vs {expected:?}"
        );
    }

    /// AC 16: with one radius for all corners the result is today's, anchor
    /// for anchor and exactly (not within a tolerance), 4 nodes sharp and 8
    /// rounded.
    #[test]
    fn four_equal_radii_within_the_limit_give_exactly_the_slice_3_outline() {
        for (x, y, w, h, r) in [
            (0.0, 0.0, 10.0, 10.0, 0.0),
            (0.0, 0.0, 20.0, 10.0, 2.0),
            (3.5, -4.25, 100.0, 40.0, 5.0),
            (-7.0, 1.0, 33.3, 77.7, 16.0),
            (0.0, 0.0, 10.0, 10.0, 5.0),
        ] {
            let b = bounds(x, y, w, h);
            let new = rect_outline(b, CornerRadii::uniform(Length::from_mm(r)));
            assert_eq!(new, legacy_rect_outline(b, r), "{w} x {h}, r {r}");
        }
    }

    /// A uniform radius above the limit is the old clamp, within the
    /// tolerance of the shared-factor rule.
    #[test]
    fn four_equal_radii_above_the_limit_match_the_slice_3_clamp() {
        for (w, h, r) in [
            (10.0, 20.0, 100.0),
            (100.0, 40.0, 25.0),
            (40.0, 100.0, 21.0),
        ] {
            let b = bounds(1.0, 2.0, w, h);
            let new = rect_outline(b, CornerRadii::uniform(Length::from_mm(r)));
            let old = legacy_rect_outline(b, r);
            assert_eq!(new.len(), old.len());
            for (n, o) in new.iter().zip(&old) {
                assert_anchor_close(n, o);
            }
        }
    }

    /// AC 16: 4 to 8 nodes, one per sharp corner and two per rounded one.
    #[test]
    fn each_rounded_corner_adds_one_node() {
        for mask in 0..16_u32 {
            let radius = |bit: u32| if mask & (1 << bit) == 0 { 0.0 } else { 6.0 };
            let outline = rect_outline(
                bounds(0.0, 0.0, 100.0, 60.0),
                radii(radius(0), radius(1), radius(2), radius(3)),
            );
            assert_eq!(outline.len(), 4 + mask.count_ones() as usize, "{mask:04b}");
            assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
        }
    }

    /// AC 16: the path starts at the end of the top-left arc, or at the
    /// top-left corner point when it is sharp.
    #[test]
    fn the_outline_starts_at_the_top_left_corner() {
        let b = bounds(10.0, 20.0, 100.0, 60.0);
        let sharp = rect_outline(b, radii(0.0, 8.0, 8.0, 8.0));
        assert_eq!(sharp[0].point, Point::new(10.0, 20.0));
        assert_eq!(sharp.len(), 7);
        let rounded = rect_outline(b, radii(8.0, 0.0, 0.0, 0.0));
        assert_eq!(rounded[0].point, Point::new(18.0, 20.0));
        assert_eq!(rounded[rounded.len() - 1].point, Point::new(10.0, 28.0));
    }

    /// AC 16: clockwise on screen (Y down), so the shoelace sum is positive.
    #[test]
    fn the_outline_runs_clockwise_on_screen() {
        for r in [
            radii(0.0, 0.0, 0.0, 0.0),
            radii(5.0, 0.0, 0.0, 0.0),
            radii(5.0, 9.0, 0.0, 12.0),
            radii(5.0, 9.0, 3.0, 12.0),
        ] {
            let outline = rect_outline(bounds(0.0, 0.0, 100.0, 60.0), r);
            let twice_area: f64 = outline
                .iter()
                .zip(outline.iter().cycle().skip(1))
                .map(|(a, b)| a.point.x * b.point.y - b.point.x * a.point.y)
                .sum();
            assert!(twice_area > 0.0, "{r:?}");
        }
    }

    /// AC 16: each arc deviates from the true quarter circle by at most
    /// 0.1 % of that corner's own radius.
    #[test]
    fn every_arc_is_within_a_tenth_of_a_percent_of_its_circle() {
        let (width, height) = (100.0, 60.0);
        let outline = rect_outline(
            bounds(0.0, 0.0, width, height),
            radii(4.0, 11.0, 23.0, 30.0),
        );
        assert_eq!(outline.len(), 8);
        // Arc k joins the pair (node 2k+1, node 2k+2) for TR, BR, BL, and the
        // last node to the first for TL; centres are the inward corner offsets.
        let arcs = [
            (1, 2, Point::new(width - 11.0, 11.0), 11.0),
            (3, 4, Point::new(width - 23.0, height - 23.0), 23.0),
            (5, 6, Point::new(30.0, height - 30.0), 30.0),
            (7, 0, Point::new(4.0, 4.0), 4.0),
        ];
        for (from, to, centre, radius) in arcs {
            let (first, last) = (outline[from], outline[to]);
            let c1 = first.point.translated(first.handle_out);
            let c2 = last.point.translated(last.handle_in);
            // The cubic's value at t = 1/2.
            let at_half = |start: f64, control1: f64, control2: f64, end: f64| {
                0.125 * start + 0.375 * control1 + 0.375 * control2 + 0.125 * end
            };
            let mid = Point::new(
                at_half(first.point.x, c1.x, c2.x, last.point.x),
                at_half(first.point.y, c1.y, c2.y, last.point.y),
            );
            let distance = centre.vector_to(mid).length();
            assert!(
                (distance - radius).abs() / radius < 0.001,
                "arc {from}->{to}: {distance} vs {radius}"
            );
        }
    }

    /// AC 9, 16: the two worked examples of the specification on 100 x 40.
    #[test]
    fn the_specification_examples_on_a_100_by_40_rectangle() {
        let b = bounds(0.0, 0.0, 100.0, 40.0);
        // TL 30, TR 30, BR 0, BL 0: f = 1, six nodes.
        let outline = rect_outline(b, radii(30.0, 30.0, 0.0, 0.0));
        let points: Vec<(f64, f64)> = outline.iter().map(|a| (a.point.x, a.point.y)).collect();
        assert_eq!(
            points,
            [
                (30.0, 0.0),
                (70.0, 0.0),
                (100.0, 30.0),
                (100.0, 40.0),
                (0.0, 40.0),
                (0.0, 30.0)
            ]
        );
        // TL 30, TR 30, BL 30, BR 0: f = 40/60, effective 20, 20, 0, 20.
        let outline = rect_outline(b, radii(30.0, 30.0, 0.0, 30.0));
        let points: Vec<(f64, f64)> = outline.iter().map(|a| (a.point.x, a.point.y)).collect();
        let expected = [
            (20.0, 0.0),
            (80.0, 0.0),
            (100.0, 20.0),
            (100.0, 40.0),
            (20.0, 40.0),
            (0.0, 20.0),
            (0.0, 20.0 + 0.0),
        ];
        // Seven nodes: TL end, TR start, TR end, BR corner, BL start, BL end, TL start.
        assert_eq!(points.len(), 7);
        for (actual, expected) in points.iter().zip(&expected[..5]) {
            assert!((actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9);
        }
        assert!((points[5].0).abs() < 1e-9 && (points[5].1 - 20.0).abs() < 1e-9);
        assert!((points[6].0).abs() < 1e-9 && (points[6].1 - 20.0).abs() < 1e-9);
    }

    /// AC 11: an effective radius within the sharp tolerance is a sharp
    /// corner (one node).
    #[test]
    fn a_radius_within_the_sharp_tolerance_is_a_sharp_corner() {
        let outline = rect_outline(
            bounds(0.0, 0.0, 10.0, 10.0),
            radii(SHARP_CORNER_EPSILON_MM / 2.0, 0.0, 0.0, 0.0),
        );
        assert_eq!(outline.len(), 4);
        let outline = rect_outline(
            bounds(0.0, 0.0, 10.0, 10.0),
            radii(SHARP_CORNER_EPSILON_MM * 10.0, 0.0, 0.0, 0.0),
        );
        assert_eq!(outline.len(), 5);
    }

    /// AC 16: two arcs meeting leave a straight segment of length 0 and two
    /// coincident nodes, as before; they are not merged.
    #[test]
    fn arcs_that_meet_keep_their_zero_length_segment() {
        let outline = rect_outline(bounds(0.0, 0.0, 100.0, 100.0), radii(50.0, 50.0, 0.0, 0.0));
        assert_eq!(outline.len(), 6);
        assert_eq!(outline[0].point, outline[1].point);
    }
}
