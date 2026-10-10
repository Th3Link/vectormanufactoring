//! Resolves a resize, rotate or skew gesture into the object's resulting
//! snapshot (`specs/0005-object-transform/adrs.md`: "a resize rewrites
//! geometry... preview and commit share one implementation"; extended by
//! `specs/0008-object-transform-refinements/adrs.md`, "one resolving function
//! per gesture"). `SelectTool`'s live preview, its commit on release and
//! the typed numeric entry all go through [`TransformDrag::resolve`]'s
//! building blocks ([`resize_by_local_delta`], [`rotate_by`],
//! [`skew_by_angle`]), so each rule exists once. The primitive-specific
//! resize arithmetic is in [`crate::transform_primitive`].

use curvyo_document_core::{
    Angle, Corner, Document, Length, ObjectSnapshot, PathSnapshot, Point, PrimitiveSnapshot, Shape,
    Vec2, effective_corner_radii,
};

use crate::ResizeDirection;
use crate::group_transform::is_stretch;
use crate::oriented_box::OrientedBox;
use crate::param_edit::{PARAM_EQUAL_EPSILON, apply_param, radius_is_limited, value_from_pointer};
use crate::param_handles::ParamHandle;
use crate::skew_math::{skew_angle, skew_factor, skew_frame};
use crate::transform_commit::{commit_gesture, sane_or};
use crate::transform_handle_layout::{EditHandle, Side, is_corner};
use crate::transform_math::{
    is_polygon_or_star, resize_anchor_local_position, resize_local_box, rotate_delta_for,
    rotate_pivot, scaled_and_floored, stroke_or_radius_factor,
};
use crate::transform_primitive::{resize_primitive, scaled_star_frame};

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

/// Whether a resize of a rectangle also scales its corner radius, the
/// "Scale corner radius" switch (`specs/0009-unified-object-editing/`, criterion
/// 23; customer decision 2026-10-06). An enum, not a `bool`, for the same
/// reason as [`StrokeScaling`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerRadiusScaling {
    /// The default: a resize keeps the radius's absolute size, and the stored
    /// register is not rewritten.
    #[default]
    Keep,
    /// The radius scales with the resize, √(sx·sy) like the stroke width
    /// (`0005` criterion 9).
    Proportional,
}

/// Whether a corner radius handle changes all four radii or only its own
/// corner: the Select bar's "Link corners" switch
/// (`specs/0013-rectangle-corner-radii/` criteria 2 and 3). Session state of the
/// class of [`ScaleModes`], not part of it: it acts on the corner handles, not
/// on a resize. An enum, not a `bool`, for the same reason as
/// [`StrokeScaling`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerLinking {
    /// The default: a corner handle sets all four radii; Shift changes one
    /// corner for that drag.
    #[default]
    Linked,
    /// A corner handle changes its own corner; Shift sets all four for that
    /// drag.
    Unlinked,
}

impl CornerLinking {
    /// Whether a drag or entry begun with this switch state and `shift` held
    /// changes one corner only: the switch and Shift differ (an exclusive or,
    /// criterion 3). Read once at the press and frozen.
    #[must_use]
    pub const fn is_unlinked_with(self, shift: bool) -> bool {
        matches!(self, Self::Unlinked) != shift
    }
}

/// The two Select-tool switches that decide what else a resize scales. Tool
/// state, never written to the document (criteria 23, 28 of slice 5 and 23
/// here): a drag captures it at the press, a typed size when the entry opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScaleModes {
    /// "Scale stroke width".
    pub stroke: StrokeScaling,
    /// "Scale corner radius".
    pub radius: CornerRadiusScaling,
}

/// The modifiers and modes a resize drag runs with: Shift (center pivot,
/// AC 7), Ctrl (proportional, AC 5) and the [`ScaleModes`] captured at the
/// press (AC 28).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResizeOptions<'a> {
    pub(crate) shift: bool,
    pub(crate) ctrl: bool,
    pub(crate) modes: ScaleModes,
    /// A typed size: a polygon's or star's corner then takes free W and H like a
    /// rectangle's, where a corner drag keeps the diagonal rule.
    pub(crate) typed: bool,
    /// The path a polygon or star becomes if the resize is a stretch
    /// (`specs/0019-multi-object-transform/` criterion 56).
    pub(crate) converted: Option<&'a PathSnapshot>,
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
    /// Whether Shift was held at the press. It reveals the four side rotate
    /// handles, a set frozen for the whole drag (criterion 6), and later
    /// inverts a corner-radius link (`specs/0013-rectangle-corner-radii/`).
    pub(crate) shift_at_press: bool,
}

impl DragOrigin {
    pub(crate) const fn new(down_at: Point, dead_zone_mm: f64, shift_at_press: bool) -> Self {
        Self {
            down_at,
            dead_zone_mm,
            passed: false,
            shift_at_press,
        }
    }

