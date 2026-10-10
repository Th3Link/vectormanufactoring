//! The Select tool's two selection gestures (`specs/0014-advanced-selection/
//! specification.md`): the marquee a press on empty canvas arms (criteria
//! 8 to 15) and the lasso a press with Alt held arms (criteria 16 to 20). The
//! kind is chosen once, at the press, and is the variant of `SelectDrag`, so
//! there is no transition between the two. Both resolve on release with the
//! modifiers of that moment.

use curvyo_document_core::{ObjectSnapshot, Point, Tolerance};

use super::cycle::ClickCycle;
use super::{SelectDrag, SelectTool};
use crate::hit_test_object::hit_test_objects_along;
use crate::marquee::{MarqueeMode, objects_in_marquee};
use crate::modifiers::Modifiers;
use crate::object_selection::{ObjectSelection, SelectionCombine};
use crate::transform_drag::DragOrigin;

/// The slack of the marquee's box comparisons, millimetres: an edge exactly
/// on the rectangle counts as inside, and rounding does not decide it.
const BOX_TOLERANCE: Tolerance = Tolerance::from_mm(1e-6);

/// How many points the lasso keeps per dead-zone width: a new point is
/// stored when it is a third of the dead zone (1 px) from the last one.
const LASSO_POINTS_PER_DEAD_ZONE: f64 = 3.0;

/// A marquee in flight: armed by a press on empty canvas with Alt up.
#[derive(Debug, Clone)]
pub(super) struct MarqueeDrag {
    pub(super) origin: DragOrigin,
}

/// A lasso in flight: armed by a press with Alt down, anywhere.
#[derive(Debug, Clone)]
pub(super) struct LassoDrag {
    pub(super) origin: DragOrigin,
    /// The hit tolerance at the press, used by the release.
    tolerance: Tolerance,
    /// The line so far, in document space, from the press point; points are
    /// added once the drag has left the dead zone.
    points: Vec<Point>,
}

impl LassoDrag {
    pub(super) fn new(origin: DragOrigin, tolerance: Tolerance) -> Self {
        Self {
            origin,
            tolerance,
            points: vec![origin.down_at],
        }
    }

    /// Records the pointer: stored once past the dead zone and far enough
    /// from the last stored point.
    fn note(&mut self, point: Point) {
        self.origin.note(point);
        if !self.origin.is_active_at(point) {
            return;
        }
        let spacing = self.origin.dead_zone_mm() / LASSO_POINTS_PER_DEAD_ZONE;
        let far_enough = self
            .points
            .last()
            .is_none_or(|last| last.vector_to(point).length() > spacing);
        if far_enough {
            self.points.push(point);
        }
    }

    /// The line with the pointer at `pointer` as its live end.
    fn line_to(&self, pointer: Point) -> Vec<Point> {
        let mut line = self.points.clone();
        line.push(pointer);
        line
    }
}

/// The geometry of a gesture for drawing it.
#[derive(Debug, Clone, PartialEq)]
pub enum GestureShape {
    /// The marquee rectangle from the press `from` to the pointer `to`, with
    /// the mode that decides its colour and what it selects.
    Box {
        /// The press point.
        from: Point,
        /// The pointer.
        to: Point,
        /// What the box selects right now (direction, inverted by Alt).
        mode: MarqueeMode,
    },
    /// The lasso's freehand line, ending at the pointer.
    Line(Vec<Point>),
}

/// A gesture past its dead zone, as it would resolve with the modifiers of
/// this moment: read live for the overlay and the legend, so a modifier
/// change reaches them with the pointer at rest (criterion 14).
#[derive(Debug, Clone, PartialEq)]
pub struct LiveGesture {
    /// What to draw.
    pub shape: GestureShape,
    /// How the result combines with the selection on release.
    pub combine: SelectionCombine,
}

/// Which selection gesture a press armed, from the press on (the cursor
/// follows it before any movement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureKind {
    /// A marquee box.
    Marquee,
    /// A freehand touch line.
    Lasso,
}

impl SelectTool {
    /// Records the pointer for a gesture in flight.
    pub(super) fn gesture_pointer_moved(&mut self, point: Point) {
        match &mut self.drag {
            SelectDrag::Marquee(drag) => drag.origin.note(point),
            SelectDrag::Lasso(drag) => drag.note(point),
            SelectDrag::None
            | SelectDrag::Moving(_)
            | SelectDrag::Transforming(_)
            | SelectDrag::GroupTransforming(_) => {}
        }
    }

