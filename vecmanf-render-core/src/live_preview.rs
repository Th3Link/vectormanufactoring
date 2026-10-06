//! The "new" half of blue-new, black-old
//! (`specs/unified-object-editing/specification.md`, criteria 10 to 15): the
//! geometry a release would commit, drawn as a hollow `--preview-new` outline
//! of constant screen width over the unchanged committed objects. The
//! committed objects are not touched here; "black old" is the absence of a
//! second render path, because the document's own strokes are drawn exactly
//! as they are.

use vecmanf_document_core::{ObjectSnapshot, ViewTransform, outline_of_rotated};

use crate::glyphs::DrawList;
use crate::shape_preview::outline_to_anchors;
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
    for object in objects {
        list.extend(match object {
            ObjectSnapshot::Primitive(primitive) => {
                let outline = outline_of_rotated(&primitive.shape, primitive.rotation);
                stroke::path_stroke(
                    &outline_to_anchors(&outline),
                    true,
                    width_mm,
                    theme::PREVIEW_NEW,
                    tolerance_mm,
                )
            }
            ObjectSnapshot::Path(path) => stroke::path_stroke(
                &path.anchors,
                path.closed,
                width_mm,
                theme::PREVIEW_NEW,
                tolerance_mm,
            ),
        });
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{
        AnchorId, Document, Length, NewAnchor, Point, RectBounds, ViewTransform,
    };

    fn view(px_per_mm: f64) -> ViewTransform {
        ViewTransform::new(px_per_mm, Point::new(0.0, 0.0))
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
        assert!(list.triangles.iter().all(|v| v.color == theme::PREVIEW_NEW));
        // Hollow: no triangle vertex lies in the middle of the rectangle.
        assert!(
            list.triangles
                .iter()
                .all(|v| (v.position.x - 5.0).abs() > 3.0 || (v.position.y - 5.0).abs() > 3.0),
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
            let (lo, hi) = list
                .triangles
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

    #[test]
    fn nothing_to_preview_draws_nothing() {
        assert_eq!(build_live_edit_preview(&[], view(1.0)).triangle_count(), 0);
    }
}