    /// The radius of the dead zone around the press, millimetres.
    pub(crate) const fn dead_zone_mm(&self) -> f64 {
        self.dead_zone_mm
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
    pub(crate) handle: EditHandle,
    /// The tool's [`ScaleModes`] as of the press: a drag uses them for
    /// its whole duration, so a toggle mid-drag applies from the next drag
    /// (AC 28 of slice 5, criterion 23 here).
    pub(crate) modes: ScaleModes,
    /// The radius gain ([`crate::radius_gain`]) of a corner-radius drag,
    /// frozen at the press: it depends on the screen scale, so a wheel zoom
    /// during the drag cannot change the mapping between frames and release
    /// equals preview. `1` for every other handle.
    pub(crate) param_gain: f64,
    /// Whether a corner radius drag changes one corner only, decided at the
    /// press from the "Link corners" switch and Shift
    /// ([`CornerLinking::is_unlinked_with`]) and frozen for the whole drag.
    /// `false` for every other handle.
    pub(crate) unlinked: bool,
    /// For an edge resize of a polygon or star, the path it becomes (criterion
    /// 56), built at the press so that the preview and the commit are the same
    /// snapshot.
    pub(crate) converted: Option<Box<PathSnapshot>>,
}

/// What the live readout of a corner radius drag needs beyond the resolved
/// object (who follows is [`crate::SelectTool::corner_drag_changes_all`]) (`specs/0013-rectangle-corner-radii/` criteria 4, 7,
/// 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamDragInfo {
    /// A limit stops the drag at this pointer position (the readout says "max").
    pub limited: bool,
    /// The press found unequal effective radii and the drag overwrites them (a
    /// linked drag): the readout adds "all corners".
    pub overwrites_unequal: bool,
}

impl TransformDrag {
    /// The readout facts of a corner radius drag with the pointer at `current`;
    /// `None` for any other handle or a value the primitive has no use for.
    pub(crate) fn param_info(&self, current: Point) -> Option<ParamDragInfo> {
        let EditHandle::Param(handle @ ParamHandle::CornerRadius(_)) = self.handle else {
            return None;
        };
        let local_delta = local_delta_of(&self.start_box, self.origin.down_at, current);
        let value = value_from_pointer(
            &self.start,
            handle,
            local_delta,
            self.param_gain,
            self.unlinked,
        )?;
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect {
                bounds,
                corner_radii,
            },
            ..
        }) = &self.start
        else {
            return None;
        };
        let effective = effective_corner_radii(*bounds, *corner_radii);
        let unequal = Corner::ALL.iter().any(|&corner| {
            (effective.get(corner).as_mm() - effective.tl.as_mm()).abs() > PARAM_EQUAL_EPSILON
        });
        Some(ParamDragInfo {
            limited: radius_is_limited(&self.start, value),
            overwrites_unequal: !self.unlinked && unequal,
        })
    }

    /// The object as it would commit with the pointer at `current` and the
    /// modifiers as given: `start` unchanged if the result would not be
    /// finite and sane, or for a handle that is not a transform handle.
    /// Always computed from the state at the press, so a Shift or Ctrl
    /// change mid-drag accumulates no error (criteria 14, 39).
    pub(crate) fn resolve(&self, current: Point, shift: bool, ctrl: bool) -> ObjectSnapshot {
        let (start, start_box, down_at) = (&self.start, &self.start_box, self.origin.down_at);
        match self.handle {
            EditHandle::Resize(direction) => {
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
                        modes: self.modes,
                        typed: false,
                        converted: self.converted.as_deref(),
                    },
                )
            }
            EditHandle::Rotate(direction) => {
                let pivot = rotate_pivot(start_box, direction, shift);
                rotate_by(
                    start,
                    pivot,
                    rotate_delta_for(start, pivot, down_at, current, ctrl),
                )
            }
            EditHandle::Skew(side) => skew_by_angle(
                start,
                start_box,
                side,
                shift,
                skew_angle(start_box, side, down_at, current, shift, ctrl),
            ),
            EditHandle::Param(handle) => {
                let local_delta = local_delta_of(start_box, down_at, current);
                value_from_pointer(start, handle, local_delta, self.param_gain, self.unlinked)
                    .map_or_else(
                        || start.clone(),
                        |value| sane_or(start, apply_param(start, value)),
                    )
            }
            EditHandle::Move => start.clone(),
        }
    }

    /// The skew angle of the drag at `current` (the readout's value), for a
    /// skew drag.
    pub(crate) fn skew_angle_at(&self, current: Point, shift: bool, ctrl: bool) -> Option<Angle> {
        let EditHandle::Skew(side) = self.handle else {
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
        commit_gesture(document, self.handle, result, self.modes);
    }
}

