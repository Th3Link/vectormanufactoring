//! What the Pen shows over a target before the click (`specs/0034-pen-path-extension` criteria 1,
//! 7, 13 and 17): the end node a press would continue or join, drawn with its idle glyph inside the
//! hover ring, and the closing segment exactly as it will be committed, dashed, with the handles
//! of the closing node and its glyph of the resolved kind. Data in, triangles out: the targets
//! and the join are resolved by `curvyo-ui-core`.

use curvyo_document_core::{
    AnchorKind, AnchorSnapshot, DocumentBackground, DocumentSize, Point, Vec2, ViewTransform,
};

use crate::color::RgbaColor;
use crate::document_area::background_at;
use crate::glyphs::{self, DrawList};
use crate::theme;

/// The closing segment and the closing node, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosingCue {
    /// The node the closing segment leaves (its outgoing handle faces it).
    pub from: AnchorSnapshot,
    /// The node it arrives at (its incoming handle faces it).
    pub to: AnchorSnapshot,
    /// The closing node with the join applied, one of `from` and `to`.
    pub node: AnchorSnapshot,
}

/// What to draw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PenCue {
    /// The end node a press would continue or join: drawn with its idle node glyph inside the
    /// hover ring. Not set while Shift asks for the alternative.
    pub target: Option<(Point, AnchorKind)>,
    /// The closing segment of a close target.
    pub closing: Option<ClosingCue>,
}

/// Dash length and gap of the closing segment, screen pixels.
const DASH_PX: f64 = 6.0;
const GAP_PX: f64 = 4.0;
/// The closing segment's line width, screen pixels.
const LINE_PX: f64 = 1.0;

fn px(view: ViewTransform, pixels: f64) -> f64 {
    pixels / view.scale()
}

/// The cue as triangles.
#[must_use]
pub fn build_pen_cue(
    cue: &PenCue,
    view: ViewTransform,
    document_size: DocumentSize,
    background: DocumentBackground,
) -> DrawList {
    let mut list = DrawList::default();
    if let Some((point, kind)) = cue.target {
        list.extend(idle_glyph(point, kind, view));
        list.extend(glyphs::ring(
            point,
            px(view, theme::HOVER_RING_DIAMETER_PX),
            px(view, 1.0),
            theme::ACCENT_HOVER,
        ));
    }
    if let Some(closing) = &cue.closing {
        list.extend(closing_segment(closing, view));
        list.extend(closing_node(closing, view, document_size, background));
    }
    list
}

/// A node's idle look: white fill inside its outline, the shape of its kind.
fn idle_glyph(point: Point, kind: AnchorKind, view: ViewTransform) -> DrawList {
    let glyph = match kind {
        AnchorKind::Corner => glyphs::square,
        AnchorKind::Symmetric => glyphs::diamond,
        AnchorKind::Asymmetric => glyphs::triangle,
    };
    let node = px(view, theme::NODE_SIZE_PX);
    let outline = px(view, theme::NODE_OUTLINE_PX);
    let mut list = DrawList::default();
    list.extend(glyph(point, node, theme::NODE_STROKE));
    list.extend(glyph(
        point,
        (node - 2.0 * outline).max(0.0),
        RgbaColor::WHITE,
    ));
    list
}

/// The closing segment as a dashed accent curve.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn closing_segment(closing: &ClosingCue, view: ViewTransform) -> DrawList {
    let (from, to) = (&closing.from, &closing.to);
    let p0 = from.point;
    let p1 = from.point.translated(from.handle_out);
    let p2 = to.point.translated(to.handle_in);
    let p3 = to.point;
    let chord = p0.vector_to(p3).length()
        + p0.vector_to(p1).length()
        + p1.vector_to(p2).length()
        + p2.vector_to(p3).length();
    let steps = ((chord * view.scale() / 3.0).ceil()).clamp(16.0, 600.0) as u32;
    // Sampled by the curve parameter: flatten, then walk the polyline dashing it.
    let samples: Vec<Point> = (0..=steps)
        .map(|i| cubic_at(p0, p1, p2, p3, f64::from(i) / f64::from(steps)))
        .collect();
    dash_polyline(
        &samples,
        px(view, DASH_PX),
        px(view, GAP_PX),
        px(view, LINE_PX),
        theme::ACCENT,
    )
}

fn cubic_at(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let rest = 1.0 - t;
    let weights = [
        rest * rest * rest,
        3.0 * rest * rest * t,
        3.0 * rest * t * t,
        t * t * t,
    ];
    let along = |pick: fn(Point) -> f64| {
        weights[0] * pick(p0)
            + weights[1] * pick(p1)
            + weights[2] * pick(p2)
            + weights[3] * pick(p3)
    };
    Point::new(along(|p| p.x), along(|p| p.y))
}

