//! Independent tests of `curvyo-ui-core`'s object-transform arithmetic
//! (`specs/0005-object-transform/specification.md`, criteria 1, 4-9, 11-13,
//! 15-18): oriented boxes, handle layout and hit-testing, resize and rotate
//! drag arithmetic. Expected values come from the specification's own
//! rules, derived here, not from the crate's functions; invariants are
//! checked with `proptest`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use std::f64::consts::{FRAC_PI_2, PI};

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, Length, NewAnchor, ObjectSnapshot, Point, RectBounds,
    Vec2,
};
use curvyo_ui_core::{
    ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, ResizeDirection, TransformHandleTolerances,
    hit_transform_handle, oriented_bounds, polygon_star_resize_factor,
    resize_anchor_local_position, resize_cursor_angle_degrees, resize_local_box,
    rotate_delta_angle, rotate_pivot, scaled_and_floored, stroke_or_radius_factor,
    transform_handles,
};
use proptest::prelude::*;

/// Handle tolerances where one screen pixel is one millimetre, with the
/// rotate offset given and no centre handle (the slice 5 tests predate it).
fn tolerances(rotate_offset_mm: f64, hit_mm: f64) -> TransformHandleTolerances {
    TransformHandleTolerances {
        resize: curvyo_document_core::Tolerance::from_mm(hit_mm),
        rotate: curvyo_document_core::Tolerance::from_mm(hit_mm),
        rotate_offset_mm,
        center_min_side_mm: f64::INFINITY,
        ..TransformHandleTolerances::at_scale(1.0)
    }
}

/// Eight resize handles plus all eight rotate handles (Shift revealed).
fn all_handles_spec() -> HandleSpec {
    HandleSpec {
        resize_directions: &ALL_EIGHT,
        skew: false,
        side_rotate: true,
    }
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn snapshot_of_path(document: &Document) -> ObjectSnapshot {
    document.object(document.object_ids()[0]).unwrap()
}

fn s_curve() -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(60.0, 0.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(-50.0, 0.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 20.0))
            },
        ],
        false,
    );
    document
}

// ---------------------------------------------------------------------
// OrientedBox for every kind
// ---------------------------------------------------------------------

#[test]
fn rect_ellipse_boxes_are_the_frame_with_the_objects_rotation() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds {
        origin: pt(2.0, 3.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    });
    let ellipse = document.create_ellipse(EllipseFrame {
        center: pt(50.0, 50.0),
        rx: Length::from_mm(8.0),
        ry: Length::from_mm(3.0),
    });
    document
        .rotate_object(
            &document
                .object(rect)
                .expect("object exists")
                .rotated(pt(7.0, 5.0), Angle::from_radians(0.5)),
        )
        .unwrap();
    let b = oriented_bounds(&document.object(rect).unwrap());
    assert!(close(b.min.x, 2.0) && close(b.min.y, 3.0));
    assert!(close(b.max.x, 12.0) && close(b.max.y, 7.0));
    assert!(close(b.angle.as_radians(), 0.5));
    let e = oriented_bounds(&document.object(ellipse).unwrap());
    assert!(close(e.min.x, 42.0) && close(e.max.x, 58.0));
    assert!(close(e.min.y, 47.0) && close(e.max.y, 53.0));
    assert!(close(e.angle.as_radians(), 0.0));
}

