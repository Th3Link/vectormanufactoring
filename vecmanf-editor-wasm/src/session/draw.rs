//! `Session`'s per-frame draw-list assembly: every tool's decorations and
//! previews combined into the one list the host submits. Split out of
//! `session/mod.rs` (`docs/technical-debt.md`, "`Session` is one module
//! past the size limit").

use vecmanf_render_core::{
    DrawList, build_draw_list, build_pen_preview, build_select_draw_list, build_transform_draw_list,
};

use super::{Session, Tool};

impl Session {
    /// Builds this frame's draw list from the document's current state,
    /// the active view transform, and the node tool's selection/hover —
    /// plus the pen tool's in-progress preview
    /// (`specification.md`'s UX notes) when it is active, every
    /// primitive's own stroke/selection/handle decorations, and (when a
    /// shape-tool drag is in flight) its own live preview outline
    /// (`specs/0003-primitive-shapes/specification.md`, "Live creation
    /// feedback").
    ///
    /// When the node tool has a node/handle drag in flight
    /// (acceptance criteria 8, 9, 10's "update live during the drag"),
    /// `live_node_drag_paths` (private: this module's own internal step,
    /// not part of its public surface) substitutes that drag's live,
    /// not-yet-committed position/handle values into the snapshot before
    /// anything downstream ever sees it — `vecmanf-render-core` needs no
    /// drag-specific code of its own for this: it already draws whatever
    /// `PathSnapshot` it is handed, so a locally live-overridden one
    /// reshapes the stroke and every decoration exactly as if it had
    /// already committed.
    #[must_use]
    pub fn draw_list(&self) -> DrawList {
        let view = self.view();
        let paths = self.live_node_drag_paths();
        let mut list = build_draw_list(&paths, view, &self.decoration_input());
        let primitives = self.primitives_for_render();
        list.extend(vecmanf_render_core::build_shape_draw_list(
            &primitives,
            view,
            &self.shape_decoration_input(),
        ));
        // The blue half of blue-new, black-old: the geometry a release would
        // commit, over the committed objects drawn above and under the boxes
        // and handles below (`specs/unified-object-editing` criterion 10).
        if let Some(live) = self.select_live_edit() {
            list.extend(vecmanf_render_core::build_live_edit_preview(
                &live.objects,
                view,
            ));
        }
        list.extend(build_select_draw_list(
            view,
            &self.select_decoration_input(),
        ));
        list.extend(build_transform_draw_list(
            view,
            &self.select_transform_decoration_input(),
        ));
        if let Some((live_shape, rotation)) = self.live_preview_shape() {
            list.extend(vecmanf_render_core::build_shape_live_preview(
                &live_shape,
                rotation,
                view,
            ));
        }
        if self.tool == Tool::Pen
            && let Some(nodes) = self.pen.in_progress_nodes()
        {
            // The id this pending anchor would actually get if the
            // gesture ended right now — `peek`, never `mint`: a preview
            // must not advance the minter's own counter out of step with
            // what might still be escaped or turn into a close gesture
            // instead (`AnchorIdMinter::peek`'s own doc comment).
            let pending = self.pointer_position.and_then(|cursor| {
                self.pen
                    .pending_anchor(self.minter.peek(), cursor, self.drag_threshold())
            });
            list.extend(build_pen_preview(
                nodes,
                self.pointer_position,
                pending.as_ref(),
                view,
                self.is_hovering_pen_close_target(),
            ));
        }
        list
    }
}
