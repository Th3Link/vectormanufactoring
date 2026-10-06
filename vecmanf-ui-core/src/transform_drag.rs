//! Resolves a resize, rotate or skew gesture into the object's resulting
//! snapshot (`specs/0005-object-transform/adrs.md`: "a resize rewrites
//! geometry... preview and commit share one implementation"; extended by
//! `specs/object-transform-refinements/adrs.md`, "one resolving function
//! per gesture"). `SelectTool`'s live preview, its commit on release and
//! the typed numeric entry all go through [`TransformDrag::resolve`]'s
//! building blocks ([`resize_by_local_delta`], [`rotate_by`],
//! [`skew_by_angle`]), so each rule exists once. The primitive-specific
//! resize arithmetic is in [`crate::transform_primitive`].

use vecmanf_document_core::{
    Angle, Document, Length, ObjectSnapshot, Point, PrimitiveSnapshot, Shape, Vec2,
};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;
use crate::skew_math::{skew_angle, skew_factor, skew_frame};
use crate::transform_commit::{commit_gesture, sane_or};
use crate::transform_handle_layout::{Side, TransformHandle};
use crate::transform_math::{
    resize_anchor_local_position, resize_local_box, rotate_delta_angle, rotate_pivot,
    scaled_and_floored, stroke_or_radius_factor,
};
use crate::transform_primitive::resize_primitive;

/// A resize/corner-radius drag can never drive a stroke width to zero
/// or below (acceptance criterion 8 of `specs/0005-object-transform/
/// specification.md`: "the stroke width stays at the smallest value
/// still above zero") — this is that smallest value. Not a design-
/// system token: no acceptance criterion pins an exact number, only
/// that it must stay strictly positive.
const MIN_STROKE_WIDTH_MM: f64 = 0.01;

/// Whether a resize also scales the object's stroke width (acceptance
/// criteria 8, 26-31 of `specs/0005-object-transform/specification.md`,
/// the "Scale stroke width" switch). Not a `bool`: the resize options already
/// takes two modifier bools, and a third trips
/// `clippy::fn_params_excessive_bools`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrokeScaling {
    /// The default: a resize leaves the stroke width exactly as it was,
    /// not even rewritten (AC 8).
    #[default]
    Keep,
    /// The stroke width scales with the resize, √(sx·sy), floored at
    /// 0.01 mm (AC 26).
    Proportional,
}

/// The modifiers and mode a resize drag runs with: Shift (center pivot,
/// AC 7), Ctrl (proportional, AC 5) and the [`StrokeScaling`] captured at
/// the press (AC 28).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResizeOptions {
    pub(crate) shift: bool,
    pub(crate) ctrl: bool,
    pub(crate) stroke_scaling: StrokeScaling,
}

/// The press point of a Select-tool drag, its dead zone and the handle
/// set it froze (`adrs.md`, "a 3 px dead zone for every Select-tool drag on
/// the selected object"): the drag writes and previews nothing until the
/// pointer has left `dead_zone_mm` around the press; from then on it is
/// active for the rest of its life, and every delta is taken from the
/// original press point, so the object follows the pointer 1:1.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DragOrigin {
    /// Where the press happened.
    pub(crate) down_at: Point,
    dead_zone_mm: f64,
    passed: bool,
    /// Whether the four side rotate handles were showing at the press: the
    /// handle set is frozen for the whole drag (criterion 6).
    pub(crate) side_rotate_revealed: bool,
}

impl DragOrigin {
    pub(crate) const fn new(down_at: Point, dead_zone_mm: f64, side_rotate_revealed: bool) -> Self {
        Self {
            down_at,
            dead_zone_mm,
            passed: false,
            side_rotate_revealed,
        }
    }

    /// Records a pointer position: once it is outside the dead zone the drag
    /// stays active.
    pub(crate) fn note(&mut self, point: Point) {
        self.passed = self.passed || self.outside_dead_zone(point);
    }

    /// Whether the drag is active at `point` (it has left the dead zone now
    /// or earlier).
    pub(crate) fn is_active_at(&self, point: Point) -> bool {
        self.passed || self.outside_dead_zone(point)
    }

    fn outside_dead_zone(&self, point: Point) -> bool {
        self.down_at.vector_to(point).length() > self.dead_zone_mm
    }
}

