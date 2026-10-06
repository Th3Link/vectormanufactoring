//! Flat-colored triangle geometry for decoration glyphs — squares,
//! diamonds, circles, thin rectangles — built directly rather than
//! through `lyon`'s fill tessellator: these are small, constant-shape UI
//! glyphs, not document geometry, so there is no curve to tessellate and
//! no [`vecmanf_document_core::Tolerance`] that could apply to them.
//!
//! Renamed from `primitives.rs` in `primitive-shapes`
//! (`specs/0003-primitive-shapes/adrs.md`: "after this slice, 'primitive'
//! means a document shape, and a module of that name holding UI glyphs
//! would mislead"). This module still means exactly what it always
//! did — UI glyph geometry — never a document primitive
//! ([`vecmanf_document_core::Shape`]); [`crate::shape_preview`] is where
//! a document primitive's own stroke and handles are built, reusing
//! these glyphs as its drawing primitives.

use vecmanf_document_core::{Point, Vec2};

use crate::color::RgbaColor;

/// One draw-list vertex: a document-space position plus a flat color.
/// No texture coordinate and no normal — every shape this crate produces
/// is a solid fill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// The vertex's document-space position (ADR 0001 §5: the view
    /// transform is applied once, uniformly, by whoever owns the GPU —
    /// never baked in here).
    pub position: Point,
    /// The vertex's flat color.
    pub color: RgbaColor,
}

/// A flat list of triangles: every three consecutive [`Vertex`]es form
/// one triangle. Ready for the host to upload as a single vertex buffer
/// (ADR 0001 §5: typed arrays, never JSON, cross the wasm boundary).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList {
    /// The triangle list. `triangles.len()` is always a multiple of 3.
    pub triangles: Vec<Vertex>,
}

impl DrawList {
    /// The number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.triangles.len() / 3
    }

    /// Appends another draw list's triangles to this one.
    pub fn extend(&mut self, other: Self) {
        self.triangles.extend(other.triangles);
    }

    fn push_triangle(&mut self, a: Point, b: Point, c: Point, color: RgbaColor) {
        self.triangles.push(Vertex { position: a, color });
        self.triangles.push(Vertex { position: b, color });
        self.triangles.push(Vertex { position: c, color });
    }

    /// Appends a quad given its four corners in order around its
    /// perimeter (either winding — this crate's glyphs are never culled
    /// by face direction).
    fn push_quad(&mut self, a: Point, b: Point, c: Point, d: Point, color: RgbaColor) {
        self.push_triangle(a, b, c, color);
        self.push_triangle(a, c, d, color);
    }
}

/// An axis-aligned square centered at `center`, `size_mm` wide — a
/// corner node glyph (`docs/design-system.md`).
#[must_use]
pub fn square(center: Point, size_mm: f64, color: RgbaColor) -> DrawList {
    let half = size_mm / 2.0;
    let mut list = DrawList::default();
    list.push_quad(
        center.translated(Vec2::new(-half, -half)),
        center.translated(Vec2::new(half, -half)),
        center.translated(Vec2::new(half, half)),
        center.translated(Vec2::new(-half, half)),
        color,
    );
    list
}

/// An axis-aligned square with rounded corners ("squircle"),
/// `size_mm` wide and corner radius `radius_mm`, centered at `center` —
/// `object-transform`'s resize-handle glyph (`docs/design-system.md`:
/// "8×8px... 2px corner radius"). `radius_mm` is clamped to half the
/// side; `0` degenerates to [`square`].
#[must_use]
pub fn rounded_square(center: Point, size_mm: f64, radius_mm: f64, color: RgbaColor) -> DrawList {
    const ARC_SEGMENTS: usize = 4;
    let half = size_mm / 2.0;
    let radius = radius_mm.clamp(0.0, half);
    // Corner arc centers and the angle (in Y-down radians) each arc
    // starts at, clockwise on screen: top-right, bottom-right,
    // bottom-left, top-left.
    let corners = [
        (
            Point::new(half - radius, -(half - radius)),
            -std::f64::consts::FRAC_PI_2,
        ),
        (Point::new(half - radius, half - radius), 0.0),
        (
            Point::new(-(half - radius), half - radius),
            std::f64::consts::FRAC_PI_2,
        ),
        (
            Point::new(-(half - radius), -(half - radius)),
            std::f64::consts::PI,
        ),
    ];
    let mut outline: Vec<Point> = Vec::with_capacity(4 * (ARC_SEGMENTS + 1));
    for (arc_center, start) in corners {
        for i in 0..=ARC_SEGMENTS {
            #[allow(clippy::cast_precision_loss)] // tiny compile-time constant
            let t = i as f64 / ARC_SEGMENTS as f64;
            let angle = start + t * std::f64::consts::FRAC_PI_2;
            outline.push(center.translated(Vec2::new(
                arc_center.x + radius * angle.cos(),
                arc_center.y + radius * angle.sin(),
            )));
        }
    }
    let mut list = DrawList::default();
    for i in 0..outline.len() {
        let next = (i + 1) % outline.len();
        list.push_triangle(center, outline[i], outline[next], color);
    }
    list
}

