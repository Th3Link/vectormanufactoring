//! One document-axes map applied to one object, kind by kind
//! (`specs/0019-multi-object-transform/`, criteria 20, 26 and 41): the result a
//! scale, a rotate or a skew of a selection gives each of its objects. The group
//! gesture computes its map once and calls [`apply`] for every object, in the
//! preview and on release (`adrs.md` decision 1), so the two cannot disagree.
//! It is a function of one object and one map, so a later group object can call
//! it with its own frame (`adrs.md`, forward note).

use curvyo_document_core::{
    Angle, Corner, Document, EllipseFrame, Length, ObjectEditError, ObjectSnapshot, Point,
    PrimitiveSnapshot, RectBounds, Shape, StarFrame,
};

use crate::group_box::GEOMETRIC_TOLERANCE_MM;
use crate::oriented_box::OrientedBox;
use crate::skew_math::{skew_angle, skew_factor, skew_frame};
use crate::transform_commit::is_sane;
use crate::transform_drag::{CornerRadiusScaling, ScaleModes, StrokeScaling, scale_stroke};
use crate::transform_handle_layout::EditHandle;
use crate::transform_math::{
    resize_anchor_local_position, resize_local_box, rotate_delta_angle, rotate_pivot,
    scaled_and_floored, stroke_or_radius_factor,
};

/// Two scale factors closer than this are one factor.
const UNIFORM_EPSILON: f64 = 1e-12;

/// What a group gesture does to every selected object, in the document axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum GroupMap {
    /// A scale by `(sx, sy)` about `pivot`.
    Scale { pivot: Point, sx: f64, sy: f64 },
    /// A turn by `delta` about `pivot`.
    Rotate { pivot: Point, delta: Angle },
    /// A shear about the line through `fixed`: `x += ku * y`, `y += kv * x`, both
    /// measured from `fixed`.
    Shear { fixed: Point, ku: f64, kv: f64 },
}

/// `object` after `map`; `modes` decide whether a scale also scales the stroke
/// width and the corner radii. The kind never changes. A skew leaves a primitive
/// as it is (the handles are not offered for one, criterion 40).
pub(crate) fn apply(object: &ObjectSnapshot, map: &GroupMap, modes: ScaleModes) -> ObjectSnapshot {
    match *map {
        GroupMap::Scale { pivot, sx, sy } => scale_object(object, pivot, sx, sy, modes),
        GroupMap::Rotate { pivot, delta } => object.rotated(pivot, delta),
        GroupMap::Shear { fixed, ku, kv } => match object {
            ObjectSnapshot::Path(path) => {
                ObjectSnapshot::Path(path.sheared_along(fixed, ku, kv, Angle::from_radians(0.0)))
            }
            ObjectSnapshot::Primitive(_) => object.clone(),
        },
    }
}

fn scale_object(
    object: &ObjectSnapshot,
    pivot: Point,
    sx: f64,
    sy: f64,
    modes: ScaleModes,
) -> ObjectSnapshot {
    let factor = stroke_or_radius_factor(sx, sy);
    let mut scaled = match object {
        ObjectSnapshot::Path(path) => {
            ObjectSnapshot::Path(path.scaled_along(pivot, sx, sy, Angle::from_radians(0.0)))
        }
        ObjectSnapshot::Primitive(primitive) => {
            ObjectSnapshot::Primitive(scale_primitive(primitive, pivot, (sx, sy), modes.radius))
        }
    };
    if modes.stroke == StrokeScaling::Proportional {
        scale_stroke(&mut scaled, factor);
    }
    scaled
}

/// Whether `rotation` turns the local x axis onto the document y axis (an odd
/// number of quarter turns). Meaningful for an aligned primitive.
fn swaps_axes(rotation: Angle) -> bool {
    let quarters = (rotation.as_radians() / std::f64::consts::FRAC_PI_2).round();
    // A finite rotation is small enough for the cast; `rem_euclid` keeps -1 odd.
    #[allow(clippy::cast_possible_truncation)]
    let quarters = quarters as i64;
    quarters.rem_euclid(2) == 1
}