#[test]
fn an_unrotated_path_box_is_tight_around_curve_extrema_not_the_control_hull() {
    // Cubic (0,0) c1(60,0) c2(-40,20) (10,20): x overshoots past the anchors.
    let object = snapshot_of_path(&s_curve());
    let b = oriented_bounds(&object);
    // Dense sampling gives the true extent independent of the crate.
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..=100_000 {
        let t = f64::from(i) / 100_000.0;
        let u = 1.0 - t;
        let (p0, p1, p2, p3) = ((0.0, 0.0), (60.0, 0.0), (-40.0, 20.0), (10.0, 20.0));
        let x =
            u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
        let y =
            u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    assert!(
        (b.min.x - x0).abs() < 1e-3 && (b.max.x - x1).abs() < 1e-3,
        "x: {b:?} vs {x0}..{x1}"
    );
    assert!(
        (b.min.y - y0).abs() < 1e-3 && (b.max.y - y1).abs() < 1e-3,
        "y: {b:?}"
    );
    assert!(b.max.x > 10.0 + 1.0, "extrema, not anchors: {b:?}");
}

#[test]
fn a_rotated_path_box_is_tight_in_the_objects_own_frame() {
    let document = s_curve();
    let id = document.object_ids()[0];
    let angle = 0.7;
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(pt(3.0, 4.0), Angle::from_radians(angle)),
        )
        .unwrap();
    let object = document.object(id).unwrap();
    assert!(close(object.rotation().as_radians(), angle));
    let b = oriented_bounds(&object);
    assert!(close(b.angle.as_radians(), angle));
    // The tight box in the local frame, found by sampling the baked curve
    // and rotating every sample back by -angle (any fixed pivot).
    let ObjectSnapshot::Path(p) = &object else {
        panic!()
    };
    let (a0, a1) = (&p.anchors[0], &p.anchors[1]);
    let c1 = pt(a0.point.x + a0.handle_out.x, a0.point.y + a0.handle_out.y);
    let c2 = pt(a1.point.x + a1.handle_in.x, a1.point.y + a1.handle_in.y);
    let origin = pt(0.0, 0.0);
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..=100_000 {
        let t = f64::from(i) / 100_000.0;
        let u = 1.0 - t;
        let x = u * u * u * a0.point.x
            + 3.0 * u * u * t * c1.x
            + 3.0 * u * t * t * c2.x
            + t * t * t * a1.point.x;
        let y = u * u * u * a0.point.y
            + 3.0 * u * u * t * c1.y
            + 3.0 * u * t * t * c2.y
            + t * t * t * a1.point.y;
        let local = rot(pt(x, y), origin, -angle);
        x0 = x0.min(local.x);
        x1 = x1.max(local.x);
        y0 = y0.min(local.y);
        y1 = y1.max(local.y);
    }
    // The box's own `to_local` may use a different fixed pivot; compare sizes
    // (frame-independent) and that every baked sample lies inside the box.
    assert!(
        ((b.width()) - (x1 - x0)).abs() < 1e-3,
        "width {} vs {}",
        b.width(),
        x1 - x0
    );
    assert!(
        ((b.height()) - (y1 - y0)).abs() < 1e-3,
        "height {} vs {}",
        b.height(),
        y1 - y0
    );
    for i in 0..=2000 {
        let t = f64::from(i) / 2000.0;
        let u = 1.0 - t;
        let x = u * u * u * a0.point.x
            + 3.0 * u * u * t * c1.x
            + 3.0 * u * t * t * c2.x
            + t * t * t * a1.point.x;
        let y = u * u * u * a0.point.y
            + 3.0 * u * u * t * c1.y
            + 3.0 * u * t * t * c2.y
            + t * t * t * a1.point.y;
        let l = b.to_local(pt(x, y));
        assert!(
            l.x >= b.min.x - 1e-3
                && l.x <= b.max.x + 1e-3
                && l.y >= b.min.y - 1e-3
                && l.y <= b.max.y + 1e-3,
            "curve sample {t} outside the oriented box"
        );
    }
}

#[test]
fn oriented_box_to_document_and_to_local_are_inverse() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(5.0, 5.0),
        width: Length::from_mm(30.0),
        height: Length::from_mm(10.0),
    });
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(pt(20.0, 10.0), Angle::from_radians(-2.2)),
        )
        .unwrap();
    let b = oriented_bounds(&document.object(id).unwrap());
    for p in [pt(0.0, 0.0), pt(12.5, -3.0), pt(1e4, -1e4)] {
        let back = b.to_local(b.to_document(p));
        assert!((back.x - p.x).abs() < 1e-6 && (back.y - p.y).abs() < 1e-6);
    }
}

// ---------------------------------------------------------------------
// Handle layout (AC 1, 11, 18)
// ---------------------------------------------------------------------

fn box_of(x0: f64, y0: f64, x1: f64, y1: f64, angle: f64) -> curvyo_ui_core::OrientedBox {
    curvyo_ui_core::OrientedBox {
        min: pt(x0, y0),
        max: pt(x1, y1),
        angle: Angle::from_radians(angle),
        pivot: pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1)),
    }
}

