//! The document's own drawing: every object's fill and then its stroke, all
//! objects in tree (z) order, bottom to top, paths and primitives interleaved
//! (`specs/0007-stroke-and-fill-styling/specification.md`, criterion 26).
//! Editor decorations are not artwork; they are drawn after it.

use curvyo_document_core::{
    FillKind, ObjectSnapshot, Opacity, PathSnapshot, PrimitiveSnapshot, Style, ViewTransform,
    outline_of_rotated,
};

use lyon::path::Path;

use crate::color::RgbaColor;
use crate::dash::{self, DashBudget};
use crate::fill;
use crate::glyphs::DrawList;
use crate::gradient::{GradientFill, GradientFrame, Ramp};
use crate::shape_preview::outline_to_anchors;
use crate::stroke::{self, OutlineRef, StrokeParams};
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
///
/// `frames[i]` is the box a gradient fill of `objects[i]` spans (the object's
/// oriented selection box, which only the editor can compute); `None`, or a
/// list shorter than `objects`, paints a gradient fill flat in its first
/// stop's colour.
#[must_use]
pub fn build_artwork(
    objects: &[ObjectSnapshot],
    frames: &[Option<GradientFrame>],
    view: ViewTransform,
) -> DrawList {
    debug_assert!(
        frames.is_empty() || frames.len() == objects.len(),
        "one frame per object, or none"
    );
    let mut budget = DashBudget::per_frame();
    let mut list = DrawList::default();
    for (index, object) in objects.iter().enumerate() {
        let frame = frames.get(index).copied().flatten();
        match object {
            ObjectSnapshot::Path(path) => {
                draw_path(&mut list, path, frame, view, &mut budget);
            }
            ObjectSnapshot::Primitive(primitive) => {
                draw_primitive(&mut list, primitive, frame, view, &mut budget);
            }
        }
    }
    list
}

fn draw_path(
    list: &mut DrawList,
    path: &PathSnapshot,
    frame: Option<GradientFrame>,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    let outlines: Vec<OutlineRef<'_>> = path
        .subpaths()
        .map(|subpath| OutlineRef {
            anchors: subpath.anchors,
            closed: subpath.closed,
        })
        .collect();
    draw_object(list, &outlines, &path.style, frame, view, budget);
}

fn draw_primitive(
    list: &mut DrawList,
    primitive: &PrimitiveSnapshot,
    frame: Option<GradientFrame>,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    let anchors = outline_to_anchors(&outline_of_rotated(&primitive.shape, primitive.rotation));
    let outline = [OutlineRef {
        anchors: &anchors,
        closed: true,
    }];
    draw_object(list, &outline, &primitive.style, frame, view, budget);
}

/// An object's outlines as the fill and the stroke read them: one for an
/// ordinary path or a primitive, several for a compound path.
fn draw_object(
    list: &mut DrawList,
    outlines: &[OutlineRef<'_>],
    style: &Style,
    frame: Option<GradientFrame>,
    view: ViewTransform,
    budget: &mut DashBudget,
) {
    // The fill is painted first, with the outline closed by a chord when the
    // path is open; the stroke below keeps the real open path.
    if style.fill.paints() {
        draw_fill(list, outlines, style, frame, view);
    }
    if style.stroke.enabled {
        draw_stroke(list, outlines, style, view, budget);
    }
}

/// The fill layer of an object whose fill paints. A solid fill is one flat
/// colour. A gradient with one stop is uniform in that stop's colour; with two
/// or more it is a [`GradientFill`] the host paints from its ramp, or, with no
/// frame to span, flat in the first stop's colour.
fn draw_fill(
    list: &mut DrawList,
    outlines: &[OutlineRef<'_>],
    style: &Style,
    frame: Option<GradientFrame>,
    view: ViewTransform,
) {
    let fill_style = &style.fill;
    let ramp = (fill_style.kind != FillKind::Solid)
        .then(|| Ramp::from_stops(&fill_style.stops))
        .flatten();
    let color = match &ramp {
        Some(ramp) => {
            let [r, g, b, a] = ramp.first();
            RgbaColor { r, g, b, a }
        }
        None => paint(fill_style.color, fill_style.opacity),
    };
    let gradient = (fill_style.stops.len() > 1)
        .then_some(ramp)
        .flatten()
        .zip(frame);
    // A fully transparent flat fill draws nothing; a gradient's alpha varies.
    if gradient.is_none() && color.a == 0 {
        return;
    }
    let Some(path) = stroke::build_fill_path(outlines) else {
        return;
    };
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    let start = list.triangles.len();
    list.extend_artwork(fill::fill(&path, color, tolerance_mm));
    if let Some((ramp, frame)) = gradient {
        list.push_gradient(GradientFill {
            start,
            end: list.triangles.len(),
            frame,
            radial: fill_style.kind == FillKind::Radial,
            ramp,
        });
    }
}

/// The stroke layer of an object. Every outline is dashed on its own, so a
/// dash pattern starts afresh at the first node of each (criterion 31a), and
/// all of them are tessellated together as one layer, so a translucent stroke
/// shows no dark spot where two outlines' strokes meet.
fn draw_stroke(
    list: &mut DrawList,
    outlines: &[OutlineRef<'_>],
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
    let solid: Vec<Path> = outlines
        .iter()
        .filter_map(|outline| stroke::build_path(outline.anchors, outline.closed))
        .collect();
    // The dash limits are the object's: either every outline is dashed or the
    // whole object is solid.
    let mut parts = dash::dashed_object(
        &solid,
        &stroke_style.dash,
        document_width_mm,
        view.scale(),
        tolerance_mm,
        budget,
    )
    .unwrap_or(solid);
    let combined = match parts.len() {
        0 => return,
        1 => parts.remove(0),
        _ => {
            let mut builder = Path::builder();
            builder.extend_from_paths(&parts.iter().map(Path::as_slice).collect::<Vec<_>>());
            builder.build()
        }
    };
    list.extend_artwork(stroke::stroke(&combined, &params));
}
