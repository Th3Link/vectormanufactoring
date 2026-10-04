//! The three primitive-shape tool state machines
//! (`specs/primitive-shapes/specification.md`, acceptance criteria
//! 1-15): [`RectangleTool`], [`EllipseTool`] and [`PolygonStarTool`].
//! Each follows [`crate::PenTool`]/[`crate::NodeTool`]'s established
//! pattern — ephemeral in-progress drag state (ADR 0009 §2), one
//! [`Document`] commit on release, a zero-movement create writes
//! nothing — extended with [`crate::PrimitiveSelection`] (shared across
//! all three) and [`crate::handle_layout`]'s handle arithmetic.

use vecmanf_document_core::{
    Document, EllipseFrame, InnerRatio, Length, NodeId, Point, PointCount, PrimitiveSnapshot,
    RectBounds, Shape, StarFrame, Tolerance,
};

use crate::PrimitiveSelection;
use crate::handle_layout::{
    self, HandleKind, ResizeDirection, corner_radius_from_drag, inner_ratio_from_drag,
    resize_ellipse_frame, resize_rect_bounds, scale_star_frame,
};
use crate::shape_hit_test::{hit_test_handle, hit_test_primitive};

/// The two hit-test tolerances every shape tool needs — mirrors
/// [`crate::HitTolerances`]'s split for the node tool. `outline` bounds
/// a selection click against a primitive's own outline; `handle` bounds
/// a click against one of its shape handles.
#[derive(Debug, Clone, Copy)]
pub struct ShapeHitTolerances {
    /// Bounds a selection hit against a primitive's outline.
    pub outline: Tolerance,
    /// Bounds a hit against a shape handle.
    pub handle: Tolerance,
}

/// Point A equals point B (acceptance criteria 1, 7, 11, 12: a drag
/// with no movement creates nothing).
fn is_degenerate(a: Point, b: Point) -> bool {
    a == b
}

/// The Ctrl-constrain square/circle endpoint (acceptance criteria 2, 8):
/// `b`, moved so both axes have the same extent from `a` — the larger
/// of the drag's own horizontal/vertical extents, keeping each axis's
/// original sign (or defaulting positive if that axis had no movement
/// at all).
fn constrained_endpoint(a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let extent = dx.abs().max(dy.abs());
    let sign = |d: f64| if d == 0.0 { 1.0 } else { d.signum() };
    Point::new(a.x + extent * sign(dx), a.y + extent * sign(dy))
}

fn rects_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Rect { .. }))
        .collect()
}

fn ellipses_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Ellipse { .. }))
        .collect()
}

fn polygons_and_stars_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }))
        .collect()
}

/// Selects (or shift-toggles) `hit`, matching the node tool's own
/// selecting-click convention. A plain click that hits nothing clears
/// the selection unless shift is held.
fn apply_selection_click(selection: &mut PrimitiveSelection, hit: Option<NodeId>, shift: bool) {
    match hit {
        Some(id) if shift => selection.toggle(id),
        Some(id) => selection.select_single(id),
        None if !shift => selection.clear(),
        None => {}
    }
}

// ---------------------------------------------------------------------
// Rectangle tool (acceptance criteria 1-6)
// ---------------------------------------------------------------------

#[derive(Debug, Default)]
enum RectDrag {
    #[default]
    None,
    Creating {
        down_at: Point,
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        start_bounds: RectBounds,
    },
    DraggingRadius {
        id: NodeId,
        down_at: Point,
        start_radius: Length,
        bounds: RectBounds,
    },
}

/// What [`RectangleTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectPointerDownOutcome {
    /// Nothing was hit; a create-drag began.
    Creating,
    /// An existing rectangle was hit and (now) selected.
    Selected,
    /// A shape handle was hit; a resize or radius drag began.
    Handle,
}

/// What [`RectangleTool::pointer_up`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectPointerUpOutcome {
    /// No drag was in flight, or it moved nowhere (acceptance criterion
    /// 1's "A = B creates nothing").
    NoOp,
    /// A new rectangle was created and committed.
    Created(NodeId),
    /// An existing rectangle was resized (acceptance criterion 3).
    Resized,
    /// The corner radius changed (acceptance criteria 4, 5, 6).
    RadiusChanged,
}

/// The rectangle tool's state (acceptance criteria 1-6). Selection is
/// shared ([`PrimitiveSelection`]), passed in rather than owned, so the
/// ellipse and polygon/star tools can accumulate a mixed-kind selection
/// alongside it (acceptance criterion 22).
#[derive(Debug, Default)]
pub struct RectangleTool {
    drag: RectDrag,
}