/// A circular-arrow icon: a `thickness_mm`-wide arc sweeping 270° around
/// `center` at `diameter_mm` outer diameter, ending in a triangular
/// arrowhead pointing clockwise — `object-transform`'s rotate-handle
/// glyph (`docs/design-system.md`: "circular-arrow icon glyph (not a
/// dot)").
#[must_use]
pub fn arc_arrow(center: Point, diameter_mm: f64, thickness_mm: f64, color: RgbaColor) -> DrawList {
    const SEGMENTS: usize = 12;
    let outer = diameter_mm / 2.0;
    let mid = (outer - thickness_mm / 2.0).max(0.0);
    let inner = (outer - thickness_mm).max(0.0);
    // Starts at the top-right (-60° from +x, Y-down), sweeps clockwise
    // 270° so the opening sits at the top-right, where the arrowhead
    // points back into it.
    let start = -std::f64::consts::FRAC_PI_3;
    let sweep = 1.5 * std::f64::consts::PI;
    let at = |radius: f64, t: f64| {
        let angle = start + t * sweep;
        center.translated(Vec2::new(radius * angle.cos(), radius * angle.sin()))
    };
    let mut list = DrawList::default();
    for i in 0..SEGMENTS {
        #[allow(clippy::cast_precision_loss)] // tiny compile-time constant
        let (t0, t1) = (i as f64 / SEGMENTS as f64, (i + 1) as f64 / SEGMENTS as f64);
        list.push_quad(
            at(outer, t0),
            at(outer, t1),
            at(inner, t1),
            at(inner, t0),
            color,
        );
    }
    // Arrowhead at the sweep's end, pointing along the clockwise tangent.
    let end_angle = start + sweep;
    let tangent = Vec2::new(-end_angle.sin(), end_angle.cos());
    let tip = at(mid, 1.0).translated(tangent.scaled(thickness_mm * 1.6));
    let base_half = thickness_mm * 1.5;
    list.push_triangle(
        tip,
        at(mid + base_half, 1.0),
        at((mid - base_half).max(0.0), 1.0),
        color,
    );
    list
}

/// A square rotated 45°, `size_mm` corner-to-corner, centered at
/// `center` — a smooth node glyph (`docs/design-system.md`).
#[must_use]
pub fn diamond(center: Point, size_mm: f64, color: RgbaColor) -> DrawList {
    let half = size_mm / 2.0;
    let mut list = DrawList::default();
    list.push_quad(
        center.translated(Vec2::new(0.0, -half)),
        center.translated(Vec2::new(half, 0.0)),
        center.translated(Vec2::new(0.0, half)),
        center.translated(Vec2::new(-half, 0.0)),
        color,
    );
    list
}

