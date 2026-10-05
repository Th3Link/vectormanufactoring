//! Hit-testing one document-space point against every object in the
//! document — any kind, one entry point (`specs/0004-canvas-navigation-
//! and-selection/adrs.md`, feature-local decision "one object hit test,
//! no third copy"): [`crate::hit_test`]'s node/handle/segment test and
//! [`crate::shape_hit_test`]'s primitive-outline test already each
//! measure the distance from a point to a run of cubic segments; this
//! module is their union for "any object, selected as a whole" (a path
//! through its own anchors, a primitive through its outline).
//!
//! [`crate::shape_hit_test::hit_test_primitive`] is reimplemented on top
//! of [`hit_test_object`] rather than kept as a second, independent
//! definition of "near an outline" (`adrs.md`: "Rejected: having the
//! Select tool call `hit_test` and `hit_test_primitive` and compare
//! results. That keeps two definitions of 'near an outline'").

use vecmanf_document_core::{NodeId, ObjectSnapshot, Point, Tolerance, Vec2, outline_of_rotated};
use vecmanf_geometry_core::nearest_point_on_segment;

use crate::hit_test::segment_pairs;

/// The distance from `point` to the nearest point on any segment of an
/// anchor run of `len` anchors (`closed` or not), within `tolerance` — the
/// shared helper `adrs.md` calls for: "a private helper computes the
/// distance from a point to an anchor run (point, `handle_in`,
/// `handle_out`, closed flag) via `nearest_point_on_segment`." `get(i)`
/// returns anchor `i`'s own `(point, handle_in, handle_out)`.
fn nearest_distance_on_run<F>(
    len: usize,
    closed: bool,
    point: Point,
    tolerance: Tolerance,
    get: F,
) -> Option<f64>
where
    F: Fn(usize) -> (Point, Vec2, Vec2),
{
    let mut best: Option<f64> = None;
    for (i, j) in segment_pairs(len, closed) {
        let (start, _start_handle_in, start_handle_out) = get(i);
        let (end, end_handle_in, _end_handle_out) = get(j);
        let (_, distance, _) = nearest_point_on_segment(
            start,
            start_handle_out,
            end_handle_in,
            end,
            point,
            tolerance,
        );
        let distance = distance.as_mm();
        if distance > tolerance.as_mm() {
            continue;
        }
        best = Some(best.map_or(distance, |current: f64| current.min(distance)));
    }
    best
}

fn distance_to_object(object: &ObjectSnapshot, point: Point, tolerance: Tolerance) -> Option<f64> {
    match object {
        ObjectSnapshot::Path(path) => {
            nearest_distance_on_run(path.anchors.len(), path.closed, point, tolerance, |i| {
                let anchor = &path.anchors[i];
                (anchor.point, anchor.handle_in, anchor.handle_out)
            })
        }
        ObjectSnapshot::Primitive(primitive) => {
            let outline = outline_of_rotated(&primitive.shape, primitive.rotation);
            nearest_distance_on_run(outline.len(), true, point, tolerance, |i| {
                let anchor = &outline[i];
                (anchor.point, anchor.handle_in, anchor.handle_out)
            })
        }
    }
}