impl RectangleTool {
    /// A tool with nothing selected and no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 1, 2 (create-drag start), 3-6 (selecting an
    /// existing rectangle, or starting a handle drag on it).
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut PrimitiveSelection,
        point: Point,
        tolerances: ShapeHitTolerances,
        shift: bool,
    ) -> RectPointerDownOutcome {
        let rects = rects_only(primitives);

        if let [only] = selection.ids()
            && let Some(snapshot) = rects.iter().find(|p| p.id == *only)
            && let Shape::Rect {
                bounds,
                corner_radius,
            } = snapshot.shape
        {
            let handles = handle_layout::rect_handles(bounds, corner_radius);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                let handle = handles[index];
                self.drag = match handle.kind {
                    HandleKind::CornerRadius => RectDrag::DraggingRadius {
                        id: snapshot.id,
                        down_at: point,
                        start_radius: corner_radius,
                        bounds,
                    },
                    HandleKind::Resize(direction) => RectDrag::Resizing {
                        id: snapshot.id,
                        direction,
                        down_at: point,
                        start_bounds: bounds,
                    },
                    HandleKind::CornerRadiusEcho | HandleKind::InnerRadius => {
                        unreachable!("not draggable; filtered by hit_test_handle")
                    }
                };
                return RectPointerDownOutcome::Handle;
            }
        }

        let hit = hit_test_primitive(&rects, point, tolerances.outline);
        if let Some(id) = hit {
            apply_selection_click(selection, Some(id), shift);
            return RectPointerDownOutcome::Selected;
        }
        apply_selection_click(selection, None, shift);
        self.drag = RectDrag::Creating { down_at: point };
        RectPointerDownOutcome::Creating
    }

    /// Acceptance criteria 1, 2, 3, 4, 5, 6: commits whatever gesture
    /// [`RectangleTool::pointer_down`] began. `constrain` is the Ctrl
    /// modifier's state at release (acceptance criterion 2).
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        constrain: bool,
    ) -> RectPointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            RectDrag::None => RectPointerUpOutcome::NoOp,
            RectDrag::Creating { down_at } => {
                let end = if constrain {
                    constrained_endpoint(down_at, point)
                } else {
                    point
                };
                if is_degenerate(down_at, end) {
                    return RectPointerUpOutcome::NoOp;
                }
                let bounds = RectBounds::from_corners(down_at, end);
                RectPointerUpOutcome::Created(document.create_rect(bounds))
            }
            RectDrag::Resizing {
                id,
                direction,
                down_at,
                start_bounds,
            } => {
                if point == down_at {
                    return RectPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let bounds = resize_rect_bounds(start_bounds, direction, delta);
                let _ = document.set_rect_bounds(id, bounds);
                RectPointerUpOutcome::Resized
            }
            RectDrag::DraggingRadius {
                id,
                down_at,
                start_radius,
                bounds,
            } => {
                if point == down_at {
                    return RectPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let radius = corner_radius_from_drag(start_radius, bounds, delta);
                let _ = document.set_corner_radius(id, radius);
                RectPointerUpOutcome::RadiusChanged
            }
        }
    }

    /// Discards any in-progress drag, writing nothing. Returns whether
    /// there was anything to discard.
    pub fn escape(&mut self) -> bool {
        let had_drag = !matches!(self.drag, RectDrag::None);
        self.drag = RectDrag::None;
        had_drag
    }

    /// Acceptance criterion 6's "remove rounding" action: zeroes the
    /// corner radius of every currently selected rectangle.
    pub fn remove_rounding(&self, document: &Document, selection: &PrimitiveSelection) {
        for &id in selection.ids() {
            let _ = document.set_corner_radius(id, Length::from_mm(0.0));
        }
    }
}

// ---------------------------------------------------------------------
// Ellipse tool (acceptance criteria 7-9)
// ---------------------------------------------------------------------

