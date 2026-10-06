//! The polygon/star tool's state machine (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 10-15) — see
//! [`crate::rectangle_tool`]'s own doc comment for the pattern this
//! follows, including [`crate::LiveShape`]'s live-preview plumbing
//! (ux-engineer review).

use vecmanf_document_core::{
    Angle, Document, InnerRatio, NodeId, Point, PointCount, PrimitiveSnapshot, Shape, StarFrame,
};

use crate::ObjectSelection;
use crate::handle_layout::{
    self, HandleKind, ResizeDirection, inner_ratio_from_drag, local_delta, scale_star_frame,
};
use crate::shape_hit_test::{hit_test_handle, hit_test_primitive};
use crate::shape_tool_common::{
    LiveShape, ShapeHitTolerances, apply_selection_click, is_degenerate, polygons_and_stars_only,
};

/// Polygon or star mode (acceptance criteria 11 vs. 12) — fixed before a
/// drag starts; an existing shape's mode never changes ("Out of
/// scope").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyStarMode {
    /// Acceptance criterion 11: a regular N-sided polygon.
    Polygon,
    /// Acceptance criterion 12: an N-pointed star.
    Star,
}

#[derive(Debug, Default)]
enum PolyStarDrag {
    #[default]
    None,
    Creating {
        center: Point,
        current: Point,
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        current: Point,
        start_frame: StarFrame,
        /// The object's own point count, captured at drag-start, so the
        /// live preview shows the real shape (not the tool's current
        /// ephemeral tool-options value, which can differ from what
        /// this particular object was actually created/last set with).
        point_count: PointCount,
        /// `None` for a polygon, `Some` for a star — same reasoning as
        /// `point_count`.
        inner_ratio: Option<InnerRatio>,
        /// The primitive's own rotation (acceptance criterion 25).
        rotation: Angle,
    },
    DraggingInnerRadius {
        id: NodeId,
        down_at: Point,
        current: Point,
        rotation: Angle,
        start_ratio: InnerRatio,
        frame: StarFrame,
        point_count: PointCount,
    },
}

/// What [`PolygonStarTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyStarPointerDownOutcome {
    /// Nothing was hit; a create-drag began.
    Creating,
    /// An existing polygon or star was hit and (now) selected.
    Selected,
    /// A resize or inner-radius handle was hit; a drag began.
    Handle,
}

/// What [`PolygonStarTool::pointer_up`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyStarPointerUpOutcome {
    /// No drag was in flight, or it moved nowhere.
    NoOp,
    /// A new polygon or star was created and committed.
    Created(NodeId),
    /// An existing shape was scaled uniformly (acceptance criterion 13).
    Resized,
    /// A star's inner/outer ratio changed (acceptance criterion 14).
    InnerRatioChanged,
}

/// The polygon/star tool's state (acceptance criteria 10-15). Unlike
/// the other two tools, this one carries persistent, ephemeral
/// tool-options state — `mode`/`point_count`/`ratio` — that survives
/// across created shapes within the session (acceptance criterion 10:
/// "the control is not reset between shapes").
#[derive(Debug)]
pub struct PolygonStarTool {
    drag: PolyStarDrag,
    mode: PolyStarMode,
    point_count: PointCount,
    ratio: InnerRatio,
    /// The ratio slider's in-progress, uncommitted value — see
    /// [`PolygonStarTool::preview_ratio`].
    ratio_preview: Option<InnerRatio>,
}

impl Default for PolygonStarTool {
    fn default() -> Self {
        Self::new()
    }
}

