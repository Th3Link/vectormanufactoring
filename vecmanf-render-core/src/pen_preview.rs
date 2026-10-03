//! The pen tool's in-progress preview (`specification.md`'s UX notes,
//! "In-progress path feedback" and "In-progress path" visual
//! convention): already-placed nodes, the stroke connecting them (which
//! already renders any placed curve correctly — reusing [`crate::stroke::
//! path_stroke`] needs only the anchors, not a document identity), and a
//! rubber-band line to the live cursor.
//!
//! A separate entry point from [`crate::build_draw_list`] rather than a
//! synthetic [`vecmanf_document_core::PathSnapshot`]: an in-progress pen
//! path has no [`vecmanf_document_core::NodeId`] yet (ADR 0009 §2 — it is
//! ephemeral `vecmanf-ui-core::PenTool` state, not a document node), and
//! `NodeId` has no public constructor outside `vecmanf-document-core`.

use vecmanf_document_core::{AnchorKind, AnchorSnapshot, Point, ViewTransform};

use crate::primitives::{self, DrawList};
use crate::stroke;
use crate::theme;

/// This slice's one placeholder stroke width (acceptance criterion 6),
/// reused for the in-progress preview's own connecting stroke so it
/// previews at the same weight the committed path will render at.
const STROKE_WIDTH_MM: f64 = 0.25;

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// Builds the pen tool's in-progress preview. `nodes` is whatever
/// `vecmanf_ui_core::PenTool::in_progress_nodes` currently holds (empty
/// or absent: nothing to preview, returns an empty list); `cursor` is the
/// live pointer position in document space for the rubber-band line
/// (acceptance criterion 1: "showing where a plain click would land") —
/// `None` suppresses it (e.g. the pointer has left the canvas).
#[must_use]
pub fn build_pen_preview(
    nodes: &[AnchorSnapshot],
    cursor: Option<Point>,
    view: ViewTransform,
) -> DrawList {
    let mut list = DrawList::default();
    if nodes.is_empty() {
        return list;
    }

    // Already-placed nodes/handles render in the same shapes but
    // hollow/outline-only in accent — distinct from a committed path's
    // node-tool selection state, so a maker never reads "still drawing"
    // as "already selected" (`specification.md`'s UX notes). The
    // connecting stroke reuses the exact tessellation a committed path
    // gets, so a placed curve (acceptance criterion 2) already previews
    // correctly with no extra code here.
    list.extend(stroke::path_stroke(
        nodes,
        false,
        STROKE_WIDTH_MM,
        theme::ACCENT,
    ));

    let node_size = screen_px_to_mm(view, theme::NODE_SIZE_PX);
    let node_outline = screen_px_to_mm(view, theme::NODE_OUTLINE_PX);
    let hover_ring_diameter = screen_px_to_mm(view, theme::HOVER_RING_DIAMETER_PX);
    let hover_ring_thickness = screen_px_to_mm(view, 1.0);

    for anchor in nodes {
        let glyph = match anchor.kind {
            AnchorKind::Corner => primitives::square,
            AnchorKind::Smooth => primitives::diamond,
        };
        list.extend(glyph(anchor.point, node_size, theme::ACCENT));
        list.extend(glyph(
            anchor.point,
            (node_size - 2.0 * node_outline).max(0.0),
            theme::CANVAS_BG,
        ));
    }

    // The most-recently-placed node marks where the next click/drag
    // extends from: it gets the hover ring treatment permanently, not
    // just on hover (`specification.md`'s UX notes).
    if let Some(last) = nodes.last() {
        list.extend(primitives::ring(
            last.point,
            hover_ring_diameter,
            hover_ring_thickness,
            theme::ACCENT_HOVER,
        ));
    }

    // Rubber-band preview: a line from the last placed node to the
    // cursor. Solid rather than the UX notes' stated 1px dashed line —
    // this crate has no dashed-line primitive yet; a flagged
    // simplification, not a missing signal (the line itself is drawn at
    // full accent opacity, same as every other editing-UI line here).
    if let (Some(last), Some(cursor)) = (nodes.last(), cursor) {
        let line_width = screen_px_to_mm(view, theme::HANDLE_LINE_WIDTH_PX);
        list.extend(primitives::thick_line(
            last.point,
            cursor,
            line_width,
            theme::ACCENT,
        ));
    }

    list
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, NewAnchor};

    use super::*;

    #[test]
    fn an_empty_in_progress_path_previews_as_nothing() {
        let list = build_pen_preview(&[], Some(Point::new(5.0, 5.0)), ViewTransform::identity());
        assert!(list.triangles.is_empty());
    }

    #[test]
    fn one_placed_node_previews_a_glyph_and_hover_ring_but_no_rubber_band_without_a_cursor() {
        let nodes = [NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))];
        let with_cursor = build_pen_preview(
            &nodes,
            Some(Point::new(10.0, 0.0)),
            ViewTransform::identity(),
        );
        let without_cursor = build_pen_preview(&nodes, None, ViewTransform::identity());
        assert!(
            !without_cursor.triangles.is_empty(),
            "glyph + hover ring still draw"
        );
        assert!(
            with_cursor.triangle_count() > without_cursor.triangle_count(),
            "the rubber-band line adds geometry"
        );
    }

    #[test]
    fn two_placed_nodes_preview_the_connecting_stroke() {
        let nodes = [
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 0.0)),
        ];
        let one_node = build_pen_preview(&nodes[..1], None, ViewTransform::identity());
        let two_nodes = build_pen_preview(&nodes, None, ViewTransform::identity());
        assert!(
            two_nodes.triangle_count() > one_node.triangle_count(),
            "the stroke between the two placed nodes adds geometry"
        );
    }
}
