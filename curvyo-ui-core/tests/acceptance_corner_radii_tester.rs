//! Independent black-box tests of `curvyo-ui-core`'s share of PART 1 of
//! `specs/rectangle-corner-radii/` (criteria 1, 11, 16, 21, 22 for the data
//! layer; "no visible change for equal radii"): hit tests, bounds, handle
//! positions, and the linked radius write, all against oracles taken from
//! `origin/main` (single radius) or computed analytically.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    missing_docs
)]

use curvyo_document_core::{
    AnchorId, AnchorKind, CornerRadii, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point,
    RectBounds, Shape, Tolerance, Vec2, effective_corner_radii, outline_of_rotated,
};
use curvyo_ui_core::{
    BarValue, Corner, ObjectSelection, ParamHandle, ParamValue, TransformHandleTolerances,
    apply_param, hit_test_object, object_bounds, object_outline_bounds, oriented_bounds,
    param_handles, radius_travel, select_bar_state,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: mm(tl),
        tr: mm(tr),
        br: mm(br),
        bl: mm(bl),
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn rect_object(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: CornerRadii,
) -> (Document, NodeId, ObjectSnapshot) {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    });
    d.set_corner_radii(&[(id, r)]).unwrap();
    let o = d.object(id).unwrap();
    (d, id, o)
}

// ---- the old (origin/main) outline as an oracle, as anchors for a path ----

fn old_effective(w: f64, h: f64, r: f64) -> f64 {
    r.clamp(0.0, (w.min(h) / 2.0).max(0.0))
}

fn old_outline(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Vec<(Point, Vec2, Vec2)> {
    let r = old_effective(w, h, radius);
    let z = Vec2::ZERO;
    if r <= 0.0 {
        return vec![
            (pt(x, y), z, z),
            (pt(x + w, y), z, z),
            (pt(x + w, y + h), z, z),
            (pt(x, y + h), z, z),
        ];
    }
    let k = curvyo_document_core::KAPPA * r;
    vec![
        (pt(x + r, y), Vec2::new(-k, 0.0), z),
        (pt(x + w - r, y), z, Vec2::new(k, 0.0)),
        (pt(x + w, y + r), Vec2::new(0.0, -k), z),
        (pt(x + w, y + h - r), z, Vec2::new(0.0, k)),
        (pt(x + w - r, y + h), Vec2::new(k, 0.0), z),
        (pt(x + r, y + h), z, Vec2::new(-k, 0.0)),
        (pt(x, y + h - r), Vec2::new(0.0, k), z),
        (pt(x, y + r), z, Vec2::new(0.0, -k)),
    ]
}

fn path_object(anchors: &[(Point, Vec2, Vec2)]) -> (Document, ObjectSnapshot) {
    let d = Document::new(7);
    let na: Vec<NewAnchor> = anchors
        .iter()
        .enumerate()
        .map(|(i, (p, hi, ho))| NewAnchor {
            id: AnchorId::new(7, i as u64 + 1),
            point: *p,
            handle_in: *hi,
            handle_out: *ho,
            kind: AnchorKind::Corner,
        })
        .collect();
    let id = d.create_path(&na, true);
    let o = d.object(id).unwrap();
    (d, o)
}

#[test]
fn hit_tests_for_equal_radii_agree_with_the_old_outline_on_a_sweep() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let mut compared = 0;
    for _ in 0..120 {
        let w = rng.range(2.0, 200.0);
        let h = rng.range(2.0, 200.0);
        let x = rng.range(-40.0, 40.0);
        let y = rng.range(-40.0, 40.0);
        let r = match (rng.next() * 4.0) as u32 {
            0 => 0.0,
            1 => rng.range(0.0, 5.0),
            2 => rng.range(0.0, w.min(h) / 2.0),
            _ => rng.range(0.0, 400.0),
        };
        let (_d, _id, prim) = rect_object(x, y, w, h, CornerRadii::uniform(mm(r)));
        let (_pd, path) = path_object(&old_outline(x, y, w, h, r));
        for tol_mm in [0.3, 1.0, 4.0] {
            let tol = Tolerance::from_mm(tol_mm);
            for _ in 0..120 {
                let p = pt(x + rng.range(-0.1, 1.1) * w, y + rng.range(-0.1, 1.1) * h);
                let a = hit_test_object(std::slice::from_ref(&prim), p, tol).is_some();
                let b = hit_test_object(std::slice::from_ref(&path), p, tol).is_some();
                assert_eq!(a, b, "rect {x} {y} {w} {h} r {r} tol {tol_mm} at {p:?}");
                compared += 1;
            }
        }
    }
    assert!(compared > 10_000);
}