/// Hit-tests `point` against every object in `objects` — a path through
/// its own anchors, a primitive through its own (rotation-aware)
/// outline ([`vecmanf_document_core::outline_of_rotated`]) — returning
/// the nearest one
/// within `tolerance`. A tie goes to the topmost object in z-order:
/// `objects` is expected in z-order (as
/// [`vecmanf_document_core::Document::object_ids`] already returns it),
/// and the last-indexed (topmost) candidate wins an exact distance tie.
#[must_use]
pub fn hit_test_object(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<NodeId> {
    let mut best: Option<(f64, NodeId)> = None;
    for object in objects {
        let Some(distance) = distance_to_object(object, point, tolerance) else {
            continue;
        };
        let better = match best {
            None => true,
            // `<=`, not `<`: later entries are higher in z-order, so an
            // exact tie favors whichever object this loop reaches last.
            Some((best_distance, _)) => distance <= best_distance,
        };
        if better {
            best = Some((distance, object.id()));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{AnchorId, Document, EllipseFrame, Length, NewAnchor, RectBounds};

    fn open_path(document: &Document, a: Point, b: Point) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), a),
                NewAnchor::corner(AnchorId::new(1, 2), b),
            ],
            false,
        )
    }

    #[test]
    fn hits_a_path_near_its_segment() {
        let document = Document::new(1);
        let path = open_path(&document, Point::new(0.0, 0.0), Point::new(20.0, 0.0));
        let objects = vec![document.object(path).expect("exists")];
        let hit = hit_test_object(&objects, Point::new(10.0, 0.3), Tolerance::from_mm(1.0));
        assert_eq!(hit, Some(path));
    }

    #[test]
    fn hits_a_primitive_near_its_outline() {
        let document = Document::new(1);
        let rect = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let objects = vec![document.object(rect).expect("exists")];
        let hit = hit_test_object(&objects, Point::new(5.0, 0.2), Tolerance::from_mm(1.0));
        assert_eq!(hit, Some(rect));
    }

    #[test]
    fn misses_everything_far_from_any_object() {
        let document = Document::new(1);
        let path = open_path(&document, Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let objects = vec![document.object(path).expect("exists")];
        let hit = hit_test_object(
            &objects,
            Point::new(1000.0, 1000.0),
            Tolerance::from_mm(1.0),
        );
        assert_eq!(hit, None);
    }

    #[test]
    fn misses_the_unfilled_interior_of_a_primitive() {
        let document = Document::new(1);
        let ellipse = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(10.0),
        });
        let objects = vec![document.object(ellipse).expect("exists")];
        let hit = hit_test_object(&objects, Point::new(0.0, 0.0), Tolerance::from_mm(1.0));
        assert_eq!(hit, None);
    }

    /// Two different-kind objects, both within tolerance of the same
    /// click: the nearer one wins, regardless of kind.
    #[test]
    fn the_nearer_object_wins_regardless_of_kind() {
        let document = Document::new(1);
        let far_path = open_path(&document, Point::new(0.0, 5.0), Point::new(20.0, 5.0));
        let near_rect = document.create_rect(RectBounds {
            origin: Point::new(0.0, -0.2),
            width: Length::from_mm(20.0),
            height: Length::from_mm(0.4),
        });
        let objects = vec![
            document.object(far_path).expect("exists"),
            document.object(near_rect).expect("exists"),
        ];
        let hit = hit_test_object(&objects, Point::new(10.0, 0.0), Tolerance::from_mm(10.0));
        assert_eq!(hit, Some(near_rect));
    }

    /// A rotated primitive hit-tests against its *rotated* outline, not
    /// its unrotated local frame — a point on the original (unrotated)
    /// right edge is now empty space once the rectangle has turned 90
    /// degrees, while a point on what is now the rotated right edge
    /// hits.
    #[test]
    fn hit_test_respects_a_primitives_rotation() {
        let document = Document::new(1);
        let rect = document.create_rect(RectBounds {
            origin: Point::new(-5.0, -5.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(
                rect,
                Point::new(0.0, 0.0),
                vecmanf_document_core::Angle::from_radians(std::f64::consts::FRAC_PI_4),
            )
            .expect("rotate");
        let objects = vec![document.object(rect).expect("exists")];
        // The unrotated top edge sat at y = -5; after a 45-degree
        // rotation about the center, nothing is there any more.
        let miss = hit_test_object(&objects, Point::new(0.0, -5.0), Tolerance::from_mm(0.5));
        assert_eq!(miss, None, "the unrotated edge position is now empty");
    }

    /// An exact distance tie between two objects: the one listed last
    /// (topmost in z-order, matching `Document::object_ids`'s own sibling
    /// order) wins.
    #[test]
    fn an_exact_tie_favors_the_topmost_object() {
        let document = Document::new(1);
        let bottom = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let top = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        // Both rectangles are identical, so a click on either's outline
        // is exactly equidistant from both.
        let objects = vec![
            document.object(bottom).expect("exists"),
            document.object(top).expect("exists"),
        ];
        let hit = hit_test_object(&objects, Point::new(5.0, 0.0), Tolerance::from_mm(1.0));
        assert_eq!(
            hit,
            Some(top),
            "the later (topmost) object must win the tie"
        );
    }
}