impl PolygonStarTool {
    /// A tool with no shape in progress, defaulting to polygon mode, 6
    /// points and a 0.5 ratio — no acceptance criterion pins these
    /// starting values, only that they persist once changed.
    ///
    /// # Panics
    /// Does not panic in practice: `6` and `0.5` are both comfortably
    /// inside `PointCount`'s and `InnerRatio`'s valid ranges.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag: PolyStarDrag::None,
            mode: PolyStarMode::Polygon,
            // invariant: 6 is within `PointCount`'s `3..=1024` range.
            #[allow(clippy::unwrap_used)]
            point_count: PointCount::new(6).unwrap(),
            // invariant: 0.5 is within `InnerRatio`'s open `(0, 1)` range.
            #[allow(clippy::unwrap_used)]
            ratio: InnerRatio::new(0.5).unwrap(),
            ratio_preview: None,
        }
    }

    /// The current mode (acceptance criteria 11 vs. 12).
    #[must_use]
    pub const fn mode(&self) -> PolyStarMode {
        self.mode
    }

    /// The current point-count control value (acceptance criterion 10).
    #[must_use]
    pub const fn point_count(&self) -> PointCount {
        self.point_count
    }

    /// The current ratio control value (acceptance criterion 12).
    #[must_use]
    pub const fn ratio(&self) -> InnerRatio {
        self.ratio
    }

    /// The mode toggle (acceptance criteria 11 vs. 12): only affects
    /// shapes drawn after the switch.
    pub fn set_mode(&mut self, mode: PolyStarMode) {
        self.mode = mode;
    }

    /// The point-count stepper (acceptance criterion 10, persists;
    /// acceptance criterion 15, live on the current selection): one
    /// commit for every currently selected polygon/star together
    /// (architect review: previously one commit per selected id).
    /// Non-polygon/star ids in the selection are filtered out first, so
    /// a mixed selection updates only the shapes this applies to.
    pub fn set_point_count(
        &mut self,
        count: PointCount,
        document: &Document,
        selection: &ObjectSelection,
    ) {
        self.point_count = count;
        let ids = polygon_or_star_ids(document, selection.ids());
        if !ids.is_empty() {
            let _ = document.set_point_count(&ids, count);
        }
    }

    /// The ratio field's instantaneous commit (acceptance criteria 12,
    /// persists; acceptance criterion 14, live): one commit for every
    /// currently selected *star* together (a polygon has no ratio to
    /// set, so it is filtered out first rather than relying on
    /// `document.set_inner_ratio`'s per-shape refusal). Use this for a
    /// control that changes in one discrete step (e.g. a numeric field
    /// committed on Enter/blur); for a continuously-dragged slider, use
    /// [`PolygonStarTool::preview_ratio`] on every tick and
    /// [`PolygonStarTool::commit_ratio_preview`] once the drag ends
    /// instead (architect review: a slider firing one commit per tick
    /// is the same class of bug ADR 0002 §9 rules out for a canvas
    /// drag).
    pub fn set_ratio(
        &mut self,
        ratio: InnerRatio,
        document: &Document,
        selection: &ObjectSelection,
    ) {
        self.ratio = ratio;
        self.ratio_preview = None;
        let ids = star_ids(document, selection.ids());
        if !ids.is_empty() {
            let _ = document.set_inner_ratio(&ids, ratio);
        }
    }

    /// The ratio slider's live, uncommitted preview (acceptance
    /// criterion 14's "updates live"): updates
    /// [`PolygonStarTool::ratio`] and [`PolygonStarTool::ratio_preview`]
    /// so the host can re-render the selected star(s) with this value,
    /// but writes nothing to the document yet — call
    /// [`PolygonStarTool::commit_ratio_preview`] once, when the slider
    /// drag ends.
    pub fn preview_ratio(&mut self, ratio: InnerRatio) {
        self.ratio = ratio;
        self.ratio_preview = Some(ratio);
    }

    /// The ratio slider's current live preview, if a drag on it is in
    /// flight — `None` once committed or if the slider was never
    /// touched. The host (`vecmanf-editor-wasm`) uses this to override
    /// the selected star(s)' rendered ratio for the one frame being
    /// drawn, without touching the document.
    #[must_use]
    pub const fn ratio_preview(&self) -> Option<InnerRatio> {
        self.ratio_preview
    }

    /// Commits whatever [`PolygonStarTool::preview_ratio`] has
    /// accumulated, as one commit for the whole selection, the same way
    /// [`PolygonStarTool::set_ratio`] does — a no-op if the slider was
    /// never touched.
    pub fn commit_ratio_preview(&mut self, document: &Document, selection: &ObjectSelection) {
        let Some(ratio) = self.ratio_preview.take() else {
            return;
        };
        let ids = star_ids(document, selection.ids());
        if !ids.is_empty() {
            let _ = document.set_inner_ratio(&ids, ratio);
        }
    }

    /// Acceptance criteria 11, 12 (create-drag start), 13, 14 (selecting
    /// an existing shape, or starting a resize/inner-radius handle drag
    /// on it).
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerances: ShapeHitTolerances,
        shift: bool,
    ) -> PolyStarPointerDownOutcome {
        let shapes = polygons_and_stars_only(primitives);

        if let [only] = selection.ids()
            && let Some(snapshot) = shapes.iter().find(|p| p.id == *only)
        {
            let (frame, point_count, inner_ratio) = match snapshot.shape {
                Shape::Polygon { frame, point_count } => (frame, point_count, None),
                Shape::Star {
                    frame,
                    point_count,
                    inner_ratio,
                } => (frame, point_count, Some(inner_ratio)),
                Shape::Rect { .. } | Shape::Ellipse { .. } => {
                    // invariant: `shapes` was filtered to polygons and
                    // stars only, just above.
                    unreachable!("filtered to polygon/star above")
                }
            };
            let handles = handle_layout::handles_for(snapshot);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                self.drag = match (handles[index].kind, inner_ratio) {
                    (HandleKind::Resize(direction), _) => PolyStarDrag::Resizing {
                        id: snapshot.id,
                        direction,
                        down_at: point,
                        current: point,
                        start_frame: frame,
                        point_count,
                        inner_ratio,
                        rotation: snapshot.rotation,
                    },
                    (HandleKind::InnerRadius, Some(start_ratio)) => {
                        PolyStarDrag::DraggingInnerRadius {
                            id: snapshot.id,
                            down_at: point,
                            current: point,
                            rotation: snapshot.rotation,
                            start_ratio,
                            frame,
                            point_count,
                        }
                    }
                    _ => {
                        // invariant: `hit_test_handle` only returns a
                        // `draggable` handle, and `InnerRadius` is only
                        // ever shown (hence only ever draggable) when
                        // `inner_ratio` is `Some` (a star).
                        unreachable!("not shown/not draggable; filtered by hit_test_handle")
                    }
                };
                return PolyStarPointerDownOutcome::Handle;
            }
        }

        let hit = hit_test_primitive(&shapes, point, tolerances.outline);
        if let Some(id) = hit {
            apply_selection_click(selection, Some(id), shift);
            return PolyStarPointerDownOutcome::Selected;
        }
        apply_selection_click(selection, None, shift);
        self.drag = PolyStarDrag::Creating {
            center: point,
            current: point,
        };
        PolyStarPointerDownOutcome::Creating
    }

    /// The pointer moved to `point` with the drag still in flight —
    /// updates [`PolygonStarTool::live_shape`]'s return value, writes
    /// nothing to the document. A no-op when no drag is in progress.
    pub fn pointer_move(&mut self, point: Point) {
        match &mut self.drag {
            PolyStarDrag::Creating { current, .. }
            | PolyStarDrag::Resizing { current, .. }
            | PolyStarDrag::DraggingInnerRadius { current, .. } => *current = point,
            PolyStarDrag::None => {}
        }
    }

    /// The live, uncommitted preview of whatever drag is in flight —
    /// `None` when idle or a create-drag is still degenerate
    /// (acceptance criteria 11, 12).
    #[must_use]
    pub fn live_shape(&self) -> Option<LiveShape> {
        match self.drag {
            PolyStarDrag::None => None,
            PolyStarDrag::Creating { center, current } => {
                if is_degenerate(center, current) {
                    return None;
                }
                let frame = StarFrame::from_center_and_vertex(center, current);
                let shape = match self.mode {
                    PolyStarMode::Polygon => Shape::Polygon {
                        frame,
                        point_count: self.point_count,
                    },
                    PolyStarMode::Star => Shape::Star {
                        frame,
                        point_count: self.point_count,
                        inner_ratio: self.ratio,
                    },
                };
                Some(LiveShape::Creating(shape, current))
            }
            PolyStarDrag::Resizing {
                direction,
                down_at,
                current,
                start_frame,
                point_count,
                inner_ratio,
                rotation,
                ..
            } => {
                let delta = local_delta(rotation, down_at, current);
                let frame = scale_star_frame(start_frame, direction, delta);
                let shape = match inner_ratio {
                    None => Shape::Polygon { frame, point_count },
                    Some(ratio) => Shape::Star {
                        frame,
                        point_count,
                        inner_ratio: ratio,
                    },
                };
                Some(LiveShape::Adjusting(shape))
            }
            PolyStarDrag::DraggingInnerRadius {
                down_at,
                current,
                start_ratio,
                frame,
                point_count,
                rotation,
                ..
            } => {
                let delta = local_delta(rotation, down_at, current);
                let ratio = inner_ratio_from_drag(frame, point_count, start_ratio, delta);
                Some(LiveShape::Adjusting(Shape::Star {
                    frame,
                    point_count,
                    inner_ratio: ratio,
                }))
            }
        }
    }

    /// Acceptance criteria 11, 12, 13, 14.
    pub fn pointer_up(&mut self, document: &Document, point: Point) -> PolyStarPointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            PolyStarDrag::None => PolyStarPointerUpOutcome::NoOp,
            PolyStarDrag::Creating { center, .. } => {
                if is_degenerate(center, point) {
                    return PolyStarPointerUpOutcome::NoOp;
                }
                let frame = StarFrame::from_center_and_vertex(center, point);
                let id = match self.mode {
                    PolyStarMode::Polygon => document.create_polygon(frame, self.point_count),
                    PolyStarMode::Star => document.create_star(frame, self.point_count, self.ratio),
                };
                PolyStarPointerUpOutcome::Created(id)
            }
            PolyStarDrag::Resizing {
                id,
                direction,
                down_at,
                start_frame,
                rotation,
                ..
            } => {
                if point == down_at {
                    return PolyStarPointerUpOutcome::NoOp;
                }
                let delta = local_delta(rotation, down_at, point);
                let frame = scale_star_frame(start_frame, direction, delta);
                let _ = document.set_star_frame(id, frame);
                PolyStarPointerUpOutcome::Resized
            }
            PolyStarDrag::DraggingInnerRadius {
                id,
                down_at,
                start_ratio,
                frame,
                point_count,
                rotation,
                ..
            } => {
                if point == down_at {
                    return PolyStarPointerUpOutcome::NoOp;
                }
                let delta = local_delta(rotation, down_at, point);
                let ratio = inner_ratio_from_drag(frame, point_count, start_ratio, delta);
                self.ratio = ratio;
                let _ = document.set_inner_ratio(&[id], ratio);
                PolyStarPointerUpOutcome::InnerRatioChanged
            }
        }
    }

    /// Discards any in-progress drag, writing nothing.
    pub fn escape(&mut self) -> bool {
        let had_drag = !matches!(self.drag, PolyStarDrag::None);
        self.drag = PolyStarDrag::None;
        had_drag
    }
}