#[test]
fn eight_resize_plus_corner_rotate_handles_at_the_corners_and_edge_midpoints() {
    let b = box_of(0.0, 0.0, 40.0, 20.0, 0.0);
    // Shift not held: eight resize and four corner rotate handles.
    let spec = HandleSpec {
        side_rotate: false,
        ..all_handles_spec()
    };
    let handles = transform_handles(&b, spec, &tolerances(32.0, 1.0));
    assert_eq!(handles.len(), 12);
    let at = |h: EditHandle| handles.iter().find(|(k, _)| *k == h).unwrap().1;
    let expect = [
        (ResizeDirection::Nw, pt(0.0, 0.0)),
        (ResizeDirection::N, pt(20.0, 0.0)),
        (ResizeDirection::Ne, pt(40.0, 0.0)),
        (ResizeDirection::E, pt(40.0, 10.0)),
        (ResizeDirection::Se, pt(40.0, 20.0)),
        (ResizeDirection::S, pt(20.0, 20.0)),
        (ResizeDirection::Sw, pt(0.0, 20.0)),
        (ResizeDirection::W, pt(0.0, 10.0)),
    ];
    for (d, want) in expect {
        let got = at(EditHandle::Resize(d));
        assert!(
            close(got.x, want.x) && close(got.y, want.y),
            "{d:?}: {got:?}"
        );
    }
    // A corner rotate handle sits 32 away on the outward diagonal.
    let offset = 32.0 / 2.0_f64.sqrt();
    let rotate = at(EditHandle::Rotate(ResizeDirection::Se));
    assert!(
        close(rotate.x, 40.0 + offset) && close(rotate.y, 20.0 + offset),
        "corner rotate handle outside the Se corner: {rotate:?}"
    );
}

#[test]
fn polygon_star_handle_set_has_four_corners_and_the_corner_rotate_handles_only() {
    let b = box_of(0.0, 0.0, 20.0, 20.0, 0.0);
    let spec = HandleSpec {
        resize_directions: &CORNERS_FOUR,
        skew: false,
        side_rotate: false,
    };
    let handles = transform_handles(&b, spec, &tolerances(5.0, 1.0));
    assert_eq!(
        handles.len(),
        8,
        "four corners and four corner rotate handles"
    );
    for (kind, _) in &handles {
        if let EditHandle::Resize(d) = kind {
            assert!(
                matches!(
                    d,
                    ResizeDirection::Ne
                        | ResizeDirection::Se
                        | ResizeDirection::Sw
                        | ResizeDirection::Nw
                ),
                "edge handle {d:?} on a polygon/star"
            );
        }
    }
}

proptest! {
    #[test]
    fn handles_are_the_axis_aligned_layout_rotated_about_the_box_pivot(
        x0 in -100.0..100.0_f64, y0 in -100.0..100.0_f64,
        w in 0.1..200.0_f64, h in 0.1..200.0_f64,
        angle in -7.0..7.0_f64, offset in 0.1..30.0_f64,
    ) {
        let flat = box_of(x0, y0, x0 + w, y0 + h, 0.0);
        let turned = box_of(x0, y0, x0 + w, y0 + h, angle);
        let a = transform_handles(&flat, all_handles_spec(), &tolerances(offset, 1.0));
        let b = transform_handles(&turned, all_handles_spec(), &tolerances(offset, 1.0));
        let c = pt(x0 + w / 2.0, y0 + h / 2.0);
        for ((ka, pa), (kb, pb)) in a.iter().zip(&b) {
            prop_assert_eq!(ka, kb);
            let want = rot(*pa, c, angle);
            prop_assert!((want.x - pb.x).abs() < 1e-6 && (want.y - pb.y).abs() < 1e-6);
        }
        // The top side rotate handle sits `offset` from the (rotated) top-edge handle along local up.
        let top = b.iter().find(|(k, _)| *k == EditHandle::Resize(ResizeDirection::N)).unwrap().1;
        let rotate = b.iter().find(|(k, _)| *k == EditHandle::Rotate(ResizeDirection::N)).unwrap().1;
        let dist = (top.x - rotate.x).hypot(top.y - rotate.y);
        prop_assert!((dist - offset).abs() < 1e-6, "offset {dist} vs {offset}");
        // direction = local up = (sin a, -cos a)
        let (dx, dy) = ((rotate.x - top.x) / dist, (rotate.y - top.y) / dist);
        prop_assert!((dx - angle.sin()).abs() < 1e-6 && (dy + angle.cos()).abs() < 1e-6);
    }
}