/// `samples` as dashes of `dash` and `gap` millimetres along their length.
fn dash_polyline(samples: &[Point], dash: f64, gap: f64, width: f64, color: RgbaColor) -> DrawList {
    let mut list = DrawList::default();
    let mut drawing = true;
    let mut left = dash;
    for pair in samples.windows(2) {
        let mut start = pair[0];
        let end = pair[1];
        let mut remaining = start.vector_to(end).length();
        while remaining > 0.0 {
            let step = remaining.min(left);
            let fraction = if remaining > 0.0 {
                step / remaining
            } else {
                0.0
            };
            let next = Point::new(
                start.x + (end.x - start.x) * fraction,
                start.y + (end.y - start.y) * fraction,
            );
            if drawing {
                list.extend(glyphs::thick_line(start, next, width, color));
            }
            start = next;
            remaining -= step;
            left -= step;
            if left <= 1e-12 {
                drawing = !drawing;
                left = if drawing { dash } else { gap };
            }
        }
    }
    list
}

/// The closing node: its handles in the idle look (1 px accent line, white circle with an accent
/// ring; a zero handle is not drawn) and its glyph of the resolved kind, hollow in accent like
/// every node of a path in progress.
fn closing_node(
    closing: &ClosingCue,
    view: ViewTransform,
    document_size: DocumentSize,
    background: DocumentBackground,
) -> DrawList {
    let node = &closing.node;
    let mut list = DrawList::default();
    // The ring of a close target (the same one the Pen draws on a node it can click).
    list.extend(glyphs::ring(
        node.point,
        px(view, theme::HOVER_RING_DIAMETER_PX),
        px(view, 1.0),
        theme::ACCENT_HOVER,
    ));
    let line = px(view, theme::HANDLE_LINE_WIDTH_PX);
    let diameter = px(view, theme::HANDLE_DIAMETER_PX);
    let ring = px(view, 1.0);
    for handle in [node.handle_out, node.handle_in] {
        if handle == Vec2::ZERO {
            continue;
        }
        let endpoint = node.point.translated(handle);
        list.extend(glyphs::thick_line(
            node.point,
            endpoint,
            line,
            theme::ACCENT,
        ));
        list.extend(glyphs::circle(endpoint, diameter, theme::ACCENT));
        list.extend(glyphs::circle(
            endpoint,
            (diameter - 2.0 * ring).max(0.0),
            RgbaColor::WHITE,
        ));
    }
    let glyph = match node.kind {
        AnchorKind::Corner => glyphs::square,
        AnchorKind::Symmetric => glyphs::diamond,
        AnchorKind::Asymmetric => glyphs::triangle,
    };
    let size = px(view, theme::NODE_SIZE_PX);
    let outline = px(view, theme::NODE_OUTLINE_PX);
    list.extend(glyph(node.point, size, theme::ACCENT));
    list.extend(glyph(
        node.point,
        (size - 2.0 * outline).max(0.0),
        background_at(document_size, background, node.point),
    ));
    list
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, NewAnchor};

    use super::*;

    fn node(x: f64, y: f64) -> AnchorSnapshot {
        NewAnchor::corner(AnchorId::new(1, 1), Point::new(x, y))
    }

    #[test]
    fn no_cue_draws_nothing() {
        let list = build_pen_cue(
            &PenCue::default(),
            ViewTransform::identity(),
            DocumentSize::default(),
            DocumentBackground::DEFAULT,
        );
        assert_eq!(list.triangle_count(), 0);
    }

    /// A target is drawn as its glyph inside the ring: more than the ring alone.
    #[test]
    fn a_target_draws_a_glyph_and_a_ring() {
        let cue = PenCue {
            target: Some((Point::new(5.0, 5.0), AnchorKind::Corner)),
            closing: None,
        };
        let list = build_pen_cue(
            &cue,
            ViewTransform::identity(),
            DocumentSize::default(),
            DocumentBackground::DEFAULT,
        );
        let ring_only = glyphs::ring(
            Point::new(5.0, 5.0),
            theme::HOVER_RING_DIAMETER_PX,
            1.0,
            theme::ACCENT_HOVER,
        );
        assert!(list.triangle_count() > ring_only.triangle_count());
    }

    /// The closing segment is dashed: a long straight segment draws several separate dashes, and a
    /// handle of the closing node adds a line and a circle.
    #[test]
    fn the_closing_segment_is_dashed_and_shows_the_handles_of_the_closing_node() {
        let from = node(0.0, 0.0);
        let to = node(100.0, 0.0);
        let plain = PenCue {
            target: None,
            closing: Some(ClosingCue { from, to, node: to }),
        };
        let with_handle = PenCue {
            target: None,
            closing: Some(ClosingCue {
                from,
                to,
                node: AnchorSnapshot {
                    handle_in: Vec2::new(-10.0, 0.0),
                    ..to
                },
            }),
        };
        let view = ViewTransform::identity();
        let size = DocumentSize::default();
        let a = build_pen_cue(&plain, view, size, DocumentBackground::DEFAULT);
        let b = build_pen_cue(&with_handle, view, size, DocumentBackground::DEFAULT);
        // 100 px at 10 px per dash and gap: about ten dashes of two triangles each, plus the glyph.
        assert!(a.triangle_count() > 15, "{}", a.triangle_count());
        assert!(b.triangle_count() > a.triangle_count());
    }
}
