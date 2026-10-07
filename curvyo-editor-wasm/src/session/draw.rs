//! `Session`'s per-frame draw-list assembly: every tool's decorations and
//! previews combined into the one list the host submits. Split out of
//! `session/mod.rs` (`docs/technical-debt.md`, "`Session` is one module
//! past the size limit").

use curvyo_render_core::{
    DrawList, TransformDecorationInput, build_draw_list, build_pen_preview, build_select_draw_list,
    build_transform_draw_list,
};

use curvyo_document_core::{ObjectSnapshot, PrimitiveSnapshot};

use super::{Session, Tool};

impl Session {
    /// Builds this frame's draw list from the document's current state,
    /// the active view transform, and the node tool's selection/hover —
    /// plus the pen tool's in-progress preview
    /// (`specification.md`'s UX notes) when it is active, every
    /// primitive's own stroke, the Select tool's boxes and handles, and (when a
    /// creation-tool drag is in flight) its live preview outline
    /// (`specs/0003-primitive-shapes/specification.md`, "Live creation
    /// feedback").
    ///
    /// When the node tool has a node/handle drag in flight
    /// (acceptance criteria 8, 9, 10's "update live during the drag"),
    /// `live_node_drag_paths_in` (private: this module's own internal step,
    /// not part of its public surface) substitutes that drag's live,
    /// not-yet-committed position/handle values into the snapshot before
    /// anything downstream ever sees it — `curvyo-render-core` needs no
    /// drag-specific code of its own for this: it already draws whatever
    /// `PathSnapshot` it is handed, so a locally live-overridden one
    /// reshapes the stroke and every decoration exactly as if it had
    /// already committed.
    #[must_use]
    pub fn draw_list(&self) -> DrawList {
        let view = self.view();
        // The document is read once per frame and every step below works on
        // that read: reading the objects out of the document is by far the
        // largest cost of a frame with many objects.
        let objects = self.objects();
        let live = self.select_live_edit_in(&objects);
        let paths = self.live_node_drag_paths_in(&objects);
        let mut list = build_draw_list(&paths, view, &self.decoration_input());
        let primitives = Self::primitives_in(&objects);
        list.extend(curvyo_render_core::build_primitive_strokes(
            &primitives,
            view,
        ));
        // The origin axes of an axis-locked move: above the artwork, below the
        // blue outline, the boxes and the handles (criterion 27).
        if let Some(axes) = self.move_axes_in(&objects) {
            list.extend(curvyo_render_core::build_move_axes(view, &axes));
        }
        // The blue half of blue-new, black-old: the geometry a release would
        // commit, over the committed objects drawn above and under the boxes
        // and handles below (`specs/unified-object-editing` criterion 10).
        if let Some(live) = &live {
            list.extend(curvyo_render_core::build_live_edit_preview(
                &live.objects,
                view,
            ));
        }
        let live_objects = Self::live_objects_in(objects, live.as_ref());
        list.extend(build_select_draw_list(
            view,
            &self.select_decoration_input_in(&live_objects),
        ));
        list.extend(build_transform_draw_list(
            view,
            &self.select_transform_decoration_input_in(&live_objects),
        ));
        if let Some(preview) = self.live_preview() {
            list.extend(curvyo_render_core::build_shape_live_preview(
                &preview.shape,
                curvyo_document_core::Angle::from_radians(0.0),
                view,
            ));
            // Shift makes the press point the centre: the pivot marker says so
            // (`docs/design-system.md`, "Modifiers in a rectangle or ellipse
            // create-drag").
            if preview.centre.is_some() {
                list.extend(build_transform_draw_list(
                    view,
                    &TransformDecorationInput {
                        pivot_marker: preview.centre,
                        ..TransformDecorationInput::default()
                    },
                ));
            }
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

    /// The primitives among `objects`, in z-order.
    fn primitives_in(objects: &[ObjectSnapshot]) -> Vec<PrimitiveSnapshot> {
        objects
            .iter()
            .filter_map(|object| match object {
                ObjectSnapshot::Primitive(primitive) => Some(*primitive),
                ObjectSnapshot::Path(_) => None,
            })
            .collect()
    }
}