#[test]
fn hit_test_picks_the_nearest_handle_within_tolerance_and_none_outside() {
    let b = box_of(0.0, 0.0, 40.0, 20.0, 0.0);
    let tol = tolerances(5.0, 1.0);
    let handles = transform_handles(&b, all_handles_spec(), &tol);
    let hit = |x: f64, y: f64| hit_transform_handle(&handles, &b, pt(x, y), &tol, false);
    assert_eq!(
        hit(40.5, 20.5),
        Some(EditHandle::Resize(ResizeDirection::Se))
    );
    // The top side rotate handle (revealed by Shift) sits 5 above the top edge.
    assert_eq!(
        hit(20.0, -4.6),
        Some(EditHandle::Rotate(ResizeDirection::N))
    );
    assert_eq!(hit(25.0, 5.0), None);
    assert_eq!(
        hit_transform_handle(&[], &b, pt(0.0, 0.0), &tol, false),
        None
    );
}

#[test]
fn rotate_and_top_edge_hit_areas_at_the_documented_radii_do_not_overlap() {
    // docs/design-system.md: resize hit radius 16 px, rotate 16 px, offset
    // 32 px (the sum of the two radii), so the two hit areas touch but never
    // overlap. The one nearest-centre rule puts the boundary at the
    // midpoint, 16 px above the top edge.
    let b = box_of(0.0, 0.0, 100.0, 100.0, 0.0);
    let tol = tolerances(32.0, 16.0);
    let handles = transform_handles(&b, all_handles_spec(), &tol);
    let hit = |y: f64| hit_transform_handle(&handles, &b, pt(50.0, y), &tol, false);
    assert_eq!(hit(-15.0), Some(EditHandle::Resize(ResizeDirection::N)));
    assert_eq!(hit(-17.0), Some(EditHandle::Rotate(ResizeDirection::N)));
}

// ---------------------------------------------------------------------
// Cursor angle (UX notes)
// ---------------------------------------------------------------------

#[test]
fn resize_cursor_angle_is_base_plus_rotation_mod_180() {
    let rotation = Angle::from_radians(30.0_f64.to_radians());
    let cases = [
        (ResizeDirection::E, 30.0),
        (ResizeDirection::W, 30.0),
        (ResizeDirection::N, 120.0),
        (ResizeDirection::S, 120.0),
        (ResizeDirection::Se, 75.0),
        (ResizeDirection::Nw, 75.0),
        (ResizeDirection::Ne, 165.0),
        (ResizeDirection::Sw, 165.0),
    ];
    for (d, want) in cases {
        let got = resize_cursor_angle_degrees(d, rotation);
        assert!((got - want).abs() < 1e-9, "{d:?}: {got} vs {want}");
    }
    for rotation in [-3.0, -0.2, 0.0, 1.0, 6.5, 100.0] {
        for d in ALL_EIGHT {
            let got = resize_cursor_angle_degrees(d, Angle::from_radians(rotation));
            assert!((0.0..180.0).contains(&got), "{d:?} @ {rotation}: {got}");
        }
    }
}

// ---------------------------------------------------------------------
// Resize arithmetic (AC 4-7, 13)
// ---------------------------------------------------------------------

fn direction_strategy() -> impl Strategy<Value = ResizeDirection> {
    prop::sample::select(ALL_EIGHT.to_vec())
}