#[test]
fn hit_tests_use_the_effective_outline_per_corner() {
    // 100 x 60 at the origin, TL rounded with 30, the others sharp.
    let (_d, id, o) = rect_object(0.0, 0.0, 100.0, 60.0, radii(30.0, 0.0, 0.0, 0.0));
    let tol = Tolerance::from_mm(1.0);
    let hit = |p: Point| hit_test_object(std::slice::from_ref(&o), p, tol) == Some(id);
    // The sharp corners hit.
    assert!(hit(pt(100.0, 0.0)));
    assert!(hit(pt(100.0, 60.0)));
    assert!(hit(pt(0.0, 60.0)));
    // The cut-off TL corner point does not (it is 12.4 mm from the arc).
    assert!(!hit(pt(0.0, 0.0)));
    assert!(!hit(pt(2.0, 2.0)));
    // The arc's midpoint does.
    let m = 30.0 - 30.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(hit(pt(m, m)));
    // Along the straight sides.
    assert!(hit(pt(60.0, 0.0)));
    assert!(hit(pt(0.0, 45.0)));
    // Inside, away from the outline: no hit.
    assert!(!hit(pt(50.0, 30.0)));

    // Two diagonal corners rounded, two sharp: each sharp corner hits, each rounded one is cut.
    let (_d, id2, o2) = rect_object(0.0, 0.0, 100.0, 60.0, radii(0.0, 25.0, 0.0, 25.0));
    let hit2 = |p: Point| hit_test_object(std::slice::from_ref(&o2), p, tol) == Some(id2);
    assert!(hit2(pt(0.0, 0.0)) && hit2(pt(100.0, 60.0)));
    assert!(!hit2(pt(100.0, 0.0)) && !hit2(pt(0.0, 60.0)));
}

#[test]
fn hit_tests_with_a_clamped_shrunken_set_follow_the_css_factor() {
    // 100 x 40: TL 30, TR 30, BR 0, BL 30 -> f = 2/3, effective 20, 20, 0, 20.
    let (_d, id, o) = rect_object(0.0, 0.0, 100.0, 40.0, radii(30.0, 30.0, 0.0, 30.0));
    let tol = Tolerance::from_mm(0.5);
    let hit = |p: Point| hit_test_object(std::slice::from_ref(&o), p, tol) == Some(id);
    // Arc midpoint of TL with r = 20.
    let m = 20.0 - 20.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(hit(pt(m, m)));
    // A point on the arc the unclamped 30 mm radius would have drawn must not hit.
    let m30 = 30.0 - 30.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(!hit(pt(m30, m30)));
    assert!(hit(pt(100.0, 40.0)), "BR is sharp");
}

#[test]
fn selection_box_and_bounds_are_the_stored_frame_whatever_the_radii() {
    for r in [
        radii(0.0, 0.0, 0.0, 0.0),
        radii(30.0, 0.0, 0.0, 0.0),
        radii(30.0, 30.0, 0.0, 30.0),
        radii(500.0, 500.0, 500.0, 500.0),
        radii(10.0, 20.0, 5.0, 0.0),
    ] {
        let (_d, _id, o) = rect_object(3.0, -4.0, 100.0, 40.0, r);
        let (min, max) = object_bounds(&o);
        assert_eq!((min, max), (pt(3.0, -4.0), pt(103.0, 36.0)));
        let bx = oriented_bounds(&o);
        assert_eq!((bx.min, bx.max), (pt(3.0, -4.0), pt(103.0, 36.0)));
        // The drawn outline touches all four sides, so its tight bounds are the frame (criterion 21).
        let (omin, omax) = object_outline_bounds(&o);
        for (a, b) in [
            (omin.x, 3.0),
            (omin.y, -4.0),
            (omax.x, 103.0),
            (omax.y, 36.0),
        ] {
            assert!((a - b).abs() < 1e-6, "{r:?}: {a} vs {b}");
        }
    }
}

