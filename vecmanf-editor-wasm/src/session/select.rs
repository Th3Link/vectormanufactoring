//! `Session`'s Select-tool glue (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23): dispatching
//! pointer events to [`vecmanf_ui_core::SelectTool`], its hover/decoration
//! input, and the double-click handoff. Split out of `session/mod.rs` the
//! same way `session/shapes.rs` is for the shape tools (architect review:
//! one responsibility per file) — a child module of `session`, so it
//! shares `Session`'s privacy boundary and its methods below join the
//! same type's `impl Session` `session/mod.rs` itself defines.

use vecmanf_document_core::{ObjectSnapshot, Point, Shape};
use vecmanf_render_core::{SelectDecorationInput, TransformDecorationInput, TransformHandleGlyph};
use vecmanf_ui_core::{
    SelectDoubleClickOutcome, SelectTool, TransformHandle, TransformHandleTolerances, double_click,
    hit_test_object, is_corner, oriented_bounds, resize_cursor_angle_degrees,
};

use super::{
    Session, TRANSFORM_RESIZE_HANDLE_TOLERANCE_PX, TRANSFORM_ROTATE_HANDLE_OFFSET_PX,
    TRANSFORM_ROTATE_HANDLE_TOLERANCE_PX, Tool,
};

/// A box side (screen px) under which only the four corner handles are
/// drawn: three times the 8 px resize-handle glyph, so the glyphs never
/// touch (`docs/design-system.md`, "Transform handle hit priority").
const MIN_SIDE_FOR_ALL_HANDLES_PX: f64 = 24.0;

/// Which tool a primitive of this shape is created/edited with — the one
/// mapping both directions funnel through
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "`Session` maps
/// kind to `Tool` with **one** function, `tool_for`... The existing
/// `shape_matches_active_tool` is then derived from it, so the two
/// directions of the mapping cannot drift apart").
pub(super) fn tool_for_shape(shape: &Shape) -> Tool {
    match shape {
        Shape::Rect { .. } => Tool::Rectangle,
        Shape::Ellipse { .. } => Tool::Ellipse,
        Shape::Polygon { .. } | Shape::Star { .. } => Tool::PolygonStar,
    }
}

/// Which tool a double-clicked object hands off to (acceptance criteria
/// 22, 23): a path to the Node tool, a primitive to its own creation
/// tool via [`tool_for_shape`].
pub(super) fn tool_for(object: &ObjectSnapshot) -> Tool {
    match object {
        ObjectSnapshot::Path(_) => Tool::Node,
        ObjectSnapshot::Primitive(primitive) => tool_for_shape(&primitive.shape),
    }
}

