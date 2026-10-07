//! `curvyo-ui-core`'s share of `specs/polygon-star-box-refit/`: the oriented
//! box of a polygon or star has the direction `orientation()` (criteria 1, 2,
//! 4, 8, 13, 15), checked as a table, as an invariant over random shapes and
//! against golden numbers computed independently of the box code. The
//! session-level gestures are in `curvyo-editor-wasm/tests/`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::similar_names)]

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId,
    ObjectSnapshot, Point, PointCount, PrimitiveSnapshot, RectBounds, Shape, StarFrame, Vec2,
    outline_of_rotated,
};
use curvyo_ui_core::{
    ParamHandle, ParamValue, TransformHandleTolerances, oriented_bounds, param_handles,
    value_from_pointer,
};
use proptest::prelude::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn deg(d: f64) -> Angle {
    Angle::from_radians(d.to_radians())
}

fn id() -> NodeId {
    Document::new(1).create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    })
}

fn frame(center: Point, radius: f64, angle: Angle) -> StarFrame {
    StarFrame {
        center,
        radius: Length::from_mm(radius),
        angle,
    }
}

fn primitive(shape: Shape, rotation: Angle) -> ObjectSnapshot {
    ObjectSnapshot::Primitive(PrimitiveSnapshot {
        id: id(),
        shape,
        style: curvyo_document_core::Style::default(),
        rotation,
    })
}

fn polygon(
    center: Point,
    radius: f64,
    n: u32,
    frame_angle: Angle,
    rotation: Angle,
) -> ObjectSnapshot {
    primitive(
        Shape::Polygon {
            frame: frame(center, radius, frame_angle),
            point_count: PointCount::new(n).unwrap(),
        },
        rotation,
    )
}

fn star(
    center: Point,
    radius: f64,
    n: u32,
    ratio: f64,
    frame_angle: Angle,
    rotation: Angle,
) -> ObjectSnapshot {
    primitive(
        Shape::Star {
            frame: frame(center, radius, frame_angle),
            point_count: PointCount::new(n).unwrap(),
            inner_ratio: InnerRatio::new(ratio).unwrap(),
        },
        rotation,
    )
}

fn rotation_of(object: &ObjectSnapshot) -> Angle {
    object.rotation()
}

/// `a - b` wrapped into `(-pi, pi]`.
fn angle_diff(a: f64, b: f64) -> f64 {
    Angle::from_radians(a - b).normalized().as_radians()
}