// ---- handle positions for equal radii: the old formula as an oracle ----

#[test]
fn knob_positions_for_equal_radii_are_what_the_old_single_radius_rule_gave() {
    let mut rng = Rng(0xABCD_EF01_2345_6789);
    for _ in 0..300 {
        let scale = rng.range(0.5, 6.0);
        let tol = TransformHandleTolerances::at_scale(scale);
        let w = rng.range(15.0, 300.0);
        let h = rng.range(15.0, 300.0);
        let r = rng.range(0.0, w.min(h) * 0.7);
        let (_d, _id, o) = rect_object(0.0, 0.0, w, h, CornerRadii::uniform(mm(r)));
        let bx = oriented_bounds(&o);
        let hs = param_handles(&o, &bx, &tol);
        let shorter = w.min(h);
        if shorter < tol.param_min_side_mm * 0.999 {
            // Below the threshold (72 px) no knob is drawn.
            assert!(hs.is_empty() || shorter >= tol.param_min_side_mm * 0.999);
            continue;
        }
        assert_eq!(hs.len(), 4);
        // Old rule: one effective radius for all four; distance = inset + rho * L(s) along the diagonal.
        let eff = old_effective(w, h, r);
        let rho = eff / (shorter / 2.0);
        let dist = tol.param_inset_mm + rho * radius_travel(shorter, &tol);
        let s2 = std::f64::consts::FRAC_1_SQRT_2;
        let want = [
            (Corner::Tl, pt(dist * s2, dist * s2)),
            (Corner::Tr, pt(w - dist * s2, dist * s2)),
            (Corner::Br, pt(w - dist * s2, h - dist * s2)),
            (Corner::Bl, pt(dist * s2, h - dist * s2)),
        ];
        for ((handle, at), (corner, p)) in hs.iter().zip(want) {
            assert_eq!(*handle, ParamHandle::CornerRadius(corner));
            assert!(
                (at.x - p.x).abs() < 1e-9 && (at.y - p.y).abs() < 1e-9,
                "{handle:?} {at:?} vs {p:?} (w {w} h {h} r {r})"
            );
        }
    }
}

#[test]
fn knobs_of_unequal_radii_follow_each_corners_own_effective_radius() {
    let tol = TransformHandleTolerances::at_scale(2.0);
    let (_d, _id, o) = rect_object(0.0, 0.0, 200.0, 100.0, radii(0.0, 20.0, 50.0, 0.0));
    let hs = param_handles(&o, &oriented_bounds(&o), &tol);
    let travel = radius_travel(100.0, &tol);
    let s2 = std::f64::consts::FRAC_1_SQRT_2;
    for (i, r) in [0.0, 20.0, 50.0, 0.0].into_iter().enumerate() {
        let d = tol.param_inset_mm + r / 50.0 * travel;
        let (sx, sy, cx, cy) = match i {
            0 => (1.0, 1.0, 0.0, 0.0),
            1 => (-1.0, 1.0, 200.0, 0.0),
            2 => (-1.0, -1.0, 200.0, 100.0),
            _ => (1.0, -1.0, 0.0, 100.0),
        };
        let want = pt(cx + sx * d * s2, cy + sy * d * s2);
        assert!(
            (hs[i].1.x - want.x).abs() < 1e-9 && (hs[i].1.y - want.y).abs() < 1e-9,
            "{i}"
        );
    }
}

