//! What `Session` draws for the Select tool (`specs/0004-canvas-navigation-
//! and-selection`, `specs/0005-object-transform`, `specs/0008-object-transform-
//! refinements`, `specs/0009-unified-object-editing`): the selection and hover
//! boxes, and the transform- and parameter-handle overlay with its pivot marker,
//! skew guide and radius guides. The cursor and hint are in `select_cursor.rs`,
//! the live readout in `select_readout.rs`; `session/select.rs` dispatches the
//! events.

use curvyo_document_core::{ObjectSnapshot, Vec2};
use curvyo_render_core::{
    SelectDecorationInput, TransformDecorationInput, TransformGlyphKind, TransformHandleGlyph,
};
use curvyo_ui_core::{
    EditHandle, LiveEdit, ParamHandle, SelectTool, corner_local_position, is_drawn_handle,
    oriented_bounds,
};

use super::Session;
use super::Tool;

/// How far the skew guide extends past each end of the box, screen pixels
/// (`docs/design-system.md`, "Transform skew fixed-line guide").
const SKEW_GUIDE_EXTEND_PX: f64 = 16.0;

impl Session {
    /// The Select tool's live edit (`specs/0009-unified-object-editing`, criteria
    /// 10 to 14): what a release at the current pointer position and
    /// modifiers would commit, for the blue half of blue-new, black-old.
    /// `None` outside the Select tool, with nothing in flight, inside the
    /// dead zone, before the pointer has ever moved over the canvas, and
    /// where the result equals the committed objects. The cached
    /// The cached `held` modifiers let this be read at render time.
    pub(super) fn select_live_edit_in(&self, objects: &[ObjectSnapshot]) -> Option<LiveEdit> {
        if self.tool != Tool::Select {
            return None;
        }
        let cursor = self.pointer_position?;
        self.select.live_edit(
            objects,
            &self.selection,
            cursor,
            self.held.shift,
            self.held.ctrl,
        )
    }

    /// The document's objects with `live` substituted: the one place live
    /// geometry enters, and only for the decorations (the boxes, handles,
    /// pivot marker and readout follow the new geometry). The objects
    /// themselves are always drawn as committed, so the old geometry stays on
    /// screen under the blue outline; a copy substitutes nothing. Takes the committed objects by value:
    /// the caller has read them for this one use.
    pub(super) fn live_objects_in(
        mut objects: Vec<ObjectSnapshot>,
        live: Option<&LiveEdit>,
    ) -> Vec<ObjectSnapshot> {
        // In a copy the blue outline travels alone: the boxes, handles and the
        // pivot stay on the originals (`edit-interaction-polish` criterion 33).
        if let Some(live) = live.filter(|live| !live.copy) {
            for new in &live.objects {
                if let Some(slot) = objects.iter_mut().find(|o| o.id() == new.id()) {
                    *slot = new.clone();
                }
            }
        }
        objects
    }

    /// Builds the Select tool's decoration input for this frame: every
    /// selected object's own box, plus a hovered-but-unselected one — each
    /// box computed from the *live* geometry, via
    /// [`curvyo_ui_core::oriented_bounds`] — so a rotated object's outline
    /// turns with it instead of re-squaring to the screen axes
    /// (`object-transform` acceptance criterion 18; the hover outline uses
    /// the same rule), and a skewed path's box is the tight oriented
    /// rectangle around the skewed preview (criterion 45).
    #[cfg(test)]
    pub(super) fn select_decoration_input(&self) -> SelectDecorationInput {
        let objects = self.objects();
        let live = self.select_live_edit_in(&objects);
        self.select_decoration_input_in(&Self::live_objects_in(objects, live.as_ref()))
    }