fn opposite_point(min: Point, max: Point, d: ResizeDirection) -> Point {
    let (cx, cy) = (f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
    let x = match d {
        ResizeDirection::Ne | ResizeDirection::E | ResizeDirection::Se => min.x,
        ResizeDirection::Nw | ResizeDirection::W | ResizeDirection::Sw => max.x,
        _ => cx,
    };
    let y = match d {
        ResizeDirection::Se | ResizeDirection::S | ResizeDirection::Sw => min.y,
        ResizeDirection::Ne | ResizeDirection::N | ResizeDirection::Nw => max.y,
        _ => cy,
    };
    pt(x, y)
}

proptest! {
    #[test]
    fn resize_never_goes_negative_and_never_yields_nan(
        w in 0.0..500.0_f64, h in 0.0..500.0_f64,
        dx in -2000.0..2000.0_f64, dy in -2000.0..2000.0_f64,
        d in direction_strategy(), shift: bool, ctrl: bool,
    ) {
        let r = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, ctrl);
        prop_assert!(r.max.x - r.min.x >= -1e-9, "width {}", r.max.x - r.min.x);
        prop_assert!(r.max.y - r.min.y >= -1e-9, "height {}", r.max.y - r.min.y);
        for v in [r.min.x, r.min.y, r.max.x, r.max.y, r.sx, r.sy] {
            prop_assert!(v.is_finite(), "non-finite {v}");
        }
        prop_assert!(r.sx >= 0.0 && r.sy >= 0.0);
    }

    #[test]
    fn without_shift_the_opposite_corner_or_edge_stays_fixed(
        x0 in -50.0..50.0_f64, y0 in -50.0..50.0_f64,
        w in 1.0..300.0_f64, h in 1.0..300.0_f64,
        dx in -400.0..400.0_f64, dy in -400.0..400.0_f64,
        d in direction_strategy(),
    ) {
        let (min, max) = (pt(x0, y0), pt(x0 + w, y0 + h));
        let r = resize_local_box(min, max, d, Vec2::new(dx, dy), false, false);
        let before = opposite_point(min, max, d);
        let after = opposite_point(r.min, r.max, d);
        // Corner: both coordinates pinned; edge: only the touched axis is pinned.
        let corner = matches!(d, ResizeDirection::Ne | ResizeDirection::Se | ResizeDirection::Sw | ResizeDirection::Nw);
        let touches_x = !matches!(d, ResizeDirection::N | ResizeDirection::S);
        let touches_y = !matches!(d, ResizeDirection::E | ResizeDirection::W);
        if touches_x { prop_assert!((before.x - after.x).abs() < 1e-9, "x pinned {before:?} {after:?}"); }
        if touches_y { prop_assert!((before.y - after.y).abs() < 1e-9, "y pinned {before:?} {after:?}"); }
        let _ = corner;
    }

    #[test]
    fn the_dragged_edge_follows_the_pointer_one_to_one_until_the_zero_clamp(
        w in 5.0..300.0_f64, h in 5.0..300.0_f64,
        dx in -4.0..400.0_f64, dy in -4.0..400.0_f64,
    ) {
        // SE corner drag, no modifiers: new extent = old + delta (delta >= -4 < w).
        let r = resize_local_box(pt(0.0, 0.0), pt(w, h), ResizeDirection::Se, Vec2::new(dx, dy), false, false);
        prop_assert!((r.max.x - (w + dx)).abs() < 1e-9 && (r.max.y - (h + dy)).abs() < 1e-9);
        prop_assert!((r.sx - (w + dx) / w).abs() < 1e-9 && (r.sy - (h + dy) / h).abs() < 1e-9);
    }

    #[test]
    fn shift_keeps_the_box_center_fixed(
        x0 in -50.0..50.0_f64, y0 in -50.0..50.0_f64,
        w in 1.0..300.0_f64, h in 1.0..300.0_f64,
        dx in -400.0..400.0_f64, dy in -400.0..400.0_f64,
        d in direction_strategy(), ctrl: bool,
    ) {
        let (min, max) = (pt(x0, y0), pt(x0 + w, y0 + h));
        let r = resize_local_box(min, max, d, Vec2::new(dx, dy), true, ctrl);
        let touches_x = !matches!(d, ResizeDirection::N | ResizeDirection::S);
        let touches_y = !matches!(d, ResizeDirection::E | ResizeDirection::W);
        let (cx, cy) = (x0 + w / 2.0, y0 + h / 2.0);
        if touches_x { prop_assert!((f64::midpoint(r.min.x, r.max.x) - cx).abs() < 1e-9); }
        if touches_y { prop_assert!((f64::midpoint(r.min.y, r.max.y) - cy).abs() < 1e-9); }
    }

    #[test]
    fn ctrl_on_a_corner_makes_both_factors_equal_and_edges_ignore_ctrl(
        w in 1.0..300.0_f64, h in 1.0..300.0_f64,
        dx in -400.0..400.0_f64, dy in -400.0..400.0_f64,
        shift: bool,
    ) {
        for d in CORNERS_FOUR {
            let r = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, true);
            prop_assert!((r.sx - r.sy).abs() < 1e-9, "{d:?}: sx {} sy {}", r.sx, r.sy);
        }
        for d in [ResizeDirection::N, ResizeDirection::E, ResizeDirection::S, ResizeDirection::W] {
            let with = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, true);
            let without = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, false);
            prop_assert_eq!(with, without);
        }
    }

    #[test]
    fn edge_handles_leave_the_other_axis_alone(
        w in 1.0..300.0_f64, h in 1.0..300.0_f64,
        dx in -400.0..400.0_f64, dy in -400.0..400.0_f64,
        shift: bool, ctrl: bool,
    ) {
        for d in [ResizeDirection::E, ResizeDirection::W] {
            let r = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, ctrl);
            prop_assert!((r.max.y - r.min.y - h).abs() < 1e-9 && (r.sy - 1.0).abs() < 1e-12);
        }
        for d in [ResizeDirection::N, ResizeDirection::S] {
            let r = resize_local_box(pt(0.0, 0.0), pt(w, h), d, Vec2::new(dx, dy), shift, ctrl);
            prop_assert!((r.max.x - r.min.x - w).abs() < 1e-9 && (r.sx - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn resizing_is_idempotent_on_a_zero_delta(
        w in 0.0..300.0_f64, h in 0.0..300.0_f64, d in direction_strategy(), shift: bool, ctrl: bool,
    ) {
        let r = resize_local_box(pt(1.0, 2.0), pt(1.0 + w, 2.0 + h), d, Vec2::ZERO, shift, ctrl);
        prop_assert!((r.min.x - 1.0).abs() < 1e-9 && (r.min.y - 2.0).abs() < 1e-9);
        prop_assert!((r.max.x - (1.0 + w)).abs() < 1e-9 && (r.max.y - (2.0 + h)).abs() < 1e-9);
        prop_assert!((r.sx - 1.0).abs() < 1e-9 && (r.sy - 1.0).abs() < 1e-9);
    }
}

#[test]
fn flip_through_clamps_to_zero_and_the_far_edge_does_not_follow() {
    // 40 wide, drag the E handle 100 to the left: width 0, W edge stays at x = 0.
    let r = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::E,
        Vec2::new(-100.0, 0.0),
        false,
        false,
    );
    assert!(close(r.max.x - r.min.x, 0.0));
    assert!(close(r.min.x, 0.0) && close(r.max.x, 0.0), "no flip: {r:?}");
    assert!(close(r.sx, 0.0));
    // Same for W handle dragged right past E.
    let r = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::W,
        Vec2::new(100.0, 0.0),
        false,
        false,
    );
    assert!(close(r.min.x, 40.0) && close(r.max.x, 40.0), "{r:?}");
    // Shift: symmetric, so over-shrinking collapses onto the centre.
    let r = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::E,
        Vec2::new(-100.0, 0.0),
        true,
        false,
    );
    assert!(close(r.min.x, 20.0) && close(r.max.x, 20.0), "{r:?}");
    // Coming back after the clamp: the same function of the *start* box, not of history.
    let r = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::E,
        Vec2::new(10.0, 0.0),
        false,
        false,
    );
    assert!(close(r.max.x, 50.0));
}

