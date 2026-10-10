//! Writing a resolved gesture to the document, and refusing a result that is
//! not finite and sane (`specs/0005-object-transform/adrs.md`; extended by
//! `specs/0008-object-transform-refinements/adrs.md`): one commit per drag or
//! confirmed entry, dispatched on the gesture's handle. Split out of
//! [`crate::transform_drag`], which resolves the gestures.

use curvyo_document_core::{
    AnchorId, CopySource, Document, Length, NodeId, ObjectSnapshot, PathEditError, Point, Shape,
    ShapeEditError, Vec2,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::param_edit::commit_param;
use crate::transform_drag::{ScaleModes, StrokeScaling};
use crate::transform_handle_layout::EditHandle;

/// The largest coordinate or size (millimetres, 10 km) a drag may write: the bound of the boolean
/// kernel, so that a file never becomes one the kernel refuses because of a drag. A pointer value
/// beyond it, or NaN or infinite, is hostile or broken input; the drag then resolves to "no
/// change" instead of writing geometry a later open would refuse (`adrs.md`: "a file must never
/// become unopenable from a drag").
pub(crate) use curvyo_geometry_core::MAX_COORDINATE_MM;

/// A move offset within this (millimetres) of zero is no move.
pub(crate) const MOVE_EQUAL_EPSILON_MM: f64 = 1e-9;

/// Whether moving the span `low..=high` by `delta` (millimetres, one axis) keeps both edges within
/// [`MAX_COORDINATE_MM`]: the one limit of a typed move and a nudge.
pub(crate) fn axis_within_limit(low: f64, high: f64, delta: f64) -> bool {
    [low + delta, high + delta]
        .iter()
        .all(|edge| edge.abs() <= MAX_COORDINATE_MM)
}

/// Whether moving objects whose tight bounds are `bounds` (`(min, max)` corners) by `offset`
/// keeps every edge within [`MAX_COORDINATE_MM`].
pub(crate) fn offset_within_limit(bounds: (Point, Point), offset: Vec2) -> bool {
    let (min, max) = bounds;
    axis_within_limit(min.x, max.x, offset.x) && axis_within_limit(min.y, max.y, offset.y)
}

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
        // A refusal here means the object is gone (see `commit_resize`).
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
                ObjectSnapshot::Path(path) => path.all_anchors().count(),
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
///
/// A width the document refuses (not finite or not above zero: the command
/// refuses before writing anything) does not cost the maker the geometry
/// resize: the write is repeated without a width, so the shape resizes and the
/// stored width stays as it was. Any other refusal means the object is gone
/// (a peer deleted it, or it changed kind since the drag began): there is
/// nothing left to write, and the selection catches up on its next lazy
/// resolve (ADR 0009 section 2).
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
            } => write_with_width_fallback(
                width(primitive.style.stroke.width),
                ShapeEditError::InvalidStrokeWidth,
                |w| document.resize_rect(id, bounds, corner_radii, w),
            ),
            Shape::Ellipse { frame } => write_with_width_fallback(
                width(primitive.style.stroke.width),
                ShapeEditError::InvalidStrokeWidth,
                |w| document.resize_ellipse(id, frame, w),
            ),
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => write_with_width_fallback(
                width(primitive.style.stroke.width),
                ShapeEditError::InvalidStrokeWidth,
                |w| document.resize_star_frame(id, frame, w),
            ),
        },
        ObjectSnapshot::Path(path) => {
            let anchors: Vec<(AnchorId, Point, Vec2, Vec2)> = path
                .all_anchors()
                .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
                .collect();
            write_with_width_fallback(
                width(path.style.stroke.width),
                PathEditError::InvalidStrokeWidth,
                |w| document.resize_path(id, &anchors, w),
            );
        }
    }
}

