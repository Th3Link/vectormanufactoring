//! The polygon/star tool's create-drag and its settings for the next shape
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criteria 10,
//! 11, 12; `specs/unified-object-editing/`, criteria 25, 26, 29): see
//! [`crate::rectangle_tool`], whose pattern this follows. Mode, point count
//! and ratio persist across shapes and never change a selected shape; that is
//! the Select bar's job.

use vecmanf_document_core::{Document, InnerRatio, Point, PointCount, Shape, StarFrame};

use crate::shape_tool_common::{CreateOutcome, CreatePreview, is_degenerate};

/// Polygon or star mode (acceptance criteria 11 vs. 12), fixed before a
/// drag starts; an existing shape's mode never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyStarMode {
    /// Acceptance criterion 11: a regular N-sided polygon.
    Polygon,
    /// Acceptance criterion 12: an N-pointed star.
    Star,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    center: Point,
    current: Point,
}

/// The polygon/star tool's state: the create-drag in flight and the settings
/// for the next shape, which persist across shapes within the session
/// (acceptance criterion 10: "the control is not reset between shapes").
#[derive(Debug)]
pub struct PolygonStarTool {
    drag: Option<Drag>,
    mode: PolyStarMode,
    point_count: PointCount,
    ratio: InnerRatio,
}

impl Default for PolygonStarTool {
    fn default() -> Self {
        Self::new()
    }
}

