//! What `Session` shows for the Select tool (`specs/0004-canvas-navigation-
//! and-selection`, `specs/0005-object-transform`, `specs/object-transform-
//! refinements`): the selection and hover boxes, the transform-handle
//! overlay with its pivot marker and skew guide, the cursor and hint for
//! the handle under the pointer, and the live numeric readout of a drag.
//! Split out of `session/select.rs`, which dispatches the events.

use vecmanf_document_core::{Angle, ObjectSnapshot, Shape, Vec2};
use vecmanf_render_core::{
    SelectDecorationInput, TransformDecorationInput, TransformGlyphKind, TransformHandleGlyph,
};
use vecmanf_ui_core::{
    SelectTool, Side, TransformHandle, format_degrees, is_corner, is_drawn_handle, oriented_bounds,
    resize_cursor_angle_degrees, skew_cursor_angle_degrees,
};

use super::Session;
use super::Tool;

/// How far the skew guide extends past each end of the box, screen pixels
/// (`docs/design-system.md`, "Transform skew fixed-line guide").
const SKEW_GUIDE_EXTEND_PX: f64 = 12.0;

impl Session {
    /// The live (possibly drag-translated, -resized, -rotated or -skewed)
    /// objects of every kind, paths first.
    fn live_objects(&self) -> Vec<ObjectSnapshot> {
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
        objects
    }

