//! Hit-testing one document-space point against every object in the
//! document — any kind, one entry point (`specs/0004-canvas-navigation-
//! and-selection/adrs.md`, feature-local decision "one object hit test,
//! no third copy"): [`crate::hit_test`]'s node/handle/segment test measures
//! the distance from a point to a run of cubic segments, and this module
//! applies the same measure to "any object, selected as a whole" (a path
//! through its own anchors, a primitive through its outline). It is the one
//! definition of "near an outline"; the shape tools no longer hit-test at all.

use curvyo_document_core::{
    NodeId, ObjectSnapshot, Point, Style, Tolerance, Vec2, outline_of_rotated,
};
use curvyo_geometry_core::{OutlineTriple, contains_point, nearest_point_on_segment};

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

/// An object's outline as `(point, handle_in, handle_out)` triples and
/// whether it is closed: a path's own anchors, a primitive's rotation-aware
/// outline.
fn outline_of(object: &ObjectSnapshot) -> (Vec<OutlineTriple>, bool) {
    match object {
        ObjectSnapshot::Path(path) => (
            path.anchors
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect(),
            path.closed,
        ),
        ObjectSnapshot::Primitive(primitive) => (
            outline_of_rotated(&primitive.shape, primitive.rotation)
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect(),
            true,
        ),
    }
}

const fn style_of(object: &ObjectSnapshot) -> &Style {
    match object {
        ObjectSnapshot::Path(path) => &path.style,
        ObjectSnapshot::Primitive(primitive) => &primitive.style,
    }
}

/// Whether `object` paints a fill whose interior contains `point`: the
/// object takes part exactly when the renderer would paint a fill for it
/// (`Fill::paints`; opacity 0 still counts), and the point lies in the area
/// that fill covers. An open path's interior is closed with a chord, a closed
/// one's is bounded by its real closing segment (acceptance criterion 23).
fn fills_point(object: &ObjectSnapshot, point: Point) -> bool {
    if !style_of(object).fill.paints() {
        return false;
    }
    let (outline, closed) = outline_of(object);
    // Cheap reject first: a point outside the control hull's box is outside
    // the shape, so a hover over thousands of objects does not run the
    // winding test on every one.
    let mut hull = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (p, handle_in, handle_out) in &outline {
        for q in [*p, p.translated(*handle_in), p.translated(*handle_out)] {
            hull = (
                hull.0.min(q.x),
                hull.1.min(q.y),
                hull.2.max(q.x),
                hull.3.max(q.y),
            );
        }
    }
    if point.x < hull.0 || point.x > hull.2 || point.y < hull.1 || point.y > hull.3 {
        return false;
    }
    contains_point(&outline, closed, point)
}