/// The pivot (or fixed point) a drag or entry of `handle` would use right
/// now (criteria 13, 28, 40, 55): the point the pivot marker shows. A
/// polygon or star always scales about its center; a resize about the
/// opposite corner/edge or, under Shift, the box center; a rotate per
/// [`rotate_pivot`]; a skew about the fixed edge's midpoint or, under Shift,
/// the center. `None` for the centre move handle.
pub(crate) fn pivot_for(
    handle: EditHandle,
    object: &ObjectSnapshot,
    box_: &OrientedBox,
    shift: bool,
) -> Option<Point> {
    match handle {
        EditHandle::Resize(direction) => {
            let local = if is_polygon_or_star(object) && is_corner(direction) {
                box_.local_center()
            } else {
                resize_anchor_local_position(box_.min, box_.max, direction, shift)
            };
            Some(box_.to_document(local))
        }
        EditHandle::Rotate(direction) => Some(rotate_pivot(box_, direction, shift)),
        EditHandle::Skew(side) => Some(skew_frame(box_, side, shift).fixed_point),
        EditHandle::Move | EditHandle::Param(_) => None,
    }
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
/// for [`StrokeScaling::Proportional`], the corner radius only for
/// [`CornerRadiusScaling::Proportional`] (AC 9, 31 of slice 5; criterion 23
/// of `unified-object-editing`).
pub(crate) fn resize_by_local_delta(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    local_delta: Vec2,
    options: ResizeOptions,
) -> ObjectSnapshot {
    let ResizeOptions {
        shift, ctrl, modes, ..
    } = options;
    let (mut resized, factor) = if let Some(stretched) =
        polygon_star_stretch(start, start_box, direction, local_delta, &options)
    {
        stretched
    } else {
        match start {
            ObjectSnapshot::Primitive(primitive) => {
                let (resized, factor) = resize_primitive(
                    primitive,
                    start_box,
                    direction,
                    local_delta,
                    (shift, ctrl),
                    modes.radius,
                );
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
                let scaled =
                    path.scaled(start_box.to_document(anchor_local), resized.sx, resized.sy);
                (
                    ObjectSnapshot::Path(scaled),
                    stroke_or_radius_factor(resized.sx, resized.sy),
                )
            }
        }
    };
    if modes.stroke == StrokeScaling::Proportional {
        scale_stroke(&mut resized, factor);
    }
    sane_or(start, resized)
}

/// A polygon's or star's resize along its box axes (`specs/0019-multi-object-
/// transform/` criterion 56): an edge drag, or a typed size, with the opposite side
/// (or the centre under Shift; always the centre for a corner) fixed. Unequal
/// factors are a stretch: the shape becomes its `converted` path, scaled along the
/// box's axes. Equal factors are a uniform scale and keep the shape. `None` for any
/// other object, and for a corner drag, which keeps the diagonal rule of
/// [`resize_primitive`] (always proportional, `0005` criterion 11).
fn polygon_star_stretch(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    local_delta: Vec2,
    options: &ResizeOptions,
) -> Option<(ObjectSnapshot, f64)> {
    let ObjectSnapshot::Primitive(primitive) = start else {
        return None;
    };
    if !is_polygon_or_star(start) || (is_corner(direction) && !options.typed) {
        return None;
    }
    let resized = resize_local_box(
        start_box.min,
        start_box.max,
        direction,
        local_delta,
        options.shift,
        options.ctrl,
    );
    let (sx, sy) = (resized.sx, resized.sy);
    if !is_stretch(sx, sy) {
        let shape = match primitive.shape {
            Shape::Polygon { frame, point_count } => Shape::Polygon {
                frame: scaled_star_frame(frame, sx),
                point_count,
            },
            Shape::Star {
                frame,
                point_count,
                inner_ratio,
            } => Shape::Star {
                frame: scaled_star_frame(frame, sx),
                point_count,
                inner_ratio,
            },
            _ => return None,
        };
        return Some((
            ObjectSnapshot::Primitive(PrimitiveSnapshot {
                shape,
                ..primitive.clone()
            }),
            sx,
        ));
    }
    let Some(path) = options.converted else {
        return Some((start.clone(), 1.0));
    };
    let pivot_local = if is_corner(direction) {
        start_box.local_center()
    } else {
        resize_anchor_local_position(start_box.min, start_box.max, direction, options.shift)
    };
    let scaled = path.scaled_along(start_box.to_document(pivot_local), sx, sy, start_box.angle);
    Some((
        ObjectSnapshot::Path(scaled),
        stroke_or_radius_factor(sx, sy),
    ))
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
    sane_or(
        start,
        skewed_unchecked(start, start_box, side, shift, angle),
    )
}

/// [`skew_by_angle`] before the sanity check: the sheared path whatever its
/// coordinates are, `start` for a primitive or a zero angle. The typed skew
/// uses it to tell a result refused for its size from one that did not change.
pub(crate) fn skewed_unchecked(
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
    ObjectSnapshot::Path(path.sheared(frame.fixed_point, ku, kv))
}

/// Scales `object`'s stroke width by `factor`, floored above zero.
pub(crate) fn scale_stroke(object: &mut ObjectSnapshot, factor: f64) {
    let floor = Length::from_mm(MIN_STROKE_WIDTH_MM);
    match object {
        ObjectSnapshot::Primitive(p) => {
            p.style.stroke.width = scaled_and_floored(p.style.stroke.width, factor, floor);
        }
        ObjectSnapshot::Path(p) => {
            p.style.stroke.width = scaled_and_floored(p.style.stroke.width, factor, floor);
        }
    }
}
