//! Hit-testing a document-space point against primitives and their
//! handles (`specs/0003-primitive-shapes/specification.md`'s "reuses that
//! slice's... hit-testing tolerances").
//!
//! [`hit_test_primitive`] is [`crate::hit_test_object`]'s one "near an
//! outline" definition, restricted to the active shape tool's own kind
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`: "`hit_test_
//! primitive` becomes the same call restricted to the active shape
//! tool's kind... Rejected: having the Select tool call `hit_test` and
//! `hit_test_primitive` and compare results. That keeps two definitions
//! of 'near an outline'"), not a second, independent implementation of
//! "near a primitive's outline".

use vecmanf_document_core::{NodeId, ObjectSnapshot, Point, PrimitiveSnapshot, Tolerance};

use crate::handle_layout::ShapeHandle;
use crate::hit_test_object::hit_test_object;

/// Hit-tests `point` against every primitive's own outline, returning
/// the nearest one within `tolerance` — the selection convention
/// (`specification.md`: "the whole object is the selection unit").
/// `primitives` is already filtered to one kind by the caller (e.g.
/// `rects_only`); [`hit_test_object`] does the actual outline distance
/// work.
#[must_use]
pub fn hit_test_primitive(
    primitives: &[PrimitiveSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<NodeId> {
    let objects: Vec<ObjectSnapshot> = primitives
        .iter()
        .copied()
        .map(ObjectSnapshot::Primitive)
        .collect();
    hit_test_object(&objects, point, tolerance)
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
