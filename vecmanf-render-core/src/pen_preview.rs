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
/// or absent, with no pending anchor either: nothing to preview, returns
/// an empty list); `cursor` is the live pointer position in document
/// space for the rubber-band line (acceptance criterion 1: "showing
/// where a plain click would land") — `None` suppresses it (e.g. the
/// pointer has left the canvas).
///
/// `pending` is `vecmanf_ui_core::PenTool::pending_anchor`'s result —
/// `Some` while the maker is holding the mouse button down, already
/// resolved (by that same method, the one place this rule is decided)
/// into exactly the anchor `vecmanf_ui_core::PenTool::pointer_up` would
/// commit if released right now: a corner node with no handles for a
/// press that hasn't moved past the drag threshold yet, a smooth node
/// with symmetric handles once it has. This function draws `pending`
/// exactly as given — the segment from the last placed node to it, its
/// own handle lines/endpoints where non-zero, and its own glyph by its
/// own `kind` — and performs no geometry of its own (no vector
/// arithmetic here duplicating what `pending_anchor` already resolved,
/// which is what let the preview and the actual commit disagree before:
/// a sub-threshold drag could preview a smooth node with handles that
/// then committed, on release, as a corner node with none).
///
/// This is a genuinely different case from just having `cursor`: a hover
/// between gestures (button up, nothing held down, so `pending` is
/// `None`) only ever warrants the straight placement preview —
/// acceptance criterion 1's "where a plain click would land" is a
/// straight line precisely because releasing now places a corner node,
/// not a curved one.
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
    pending: Option<&AnchorSnapshot>,
    view: ViewTransform,
    is_hovering_close_target: bool,
) -> DrawList {
    let mut list = DrawList::default();
    if nodes.is_empty() && pending.is_none() {
        return list;
    }

    // Already-placed nodes/handles render in the same shapes but
    // hollow/outline-only in accent — distinct from a committed path's
    // node-tool selection state, so a maker never reads "still drawing"
    // as "already selected" (`specification.md`'s UX notes). The
    // connecting stroke reuses the exact tessellation a committed path
    // gets, so a placed curve (acceptance criterion 2) already previews
    // correctly with no extra code here.
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    list.extend(stroke::path_stroke(
        nodes,
        false,
        STROKE_WIDTH_MM,
        theme::ACCENT,
        tolerance_mm,
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

    if let Some(pending) = pending {
        // Acceptance criterion 2's live curve preview, and acceptance
        // criterion 1's plain-click preview while a press is held down —
        // `pending` is already resolved to exactly one or the other
        // (`PenTool::pending_anchor`/`resolve_anchor`), so this is purely
        // reading its fields, never deriving them.

        // Reshape the B→pending segment live — only when a B exists to
        // connect from; dragging while placing the very first node of a
        // brand-new path has no prior node yet, so there is nothing to
        // reshape (pending's own handles below still preview). A
        // sub-threshold press previews a straight segment the same way,
        // since `pending.handle_in` is then `Vec2::ZERO`.
        if let Some(last) = nodes.last() {
            list.extend(stroke::segment_stroke(
                last.point,
                last.handle_out,
                pending.handle_in,
                pending.point,
                STROKE_WIDTH_MM,
                theme::ACCENT,
                tolerance_mm,
            ));
        }

        // Both symmetric handle lines/endpoints growing from C in real
        // time — only drawn once the press has moved past the drag
        // threshold (`pending.handle_in`/`handle_out` are both
        // `Vec2::ZERO` below it, matching the corner node that would
        // actually commit; a zero handle has no line/endpoint to show,
        // same rule `vecmanf-ui-core::hit_test` already uses for a
        // committed corner node's handles). Filled accent endpoints,
        // matching `docs/design-system.md`'s "being dragged: filled
        // accent" handle convention — this drag is live, the same visual
        // a node tool handle drag already uses.
        for handle in [pending.handle_out, pending.handle_in] {
            if handle == Vec2::ZERO {
                continue;
            }
            let endpoint = pending.point.translated(handle);
            list.extend(glyphs::thick_line(
                pending.point,
                endpoint,
                line_width,
                theme::ACCENT,
            ));
            list.extend(glyphs::circle(endpoint, handle_diameter, theme::ACCENT));
        }

        // Pending's own node glyph, in the same hollow style as every
        // other placed node above, by its own resolved `kind` — a corner
        // node below the drag threshold, a smooth node past it, matching
        // exactly what would commit.
        let glyph = match pending.kind {
            AnchorKind::Corner => glyphs::square,
            AnchorKind::Smooth => glyphs::diamond,
        };
        list.extend(glyph(pending.point, node_size, theme::ACCENT));
        list.extend(glyph(
            pending.point,
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
    /// slice fixes: a held press resolved to a smooth node with handles
    /// (what `vecmanf_ui_core::PenTool::pending_anchor` hands this
    /// function — constructed here directly, the same shape that method
    /// would resolve to, since this crate's own job is just drawing it)
    /// must draw strictly more geometry than the same moment with no
    /// press held — the B→C curve segment plus C's own symmetric handle
    /// lines/endpoints, not just the plain rubber-band line to the
    /// cursor.
    #[test]
    fn dragging_while_placing_previews_the_live_curve_and_handles() {
        let nodes = [NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))];
        let cursor = Some(Point::new(13.0, 4.0));
        let pending = NewAnchor {
            id: AnchorId::new(1, 2),
            point: Point::new(10.0, 0.0),
            handle_in: Vec2::new(-3.0, -4.0),
            handle_out: Vec2::new(3.0, 4.0),
            kind: AnchorKind::Smooth,
        };

        let hovering_only =
            build_pen_preview(&nodes, cursor, None, ViewTransform::identity(), false);
        let dragging = build_pen_preview(
            &nodes,
            cursor,
            Some(&pending),
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
        let cursor = Some(Point::new(5.0, 5.0));
        let pending = NewAnchor {
            id: AnchorId::new(1, 1),
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-5.0, -5.0),
            handle_out: Vec2::new(5.0, 5.0),
            kind: AnchorKind::Smooth,
        };
        let dragging = build_pen_preview(
            &[],
            cursor,
            Some(&pending),
            ViewTransform::identity(),
            false,
        );
        assert!(
            !dragging.triangles.is_empty(),
            "C's own handle lines/endpoints and node glyph must still draw"
        );
    }

    /// A zero-length drag (the cursor hasn't moved off the press point
    /// yet) resolves to a corner node with no handles
    /// (`vecmanf_ui_core::PenTool::resolve_anchor`'s own rule) — this
    /// must not draw degenerate zero-length handle lines, and must draw
    /// the square corner glyph, not the diamond smooth one.
    #[test]
    fn a_zero_length_drag_draws_no_handle_geometry() {
        let origin = Point::new(5.0, 5.0);
        let pending = NewAnchor::corner(AnchorId::new(1, 1), origin);
        let dragging = build_pen_preview(
            &[],
            Some(origin),
            Some(&pending),
            ViewTransform::identity(),
            false,
        );
        // Only C's own node glyph (hollow square: outline + fill, 2
        // quads each) should draw — no handle lines, no handle endpoints.
        assert_eq!(dragging.triangle_count(), 4);
    }

    /// The bug this run fixes, pinned directly at this crate's own
    /// boundary: a `pending` resolved below the drag threshold (a corner
    /// node, `handle_in`/`handle_out` both zero) must render the corner
    /// glyph and no handle geometry, exactly like the zero-length case —
    /// `build_pen_preview` must never re-derive "is this a drag" from
    /// `cursor` distance itself, only ever read `pending.kind`/handles as
    /// given.
    #[test]
    fn a_sub_threshold_pending_anchor_previews_a_corner_not_a_smooth_node() {
        let origin = Point::new(0.0, 0.0);
        // A small but nonzero cursor offset — if this function still did
        // its own distance math, it could mistake this for a drag.
        let cursor = Some(Point::new(0.3, 0.0));
        let pending = NewAnchor::corner(AnchorId::new(1, 1), origin);
        let corner_preview = build_pen_preview(
            &[],
            cursor,
            Some(&pending),
            ViewTransform::identity(),
            false,
        );

        let smooth_pending = NewAnchor {
            id: AnchorId::new(1, 1),
            point: origin,
            handle_in: Vec2::new(-0.3, 0.0),
            handle_out: Vec2::new(0.3, 0.0),
            kind: AnchorKind::Smooth,
        };
        let smooth_preview = build_pen_preview(
            &[],
            cursor,
            Some(&smooth_pending),
            ViewTransform::identity(),
            false,
        );

        assert!(
            corner_preview.triangle_count() < smooth_preview.triangle_count(),
            "a corner `pending` (no handles) must draw strictly less geometry than a smooth \
             one at the same cursor position — proving the handle lines/endpoints come from \
             `pending`'s own fields, not from re-deriving drag distance in this crate"
        );
    }
}
