//! The axis-aligned selection-box bounds of any object
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`, feature-local
//! decision "one bounds rule, used by both drawing and the tools"): a
//! primitive's own frame box
//! ([`curvyo_document_core::shape_frame_bounds`]) or, for a path, the
//! union of [`curvyo_geometry_core::segment_bounds`] over every segment —
//! tight to the curve's own extrema, not its (looser) control-point hull
//! (acceptance criterion 14). Reaches `curvyo-render-core` as plain
//! rectangles in the decoration input, the same way `primitive-shapes`
//! passes handle positions.

use curvyo_document_core::{
    ObjectSnapshot, PathSnapshot, Point, Vec2, outline_of_rotated, shape_frame_bounds,
};
use curvyo_geometry_core::segment_bounds;

use crate::hit_test::segment_pairs;

/// The tight bounds of a closed or open chain of anchors given as
/// `(point, handle_in, handle_out)`: the union of every segment's own
/// extrema.
fn anchors_bounds(anchors: &[(Point, Vec2, Vec2)], closed: bool) -> (Point, Point) {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (i, j) in segment_pairs(anchors.len(), closed) {
        let (start, _, handle_out) = anchors[i];
        let (end, handle_in, _) = anchors[j];
        let (seg_min, seg_max) = segment_bounds(start, handle_out, handle_in, end);
        min.x = min.x.min(seg_min.x);
        min.y = min.y.min(seg_min.y);
        max.x = max.x.max(seg_max.x);
        max.y = max.y.max(seg_max.y);
    }
    if min.x.is_infinite() {
        // A degenerate single-anchor path (should not exist in practice —
        // every stored path has at least two anchors — but this avoids an
        // infinite box rather than assuming it never happens).
        let only = anchors.first().map_or(Point::new(0.0, 0.0), |a| a.0);
        return (only, only);
    }
    (min, max)
}

fn path_bounds(path: &PathSnapshot) -> (Point, Point) {
    let anchors: Vec<_> = path
        .anchors
        .iter()
        .map(|a| (a.point, a.handle_in, a.handle_out))
        .collect();
    anchors_bounds(&anchors, path.closed)
}

/// The selection-box bounds of `object`, whichever kind it is (acceptance
/// criterion 14: "extended here to paths too").
#[must_use]
pub fn object_bounds(object: &ObjectSnapshot) -> (Point, Point) {
    match object {
        ObjectSnapshot::Path(path) => path_bounds(path),
        ObjectSnapshot::Primitive(primitive) => shape_frame_bounds(&primitive.shape),
    }
}

/// The tight bounds of the outline `object` is drawn with, in document space
/// (`specs/0010-edit-interaction-polish/` criterion 21, the reference of the typed
/// absolute move): a path's curve-accurate bounds, and for a primitive the
/// bounds of its rotated outline, so a rotated rectangle and a star are
/// measured on what is on screen. Not [`object_bounds`] (a primitive's
/// unrotated frame box) and without the stroke width.
#[must_use]
pub fn object_outline_bounds(object: &ObjectSnapshot) -> (Point, Point) {
    match object {
        ObjectSnapshot::Path(path) => path_bounds(path),
        ObjectSnapshot::Primitive(primitive) => {
            let anchors: Vec<_> = outline_of_rotated(&primitive.shape, primitive.rotation)
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect();
            anchors_bounds(&anchors, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{AnchorId, Document, Length, NewAnchor, RectBounds};

    #[test]
    fn a_straight_path_bounds_to_its_own_extent() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 5.0)),
            ],
            false,
        );
        let object = document.object(id).expect("exists");
        let (min, max) = object_bounds(&object);
        assert_eq!(min, Point::new(0.0, 0.0));
        assert_eq!(max, Point::new(10.0, 5.0));
    }

    /// Tight bounds, not the (looser) control-point hull: a curve whose
    /// handles bulge well past its own two endpoints must bound to the
    /// curve's real extent.
    #[test]
    fn a_curved_path_bounds_tightly_to_its_curve_not_its_control_hull() {
        use curvyo_document_core::{AnchorKind, Vec2};

        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(0.0, 30.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(0.0, 30.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Symmetric,
                },
            ],
            false,
        );
        let object = document.object(id).expect("exists");
        let (_, max) = object_bounds(&object);
        // Both endpoints sit at y=0; the handles pull the curve well
        // below that (Y-down), so the tight bound must reflect the
        // bulge, not just the endpoints.
        assert!(
            max.y > 5.0,
            "bounds must include the curve's bulge: {max:?}"
        );
    }

    #[test]
    fn a_primitive_bounds_to_its_frame() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(1.0, 2.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(5.0),
        });
        let object = document.object(id).expect("exists");
        let (min, max) = object_bounds(&object);
        assert_eq!(min, Point::new(1.0, 2.0));
        assert_eq!(max, Point::new(11.0, 7.0));
    }

    /// Criterion 21: the outline bounds follow the drawn, rotated shape. A
    /// 20 x 10 rectangle turned by 90 degrees about its centre stands 10 wide
    /// and 20 high; the frame box would still say 20 x 10.
    #[test]
    fn a_rotated_rectangle_bounds_to_its_drawn_outline() {
        use curvyo_document_core::Angle;
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(10.0, 20.0),
            width: Length::from_mm(20.0),
            height: Length::from_mm(10.0),
        });
        let turned = document.object(id).expect("exists").rotated(
            Point::new(20.0, 25.0),
            Angle::from_radians(std::f64::consts::FRAC_PI_2),
        );
        document.rotate_object(&turned).expect("rotates");
        let object = document.object(id).expect("exists");
        let (min, max) = object_outline_bounds(&object);
        for (got, want) in [(min.x, 15.0), (min.y, 15.0), (max.x, 25.0), (max.y, 35.0)] {
            assert!((got - want).abs() < 1e-9, "{got} vs {want}");
        }
        assert_eq!(
            object_bounds(&object).1.x - object_bounds(&object).0.x,
            20.0,
            "the frame box ignores the rotation"
        );
    }

    /// Criterion 21: a star's bounds are the box around its vertices, not its
    /// circumscribed square. A 5-point star with its first tip straight up
    /// (-90 degrees) has the tip at the top, y = centre - R, and the lowest
    /// points at the two bottom tips, below the centre by R cos 36 degrees.
    #[test]
    fn a_star_bounds_to_its_vertices_not_its_circumscribed_square() {
        use curvyo_document_core::{Angle, InnerRatio, PointCount, StarFrame};
        let document = Document::new(1);
        let id = document.create_star(
            StarFrame {
                center: Point::new(100.0, 50.0),
                radius: Length::from_mm(10.0),
                angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
            },
            PointCount::new(5).expect("count"),
            InnerRatio::new(0.5).expect("ratio"),
        );
        let object = document.object(id).expect("exists");
        let (min, max) = object_outline_bounds(&object);
        let want_bottom = 50.0 + 10.0 * 36.0_f64.to_radians().cos();
        assert!((min.y - 40.0).abs() < 1e-9, "tip at the top: {min:?}");
        assert!((max.y - want_bottom).abs() < 1e-9, "{max:?}");
        assert!(max.y < 60.0, "smaller than the circumscribed square");
        let half_width = 10.0 * 18.0_f64.to_radians().cos();
        assert!((min.x - (100.0 - half_width)).abs() < 1e-9, "{min:?}");
        assert!((max.x - (100.0 + half_width)).abs() < 1e-9, "{max:?}");
    }

    /// A circle's outline bounds are curve-accurate (its Bezier extrema), so
    /// they equal its box to tolerance.
    #[test]
    fn an_ellipse_bounds_to_its_curve() {
        use curvyo_document_core::EllipseFrame;
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(30.0, 40.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(5.0),
        });
        let object = document.object(id).expect("exists");
        let (min, max) = object_outline_bounds(&object);
        for (got, want) in [(min.x, 20.0), (min.y, 35.0), (max.x, 40.0), (max.y, 45.0)] {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
    }

    /// A curved path measures its curve, as the box does.
    #[test]
    fn a_path_outline_bounds_equal_its_selection_bounds() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(3.0, 4.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(13.0, 9.0)),
            ],
            false,
        );
        let object = document.object(id).expect("exists");
        assert_eq!(object_outline_bounds(&object), object_bounds(&object));
    }
}
