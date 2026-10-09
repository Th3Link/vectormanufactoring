//! The "new" half of blue-new, black-old
//! (`specs/0009-unified-object-editing/specification.md`, criteria 10 to 15): the
//! geometry a release would commit, drawn as a hollow `--preview-new` outline
//! of constant screen width over the unchanged committed objects. The
//! committed objects are not touched here; "black old" is the absence of a
//! second render path, because the document's own strokes are drawn exactly
//! as they are.

use curvyo_document_core::{ObjectSnapshot, ViewTransform};

use crate::glyphs::DrawList;
use crate::shape_preview::shape_live_outline;
use crate::stroke;
use crate::theme;

/// The hollow `--preview-new` outline of every object in `objects` (the
/// resolved geometry of a Select-tool drag or bar edit): a primitive through
/// its rotated outline, a path through its anchors. 1.5 px wide on screen
/// whatever the object's own stroke width; no fill preview.
#[must_use]
pub fn build_live_edit_preview(objects: &[ObjectSnapshot], view: ViewTransform) -> DrawList {
    let width_mm = theme::LIVE_PREVIEW_STROKE_PX / view.scale();
    let tolerance_mm = theme::DISPLAY_TOLERANCE_PX / view.scale();
    let mut list = DrawList::default();
    // Every casing first, then every line: the casing of one object never
    // covers the line of another (`0007` criterion 40; the outline is always
    // above all artwork because it belongs to the overlay).
    for casing in [true, false] {
        for object in objects {
            list.extend(match object {
                ObjectSnapshot::Primitive(primitive) => {
                    shape_live_outline(&primitive.shape, primitive.rotation, view, casing)
                }
                ObjectSnapshot::Path(path) => stroke::path_stroke(
                    &path.anchors,
                    path.closed,
                    if casing {
                        width_mm * theme::CASING_WIDTH_FACTOR
                    } else {
                        width_mm
                    },
                    if casing {
                        theme::SELECTION_CASING
                    } else {
                        theme::PREVIEW_NEW
                    },
                    tolerance_mm,
                ),
            });
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        AnchorId, Document, Length, NewAnchor, Point, RectBounds, ViewTransform,
    };

    fn view(px_per_mm: f64) -> ViewTransform {
        ViewTransform::new(px_per_mm, Point::new(0.0, 0.0))
    }

    /// The `--preview-new` line triangles only, without the white casing.
    fn line(list: &DrawList) -> Vec<crate::glyphs::Vertex> {
        list.triangles
            .iter()
            .copied()
            .filter(|v| v.color == theme::PREVIEW_NEW)
            .collect()
    }

    #[test]
    fn a_primitive_draws_a_hollow_outline_in_the_preview_colour_only() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let list = build_live_edit_preview(&[document.object(id).expect("exists")], view(1.0));
        assert_ne!(list.triangle_count(), 0);
        assert!(
            list.triangles
                .iter()
                .all(|v| v.color == theme::PREVIEW_NEW || v.color == theme::SELECTION_CASING),
            "the line, over its white casing, and nothing else"
        );
        // Hollow: no triangle vertex lies in the middle of the rectangle.
        assert!(
            list.triangles
                .iter()
                .all(|v| (v.position.x - 5.0).abs() > 2.5 || (v.position.y - 5.0).abs() > 2.5),
            "no fill preview"
        );
    }

    #[test]
    fn a_path_draws_through_its_anchors() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 0.0)),
            ],
            false,
        );
        let list = build_live_edit_preview(&[document.object(id).expect("exists")], view(1.0));
        assert_ne!(list.triangle_count(), 0);
        let far = list
            .triangles
            .iter()
            .map(|v| v.position.x)
            .fold(f64::MIN, f64::max);
        assert!((far - 20.0).abs() < 1.0);
    }

    /// The outline is 1.5 px on screen at every zoom, not the object's own
    /// stroke width.
    #[test]
    fn the_outline_is_one_and_a_half_screen_pixels_at_every_zoom() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(100.0, 0.0)),
            ],
            false,
        );
        let object = document.object(id).expect("exists");
        for scale in [0.5, 1.0, 8.0] {
            let list = build_live_edit_preview(std::slice::from_ref(&object), view(scale));
            let (lo, hi) = line(&list)
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                    (lo.min(v.position.y), hi.max(v.position.y))
                });
            assert!(
                ((hi - lo) * scale - theme::LIVE_PREVIEW_STROKE_PX).abs() < 1e-6,
                "scale {scale}: {} px",
                (hi - lo) * scale
            );
        }
    }

    /// `0007` criterion 40: a white casing 1.5 px wider on each side (4.5 px
    /// in all), drawn before the line, for every object before any line.
    #[test]
    fn the_outline_sits_on_a_white_casing_one_line_width_wider_on_each_side() {
        let document = Document::new(1);
        let ids: Vec<_> = [0.0, 50.0]
            .into_iter()
            .map(|y| {
                document.create_path(
                    &[
                        NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, y)),
                        NewAnchor::corner(AnchorId::new(1, 2), Point::new(100.0, y)),
                    ],
                    false,
                )
            })
            .collect();
        let objects: Vec<_> = ids.iter().map(|id| document.object(*id).unwrap()).collect();
        let scale = 2.0;
        let list = build_live_edit_preview(&objects, view(scale));
        let casing: Vec<_> = list
            .triangles
            .iter()
            .filter(|v| v.color == theme::SELECTION_CASING)
            .collect();
        let (lo, hi) = casing
            .iter()
            .filter(|v| v.position.y < 25.0)
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                (lo.min(v.position.y), hi.max(v.position.y))
            });
        assert!(
            ((hi - lo) * scale - 3.0 * theme::LIVE_PREVIEW_STROKE_PX).abs() < 1e-6,
            "casing is {} px",
            (hi - lo) * scale
        );
        let last_casing = list
            .triangles
            .iter()
            .rposition(|v| v.color == theme::SELECTION_CASING)
            .unwrap();
        let first_line = list
            .triangles
            .iter()
            .position(|v| v.color == theme::PREVIEW_NEW)
            .unwrap();
        assert!(last_casing < first_line, "every casing before any line");
    }

    #[test]
    fn nothing_to_preview_draws_nothing() {
        assert_eq!(build_live_edit_preview(&[], view(1.0)).triangle_count(), 0);
    }
}
