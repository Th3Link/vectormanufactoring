//! The one value rule of a primitive's own parameters: a pointer drag, a
//! typed entry and a bar control each turn their input into a
//! [`ParamValue`], and [`apply_param`] turns that into the resulting
//! snapshot, clamps included, so the three can never disagree
//! (`specs/unified-object-editing/adrs.md`, "one resolving function per
//! parameter"). Writing the result is [`commit_param`] and
//! [`commit_param_batch`], over the `Document` commands the shape tools
//! already used.

use vecmanf_document_core::{
    Document, InnerRatio, Length, NodeId, ObjectSnapshot, PointCount, PrimitiveSnapshot, Shape,
    ShapeEditError, Vec2, effective_corner_radius,
};

use crate::param_handles::ParamHandle;

/// A value within this (millimetres, or a ratio) of the current one is "equal":
/// nothing is written (criteria 9, 12, 18).
pub(crate) const PARAM_EQUAL_EPSILON: f64 = 1e-9;

/// The inner ratio's limits (criterion 3): the star bar's own bounds, strictly
/// inside [`InnerRatio`]'s open interval.
pub const MIN_INNER_RATIO: f64 = 0.01;
/// See [`MIN_INNER_RATIO`].
pub const MAX_INNER_RATIO: f64 = 0.99;

/// A primitive parameter and its new value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    /// A rectangle's corner radius (all four corners follow).
    Radius(Length),
    /// A star's inner/outer ratio.
    Ratio(InnerRatio),
    /// A polygon or star's point count.
    PointCount(PointCount),
}

/// The ratio `value`, limited to `MIN_INNER_RATIO..=MAX_INNER_RATIO`; a NaN
/// becomes the lower limit.
///
/// # Panics
/// Never: the limits lie strictly inside `(0, 1)`, the interval
/// [`InnerRatio`] accepts.
#[must_use]
pub fn clamped_ratio(value: f64) -> InnerRatio {
    let limited = if value.is_nan() {
        MIN_INNER_RATIO
    } else {
        value.clamp(MIN_INNER_RATIO, MAX_INNER_RATIO)
    };
    // invariant: the limits `0.01..=0.99` lie strictly inside `(0, 1)`.
    #[allow(clippy::unwrap_used)]
    InnerRatio::new(limited).unwrap()
}

/// The pointer displacement `local_delta` (in the primitive's own frame)
/// of a drag of `handle` turned into the value it asks for, from the
/// primitive `start` as it was at the press. `gain` is the radius gain
/// ([`crate::radius_gain`]) frozen at the press; the inner ratio uses one
/// pixel per pixel. `None` for a handle the primitive does not have or a
/// non-finite input.
#[must_use]
pub fn value_from_pointer(
    start: &ObjectSnapshot,
    handle: ParamHandle,
    local_delta: Vec2,
    gain: f64,
) -> Option<ParamValue> {
    if !(local_delta.x.is_finite() && local_delta.y.is_finite() && gain.is_finite()) {
        return None;
    }
    let ObjectSnapshot::Primitive(primitive) = start else {
        return None;
    };
    match (handle, primitive.shape) {
        (
            ParamHandle::CornerRadius(corner),
            Shape::Rect {
                bounds,
                corner_radius,
            },
        ) => {
            let diagonal = corner.inward_diagonal();
            let projected = local_delta.x * diagonal.x + local_delta.y * diagonal.y;
            let effective = effective_corner_radius(bounds, corner_radius).as_mm();
            Some(ParamValue::Radius(Length::from_mm(
                effective + gain * projected,
            )))
        }
        (
            ParamHandle::InnerRadius,
            Shape::Star {
                frame,
                point_count,
                inner_ratio,
            },
        ) => {
            let step = std::f64::consts::TAU / f64::from(point_count.get());
            let theta = frame.angle.as_radians() + step / 2.0;
            let projected = local_delta.x * theta.cos() + local_delta.y * theta.sin();
            let start_distance = frame.radius.as_mm() * inner_ratio.get();
            let outer = frame.radius.as_mm().max(f64::EPSILON);
            Some(ParamValue::Ratio(clamped_ratio(
                (start_distance + projected) / outer,
            )))
        }
        _ => None,
    }
}

