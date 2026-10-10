//! `Session`'s per-frame draw-list assembly: every tool's decorations and
//! previews combined into the one list the host submits. Split out of
//! `session/mod.rs` (`docs/technical-debt.md`, "`Session` is one module
//! past the size limit").

use std::borrow::Cow;

use curvyo_render_core::{
    DrawList, GradientFrame, TransformDecorationInput, build_artwork, build_decorations,
    build_group_draw_list, build_marquee_overlay, build_pen_preview, build_select_draw_list,
    build_transform_draw_list,
};

use curvyo_document_core::{FillKind, ObjectSnapshot, PathSnapshot};
use curvyo_ui_core::oriented_bounds;

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
        // The artwork: every object in tree order, paths and primitives
        // interleaved, each with its fill then its stroke
        // (`specs/0007-stroke-and-fill-styling` criterion 26). The Node
        // tool's live drag reshapes the paths it moves.
        let mut artwork_objects = if self.tool == Tool::Node {
            Cow::Owned(Self::with_paths(&objects, &paths))
        } else {
            Cow::Borrowed(&objects[..])
        };
        // A panel drag draws the objects in the style being previewed
        // (criterion 36); nothing is written until the release.
        if self.style.is_active() {
            self.style.apply_to(artwork_objects.to_mut());
        }
        let frames = Self::gradient_frames(&artwork_objects);
        let mut list = build_artwork(&artwork_objects, &frames, view);
        // A compound path shows no node, handle or segment (`0016-boolean-operations` criterion 38).
        let editable: Vec<_> = paths
            .iter()
            .filter(|path| !path.is_compound())
            .cloned()
            .collect();
        list.extend(build_decorations(&editable, view, &self.decoration_input()));
        // The origin axes of an axis-locked move: above the artwork, below the
        // blue outline, the boxes and the handles (criterion 27).
        if let Some(axes) = self.move_axes_in(&objects) {
            list.extend(curvyo_render_core::build_move_axes(view, &axes));
        }
        // The blue half of blue-new, black-old: the geometry a release would
        // commit, over the committed objects drawn above and under the boxes
        // and handles below (`specs/0009-unified-object-editing` criterion 10).
        if let Some(live) = &live {
            list.extend(curvyo_render_core::build_live_edit_preview(
                &live.objects,
                view,
            ));
        }
        // The red hollow outline of the objects a refused boolean operation names.
        let refused = self.refusal_objects(&objects);
        if !refused.is_empty() {
            list.extend(curvyo_render_core::build_refusal_outlines(&refused, view));
        }
        let live_objects = Self::live_objects_in(objects, live.as_ref());
        list.extend(build_select_draw_list(
            view,
            &self.select_decoration_input_in(&live_objects),
        ));
        if let Some(group) = self.group_decoration_input_in(&live_objects) {
            list.extend(build_group_draw_list(view, &group));
        }
        list.extend(build_transform_draw_list(
            view,
            &self.select_transform_decoration_input_in(&live_objects),
        ));
        // The marquee box or lasso line: above the boxes and handles, the
        // topmost layer of a selection drag.
        if let Some(overlay) = self.marquee_overlay() {
            list.extend(build_marquee_overlay(
                view,
                &overlay,
                self.device_pixel_ratio,
            ));
        }
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
                self.document.size(),
            ));
        }
        list
    }

    /// The box each gradient-filled object's gradient spans: its oriented
    /// selection box (`specs/0007-stroke-and-fill-styling` criteria 21, 22).
    /// Objects without a gradient fill get `None`, so nothing is computed for
    /// them.
    fn gradient_frames(objects: &[ObjectSnapshot]) -> Vec<Option<GradientFrame>> {
        objects
            .iter()
            .map(|object| {
                let fill = &object.style().fill;
                (fill.paints() && fill.kind != FillKind::Solid).then(|| {
                    let oriented = oriented_bounds(object);
                    GradientFrame {
                        min: oriented.min,
                        max: oriented.max,
                        angle: oriented.angle,
                        pivot: oriented.pivot,
                    }
                })
            })
            .collect()
    }

    /// `objects` with each path replaced by the path of the same id in `paths`
    /// (possibly reshaped by a live drag); a path with no match stays as it is.
    fn with_paths(objects: &[ObjectSnapshot], paths: &[PathSnapshot]) -> Vec<ObjectSnapshot> {
        objects
            .iter()
            .map(|object| match object {
                ObjectSnapshot::Path(original) => ObjectSnapshot::Path(
                    paths
                        .iter()
                        .find(|path| path.id == original.id)
                        .unwrap_or(original)
                        .clone(),
                ),
                ObjectSnapshot::Primitive(_) => object.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Document, NewAnchor, Point};

    use super::*;

    /// The live drag's paths replace the artwork's by id, whatever their
    /// order, and a path with no match keeps its own geometry.
    #[test]
    fn dragged_paths_replace_objects_by_id_not_by_position() {
        let document = Document::new(1);
        let make = |y: f64, n: u64| {
            document.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, n), Point::new(0.0, y)),
                    NewAnchor::corner(AnchorId::new(1, n + 1), Point::new(10.0, y)),
                ],
                false,
            )
        };
        let (first, second) = (make(0.0, 1), make(50.0, 3));
        let objects: Vec<ObjectSnapshot> = [first, second]
            .iter()
            .map(|id| document.object(*id).unwrap())
            .collect();
        let mut moved = document.path(second).unwrap();
        moved.anchors[0].point = Point::new(0.0, 99.0);
        // Only the second path is in the list, and it is listed alone.
        let replaced = Session::with_paths(&objects, std::slice::from_ref(&moved));
        let ObjectSnapshot::Path(a) = &replaced[0] else {
            panic!("a path");
        };
        let ObjectSnapshot::Path(b) = &replaced[1] else {
            panic!("a path");
        };
        assert_eq!(a.anchors[0].point, Point::new(0.0, 0.0), "untouched");
        assert_eq!(b.anchors[0].point, Point::new(0.0, 99.0), "the dragged one");
    }
}
