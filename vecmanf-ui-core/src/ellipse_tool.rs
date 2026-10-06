//! The ellipse/circle tool's state machine (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 7-9) — see
//! [`crate::rectangle_tool`]'s own doc comment for the pattern this
//! follows, including [`crate::LiveShape`]'s live-preview plumbing
//! (ux-engineer review).

use vecmanf_document_core::{
    Angle, Document, EllipseFrame, NodeId, Point, PrimitiveSnapshot, Shape,
};

use crate::ObjectSelection;
use crate::handle_layout::{self, HandleKind, ResizeDirection, local_delta, resize_ellipse_frame};
use crate::shape_hit_test::{hit_test_handle, hit_test_primitive};
use crate::shape_tool_common::{
    LiveShape, ShapeHitTolerances, apply_selection_click, constrained_endpoint, ellipses_only,
    is_degenerate,
};
use crate::transform_primitive::pin_ellipse_resize;

#[derive(Debug, Default)]
enum EllipseDrag {
    #[default]
    None,
    Creating {
        down_at: Point,
        current: Point,
        constrain: bool,
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        current: Point,
        start_frame: EllipseFrame,
        /// The primitive's own rotation (acceptance criterion 25).
        rotation: Angle,
    },
}

/// What [`EllipseTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EllipsePointerDownOutcome {
    /// Nothing was hit; a create-drag began.
    Creating,
    /// An existing ellipse was hit and (now) selected.
    Selected,
    /// A resize handle was hit; a drag began.
    Handle,
}

/// What [`EllipseTool::pointer_up`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EllipsePointerUpOutcome {
    /// No drag was in flight, or it moved nowhere.
    NoOp,
    /// A new ellipse was created and committed.
    Created(NodeId),
    /// An existing ellipse was resized (acceptance criterion 9).
    Resized,
}

/// The ellipse/circle tool's state (acceptance criteria 7-9).
#[derive(Debug, Default)]
pub struct EllipseTool {
    drag: EllipseDrag,
}

