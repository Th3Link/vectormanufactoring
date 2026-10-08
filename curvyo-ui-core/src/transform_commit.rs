//! Writing a resolved gesture to the document, and refusing a result that is
//! not finite and sane (`specs/0005-object-transform/adrs.md`; extended by
//! `specs/object-transform-refinements/adrs.md`): one commit per drag or
//! confirmed entry, dispatched on the gesture's handle. Split out of
//! [`crate::transform_drag`], which resolves the gestures.

use curvyo_document_core::{
    AnchorId, CopySource, Document, Length, NodeId, ObjectSnapshot, Point, Shape, Vec2,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::param_edit::commit_param;
use crate::transform_drag::{ScaleModes, StrokeScaling};
use crate::transform_handle_layout::EditHandle;

/// The largest coordinate or size (millimetres, 10 km) a drag may write.
/// A pointer value beyond it — or NaN/infinite — is hostile or broken
/// input; the drag then resolves to "no change" instead of writing
/// geometry a later open would refuse (`adrs.md`: "a file must never
/// become unopenable from a drag").
pub(crate) const MAX_COORDINATE_MM: f64 = 1e7;

/// A move offset within this (millimetres) of zero is no move.
pub(crate) const MOVE_EQUAL_EPSILON_MM: f64 = 1e-9;

/// Writes a gesture's resulting snapshot (shared by a drag's release and a
/// typed entry's Enter).
pub(crate) fn commit_gesture(
    document: &Document,
    handle: EditHandle,
    result: &ObjectSnapshot,
    modes: ScaleModes,
) {
    match handle {
        EditHandle::Resize(_) => commit_resize(document, result.id(), result, modes.stroke),
        EditHandle::Rotate(_) => {
            let _ = document.rotate_object(result);
        }
        EditHandle::Skew(_) => {
            commit_resize(document, result.id(), result, StrokeScaling::Keep);
        }
        EditHandle::Param(param) => commit_param(document, param, result),
        EditHandle::Move => {}
    }
}

/// Writes a move of `ids` by `offset` as one commit, or, with `copy`, one copy
/// of each of them displaced by `offset` and the originals untouched (shared
/// by a drag's release, a Ctrl release and the typed move's Enter, so every
/// route leaves the same registers). A stale id refuses the whole call, which
/// writes nothing. Returns the copies' ids in `ids` order after a copy, `None`
/// after a move or a refusal: the caller selects them.
pub(crate) fn commit_move(
    document: &Document,
    ids: &[NodeId],
    offset: Vec2,
    copy: bool,
    minter: &mut AnchorIdMinter,
) -> Option<Vec<NodeId>> {
    if !copy {
        let _ = document.translate_objects(ids, offset);
        return None;
    }
    let sources: Option<Vec<CopySource>> = ids
        .iter()
        .map(|&id| {
            let anchor_count = match document.object(id)? {
                ObjectSnapshot::Path(path) => path.anchors.len(),
                ObjectSnapshot::Primitive(_) => 0,
            };
            Some(CopySource {
                id,
                anchor_ids: (0..anchor_count).map(|_| minter.mint()).collect(),
            })
        })
        .collect();
    document.duplicate_objects(&sources?, offset).ok()
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
                corner_radii,
            } => {
                let _ = document.resize_rect(
                    id,
                    bounds,
                    corner_radii,
                    width(primitive.style.stroke.width),
                );
            }
            Shape::Ellipse { frame } => {
                let _ = document.resize_ellipse(id, frame, width(primitive.style.stroke.width));
            }
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => {
                let _ = document.resize_star_frame(id, frame, width(primitive.style.stroke.width));
            }
        },
        ObjectSnapshot::Path(path) => {
            let anchors: Vec<(AnchorId, Point, Vec2, Vec2)> = path
                .anchors
                .iter()
                .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
                .collect();
            let _ = document.resize_path(id, &anchors, width(path.style.stroke.width));
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

pub(crate) fn is_sane(object: &ObjectSnapshot) -> bool {
    let ok = |v: f64| v.is_finite() && v.abs() <= MAX_COORDINATE_MM;
    let (numbers, rotation) = numbers_of(object);
    ok(rotation) && numbers.into_iter().all(ok)
}

/// Two snapshots of one object are the same within 1e-9 mm and 1e-12 rad: a
/// resolved edit that equals the committed object is no edit at all, so it
/// shows no preview and writes nothing (criterion 12 of
/// `specs/unified-object-editing/`).
pub(crate) fn same_within_tolerance(a: &ObjectSnapshot, b: &ObjectSnapshot) -> bool {
    let ((xs, ra), (ys, rb)) = (numbers_of(a), numbers_of(b));
    xs.len() == ys.len()
        && (ra - rb).abs() <= SAME_ANGLE_EPSILON_RAD
        && xs
            .iter()
            .zip(&ys)
            .all(|(x, y)| (x - y).abs() <= SAME_LENGTH_EPSILON_MM)
}

/// See [`same_within_tolerance`].
const SAME_LENGTH_EPSILON_MM: f64 = 1e-9;
/// See [`same_within_tolerance`].
const SAME_ANGLE_EPSILON_RAD: f64 = 1e-12;

/// Every length-like number of `object` (stroke width included) in a fixed
/// order, and its rotation in radians.
fn numbers_of(object: &ObjectSnapshot) -> (Vec<f64>, f64) {
    match object {
        ObjectSnapshot::Path(path) => {
            let mut v = vec![path.style.stroke.width.as_mm()];
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
            (v, path.rotation.as_radians())
        }
        ObjectSnapshot::Primitive(p) => {
            let mut v = vec![p.style.stroke.width.as_mm()];
            match p.shape {
                Shape::Rect {
                    bounds,
                    corner_radii,
                } => v.extend([
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.width.as_mm(),
                    bounds.height.as_mm(),
                    corner_radii.tl.as_mm(),
                    corner_radii.tr.as_mm(),
                    corner_radii.br.as_mm(),
                    corner_radii.bl.as_mm(),
                ]),
                Shape::Ellipse { frame } => v.extend([
                    frame.center.x,
                    frame.center.y,
                    frame.rx.as_mm(),
                    frame.ry.as_mm(),
                ]),
                Shape::Polygon { frame, point_count } => v.extend([
                    frame.center.x,
                    frame.center.y,
                    frame.radius.as_mm(),
                    frame.angle.as_radians(),
                    f64::from(point_count.get()),
                ]),
                Shape::Star {
                    frame,
                    point_count,
                    inner_ratio,
                } => v.extend([
                    frame.center.x,
                    frame.center.y,
                    frame.radius.as_mm(),
                    frame.angle.as_radians(),
                    f64::from(point_count.get()),
                    inner_ratio.get(),
                ]),
            }
            (v, p.rotation.as_radians())
        }
    }
}