/// An equilateral triangle, point-up, centered at `center`, with its
/// three corners inscribed on a circle of radius `size_mm / 2` — the
/// same circumradius `diamond`'s own four corners sit on, so the two
/// glyphs read as comparably sized — an asymmetric node glyph
/// (`docs/design-system.md`). Three corners vs. `square`/`diamond`'s
/// four is a genuine silhouette difference, not a third rotation of the
/// same polygon (`specs/0006-path-merge-split-and-node-types/
/// specification.md`'s UX notes).
#[must_use]
pub fn triangle(center: Point, size_mm: f64, color: RgbaColor) -> DrawList {
    let radius = size_mm / 2.0;
    // The other two corners sit 120°/240° around from the top one;
    // `cos(30°) = √3/2` is this triangle's own half-width at that radius.
    let half_width = radius * 3.0_f64.sqrt() / 2.0;
    let mut list = DrawList::default();
    list.push_triangle(
        center.translated(Vec2::new(0.0, -radius)),
        center.translated(Vec2::new(half_width, radius * 0.5)),
        center.translated(Vec2::new(-half_width, radius * 0.5)),
        color,
    );
    list
}

/// A thin rectangle running from `a` to `b`, `width_mm` wide — a handle
/// line. Empty when `a` and `b` coincide (nothing to draw, not a
/// division by zero).
#[must_use]
pub fn thick_line(a: Point, b: Point, width_mm: f64, color: RgbaColor) -> DrawList {
    let direction = a.vector_to(b);
    let mut list = DrawList::default();
    if direction.length() <= f64::EPSILON {
        return list;
    }
    let normal = Vec2::new(-direction.y, direction.x).normalized_to(width_mm / 2.0);
    list.push_quad(
        a.translated(normal),
        b.translated(normal),
        b.translated(normal.negated()),
        a.translated(normal.negated()),
        color,
    );
    list
}

/// A closed four-corner outline, `width_mm` thick on every edge — the
/// selection/hover bounding-box decoration (`select_decoration.rs`,
/// `shape_preview.rs`). Takes corners rather than `(min, max)` so a
/// rotated object's box follows its own orientation (`object-transform`
/// acceptance criterion 18).
#[must_use]
pub fn quad_outline(corners: [Point; 4], width_mm: f64, color: RgbaColor) -> DrawList {
    let mut list = DrawList::default();
    for i in 0..4 {
        list.extend(thick_line(
            corners[i],
            corners[(i + 1) % 4],
            width_mm,
            color,
        ));
    }
    list
}

/// A filled circle centered at `center`, `diameter_mm` across — a handle
/// endpoint glyph.
#[must_use]
pub fn circle(center: Point, diameter_mm: f64, color: RgbaColor) -> DrawList {
    ring_or_disc(center, diameter_mm, None, color)
}

/// A ring (annulus), `diameter_mm` outer diameter and `thickness_mm`
/// thick, centered at `center` — the hover ring.
#[must_use]
pub fn ring(center: Point, diameter_mm: f64, thickness_mm: f64, color: RgbaColor) -> DrawList {
    ring_or_disc(center, diameter_mm, Some(thickness_mm), color)
}

