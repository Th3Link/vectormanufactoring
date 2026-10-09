//! Fit the document to the extent of all its objects
//! (`specs/0015-document-size-and-rulers/` criteria 22-27a): the content box
//! comes from here because curve extremes need `curvyo-geometry-core`, and
//! `curvyo-document-core` writes the size and moves the objects.

use curvyo_document_core::{Document, DocumentSizeError};

use crate::object_bounds::content_bounds;

/// Sets the document to the box around all objects' outlines with a 0 mm
/// margin and moves every object so the box starts at (0, 0), as one commit
/// (`Document::fit_to_content`). Returns `Ok(false)` and writes nothing for a
/// document without objects or one that already fits.
///
/// # Errors
/// [`DocumentSizeError::OutOfRange`] if the content is larger than the
/// largest document; nothing is changed then.
pub fn fit_document_to_content(document: &Document) -> Result<bool, DocumentSizeError> {
    match content_bounds(document) {
        Some(bounds) => document.fit_to_content(bounds),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        AnchorId, AnchorKind, Angle, DocumentSize, EllipseFrame, Length, NewAnchor, ObjectSnapshot,
        Point, RectBounds, Vec2,
    };

    fn bulging_path(document: &Document) -> curvyo_document_core::NodeId {
        document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(0.0, 40.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(10.0, 0.0),
                    handle_in: Vec2::new(0.0, 40.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Symmetric,
                },
            ],
            false,
        )
    }

    /// Criterion 23: a curve contributes its true extreme (3/4 of the handle
    /// length for this symmetric bulge, 30 mm), not its control points (40).
    #[test]
    fn a_curve_fits_to_its_true_extreme_not_its_control_points() {
        let document = Document::new(1);
        bulging_path(&document);

        assert_eq!(fit_document_to_content(&document), Ok(true));

        let size = document.size();
        assert!((size.width.as_mm() - 10.0).abs() < 1e-9);
        assert!((size.height.as_mm() - 30.0).abs() < 1e-9, "{size:?}");
    }

    /// Criterion 23: a rotated rectangle is measured on its rotated outline,
    /// objects on the pasteboard (negative coordinates) are included, and
    /// the stroke width is not.
    #[test]
    fn a_rotated_rectangle_and_the_pasteboard_are_included_and_the_stroke_is_not() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(-50.0, -20.0),
            width: Length::from_mm(20.0),
            height: Length::from_mm(10.0),
        });
        let object = document.object(id).unwrap();
        let center = Point::new(-40.0, -15.0);
        document
            .rotate_object(
                &object.rotated(center, Angle::from_radians(std::f64::consts::FRAC_PI_2)),
            )
            .unwrap();
        let _ = document.create_ellipse(EllipseFrame {
            center: Point::new(100.0, 100.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(10.0),
        });

        assert_eq!(fit_document_to_content(&document), Ok(true));

        // The rectangle is 10 wide and 20 tall once turned; its box starts at
        // x = -45, y = -25. The ellipse ends at 110, 110.
        let size = document.size();
        assert!((size.width.as_mm() - 155.0).abs() < 1e-9, "{size:?}");
        assert!((size.height.as_mm() - 135.0).abs() < 1e-9, "{size:?}");
        let bounds = content_bounds(&document).unwrap();
        assert!(
            bounds.0.x.abs() < 1e-9 && bounds.0.y.abs() < 1e-9,
            "{bounds:?}"
        );
    }

    /// Criterion 26: applying it again changes nothing and commits nothing.
    #[test]
    fn fitting_twice_changes_nothing_the_second_time() {
        let document = Document::new(1);
        bulging_path(&document);
        fit_document_to_content(&document).unwrap();
        let size = document.size();
        let objects: Vec<_> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();

        assert_eq!(fit_document_to_content(&document), Ok(false));

        assert_eq!(document.size(), size);
        let after: Vec<ObjectSnapshot> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();
        assert_eq!(after, objects);
    }

    #[test]
    fn an_empty_document_is_left_alone() {
        let document = Document::new(1);
        assert_eq!(fit_document_to_content(&document), Ok(false));
        assert_eq!(document.size(), DocumentSize::default());
        assert_eq!(content_bounds(&document), None);
    }

    /// Criterion 27a: content over 100 000 mm is refused and nothing changes.
    #[test]
    fn content_larger_than_the_largest_document_is_refused() {
        let document = Document::new(1);
        let _ = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(150_000.0),
            height: Length::from_mm(10.0),
        });

        assert_eq!(
            fit_document_to_content(&document),
            Err(DocumentSizeError::OutOfRange)
        );
        assert_eq!(document.size(), DocumentSize::default());
    }
}