/// `start` with `value` applied, the one rule drag, entry and bar share. A
/// radius is limited to `0..=half the shorter side`, a ratio to
/// `0.01..=0.99`. The start is returned unchanged when the limited value
/// equals the start's current effective value (within 1e-9), so a drag back
/// to its start writes nothing even where the stored radius is larger than
/// the effective one (criterion 12), and for a value the primitive has no
/// use for.
#[must_use]
pub fn apply_param(start: &ObjectSnapshot, value: ParamValue) -> ObjectSnapshot {
    let ObjectSnapshot::Primitive(primitive) = start else {
        return start.clone();
    };
    ObjectSnapshot::Primitive(apply_to_primitive(primitive, value))
}

fn apply_to_primitive(start: &PrimitiveSnapshot, value: ParamValue) -> PrimitiveSnapshot {
    let shape = match (start.shape, value) {
        (
            Shape::Rect {
                bounds,
                corner_radius,
            },
            ParamValue::Radius(radius),
        ) => {
            let half = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
            let limited = if radius.as_mm().is_finite() {
                radius.as_mm().clamp(0.0, half.max(0.0))
            } else {
                return *start;
            };
            let current = effective_corner_radius(bounds, corner_radius).as_mm();
            if (limited - current).abs() <= PARAM_EQUAL_EPSILON {
                return *start;
            }
            Shape::Rect {
                bounds,
                corner_radius: Length::from_mm(limited),
            }
        }
        (
            Shape::Star {
                frame,
                point_count,
                inner_ratio,
            },
            ParamValue::Ratio(ratio),
        ) => {
            let limited = clamped_ratio(ratio.get());
            if (limited.get() - inner_ratio.get()).abs() <= PARAM_EQUAL_EPSILON {
                return *start;
            }
            Shape::Star {
                frame,
                point_count,
                inner_ratio: limited,
            }
        }
        (Shape::Polygon { frame, point_count }, ParamValue::PointCount(count)) => {
            if count == point_count {
                return *start;
            }
            Shape::Polygon {
                frame,
                point_count: count,
            }
        }
        (
            Shape::Star {
                frame,
                point_count,
                inner_ratio,
            },
            ParamValue::PointCount(count),
        ) => {
            if count == point_count {
                return *start;
            }
            Shape::Star {
                frame,
                point_count: count,
                inner_ratio,
            }
        }
        _ => return *start,
    };
    PrimitiveSnapshot { shape, ..*start }
}

/// Writes a parameter drag's or entry's `result` (from [`apply_param`]) for
/// `handle`: one commit through the command the shape tools used, so the
/// stored fields are the same ones (criterion 24).
pub(crate) fn commit_param(document: &Document, handle: ParamHandle, result: &ObjectSnapshot) {
    let ObjectSnapshot::Primitive(primitive) = result else {
        return;
    };
    match (handle, primitive.shape) {
        (ParamHandle::CornerRadius(_), Shape::Rect { corner_radius, .. }) => {
            let _ = document.set_corner_radius(&[primitive.id], corner_radius);
        }
        (ParamHandle::InnerRadius, Shape::Star { inner_ratio, .. }) => {
            let _ = document.set_inner_ratio(&[primitive.id], inner_ratio);
        }
        _ => {}
    }
}

