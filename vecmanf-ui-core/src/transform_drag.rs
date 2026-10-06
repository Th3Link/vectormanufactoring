//! Resolves a resize or rotate drag into the object's resulting snapshot
//! (`specs/0005-object-transform/adrs.md`: "a resize rewrites geometry...
//! preview and commit share one implementation"). `SelectTool`'s live
//! preview and its commit on release both call [`compute_resize`] /
//! [`compute_rotate`], and the shape tools' own rectangle and ellipse
//! resizes share [`pin_resize_anchor`], so each rule exists once.

use vecmanf_document_core::{
    AnchorId, Angle, Document, EllipseFrame, Length, NodeId, ObjectSnapshot, Point,
    PrimitiveSnapshot, RectBounds, Shape, StarFrame, Vec2, shape_center, translate_shape,
};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;
use crate::transform_math::{
    polygon_star_resize_factor, resize_anchor_local_position, resize_local_box, rotate_delta_angle,
    rotate_pivot, scaled_and_floored, stroke_or_radius_factor,
};

/// A resize/corner-radius drag can never drive a stroke width to zero
/// or below (acceptance criterion 8 of `specs/0005-object-transform/
/// specification.md`: "the stroke width stays at the smallest value
/// still above zero") — this is that smallest value. Not a design-
/// system token: no acceptance criterion pins an exact number, only
/// that it must stay strictly positive.
const MIN_STROKE_WIDTH_MM: f64 = 0.01;

/// The largest coordinate or size (millimetres, 10 km) a drag may write.
/// A pointer value beyond it — or NaN/infinite — is hostile or broken
/// input; the drag then resolves to "no change" instead of writing
/// geometry a later open would refuse (`adrs.md`: "a file must never
/// become unopenable from a drag").
const MAX_COORDINATE_MM: f64 = 1e7;

/// Whether a resize also scales the object's stroke width (acceptance
/// criteria 8, 26-31 of `specs/0005-object-transform/specification.md`,
/// the "Scale stroke width" switch). Not a `bool`: `compute_resize` already
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