// ---- PR 1 writes: a linked radius write (a drag / entry / bar) sets all four ----

fn radii_of(o: &ObjectSnapshot) -> CornerRadii {
    let ObjectSnapshot::Primitive(p) = o else {
        panic!()
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!()
    };
    corner_radii
}

#[test]
fn a_radius_value_applies_to_all_four_corners_limited_to_half_the_shorter_side() {
    let (_d, _id, o) = rect_object(0.0, 0.0, 100.0, 40.0, radii(12.0, 0.0, 3.0, 0.0));
    let r = radii_of(&apply_param(&o, ParamValue::Radius(mm(7.0))));
    assert_eq!(r, CornerRadii::uniform(mm(7.0)));
    let r = radii_of(&apply_param(&o, ParamValue::Radius(mm(999.0))));
    assert_eq!(r, CornerRadii::uniform(mm(20.0)));
    let r = radii_of(&apply_param(&o, ParamValue::Radius(mm(0.0))));
    assert_eq!(r, CornerRadii::uniform(mm(0.0)));
    // Negative is floored.
    let r = radii_of(&apply_param(&o, ParamValue::Radius(mm(-4.0))));
    assert_eq!(r, CornerRadii::uniform(mm(0.0)));
    // Uniform start, same value: unchanged object.
    let (_d, _id, u) = rect_object(0.0, 0.0, 100.0, 40.0, CornerRadii::uniform(mm(6.0)));
    assert_eq!(apply_param(&u, ParamValue::Radius(mm(6.0))), u);
    // Unequal start, value equal to one corner: still changes the other three.
    let (_d, _id, m) = rect_object(0.0, 0.0, 100.0, 40.0, radii(6.0, 0.0, 0.0, 0.0));
    assert_ne!(apply_param(&m, ParamValue::Radius(mm(6.0))), m);
}

#[test]
fn bar_state_reads_effective_values_of_every_corner_of_every_selected_rectangle() {
    let d = Document::new(1);
    let a = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(100.0),
        height: mm(40.0),
    });
    let b = d.create_rect(RectBounds {
        origin: pt(0.0, 50.0),
        width: mm(100.0),
        height: mm(40.0),
    });
    d.set_corner_radius(&[a, b], mm(5.0)).unwrap();
    let objects = vec![d.object(a).unwrap(), d.object(b).unwrap()];
    let mut sel = ObjectSelection::default();
    sel.set(&[a, b]);
    assert_eq!(
        select_bar_state(&objects, &sel, None).radius,
        Some(BarValue::Uniform(mm(5.0)))
    );
    d.set_corner_radii(&[(b, radii(5.0, 5.0, 5.0, 4.0))])
        .unwrap();
    let objects = vec![d.object(a).unwrap(), d.object(b).unwrap()];
    assert_eq!(
        select_bar_state(&objects, &sel, None).radius,
        Some(BarValue::Mixed)
    );
    // The effective evaluation is the one the bar uses: after shrinking, both read equal again.
    let e = effective_corner_radii(
        RectBounds {
            origin: pt(0.0, 0.0),
            width: mm(100.0),
            height: mm(40.0),
        },
        radii(5.0, 5.0, 5.0, 4.0),
    );
    assert_eq!(e.bl, mm(4.0));
}

#[test]
fn the_outline_used_for_conversion_has_four_to_eight_corner_anchors() {
    for mask in 0u32..16 {
        let r = |bit: u32, v: f64| if mask & (1 << bit) != 0 { v } else { 0.0 };
        let (_d, _id, o) = rect_object(
            0.0,
            0.0,
            100.0,
            60.0,
            radii(r(0, 10.0), r(1, 12.0), r(2, 14.0), r(3, 16.0)),
        );
        let ObjectSnapshot::Primitive(p) = &o else {
            panic!()
        };
        let out = outline_of_rotated(&p.shape, p.rotation);
        assert_eq!(out.len(), 4 + mask.count_ones() as usize);
        assert!(out.iter().all(|a| a.kind == AnchorKind::Corner));
    }
}
