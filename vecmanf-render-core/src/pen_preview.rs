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

use vecmanf_document_core::{AnchorKind, AnchorSnapshot, Point, Vec2, ViewTransform};

use crate::glyphs::{self, DrawList};
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
/// or absent, with no drag in flight either: nothing to preview, returns
/// an empty list); `cursor` is the live pointer position in document
/// space for the rubber-band line (acceptance criterion 1: "showing
/// where a plain click would land") — `None` suppresses it (e.g. the
/// pointer has left the canvas).
///
/// `drag_origin` is `vecmanf_ui_core::PenTool::pending_drag_origin` —
/// `Some(C)` while the maker is holding the mouse button down over the
/// new point C, `None` otherwise. When `Some`, this is acceptance
/// criterion 2's live drag-to-curve preview: both symmetric handle
/// lines/endpoints growing from C, and (when there is a previously
/// placed node B to connect from) the B→C segment reshaped live as a
/// curve using C's still-uncommitted handle — in place of the plain
/// straight rubber-band line `cursor` alone would otherwise draw. This
/// is a genuinely different case from just having `cursor`: a hover
/// between gestures (button up) only ever warrants the straight
/// placement preview: acceptance criterion 1's "where a plain click
/// would land" is a straight line precisely because releasing now
/// places a corner node, not a curved one.
///
/// `is_hovering_close_target` is the host's own
/// `is_hovering_pen_close_target()` (acceptance criterion 5's "Cursors"
/// UX note): when `true`, the first placed node also gets the hover ring
/// treatment, the same visual the most-recently-placed node always
/// carries — the spec calls for both the cursor swap *and* this ring as
/// two independent, deliberately redundant signals that closing is one
/// click away, not an either/or.
#[must_use]
pub fn build_pen_preview(
    nodes: &[AnchorSnapshot],
    cursor: Option<Point>,
    drag_origin: Option<Point>,
    view: ViewTransform,
    is_hovering_close_target: bool,
) -> DrawList {
    let mut list = DrawList::default();
    if nodes.is_empty() && drag_origin.is_none() {
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
    let handle_diameter = screen_px_to_mm(view, theme::HANDLE_DIAMETER_PX);
    let line_width = screen_px_to_mm(view, theme::HANDLE_LINE_WIDTH_PX);

    for anchor in nodes {
        let glyph = match anchor.kind {
            AnchorKind::Corner => glyphs::square,
            AnchorKind::Smooth => glyphs::diamond,
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
        list.extend(glyphs::ring(
            last.point,
            hover_ring_diameter,
            hover_ring_thickness,
            theme::ACCENT_HOVER,
        ));
    }

    // Acceptance criterion 5's second close-target signal: the path's
    // own first node also gets the hover ring while the cursor is over
    // it and closing is one click away. `is_hovering_close_target` only
    // ever comes in `true` once at least two nodes are placed — the
    // same precondition `vecmanf_ui_core::PenTool`'s own close decision
    // uses — but `nodes.len() > 1` is checked here too rather than
    // trusting the caller, so a single placed node (first and last are
    // the same node, already ringed above) can never be drawn twice.
    if is_hovering_close_target
        && nodes.len() > 1
        && let Some(first) = nodes.first()
    {
        list.extend(glyphs::ring(
            first.point,
            hover_ring_diameter,
            hover_ring_thickness,
            theme::ACCENT_HOVER,
        ));
    }

    if let (Some(origin), Some(cursor)) = (drag_origin, cursor) {
        // Acceptance criterion 2's live curve preview. `handle_out` is
        // exactly what `PenTool::pointer_up` will commit if the maker
        // released right now (the press→cursor displacement); mirroring
        // it for `handle_in` previews the same symmetric-handle node
        // `pointer_up` creates.
        let handle_out = origin.vector_to(cursor);
        let handle_in = handle_out.negated();

        // Reshape the B→C segment live as a curve — only when a B
        // exists to connect from; dragging while placing the very first
        // node of a brand-new path has no prior node yet, so there is
        // nothing to reshape (C's own handles below still preview).
        if let Some(last) = nodes.last() {
            list.extend(stroke::segment_stroke(
                last.point,
                last.handle_out,
                handle_in,
                origin,
                STROKE_WIDTH_MM,
                theme::ACCENT,
            ));
        }

        // Both symmetric handle lines/endpoints growing from C in real
        // time. Filled accent endpoints, matching `docs/design-
        // system.md`'s "being dragged: filled accent" handle convention
        // — this drag is live, the same visual a node tool handle drag
        // already uses.
        for handle in [handle_out, handle_in] {
            if handle == Vec2::ZERO {
                continue;
            }
            let endpoint = origin.translated(handle);
            list.extend(glyphs::thick_line(
                origin,
                endpoint,
                line_width,
                theme::ACCENT,
            ));
            list.extend(glyphs::circle(endpoint, handle_diameter, theme::ACCENT));
        }

        // C's own node glyph, in the same hollow style as every other
        // placed node above — a drag always produces a Smooth node
        // (acceptance criterion 2), so the diamond glyph previews the
        // kind it is about to commit as, not just its position.
        list.extend(glyphs::diamond(origin, node_size, theme::ACCENT));
        list.extend(glyphs::diamond(
            origin,
            (node_size - 2.0 * node_outline).max(0.0),
            theme::CANVAS_BG,
        ));
    } else if let (Some(last), Some(cursor)) = (nodes.last(), cursor) {
        // Rubber-band preview: a line from the last placed node to the
        // cursor, for the plain-click case (acceptance criterion 1).
        // Solid rather than the UX notes' stated 1px dashed line — this
        // crate has no dashed-line primitive yet; a flagged
        // simplification, not a missing signal (the line itself is drawn
        // at full accent opacity, same as every other editing-UI line
        // here).
        list.extend(glyphs::thick_line(
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
        let list = build_pen_preview(
            &[],
            Some(Point::new(5.0, 5.0)),
            None,
            ViewTransform::identity(),
            false,
        );
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn one_placed_node_previews_a_glyph_and_hover_ring_but_no_rubber_band_without_a_cursor() {
        let nodes = [NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))];
        let with_cursor = build_pen_preview(
            &nodes,
            Some(Point::new(10.0, 0.0)),
            None,
            ViewTransform::identity(),
            false,
        );
        let without_cursor =
            build_pen_preview(&nodes, None, None, ViewTransform::identity(), false);
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
        let one_node = build_pen_preview(&nodes[..1], None, None, ViewTransform::identity(), false);
        let two_nodes = build_pen_preview(&nodes, None, None, ViewTransform::identity(), false);
        assert!(
            two_nodes.triangle_count() > one_node.triangle_count(),
            "the stroke between the two placed nodes adds geometry"
        );
    }

    /// Acceptance criterion 5's second close-target signal: hovering the
    /// in-progress path's own first node, with the host reporting
    /// `is_hovering_close_target`, must draw that first node's hover
    /// ring in addition to the last node's permanent one — a strictly
    /// bigger draw list than the same nodes without the flag set.
    #[test]
    fn hovering_the_close_target_rings_the_first_node_too() {
        let nodes = [
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), Point::new(5.0, 10.0)),
        ];
        let not_hovering = build_pen_preview(&nodes, None, None, ViewTransform::identity(), false);
        let hovering = build_pen_preview(&nodes, None, None, ViewTransform::identity(), true);
        assert!(
            hovering.triangle_count() > not_hovering.triangle_count(),
            "the first node's hover ring must add geometry when closing is one click away"
        );
    }

    /// The flag must not conjure a ring out of nothing when there is no
    /// path to close yet (a single placed node, where "first" and
    /// "last" are the same node that already always rings) — guards
    /// against the two branches silently double-drawing the same ring.
    #[test]
    fn hovering_close_target_with_one_node_does_not_double_the_ring() {
        let nodes = [NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))];
        let not_hovering = build_pen_preview(&nodes, None, None, ViewTransform::identity(), false);
        let hovering = build_pen_preview(&nodes, None, None, ViewTransform::identity(), true);
        assert_eq!(
            hovering.triangle_count(),
            not_hovering.triangle_count(),
            "first == last for one node; the flag must not draw a second ring on top"
        );
    }

    /// Acceptance criterion 2's live drag-to-curve preview, the bug this
    /// slice fixes: dragging while placing a new node C (one node B
    /// already placed) must draw strictly more geometry than the same
    /// moment with no drag in flight — the B→C curve segment plus C's
    /// own symmetric handle lines/endpoints, not just the plain
    /// rubber-band line to the cursor.
    #[test]
    fn dragging_while_placing_previews_the_live_curve_and_handles() {
        let nodes = [NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))];
        let cursor = Some(Point::new(13.0, 4.0));
        let origin = Point::new(10.0, 0.0);

        let hovering_only =
            build_pen_preview(&nodes, cursor, None, ViewTransform::identity(), false);
        let dragging = build_pen_preview(
            &nodes,
            cursor,
            Some(origin),
            ViewTransform::identity(),
            false,
        );
        assert!(
            dragging.triangle_count() > hovering_only.triangle_count(),
            "the live curve segment and C's handle lines/endpoints must add geometry over the \
             plain straight rubber-band preview"
        );
    }

    /// Dragging while placing the very first node of a brand-new path
    /// (no B exists yet to connect from) still previews C's own growing
    /// handles — there just is no segment to reshape.
    #[test]
    fn dragging_the_very_first_node_previews_its_own_handles_with_no_segment() {
        let origin = Point::new(0.0, 0.0);
        let cursor = Some(Point::new(5.0, 5.0));
        let dragging =
            build_pen_preview(&[], cursor, Some(origin), ViewTransform::identity(), false);
        assert!(
            !dragging.triangles.is_empty(),
            "C's own handle lines/endpoints and node glyph must still draw"
        );
    }

    /// A zero-length drag (the cursor hasn't moved off the press point
    /// yet) must not draw degenerate zero-length handle lines.
    #[test]
    fn a_zero_length_drag_draws_no_handle_geometry() {
        let origin = Point::new(5.0, 5.0);
        let dragging = build_pen_preview(
            &[],
            Some(origin),
            Some(origin),
            ViewTransform::identity(),
            false,
        );
        // Only C's own node glyph (hollow diamond: outline + fill, 2
        // quads each) should draw — no handle lines, no handle endpoints.
        assert_eq!(dragging.triangle_count(), 4);
    }
}
