//! The red hollow outline of the objects a boolean operation was refused for
//! (`specs/0016-boolean-operations` criteria 15 and 16): drawn over the artwork while the
//! refusal notice is shown, stored nowhere, selecting nothing.

use curvyo_document_core::{ObjectSnapshot, ViewTransform, outline_of_rotated};

use crate::glyphs::DrawList;
use crate::shape_preview::outline_to_anchors;
use crate::stroke;
use crate::theme;

/// A 2 px `--field-invalid` outline of every outline of every object in `objects`, over a white
/// casing, of constant screen width. Every casing comes first, then every line, so one object's
/// casing never covers another's line.
#[must_use]
pub fn build_refusal_outlines(objects: &[ObjectSnapshot], view: ViewTransform) -> DrawList {
    let width_mm = theme::REFUSAL_OUTLINE_STROKE_PX / view.scale();
    let tolerance_mm = theme::DISPLAY_TOLERANCE_PX / view.scale();
    let mut list = DrawList::default();
    for casing in [true, false] {
        let (width, color) = if casing {
            (
                width_mm * theme::CASING_WIDTH_FACTOR,
                theme::SELECTION_CASING,
            )
        } else {
            (width_mm, theme::REFUSAL_OUTLINE)
        };
        for object in objects {
            match object {
                ObjectSnapshot::Path(path) => {
                    for subpath in path.subpaths() {
                        list.extend(stroke::path_stroke(
                            subpath.anchors,
                            subpath.closed,
                            width,
                            color,
                            tolerance_mm,
                        ));
                    }
                }
                ObjectSnapshot::Primitive(primitive) => {
                    let anchors = outline_to_anchors(&outline_of_rotated(
                        &primitive.shape,
                        primitive.rotation,
                    ));
                    list.extend(stroke::path_stroke(
                        &anchors,
                        true,
                        width,
                        color,
                        tolerance_mm,
                    ));
                }
            }
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{AnchorId, Document, Length, NewAnchor, Point, RectBounds};

    fn view() -> ViewTransform {
        ViewTransform::new(2.0, Point::new(0.0, 0.0))
    }

    #[test]
    fn a_rectangle_and_an_open_path_are_outlined_in_red_over_white_only() {
        let document = Document::new(1);
        let rect = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let open = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(20.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(30.0, 5.0)),
            ],
            false,
        );
        let objects = [
            document.object(rect).unwrap(),
            document.object(open).unwrap(),
        ];
        let list = build_refusal_outlines(&objects, view());
        assert_ne!(list.triangle_count(), 0);
        assert!(
            list.triangles
                .iter()
                .all(|v| v.color == theme::REFUSAL_OUTLINE || v.color == theme::SELECTION_CASING)
        );
        assert!(
            list.triangles
                .iter()
                .all(|v| (v.position.x - 5.0).abs() > 2.0 || (v.position.y - 5.0).abs() > 2.0),
            "hollow: nothing in the middle of the rectangle"
        );
        assert_eq!(
            build_refusal_outlines(&[], view()).triangle_count(),
            0,
            "no object, nothing drawn"
        );
    }
}