impl Session {
    /// The three tolerances the Select tool's own transform handles need
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criterion 1), converted from screen pixels to document
    /// millimetres at the current zoom — the same conversion every
    /// other hit-test tolerance in this module already uses.
    fn transform_handle_tolerances(&self) -> TransformHandleTolerances {
        let scale = self.view().scale();
        TransformHandleTolerances {
            resize: vecmanf_document_core::Tolerance::from_mm(
                TRANSFORM_RESIZE_HANDLE_TOLERANCE_PX / scale,
            ),
            rotate: vecmanf_document_core::Tolerance::from_mm(
                TRANSFORM_ROTATE_HANDLE_TOLERANCE_PX / scale,
            ),
            rotate_offset_mm: TRANSFORM_ROTATE_HANDLE_OFFSET_PX / scale,
        }
    }

    /// Acceptance criteria 1, 4-18: dispatches a press to the Select
    /// tool — first against the current single-object selection's own
    /// transform handles, then (unchanged) against every object's body.
    pub(super) fn select_pointer_down(&mut self, point: Point, shift: bool) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let handle_tolerances = self.transform_handle_tolerances();
        self.select.pointer_down(
            &objects,
            &mut self.selection,
            point,
            tolerance,
            handle_tolerances,
            shift,
        );
    }

    /// Acceptance criteria 3, 15-18, 20: commits whatever move/resize/
    /// rotate drag [`Session::select_pointer_down`] began. `shift`/
    /// `ctrl` are the modifiers' state at release.
    pub(super) fn select_pointer_up(&mut self, point: Point, shift: bool, ctrl: bool) {
        let objects = self.objects();
        self.select.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            point,
            shift,
            ctrl,
        );
    }

    /// Updates the Select tool's own hover state (UX notes: "Hover, tool
    /// not yet clicked into selection: same box at `--accent-hover`").
    pub(super) fn select_hover(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        self.hovered_object = hit_test_object(&objects, point, tolerance);
    }

    /// Acceptance criteria 22, 23: a double-click while the Select tool
    /// is active hands off to the hit object's own tool — clearing the
    /// node tool's selection for a path (so it is "ready for node
    /// editing with no nodes selected", `adrs.md`), or selecting the
    /// primitive (so its own shape tool shows its handles at once).
    pub(super) fn select_double_click(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let outcome = double_click(&objects, point, tolerance);
        let SelectDoubleClickOutcome::Hit(object) = outcome else {
            return;
        };
        let target = tool_for(&object);
        match &object {
            ObjectSnapshot::Path(_) => {
                self.node.escape();
            }
            ObjectSnapshot::Primitive(primitive) => {
                self.selection.select_single(primitive.id);
            }
        }
        self.tool = target;
    }

    /// Builds the Select tool's decoration input for this frame: every
    /// selected object's own box, plus a hovered-but-unselected one — each
    /// box computed from the *live* (possibly drag-translated, -resized
    /// or -rotated) geometry, via [`vecmanf_ui_core::oriented_bounds`] —
    /// so a rotated object's outline turns with it instead of
    /// re-squaring to the screen axes (`object-transform` acceptance
    /// criterion 18; the hover outline uses the same rule).
    pub(super) fn select_decoration_input(&self) -> SelectDecorationInput {
        if self.tool != Tool::Select {
            return SelectDecorationInput::default();
        }
        let mut objects: Vec<ObjectSnapshot> = self
            .live_node_drag_paths()
            .into_iter()
            .map(ObjectSnapshot::Path)
            .collect();
        objects.extend(
            self.primitives_for_render()
                .into_iter()
                .map(ObjectSnapshot::Primitive),
        );
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
        let hovered = self
            .hovered_object
            .filter(|&id| !self.selection.contains(id))
            .and_then(|id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, oriented_bounds(object).document_corners()))
            });
        SelectDecorationInput { selected, hovered }
    }

    /// Builds the Select tool's transform-handle overlay for this frame
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criteria 1, 14-17, 22): every handle of the current single-object
    /// selection, using the *live* (possibly drag-resized/rotated)
    /// object so the handles themselves track the live preview, plus the
    /// active pivot marker while a drag is in flight.
    ///
    ///
    /// The bounding-box outline (`select_decoration_input`) and these
    /// handles are both oriented to the object's own rotation.
    pub(super) fn select_transform_decoration_input(&self) -> TransformDecorationInput {
        if self.tool != Tool::Select {
            return TransformDecorationInput::default();
        }
        let mut objects: Vec<ObjectSnapshot> = self
            .live_node_drag_paths()
            .into_iter()
            .map(ObjectSnapshot::Path)
            .collect();
        objects.extend(
            self.primitives_for_render()
                .into_iter()
                .map(ObjectSnapshot::Primitive),
        );
        let tolerances = self.transform_handle_tolerances();
        let dragging = self.select.dragging_handle();
        let hovered = if dragging.is_none() {
            self.select_hovered_handle(&objects)
        } else {
            None
        };
        // Below ~24 px (3 × the 8 px glyph) a box side is too short for eight
        // handle glyphs — they merge into a blob — so only the four
        // corners are drawn. Hit-testing is unchanged: every handle still
        // works where it is.
        let corners_only = match self.selection.ids() {
            [only] => objects.iter().find(|o| o.id() == *only).is_some_and(|o| {
                let b = oriented_bounds(o);
                b.width().min(b.height()) * self.view().scale() < MIN_SIDE_FOR_ALL_HANDLES_PX
            }),
            _ => false,
        };
        let handles = SelectTool::transform_handles(&objects, &self.selection, tolerances)
            .into_iter()
            .filter(|(handle, _)| match handle {
                TransformHandle::Resize(direction) => !corners_only || is_corner(*direction),
                TransformHandle::Rotate => true,
            })
            .map(|(handle, position)| TransformHandleGlyph {
                position,
                is_rotate: matches!(handle, TransformHandle::Rotate),
                dragging: dragging == Some(handle),
                hovered: hovered == Some(handle),
            })
            .collect();
        let pivot_marker = self.select.live_pivot(self.select_shift_held);
        TransformDecorationInput {
            handles,
            pivot_marker,
        }
    }

    /// The transform handle currently under the pointer, if any — the one
    /// hit test a press would use (`SelectTool::handle_at`).
    fn select_hovered_handle(&self, objects: &[ObjectSnapshot]) -> Option<TransformHandle> {
        let point = self.pointer_position?;
        SelectTool::handle_at(
            objects,
            &self.selection,
            point,
            self.transform_handle_tolerances(),
        )
        .map(|(_, _, handle)| handle)
    }

    /// Which cursor the canvas should show (`specs/0005-object-transform/
    /// specification.md`'s UX notes, "Cursor feedback"), as a plain string
    /// the host turns into CSS: `"default"` (the Select tool's normal
    /// arrow/move cursor, also every other tool), `"rotate"` (the
    /// non-rotating circular-arrow cursor over — or while dragging — the
    /// rotate handle), or `"resize:<degrees>"` (a double-headed arrow
    /// rotated to that on-screen angle, clockwise from horizontal: the
    /// handle's own base angle plus the object's rotation).
    #[must_use]
    pub fn cursor_hint(&self) -> String {
        if self.tool != Tool::Select {
            return "default".to_string();
        }
        let objects = self.objects();
        let handle = self
            .select
            .dragging_handle()
            .or_else(|| self.select_hovered_handle(&objects));
        match handle {
            Some(TransformHandle::Rotate) => "rotate".to_string(),
            Some(TransformHandle::Resize(direction)) => {
                let rotation = match self.selection.ids() {
                    [only] => objects.iter().find(|o| o.id() == *only).map_or_else(
                        || vecmanf_document_core::Angle::from_radians(0.0),
                        ObjectSnapshot::rotation,
                    ),
                    _ => vecmanf_document_core::Angle::from_radians(0.0),
                };
                format!(
                    "resize:{:.1}",
                    resize_cursor_angle_degrees(direction, rotation)
                )
            }
            None => "default".to_string(),
        }
    }

    /// The on-canvas numeric readout for an in-flight Select-tool resize
    /// or rotate (acceptance criteria 14, 22): the object's live size in
    /// millimetres ("W × H mm"; a polygon/star's single outer radius,
    /// "r R mm") or its live rotation in degrees ("37.4°", with no
    /// decimal when exactly whole, as under Ctrl's 15° snap), anchored at
    /// the pointer. `None` outside such a drag.
    pub(super) fn select_live_readout(&self) -> Option<super::shapes::LiveReadout> {
        if self.tool != Tool::Select {
            return None;
        }
        let anchor = self.pointer_position?;
        let live = self.select_live_transform()?;
        let text = match self.select.dragging_handle()? {
            TransformHandle::Rotate => format_degrees(live.rotation().as_radians().to_degrees()),
            TransformHandle::Resize(_) => match &live {
                ObjectSnapshot::Primitive(p)
                    if matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }) =>
                {
                    let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = p.shape else {
                        return None;
                    };
                    format!("r {:.1} mm", frame.radius.as_mm())
                }
                other => {
                    let b = oriented_bounds(other);
                    format!("{:.1} × {:.1} mm", b.width(), b.height())
                }
            },
        };
        Some(super::shapes::LiveReadout { text, anchor })
    }
}

