//! Resizes a primitive's frame by a handle drag or a typed size and keeps the
//! resize's anchor fixed in document space
//! (`specs/0005-object-transform/adrs.md`: "a resize rewrites geometry").
//! Split out of [`crate::transform_drag`], which resolves the gestures.

use vecmanf_document_core::{
    Angle, EllipseFrame, Length, Point, PrimitiveSnapshot, RectBounds, Shape, StarFrame, Vec2,
    shape_center, translate_shape,
};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;
use crate::transform_drag::CornerRadiusScaling;
use crate::transform_math::{
    polygon_star_resize_factor, resize_anchor_local_position, resize_local_box, scaled_and_floored,
    stroke_or_radius_factor,
};

/// A primitive's resized frame and the stroke/radius factor that goes
/// with it. Rectangles and ellipses resize their local box (the corner
/// radius scales with the factor only under [`CornerRadiusScaling::Proportional`],
/// acceptance criterion 9 of slice 5, criterion 23 of `unified-object-editing`); a polygon or
/// star is always a uniform outer-radius scale (criterion 11).
pub(crate) fn resize_primitive(
    primitive: &PrimitiveSnapshot,
    start_box: &OrientedBox,
    direction: ResizeDirection,
    local_delta: Vec2,
    modifiers: (bool, bool),
    radius_scaling: CornerRadiusScaling,
) -> (PrimitiveSnapshot, f64) {
    let (shift, ctrl) = modifiers;
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
            // `Keep` hands the stored radius back untouched, so the register
            // is not rewritten and a concurrent radius edit is not beaten.
            let radius = match radius_scaling {
                CornerRadiusScaling::Keep => corner_radius,
                CornerRadiusScaling::Proportional => {
                    scaled_and_floored(corner_radius, factor, Length::from_mm(0.0))
                }
            };
            let shape = Shape::Rect {
                bounds: RectBounds {
                    origin: resized.min,
                    width: Length::from_mm(resized.max.x - resized.min.x),
                    height: Length::from_mm(resized.max.y - resized.min.y),
                },
                corner_radius: radius,
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

pub(crate) fn scaled_star_frame(frame: StarFrame, factor: f64) -> StarFrame {
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
