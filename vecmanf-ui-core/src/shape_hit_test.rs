//! Hit-testing a document-space point against primitives and their
//! handles (`specs/0003-primitive-shapes/specification.md`'s "reuses that
//! slice's... hit-testing tolerances"). Mirrors [`crate::hit_test`]'s
//! shape for paths: a primitive's own outline
//! ([`vecmanf_document_core::outline_of`]) is hit-tested the same way a
//! path's segments are, via `vecmanf-geometry-core`'s
//! `nearest_point_on_segment` — outline construction, not a second
//! geometry implementation.

use vecmanf_document_core::{NodeId, Point, PrimitiveSnapshot, Tolerance, outline_of};
use vecmanf_geometry_core::nearest_point_on_segment;

use crate::handle_layout::ShapeHandle;

/// Hit-tests `point` against every primitive's own outline, returning
/// the nearest one within `tolerance` — the selection convention
/// (`specification.md`: "the whole object is the selection unit").
#[must_use]
pub fn hit_test_primitive(
    primitives: &[PrimitiveSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<NodeId> {
    let mut best: Option<(f64, NodeId)> = None;
    for snapshot in primitives {
        let outline = outline_of(&snapshot.shape);
        if outline.len() < 2 {
            continue;
        }
        for i in 0..outline.len() {
            let j = (i + 1) % outline.len();
            let start = outline[i];
            let end = outline[j];
            let (_, distance, _) = nearest_point_on_segment(
                start.point,
                start.handle_out,
                end.handle_in,
                end.point,
                point,
                tolerance,
            );
            let distance = distance.as_mm();
            if distance > tolerance.as_mm() {
                continue;
            }
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, snapshot.id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Hit-tests `point` against a selected primitive's own handles,
/// returning the nearest one within `tolerance` (8px screen-space,
/// `docs/design-system.md`, same margin rule as the node tool's).
#[must_use]
pub fn hit_test_handle(
    handles: &[ShapeHandle],
    point: Point,
    tolerance: Tolerance,
) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (index, handle) in handles.iter().enumerate() {
        if !handle.draggable {
            continue;
        }
        let distance = handle.position.vector_to(point).length();
        if distance > tolerance.as_mm() {
            continue;
        }
        if best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, index));
        }
    }
    best.map(|(_, index)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, EllipseFrame, Length, RectBounds};

    #[test]
    fn hit_test_primitive_finds_a_point_near_the_outline() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let primitives = vec![document.primitive(id).expect("exists")];
        let hit = hit_test_primitive(&primitives, Point::new(5.0, 0.2), Tolerance::from_mm(1.0));
        assert_eq!(hit, Some(id));
    }

    #[test]
    fn hit_test_primitive_misses_the_interior_of_an_unfilled_shape() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(10.0),
        });
        let primitives = vec![document.primitive(id).expect("exists")];
        let hit = hit_test_primitive(&primitives, Point::new(0.0, 0.0), Tolerance::from_mm(1.0));
        assert_eq!(hit, None, "the center is nowhere near the outline");
    }
}