    /// [`Session::select_decoration_input`] over the live objects the caller
    /// already built, so a frame reads and substitutes once.
    pub(super) fn select_decoration_input_in(
        &self,
        objects: &[ObjectSnapshot],
    ) -> SelectDecorationInput {
        // A creation tool draws no selection box at all (choosing one clears
        // the selection, customer decision 2026-10-06 reversing criterion 27);
        // the Pen and Node tools draw their own.
        if self.tool != Tool::Select {
            return SelectDecorationInput::default();
        }
        let selected = self
            .selection
            .ids()
            .iter()
            .filter_map(|&id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, oriented_bounds(object).document_corners()))
            })
            .collect();
        // No hover highlight of other objects while any Select-tool drag or
        // bar slider edit is in flight.
        let hovering = !self.select.drag_in_flight() && self.select.bar_preview().is_none();
        let hovered = self
            .hovered_object
            .filter(|&id| hovering && !self.selection.contains(id))
            .and_then(|id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, oriented_bounds(object).document_corners()))
            });
        SelectDecorationInput {
            selected,
            hovered,
            device_pixel_ratio: self.device_pixel_ratio,
            skew_guide: self.skew_guide_now(),
        }
    }

    /// The skew fixed-line guide of the drag in flight, if it is a skew drag:
    /// drawn by the transform overlay, and the box leaves its own dashes off
    /// the edge it covers.
    fn skew_guide_now(&self) -> Option<(curvyo_document_core::Point, curvyo_document_core::Point)> {
        self.select
            .skew_guide(self.held.shift, SKEW_GUIDE_EXTEND_PX / self.view().scale())
    }

    /// The handle under the pointer, if any, with its object and box — the
    /// one hit rule a press would use, plus the centre handle for hover
    /// (`SelectTool::hover_handle_at`). Only while no drag runs and no
    /// entry is open.
    pub(super) fn select_hovered_handle(
        &self,
        objects: &[ObjectSnapshot],
    ) -> Option<(ObjectSnapshot, curvyo_ui_core::OrientedBox, EditHandle)> {
        if self.select.drag_in_flight() || self.select.has_entry() {
            return None;
        }
        let point = self.pointer_position?;
        SelectTool::hover_handle_at(
            objects,
            &self.selection,
            point,
            self.transform_handle_tolerances(),
            self.held.shift,
        )
        .map(|(object, box_, handle)| (object.clone(), box_, handle))
    }

    /// Builds the Select tool's handle overlay for this frame
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criteria 1, 14-17, 22; `object-transform-refinements` 1, 5-8, 37,
    /// 55, 56; `unified-object-editing` 1, 7, 8): every handle of the
    /// current single-object selection, using the *live* object so the
    /// handles track the live preview, the pivot marker while a drag runs or
    /// an entry is open (or, with Shift held, the pivot the hovered handle
    /// would use), the skew guide and the radius guides. Parameter handles
    /// are left out while the same object is moved, resized, rotated or
    /// skewed by drag (criterion 7).
    #[cfg(test)]
    pub(super) fn select_transform_decoration_input(&self) -> TransformDecorationInput {
        let objects = self.objects();
        let live = self.select_live_edit_in(&objects);
        self.select_transform_decoration_input_in(&Self::live_objects_in(objects, live.as_ref()))
    }

    /// [`Session::select_transform_decoration_input`] over the live objects
    /// the caller already built, so a frame reads and substitutes once.
    pub(super) fn select_transform_decoration_input_in(
        &self,
        objects: &[ObjectSnapshot],
    ) -> TransformDecorationInput {
        if self.tool != Tool::Select {
            return TransformDecorationInput::default();
        }
        let tolerances = self.transform_handle_tolerances();
        let dragging = self.select.dragging_handle();
        let entry_handle = self.select.entry_handle();
        let highlighted = dragging.or(entry_handle);
        let hover = self.select_hovered_handle(objects);
        let hovered = hover.as_ref().map(|(_, _, handle)| *handle);
        let side_rotate = self.select.side_rotate_revealed(self.held.shift);
        let [only] = self.selection.ids() else {
            return TransformDecorationInput::default();
        };
        let Some(object) = objects.iter().find(|o| o.id() == *only) else {
            return TransformDecorationInput::default();
        };
        let box_ = oriented_bounds(object);
        // The centre handle hides while a resize, rotate, skew or parameter
        // drag runs or an entry is open: the pivot marker may live there, and
        // a parameter drag has it yield (criterion 8).
        let centre_chip = self.select.centre_chip_open();
        let hide_center =
            centre_chip || highlighted.is_some_and(|handle| handle != EditHandle::Move);
        let params_visible = self.select.param_handles_visible();
        // The other three radius knobs follow only while the drag changes all
        // four corners (decided at the press, criterion 23).
        let radius_in_use = matches!(
            highlighted,
            Some(EditHandle::Param(ParamHandle::CornerRadius(_)))
        ) && self.select.corner_drag_changes_all() == Some(true);
        let drawn: Vec<(EditHandle, curvyo_document_core::Point)> =
            SelectTool::transform_handles(objects, &self.selection, tolerances, side_rotate)
                .into_iter()
                .filter(|(handle, _)| {
                    is_drawn_handle(*handle, &box_, &tolerances)
                        && !(hide_center && *handle == EditHandle::Move)
                        && (params_visible || !matches!(handle, EditHandle::Param(_)))
                })
                .collect();
        let mut param_guides = Vec::new();
        let handles = drawn
            .into_iter()
            .map(|(handle, position)| {
                let active = highlighted == Some(handle) || hovered == Some(handle);
                if let (EditHandle::Param(ParamHandle::CornerRadius(corner)), true) =
                    (handle, active)
                {
                    param_guides.push((
                        box_.to_document(corner_local_position(corner, &box_)),
                        position,
                    ));
                }
                TransformHandleGlyph {
                    position,
                    kind: glyph_kind(handle, &box_),
                    dragging: highlighted == Some(handle),
                    // The other three radius handles of a radius drag take the
                    // hover ground: they move in step (criterion 2).
                    hovered: hovered == Some(handle)
                        || (radius_in_use
                            && highlighted != Some(handle)
                            && matches!(handle, EditHandle::Param(ParamHandle::CornerRadius(_)))),
                }
            })
            .collect();
        let live_pivot = self.select.live_pivot(self.held.shift);
        // Criterion 55: Shift held, nothing running, the pointer on a handle:
        // the marker previews the point that handle would use.
        let preview = if self.held.shift && highlighted.is_none() {
            hover.as_ref().and_then(|(object, box_, handle)| {
                SelectTool::hover_pivot(object, box_, *handle, true)
            })
        } else {
            None
        };
        TransformDecorationInput {
            handles,
            pivot_marker: live_pivot.or(preview),
            // The S chip highlights no handle: its marker at the centre is
            // the one sign of the fixed point, so it is drawn solid.
            pivot_marker_full: centre_chip,
            skew_guide: self.skew_guide_now(),
            param_guides,
            device_pixel_ratio: self.device_pixel_ratio,
        }
    }
}