/// A resize, rotate or skew drag in flight: the snapshot and box at the
/// press, the handle grabbed, and the mode captured at the press. Preview,
/// release and typed entry all resolve it through the functions below, so
/// each rule exists once (`adrs.md`, "one resolving function per
/// gesture").
#[derive(Debug, Clone)]
pub(crate) struct TransformDrag {
    pub(crate) origin: DragOrigin,
    pub(crate) start: ObjectSnapshot,
    pub(crate) start_box: OrientedBox,
    pub(crate) handle: TransformHandle,
    /// The tool's [`StrokeScaling`] as of the press: a drag uses it for
    /// its whole duration, so a toggle mid-drag applies from the next drag
    /// (AC 28 of slice 5).
    pub(crate) stroke_scaling: StrokeScaling,
}

impl TransformDrag {
    /// The object as it would commit with the pointer at `current` and the
    /// modifiers as given: `start` unchanged if the result would not be
    /// finite and sane, or for a handle that is not a transform handle.
    /// Always computed from the state at the press, so a Shift or Ctrl
    /// change mid-drag accumulates no error (criteria 14, 39).
    pub(crate) fn resolve(&self, current: Point, shift: bool, ctrl: bool) -> ObjectSnapshot {
        let (start, start_box, down_at) = (&self.start, &self.start_box, self.origin.down_at);
        match self.handle {
            TransformHandle::Resize(direction) => {
                // `f64::max`/`clamp` swallow a NaN (turning it into 0 or an
                // edge), so a non-finite pointer must be refused first.
                let local_delta = local_delta_of(start_box, down_at, current);
                if !(local_delta.x.is_finite() && local_delta.y.is_finite()) {
                    return start.clone();
                }
                resize_by_local_delta(
                    start,
                    start_box,
                    direction,
                    local_delta,
                    ResizeOptions {
                        shift,
                        ctrl,
                        stroke_scaling: self.stroke_scaling,
                    },
                )
            }
            TransformHandle::Rotate(direction) => {
                let pivot = rotate_pivot(start_box, direction, shift);
                rotate_by(
                    start,
                    pivot,
                    rotate_delta_angle(pivot, down_at, current, ctrl),
                )
            }
            TransformHandle::Skew(side) => skew_by_angle(
                start,
                start_box,
                side,
                shift,
                skew_angle(start_box, side, down_at, current, shift, ctrl),
            ),
            TransformHandle::Move => start.clone(),
        }
    }

    /// The skew angle of the drag at `current` (the readout's value), for a
    /// skew drag.
    pub(crate) fn skew_angle_at(&self, current: Point, shift: bool, ctrl: bool) -> Option<Angle> {
        let TransformHandle::Skew(side) = self.handle else {
            return None;
        };
        Some(skew_angle(
            &self.start_box,
            side,
            self.origin.down_at,
            current,
            shift,
            ctrl,
        ))
    }

    /// Writes `result` (from [`TransformDrag::resolve`]) with the one-commit
    /// `Document` method matching the gesture (criterion 41, 46): a resize
    /// through `commit_resize` with the stroke mode captured at the press, a
    /// rotate through `rotate_object`, a skew through `commit_resize` with
    /// no stroke width (anchors only).
    pub(crate) fn commit(&self, document: &Document, result: &ObjectSnapshot) {
        commit_gesture(document, self.handle, result, self.stroke_scaling);
    }
}

/// The pivot (or fixed point) a drag or entry of `handle` would use right
/// now (criteria 13, 28, 40, 55): the point the pivot marker shows. A
/// polygon or star always scales about its center; a resize about the
/// opposite corner/edge or, under Shift, the box center; a rotate per
/// [`rotate_pivot`]; a skew about the fixed edge's midpoint or, under Shift,
/// the center. `None` for the centre move handle.
pub(crate) fn pivot_for(
    handle: TransformHandle,
    object: &ObjectSnapshot,
    box_: &OrientedBox,
    shift: bool,
) -> Option<Point> {
    match handle {
        TransformHandle::Resize(direction) => {
            let local = if is_polygon_or_star(object) {
                box_.local_center()
            } else {
                resize_anchor_local_position(box_.min, box_.max, direction, shift)
            };
            Some(box_.to_document(local))
        }
        TransformHandle::Rotate(direction) => Some(rotate_pivot(box_, direction, shift)),
        TransformHandle::Skew(side) => Some(skew_frame(box_, side, shift).fixed_point),
        TransformHandle::Move => None,
    }
}