impl PolygonStarTool {
    /// A tool defaulting to polygon mode, 6 points and a 0.5 ratio; no
    /// acceptance criterion pins these starting values, only that they
    /// persist once changed.
    ///
    /// # Panics
    /// Does not panic in practice: `6` and `0.5` are inside `PointCount`'s
    /// and `InnerRatio`'s valid ranges.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag: None,
            mode: PolyStarMode::Polygon,
            // invariant: 6 is within `PointCount`'s `3..=1024` range.
            #[allow(clippy::unwrap_used)]
            point_count: PointCount::new(6).unwrap(),
            // invariant: 0.5 is within `InnerRatio`'s open `(0, 1)` range.
            #[allow(clippy::unwrap_used)]
            ratio: InnerRatio::new(0.5).unwrap(),
        }
    }

    /// The mode for the next shape (acceptance criteria 11 vs. 12).
    #[must_use]
    pub const fn mode(&self) -> PolyStarMode {
        self.mode
    }

    /// The point count for the next shape (acceptance criterion 10).
    #[must_use]
    pub const fn point_count(&self) -> PointCount {
        self.point_count
    }

    /// The inner ratio for the next star (acceptance criterion 12).
    #[must_use]
    pub const fn ratio(&self) -> InnerRatio {
        self.ratio
    }

    /// The mode toggle: only affects shapes drawn after the switch.
    pub fn set_mode(&mut self, mode: PolyStarMode) {
        self.mode = mode;
    }

    /// The point-count setting for the next shape; it changes no selected
    /// shape (`unified-object-editing` criterion 29).
    pub fn set_point_count(&mut self, count: PointCount) {
        self.point_count = count;
    }

    /// The ratio setting for the next star; it changes no selected shape.
    pub fn set_ratio(&mut self, ratio: InnerRatio) {
        self.ratio = ratio;
    }

    /// Acceptance criteria 11, 12: a press at `point` starts a create-drag
    /// with `point` as the centre.
    pub fn pointer_down(&mut self, point: Point) {
        self.drag = Some(Drag {
            center: point,
            current: point,
        });
    }

    /// The pointer moved with the drag in flight; writes nothing.
    pub fn pointer_move(&mut self, point: Point) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
        }
    }

    fn shape_at(&self, center: Point, vertex: Point) -> Shape {
        let frame = StarFrame::from_center_and_vertex(center, vertex);
        match self.mode {
            PolyStarMode::Polygon => Shape::Polygon {
                frame,
                point_count: self.point_count,
            },
            PolyStarMode::Star => Shape::Star {
                frame,
                point_count: self.point_count,
                inner_ratio: self.ratio,
            },
        }
    }

    /// The live, uncommitted preview: `None` when idle or still degenerate.
    #[must_use]
    pub fn live_shape(&self) -> Option<CreatePreview> {
        let drag = self.drag?;
        (!is_degenerate(drag.center, drag.current)).then(|| CreatePreview {
            shape: self.shape_at(drag.center, drag.current),
            anchor: drag.current,
        })
    }

    /// Acceptance criteria 11, 12: commits the create-drag.
    pub fn pointer_up(&mut self, document: &Document, point: Point) -> CreateOutcome {
        let Some(drag) = self.drag.take() else {
            return CreateOutcome::NoOp;
        };
        if is_degenerate(drag.center, point) {
            return CreateOutcome::NoOp;
        }
        let frame = StarFrame::from_center_and_vertex(drag.center, point);
        CreateOutcome::Created(match self.mode {
            PolyStarMode::Polygon => document.create_polygon(frame, self.point_count),
            PolyStarMode::Star => document.create_star(frame, self.point_count, self.ratio),
        })
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

    fn created(tool: &mut PolygonStarTool, document: &Document, centre: Point, to: Point) -> Shape {
        tool.pointer_down(centre);
        let CreateOutcome::Created(id) = tool.pointer_up(document, to) else {
            panic!("expected Created");
        };
        document.primitive(id).expect("exists").shape
    }

    /// AC10: the point count persists across shapes and is not reset.
    #[test]
    fn ac10_the_settings_persist_across_shapes() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        tool.set_point_count(PointCount::new(9).expect("count"));
        tool.set_mode(PolyStarMode::Star);
        tool.set_ratio(InnerRatio::new(0.3).expect("ratio"));
        for x in [10.0, 40.0] {
            let Shape::Star {
                point_count,
                inner_ratio,
                ..
            } = created(
                &mut tool,
                &document,
                Point::new(0.0, 0.0),
                Point::new(x, 0.0),
            )
            else {
                panic!("a star");
            };
            assert_eq!(point_count.get(), 9);
            assert!((inner_ratio.get() - 0.3).abs() < 1e-12);
        }
    }

    /// AC11: a polygon is centred at A with a vertex at B; A == B creates
    /// nothing; the preview is what is committed.
    #[test]
    fn ac11_polygon_drag_centers_at_a_one_vertex_at_b() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        tool.pointer_down(Point::new(5.0, 5.0));
        assert_eq!(tool.live_shape(), None);
        tool.pointer_move(Point::new(15.0, 5.0));
        let preview = tool.live_shape().expect("a preview");
        let CreateOutcome::Created(id) = tool.pointer_up(&document, Point::new(15.0, 5.0)) else {
            panic!("expected Created");
        };
        let shape = document.primitive(id).expect("exists").shape;
        assert_eq!(shape, preview.shape);
        let Shape::Polygon { frame, point_count } = shape else {
            panic!("a polygon");
        };
        assert_eq!(frame.center, Point::new(5.0, 5.0));
        assert!((frame.radius.as_mm() - 10.0).abs() < 1e-12);
        assert_eq!(point_count.get(), 6);
        tool.pointer_down(Point::new(1.0, 1.0));
        assert_eq!(
            tool.pointer_up(&document, Point::new(1.0, 1.0)),
            CreateOutcome::NoOp
        );
    }

    /// The settings never touch an existing shape: changing them writes
    /// nothing (the Select bar owns that).
    #[test]
    fn changing_a_setting_writes_nothing_and_escape_discards_the_drag() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let _ = created(
            &mut tool,
            &document,
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
        );
        tool.set_point_count(PointCount::new(12).expect("count"));
        let Shape::Polygon { point_count, .. } = document
            .primitive(document.object_ids()[0])
            .expect("exists")
            .shape
        else {
            panic!("a polygon");
        };
        assert_eq!(point_count.get(), 6);
        tool.pointer_down(Point::new(0.0, 0.0));
        assert!(tool.escape());
        assert!(!tool.escape());
    }
}
