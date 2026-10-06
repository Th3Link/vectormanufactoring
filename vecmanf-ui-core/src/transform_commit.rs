//! Writing a resolved gesture to the document, and refusing a result that is
//! not finite and sane (`specs/0005-object-transform/adrs.md`; extended by
//! `specs/object-transform-refinements/adrs.md`): one commit per drag or
//! confirmed entry, dispatched on the gesture's handle. Split out of
//! [`crate::transform_drag`], which resolves the gestures.

use vecmanf_document_core::{
    AnchorId, Document, Length, NodeId, ObjectSnapshot, Point, Shape, Vec2,
};

use crate::transform_drag::StrokeScaling;
use crate::transform_handle_layout::TransformHandle;

/// The largest coordinate or size (millimetres, 10 km) a drag may write.
/// A pointer value beyond it — or NaN/infinite — is hostile or broken
/// input; the drag then resolves to "no change" instead of writing
/// geometry a later open would refuse (`adrs.md`: "a file must never
/// become unopenable from a drag").
pub(crate) const MAX_COORDINATE_MM: f64 = 1e7;

/// Writes a gesture's resulting snapshot (shared by a drag's release and a
/// typed entry's Enter).
pub(crate) fn commit_gesture(
    document: &Document,
    handle: TransformHandle,
    result: &ObjectSnapshot,
    stroke_scaling: StrokeScaling,
) {
    match handle {
        TransformHandle::Resize(_) => commit_resize(document, result.id(), result, stroke_scaling),
        TransformHandle::Rotate(_) => {
            let _ = document.rotate_object(result);
        }
        TransformHandle::Skew(_) => {
            commit_resize(document, result.id(), result, StrokeScaling::Keep);
        }
        TransformHandle::Move => {}
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
pub(crate) fn sane_or(start: &ObjectSnapshot, resolved: ObjectSnapshot) -> ObjectSnapshot {
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