impl EllipseTool {
    /// A tool with nothing selected and no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 7, 8 (create-drag start), 9 (selecting an
    /// existing ellipse, or starting a resize-handle drag on it). The
    /// constrain (Ctrl) flag is seeded by the first
    /// [`EllipseTool::pointer_move`] call instead — same reasoning as
    /// [`crate::RectangleTool::pointer_down`]'s own doc comment.
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerances: ShapeHitTolerances,
        shift: bool,
    ) -> EllipsePointerDownOutcome {
        let ellipses = ellipses_only(primitives);

        if let [only] = selection.ids()
            && let Some(snapshot) = ellipses.iter().find(|p| p.id == *only)
            && let Shape::Ellipse { frame } = snapshot.shape
        {
            let handles = handle_layout::handles_for(snapshot);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                let HandleKind::Resize(direction) = handles[index].kind else {
                    // invariant: `ellipse_handles` only ever produces
                    // `Resize` handles.
                    unreachable!("an ellipse shows only resize handles")
                };
                self.drag = EllipseDrag::Resizing {
                    id: snapshot.id,
                    direction,
                    down_at: point,
                    current: point,
                    start_frame: frame,
                    rotation: snapshot.rotation,
                };
                return EllipsePointerDownOutcome::Handle;
            }
        }

        let hit = hit_test_primitive(&ellipses, point, tolerances.outline);
        if let Some(id) = hit {
            apply_selection_click(selection, Some(id), shift);
            return EllipsePointerDownOutcome::Selected;
        }
        apply_selection_click(selection, None, shift);
        self.drag = EllipseDrag::Creating {
            down_at: point,
            current: point,
            constrain: false,
        };
        EllipsePointerDownOutcome::Creating
    }

    /// The pointer moved to `point` with the drag still in flight —
    /// updates [`EllipseTool::live_shape`]'s return value, writes
    /// nothing to the document. A no-op when no drag is in progress.
    pub fn pointer_move(&mut self, point: Point, constrain: bool) {
        match &mut self.drag {
            EllipseDrag::Creating {
                current,
                constrain: stored_constrain,
                ..
            } => {
                *current = point;
                *stored_constrain = constrain;
            }
            EllipseDrag::Resizing { current, .. } => *current = point,
            EllipseDrag::None => {}
        }
    }

    /// The live, uncommitted preview of whatever drag is in flight —
    /// `None` when idle or a create-drag is still degenerate.
    #[must_use]
    pub fn live_shape(&self) -> Option<LiveShape> {
        match self.drag {
            EllipseDrag::None => None,
            EllipseDrag::Creating {
                down_at,
                current,
                constrain,
            } => {
                let end = if constrain {
                    constrained_endpoint(down_at, current)
                } else {
                    current
                };
                if is_degenerate(down_at, end) {
                    return None;
                }
                let frame = EllipseFrame::from_corners(down_at, end);
                Some(LiveShape::Creating(Shape::Ellipse { frame }, end))
            }
            EllipseDrag::Resizing {
                direction,
                down_at,
                current,
                start_frame,
                rotation,
                ..
            } => {
                let delta = local_delta(rotation, down_at, current);
                let frame = pin_ellipse_resize(
                    start_frame,
                    resize_ellipse_frame(start_frame, direction, delta),
                    direction,
                    rotation,
                );
                Some(LiveShape::Adjusting(Shape::Ellipse { frame }))
            }
        }
    }

    /// Acceptance criteria 7, 8, 9.
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        constrain: bool,
    ) -> EllipsePointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            EllipseDrag::None => EllipsePointerUpOutcome::NoOp,
            EllipseDrag::Creating { down_at, .. } => {
                let end = if constrain {
                    constrained_endpoint(down_at, point)
                } else {
                    point
                };
                if is_degenerate(down_at, end) {
                    return EllipsePointerUpOutcome::NoOp;
                }
                let frame = EllipseFrame::from_corners(down_at, end);
                EllipsePointerUpOutcome::Created(document.create_ellipse(frame))
            }
            EllipseDrag::Resizing {
                id,
                direction,
                down_at,
                start_frame,
                rotation,
                ..
            } => {
                if point == down_at {
                    return EllipsePointerUpOutcome::NoOp;
                }
                let delta = local_delta(rotation, down_at, point);
                let frame = pin_ellipse_resize(
                    start_frame,
                    resize_ellipse_frame(start_frame, direction, delta),
                    direction,
                    rotation,
                );
                let _ = document.set_ellipse_frame(id, frame);
                EllipsePointerUpOutcome::Resized
            }
        }
    }

    /// Discards any in-progress drag, writing nothing.
    pub fn escape(&mut self) -> bool {
        let had_drag = !matches!(self.drag, EllipseDrag::None);
        self.drag = EllipseDrag::None;
        had_drag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, Length, Tolerance};

    const TOLERANCES: ShapeHitTolerances = ShapeHitTolerances {
        outline: Tolerance::from_mm(1.0),
        handle: Tolerance::from_mm(2.0),
    };

    fn snapshots(document: &Document) -> Vec<PrimitiveSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.primitive(id))
            .collect()
    }

    /// `object-transform` acceptance criterion 25: a rotated ellipse's
    /// handles sit on its turned bounding box and drag along its own
    /// axes — its rotated E handle pulled 4 mm out along local X grows
    /// rx by 2 mm (the box widens by 4 mm, anchored at the far edge).
    #[test]
    fn ac25_a_rotated_ellipses_own_handles_follow_and_drag_in_local_axes() {
        use vecmanf_document_core::Vec2;
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let angle = Angle::from_radians(45.0_f64.to_radians());
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(0.0, 0.0), angle),
            )
            .expect("rotate");
        let mut tool = EllipseTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let e_handle = Point::new(5.0, 0.0).rotated_around(Point::new(0.0, 0.0), angle);
        let outcome = tool.pointer_down(
            &snapshots(&document),
            &mut selection,
            e_handle,
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, EllipsePointerDownOutcome::Handle);
        tool.pointer_up(
            &document,
            e_handle.translated(Vec2::new(4.0, 0.0).rotated(angle)),
            false,
        );
        let Shape::Ellipse { frame } = document.primitive(id).expect("exists").shape else {
            panic!("expected ellipse");
        };
        assert!((frame.rx.as_mm() - 7.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 5.0).abs() < 1e-9);
    }

    /// Architect item 4, for the ellipse tool: the far (NW) point of a
    /// rotated ellipse's bounding box stays put while its SE handle is
    /// dragged.
    #[test]
    fn a_rotated_ellipses_own_se_resize_keeps_the_opposite_corner_put() {
        use vecmanf_document_core::Vec2;
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(3.0),
        });
        let angle = Angle::from_radians(30.0_f64.to_radians());
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(0.0, 0.0), angle),
            )
            .expect("rotate");
        // The local NW box corner (-5, -3) on screen.
        let nw = Point::new(-5.0, -3.0).rotated_around(Point::new(0.0, 0.0), angle);
        let mut tool = EllipseTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let se = Point::new(5.0, 3.0).rotated_around(Point::new(0.0, 0.0), angle);
        let outcome =
            tool.pointer_down(&snapshots(&document), &mut selection, se, TOLERANCES, false);
        assert_eq!(outcome, EllipsePointerDownOutcome::Handle);
        tool.pointer_up(
            &document,
            se.translated(Vec2::new(4.0, 2.0).rotated(angle)),
            false,
        );
        let p = document.primitive(id).expect("exists");
        let Shape::Ellipse { frame } = p.shape else {
            panic!("ellipse");
        };
        assert!((frame.rx.as_mm() - 7.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 4.0).abs() < 1e-9);
        let nw_after = Point::new(
            frame.center.x - frame.rx.as_mm(),
            frame.center.y - frame.ry.as_mm(),
        )
        .rotated_around(frame.center, p.rotation);
        assert!((nw_after.x - nw.x).abs() < 1e-9, "{nw_after:?} vs {nw:?}");
        assert!((nw_after.y - nw.y).abs() < 1e-9);
    }

    /// AC7, AC8: an ellipse drag creates rx/ry from the bbox, and Ctrl
    /// constrains to a circle — live, not just at release.
    #[test]
    fn ac7_ac8_ellipse_drag_and_circle_constrain() {
        let document = Document::new(1);
        let mut tool = EllipseTool::new();
        let mut selection = ObjectSelection::new();

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_move(Point::new(20.0, 10.0), false);
        let Some(LiveShape::Creating(Shape::Ellipse { frame: live_frame }, anchor)) =
            tool.live_shape()
        else {
            panic!("expected a Creating ellipse preview");
        };
        assert!((live_frame.rx.as_mm() - 10.0).abs() < 1e-9);
        assert_eq!(anchor, Point::new(20.0, 10.0));

        let outcome = tool.pointer_up(&document, Point::new(20.0, 10.0), false);
        let EllipsePointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Ellipse { frame } = snapshot.shape else {
            panic!("expected ellipse");
        };
        assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 5.0).abs() < 1e-9);

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(20.0, 10.0), true);
        let EllipsePointerUpOutcome::Created(circle_id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(circle_id).expect("exists");
        let Shape::Ellipse { frame } = snapshot.shape else {
            panic!("expected ellipse");
        };
        assert!(
            (frame.rx.as_mm() - frame.ry.as_mm()).abs() < 1e-9,
            "a circle"
        );
    }

    /// AC9: resizing an ellipse's handle can make rx != ry (a circle
    /// reshaped into a non-circular ellipse); the live preview shows it
    /// along the way.
    #[test]
    fn ac9_ellipse_resize_can_break_circularity() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(10.0),
        });
        let mut tool = EllipseTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0),
            TOLERANCES,
            false,
        );
        tool.pointer_move(Point::new(20.0, 0.0), false);
        let Some(LiveShape::Adjusting(Shape::Ellipse { frame: live_frame })) = tool.live_shape()
        else {
            panic!("expected an Adjusting ellipse preview");
        };
        assert!((live_frame.rx.as_mm() - 15.0).abs() < 1e-9);

        tool.pointer_up(&document, Point::new(20.0, 0.0), false);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Ellipse { frame } = snapshot.shape else {
            panic!("expected ellipse");
        };
        assert!((frame.rx.as_mm() - 15.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 10.0).abs() < 1e-9);
    }
}