/// Whether `object` is a polygon or a star — the one kind whose transform
/// handles are corner-only and always-uniform (slice 5, criterion 11).
pub(crate) fn is_polygon_or_star(object: &ObjectSnapshot) -> bool {
    matches!(
        object,
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Polygon { .. } | Shape::Star { .. },
            ..
        })
    )
}

/// The pointer's displacement from `down_at` to `current`, in the box's own
/// local frame.
pub(crate) fn local_delta_of(start_box: &OrientedBox, down_at: Point, current: Point) -> Vec2 {
    start_box
        .to_local(down_at)
        .vector_to(start_box.to_local(current))
}

/// The object after resizing it by moving `direction`'s handle by
/// `local_delta` (already in the box's local frame; acceptance criteria
/// 4-13 of slice 5): the one resize rule, called with the pointer's delta by
/// a drag and with a typed size's delta by the entry. `start` unchanged if
/// the result would not be finite and sane. The stroke width scales only
/// for [`StrokeScaling::Proportional`]; the corner radius scales either way
/// (AC 9, 31).
pub(crate) fn resize_by_local_delta(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    local_delta: Vec2,
    options: ResizeOptions,
) -> ObjectSnapshot {
    let ResizeOptions {
        shift,
        ctrl,
        stroke_scaling,
    } = options;
    let (mut resized, factor) = match start {
        ObjectSnapshot::Primitive(primitive) => {
            let (resized, factor) =
                resize_primitive(primitive, start_box, direction, local_delta, shift, ctrl);
            (ObjectSnapshot::Primitive(resized), factor)
        }
        ObjectSnapshot::Path(path) => {
            let resized = resize_local_box(
                start_box.min,
                start_box.max,
                direction,
                local_delta,
                shift,
                ctrl,
            );
            let anchor_local =
                resize_anchor_local_position(start_box.min, start_box.max, direction, shift);
            let scaled = path.scaled(start_box.to_document(anchor_local), resized.sx, resized.sy);
            (
                ObjectSnapshot::Path(scaled),
                stroke_or_radius_factor(resized.sx, resized.sy),
            )
        }
    };
    if stroke_scaling == StrokeScaling::Proportional {
        scale_stroke(&mut resized, factor);
    }
    sane_or(start, resized)
}

/// The object rotated by `delta` about `pivot` (criteria 12-17): the one
/// rotate rule, called with the pointer's swept angle by a drag and with
/// `target − rotation` by the entry. `start` unchanged if the result would
/// not be finite and sane.
pub(crate) fn rotate_by(start: &ObjectSnapshot, pivot: Point, delta: Angle) -> ObjectSnapshot {
    sane_or(start, start.rotated(pivot, delta))
}

/// The object skewed by `angle` from `side`'s handle (criteria 38, 39, 51):
/// a path's anchors and handle vectors sheared about the fixed line, with
/// `rotation` untouched. A primitive is returned unchanged, and so is a
/// zero angle (a drag back to its start must not commit a rounding
/// residue).
pub(crate) fn skew_by_angle(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    side: Side,
    shift: bool,
    angle: Angle,
) -> ObjectSnapshot {
    let ObjectSnapshot::Path(path) = start else {
        return start.clone();
    };
    let frame = skew_frame(start_box, side, shift);
    let k = skew_factor(&frame, angle);
    if k.abs() <= 0.0 {
        return start.clone();
    }
    let (ku, kv) = if frame.along_u { (k, 0.0) } else { (0.0, k) };
    sane_or(
        start,
        ObjectSnapshot::Path(path.sheared(frame.fixed_point, ku, kv)),
    )
}

/// Scales `object`'s stroke width by `factor`, floored above zero.
fn scale_stroke(object: &mut ObjectSnapshot, factor: f64) {
    let floor = Length::from_mm(MIN_STROKE_WIDTH_MM);
    match object {
        ObjectSnapshot::Primitive(p) => {
            p.stroke_width = scaled_and_floored(p.stroke_width, factor, floor);
        }
        ObjectSnapshot::Path(p) => {
            p.stroke_width = scaled_and_floored(p.stroke_width, factor, floor);
        }
    }
}
