//! The document's own drawing: every object's fill and then its stroke, all
//! objects in tree (z) order, bottom to top, paths and primitives interleaved
//! (`specs/0007-stroke-and-fill-styling/specification.md`, criterion 26).
//! Editor decorations are not artwork; they are drawn after it.

use curvyo_document_core::{
    ObjectSnapshot, Opacity, PathSnapshot, PrimitiveSnapshot, Style, ViewTransform,
    outline_of_rotated,
};

use crate::color::RgbaColor;
use crate::dash::{self, DashBudget};
use crate::fill;
use crate::glyphs::DrawList;
use crate::shape_preview::outline_to_anchors;
use crate::stroke::{self, StrokeParams};
use crate::theme;

/// The widest stroke ever tessellated, in millimetres. A stored width has no
/// upper bound; this keeps a crafted one from producing absurd geometry (and
/// `f32` overflow in the tessellator). Display only: the stored width is
/// never touched.
pub(crate) const MAX_DISPLAY_STROKE_WIDTH_MM: f64 = 100_000.0;

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// `color` at `opacity`, as a draw-list colour.
fn paint(color: curvyo_document_core::Color, opacity: Opacity) -> RgbaColor {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let alpha = (opacity.get() * 255.0).round() as u8;
    RgbaColor::from(color).with_alpha(alpha)
}

/// Builds the artwork of `objects`, in the order given (tree order).
///
/// Each object contributes its fill layer and then its stroke layer, each a
/// layer of the returned list (see [`DrawList::layers`]): a host that paints a
/// layer at most once per pixel shows a translucent stroke without dark spots
/// where `lyon`'s tessellation overlaps itself, and later objects cover
/// earlier ones.
///
/// A path and a primitive are drawn through the same code: a primitive's
/// outline is the one "object to path" would convert, so the conversion is
/// visually identical.
#[must_use]
pub fn build_artwork(objects: &[ObjectSnapshot], view: ViewTransform) -> DrawList {
    let mut budget = DashBudget::per_frame();
    let mut list = DrawList::default();
    for object in objects {
        match object {
            ObjectSnapshot::Path(path) => draw_path(&mut list, path, view, &mut budget),
            ObjectSnapshot::Primitive(primitive) => {
                draw_primitive(&mut list, primitive, view, &mut budget);
            }
        }
    }
    list
}

fn draw_path(
    list: &mut DrawList,
    path: &PathSnapshot,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    draw_object(list, &path.anchors, path.closed, &path.style, view, budget);
}

fn draw_primitive(
    list: &mut DrawList,
    primitive: &PrimitiveSnapshot,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    let anchors = outline_to_anchors(&outline_of_rotated(&primitive.shape, primitive.rotation));
    draw_object(list, &anchors, true, &primitive.style, view, budget);
}

fn draw_object(
    list: &mut DrawList,
    anchors: &[curvyo_document_core::AnchorSnapshot],
    closed: bool,
    style: &Style,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    // The fill is painted first, with the outline closed by a chord when the
    // path is open; the stroke below keeps the real open path.
    if style.fill.paints() && style.fill.kind == curvyo_document_core::FillKind::Solid {
        let color = paint(style.fill.color, style.fill.opacity);
        if color.a > 0
            && let Some(path) = stroke::build_fill_path(anchors, closed)
        {
            list.extend_artwork(fill::fill(&path, color, tolerance_mm));
        }
    }
    if style.stroke.enabled {
        draw_stroke(list, anchors, closed, style, view, budget);
    }
}

fn draw_stroke(
    list: &mut DrawList,
    anchors: &[curvyo_document_core::AnchorSnapshot],
    closed: bool,
    style: &Style,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    let stroke_style = &style.stroke;
    let color = paint(stroke_style.color, stroke_style.opacity);
    let document_width_mm = stroke_style.width.as_mm();
    if color.a == 0 || !document_width_mm.is_finite() || document_width_mm <= 0.0 {
        return;
    }
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    let min_width_mm = screen_px_to_mm(view, theme::MIN_DISPLAY_STROKE_WIDTH_PX);
    let params = StrokeParams {
        width_mm: document_width_mm
            .max(min_width_mm)
            .min(MAX_DISPLAY_STROKE_WIDTH_MM),
        color,
        tolerance_mm,
        join: stroke_style.join,
        cap: stroke_style.cap,
    };
    let Some(path) = stroke::build_path(anchors, closed) else {
        return;
    };
    let dashed = dash::dashed(
        &path,
        &stroke_style.dash,
        document_width_mm,
        view.scale(),
        tolerance_mm,
        budget,
    );
    list.extend_artwork(stroke::stroke(dashed.as_ref().unwrap_or(&path), &params));
}