/// The multipliers of a rectangle's or ellipse's width and height (rx and ry),
/// and the rotation it ends with (criterion 20, second and third rows).
fn extent_factors(primitive: &PrimitiveSnapshot, (sx, sy): (f64, f64)) -> ((f64, f64), Angle) {
    let rotation = primitive.rotation;
    if (sx - sy).abs() <= UNIFORM_EPSILON {
        return ((sx, sx), rotation);
    }
    if let Shape::Ellipse { frame } = primitive.shape
        && (frame.rx.as_mm() - frame.ry.as_mm()).abs() < GEOMETRIC_TOLERANCE_MM
    {
        // A circle becomes an ellipse along the document axes, whatever its
        // rotation: the one write beyond a single object's resize (criterion 31).
        return ((sx, sy), Angle::from_radians(0.0));
    }
    if swaps_axes(rotation) {
        ((sy, sx), rotation)
    } else {
        ((sx, sy), rotation)
    }
}

fn scale_primitive(
    primitive: &PrimitiveSnapshot,
    pivot: Point,
    (sx, sy): (f64, f64),
    radius_scaling: CornerRadiusScaling,
) -> PrimitiveSnapshot {
    let map_point = |c: Point| {
        Point::new(
            pivot.x + (c.x - pivot.x) * sx,
            pivot.y + (c.y - pivot.y) * sy,
        )
    };
    let factor = stroke_or_radius_factor(sx, sy);
    let ((kx, ky), rotation) = extent_factors(primitive, (sx, sy));
    let shape = match primitive.shape {
        Shape::Rect {
            bounds,
            corner_radii,
        } => {
            let centre = map_point(Point::new(
                bounds.origin.x + bounds.width.as_mm() / 2.0,
                bounds.origin.y + bounds.height.as_mm() / 2.0,
            ));
            let (width, height) = (bounds.width.as_mm() * kx, bounds.height.as_mm() * ky);
            let radii = match radius_scaling {
                CornerRadiusScaling::Keep => corner_radii,
                CornerRadiusScaling::Proportional => {
                    Corner::ALL.iter().fold(corner_radii, |scaled, &corner| {
                        scaled.with(
                            corner,
                            scaled_and_floored(
                                corner_radii.get(corner),
                                factor,
                                Length::from_mm(0.0),
                            ),
                        )
                    })
                }
            };
            Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(centre.x - width / 2.0, centre.y - height / 2.0),
                    width: Length::from_mm(width),
                    height: Length::from_mm(height),
                },
                corner_radii: radii,
            }
        }
        Shape::Ellipse { frame } => Shape::Ellipse {
            frame: EllipseFrame {
                center: map_point(frame.center),
                rx: Length::from_mm(frame.rx.as_mm() * kx),
                ry: Length::from_mm(frame.ry.as_mm() * ky),
            },
        },
        Shape::Polygon { frame, point_count } => Shape::Polygon {
            frame: scaled_star_frame(frame, map_point(frame.center), factor),
            point_count,
        },
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => Shape::Star {
            frame: scaled_star_frame(frame, map_point(frame.center), factor),
            point_count,
            inner_ratio,
        },
    };
    PrimitiveSnapshot {
        shape,
        rotation,
        ..primitive.clone()
    }
}

/// `frame` with its centre moved to `centre` and its outer radius times `factor`;
/// the angle and the inner ratio (a star's) are unchanged.
fn scaled_star_frame(frame: StarFrame, centre: Point, factor: f64) -> StarFrame {
    StarFrame {
        center: centre,
        radius: Length::from_mm(frame.radius.as_mm() * factor),
        angle: frame.angle,
    }
}