/// Filters `ids` down to the ones currently naming a polygon or a star
/// — `document.set_point_count`'s batch refuses the whole call if any
/// id in it is neither, so the caller pre-filters.
fn polygon_or_star_ids(document: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter()
        .copied()
        .filter(|&id| {
            matches!(
                document.primitive(id).map(|p| p.shape),
                Some(Shape::Polygon { .. } | Shape::Star { .. })
            )
        })
        .collect()
}

/// Same, but stars only — `document.set_inner_ratio`'s own batch.
fn star_ids(document: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter()
        .copied()
        .filter(|&id| {
            matches!(
                document.primitive(id).map(|p| p.shape),
                Some(Shape::Star { .. })
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Angle, Document, Length, Tolerance, Vec2};

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

    /// AC10: the point-count control persists across shapes and is
    /// capped to `3..=1024` by [`PointCount`] itself.
    #[test]
    fn ac10_point_count_persists_between_shapes() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        tool.set_point_count(PointCount::new(8).unwrap(), &document, &selection);

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(10.0, 0.0));
        let PolyStarPointerUpOutcome::Created(first) = outcome else {
            panic!("expected Created");
        };
        tool.pointer_down(
            &[],
            &mut selection,
            Point::new(50.0, 0.0),
            TOLERANCES,
            false,
        );
        let outcome = tool.pointer_up(&document, Point::new(60.0, 0.0));
        let PolyStarPointerUpOutcome::Created(second) = outcome else {
            panic!("expected Created");
        };

        for id in [first, second] {
            let snapshot = document.primitive(id).expect("exists");
            let Shape::Polygon { point_count, .. } = snapshot.shape else {
                panic!("expected polygon");
            };
            assert_eq!(point_count.get(), 8, "control was not reset between shapes");
        }
    }

    /// AC11: a polygon drag places N points evenly around A, one at B,
    /// and the live preview shows exactly that before release.
    #[test]
    fn ac11_polygon_drag_places_a_vertex_at_b() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_move(Point::new(10.0, 0.0));
        let Some(LiveShape::Creating(
            Shape::Polygon {
                frame: live_frame, ..
            },
            anchor,
        )) = tool.live_shape()
        else {
            panic!("expected a Creating polygon preview");
        };
        assert!((live_frame.radius.as_mm() - 10.0).abs() < 1e-9);
        assert_eq!(anchor, Point::new(10.0, 0.0));

        let outcome = tool.pointer_up(&document, Point::new(10.0, 0.0));
        let PolyStarPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Polygon { frame, .. } = snapshot.shape else {
            panic!("expected polygon");
        };
        assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
        assert!(frame.angle.as_radians().abs() < 1e-9);
    }

    /// AC12: a star drag places N outer/N inner vertices, inner =
    /// `ratio * outer`.
    #[test]
    fn ac12_star_drag_sets_outer_and_inner_radius() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        tool.set_mode(PolyStarMode::Star);
        tool.set_ratio(
            InnerRatio::new(0.3).unwrap(),
            &document,
            &ObjectSelection::new(),
        );
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(10.0, 0.0));
        let PolyStarPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Star {
            frame, inner_ratio, ..
        } = snapshot.shape
        else {
            panic!("expected star");
        };
        assert!((frame.radius.as_mm() - 10.0).abs() < 1e-9);
        assert!((inner_ratio.get() - 0.3).abs() < 1e-9);
    }

    /// AC13: dragging a cardinal resize handle scales a star uniformly
    /// — outer radius changes, inner ratio (and so inner radius) stays
    /// proportional, point count and orientation unchanged. Live
    /// preview matches along the way.
    #[test]
    fn ac13_polygon_star_uniform_scale_keeps_ratio_and_count() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.4).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0), // the E handle
            TOLERANCES,
            false,
        );
        tool.pointer_move(Point::new(20.0, 0.0));
        let Some(LiveShape::Adjusting(Shape::Star {
            frame: live_frame,
            point_count: live_count,
            inner_ratio: live_ratio,
        })) = tool.live_shape()
        else {
            panic!("expected an Adjusting star preview");
        };
        assert!((live_frame.radius.as_mm() - 20.0).abs() < 1e-9);
        assert_eq!(live_count.get(), 5);
        assert!((live_ratio.get() - 0.4).abs() < 1e-9);

        tool.pointer_up(&document, Point::new(20.0, 0.0));

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Star {
            frame: new_frame,
            point_count,
            inner_ratio,
        } = snapshot.shape
        else {
            panic!("expected star");
        };
        assert!((new_frame.radius.as_mm() - 20.0).abs() < 1e-9);
        assert_eq!(point_count.get(), 5);
        assert!((inner_ratio.get() - 0.4).abs() < 1e-9);
    }

    /// `object-transform` acceptance criterion 25: a rotated star's
    /// inner-radius handle hit-tests at the rotated position, and
    /// pulling it 1 mm further out along its own (rotated) direction
    /// raises the ratio by exactly 0.1 of the 10 mm outer radius.
    #[test]
    fn ac25_a_rotated_stars_inner_radius_handle_follows_and_drags_in_local_axes() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let rotation = Angle::from_radians(0.9);
        document
            .rotate_object(id, Point::new(0.0, 0.0), rotation)
            .expect("rotate");
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let local = handle_layout::inner_radius_handle_position(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let rotated = local.rotated_around(frame.center, rotation);
        let outcome = tool.pointer_down(
            &snapshots(&document),
            &mut selection,
            rotated,
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, PolyStarPointerDownOutcome::Handle);
        let outward = frame.center.vector_to(rotated).normalized_to(1.0);
        tool.pointer_up(&document, rotated.translated(outward));
        let Shape::Star { inner_ratio, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected star");
        };
        assert!(
            (inner_ratio.get() - 0.6).abs() < 1e-9,
            "got {}",
            inner_ratio.get()
        );
    }

    /// AC14: dragging the inner-radius handle changes only the ratio; a
    /// plain polygon never shows that handle at all.
    #[test]
    fn ac14_inner_radius_drag_changes_only_ratio_star_only() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let star_id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(star_id);

        let handle_position = handle_layout::inner_radius_handle_position(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            handle_position,
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, PolyStarPointerDownOutcome::Handle);
        let up_outcome =
            tool.pointer_up(&document, handle_position.translated(Vec2::new(0.5, 0.0)));
        assert_eq!(up_outcome, PolyStarPointerUpOutcome::InnerRatioChanged);

        let snapshot = document.primitive(star_id).expect("exists");
        let Shape::Star {
            frame: unchanged_frame,
            inner_ratio,
            ..
        } = snapshot.shape
        else {
            panic!("expected star");
        };
        assert_eq!(unchanged_frame, frame, "outer radius untouched");
        assert!(inner_ratio.get() > 0.5, "ratio increased");

        // A plain polygon shows no inner-radius handle at all.
        let polygon_id = document.create_polygon(frame, PointCount::new(5).unwrap());
        let mut polygon_selection = ObjectSelection::new();
        polygon_selection.select_single(polygon_id);
        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut polygon_selection,
            handle_position,
            TOLERANCES,
            false,
        );
        assert_ne!(outcome, PolyStarPointerDownOutcome::Handle);
    }

    /// AC15: changing the point-count control on a selected shape
    /// updates it live, keeping size/ratio/orientation.
    #[test]
    fn ac15_point_count_change_on_selection_is_live() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(1.0, 2.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.2),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        tool.set_point_count(PointCount::new(9).unwrap(), &document, &selection);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Star {
            frame: unchanged_frame,
            point_count,
            inner_ratio,
        } = snapshot.shape
        else {
            panic!("expected star");
        };
        assert_eq!(point_count.get(), 9);
        assert_eq!(unchanged_frame, frame);
        assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
    }

    /// Architect review: a point-count/ratio change on a multi-shape
    /// selection is one commit, not one per object.
    #[test]
    fn set_point_count_on_a_multi_selection_is_one_commit_and_skips_other_kinds() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let polygon_id = document.create_polygon(frame, PointCount::new(5).unwrap());
        let star_id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let rect_id = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
        ));

        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(polygon_id);
        selection.toggle(star_id);
        selection.toggle(rect_id);

        tool.set_point_count(PointCount::new(9).unwrap(), &document, &selection);

        let Shape::Polygon { point_count, .. } = document.primitive(polygon_id).unwrap().shape
        else {
            panic!("expected polygon");
        };
        assert_eq!(point_count.get(), 9);
        let Shape::Star { point_count, .. } = document.primitive(star_id).unwrap().shape else {
            panic!("expected star");
        };
        assert_eq!(point_count.get(), 9);
        // The rect in the mixed selection is untouched (filtered out
        // before the batch call, rather than being refused as a whole).
        assert!(matches!(
            document.primitive(rect_id).unwrap().shape,
            Shape::Rect { .. }
        ));
    }

    /// Escape mid-drag cancels it, writing nothing.
    #[test]
    fn escape_mid_drag_writes_nothing() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(tool.escape());
        assert!(tool.live_shape().is_none());
        let outcome = tool.pointer_up(&document, Point::new(50.0, 50.0));
        assert_eq!(outcome, PolyStarPointerUpOutcome::NoOp);
        assert_eq!(document.object_ids(), Vec::new());
    }

    /// Architect review: the ratio slider's preview writes nothing to
    /// the document on every tick — only `commit_ratio_preview` does,
    /// once, as one commit for the whole selection.
    #[test]
    fn preview_ratio_writes_nothing_until_committed() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        for tick in [0.3, 0.4, 0.6, 0.7] {
            tool.preview_ratio(InnerRatio::new(tick).unwrap());
            let Shape::Star { inner_ratio, .. } = document.primitive(id).unwrap().shape else {
                panic!("expected star");
            };
            assert!(
                (inner_ratio.get() - 0.5).abs() < 1e-9,
                "no tick writes to the document"
            );
        }
        assert!(
            (tool.ratio().get() - 0.7).abs() < 1e-9,
            "the tool's own display tracks the preview"
        );

        tool.commit_ratio_preview(&document, &selection);
        let Shape::Star { inner_ratio, .. } = document.primitive(id).unwrap().shape else {
            panic!("expected star");
        };
        assert!(
            (inner_ratio.get() - 0.7).abs() < 1e-9,
            "committed exactly once"
        );
        assert!(tool.ratio_preview().is_none(), "cleared after commit");
    }
}
