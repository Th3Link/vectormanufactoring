//! The polygon/star tool's create-drag and its settings for the next shape
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criteria 10,
//! 11, 12; `specs/unified-object-editing/`, criteria 25, 26, 29): see
//! [`crate::rectangle_tool`], whose pattern this follows. Mode, point count
//! and ratio persist across shapes and never change a selected shape; that is
//! the Select bar's job.

use vecmanf_document_core::{Document, InnerRatio, Point, PointCount, Shape, StarFrame};

use crate::angle_snap::snap_angle;
use crate::modifiers::Modifiers;
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
    /// The modifiers of the latest pointer event; the preview reads them, the
    /// release replaces them with its own event's.
    modifiers: Modifiers,
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
            modifiers: Modifiers::NONE,
        });
    }

    /// The pointer moved with the drag in flight; writes nothing. Ctrl
    /// snaps the created angle (`specs/edit-interaction-polish/` criterion
    /// 4); Shift has no effect on a polygon or star.
    pub fn pointer_move(&mut self, point: Point, modifiers: Modifiers) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
            drag.modifiers = modifiers;
        }
    }

    /// The one computation of the shape a drag from `center` to `vertex`
    /// makes under `modifiers`: preview and release both call it, so what is
    /// drawn is what commits (criterion 5).
    fn shape_at(&self, center: Point, vertex: Point, modifiers: Modifiers) -> Shape {
        let frame = created_frame(center, vertex, modifiers);
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
            shape: self.shape_at(drag.center, drag.current, drag.modifiers),
            anchor: drag.current,
        })
    }

    /// Acceptance criteria 11, 12: commits the create-drag, built from the
    /// release event's position and `modifiers` (criterion 5).
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        modifiers: Modifiers,
    ) -> CreateOutcome {
        let Some(drag) = self.drag.take() else {
            return CreateOutcome::NoOp;
        };
        if is_degenerate(drag.center, point) {
            return CreateOutcome::NoOp;
        }
        let frame = created_frame(drag.center, point, modifiers);
        CreateOutcome::Created(match self.mode {
            PolyStarMode::Polygon => document.create_polygon(frame, self.point_count),
            PolyStarMode::Star => document.create_star(frame, self.point_count, self.ratio),
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

/// The frame of a polygon or star dragged from `center` to `vertex`: the
/// radius is |center vertex| and the angle that of the first vertex. Ctrl
/// replaces the angle by the nearest stop of the rotate snap table
/// ([`snap_angle`]) and keeps the radius, so the first vertex lies on the
/// snapped direction (criterion 4).
fn created_frame(center: Point, vertex: Point, modifiers: Modifiers) -> StarFrame {
    let frame = StarFrame::from_center_and_vertex(center, vertex);
    if !modifiers.ctrl {
        return frame;
    }
    StarFrame {
        center,
        radius: frame.radius,
        angle: snap_angle(frame.angle).normalized(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn created(tool: &mut PolygonStarTool, document: &Document, centre: Point, to: Point) -> Shape {
        tool.pointer_down(centre);
        let CreateOutcome::Created(id) = tool.pointer_up(document, to, Modifiers::NONE) else {
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
        tool.pointer_move(Point::new(15.0, 5.0), Modifiers::NONE);
        let preview = tool.live_shape().expect("a preview");
        let CreateOutcome::Created(id) =
            tool.pointer_up(&document, Point::new(15.0, 5.0), Modifiers::NONE)
        else {
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
            tool.pointer_up(&document, Point::new(1.0, 1.0), Modifiers::NONE),
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

    fn dragged(
        tool: &mut PolygonStarTool,
        document: &Document,
        from: Point,
        to: Point,
        modifiers: Modifiers,
    ) -> (Option<CreatePreview>, Shape) {
        tool.pointer_down(from);
        tool.pointer_move(to, modifiers);
        let preview = tool.live_shape();
        let CreateOutcome::Created(id) = tool.pointer_up(document, to, modifiers) else {
            panic!("expected Created");
        };
        (preview, document.primitive(id).expect("exists").shape)
    }

    fn frame_of(shape: Shape) -> StarFrame {
        let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = shape else {
            panic!("a polygon or star");
        };
        frame
    }

    /// Criterion 3: without a modifier the angle is the raw direction from A
    /// to B (right 0, down 90, up -90), as a vertex at B.
    #[test]
    fn a_plain_drag_keeps_the_raw_angle() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        for (to, degrees) in [
            (Point::new(110.0, 50.0), 0.0),
            (Point::new(100.0, 60.0), 90.0),
            (Point::new(100.0, 40.0), -90.0),
            (Point::new(110.0, 48.0), -11.309_932_474),
        ] {
            let (preview, shape) = dragged(
                &mut tool,
                &document,
                Point::new(100.0, 50.0),
                to,
                Modifiers::NONE,
            );
            assert_eq!(preview.expect("a preview").shape, shape);
            let frame = frame_of(shape);
            assert!(
                (frame.angle.as_radians().to_degrees() - degrees).abs() < 1e-6,
                "{to:?}: {}",
                frame.angle.as_radians().to_degrees()
            );
        }
    }

    /// Criterion 4: Ctrl replaces the angle by the nearest stop of the snap
    /// table and keeps the radius |AB|. A = (100, 50), B = (110, 48): the raw
    /// angle is -11.3 degrees, the stop -15, the first vertex (109.85, 47.36)
    /// to 0.01 mm, the radius 10.20 mm.
    #[test]
    fn ctrl_snaps_the_created_angle_and_keeps_the_radius() {
        let document = Document::new(1);
        for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
            let mut tool = PolygonStarTool::new();
            tool.set_mode(mode);
            let (preview, shape) = dragged(
                &mut tool,
                &document,
                Point::new(100.0, 50.0),
                Point::new(110.0, 48.0),
                Modifiers::new(false, true),
            );
            assert_eq!(preview.expect("a preview").shape, shape);
            let frame = frame_of(shape);
            assert!((frame.angle.as_radians().to_degrees() + 15.0).abs() < 1e-9);
            assert!((frame.radius.as_mm() - 10.198).abs() < 0.005);
            let vertex = (
                frame.center.x + frame.radius.as_mm() * frame.angle.as_radians().cos(),
                frame.center.y + frame.radius.as_mm() * frame.angle.as_radians().sin(),
            );
            assert!((vertex.0 - 109.85).abs() < 0.01, "{vertex:?}");
            assert!((vertex.1 - 47.36).abs() < 0.01, "{vertex:?}");
        }
    }

    /// Criterion 4: the table is the one of the rotate snap, in every
    /// quadrant, positive and negative; a drag along an axis stays on it.
    #[test]
    fn ctrl_uses_the_rotate_snap_table_in_every_quadrant() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        for (raw, stop) in [
            (10.0, 15.0),
            (19.0, 22.5),
            (-26.5, -30.0),
            (100.0, 105.0),
            (-100.0, -105.0),
            (170.0, 165.0),
            (-179.0, 180.0),
            (0.0, 0.0),
        ] {
            let to = Point::new(
                50.0 + 20.0 * f64::to_radians(raw).cos(),
                50.0 + 20.0 * f64::to_radians(raw).sin(),
            );
            let (_, shape) = dragged(
                &mut tool,
                &document,
                Point::new(50.0, 50.0),
                to,
                Modifiers::new(false, true),
            );
            let angle = frame_of(shape).angle.as_radians().to_degrees();
            // A half turn is 180 or -180: the same direction.
            let same = (angle - stop).abs() < 1e-6
                || ((angle.abs() - 180.0).abs() < 1e-6 && (stop.abs() - 180.0).abs() < 1e-6);
            assert!(same, "raw {raw}: got {angle}, want {stop}");
        }
    }

    /// Criterion 5: the shape committed on release is built from the
    /// pointer position and the Ctrl state of the release event, not from
    /// the last preview; the preview follows the Ctrl state with the pointer
    /// held still. Shift has no effect on a polygon or star.
    #[test]
    fn the_release_event_decides_and_shift_does_nothing() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let (a, b) = (Point::new(100.0, 50.0), Point::new(110.0, 48.0));
        tool.pointer_down(a);
        tool.pointer_move(b, Modifiers::NONE);
        let free = tool.live_shape().expect("preview").shape;
        tool.pointer_move(b, Modifiers::new(false, true));
        let snapped = tool.live_shape().expect("preview").shape;
        assert_ne!(free, snapped);
        tool.pointer_move(b, Modifiers::new(true, false));
        assert_eq!(tool.live_shape().expect("preview").shape, free);
        // The last preview was snapped; the release has no Ctrl: free.
        tool.pointer_move(b, Modifiers::new(false, true));
        let CreateOutcome::Created(id) = tool.pointer_up(&document, b, Modifiers::new(true, false))
        else {
            panic!("expected Created");
        };
        assert_eq!(document.primitive(id).expect("exists").shape, free);
        // And the other way round: preview free, release with Ctrl.
        tool.pointer_down(a);
        tool.pointer_move(b, Modifiers::NONE);
        let CreateOutcome::Created(id) = tool.pointer_up(&document, b, Modifiers::new(false, true))
        else {
            panic!("expected Created");
        };
        assert_eq!(document.primitive(id).expect("exists").shape, snapped);
    }

    /// A = B still creates nothing under Ctrl, and Escape writes nothing.
    #[test]
    fn a_degenerate_ctrl_drag_creates_nothing_and_escape_cancels() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        tool.pointer_down(Point::new(5.0, 5.0));
        assert_eq!(
            tool.pointer_up(&document, Point::new(5.0, 5.0), Modifiers::new(false, true)),
            CreateOutcome::NoOp
        );
        tool.pointer_down(Point::new(5.0, 5.0));
        tool.pointer_move(Point::new(9.0, 5.0), Modifiers::new(false, true));
        assert!(tool.escape());
        assert_eq!(tool.live_shape(), None);
        assert_eq!(document.object_ids().len(), 0, "nothing was written");
    }
}