/// The map of a group gesture on `handle` of `start_box`, with the pointer going
/// from `down_at` to `current` (a pair) and the `(shift, ctrl)` modifiers.
pub(crate) fn group_map(
    start_box: &OrientedBox,
    handle: EditHandle,
    (down_at, current): (Point, Point),
    (shift, ctrl): (bool, bool),
    uniform_only: bool,
) -> Option<GroupMap> {
    match handle {
        EditHandle::Resize(direction) => {
            let delta = down_at.vector_to(current);
            // `f64::max` and `clamp` swallow a NaN, so a non-finite pointer is refused first.
            if !(delta.x.is_finite() && delta.y.is_finite()) {
                return None;
            }
            let resized = resize_local_box(
                start_box.min,
                start_box.max,
                direction,
                delta,
                shift,
                ctrl || uniform_only,
            );
            let anchor =
                resize_anchor_local_position(start_box.min, start_box.max, direction, shift);
            Some(GroupMap::Scale {
                pivot: start_box.to_document(anchor),
                sx: resized.sx,
                sy: resized.sy,
            })
        }
        EditHandle::Rotate(direction) => {
            let pivot = rotate_pivot(start_box, direction, shift);
            Some(GroupMap::Rotate {
                pivot,
                delta: rotate_delta_angle(pivot, down_at, current, ctrl),
            })
        }
        EditHandle::Skew(side) => {
            let frame = skew_frame(start_box, side, shift);
            let k = skew_factor(
                &frame,
                skew_angle(start_box, side, down_at, current, shift, ctrl),
            );
            if k.abs() <= 0.0 {
                return None;
            }
            let (ku, kv) = if frame.along_u { (k, 0.0) } else { (0.0, k) };
            Some(GroupMap::Shear {
                fixed: frame.fixed_point,
                ku,
                kv,
            })
        }
        EditHandle::Move | EditHandle::Param(_) => None,
    }
}

/// `starts` after `map`, or `None` if any result is not finite and sane
/// (criterion 22: all or nothing).
pub(crate) fn map_all_checked(
    starts: &[ObjectSnapshot],
    map: &GroupMap,
    modes: ScaleModes,
) -> Option<Vec<ObjectSnapshot>> {
    let mapped: Vec<ObjectSnapshot> = starts
        .iter()
        .map(|object| apply(object, map, modes))
        .collect();
    mapped.iter().all(is_sane).then_some(mapped)
}

/// [`map_all_checked`], or `starts` unchanged if a result is not sane.
pub(crate) fn map_all(
    starts: &[ObjectSnapshot],
    map: &GroupMap,
    modes: ScaleModes,
) -> Vec<ObjectSnapshot> {
    map_all_checked(starts, map, modes).unwrap_or_else(|| starts.to_vec())
}

/// The point a group gesture on `handle` holds fixed (the pivot marker): the
/// opposite corner or side, the centre under Shift for a resize; the rotate
/// pivot; the fixed line's point for a skew. `None` for the centre handle.
pub(crate) fn group_pivot(box_: &OrientedBox, handle: EditHandle, shift: bool) -> Option<Point> {
    match handle {
        EditHandle::Resize(direction) => Some(box_.to_document(resize_anchor_local_position(
            box_.min, box_.max, direction, shift,
        ))),
        EditHandle::Rotate(direction) => Some(rotate_pivot(box_, direction, shift)),
        EditHandle::Skew(side) => Some(skew_frame(box_, side, shift).fixed_point),
        EditHandle::Move | EditHandle::Param(_) => None,
    }
}