#[test]
fn ctrl_corner_dominant_axis_boundary_is_deterministic_on_a_tie() {
    let a = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::Se,
        Vec2::new(10.0, 10.0),
        false,
        true,
    );
    let b = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::Se,
        Vec2::new(10.0, 10.0),
        false,
        true,
    );
    assert_eq!(a, b);
    assert!((a.sx - a.sy).abs() < 1e-12);
    // Dominant by |delta|: x -> 1.25; y -> 1.5.
    let x = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::Se,
        Vec2::new(11.0, 10.0),
        false,
        true,
    );
    assert!(close(x.sx, 51.0 / 40.0));
    let y = resize_local_box(
        pt(0.0, 0.0),
        pt(40.0, 20.0),
        ResizeDirection::Se,
        Vec2::new(10.0, 11.0),
        false,
        true,
    );
    assert!(close(y.sy, 31.0 / 20.0));
}

#[test]
fn ctrl_resize_of_a_zero_dimension_box_is_finite() {
    for (w, h) in [(0.0, 0.0), (0.0, 20.0), (40.0, 0.0)] {
        for d in CORNERS_FOUR {
            let r = resize_local_box(
                pt(0.0, 0.0),
                pt(w, h),
                d,
                Vec2::new(10.0, 10.0),
                false,
                true,
            );
            for v in [r.min.x, r.min.y, r.max.x, r.max.y, r.sx, r.sy] {
                assert!(v.is_finite(), "{w}x{h} {d:?}: {r:?}");
            }
        }
    }
}