/// Writes `value` to every primitive in `ids` as one commit: the bar's
/// "Remove rounding", "Radius", "Points" and "Ratio" controls
/// (`specs/unified-object-editing/`, criteria 21, 21a). `ids` holds only the
/// objects the control acts on ([`crate::ids_of_kind`]).
///
/// # Errors
/// The `Document` command's error if an id no longer names an object of the
/// right kind; nothing is written then.
pub fn commit_param_batch(
    document: &Document,
    ids: &[NodeId],
    value: ParamValue,
) -> Result<(), ShapeEditError> {
    if ids.is_empty() {
        return Ok(());
    }
    match value {
        ParamValue::Radius(radius) => document.set_corner_radius(ids, radius),
        ParamValue::Ratio(ratio) => document.set_inner_ratio(ids, ratio),
        ParamValue::PointCount(count) => document.set_point_count(ids, count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param_handles::Corner;
    use vecmanf_document_core::{Angle, Point, RectBounds, StarFrame};

    fn rect(document: &Document, width: f64, height: f64, radius: f64) -> ObjectSnapshot {
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(width),
            height: Length::from_mm(height),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(radius))
            .expect("radius");
        document.object(id).expect("exists")
    }

    fn star(document: &Document, ratio: f64) -> ObjectSnapshot {
        let id = document.create_star(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).expect("count"),
            InnerRatio::new(ratio).expect("ratio"),
        );
        document.object(id).expect("exists")
    }

    fn radius_of(object: &ObjectSnapshot) -> f64 {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radius, .. },
            ..
        }) = object
        else {
            panic!("a rectangle");
        };
        corner_radius.as_mm()
    }

    fn ratio_of(object: &ObjectSnapshot) -> f64 {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Star { inner_ratio, .. },
            ..
        }) = object
        else {
            panic!("a star");
        };
        inner_ratio.get()
    }

    /// Ported from the Rectangle tool's AC4, AC5: a drag far past half the
    /// shorter side is clamped to it.
    #[test]
    fn a_radius_drag_is_clamped_to_half_the_shorter_side() {
        let document = Document::new(1);
        let start = rect(&document, 10.0, 10.0, 0.0);
        let value = value_from_pointer(
            &start,
            ParamHandle::CornerRadius(Corner::Tr),
            Vec2::new(-100.0, 100.0),
            1.0,
        )
        .expect("a value");
        let result = apply_param(&start, value);
        assert!((radius_of(&result) - 5.0).abs() < 1e-9);
    }

    /// Ported from AC6 of the Rectangle tool: dragging back past the zero
    /// position gives exactly 0.
    #[test]
    fn dragging_back_toward_the_corner_gives_exactly_zero() {
        let document = Document::new(1);
        let start = rect(&document, 100.0, 100.0, 10.0);
        let value = value_from_pointer(
            &start,
            ParamHandle::CornerRadius(Corner::Tl),
            Vec2::new(-30.0, -30.0),
            1.2,
        )
        .expect("a value");
        let ParamValue::Radius(radius) = value else {
            panic!("a radius");
        };
        assert!(radius.as_mm() < 0.0, "the raw value is below zero");
        assert_eq!(radius_of(&apply_param(&start, value)), 0.0);
    }

    /// The gain: one millimetre of pointer travel along the diagonal moves
    /// the radius by `gain` millimetres, on every corner's own diagonal.
    #[test]
    fn each_corner_projects_on_its_own_inward_diagonal() {
        let document = Document::new(1);
        let start = rect(&document, 100.0, 100.0, 10.0);
        let along = |corner: Corner| {
            let diagonal = corner.inward_diagonal();
            let value = value_from_pointer(
                &start,
                ParamHandle::CornerRadius(corner),
                diagonal.scaled(4.0),
                1.5,
            )
            .expect("a value");
            let ParamValue::Radius(radius) = value else {
                panic!("a radius");
            };
            radius.as_mm()
        };
        for corner in Corner::ALL {
            assert!((along(corner) - 16.0).abs() < 1e-9, "{corner:?}");
        }
    }

    /// Ported from the Rectangle tool's "starts from the effective radius
    /// after a shrink": a stored 40 on a 100 x 20 box starts from 10.
    #[test]
    fn a_radius_drag_starts_from_the_effective_radius() {
        let document = Document::new(1);
        let start = rect(&document, 100.0, 20.0, 40.0);
        let value = value_from_pointer(
            &start,
            ParamHandle::CornerRadius(Corner::Tr),
            Corner::Tr.inward_diagonal().negated(),
            1.0,
        )
        .expect("a value");
        let ParamValue::Radius(radius) = value else {
            panic!("a radius");
        };
        assert!(
            radius.as_mm() < 10.0,
            "shrinks at once from 10, not from 40"
        );
    }

    /// Criterion 12: a result equal to the effective start is the start, so a
    /// drag back to its start writes nothing even over a larger stored value.
    #[test]
    fn applying_the_effective_radius_returns_the_start_unchanged() {
        let document = Document::new(1);
        let start = rect(&document, 100.0, 20.0, 40.0);
        let result = apply_param(&start, ParamValue::Radius(Length::from_mm(10.0)));
        assert_eq!(result, start);
        let zero_drag = value_from_pointer(
            &start,
            ParamHandle::CornerRadius(Corner::Bl),
            Vec2::new(0.0, 0.0),
            1.0,
        )
        .expect("a value");
        assert_eq!(apply_param(&start, zero_drag), start);
    }

    /// Ported from the polygon/star tool's inner-radius drag: the ratio
    /// follows the handle along its own direction and is limited to
    /// 0.01..=0.99.
    #[test]
    fn an_inner_ratio_drag_moves_along_the_handle_direction_and_is_limited() {
        let document = Document::new(1);
        let start = star(&document, 0.5);
        let theta = std::f64::consts::PI / 5.0;
        let outward = Vec2::new(theta.cos(), theta.sin());
        let value = value_from_pointer(&start, ParamHandle::InnerRadius, outward.scaled(2.0), 1.0)
            .expect("a value");
        assert!((ratio_of(&apply_param(&start, value)) - 0.6).abs() < 1e-9);
        let far = value_from_pointer(&start, ParamHandle::InnerRadius, outward.scaled(1e6), 1.0)
            .expect("a value");
        assert!((ratio_of(&apply_param(&start, far)) - 0.99).abs() < 1e-12);
        let back = value_from_pointer(&start, ParamHandle::InnerRadius, outward.scaled(-1e6), 1.0)
            .expect("a value");
        assert!((ratio_of(&apply_param(&start, back)) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn the_outer_radius_stays_fixed_when_the_inner_radius_changes() {
        let document = Document::new(1);
        let start = star(&document, 0.5);
        let result = apply_param(
            &start,
            ParamValue::Ratio(InnerRatio::new(0.8).expect("ratio")),
        );
        let (ObjectSnapshot::Primitive(before), ObjectSnapshot::Primitive(after)) =
            (&start, &result)
        else {
            panic!("primitives");
        };
        let (Shape::Star { frame: a, .. }, Shape::Star { frame: b, .. }) =
            (before.shape, after.shape)
        else {
            panic!("stars");
        };
        assert_eq!(a, b);
    }

    #[test]
    fn non_finite_input_resolves_to_no_value_and_a_wrong_kind_to_the_start() {
        let document = Document::new(1);
        let start = rect(&document, 10.0, 10.0, 1.0);
        assert!(
            value_from_pointer(
                &start,
                ParamHandle::CornerRadius(Corner::Tl),
                Vec2::new(f64::NAN, 0.0),
                1.0
            )
            .is_none()
        );
        assert!(
            value_from_pointer(&start, ParamHandle::InnerRadius, Vec2::new(1.0, 1.0), 1.0)
                .is_none()
        );
        assert_eq!(
            apply_param(
                &start,
                ParamValue::Ratio(InnerRatio::new(0.3).expect("ratio"))
            ),
            start
        );
        assert_eq!(
            apply_param(&start, ParamValue::Radius(Length::from_mm(f64::NAN))),
            start
        );
    }

    #[test]
    fn a_point_count_applies_to_polygons_and_stars_only() {
        let document = Document::new(1);
        let star = star(&document, 0.5);
        let result = apply_param(
            &star,
            ParamValue::PointCount(PointCount::new(7).expect("count")),
        );
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Star { point_count, .. },
            ..
        }) = result
        else {
            panic!("a star");
        };
        assert_eq!(point_count.get(), 7);
        let rect = rect(&document, 10.0, 10.0, 0.0);
        assert_eq!(
            apply_param(
                &rect,
                ParamValue::PointCount(PointCount::new(7).expect("count"))
            ),
            rect
        );
    }

    /// Criterion 24: a drag commits through the shape tools' own commands.
    #[test]
    fn committing_a_radius_writes_one_commit_through_set_corner_radius() {
        let document = Document::new(1);
        let start = rect(&document, 100.0, 100.0, 0.0);
        let result = apply_param(&start, ParamValue::Radius(Length::from_mm(12.0)));
        commit_param(&document, ParamHandle::CornerRadius(Corner::Tl), &result);
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radius, .. },
            ..
        }) = document.object(start.id()).expect("exists")
        else {
            panic!("a rectangle");
        };
        assert!((corner_radius.as_mm() - 12.0).abs() < 1e-12);
    }

    #[test]
    fn a_batch_writes_every_named_id_in_one_commit() {
        let document = Document::new(1);
        let a = rect(&document, 100.0, 100.0, 5.0);
        let b = rect(&document, 50.0, 50.0, 5.0);
        commit_param_batch(
            &document,
            &[a.id(), b.id()],
            ParamValue::Radius(Length::from_mm(0.0)),
        )
        .expect("batch");
        assert_eq!(radius_of(&document.object(a.id()).expect("a")), 0.0);
        assert_eq!(radius_of(&document.object(b.id()).expect("b")), 0.0);
    }
}
