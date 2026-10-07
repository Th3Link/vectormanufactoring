//! The ellipse/circle tool's create-drag (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 7 and 8; `specs/unified-object-
//! editing/`, criteria 25 and 26): see [`crate::rectangle_tool`], whose
//! pattern this follows.

use vecmanf_document_core::{Document, EllipseFrame, Point, Shape};

use crate::shape_tool_common::{CreateOutcome, CreatePreview, constrained_endpoint, is_degenerate};

#[derive(Debug, Clone, Copy)]
struct Drag {
    down_at: Point,
    current: Point,
    constrain: bool,
}

/// The ellipse tool's state: the create-drag in flight, if any.
#[derive(Debug, Default)]
pub struct EllipseTool {
    drag: Option<Drag>,
}

impl EllipseTool {
    /// A tool with no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 7, 8: a press at `point` starts a create-drag.
    pub fn pointer_down(&mut self, point: Point) {
        self.drag = Some(Drag {
            down_at: point,
            current: point,
            constrain: false,
        });
    }

    /// The pointer moved with the drag in flight; writes nothing.
    pub fn pointer_move(&mut self, point: Point, constrain: bool) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
            drag.constrain = constrain;
        }
    }

    /// The live, uncommitted preview: `None` when idle or still degenerate.
    #[must_use]
    pub fn live_shape(&self) -> Option<CreatePreview> {
        let drag = self.drag?;
        let end = if drag.constrain {
            constrained_endpoint(drag.down_at, drag.current)
        } else {
            drag.current
        };
        (!is_degenerate(drag.down_at, end)).then(|| CreatePreview {
            shape: Shape::Ellipse {
                frame: EllipseFrame::from_corners(drag.down_at, end),
            },
            anchor: end,
        })
    }

    /// Acceptance criteria 7, 8: commits the create-drag; `constrain` is
    /// Ctrl's state at release.
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        constrain: bool,
    ) -> CreateOutcome {
        let Some(drag) = self.drag.take() else {
            return CreateOutcome::NoOp;
        };
        let end = if constrain {
            constrained_endpoint(drag.down_at, point)
        } else {
            point
        };
        if is_degenerate(drag.down_at, end) {
            return CreateOutcome::NoOp;
        }
        CreateOutcome::Created(
            document.create_ellipse(EllipseFrame::from_corners(drag.down_at, end)),
        )
    }

    /// Whether a create-drag is in flight (the button is down), also while it
    /// is still degenerate and shows no preview.
    #[must_use]
    pub const fn drag_in_flight(&self) -> bool {
        self.drag.is_some()
    }

    /// Discards any in-progress drag, writing nothing. Returns whether there
    /// was one.
    pub fn escape(&mut self) -> bool {
        self.drag.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC7, AC8: a drag from A to B is an ellipse with half-extent radii,
    /// Ctrl makes a circle, A == B creates nothing, the preview matches.
    #[test]
    fn ac7_ac8_ellipse_drag_and_circle_constrain() {
        let document = Document::new(1);
        let mut tool = EllipseTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        tool.pointer_move(Point::new(20.0, 10.0), false);
        let preview = tool.live_shape().expect("a preview");
        assert_eq!(preview.anchor, Point::new(20.0, 10.0));
        let CreateOutcome::Created(id) = tool.pointer_up(&document, Point::new(20.0, 10.0), false)
        else {
            panic!("expected Created");
        };
        let shape = document.primitive(id).expect("exists").shape;
        assert_eq!(shape, preview.shape);
        let Shape::Ellipse { frame } = shape else {
            panic!("an ellipse");
        };
        assert_eq!(frame.rx.as_mm(), 10.0);
        assert_eq!(frame.ry.as_mm(), 5.0);

        tool.pointer_down(Point::new(0.0, 0.0));
        let CreateOutcome::Created(circle) =
            tool.pointer_up(&document, Point::new(20.0, 10.0), true)
        else {
            panic!("expected Created");
        };
        let Shape::Ellipse { frame } = document.primitive(circle).expect("exists").shape else {
            panic!("an ellipse");
        };
        assert_eq!(frame.rx, frame.ry);

        tool.pointer_down(Point::new(3.0, 3.0));
        assert_eq!(
            tool.pointer_up(&document, Point::new(3.0, 3.0), false),
            CreateOutcome::NoOp
        );
    }

    #[test]
    fn escape_mid_drag_writes_nothing() {
        let document = Document::new(1);
        let mut tool = EllipseTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        assert!(tool.escape());
        assert_eq!(
            tool.pointer_up(&document, Point::new(5.0, 5.0), false),
            CreateOutcome::NoOp
        );
        assert_eq!(document.object_ids().len(), 0);
    }
}
