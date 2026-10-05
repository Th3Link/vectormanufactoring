//! A primitive's closed-form outline (`specs/0003-primitive-shapes/adrs.md`,
//! "outline construction lives in `vecmanf-document-core`"): one pure
//! function per shape, parameters in, [`OutlineAnchor`]s out, no curve
//! ever evaluated (flattened, projected, subdivided or intersected) —
//! only trigonometry and the fixed Bézier-circle constant `KAPPA`. Three
//! consumers reach these functions over edges that already exist:
//! `vecmanf-render-core` (stroking, acceptance criterion 16),
//! `vecmanf-ui-core` (hit-testing and "object to path", acceptance
//! criteria 17-20), and a later SVG exporter.
//!
//! Every outline here is closed, and direction is increasing angle in
//! Y-down document space — clockwise on screen, matching Inkscape's own
//! rect/ellipse/polygon → path conversion (`adrs.md`).

use crate::path_model::AnchorKind;
use crate::primitive_model::{EllipseFrame, PointCount, RectBounds, Shape, StarFrame};
use crate::units::{Length, Point, Vec2};

/// The standard cubic-Bézier approximation of a quarter circle:
/// `4 * (√2 - 1) / 3`. Used for ellipse quadrants (acceptance criterion
/// 19) and rounded-rectangle corners (acceptance criterion 18) alike.
#[allow(clippy::excessive_precision)]
pub const KAPPA: f64 = 0.552_284_749_830_793_4;

/// One outline anchor, structurally identical to
/// [`crate::path_model::NewAnchor`] minus the identity: an outline is a
/// pure function of a shape's parameters and has no [`crate::AnchorId`]
/// of its own until a caller (`vecmanf-ui-core`, converting one to a
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
    const fn corner(point: Point) -> Self {
        Self {
            point,
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind: AnchorKind::Corner,
        }
    }
}

/// A rectangle's effective corner radius: stored raw, clamped only here
/// (`adrs.md`'s "the corner radius is stored raw and clamped where it is
/// evaluated" decision) — acceptance criterion 5. Every consumer
/// (outline, rendering, hit-testing, handle placement, conversion) calls
/// this rather than reading `corner_radius` directly.
#[must_use]
pub fn effective_corner_radius(bounds: RectBounds, corner_radius: Length) -> Length {
    let half_shorter_side = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
    Length::from_mm(corner_radius.as_mm().clamp(0.0, half_shorter_side.max(0.0)))
}

/// A rectangle's outline (acceptance criteria 1-6, 18): 4 sharp corners
/// at effective radius 0, or 8 tangent points (all [`AnchorKind::Corner`]
/// — a line-to-arc join cannot be [`AnchorKind::Smooth`], see `adrs.md`)
/// once rounded.
#[must_use]
pub fn rect_outline(bounds: RectBounds, corner_radius: Length) -> Vec<OutlineAnchor> {
    let radius = effective_corner_radius(bounds, corner_radius).as_mm();
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
    let corner = AnchorKind::Corner;
    vec![
        // Top edge, left tangent point — end of the top-left arc.
        OutlineAnchor {
            point: Point::new(left + radius, top),
            handle_in: Vec2::new(-handle, 0.0),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        // Top edge, right tangent point — start of the top-right arc.
        OutlineAnchor {
            point: Point::new(left + width - radius, top),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(handle, 0.0),
            kind: corner,
        },
        // Right edge, top tangent point — end of the top-right arc.
        OutlineAnchor {
            point: Point::new(left + width, top + radius),
            handle_in: Vec2::new(0.0, -handle),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        // Right edge, bottom tangent point — start of the bottom-right arc.
        OutlineAnchor {
            point: Point::new(left + width, top + height - radius),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(0.0, handle),
            kind: corner,
        },
        // Bottom edge, right tangent point — end of the bottom-right arc.
        OutlineAnchor {
            point: Point::new(left + width - radius, top + height),
            handle_in: Vec2::new(handle, 0.0),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        // Bottom edge, left tangent point — start of the bottom-left arc.
        OutlineAnchor {
            point: Point::new(left + radius, top + height),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(-handle, 0.0),
            kind: corner,
        },
        // Left edge, bottom tangent point — end of the bottom-left arc.
        OutlineAnchor {
            point: Point::new(left, top + height - radius),
            handle_in: Vec2::new(0.0, handle),
            handle_out: Vec2::ZERO,
            kind: corner,
        },
        // Left edge, top tangent point — start of the top-left arc
        // (closing the loop back to the first anchor).
        OutlineAnchor {
            point: Point::new(left, top + radius),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(0.0, -handle),
            kind: corner,
        },
    ]
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
    let smooth = AnchorKind::Smooth;
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
            corner_radius,
        } => rect_outline(bounds, corner_radius),
        Shape::Ellipse { frame } => ellipse_outline(frame),
        Shape::Polygon { frame, point_count } => polygon_outline(frame, point_count),
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => star_outline(frame, point_count, inner_ratio),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive_model::InnerRatio;

    fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        }
    }

    /// AC5: the effective radius never exceeds half the shorter side,
    /// even when the stored value is far larger.
    #[test]
    fn effective_corner_radius_clamps_to_half_the_shorter_side() {
        let b = bounds(0.0, 0.0, 10.0, 20.0);
        let effective = effective_corner_radius(b, Length::from_mm(100.0));
        assert!((effective.as_mm() - 5.0).abs() < 1e-9);
    }

    /// `adrs.md`'s "clamped on read, not on write" round-trip: shrinking
    /// a rectangle and growing it back restores the original radius,
    /// because the stored value was never rewritten.
    #[test]
    fn shrink_then_regrow_restores_the_original_radius() {
        let original_bounds = bounds(0.0, 0.0, 20.0, 20.0);
        let stored_radius = Length::from_mm(8.0);
        // Shrink: the effective radius clamps down...
        let shrunk = bounds(0.0, 0.0, 10.0, 10.0);
        let effective_when_shrunk = effective_corner_radius(shrunk, stored_radius);
        assert!((effective_when_shrunk.as_mm() - 5.0).abs() < 1e-9);
        // ...but the stored value itself is untouched, so growing back
        // gives the original radius again.
        let effective_when_regrown = effective_corner_radius(original_bounds, stored_radius);
        assert!((effective_when_regrown.as_mm() - 8.0).abs() < 1e-9);
    }

    /// AC18: zero radius is exactly 4 corner nodes, straight segments.
    #[test]
    fn rect_outline_at_zero_radius_has_four_corner_nodes() {
        let outline = rect_outline(bounds(0.0, 0.0, 10.0, 10.0), Length::from_mm(0.0));
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
        let outline = rect_outline(bounds(0.0, 0.0, 20.0, 10.0), Length::from_mm(2.0));
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
        assert!(outline.iter().all(|a| a.kind == AnchorKind::Smooth));
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
}
