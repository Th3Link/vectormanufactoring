//! The ellipse/circle tool's create-drag (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 7 and 8; `specs/unified-object-
//! editing/`, criteria 25 and 26): see [`crate::rectangle_tool`], whose
//! pattern this follows.

use vecmanf_document_core::{Document, EllipseFrame, Point, Shape};

use crate::modifiers::Modifiers;
use crate::shape_tool_common::{CreateOutcome, CreatePreview, create_drag_box};

#[derive(Debug, Clone, Copy)]
struct Drag {
    down_at: Point,
    current: Point,
    modifiers: Modifiers,
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
            modifiers: Modifiers::NONE,
        });
    }

    /// The pointer moved, or a modifier changed with the pointer at rest
    /// (`specs/shape-creation-from-center/` criterion 8), with the drag in
    /// flight; writes nothing.
    pub fn pointer_move(&mut self, point: Point, modifiers: Modifiers) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
            drag.modifiers = modifiers;
        }
    }

    /// The live, uncommitted preview: `None` when idle or still degenerate.
    #[must_use]
    pub fn live_shape(&self) -> Option<CreatePreview> {
        let drag = self.drag?;
        let (frame, anchor) = created_frame(drag.down_at, drag.current, drag.modifiers)?;
        Some(CreatePreview {
            shape: Shape::Ellipse { frame },
            anchor,
        })
    }

    /// Acceptance criteria 7, 8: commits the create-drag, built from the
    /// release event's position and `modifiers` (criterion 10 of
    /// `specs/shape-creation-from-center/`), the same computation as the
    /// preview.
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        modifiers: Modifiers,
    ) -> CreateOutcome {
        let Some(drag) = self.drag.take() else {
            return CreateOutcome::NoOp;
        };
        created_frame(drag.down_at, point, modifiers).map_or(CreateOutcome::NoOp, |(frame, _)| {
            CreateOutcome::Created(document.create_ellipse(frame))
        })
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

/// The one computation of the ellipse a drag from `down_at` to `point` makes
/// under `modifiers`, and the readout anchor; `None` when it makes none.
/// Preview and release both call it.
fn created_frame(
    down_at: Point,
    point: Point,
    modifiers: Modifiers,
) -> Option<(EllipseFrame, Point)> {
    let b = create_drag_box(down_at, point, modifiers)?;
    Some((EllipseFrame::from_corners(b.corner_a, b.corner_b), b.anchor))
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
        tool.pointer_move(Point::new(20.0, 10.0), Modifiers::NONE);
        let preview = tool.live_shape().expect("a preview");
        assert_eq!(preview.anchor, Point::new(20.0, 10.0));
        let CreateOutcome::Created(id) =
            tool.pointer_up(&document, Point::new(20.0, 10.0), Modifiers::NONE)
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
        let CreateOutcome::Created(circle) = tool.pointer_up(
            &document,
            Point::new(20.0, 10.0),
            Modifiers::new(false, true),
        ) else {
            panic!("expected Created");
        };
        let Shape::Ellipse { frame } = document.primitive(circle).expect("exists").shape else {
            panic!("an ellipse");
        };
        assert_eq!(frame.rx, frame.ry);

        tool.pointer_down(Point::new(3.0, 3.0));
        assert_eq!(
            tool.pointer_up(&document, Point::new(3.0, 3.0), Modifiers::NONE),
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
            tool.pointer_up(&document, Point::new(5.0, 5.0), Modifiers::NONE),
            CreateOutcome::NoOp
        );
        assert_eq!(document.object_ids().len(), 0);
    }
}