    /// Builds the Select tool's decoration input for this frame: every
    /// selected object's own box, plus a hovered-but-unselected one — each
    /// box computed from the *live* geometry, via
    /// [`vecmanf_ui_core::oriented_bounds`] — so a rotated object's outline
    /// turns with it instead of re-squaring to the screen axes
    /// (`object-transform` acceptance criterion 18; the hover outline uses
    /// the same rule), and a skewed path's box is the tight oriented
    /// rectangle around the skewed preview (criterion 45).
    pub(super) fn select_decoration_input(&self) -> SelectDecorationInput {
        if self.tool != Tool::Select {
            return SelectDecorationInput::default();
        }
        let objects = self.live_objects();
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

    /// The handle under the pointer, if any, with its object and box — the
    /// one hit rule a press would use, plus the centre handle for hover
    /// (`SelectTool::hover_handle_at`). Only while no drag runs and no
    /// entry is open.
    fn select_hovered_handle(
        &self,
        objects: &[ObjectSnapshot],
    ) -> Option<(
        ObjectSnapshot,
        vecmanf_ui_core::OrientedBox,
        TransformHandle,
    )> {
        if self.select.dragging_handle().is_some() || self.select.entry().is_some() {
            return None;
        }
        let point = self.pointer_position?;
        SelectTool::hover_handle_at(
            objects,
            &self.selection,
            point,
            self.transform_handle_tolerances(),
            self.select_shift_held,
        )
        .map(|(object, box_, handle)| (object.clone(), box_, handle))
    }

    /// Builds the Select tool's transform-handle overlay for this frame
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criteria 1, 14-17, 22; `object-transform-refinements` 1, 5-8, 37,
    /// 55, 56): every handle of the current single-object selection, using
    /// the *live* object so the handles track the live preview, the pivot
    /// marker while a drag runs or an entry is open (or, with Shift held,
    /// the pivot the hovered handle would use), and the skew guide.
    pub(super) fn select_transform_decoration_input(&self) -> TransformDecorationInput {
        if self.tool != Tool::Select {
            return TransformDecorationInput::default();
        }
        let objects = self.live_objects();
        let tolerances = self.transform_handle_tolerances();
        let dragging = self.select.dragging_handle();
        let entry_handle = self
            .select
            .entry()
            .map(vecmanf_ui_core::TransformEntry::handle);
        let highlighted = dragging.or(entry_handle);
        let hover = self.select_hovered_handle(&objects);
        let hovered = hover.as_ref().map(|(_, _, handle)| *handle);
        let side_rotate = self.select.side_rotate_revealed(self.select_shift_held);
        let [only] = self.selection.ids() else {
            return TransformDecorationInput::default();
        };
        let Some(object) = objects.iter().find(|o| o.id() == *only) else {
            return TransformDecorationInput::default();
        };
        let box_ = oriented_bounds(object);
        // The centre handle hides while a resize, rotate or skew drag runs
        // or an entry is open: the pivot marker may live there.
        let hide_center = highlighted.is_some_and(|handle| handle != TransformHandle::Move);
        let (sin, cos) = box_.angle.as_radians().sin_cos();
        let handles =
            SelectTool::transform_handles(&objects, &self.selection, tolerances, side_rotate)
                .into_iter()
                .filter(|(handle, _)| {
                    is_drawn_handle(*handle, &box_, &tolerances)
                        && !(hide_center && *handle == TransformHandle::Move)
                })
                .map(|(handle, position)| TransformHandleGlyph {
                    position,
                    kind: match handle {
                        TransformHandle::Resize(_) => TransformGlyphKind::Resize,
                        TransformHandle::Rotate(_) => TransformGlyphKind::Rotate,
                        TransformHandle::Skew(side) => TransformGlyphKind::Skew {
                            direction: if side.skews_along_u() {
                                Vec2::new(cos, sin)
                            } else {
                                Vec2::new(-sin, cos)
                            },
                        },
                        TransformHandle::Move => TransformGlyphKind::Move,
                    },
                    dragging: highlighted == Some(handle),
                    hovered: hovered == Some(handle),
                })
                .collect();
        let live_pivot = self.select.live_pivot(self.select_shift_held);
        // Criterion 55: Shift held, nothing running, the pointer on a handle:
        // the marker previews the point that handle would use.
        let preview = if self.select_shift_held && highlighted.is_none() {
            hover.as_ref().and_then(|(object, box_, handle)| {
                SelectTool::hover_pivot(object, box_, *handle, true)
            })
        } else {
            None
        };
        let skew_guide = self.select.skew_guide(
            self.select_shift_held,
            SKEW_GUIDE_EXTEND_PX / self.view().scale(),
        );
        TransformDecorationInput {
            handles,
            pivot_marker: live_pivot.or(preview),
            skew_guide,
        }
    }

    /// The handle the cursor and hint describe: the one being dragged, else
    /// the one under the pointer.
    fn select_cursor_handle(&self, objects: &[ObjectSnapshot]) -> Option<TransformHandle> {
        self.select.dragging_handle().or_else(|| {
            self.select_hovered_handle(objects)
                .map(|(_, _, handle)| handle)
        })
    }

    /// Which cursor the canvas should show (`specs/0005-object-transform/
    /// specification.md`'s UX notes, "Cursor feedback"; `object-transform-
    /// refinements`' "Cursors"), as a plain string the host turns into CSS:
    /// `"default"` (the Select tool's normal cursor, also every other tool),
    /// `"rotate"` (the non-rotating circular arrow over — or while dragging
    /// — any of the eight rotate handles), `"resize:<degrees>"` or
    /// `"skew:<degrees>"` (a double or paired arrow rotated to that
    /// on-screen angle, clockwise from horizontal: the handle's own base
    /// angle plus the object's rotation) or `"move"` (the centre handle).
    /// Never changes with Shift or Ctrl.
    #[must_use]
    pub fn cursor_hint(&self) -> String {
        if self.tool != Tool::Select {
            return "default".to_string();
        }
        let objects = self.objects();
        let rotation = match self.selection.ids() {
            [only] => objects
                .iter()
                .find(|o| o.id() == *only)
                .map_or(Angle::from_radians(0.0), ObjectSnapshot::rotation),
            _ => Angle::from_radians(0.0),
        };
        match self.select_cursor_handle(&objects) {
            Some(TransformHandle::Rotate(_)) => "rotate".to_string(),
            Some(TransformHandle::Resize(direction)) => {
                format!(
                    "resize:{:.1}",
                    resize_cursor_angle_degrees(direction, rotation)
                )
            }
            Some(TransformHandle::Skew(side)) => {
                format!("skew:{:.1}", skew_cursor_angle_degrees(side, rotation))
            }
            Some(TransformHandle::Move) => "move".to_string(),
            None => "default".to_string(),
        }
    }

    /// Which hint the handle under the pointer earns (criterion 54), as a
    /// plain string for the host's hover chip: `""` (none), `"resize-edge"`,
    /// `"resize-corner"`, `"resize-corner-uniform"` (polygon and star: no
    /// Ctrl line), `"rotate-corner"`, `"rotate-side"`, `"skew"` or `"move"`.
    /// Empty while a drag runs or an entry is open.
    #[must_use]
    pub fn handle_hint(&self) -> String {
        if self.tool != Tool::Select {
            return String::new();
        }
        let objects = self.objects();
        let Some((object, _, handle)) = self.select_hovered_handle(&objects) else {
            return String::new();
        };
        let uniform = matches!(
            &object,
            ObjectSnapshot::Primitive(p)
                if matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. })
        );
        match handle {
            TransformHandle::Resize(direction) if !is_corner(direction) => "resize-edge",
            TransformHandle::Resize(_) if uniform => "resize-corner-uniform",
            TransformHandle::Resize(_) => "resize-corner",
            TransformHandle::Rotate(direction) if is_corner(direction) => "rotate-corner",
            TransformHandle::Rotate(_) => "rotate-side",
            TransformHandle::Skew(_) => "skew",
            TransformHandle::Move => "move",
        }
        .to_string()
    }

    /// The on-canvas numeric readout for an in-flight Select-tool resize,
    /// rotate or skew (acceptance criteria 14, 22 of slice 5; 35, 40 here):
    /// the object's live size in millimetres ("W × H mm"; a polygon/star's
    /// single outer radius, "r R mm"), its live rotation ("37.4°", one
    /// decimal at most, "22.5°" on a snap stop) or the skew ("Skew x
    /// +12.5°", a real minus sign when negative), anchored at the pointer.
    /// `None` outside such a drag.
    pub(super) fn select_live_readout(&self) -> Option<super::shapes::LiveReadout> {
        if self.tool != Tool::Select {
            return None;
        }
        let anchor = self.pointer_position?;
        let handle = self.select.dragging_handle()?;
        let text = match handle {
            TransformHandle::Skew(side) => {
                let angle = self.select.live_skew_angle(
                    anchor,
                    self.select_shift_held,
                    self.select_ctrl_held,
                )?;
                skew_readout(side, angle.as_radians().to_degrees())
            }
            TransformHandle::Rotate(_) => {
                let live = self.select_live_transform()?;
                format_degrees(live.rotation().as_radians().to_degrees())
            }
            TransformHandle::Resize(_) => match &self.select_live_transform()? {
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
            TransformHandle::Move => return None,
        };
        Some(super::shapes::LiveReadout { text, anchor })
    }
}

/// `Skew x +12.5°`: the axis (x for the top and bottom handles, y for left
/// and right), the sign (a real minus, U+2212) and one decimal at most.
fn skew_readout(side: Side, degrees: f64) -> String {
    let axis = if side.skews_along_u() { 'x' } else { 'y' };
    let magnitude = format_degrees(degrees.abs());
    let sign = if magnitude == "0°" {
        ""
    } else if degrees < 0.0 {
        "\u{2212}"
    } else {
        "+"
    };
    format!("Skew {axis} {sign}{magnitude}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Angle, Length, Point, RectBounds};

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
            false,
        )
        .into_iter()
        .find(|(h, _)| matches!(h, TransformHandle::Rotate(_)))
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
            false,
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
            false,
        )
        .into_iter()
        .find(|(h, _)| matches!(h, TransformHandle::Rotate(_)))
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
}
