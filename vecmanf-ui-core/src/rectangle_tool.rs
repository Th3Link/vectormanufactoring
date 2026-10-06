//! The rectangle tool's create-drag (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 1 and 2; `specs/unified-object-
//! editing/`, criteria 25 and 26): a press anywhere starts a new rectangle,
//! even on an existing outline, and never selects, moves or handle-drags
//! anything. Ephemeral drag state (ADR 0009 §2), one
//! [`vecmanf_document_core::Document`] commit on release, a zero-movement
//! press writes nothing and leaves the selection alone.

use vecmanf_document_core::{Document, Length, Point, RectBounds, Shape};

use crate::shape_tool_common::{CreateOutcome, CreatePreview, constrained_endpoint, is_degenerate};

#[derive(Debug, Clone, Copy)]
struct Drag {
    down_at: Point,
    current: Point,
    constrain: bool,
}

/// The rectangle tool's state: the create-drag in flight, if any.
#[derive(Debug, Default)]
pub struct RectangleTool {
    drag: Option<Drag>,
}

impl RectangleTool {
    /// A tool with no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 1, 2: a press at `point` starts a create-drag. The
    /// constrain (Ctrl) flag is not known yet at press time and is seeded by
    /// the first [`RectangleTool::pointer_move`].
    pub fn pointer_down(&mut self, point: Point) {
        self.drag = Some(Drag {
            down_at: point,
            current: point,
            constrain: false,
        });
    }

    /// The pointer moved with the drag in flight: updates
    /// [`RectangleTool::live_shape`], writes nothing. A no-op when idle.
    pub fn pointer_move(&mut self, point: Point, constrain: bool) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
            drag.constrain = constrain;
        }
    }

    /// The live, uncommitted preview: `None` when idle or while the drag is
    /// still degenerate (the UX notes: a drag that would create nothing shows
    /// no preview at all).
    #[must_use]
    pub fn live_shape(&self) -> Option<CreatePreview> {
        let drag = self.drag?;
        let end = if drag.constrain {
            constrained_endpoint(drag.down_at, drag.current)
        } else {
            drag.current
        };
        (!is_degenerate(drag.down_at, end)).then(|| CreatePreview {
            shape: rect_shape(drag.down_at, end),
            anchor: end,
        })
    }

    /// Acceptance criteria 1, 2: commits the create-drag. `constrain` is
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
        CreateOutcome::Created(document.create_rect(RectBounds::from_corners(drag.down_at, end)))
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

fn rect_shape(a: Point, b: Point) -> Shape {
    Shape::Rect {
        bounds: RectBounds::from_corners(a, b),
        corner_radius: Length::from_mm(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC1: a drag from A to B creates a rectangle with A, B as opposite
    /// corners and zero radius; A == B creates nothing.
    #[test]
    fn ac1_rectangle_drag_creates_a_zero_radius_rect() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        let CreateOutcome::Created(id) = tool.pointer_up(&document, Point::new(10.0, 20.0), false)
        else {
            panic!("expected Created");
        };
        assert_eq!(
            document.primitive(id).expect("exists").shape,
            rect_shape(Point::new(0.0, 0.0), Point::new(10.0, 20.0))
        );
        tool.pointer_down(Point::new(5.0, 5.0));
        assert_eq!(
            tool.pointer_up(&document, Point::new(5.0, 5.0), false),
            CreateOutcome::NoOp
        );
        assert_eq!(document.object_ids().len(), 1);
    }

    /// During a create-drag the live shape is what `pointer_up` would commit,
    /// and a degenerate drag shows nothing.
    #[test]
    fn the_live_shape_matches_what_pointer_up_would_commit() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        assert_eq!(tool.live_shape(), None);
        tool.pointer_move(Point::new(20.0, 10.0), false);
        let preview = tool.live_shape().expect("a preview");
        assert_eq!(preview.anchor, Point::new(20.0, 10.0));
        let CreateOutcome::Created(id) = tool.pointer_up(&document, Point::new(20.0, 10.0), false)
        else {
            panic!("expected Created");
        };
        assert_eq!(document.primitive(id).expect("exists").shape, preview.shape);
    }

    /// AC2: Ctrl makes a square sized to the larger extent, in the preview too.
    #[test]
    fn ac2_constrain_makes_a_square_sized_to_the_larger_extent() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        tool.pointer_move(Point::new(30.0, 10.0), true);
        assert_eq!(
            tool.live_shape().expect("a preview").shape,
            rect_shape(Point::new(0.0, 0.0), Point::new(30.0, 30.0))
        );
        let CreateOutcome::Created(id) = tool.pointer_up(&document, Point::new(30.0, 10.0), true)
        else {
            panic!("expected Created");
        };
        assert_eq!(
            document.primitive(id).expect("exists").shape,
            rect_shape(Point::new(0.0, 0.0), Point::new(30.0, 30.0))
        );
    }

    #[test]
    fn escape_mid_drag_writes_nothing() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        tool.pointer_down(Point::new(0.0, 0.0));
        tool.pointer_move(Point::new(10.0, 10.0), false);
        assert!(tool.escape());
        assert!(!tool.escape());
        assert_eq!(
            tool.pointer_up(&document, Point::new(10.0, 10.0), false),
            CreateOutcome::NoOp
        );
        assert_eq!(document.object_ids().len(), 0);
    }
}