#[test]
fn resize_anchor_is_the_opposite_point_or_the_center_under_shift() {
    let (min, max) = (pt(0.0, 0.0), pt(40.0, 20.0));
    assert_eq!(
        resize_anchor_local_position(min, max, ResizeDirection::Se, false),
        pt(0.0, 0.0)
    );
    assert_eq!(
        resize_anchor_local_position(min, max, ResizeDirection::Nw, false),
        pt(40.0, 20.0)
    );
    assert_eq!(
        resize_anchor_local_position(min, max, ResizeDirection::Se, true),
        pt(20.0, 10.0)
    );
    let e = resize_anchor_local_position(min, max, ResizeDirection::E, false);
    assert!(close(e.x, 0.0));
}

// ---------------------------------------------------------------------
// Stroke / radius factors (AC 8, 9)
// ---------------------------------------------------------------------

proptest! {
    #[test]
    fn stroke_factor_is_the_geometric_mean_and_equals_s_when_sx_eq_sy(
        sx in 0.0..100.0_f64, sy in 0.0..100.0_f64,
    ) {
        let f = stroke_or_radius_factor(sx, sy);
        prop_assert!((f - (sx * sy).sqrt()).abs() < 1e-9 * (1.0 + f));
        prop_assert!((stroke_or_radius_factor(sx, sx) - sx).abs() < 1e-9 * (1.0 + sx));
        prop_assert!(f.is_finite() && f >= 0.0);
        // symmetric
        prop_assert!((f - stroke_or_radius_factor(sy, sx)).abs() < 1e-12);
    }

    #[test]
    fn a_floored_stroke_never_reaches_zero_for_any_factor(
        start in 0.001..50.0_f64, factor in -5.0..50.0_f64, floor in 1e-6..0.01_f64,
    ) {
        let v = scaled_and_floored(Length::from_mm(start), factor, Length::from_mm(floor)).as_mm();
        prop_assert!(v >= floor - 1e-15 && v.is_finite());
    }
}

#[test]
fn stroke_factor_with_a_negative_or_nan_input_never_panics_or_goes_negative() {
    assert_eq!(stroke_or_radius_factor(-1.0, 4.0), 0.0);
    assert!(
        stroke_or_radius_factor(f64::NAN, 1.0).is_nan()
            || stroke_or_radius_factor(f64::NAN, 1.0) == 0.0
    );
}

// ---------------------------------------------------------------------
// Polygon / star uniform factor (AC 11)
// ---------------------------------------------------------------------

proptest! {
    #[test]
    fn polygon_star_factor_is_non_negative_finite_and_ignores_the_other_axis_sign_convention(
        r in 0.01..500.0_f64, dx in -1000.0..1000.0_f64, dy in -1000.0..1000.0_f64,
    ) {
        for d in CORNERS_FOUR {
            let f = polygon_star_resize_factor(r, d, Vec2::new(dx, dy));
            prop_assert!(f.is_finite() && f >= 0.0, "{d:?} {f}");
        }
        // Outward drag along each corner's own diagonal grows the radius.
        let unit = ResizeDirection::Se.unit_vector().normalized_to(1.0);
        let grow = polygon_star_resize_factor(r, ResizeDirection::Se, Vec2::new(unit.x * 5.0, unit.y * 5.0));
        prop_assert!(grow > 1.0);
        let shrink = polygon_star_resize_factor(r, ResizeDirection::Se, Vec2::new(-unit.x * 5.0, -unit.y * 5.0));
        prop_assert!(shrink < 1.0);
    }
}

// ---------------------------------------------------------------------
// Rotate arithmetic (AC 15-17)
// ---------------------------------------------------------------------

fn on_circle(c: Point, r: f64, a: f64) -> Point {
    pt(c.x + r * a.cos(), c.y + r * a.sin())
}