/// The object after resizing it by dragging `direction`'s handle from
/// `down_at` to `current` (acceptance criteria 4-13), or `start`
/// unchanged if the result would not be finite and sane. The stroke width
/// scales only for [`StrokeScaling::Proportional`]; the corner radius
/// scales either way (AC 9, 31).
pub(crate) fn compute_resize(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    down_at: Point,
    current: Point,
    options: ResizeOptions,
) -> ObjectSnapshot {
    let ResizeOptions {
        shift,
        ctrl,
        stroke_scaling,
    } = options;
    let local_delta = start_box
        .to_local(down_at)
        .vector_to(start_box.to_local(current));
    // `f64::max`/`clamp` swallow a NaN (turning it into 0 or an edge), so
    // a non-finite pointer must be refused before any arithmetic.
    if !(local_delta.x.is_finite() && local_delta.y.is_finite()) {
        return start.clone();
    }
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

/// The object after rotating it by dragging the rotate handle from
/// `down_at` to `current` (acceptance criteria 15-17): about the box
/// center, or — under `shift` — the bottom-edge midpoint; Ctrl snaps the
/// swept angle to 15°. `start` unchanged if the result would not be
/// finite and sane.
pub(crate) fn compute_rotate(
    start: &ObjectSnapshot,
    start_box: &OrientedBox,
    down_at: Point,
    current: Point,
    shift: bool,
    ctrl: bool,
) -> ObjectSnapshot {
    let pivot = rotate_pivot(start_box, shift);
    let delta_angle = rotate_delta_angle(pivot, down_at, current, ctrl);
    sane_or(start, start.rotated(pivot, delta_angle))
}

/// A primitive's resized frame and the stroke/radius factor that goes
/// with it. Rectangles and ellipses resize their local box (the corner
/// radius scales with the factor, acceptance criterion 9); a polygon or
/// star is always a uniform outer-radius scale (criterion 11).
fn resize_primitive(
    primitive: &PrimitiveSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    local_delta: Vec2,
    shift: bool,
    ctrl: bool,
) -> (PrimitiveSnapshot, f64) {
    let box_resize = || {
        resize_local_box(
            start_box.min,
            start_box.max,
            direction,
            local_delta,
            shift,
            ctrl,
        )
    };
    let (shape, factor) = match primitive.shape {
        Shape::Rect { corner_radius, .. } => {
            let resized = box_resize();
            let factor = stroke_or_radius_factor(resized.sx, resized.sy);
            let shape = Shape::Rect {
                bounds: RectBounds {
                    origin: resized.min,
                    width: Length::from_mm(resized.max.x - resized.min.x),
                    height: Length::from_mm(resized.max.y - resized.min.y),
                },
                corner_radius: scaled_and_floored(corner_radius, factor, Length::from_mm(0.0)),
            };
            (shape, factor)
        }
        Shape::Ellipse { .. } => {
            let resized = box_resize();
            let shape = Shape::Ellipse {
                frame: EllipseFrame {
                    center: Point::new(
                        f64::midpoint(resized.min.x, resized.max.x),
                        f64::midpoint(resized.min.y, resized.max.y),
                    ),
                    rx: Length::from_mm((resized.max.x - resized.min.x) / 2.0),
                    ry: Length::from_mm((resized.max.y - resized.min.y) / 2.0),
                },
            };
            (shape, stroke_or_radius_factor(resized.sx, resized.sy))
        }
        Shape::Polygon { frame, point_count } => {
            let factor = polygon_star_resize_factor(frame.radius.as_mm(), direction, local_delta);
            let shape = Shape::Polygon {
                frame: scaled_star_frame(frame, factor),
                point_count,
            };
            (shape, factor)
        }
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => {
            let factor = polygon_star_resize_factor(frame.radius.as_mm(), direction, local_delta);
            let shape = Shape::Star {
                frame: scaled_star_frame(frame, factor),
                point_count,
                inner_ratio,
            };
            (shape, factor)
        }
    };
    let anchor_local = resize_anchor_local_position(start_box.min, start_box.max, direction, shift);
    let pinned = pin_resize_anchor(shape, start_box.pivot, primitive.rotation, anchor_local);
    (
        PrimitiveSnapshot {
            shape: pinned,
            ..*primitive
        },
        factor,
    )
}

fn scaled_star_frame(frame: StarFrame, factor: f64) -> StarFrame {
    StarFrame {
        radius: Length::from_mm(frame.radius.as_mm() * factor),
        ..frame
    }
}

/// Keeps a resize's anchor fixed in document space.
///
/// A primitive rotates about its *own* frame center, which a resize just
/// moved — so on a rotated object the anchor (the opposite corner or
/// edge, or the center under Shift) would swing across the canvas.
/// `after` is the resized shape in its local frame, `before_pivot` the
/// frame center before the resize, `anchor_local` the anchor in local
/// coordinates (unchanged by the resize); the whole frame is translated
/// by however far the anchor moved. A no-op at zero rotation, where the
/// local and document frames agree, and for a polygon/star, whose center
/// does not move. The Select tool and the rectangle and ellipse tools'
/// own resizes all call this, so they cannot disagree.
pub(crate) fn pin_resize_anchor(
    after: Shape,
    before_pivot: Point,
    rotation: Angle,
    anchor_local: Point,
) -> Shape {
    let before = anchor_local.rotated_around(before_pivot, rotation);
    let after_anchor = anchor_local.rotated_around(shape_center(&after), rotation);
    translate_shape(after, after_anchor.vector_to(before))
}

/// [`pin_resize_anchor`] for the rectangle tool's own resize: the
/// opposite corner/edge of `start` stays put on screen.
pub(crate) fn pin_rect_resize(
    start: RectBounds,
    resized: RectBounds,
    direction: ResizeDirection,
    rotation: Angle,
) -> RectBounds {
    let (min, max) = (
        start.origin,
        Point::new(
            start.origin.x + start.width.as_mm(),
            start.origin.y + start.height.as_mm(),
        ),
    );
    let anchor = resize_anchor_local_position(min, max, direction, false);
    let center = Point::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
    let shape = Shape::Rect {
        bounds: resized,
        corner_radius: Length::from_mm(0.0),
    };
    match pin_resize_anchor(shape, center, rotation, anchor) {
        Shape::Rect { bounds, .. } => bounds,
        // invariant: `pin_resize_anchor` only translates; it never changes
        // the kind.
        _ => resized,
    }
}

/// [`pin_resize_anchor`] for the ellipse tool's own resize.
pub(crate) fn pin_ellipse_resize(
    start: EllipseFrame,
    resized: EllipseFrame,
    direction: ResizeDirection,
    rotation: Angle,
) -> EllipseFrame {
    let min = Point::new(
        start.center.x - start.rx.as_mm(),
        start.center.y - start.ry.as_mm(),
    );
    let max = Point::new(
        start.center.x + start.rx.as_mm(),
        start.center.y + start.ry.as_mm(),
    );
    let anchor = resize_anchor_local_position(min, max, direction, false);
    match pin_resize_anchor(
        Shape::Ellipse { frame: resized },
        start.center,
        rotation,
        anchor,
    ) {
        Shape::Ellipse { frame } => frame,
        // invariant: see `pin_rect_resize`.
        _ => resized,
    }
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

/// Writes a resize's resulting geometry, dispatching on the object's own
/// kind to the matching one-commit `Document` method. With
/// [`StrokeScaling::Keep`] the stroke width is passed as `None`, so the
/// stored value is never touched (AC 8: not even rewritten).
pub(crate) fn commit_resize(
    document: &Document,
    id: NodeId,
    result: &ObjectSnapshot,
    stroke_scaling: StrokeScaling,
) {
    let width = |w: Length| (stroke_scaling == StrokeScaling::Proportional).then_some(w);
    match result {
        ObjectSnapshot::Primitive(primitive) => match primitive.shape {
            Shape::Rect {
                bounds,
                corner_radius,
            } => {
                let _ =
                    document.resize_rect(id, bounds, corner_radius, width(primitive.stroke_width));
            }
            Shape::Ellipse { frame } => {
                let _ = document.resize_ellipse(id, frame, width(primitive.stroke_width));
            }
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => {
                let _ = document.resize_star_frame(id, frame, width(primitive.stroke_width));
            }
        },
        ObjectSnapshot::Path(path) => {
            let anchors: Vec<(AnchorId, Point, Vec2, Vec2)> = path
                .anchors
                .iter()
                .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
                .collect();
            let _ = document.resize_path(id, &anchors, width(path.stroke_width));
        }
    }
}

/// `resolved` if every number in it is finite and within
/// [`MAX_COORDINATE_MM`], otherwise `start` (no change).
fn sane_or(start: &ObjectSnapshot, resolved: ObjectSnapshot) -> ObjectSnapshot {
    if is_sane(&resolved) {
        resolved
    } else {
        start.clone()
    }
}

fn is_sane(object: &ObjectSnapshot) -> bool {
    let ok = |v: f64| v.is_finite() && v.abs() <= MAX_COORDINATE_MM;
    let numbers: Vec<f64> = match object {
        ObjectSnapshot::Path(path) => {
            let mut v = vec![path.stroke_width.as_mm(), path.rotation.as_radians()];
            for a in &path.anchors {
                v.extend([
                    a.point.x,
                    a.point.y,
                    a.handle_in.x,
                    a.handle_in.y,
                    a.handle_out.x,
                    a.handle_out.y,
                ]);
            }
            v
        }
        ObjectSnapshot::Primitive(p) => {
            let mut v = vec![p.stroke_width.as_mm(), p.rotation.as_radians()];
            match p.shape {
                Shape::Rect {
                    bounds,
                    corner_radius,
                } => v.extend([
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.width.as_mm(),
                    bounds.height.as_mm(),
                    corner_radius.as_mm(),
                ]),
                Shape::Ellipse { frame } => v.extend([
                    frame.center.x,
                    frame.center.y,
                    frame.rx.as_mm(),
                    frame.ry.as_mm(),
                ]),
                Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => v.extend([
                    frame.center.x,
                    frame.center.y,
                    frame.radius.as_mm(),
                    frame.angle.as_radians(),
                ]),
            }
            v
        }
    };
    numbers.into_iter().all(ok)
}