fn near(a: Point, b: Point, tol: f64) -> bool {
    (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
}

/// The (frame angle, rotation) pairs of criterion 13 and the ADR's table, in
/// degrees.
const REGISTERS: [(f64, f64); 7] = [
    (0.0, 0.0),
    (30.0, 0.0),
    (78.7, 0.0),
    (10.0, 30.0),
    (-90.0, 0.0),
    (180.0, 0.0),
    (0.0, -78.7),
];

// ---------------------------------------------------------------------
// Criterion 1: the box of a polygon or star
// ---------------------------------------------------------------------

#[test]
fn the_box_direction_is_the_shown_angle_for_every_register_pair() {
    for (fa, rot) in REGISTERS {
        for object in [
            polygon(pt(3.0, -4.0), 10.0, 6, deg(fa), deg(rot)),
            star(pt(3.0, -4.0), 10.0, 5, 0.5, deg(fa), deg(rot)),
        ] {
            let b = oriented_bounds(&object);
            assert!(
                angle_diff(b.angle.as_radians(), object.orientation().as_radians()).abs() < 1e-12,
                "fa {fa} rot {rot}: box {} vs shown {}",
                b.angle.as_radians(),
                object.orientation().as_radians()
            );
            assert_eq!((b.min, b.max), (pt(-7.0, -14.0), pt(13.0, 6.0)));
            assert_eq!(b.pivot, pt(3.0, -4.0));
        }
    }
}

#[test]
fn the_worked_example_corners_at_thirty_degrees() {
    let object = polygon(pt(0.0, 0.0), 10.0, 6, deg(30.0), deg(0.0));
    let corners = oriented_bounds(&object).document_corners();
    let expected = [
        pt(-3.66, -13.66),
        pt(13.66, -3.66),
        pt(3.66, 13.66),
        pt(-13.66, 3.66),
    ];
    for (c, e) in corners.iter().zip(expected) {
        assert!(near(*c, e, 0.01), "{c:?} vs {e:?}");
    }
}

#[test]
fn at_zero_degrees_the_box_is_axis_aligned() {
    let object = star(pt(5.0, 5.0), 10.0, 5, 0.4, deg(0.0), deg(0.0));
    let corners = oriented_bounds(&object).document_corners();
    let expected = [
        pt(-5.0, -5.0),
        pt(15.0, -5.0),
        pt(15.0, 15.0),
        pt(-5.0, 15.0),
    ];
    for (c, e) in corners.iter().zip(expected) {
        assert!(near(*c, e, 1e-9), "{c:?} vs {e:?}");
    }
}

#[test]
fn a_shape_made_at_78_7_and_turned_to_zero_has_an_upright_box() {
    // Frame angle 78.7 and a rotation of -78.7: the shown angle is 0.
    let object = polygon(pt(0.0, 0.0), 10.0, 7, deg(78.7), deg(-78.7));
    let corners = oriented_bounds(&object).document_corners();
    for (c, e) in corners.iter().zip([
        pt(-10.0, -10.0),
        pt(10.0, -10.0),
        pt(10.0, 10.0),
        pt(-10.0, 10.0),
    ]) {
        assert!(near(*c, e, 1e-9), "{c:?} vs {e:?}");
    }
}

// ---------------------------------------------------------------------
// Criterion 15: the other kinds keep the box they had
// ---------------------------------------------------------------------

#[test]
fn rectangle_ellipse_and_path_keep_the_rotation_register_as_box_direction() {
    let rect = primitive(
        Shape::Rect {
            bounds: RectBounds {
                origin: pt(10.0, 20.0),
                width: Length::from_mm(40.0),
                height: Length::from_mm(10.0),
            },
            corner_radii: curvyo_document_core::CornerRadii::uniform(Length::from_mm(0.0)),
        },
        deg(33.0),
    );
    let ellipse = primitive(
        Shape::Ellipse {
            frame: EllipseFrame {
                center: pt(0.0, 0.0),
                rx: Length::from_mm(6.0),
                ry: Length::from_mm(3.0),
            },
        },
        deg(-120.0),
    );
    for object in [&rect, &ellipse] {
        let b = oriented_bounds(object);
        assert_eq!(b.angle, rotation_of(object));
    }
    let b = oriented_bounds(&rect);
    assert_eq!((b.min, b.max), (pt(10.0, 20.0), pt(50.0, 30.0)));
    assert_eq!(b.pivot, pt(30.0, 25.0));

    let document = Document::new(1);
    let path = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(20.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(20.0, 10.0)),
        ],
        false,
    );
    let path = document.object(path).unwrap();
    let b = oriented_bounds(&path);
    assert_eq!(b.angle, path.rotation());
    assert_eq!((b.min, b.max), (pt(0.0, 0.0), pt(20.0, 10.0)));
}

// ---------------------------------------------------------------------
// Criterion 8 and 13: the star handle stays where it was; golden numbers
// ---------------------------------------------------------------------

/// The old position of the first inner vertex, computed from the formula the
/// outline uses (angle sum plus a half step), not from the box.
fn golden_inner_vertex(
    center: Point,
    radius: f64,
    n: u32,
    ratio: f64,
    frame_angle: f64,
    rotation: f64,
) -> Point {
    let theta = frame_angle + rotation + std::f64::consts::PI / f64::from(n);
    pt(
        center.x + radius * ratio * theta.cos(),
        center.y + radius * ratio * theta.sin(),
    )
}

fn handle_position(object: &ObjectSnapshot) -> Point {
    let handles = param_handles(
        object,
        &oriented_bounds(object),
        &TransformHandleTolerances::at_scale(20.0),
    );
    let [(ParamHandle::InnerRadius, at)] = handles.as_slice() else {
        panic!("one inner-radius handle, got {handles:?}");
    };
    *at
}

#[test]
fn the_star_handle_is_where_main_put_it_for_old_files() {
    // The two shapes of criterion 13: a star at frame angle 10 / rotation 30,
    // and one at 78.7 / 0.
    for (fa, rot) in [(10.0, 30.0), (78.7, 0.0), (0.0, 0.0), (-170.0, 25.0)] {
        let c = pt(50.0, 40.0);
        let object = star(c, 50.0, 5, 0.5, deg(fa), deg(rot));
        let want = golden_inner_vertex(c, 50.0, 5, 0.5, fa.to_radians(), rot.to_radians());
        let got = handle_position(&object);
        assert!(
            near(got, want, 1e-9),
            "fa {fa} rot {rot}: {got:?} vs {want:?}"
        );
    }
}