/// Runs `write` with `width`; if the document refused that width, runs it once
/// more with no width. Generic over the error type because the primitive and
/// the path commands return different ones.
fn write_with_width_fallback<E: PartialEq>(
    width: Option<Length>,
    invalid_width: E,
    write: impl Fn(Option<Length>) -> Result<(), E>,
) {
    let refused = width.is_some() && write(width).err() == Some(invalid_width);
    if refused || width.is_none() {
        // The geometry still resizes; the stored width stays.
        let _ = write(None);
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
/// `specs/0009-unified-object-editing/`).
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
            for a in path.all_anchors() {
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

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Length, NewAnchor, RectBounds};

    use super::*;

    fn rect(document: &Document) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    }

    fn resized_rect_with_width(document: &Document, id: NodeId, width: f64) -> ObjectSnapshot {
        let Some(ObjectSnapshot::Primitive(mut primitive)) = document.object(id) else {
            panic!("a primitive");
        };
        if let Shape::Rect { bounds, .. } = &mut primitive.shape {
            bounds.width = Length::from_mm(30.0);
        }
        primitive.style.stroke.width = Length::from_mm(width);
        ObjectSnapshot::Primitive(primitive)
    }

    /// A width the document refuses must not cost the geometry: the shape
    /// resizes and the stored width stays.
    #[test]
    fn a_refused_stroke_width_still_resizes_the_shape_and_keeps_the_stored_width() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let document = Document::new(1);
            let id = rect(&document);
            let result = resized_rect_with_width(&document, id, bad);
            commit_resize(&document, id, &result, StrokeScaling::Proportional);
            let Some(ObjectSnapshot::Primitive(after)) = document.object(id) else {
                panic!("a primitive");
            };
            let Shape::Rect { bounds, .. } = after.shape else {
                panic!("a rectangle");
            };
            assert_eq!(bounds.width, Length::from_mm(30.0), "width {bad}");
            assert_eq!(
                after.style.stroke.width,
                Length::from_mm(0.25),
                "width {bad}"
            );
        }
    }

    #[test]
    fn a_good_width_is_written_with_the_geometry() {
        let document = Document::new(1);
        let id = rect(&document);
        let result = resized_rect_with_width(&document, id, 0.5);
        commit_resize(&document, id, &result, StrokeScaling::Proportional);
        let Some(ObjectSnapshot::Primitive(after)) = document.object(id) else {
            panic!("a primitive");
        };
        assert_eq!(after.style.stroke.width, Length::from_mm(0.5));
        // With "Scale stroke width" off the width is never passed.
        let result = resized_rect_with_width(&document, id, 0.9);
        commit_resize(&document, id, &result, StrokeScaling::Keep);
        let Some(ObjectSnapshot::Primitive(after)) = document.object(id) else {
            panic!("a primitive");
        };
        assert_eq!(after.style.stroke.width, Length::from_mm(0.5));
    }

    #[test]
    fn a_path_with_a_refused_width_still_moves_its_anchors() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            ],
            false,
        );
        let Some(ObjectSnapshot::Path(mut path)) = document.object(id) else {
            panic!("a path");
        };
        path.anchors[1].point = Point::new(30.0, 0.0);
        path.style.stroke.width = Length::from_mm(0.0);
        commit_resize(
            &document,
            id,
            &ObjectSnapshot::Path(path),
            StrokeScaling::Proportional,
        );
        let after = document.path(id).unwrap();
        assert_eq!(after.anchors[1].point, Point::new(30.0, 0.0));
        assert_eq!(after.style.stroke.width, Length::from_mm(0.25));
    }

    #[test]
    fn a_resize_of_an_object_that_is_gone_writes_nothing_and_does_not_panic() {
        let document = Document::new(1);
        let id = rect(&document);
        let result = resized_rect_with_width(&document, id, 0.5);
        document.delete_objects(&[id]).unwrap();
        commit_resize(&document, id, &result, StrokeScaling::Proportional);
        assert!(document.object(id).is_none());
    }
}