/// Hit-tests `point` against every object in `objects` — a path through
/// its own anchors, a primitive through its own (rotation-aware)
/// outline ([`curvyo_document_core::outline_of_rotated`]) — returning the one
/// object a click there selects (`specs/0007-stroke-and-fill-styling`,
/// criterion 27). `objects` is expected in z-order (as
/// [`curvyo_document_core::Document::object_ids`] already returns it).
///
/// Let F be the topmost object whose **filled interior** contains the point.
/// Among the objects at or above F (all objects when there is no F) the one
/// whose **outline** is nearest to the point within `tolerance` wins, an exact
/// distance tie going to the topmost. If no outline is within tolerance, F
/// wins. An object below F never wins, because F covers it. When nothing under
/// the point is filled this is exactly the earlier rule: nearest outline, a
/// tie to the topmost.
#[must_use]
pub fn hit_test_object(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<NodeId> {
    let floor = objects
        .iter()
        .rposition(|object| fills_point(object, point));
    let mut best: Option<(f64, NodeId)> = None;
    for object in &objects[floor.unwrap_or(0)..] {
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
        .or_else(|| floor.map(|index| objects[index].id()))
}

/// The topmost object above `selected` in tree order whose filled interior
/// contains `point` (criterion 29): the object a plain press inside the sole
/// selected object's box goes to instead of moving the selection. An outline
/// of another object, an unfilled object and an object below `selected` never
/// qualify. `None` when `selected` is not among `objects`.
#[must_use]
pub(crate) fn filled_interior_above(
    objects: &[ObjectSnapshot],
    selected: NodeId,
    point: Point,
) -> Option<NodeId> {
    let index = objects.iter().position(|object| object.id() == selected)?;
    objects[index + 1..]
        .iter()
        .rev()
        .find(|object| fills_point(object, point))
        .map(ObjectSnapshot::id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{AnchorId, Document, EllipseFrame, Length, NewAnchor, RectBounds};

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

    /// Regression: on a 40x20 rect only edge points near the segment's
    /// t = 0, 0.5 and 1 used to hit (6/39 at 100 % zoom), because the hit
    /// tolerance doubled as the nearest-point search accuracy.
    #[test]
    fn hits_every_point_along_a_primitive_edge_at_any_hit_tolerance() {
        let document = Document::new(1);
        let rect = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(20.0),
        });
        let objects = vec![document.object(rect).expect("exists")];
        for tolerance_mm in [0.35, 1.06, 2.5] {
            let tolerance = Tolerance::from_mm(tolerance_mm);
            for step in 1..40 {
                let x = f64::from(step);
                assert_eq!(
                    hit_test_object(&objects, Point::new(x, 0.0), tolerance),
                    Some(rect),
                    "top edge x={x} tolerance={tolerance_mm}"
                );
                assert_eq!(
                    hit_test_object(&objects, Point::new(x, 20.0), tolerance),
                    Some(rect),
                    "bottom edge x={x} tolerance={tolerance_mm}"
                );
                assert_eq!(
                    hit_test_object(&objects, Point::new(x, -(tolerance_mm + 0.5)), tolerance),
                    None,
                    "just outside the top edge x={x} tolerance={tolerance_mm}"
                );
            }
        }
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
            .rotate_object(&document.object(rect).expect("object exists").rotated(
                Point::new(0.0, 0.0),
                curvyo_document_core::Angle::from_radians(std::f64::consts::FRAC_PI_4),
            ))
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

    // -----------------------------------------------------------------
    // `0007` criteria 23, 27 and 29: a filled interior is hittable
    // -----------------------------------------------------------------

    use curvyo_document_core::{
        FillMode, FillModeTarget, GradientStop, Opacity, StopId, StyleEdit,
    };

    fn fill(document: &Document, id: NodeId) {
        document
            .set_fill_mode(
                FillMode::Solid,
                &[FillModeTarget {
                    id,
                    seed_stops: vec![],
                }],
            )
            .expect("fill on");
    }

    fn square(document: &Document, x: f64, y: f64, size: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(size),
            height: Length::from_mm(size),
        })
    }

    fn circle(document: &Document, x: f64, y: f64, r: f64) -> NodeId {
        document.create_ellipse(EllipseFrame {
            center: Point::new(x, y),
            rx: Length::from_mm(r),
            ry: Length::from_mm(r),
        })
    }

    fn objects_of(document: &Document) -> Vec<ObjectSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect()
    }

    const TOL: Tolerance = Tolerance::from_mm(1.0);

    #[test]
    fn ac23_the_interior_of_a_filled_object_is_hit_and_an_unfilled_one_is_not() {
        let document = Document::new(1);
        let hollow = square(&document, 0.0, 0.0, 20.0);
        let filled = square(&document, 100.0, 0.0, 20.0);
        fill(&document, filled);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(110.0, 10.0), TOL),
            Some(filled)
        );
        assert_eq!(hit_test_object(&objects, Point::new(10.0, 10.0), TOL), None);
        assert_eq!(
            hit_test_object(&objects, Point::new(0.0, 10.0), TOL),
            Some(hollow)
        );
        assert_eq!(
            hit_test_object(&objects, Point::new(130.0, 10.0), TOL),
            None
        );
    }

    #[test]
    fn ac23_a_fill_at_opacity_zero_is_still_hit() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 0.0, 20.0);
        fill(&document, id);
        document
            .edit_style(&[id], &StyleEdit::FillOpacity(Opacity::new(0.0).unwrap()))
            .expect("opacity");
        assert_eq!(
            hit_test_object(&objects_of(&document), Point::new(10.0, 10.0), TOL),
            Some(id)
        );
    }

    #[test]
    fn ac23_a_gradient_without_stops_paints_nothing_and_is_not_hit() {
        let document = Document::new(1);
        let id = square(&document, 0.0, 0.0, 20.0);
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id,
                    seed_stops: vec![],
                }],
            )
            .expect("gradient mode");
        assert_eq!(
            hit_test_object(&objects_of(&document), Point::new(10.0, 10.0), TOL),
            None
        );
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id,
                    seed_stops: GradientStop::default_pair(
                        curvyo_document_core::Color::BLACK,
                        StopId::new(1, 1),
                        StopId::new(1, 2),
                    )
                    .to_vec(),
                }],
            )
            .expect("seeded");
        assert_eq!(
            hit_test_object(&objects_of(&document), Point::new(10.0, 10.0), TOL),
            Some(id)
        );
    }

    #[test]
    fn ac23_an_open_filled_path_is_hit_inside_the_chord_closed_area() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(0.0, 20.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(20.0, 20.0)),
                NewAnchor::corner(AnchorId::new(1, 4), Point::new(20.0, 0.0)),
            ],
            false,
        );
        fill(&document, id);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(10.0, 10.0), TOL),
            Some(id)
        );
        assert_eq!(
            hit_test_object(&objects, Point::new(10.0, 5.0), TOL),
            Some(id),
            "above the bottom edge, inside the chord-closed area"
        );
        assert_eq!(hit_test_object(&objects, Point::new(10.0, -5.0), TOL), None);
    }

    /// Criterion 27, first example: a filled rectangle with a hollow ellipse
    /// above it.
    #[test]
    fn ac27_a_click_on_the_ellipse_outline_selects_it_and_inside_it_selects_the_rectangle() {
        let document = Document::new(1);
        let rectangle = square(&document, 0.0, 0.0, 100.0);
        fill(&document, rectangle);
        let ellipse = circle(&document, 50.0, 50.0, 20.0);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(70.0, 50.0), TOL),
            Some(ellipse)
        );
        assert_eq!(
            hit_test_object(&objects, Point::new(50.0, 50.0), TOL),
            Some(rectangle)
        );
    }

    /// Criterion 27, second example: a hollow circle below a filled, opaque
    /// rectangle; its outline under the rectangle is covered.
    #[test]
    fn ac27_an_outline_hidden_under_a_filled_object_does_not_win() {
        let document = Document::new(1);
        let circle_id = circle(&document, 50.0, 50.0, 20.0);
        let rectangle = square(&document, 0.0, 0.0, 100.0);
        fill(&document, rectangle);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(70.0, 50.0), TOL),
            Some(rectangle),
            "the circle's outline lies under the rectangle"
        );
        // Outside the rectangle the circle is not involved either way; and a
        // circle that sticks out of it is hit there.
        let _ = circle_id;
        let wide = circle(&document, 100.0, 50.0, 20.0);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(120.0, 50.0), TOL),
            Some(wide)
        );
    }

    #[test]
    fn ac27_the_nearest_outline_among_those_at_or_above_the_floor_wins() {
        let document = Document::new(1);
        let below = square(&document, 0.0, 0.0, 100.0);
        let floor = square(&document, 10.0, 10.0, 80.0);
        fill(&document, floor);
        // A hollow square above the floor whose outline is nearer to the
        // click than the floor's own.
        let above = square(&document, 30.0, 30.0, 40.0);
        let objects = objects_of(&document);
        let _ = below;
        assert_eq!(
            hit_test_object(&objects, Point::new(30.0, 50.0), TOL),
            Some(above)
        );
        // Click on the floor's own outline, where the hollow square is far.
        assert_eq!(
            hit_test_object(&objects, Point::new(10.0, 50.0), TOL),
            Some(floor)
        );
        // Inside the floor and away from every outline: the floor wins.
        assert_eq!(
            hit_test_object(&objects, Point::new(20.0, 50.0), TOL),
            Some(floor)
        );
    }

    #[test]
    fn ac27_with_two_filled_objects_the_topmost_floor_wins_inside_both() {
        let document = Document::new(1);
        let lower = square(&document, 0.0, 0.0, 100.0);
        fill(&document, lower);
        let upper = square(&document, 20.0, 20.0, 60.0);
        fill(&document, upper);
        let objects = objects_of(&document);
        assert_eq!(
            hit_test_object(&objects, Point::new(50.0, 50.0), TOL),
            Some(upper)
        );
        assert_eq!(
            hit_test_object(&objects, Point::new(5.0, 50.0), TOL),
            Some(lower)
        );
    }

    #[test]
    fn ac29_only_a_filled_interior_above_the_selected_object_qualifies() {
        let document = Document::new(1);
        let selected = square(&document, 0.0, 0.0, 100.0);
        fill(&document, selected);
        let small = circle(&document, 50.0, 50.0, 5.0);
        fill(&document, small);
        let hollow_above = circle(&document, 20.0, 20.0, 5.0);
        let objects = objects_of(&document);
        assert_eq!(
            filled_interior_above(&objects, selected, Point::new(50.0, 50.0)),
            Some(small)
        );
        assert_eq!(
            filled_interior_above(&objects, selected, Point::new(20.0, 20.0)),
            None,
            "an unfilled object above does not qualify"
        );
        assert_eq!(
            filled_interior_above(&objects, selected, Point::new(25.0, 20.0)),
            None,
            "nor does its outline"
        );
        assert_eq!(
            filled_interior_above(&objects, small, Point::new(50.0, 50.0)),
            None,
            "the filled square lies below the circle"
        );
        document.delete_objects(&[hollow_above]).expect("delete");
        assert_eq!(
            filled_interior_above(&objects_of(&document), hollow_above, Point::new(0.0, 0.0)),
            None,
            "an id that is gone"
        );
    }
}