#[test]
fn dragging_the_star_handle_changes_the_ratio_one_millimetre_per_millimetre() {
    // One millimetre along the inner vertex's direction in the box-local
    // frame, where the first outer vertex is at angle 0, so the inner vertex
    // is at pi / N.
    let n = 5_u32;
    let radius = 50.0;
    for (fa, rot) in [(0.0, 0.0), (78.7, 0.0), (10.0, 30.0)] {
        let object = star(pt(50.0, 40.0), radius, n, 0.5, deg(fa), deg(rot));
        let theta = std::f64::consts::PI / f64::from(n);
        let delta = Vec2::new(theta.cos(), theta.sin());
        let Some(ParamValue::Ratio(ratio)) =
            value_from_pointer(&object, ParamHandle::InnerRadius, delta, 1.0, false)
        else {
            panic!("a ratio");
        };
        assert!(
            (ratio.get() - (0.5 + 1.0 / radius)).abs() < 1e-9,
            "fa {fa} rot {rot}: {}",
            ratio.get()
        );
    }
}

// ---------------------------------------------------------------------
// Invariants over random shapes (criteria 1, 2, 8)
// ---------------------------------------------------------------------

proptest! {
    #[test]
    fn every_outline_vertex_lies_inside_the_box_and_the_first_outer_vertex_is_at_local_zero(
        cx in -200.0..200.0_f64, cy in -200.0..200.0_f64,
        radius in 5.0..150.0_f64, n in 3_u32..=24, ratio in 0.05..0.95_f64,
        frame_angle in -3.1..3.1_f64, rotation in -6.0..6.0_f64,
        is_star: bool,
    ) {
        let c = pt(cx, cy);
        let (fa, rot) = (Angle::from_radians(frame_angle), Angle::from_radians(rotation));
        let object = if is_star {
            star(c, radius, n, ratio, fa, rot)
        } else {
            polygon(c, radius, n, fa, rot)
        };
        let b = oriented_bounds(&object);
        let ObjectSnapshot::Primitive(p) = &object else { unreachable!() };
        let outline = outline_of_rotated(&p.shape, p.rotation);
        for anchor in &outline {
            let local = b.to_local(anchor.point);
            prop_assert!(
                local.x >= b.min.x - 1e-9 && local.x <= b.max.x + 1e-9
                    && local.y >= b.min.y - 1e-9 && local.y <= b.max.y + 1e-9,
                "vertex {:?} (local {local:?}) outside {:?}..{:?}", anchor.point, b.min, b.max
            );
        }
        // The first outer vertex is on the middle of the right-hand side.
        let first = b.to_local(outline[0].point);
        prop_assert!((first.x - (cx + radius)).abs() < 1e-9 && (first.y - cy).abs() < 1e-9,
            "first vertex local {first:?}");
        prop_assert!(near(b.to_document(pt(cx + radius, cy)), outline[0].point, 1e-9));
        prop_assert!(near(b.pivot, c, 1e-12));
        if is_star {
            let handle = handle_position(&object);
            prop_assert!(near(handle, outline[1].point, 1e-9),
                "handle {handle:?} vs inner vertex {:?}", outline[1].point);
        }
    }

    #[test]
    fn rotating_by_delta_about_the_centre_turns_the_box_by_delta(
        cx in -100.0..100.0_f64, cy in -100.0..100.0_f64,
        radius in 0.5..100.0_f64, n in 3_u32..=24,
        frame_angle in -3.1..3.1_f64, rotation in -3.1..3.1_f64,
        delta in -6.0..6.0_f64, is_star: bool,
    ) {
        let c = pt(cx, cy);
        let (fa, rot) = (Angle::from_radians(frame_angle), Angle::from_radians(rotation));
        let before = if is_star {
            star(c, radius, n, 0.5, fa, rot)
        } else {
            polygon(c, radius, n, fa, rot)
        };
        let after = before.rotated(c, Angle::from_radians(delta));
        let (b0, b1) = (oriented_bounds(&before), oriented_bounds(&after));
        prop_assert!(
            angle_diff(b1.angle.as_radians() - b0.angle.as_radians(), delta).abs() < 1e-9,
            "box turned by {} not {delta}", b1.angle.as_radians() - b0.angle.as_radians()
        );
        for (k0, k1) in b0.document_corners().iter().zip(b1.document_corners()) {
            let expected = k0.rotated_around(c, Angle::from_radians(delta));
            prop_assert!(near(k1, expected, 1e-9), "{k1:?} vs {expected:?}");
        }
        // The shown angle moves by the same delta.
        prop_assert!(
            angle_diff(
                after.orientation().as_radians() - before.orientation().as_radians(),
                delta
            ).abs() < 1e-9
        );
    }
}