proptest! {
    #[test]
    fn rotate_delta_equals_the_angle_swept_about_the_pivot(
        cx in -50.0..50.0_f64, cy in -50.0..50.0_f64, r in 1.0..100.0_f64, r2 in 1.0..100.0_f64,
        a0 in -3.0..3.0_f64, sweep in -3.0..3.0_f64,
    ) {
        let c = pt(cx, cy);
        let delta = rotate_delta_angle(c, on_circle(c, r, a0), on_circle(c, r2, a0 + sweep), false).as_radians();
        prop_assert!((delta - sweep).abs() < 1e-9 || ((delta - sweep).abs() - 2.0 * PI).abs() < 1e-9, "{delta} vs {sweep}");
        prop_assert!(delta > -PI - 1e-12 && delta <= PI + 1e-12);
    }

    /// Superseded by `object-transform-refinements` criterion 33: the stops
    /// are the multiples of 15 and of 22.5 degrees, so the snapped delta is
    /// always a stop and never further than half the widest gap (15 degrees,
    /// between 30 and 45) from the free delta.
    #[test]
    fn ctrl_snaps_the_delta_to_a_15_or_22_5_degree_stop_within_half_a_gap(
        r in 1.0..100.0_f64, a0 in -3.0..3.0_f64, sweep in -3.1..3.1_f64,
    ) {
        let c = pt(0.0, 0.0);
        let free = rotate_delta_angle(c, on_circle(c, r, a0), on_circle(c, r, a0 + sweep), false).as_radians();
        let snapped = rotate_delta_angle(c, on_circle(c, r, a0), on_circle(c, r, a0 + sweep), true).as_radians();
        let in_period = snapped.to_degrees().abs().rem_euclid(45.0);
        let is_stop = [0.0, 15.0, 22.5, 30.0, 45.0].iter().any(|s| (in_period - s).abs() < 1e-6);
        prop_assert!(is_stop, "not a stop: {} deg", snapped.to_degrees());
        let mut d = (snapped - free).abs();
        if d > PI { d = 2.0 * PI - d; }
        prop_assert!(d <= 7.5_f64.to_radians() + 1e-9, "snap moved {} deg", d.to_degrees());
    }
}

#[test]
fn ctrl_snap_boundaries_between_adjacent_stops() {
    let c = pt(0.0, 0.0);
    let from = pt(10.0, 0.0);
    let delta = |deg: f64| {
        rotate_delta_angle(c, from, on_circle(c, 10.0, deg.to_radians()), true)
            .as_radians()
            .to_degrees()
    };
    for (raw, want) in [
        (7.49, 0.0),
        (7.51, 15.0),
        (18.74, 15.0),
        (18.76, 22.5),
        (26.24, 22.5),
        (26.26, 30.0),
        (37.49, 30.0),
        (37.51, 45.0),
    ] {
        assert!((delta(raw) - want).abs() < 1e-9, "{raw}");
        assert!((delta(-raw) + want).abs() < 1e-9, "-{raw}");
    }
    assert!((delta(179.0) - 180.0).abs() < 1e-9);
    // across the +-180 seam
    assert!((delta(-179.0).abs() - 180.0).abs() < 1e-9);
}

#[test]
fn rotate_delta_is_zero_when_the_pointer_sits_on_the_pivot() {
    let c = pt(5.0, 5.0);
    assert_eq!(
        rotate_delta_angle(c, pt(5.0, 0.0), c, false).as_radians(),
        0.0
    );
    assert_eq!(
        rotate_delta_angle(c, c, pt(9.0, 5.0), true).as_radians(),
        0.0
    );
}

#[test]
fn rotate_delta_never_leaks_a_nan_angle() {
    let c = pt(5.0, 5.0);
    let d = rotate_delta_angle(c, pt(5.0, 0.0), pt(f64::NAN, f64::NAN), false).as_radians();
    assert!(
        d.is_finite(),
        "NaN pointer produced a non-finite delta: {d}"
    );
}

#[test]
fn rotate_pivot_is_the_center_or_the_bottom_edge_midpoint_in_document_space() {
    let b = box_of(0.0, 0.0, 40.0, 20.0, 0.0);
    let center = rotate_pivot(&b, ResizeDirection::N, false);
    assert!(close(center.x, 20.0) && close(center.y, 10.0));
    let bottom = rotate_pivot(&b, ResizeDirection::N, true);
    assert!(close(bottom.x, 20.0) && close(bottom.y, 20.0));
    let b = box_of(0.0, 0.0, 40.0, 20.0, FRAC_PI_2);
    let bottom = rotate_pivot(&b, ResizeDirection::N, true);
    // local bottom-mid (20,20) about (20,10) by +90 deg -> (10,10)
    assert!(close(bottom.x, 10.0) && close(bottom.y, 10.0), "{bottom:?}");
}
