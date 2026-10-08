//! A primitive's outline for drawing and the shape tools' create-drag preview
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criterion 16):
//! the outline anchors [`crate::artwork`] draws a primitive from (the same
//! [`curvyo_document_core::outline_of`] output "object to path" converts, so
//! the conversion is visually identical, `adrs.md`) and the live outline of a
//! shape being created. Selection boxes and handles are the Select tool's
//! (`crate::select_decoration`); the shape tools only create.

use curvyo_document_core::{Angle, Point, Shape, ViewTransform, outline_of_rotated};

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList};
use crate::stroke;
use crate::theme;

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// A dummy identity used only to satisfy
/// [`crate::stroke::path_stroke`]'s `AnchorSnapshot` parameter type — an
/// outline anchor has no real id (`curvyo_document_core::OutlineAnchor`
/// is deliberately id-less), and nothing here ever reads it back.
const UNUSED_ANCHOR_ID: curvyo_document_core::AnchorId = curvyo_document_core::AnchorId::new(0, 0);

pub(crate) fn outline_to_anchors(
    outline: &[curvyo_document_core::OutlineAnchor],
) -> Vec<curvyo_document_core::AnchorSnapshot> {
    outline
        .iter()
        .map(|a| curvyo_document_core::NewAnchor {
            id: UNUSED_ANCHOR_ID,
            point: a.point,
            handle_in: a.handle_in,
            handle_out: a.handle_out,
            kind: a.kind,
        })
        .collect()
}

/// A shape tool's live, uncommitted create-drag preview
/// (`specs/0003-primitive-shapes/specification.md`'s "Live creation
/// feedback": "a maker dragging out a rectangle sees a rectangle
/// updating live, not a placeholder box that snaps to shape on
/// release"). Hollow, `--accent` outline, screen-space-constant
/// stroke weight — distinct from this module's own placeholder stroke
/// (acceptance criterion 16's black, document-mm-weighted one), since
/// nothing has committed yet. `curvyo-ui-core`'s own shape tools
/// decide *whether* there is a live shape to preview right now
/// (`curvyo_ui_core::CreatePreview`); this function only draws the
/// [`Shape`] it is given.
#[must_use]
pub fn build_shape_live_preview(shape: &Shape, rotation: Angle, view: ViewTransform) -> DrawList {
    let mut list = shape_live_outline(shape, rotation, view, true);
    list.extend(shape_live_outline(shape, rotation, view, false));
    list
}

/// The hollow outline of `shape` at the live-preview weight: its white casing
/// (one line width wider on each side, `0007` criterion 40) or the
/// `--preview-new` line itself. [`crate::live_preview`] draws every object's
/// casing before any line so that one outline's casing never covers another's
/// line.
pub(crate) fn shape_live_outline(
    shape: &Shape,
    rotation: Angle,
    view: ViewTransform,
    casing: bool,
) -> DrawList {
    let outline = outline_of_rotated(shape, rotation);
    let anchors = outline_to_anchors(&outline);
    let width = screen_px_to_mm(view, theme::LIVE_PREVIEW_STROKE_PX);
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    if casing {
        stroke::path_stroke(
            &anchors,
            true,
            width * theme::CASING_WIDTH_FACTOR,
            theme::SELECTION_CASING,
            tolerance_mm,
        )
    } else {
        stroke::path_stroke(&anchors, true, width, theme::ACCENT, tolerance_mm)
    }
}

/// A dashed guide line (`docs/design-system.md`: the radius guide and the skew
/// fixed-line guide, visually distinct from the node tool's *solid* handle
/// line). A flagged simplification like
/// `pen_preview.rs`'s rubber-band line: built from short solid segments
/// rather than a real stippled-stroke primitive, since this crate has
/// none yet.
pub(crate) fn dashed_guide(
    a: Point,
    b: Point,
    width_mm: f64,
    color: RgbaColor,
    dash_mm: f64,
    gap_mm: f64,
) -> DrawList {
    let mut list = DrawList::default();
    let total = a.vector_to(b).length();
    if total <= f64::EPSILON || dash_mm <= 0.0 {
        return list;
    }
    let direction = a.vector_to(b).normalized_to(1.0);
    let step = dash_mm + gap_mm;
    let mut travelled = 0.0;
    while travelled < total {
        let dash_end = (travelled + dash_mm).min(total);
        let start = a.translated(direction.scaled(travelled));
        let end = a.translated(direction.scaled(dash_end));
        list.extend(glyphs::thick_line(start, end, width_mm, color));
        travelled += step;
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        Document, Length, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot, RectBounds,
    };

    fn rect_snapshot(document: &Document) -> (NodeId, PrimitiveSnapshot) {
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        (id, document.primitive(id).expect("exists"))
    }

    /// A live preview draws a non-empty outline for an in-progress shape,
    /// independent of any committed primitive.
    #[test]
    fn build_shape_live_preview_draws_a_rect_outline() {
        let shape = Shape::Rect {
            bounds: RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            },
            corner_radii: curvyo_document_core::CornerRadii::uniform(Length::from_mm(0.0)),
        };
        let list =
            build_shape_live_preview(&shape, Angle::from_radians(0.0), ViewTransform::identity());
        assert_ne!(list.triangles, Vec::<glyphs::Vertex>::new());
    }

    /// Acceptance criterion 25 (and the slice's whole point): a rotated
    /// primitive's stroke is drawn turned, not as its unrotated frame — a
    /// 10 × 10 square rotated 45° about its center reaches the apex
    /// `(5, 5 - 7.07)` and leaves its unrotated corner `(0, 0)` empty.
    #[test]
    fn a_rotated_primitives_stroke_is_drawn_turned() {
        let document = Document::new(1);
        let (id, _) = rect_snapshot(&document);
        document
            .rotate_object(&document.object(id).expect("object exists").rotated(
                Point::new(5.0, 5.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_4),
            ))
            .expect("rotate");
        let snapshot = document.primitive(id).expect("exists");
        let list = crate::build_artwork(
            &[ObjectSnapshot::Primitive(snapshot)],
            &[],
            ViewTransform::identity(),
        );
        let reaches = |target: Point| {
            list.triangles
                .iter()
                .any(|v| v.position.vector_to(target).length() < 1.0)
        };
        assert!(
            reaches(Point::new(5.0, 5.0 - 50.0_f64.sqrt())),
            "apex drawn"
        );
        assert!(
            !reaches(Point::new(0.0, 0.0)),
            "unrotated corner left empty"
        );
    }

    /// A primitive draws only its stroke: no box, no handle (those are the
    /// Select tool's decorations).
    #[test]
    fn a_primitive_draws_only_its_stroke() {
        let document = Document::new(1);
        let (_, snapshot) = rect_snapshot(&document);
        let list = crate::build_artwork(
            &[ObjectSnapshot::Primitive(snapshot)],
            &[],
            ViewTransform::identity(),
        );
        assert!(!list.triangles.is_empty(), "the stroke itself draws");
        assert!(
            list.triangles.iter().all(|v| v.color == RgbaColor::BLACK),
            "nothing but the black stroke"
        );
    }
}