    /// Ends a marquee or lasso in flight without resolving it (the tool was
    /// switched away from): its legend and overlay must not return with the
    /// Select tool. A move or transform drag is left as it is.
    pub fn cancel_gesture(&mut self) {
        if self.gesture_kind().is_some() {
            self.drag = SelectDrag::None;
        }
    }

    /// Ends the Alt-click cycle: the selection was cleared by Escape, so the
    /// next Alt-click starts again at the nearest candidate.
    pub fn end_cycle(&mut self) {
        self.cycle = None;
    }

    /// The marquee or lasso armed by the press in flight, even before it has
    /// left the dead zone.
    #[must_use]
    pub const fn gesture_kind(&self) -> Option<GestureKind> {
        match self.drag {
            SelectDrag::Marquee(_) => Some(GestureKind::Marquee),
            SelectDrag::Lasso(_) => Some(GestureKind::Lasso),
            SelectDrag::None
            | SelectDrag::Moving(_)
            | SelectDrag::Transforming(_)
            | SelectDrag::GroupTransforming(_) => None,
        }
    }

    /// The gesture in flight as a release at `pointer` with `modifiers` would
    /// resolve it, or `None` when none runs or it has not left the 3 px dead
    /// zone (it is still a click).
    #[must_use]
    pub fn live_gesture(&self, pointer: Point, modifiers: Modifiers) -> Option<LiveGesture> {
        let combine = SelectionCombine::from_modifiers(modifiers.shift, modifiers.ctrl);
        match &self.drag {
            SelectDrag::Marquee(drag) if drag.origin.is_active_at(pointer) => Some(LiveGesture {
                shape: GestureShape::Box {
                    from: drag.origin.down_at,
                    to: pointer,
                    mode: MarqueeMode::for_drag(drag.origin.down_at, pointer, modifiers.alt),
                },
                combine,
            }),
            SelectDrag::Lasso(drag) if drag.origin.is_active_at(pointer) => Some(LiveGesture {
                shape: GestureShape::Line(drag.line_to(pointer)),
                combine,
            }),
            _ => None,
        }
    }

    /// The release of a marquee at `point`. A release inside the dead zone is
    /// a click on empty canvas: it clears the selection, unless Shift or Ctrl
    /// is held (criterion 8; slice 4 criterion 15). A drag selects what its
    /// box finds in the mode in effect now and combines it with the selection
    /// by the modifiers of this moment (criteria 9 to 13).
    pub(super) fn finish_marquee(
        &mut self,
        drag: &MarqueeDrag,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        modifiers: Modifiers,
    ) {
        let combine = SelectionCombine::from_modifiers(modifiers.shift, modifiers.ctrl);
        if !drag.origin.is_active_at(point) {
            if combine == SelectionCombine::Replace {
                selection.clear();
            }
            return;
        }
        self.cycle = None;
        let from = drag.origin.down_at;
        let mode = MarqueeMode::for_drag(from, point, modifiers.alt);
        let found = objects_in_marquee(objects, from, point, mode, BOX_TOLERANCE);
        selection.retain_existing(objects);
        selection.apply(combine, &found);
    }

    /// The release of a lasso at `point`. Inside the dead zone it is an
    /// Alt-click: one step of the candidate cycle (criteria 4 to 7), which
    /// replaces the selection with that candidate. Past it, every object the
    /// line came within tolerance of is combined with the selection by the
    /// modifiers of this moment (criteria 18 to 20).
    pub(super) fn finish_lasso(
        &mut self,
        drag: &LassoDrag,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        modifiers: Modifiers,
    ) {
        let tolerance = drag.tolerance;
        if !drag.origin.is_active_at(point) {
            let at = drag.origin.down_at;
            let mut cycle = match self.cycle.take() {
                Some(cycle) if cycle.continues_at(at, tolerance) => cycle,
                _ => ClickCycle::fresh(at),
            };
            if let Some(chosen) = cycle.advance(objects, tolerance) {
                selection.select_single(chosen);
                self.cycle = Some(cycle);
            }
            return;
        }
        self.cycle = None;
        let found = hit_test_objects_along(objects, &drag.line_to(point), tolerance);
        selection.retain_existing(objects);
        selection.apply(
            SelectionCombine::from_modifiers(modifiers.shift, modifiers.ctrl),
            &found,
        );
    }
}
