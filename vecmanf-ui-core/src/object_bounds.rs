//! The axis-aligned selection-box bounds of any object
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`, feature-local
//! decision "one bounds rule, used by both drawing and the tools"): a
//! primitive's own frame box
//! ([`vecmanf_document_core::shape_frame_bounds`]) or, for a path, the
//! union of [`vecmanf_geometry_core::segment_bounds`] over every segment —
//! tight to the curve's own extrema, not its (looser) control-point hull
//! (acceptance criterion 14). Reaches `vecmanf-render-core` as plain
//! rectangles in the decoration input, the same way `primitive-shapes`
//! passes handle positions.

use vecmanf_document_core::{ObjectSnapshot, PathSnapshot, Point, shape_frame_bounds};
use vecmanf_geometry_core::segment_bounds;

use crate::hit_test::segment_pairs;

fn path_bounds(path: &PathSnapshot) -> (Point, Point) {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (i, j) in segment_pairs(path.anchors.len(), path.closed) {
        let start = &path.anchors[i];
        let end = &path.anchors[j];
        let (seg_min, seg_max) =
            segment_bounds(start.point, start.handle_out, end.handle_in, end.point);
        min.x = min.x.min(seg_min.x);
        min.y = min.y.min(seg_min.y);
        max.x = max.x.max(seg_max.x);
        max.y = max.y.max(seg_max.y);
    }
    if min.x.is_infinite() {
        // A degenerate single-anchor path (should not exist in practice —
        // every stored path has at least two anchors — but this avoids an
        // infinite box rather than assuming it never happens).
        let only = path
            .anchors
            .first()
            .map_or(Point::new(0.0, 0.0), |a| a.point);
        return (only, only);
    }
    (min, max)
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

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{AnchorId, Document, Length, NewAnchor, RectBounds};

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
        use vecmanf_document_core::{AnchorKind, Vec2};

        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(0.0, 30.0),
                    kind: AnchorKind::Smooth,
                },
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(0.0, 30.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Smooth,
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
}