#[derive(Debug, Default)]
enum EllipseDrag {
    #[default]
    None,
    Creating {
        down_at: Point,
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        start_frame: EllipseFrame,
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
    /// existing ellipse, or starting a resize-handle drag on it).
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut PrimitiveSelection,
        point: Point,
        tolerances: ShapeHitTolerances,
        shift: bool,
    ) -> EllipsePointerDownOutcome {
        let ellipses = ellipses_only(primitives);

        if let [only] = selection.ids()
            && let Some(snapshot) = ellipses.iter().find(|p| p.id == *only)
            && let Shape::Ellipse { frame } = snapshot.shape
        {
            let handles = handle_layout::ellipse_handles(frame);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                let HandleKind::Resize(direction) = handles[index].kind else {
                    unreachable!("an ellipse shows only resize handles")
                };
                self.drag = EllipseDrag::Resizing {
                    id: snapshot.id,
                    direction,
                    down_at: point,
                    start_frame: frame,
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
        self.drag = EllipseDrag::Creating { down_at: point };
        EllipsePointerDownOutcome::Creating
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
            EllipseDrag::Creating { down_at } => {
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
            } => {
                if point == down_at {
                    return EllipsePointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let frame = resize_ellipse_frame(start_frame, direction, delta);
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

// ---------------------------------------------------------------------
// Polygon/star tool (acceptance criteria 10-15)
// ---------------------------------------------------------------------

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
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        start_frame: StarFrame,
    },
    DraggingInnerRadius {
        id: NodeId,
        down_at: Point,
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
    /// acceptance criterion 15, live on the current selection): updates
    /// every currently selected polygon/star in `primitives`, each as
    /// its own commit — a stepper change is one interaction per
    /// selected shape, not a batch (`adrs.md`'s "one committed change to
    /// the point-count or ratio control" wording covers the common
    /// single-selection case; a multi-selection here simply repeats it).
    pub fn set_point_count(
        &mut self,
        count: PointCount,
        document: &Document,
        selection: &PrimitiveSelection,
    ) {
        self.point_count = count;
        for &id in selection.ids() {
            let _ = document.set_point_count(id, count);
        }
    }

    /// The ratio field/slider (acceptance criteria 12, persists;
    /// acceptance criterion 14, live): updates every currently selected
    /// *star* (a polygon has no ratio to set — `document.set_inner_
    /// ratio` already refuses that case, so this simply ignores the
    /// refusal for a selected polygon).
    pub fn set_ratio(
        &mut self,
        ratio: InnerRatio,
        document: &Document,
        selection: &PrimitiveSelection,
    ) {
        self.ratio = ratio;
        for &id in selection.ids() {
            let _ = document.set_inner_ratio(id, ratio);
        }
    }

    /// Acceptance criteria 11, 12 (create-drag start), 13, 14 (selecting
    /// an existing shape, or starting a resize/inner-radius handle drag
    /// on it).
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut PrimitiveSelection,
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
                Shape::Rect { .. } | Shape::Ellipse { .. } => unreachable!("filtered above"),
            };
            let handles = handle_layout::polygon_or_star_handles(frame, point_count, inner_ratio);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                self.drag = match (handles[index].kind, inner_ratio) {
                    (HandleKind::Resize(direction), _) => PolyStarDrag::Resizing {
                        id: snapshot.id,
                        direction,
                        down_at: point,
                        start_frame: frame,
                    },
                    (HandleKind::InnerRadius, Some(start_ratio)) => {
                        PolyStarDrag::DraggingInnerRadius {
                            id: snapshot.id,
                            down_at: point,
                            start_ratio,
                            frame,
                            point_count,
                        }
                    }
                    _ => unreachable!("not shown/not draggable; filtered by hit_test_handle"),
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
        self.drag = PolyStarDrag::Creating { center: point };
        PolyStarPointerDownOutcome::Creating
    }

    /// Acceptance criteria 11, 12, 13, 14.
    pub fn pointer_up(&mut self, document: &Document, point: Point) -> PolyStarPointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            PolyStarDrag::None => PolyStarPointerUpOutcome::NoOp,
            PolyStarDrag::Creating { center } => {
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
            } => {
                if point == down_at {
                    return PolyStarPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
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
            } => {
                if point == down_at {
                    return PolyStarPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let ratio = inner_ratio_from_drag(frame, point_count, start_ratio, delta);
                self.ratio = ratio;
                let _ = document.set_inner_ratio(id, ratio);
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

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, Vec2};

    const TOLERANCES: ShapeHitTolerances = ShapeHitTolerances {
        outline: Tolerance::from_mm(1.0),
        handle: Tolerance::from_mm(2.0),
    };

    fn snapshots(document: &Document) -> Vec<PrimitiveSnapshot> {
        document
            .path_ids()
            .into_iter()
            .filter_map(|id| document.primitive(id))
            .collect()
    }

    /// AC1: a drag from A to B creates a rectangle with A, B as opposite
    /// corners, zero radius, and A == B creates nothing.
    #[test]
    fn ac1_rectangle_drag_creates_a_zero_radius_rect() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(10.0, 20.0), false);
        let RectPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created, got {outcome:?}");
        };
        let snapshot = document.primitive(id).expect("exists");
        assert_eq!(
            snapshot.shape,
            Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(0.0, 0.0),
                    width: Length::from_mm(10.0),
                    height: Length::from_mm(20.0),
                },
                corner_radius: Length::from_mm(0.0),
            }
        );

        // A = B creates nothing.
        tool.pointer_down(&[], &mut selection, Point::new(5.0, 5.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(5.0, 5.0), false);
        assert_eq!(outcome, RectPointerUpOutcome::NoOp);
    }

    /// AC2: Ctrl-constrain sizes the rectangle to the larger extent on
    /// both axes.
    #[test]
    fn ac2_constrain_makes_a_square_sized_to_the_larger_extent() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(5.0, 20.0), true);
        let RectPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 20.0).abs() < 1e-9);
    }

    /// AC3: dragging a resize handle changes width/height, keeps the
    /// radius (absolute length) and the object stays a rectangle.
    #[test]
    fn ac3_resize_handle_drag_changes_bounds_keeps_radius() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .set_corner_radius(id, Length::from_mm(2.0))
            .expect("set radius");
        let mut tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 5.0), // the E resize handle
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, RectPointerDownOutcome::Handle);
        let up_outcome = tool.pointer_up(&document, Point::new(20.0, 5.0), false);
        assert_eq!(up_outcome, RectPointerUpOutcome::Resized);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect {
            bounds,
            corner_radius,
        } = snapshot.shape
        else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((corner_radius.as_mm() - 2.0).abs() < 1e-9);
    }

    /// AC4, AC5: dragging the corner-radius handle out rounds all four
    /// corners, clamped to half the shorter side.
    #[test]
    fn ac4_ac5_corner_radius_handle_drag_rounds_and_clamps() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let mut tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0), // the corner-radius handle, at zero radius
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, RectPointerDownOutcome::Handle);
        // Drag far past half the shorter side (5mm).
        let up_outcome = tool.pointer_up(&document, Point::new(-90.0, 90.0), false);
        assert_eq!(up_outcome, RectPointerUpOutcome::RadiusChanged);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { corner_radius, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!((corner_radius.as_mm() - 5.0).abs() < 1e-6);
    }

    /// AC6: "remove rounding" zeroes the radius.
    #[test]
    fn ac6_remove_rounding_zeroes_the_radius() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .set_corner_radius(id, Length::from_mm(4.0))
            .expect("set radius");
        let tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();
        selection.select_single(id);
        tool.remove_rounding(&document, &selection);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { corner_radius, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!(corner_radius.as_mm().abs() < f64::EPSILON);
    }

    /// AC7, AC8: an ellipse drag creates rx/ry from the bbox, and Ctrl
    /// constrains to a circle.
    #[test]
    fn ac7_ac8_ellipse_drag_and_circle_constrain() {
        let document = Document::new(1);
        let mut tool = EllipseTool::new();
        let mut selection = PrimitiveSelection::new();

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
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
    /// reshaped into a non-circular ellipse).
    #[test]
    fn ac9_ellipse_resize_can_break_circularity() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(10.0),
            ry: Length::from_mm(10.0),
        });
        let mut tool = EllipseTool::new();
        let mut selection = PrimitiveSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0),
            TOLERANCES,
            false,
        );
        tool.pointer_up(&document, Point::new(20.0, 0.0), false);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Ellipse { frame } = snapshot.shape else {
            panic!("expected ellipse");
        };
        assert!((frame.rx.as_mm() - 15.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 10.0).abs() < 1e-9);
    }

    /// AC10: the point-count control persists across shapes and is
    /// capped to `3..=1024` by [`PointCount`] itself.
    #[test]
    fn ac10_point_count_persists_between_shapes() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let mut selection = PrimitiveSelection::new();
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

    /// AC11: a polygon drag places N points evenly around A, one at B.
    #[test]
    fn ac11_polygon_drag_places_a_vertex_at_b() {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        let mut selection = PrimitiveSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
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
            &PrimitiveSelection::new(),
        );
        let mut selection = PrimitiveSelection::new();
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
    /// proportional, point count and orientation unchanged.
    #[test]
    fn ac13_polygon_star_uniform_scale_keeps_ratio_and_count() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.4).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = PrimitiveSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0), // the E handle
            TOLERANCES,
            false,
        );
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

    /// AC14: dragging the inner-radius handle changes only the ratio; a
    /// plain polygon never shows that handle at all.
    #[test]
    fn ac14_inner_radius_drag_changes_only_ratio_star_only() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let star_id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = PrimitiveSelection::new();
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
        let mut polygon_selection = PrimitiveSelection::new();
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
            angle: vecmanf_document_core::Angle::from_radians(0.2),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let mut tool = PolygonStarTool::new();
        let mut selection = PrimitiveSelection::new();
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

    /// Escape mid-drag cancels it, writing nothing.
    #[test]
    fn escape_mid_drag_writes_nothing() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = PrimitiveSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(tool.escape());
        let outcome = tool.pointer_up(&document, Point::new(50.0, 50.0), false);
        assert_eq!(outcome, RectPointerUpOutcome::NoOp);
        assert_eq!(document.path_ids(), Vec::new());
    }
}