/// Writes a resolved group gesture with `transform_objects`; a stroke width the
/// document refuses is left out and the geometry still lands.
pub(crate) fn commit_group(document: &Document, results: &[ObjectSnapshot], stroke: StrokeScaling) {
    let with_width = stroke == StrokeScaling::Proportional;
    let outcome = document.transform_objects(results, with_width);
    if with_width && outcome == Err(ObjectEditError::InvalidStrokeWidth) {
        // Anything else means an object is gone: nothing is left to write.
        let _ = document.transform_objects(results, false);
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{
        AnchorId, CornerRadii, Document, InnerRatio, NewAnchor, NodeId, PointCount,
    };

    use super::*;

    fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> ObjectSnapshot {
        let id = document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        });
        document.object(id).expect("exists")
    }

    fn turned(document: &Document, id: NodeId, degrees: f64) -> ObjectSnapshot {
        let object = document.object(id).expect("exists");
        let rotated = object.rotated(
            Point::new(0.0, 0.0),
            Angle::from_radians(degrees.to_radians()),
        );
        document.rotate_object(&rotated).expect("rotates");
        document.object(id).expect("exists")
    }

    fn scale(sx: f64, sy: f64) -> GroupMap {
        GroupMap::Scale {
            pivot: Point::new(0.0, 0.0),
            sx,
            sy,
        }
    }

    fn bounds_of(object: &ObjectSnapshot) -> RectBounds {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { bounds, .. },
            ..
        }) = object
        else {
            panic!("a rectangle");
        };
        *bounds
    }

    /// Criterion 18, worked example: a 10 x 10 rectangle at (20, 0) scaled by
    /// (2, 3) about the origin is (40, 0) to (60, 30).
    #[test]
    fn a_rectangle_is_stretched_along_the_document_axes() {
        let document = Document::new(1);
        let object = rect(&document, 20.0, 0.0, 10.0, 10.0);
        let result = apply(&object, &scale(2.0, 3.0), ScaleModes::default());
        let b = bounds_of(&result);
        assert_eq!((b.origin.x, b.origin.y), (40.0, 0.0));
        assert_eq!((b.width.as_mm(), b.height.as_mm()), (20.0, 30.0));
        assert_eq!(result.rotation(), object.rotation());
    }

    /// Criterion 20: an aligned rectangle turned 90 degrees swaps the factors
    /// onto its width and height; its rotation stays.
    #[test]
    fn a_quarter_turned_rectangle_takes_the_factors_swapped() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(-10.0, -5.0),
            width: Length::from_mm(20.0),
            height: Length::from_mm(10.0),
        });
        let object = turned(&document, id, 90.0);
        let result = apply(&object, &scale(2.0, 3.0), ScaleModes::default());
        let b = bounds_of(&result);
        assert!((b.width.as_mm() - 60.0).abs() < 1e-9, "width times sy");
        assert!((b.height.as_mm() - 20.0).abs() < 1e-9, "height times sx");
        assert!((result.rotation().as_radians() - object.rotation().as_radians()).abs() < 1e-12);
    }

    /// Criterion 20 and D3: a circle stretched becomes an ellipse along the
    /// document axes with rotation 0, whatever its rotation was.
    #[test]
    fn a_stretched_circle_becomes_an_ellipse_with_rotation_zero() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(10.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let object = turned(&document, id, 30.0);
        let result = apply(&object, &scale(2.0, 3.0), ScaleModes::default());
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Ellipse { frame },
            rotation,
            ..
        }) = result
        else {
            panic!("an ellipse");
        };
        assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9 && (frame.ry.as_mm() - 15.0).abs() < 1e-9);
        assert_eq!(rotation.as_radians(), 0.0);
        // The same factor twice keeps the rotation: a circle stays a circle.
        let kept = apply(&object, &scale(2.0, 2.0), ScaleModes::default());
        assert_eq!(kept.rotation(), object.rotation());
    }

    /// Criterion 20: a polygon or star scales its outer radius by the one factor
    /// and moves its centre; count, ratio and angle are unchanged.
    #[test]
    fn a_star_scales_its_outer_radius() {
        let document = Document::new(1);
        let id = document.create_star(
            StarFrame {
                center: Point::new(10.0, 10.0),
                radius: Length::from_mm(4.0),
                angle: Angle::from_radians(0.5),
            },
            PointCount::new(5).expect("count"),
            InnerRatio::new(0.4).expect("ratio"),
        );
        let object = document.object(id).expect("exists");
        let result = apply(&object, &scale(2.0, 2.0), ScaleModes::default());
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape:
                Shape::Star {
                    frame,
                    point_count,
                    inner_ratio,
                },
            ..
        }) = result
        else {
            panic!("a star");
        };
        assert_eq!((frame.center.x, frame.center.y), (20.0, 20.0));
        assert_eq!(frame.radius.as_mm(), 8.0);
        assert_eq!(frame.angle.as_radians(), 0.5);
        assert_eq!(point_count.get(), 5);
        assert!((inner_ratio.get() - 0.4).abs() < 1e-12);
    }

    /// Criterion 24: the stroke width and the corner radii scale by the square
    /// root of the factors only when their switch is on.
    #[test]
    fn the_switches_decide_stroke_width_and_corner_radius() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(2.0))
            .expect("radius");
        let mut object = document.object(id).expect("exists");
        object.style_mut().stroke.width = Length::from_mm(1.0);
        let off = apply(&object, &scale(4.0, 1.0), ScaleModes::default());
        assert_eq!(off.style().stroke.width.as_mm(), 1.0);
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        }) = &off
        else {
            panic!("a rectangle");
        };
        assert_eq!(*corner_radii, CornerRadii::uniform(Length::from_mm(2.0)));
        let on = apply(
            &object,
            &scale(4.0, 1.0),
            ScaleModes {
                stroke: StrokeScaling::Proportional,
                radius: CornerRadiusScaling::Proportional,
            },
        );
        assert!((on.style().stroke.width.as_mm() - 2.0).abs() < 1e-9);
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        }) = &on
        else {
            panic!("a rectangle");
        };
        assert!((corner_radii.tl.as_mm() - 4.0).abs() < 1e-9);
    }

    /// Criterion 20: a path's anchors map and its handle vectors scale in the
    /// document axes, whatever the path's own rotation; `rotation` is unchanged.
    #[test]
    fn a_path_scales_in_the_document_axes_whatever_its_rotation() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(1.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(2.0, 1.0)),
            ],
            false,
        );
        let object = turned(&document, id, 40.0);
        let ObjectSnapshot::Path(before) = &object else {
            panic!("a path");
        };
        let ObjectSnapshot::Path(after) = apply(&object, &scale(2.0, 3.0), ScaleModes::default())
        else {
            panic!("a path");
        };
        for (a, b) in before.anchors.iter().zip(&after.anchors) {
            assert!((b.point.x - a.point.x * 2.0).abs() < 1e-9);
            assert!((b.point.y - a.point.y * 3.0).abs() < 1e-9);
        }
        assert_eq!(after.rotation, before.rotation);
    }

    /// Criterion 26: a rotate turns every kind about the pivot and advances its
    /// `rotation`; turning back restores the geometry.
    #[test]
    fn rotating_and_rotating_back_restores_the_object() {
        let document = Document::new(1);
        let object = rect(&document, 20.0, 5.0, 10.0, 10.0);
        let pivot = Point::new(7.0, 3.0);
        let there = apply(
            &object,
            &GroupMap::Rotate {
                pivot,
                delta: Angle::from_radians(0.7),
            },
            ScaleModes::default(),
        );
        assert!((there.rotation().as_radians() - 0.7).abs() < 1e-12);
        let back = apply(
            &there,
            &GroupMap::Rotate {
                pivot,
                delta: Angle::from_radians(-0.7),
            },
            ScaleModes::default(),
        );
        let (a, b) = (bounds_of(&object), bounds_of(&back));
        assert!((a.origin.x - b.origin.x).abs() < 1e-9 && (a.origin.y - b.origin.y).abs() < 1e-9);
        assert!(back.rotation().as_radians().abs() < 1e-12);
    }

    /// Criterion 41, worked example: the top handle of two paths with the bottom
    /// side fixed, 45 degrees: every point at y = 0 moves 10 mm in x, every point
    /// at y = 10 stays.
    #[test]
    fn a_shear_moves_points_in_proportion_to_their_distance_from_the_fixed_line() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 10.0)),
            ],
            false,
        );
        let object = document.object(id).expect("exists");
        // The bottom side (y = 10) is fixed; the top moves: ku = -tan(45) for a
        // point above the line moving right means x += ku * (y - 10).
        let result = apply(
            &object,
            &GroupMap::Shear {
                fixed: Point::new(5.0, 10.0),
                ku: -1.0,
                kv: 0.0,
            },
            ScaleModes::default(),
        );
        let ObjectSnapshot::Path(path) = result else {
            panic!("a path");
        };
        assert!(
            (path.anchors[0].point.x - 10.0).abs() < 1e-9,
            "top moved 10"
        );
        assert!(
            (path.anchors[1].point.x - 10.0).abs() < 1e-9,
            "bottom stays"
        );
    }
}