/// The glyph a handle draws; a skew glyph's arrows run along its side's axis
/// in the box's own rotated frame.
fn glyph_kind(handle: EditHandle, box_: &curvyo_ui_core::OrientedBox) -> TransformGlyphKind {
    let (sin, cos) = box_.angle.as_radians().sin_cos();
    match handle {
        EditHandle::Resize(_) => TransformGlyphKind::Resize,
        EditHandle::Rotate(_) => TransformGlyphKind::Rotate,
        EditHandle::Skew(side) => TransformGlyphKind::Skew {
            direction: if side.skews_along_u() {
                Vec2::new(cos, sin)
            } else {
                Vec2::new(-sin, cos)
            },
        },
        EditHandle::Move => TransformGlyphKind::Move,
        EditHandle::Param(_) => TransformGlyphKind::Parameter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{Angle, Length, Point, RectBounds};

    /// A 10 × 10 mm rectangle at the origin, selected under the Select
    /// tool (a click on its left edge).
    fn session_with_selected_rect() -> (Session, curvyo_document_core::NodeId) {
        let mut session = Session::new(1);
        let id = session.document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(0.0, 5.0), false);
        session.pointer_up(Point::new(0.0, 5.0), false, false);
        (session, id)
    }

    /// `edit-interaction-polish` criterion 33: while a copy is dragged the
    /// selection box stays on the original and only the blue outline travels;
    /// in a move the box follows the blue outline, in the frame the key
    /// changes, with the pointer at rest.
    #[test]
    fn the_selection_box_stays_on_the_original_in_a_copy() {
        let (mut session, _) = session_with_selected_rect();
        let original = session.select_decoration_input().selected[0].1;
        // Inside the box and on no handle: a move press.
        let start = Point::new(5.0, 5.0);
        let to = Point::new(30.0, 12.0);
        session.pointer_down(start, false);
        session.pointer_hover(to, false, false);
        let moved = session.select_decoration_input().selected[0].1;
        assert_ne!(moved, original, "a move: the box follows");
        // Ctrl pressed with the pointer at rest: the box jumps back.
        session.modifiers_changed(false, true, false);
        session.pointer_hover(to, false, true);
        assert_eq!(session.select_decoration_input().selected[0].1, original);
        let live = session.select_live_edit_in(&session.objects()).unwrap();
        assert!(live.copy && live.objects.len() == 1, "the copy still shows");
        // Released again: the box follows again.
        session.modifiers_changed(false, false, false);
        session.pointer_hover(to, false, false);
        assert_eq!(session.select_decoration_input().selected[0].1, moved);
    }

    /// `edit-interaction-polish` criterion 65: the device pixel ratio the host
    /// reports reaches the selection decoration, so an axis-aligned box can
    /// snap to whole device pixels; a ratio that is not a positive finite
    /// number reads as 1.
    #[test]
    fn the_device_pixel_ratio_reaches_the_selection_decoration() {
        let (mut session, _) = session_with_selected_rect();
        assert!((session.select_decoration_input().device_pixel_ratio - 1.0).abs() < 1e-12);
        session.set_device_pixel_ratio(1.5);
        assert!((session.select_decoration_input().device_pixel_ratio - 1.5).abs() < 1e-12);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            session.set_device_pixel_ratio(bad);
            assert!((session.select_decoration_input().device_pixel_ratio - 1.0).abs() < 1e-12);
        }
    }

    /// Criterion 63 end to end: a selected rectangle's box is dashed (more
    /// than the four solid quads) and the hover box of another is solid.
    #[test]
    fn a_selected_object_draws_a_dashed_box() {
        let (session, _) = session_with_selected_rect();
        let solid = curvyo_render_core::build_select_draw_list(
            session.view(),
            &SelectDecorationInput {
                hovered: session.select_decoration_input().selected.first().copied(),
                ..SelectDecorationInput::default()
            },
        );
        assert_eq!(
            solid.triangle_count(),
            16,
            "the hover box is four solid quads on four casing quads"
        );
        let dashed = curvyo_render_core::build_select_draw_list(
            session.view(),
            &session.select_decoration_input(),
        );
        assert!(dashed.triangle_count() > solid.triangle_count());
    }

    /// Customer decision 2026-10-06 (reverses criterion 27): choosing a
    /// creation tool, by any route, clears the selection and draws no
    /// selection box; the Node and Pen tools keep the selection.
    #[test]
    fn choosing_a_creation_tool_clears_the_selection_and_draws_no_box() {
        for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
            let (mut session, _) = session_with_selected_rect();
            assert_eq!(session.select_decoration_input().selected.len(), 1);
            session.set_tool(tool);
            assert!(session.selection.ids().is_empty(), "{tool:?}: cleared");
            let input = session.select_decoration_input();
            assert!(input.selected.is_empty(), "{tool:?}: no selection box");
            assert!(input.hovered.is_none(), "{tool:?}: no hover box");
            // Creating a shape still hands over to Select with it selected.
            session.pointer_down(Point::new(20.0, 20.0), false);
            session.pointer_up(Point::new(40.0, 40.0), false, false);
            assert_eq!(session.tool(), Tool::Select, "{tool:?}: hand-over");
            assert_eq!(session.selection.ids().len(), 1, "{tool:?}: new shape");
            assert_eq!(session.select_decoration_input().selected.len(), 1);
        }
        for tool in [Tool::Node, Tool::Pen] {
            let (mut session, id) = session_with_selected_rect();
            session.set_tool(tool);
            assert_eq!(session.selection.ids(), &[id], "{tool:?}: selection kept");
        }
    }

    /// Customer bug 2026-10-06: while a Select-tool drag runs, the other
    /// objects' hover boxes are not drawn and hover does not change the
    /// cursor; both return after the release.
    #[test]
    fn no_hover_highlight_or_hover_cursor_while_a_select_drag_runs() {
        let (mut session, a) = session_with_selected_rect();
        let b = session.document.create_rect(RectBounds {
            origin: Point::new(50.0, 50.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let on_b = Point::new(55.0, 50.0);
        session.pointer_hover(on_b, false, false);
        assert_eq!(
            session.select_decoration_input().hovered.map(|(id, _)| id),
            Some(b),
            "idle hover lights up B"
        );
        // A move drag of A, with the pointer passing over B's outline and
        // over a handle spot of A.
        session.pointer_down(Point::new(5.0, 5.0), false);
        for over in [on_b, Point::new(10.0, 10.0), on_b] {
            session.pointer_hover(over, false, false);
            assert!(
                session.select_decoration_input().hovered.is_none(),
                "no hover box mid-drag at {over:?}"
            );
            assert_eq!(session.cursor_hint(), "default", "no hover cursor mid-drag");
        }
        session.pointer_up(Point::new(5.0, 5.0), false, false);
        assert_eq!(session.selection.ids(), &[a]);
        session.pointer_hover(on_b, false, false);
        assert_eq!(
            session.select_decoration_input().hovered.map(|(id, _)| id),
            Some(b),
            "hover returns after the release"
        );
    }

    /// Acceptance criterion 18: a rotated object's selection outline is
    /// drawn through its own turned corners.
    #[test]
    fn a_rotated_objects_selection_outline_is_oriented_not_axis_aligned() {
        let (session, id) = session_with_selected_rect();
        session
            .document
            .rotate_object(
                &session
                    .document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(5.0, 5.0), Angle::from_radians(0.5)),
            )
            .expect("rotate");
        let input = session.select_decoration_input();
        let (_, corners) = input.selected.first().expect("selected");
        let edge = corners[0].vector_to(corners[1]);
        assert!(
            edge.x.abs() > 1e-6 && edge.y.abs() > 1e-6,
            "an edge of a 0.5 rad-rotated box is neither horizontal nor vertical: {edge:?}"
        );
        assert!((edge.length() - 10.0).abs() < 1e-9, "still the 10 mm side");
    }

    /// The hover outline follows the same rule (adrs.md: "The hover box
    /// uses the same rule").
    #[test]
    fn a_rotated_objects_hover_outline_is_oriented_too() {
        let (mut session, id) = session_with_selected_rect();
        session
            .document
            .rotate_object(
                &session
                    .document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(5.0, 5.0), Angle::from_radians(0.5)),
            )
            .expect("rotate");
        // Deselect, then hover the rotated outline's turned top-left edge.
        session.pointer_down(Point::new(500.0, 500.0), false);
        session.pointer_up(Point::new(500.0, 500.0), false, false);
        let on_outline = Point::new(5.0, 5.0).translated(
            curvyo_document_core::Vec2::new(-5.0, 0.0).rotated(Angle::from_radians(0.5)),
        );
        session.pointer_hover(on_outline, false, false);
        let input = session.select_decoration_input();
        let (_, corners) = input.hovered.expect("hovered");
        let edge = corners[0].vector_to(corners[1]);
        assert!(edge.x.abs() > 1e-6 && edge.y.abs() > 1e-6);
    }

    /// Acceptance criterion 14: a live "W × H mm" readout during a
    /// resize, anchored at the pointer; absent once released.
    #[test]
    fn resize_drag_shows_a_live_size_readout_at_the_pointer() {
        let (mut session, _) = session_with_selected_rect();
        session.pointer_down(Point::new(10.0, 10.0), false); // Se handle
        session.pointer_hover(Point::new(15.0, 13.0), false, false);
        let readout = session.live_readout().expect("a resize is in flight");
        assert_eq!(readout.text, "15.0 × 13.0 mm");
        assert_eq!(readout.anchor, Point::new(15.0, 13.0));
        session.pointer_up(Point::new(15.0, 13.0), false, false);
        assert!(session.live_readout().is_none());
    }

    /// Acceptance criterion 22: a live angle readout, one decimal
    /// unconstrained, whole degrees under Ctrl's 15° snap.
    #[test]
    fn rotate_drag_shows_a_live_angle_readout() {
        let (mut session, _) = session_with_selected_rect();
        let handle = SelectTool::transform_handles(
            &session.objects(),
            &session.selection,
            session.transform_handle_tolerances(),
            false,
        )
        .into_iter()
        .find(|(h, _)| matches!(h, EditHandle::Rotate(_)))
        .expect("rotate handle")
        .1;
        session.pointer_down(handle, false);
        let center = Point::new(5.0, 5.0);
        let to_handle = center.vector_to(handle);
        let swing = |deg: f64| {
            let a = to_handle.y.atan2(to_handle.x) + deg.to_radians();
            center.translated(curvyo_document_core::Vec2::new(
                to_handle.length() * a.cos(),
                to_handle.length() * a.sin(),
            ))
        };
        session.pointer_hover(swing(37.4), false, false);
        assert_eq!(session.live_readout().expect("rotating").text, "37.4°");
        session.pointer_hover(swing(44.0), false, true);
        assert_eq!(session.live_readout().expect("rotating").text, "45°");
    }

    /// UX notes, "Cursor feedback": resize cursors carry the handle's
    /// base angle plus the object's rotation; the rotate handle gets the
    /// non-rotating rotate cursor; elsewhere the default one.
    #[test]
    fn cursor_hint_reports_rotated_resize_rotate_and_default() {
        let (mut session, id) = session_with_selected_rect();
        session.pointer_hover(Point::new(10.0, 5.0), false, false); // E handle
        assert_eq!(session.cursor_hint(), "resize:0.0");
        session.pointer_hover(Point::new(5.0, 0.0), false, false); // N handle
        assert_eq!(session.cursor_hint(), "resize:90.0");
        session.pointer_hover(Point::new(300.0, 300.0), false, false);
        assert_eq!(session.cursor_hint(), "default");

        session
            .document
            .rotate_object(
                &session.document.object(id).expect("object exists").rotated(
                    Point::new(5.0, 5.0),
                    Angle::from_radians(std::f64::consts::FRAC_PI_4),
                ),
            )
            .expect("rotate");
        let n_handle = SelectTool::transform_handles(
            &session.objects(),
            &session.selection,
            session.transform_handle_tolerances(),
            false,
        )
        .into_iter()
        .find(|(h, _)| matches!(h, EditHandle::Resize(curvyo_ui_core::ResizeDirection::N)))
        .expect("N handle")
        .1;
        session.pointer_hover(n_handle, false, false);
        assert_eq!(
            session.cursor_hint(),
            "resize:135.0",
            "90° base + 45° rotation"
        );

        let rotate_handle = SelectTool::transform_handles(
            &session.objects(),
            &session.selection,
            session.transform_handle_tolerances(),
            false,
        )
        .into_iter()
        .find(|(h, _)| matches!(h, EditHandle::Rotate(_)))
        .expect("rotate handle")
        .1;
        session.pointer_hover(rotate_handle, false, false);
        assert_eq!(session.cursor_hint(), "rotate");
    }

    /// UX review item 3: below ~24 px a box side only the four corner
    /// resize handles (plus the four corner rotate handles) are drawn; a
    /// normal-sized box shows all eight resize handles. Hit-testing is
    /// unaffected (it works from `transform_handles`).
    #[test]
    fn a_tiny_box_draws_only_its_corner_handles() {
        let draw_count = |size_mm: f64| {
            let mut session = Session::new(1);
            let id = session.document.create_rect(RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(size_mm),
                height: Length::from_mm(size_mm),
            });
            session.set_tool(Tool::Select);
            session.selection.select_single(id);
            session.select_transform_decoration_input().handles.len()
        };
        // The default 96 dpi view is ~3.8 px per mm: 5 mm is ~19 px, 10 mm ~38 px.
        assert_eq!(draw_count(5.0), 8, "4 corner resize + 4 corner rotate");
        assert_eq!(draw_count(10.0), 12, "8 resize + 4 corner rotate");
    }
    /// The decoration's handle count and pivot marker for a rectangle
    /// selected at the default zoom (a 100 x 60 mm box is 378 x 227 px).
    fn big_rect_session() -> Session {
        let mut session = Session::new(1);
        let _ = session.document.create_rect(RectBounds {
            origin: Point::new(10.0, 20.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(10.0, 50.0), false);
        session.pointer_up(Point::new(10.0, 50.0), false, false);
        session
    }

    fn handle_count(session: &Session) -> usize {
        session.select_transform_decoration_input().handles.len()
    }

    /// A 227 px shorter side is above the 72 px threshold, so a selected
    /// rectangle at rest also draws its four corner-radius handles
    /// (`unified-object-editing` criteria 1, 7).
    const RADIUS_HANDLES: usize = 4;

    /// Criteria 1, 5, 6: eight resize, four corner rotate and the centre
    /// handle; Shift adds four side rotate handles in the same frame, with no
    /// pointer movement, and moves none.
    #[test]
    fn shift_adds_the_side_rotate_handles_and_modifiers_changed_needs_no_pointer() {
        let mut session = big_rect_session();
        assert_eq!(handle_count(&session), 13 + RADIUS_HANDLES);
        session.modifiers_changed(true, false, false);
        assert_eq!(handle_count(&session), 17 + RADIUS_HANDLES);
        session.modifiers_changed(false, false, false);
        assert_eq!(handle_count(&session), 13 + RADIUS_HANDLES);
    }

    /// Criterion 6: the set is frozen during a drag (Shift pressed
    /// mid-drag reveals nothing, a dragged side handle stays when Shift is
    /// released); criterion 1's centre handle is not drawn while a rotate,
    /// resize or skew drag runs, the pivot marker being there.
    #[test]
    fn the_handle_set_is_frozen_and_the_centre_hides_during_a_transform_drag() {
        let mut session = big_rect_session();
        let corner = Point::new(
            110.0 + 32.0 / std::f64::consts::SQRT_2 / session.view().scale(),
            20.0 - 32.0 / std::f64::consts::SQRT_2 / session.view().scale(),
        );
        session.pointer_hover(corner, false, false);
        session.pointer_down(corner, false);
        assert_eq!(
            handle_count(&session),
            12,
            "centre hidden during a rotate, and no radius handle during another drag"
        );
        session.modifiers_changed(true, false, false);
        assert_eq!(handle_count(&session), 12, "Shift mid-drag reveals nothing");
        session.escape();
        session.modifiers_changed(false, false, false);

        let side = Point::new(60.0, 20.0 - 32.0 / session.view().scale());
        session.modifiers_changed(true, false, false);
        session.pointer_hover(side, true, false);
        session.pointer_down(side, true);
        session.modifiers_changed(false, false, false);
        assert_eq!(handle_count(&session), 16, "the dragged side handle stays");
        session.escape();
        assert_eq!(handle_count(&session), 13 + RADIUS_HANDLES);
    }

    /// Criteria 40, 56 and 55: the pivot marker while a drag runs, the
    /// pivot preview under Shift on a hovered handle, and the skew guide.
    #[test]
    fn pivot_marker_preview_and_skew_guide() {
        let mut session = big_rect_session();
        let scale = session.view().scale();
        let corner_offset = 32.0 / std::f64::consts::SQRT_2 / scale;
        let ne_rotate = Point::new(110.0 + corner_offset, 20.0 - corner_offset);
        // Nothing hovered: no marker.
        assert!(
            session
                .select_transform_decoration_input()
                .pivot_marker
                .is_none()
        );
        // Shift held over the corner rotate handle: the marker previews the
        // opposite corner (10, 80), the pivot a Shift drag would use.
        session.modifiers_changed(true, false, false);
        session.pointer_hover(ne_rotate, true, false);
        let preview = session
            .select_transform_decoration_input()
            .pivot_marker
            .unwrap();
        assert!(
            (preview.x - 10.0).abs() < 1e-9 && (preview.y - 80.0).abs() < 1e-9,
            "{preview:?}"
        );
        // Over a resize handle the preview is the box centre.
        session.pointer_hover(Point::new(110.0, 80.0), true, false);
        let preview = session
            .select_transform_decoration_input()
            .pivot_marker
            .unwrap();
        assert!((preview.x - 60.0).abs() < 1e-9 && (preview.y - 50.0).abs() < 1e-9);
        // Shift released: the preview is gone again.
        session.modifiers_changed(false, false, false);
        session.pointer_hover(Point::new(110.0, 80.0), false, false);
        assert!(
            session
                .select_transform_decoration_input()
                .pivot_marker
                .is_none()
        );
        // A rotate drag without Shift shows the centre; with Shift the corner.
        session.pointer_down(ne_rotate, false);
        let marker = session
            .select_transform_decoration_input()
            .pivot_marker
            .unwrap();
        assert!((marker.x - 60.0).abs() < 1e-9 && (marker.y - 50.0).abs() < 1e-9);
        session.modifiers_changed(true, false, false);
        let marker = session
            .select_transform_decoration_input()
            .pivot_marker
            .unwrap();
        assert!((marker.x - 10.0).abs() < 1e-9 && (marker.y - 80.0).abs() < 1e-9);
        session.escape();
        assert!(
            session
                .select_transform_decoration_input()
                .skew_guide
                .is_none()
        );
    }

    /// Criteria 57a and 59: while the chip of the key S is open the pivot
    /// marker shows at the box centre, the fixed point of the typed size; the
    /// chip of a double-click on the same handle shows the opposite corner.
    #[test]
    fn the_s_chip_shows_the_pivot_marker_at_the_box_centre() {
        let mut session = big_rect_session();
        assert_eq!(
            session.key_down(crate::KeyInput {
                key: "s",
                ..crate::KeyInput::default()
            }),
            crate::KeyOutcome::EntryOpened
        );
        let marker = session
            .select_transform_decoration_input()
            .pivot_marker
            .expect("a marker while the chip is open");
        assert!((marker.x - 60.0).abs() < 1e-9 && (marker.y - 50.0).abs() < 1e-9);
        // The S chip: the marker is solid, no handle is in its dragging look,
        // the centre glyph makes way for the marker, and the chip is placed
        // by the centre (criterion 59).
        let input = session.select_transform_decoration_input();
        assert!(input.pivot_marker_full);
        assert!(input.handles.iter().all(|glyph| !glyph.dragging));
        let handles_with_chip = input.handles.len();
        let view = session.transform_entry().expect("the chip");
        assert!(view.at_centre);
        assert!((view.handle.x - 60.0).abs() < 1e-9 && (view.handle.y - 50.0).abs() < 1e-9);
        session.cancel_transform_entry();
        let input = session.select_transform_decoration_input();
        assert!(input.pivot_marker.is_none());
        assert_eq!(
            input.handles.len(),
            handles_with_chip + 1,
            "the centre handle is drawn again"
        );
        let corner = Point::new(110.0, 80.0);
        session.pointer_hover(corner, false, false);
        session.pointer_down(corner, false);
        session.pointer_up(corner, false, false);
        session.double_click(corner, false, false);
        let marker = session
            .select_transform_decoration_input()
            .pivot_marker
            .expect("a marker while the chip is open");
        assert!((marker.x - 10.0).abs() < 1e-9 && (marker.y - 20.0).abs() < 1e-9);
        // The double-click route keeps the handle's dragging look, the chip at
        // the handle and the dimmer marker.
        let input = session.select_transform_decoration_input();
        assert!(!input.pivot_marker_full);
        assert_eq!(
            input.handles.iter().filter(|glyph| glyph.dragging).count(),
            1
        );
        let view = session.transform_entry().expect("the chip");
        assert!(!view.at_centre);
        assert!((view.handle.x - 110.0).abs() < 1e-9 && (view.handle.y - 80.0).abs() < 1e-9);
    }

    /// Criterion 56: a skew drag draws a dashed guide along the fixed line
    /// (the bottom edge for a top handle, the centre line under Shift),
    /// 16 px past each end of the box; it is gone after Escape.
    #[test]
    fn a_skew_drag_draws_the_fixed_line_guide() {
        use curvyo_document_core::{AnchorId, NewAnchor};
        let mut session = Session::new(1);
        let _ = session.document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(10.0, 20.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(110.0, 20.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(110.0, 80.0)),
                NewAnchor::corner(AnchorId::new(1, 4), Point::new(10.0, 80.0)),
            ],
            true,
        );
        session.set_tool(Tool::Select);
        session.pointer_down(Point::new(10.0, 50.0), false);
        session.pointer_up(Point::new(10.0, 50.0), false, false);
        let scale = session.view().scale();
        let top_skew = Point::new(60.0, 20.0 - 16.0 / scale);
        session.pointer_hover(top_skew, false, false);
        session.pointer_down(top_skew, false);
        let guide = session
            .select_transform_decoration_input()
            .skew_guide
            .unwrap();
        let pad = 16.0 / scale;
        assert!((guide.0.x - (10.0 - pad)).abs() < 1e-9 && (guide.0.y - 80.0).abs() < 1e-9);
        assert!((guide.1.x - (110.0 + pad)).abs() < 1e-9 && (guide.1.y - 80.0).abs() < 1e-9);
        session.modifiers_changed(true, false, false);
        let guide = session
            .select_transform_decoration_input()
            .skew_guide
            .unwrap();
        assert!((guide.0.y - 50.0).abs() < 1e-9, "Shift: through the centre");
        // Criterion 68: the box learns the guide, and under a fixed-edge guide
        // leaves its own dashes off that edge (fewer triangles); the centre
        // line under Shift covers none of its edges.
        let box_triangles = |session: &Session| {
            curvyo_render_core::build_select_draw_list(
                session.view(),
                &session.select_decoration_input(),
            )
            .triangle_count()
        };
        assert_eq!(session.select_decoration_input().skew_guide, Some(guide));
        let centre_line = box_triangles(&session);
        session.modifiers_changed(false, false, false);
        let fixed_edge = box_triangles(&session);
        session.escape();
        assert!(session.select_decoration_input().skew_guide.is_none());
        let at_rest = box_triangles(&session);
        assert_eq!(centre_line, at_rest, "Shift: the centre line cuts no edge");
        assert!(fixed_edge < at_rest, "{fixed_edge} vs {at_rest}");
        session.escape();
        assert!(
            session
                .select_transform_decoration_input()
                .skew_guide
                .is_none()
        );
    }
}