/// `37.4°`, or `45°` when the value is whole (exact under Ctrl's snap).
fn format_degrees(degrees: f64) -> String {
    if (degrees - degrees.round()).abs() < 1e-6 {
        format!("{:.0}°", degrees.round() + 0.0)
    } else {
        format!("{degrees:.1}°")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Angle, Length, RectBounds};

    /// A 10 × 10 mm rectangle at the origin, selected under the Select
    /// tool (a click on its left edge).
    fn session_with_selected_rect() -> (Session, vecmanf_document_core::NodeId) {
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
        let on_outline = Point::new(5.0, 5.0).translated(
            vecmanf_document_core::Vec2::new(-5.0, 0.0).rotated(Angle::from_radians(0.5)),
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
        )
        .into_iter()
        .find(|(h, _)| matches!(h, TransformHandle::Rotate))
        .expect("rotate handle")
        .1;
        session.pointer_down(handle, false);
        let center = Point::new(5.0, 5.0);
        let to_handle = center.vector_to(handle);
        let swing = |deg: f64| {
            let a = to_handle.y.atan2(to_handle.x) + deg.to_radians();
            center.translated(vecmanf_document_core::Vec2::new(
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
        )
        .into_iter()
        .find(|(h, _)| {
            matches!(
                h,
                TransformHandle::Resize(vecmanf_ui_core::ResizeDirection::N)
            )
        })
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
        )
        .into_iter()
        .find(|(h, _)| matches!(h, TransformHandle::Rotate))
        .expect("rotate handle")
        .1;
        session.pointer_hover(rotate_handle, false, false);
        assert_eq!(session.cursor_hint(), "rotate");
    }

    /// UX review item 3: below ~24 px a box side only the four corner
    /// handles (plus rotate) are drawn; a normal-sized box shows all nine.
    /// Hit-testing is unaffected (it works from `transform_handles`).
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
        assert_eq!(draw_count(5.0), 5, "4 corners + rotate");
        assert_eq!(draw_count(10.0), 9, "8 resize + rotate");
    }
}