fn ring_or_disc(
    center: Point,
    diameter_mm: f64,
    thickness_mm: Option<f64>,
    color: RgbaColor,
) -> DrawList {
    const SEGMENTS: usize = crate::theme::CIRCLE_SEGMENTS;
    let outer = diameter_mm / 2.0;
    let inner = thickness_mm.map(|t| (outer - t).max(0.0));
    // `SEGMENTS` is a small compile-time constant (`crate::theme::
    // CIRCLE_SEGMENTS`), nowhere near `f64`'s 52-bit mantissa limit.
    #[allow(clippy::cast_precision_loss)]
    let on_circle = |radius: f64, i: usize| {
        let angle = (i as f64) / (SEGMENTS as f64) * std::f64::consts::TAU;
        center.translated(Vec2::new(angle.cos() * radius, angle.sin() * radius))
    };

    let mut list = DrawList::default();
    for i in 0..SEGMENTS {
        let next = (i + 1) % SEGMENTS;
        match inner {
            None => list.push_triangle(center, on_circle(outer, i), on_circle(outer, next), color),
            Some(inner) => list.push_quad(
                on_circle(outer, i),
                on_circle(outer, next),
                on_circle(inner, next),
                on_circle(inner, i),
                color,
            ),
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_has_two_triangles_at_the_right_bounds() {
        let list = square(Point::new(0.0, 0.0), 2.0, RgbaColor::BLACK);
        assert_eq!(list.triangle_count(), 2);
        for vertex in &list.triangles {
            assert!(vertex.position.x.abs() <= 1.0 + 1e-9);
            assert!(vertex.position.y.abs() <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn rounded_square_stays_inside_its_square_and_cuts_the_corners() {
        let list = rounded_square(Point::new(0.0, 0.0), 8.0, 2.0, RgbaColor::BLACK);
        assert!(
            list.triangle_count() > 2,
            "more than a plain square's two triangles"
        );
        for vertex in &list.triangles {
            assert!(vertex.position.x.abs() <= 4.0 + 1e-9);
            assert!(vertex.position.y.abs() <= 4.0 + 1e-9);
            // No vertex sits on the sharp corner (4, 4).
            let near_corner = (vertex.position.x.abs() - 4.0).abs() < 1e-9
                && (vertex.position.y.abs() - 4.0).abs() < 1e-9;
            assert!(
                !near_corner,
                "corner must be rounded: {:?}",
                vertex.position
            );
        }
    }

    #[test]
    fn arc_arrow_is_an_open_arc_with_an_arrowhead_inside_its_diameter_budget() {
        let list = arc_arrow(Point::new(0.0, 0.0), 12.0, 1.5, RgbaColor::BLACK);
        assert!(list.triangle_count() > 12, "arc strip plus arrowhead");
        // A 270° arc: fewer arc triangles than a full ring's own.
        let full_ring = ring(Point::new(0.0, 0.0), 12.0, 1.5, RgbaColor::BLACK);
        assert!(list.triangle_count() < full_ring.triangle_count() * 2);
        for vertex in &list.triangles {
            let r = vertex.position.vector_to(Point::new(0.0, 0.0)).length();
            assert!(
                r <= 6.0 + 2.5,
                "arrowhead may flare outside the arc, but only slightly"
            );
        }
    }

    #[test]
    fn triangle_is_one_triangle_point_up_inscribed_in_the_box() {
        let list = triangle(Point::new(0.0, 0.0), 14.0, RgbaColor::BLACK);
        assert_eq!(list.triangle_count(), 1);
        for vertex in &list.triangles {
            assert!(vertex.position.x.abs() <= 7.0 + 1e-9);
            assert!(vertex.position.y.abs() <= 7.0 + 1e-9);
        }
        // Exactly one vertex sits at the top point (0, -radius); the
        // other two sit lower (point-up, not point-down or sideways).
        let top_count = list
            .triangles
            .iter()
            .filter(|v| v.position.y < -6.9)
            .count();
        assert_eq!(top_count, 1);
    }

    #[test]
    fn thick_line_between_coincident_points_is_empty() {
        let list = thick_line(
            Point::new(1.0, 1.0),
            Point::new(1.0, 1.0),
            1.0,
            RgbaColor::BLACK,
        );
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn thick_line_is_perpendicular_to_its_direction() {
        let list = thick_line(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            2.0,
            RgbaColor::BLACK,
        );
        assert_eq!(list.triangle_count(), 2);
        // A horizontal line's thickness is vertical: some vertex must sit
        // away from y=0.
        assert!(list.triangles.iter().any(|v| v.position.y.abs() > 0.5));
    }

    #[test]
    fn circle_approximates_the_requested_radius() {
        let list = circle(Point::new(0.0, 0.0), 10.0, RgbaColor::BLACK);
        assert_eq!(list.triangle_count(), crate::theme::CIRCLE_SEGMENTS);
        for vertex in &list.triangles {
            let r = vertex.position.vector_to(Point::new(0.0, 0.0)).length();
            assert!(r <= 5.0 + 1e-9);
        }
    }

    #[test]
    fn ring_has_an_outer_and_inner_radius() {
        let list = ring(Point::new(0.0, 0.0), 10.0, 2.0, RgbaColor::BLACK);
        let radii: Vec<f64> = list
            .triangles
            .iter()
            .map(|v| v.position.vector_to(Point::new(0.0, 0.0)).length())
            .collect();
        let max = radii.iter().copied().fold(0.0, f64::max);
        let min = radii.iter().copied().fold(f64::MAX, f64::min);
        assert!((max - 5.0).abs() < 1e-9);
        assert!((min - 3.0).abs() < 1e-9);
    }
}
