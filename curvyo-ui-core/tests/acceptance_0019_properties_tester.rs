//! Property tests (proptest) for the invariants of a group gesture
//! (`specs/0019-multi-object-transform/specification.md` criteria 1, 16, 18, 20,
//! 26, 41): the group box contains every anchor; a move translates exactly; a
//! scale is the exact linear image about the opposite corner; a rotate is a
//! rigid motion with the `rotation` registers added; a shear keeps the signed
//! area of every closed polygon and the fixed line in place. The expected values
//! are computed here from the specification's formulas; only the group box
//! (validated against the spec's worked example in
//! `acceptance_0019_tester.rs`) is read from `SelectTool::group_of`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, missing_docs)]
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::doc_markdown
)]
#![allow(clippy::suboptimal_flops, clippy::manual_midpoint)]

use curvyo_document_core::{
    AnchorId, Document, NewAnchor, NodeId, ObjectSnapshot, Point, Tolerance, Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, Modifiers, ObjectSelection, SelectTool, TransformHandleTolerances,
};
use proptest::prelude::*;

const K: f64 = 4.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[derive(Debug, Clone)]
struct PathSpec {
    closed: bool,
    anchors: Vec<(f64, f64, f64, f64, f64, f64)>,
}

fn path_spec(handles: bool) -> impl Strategy<Value = PathSpec> {
    let h = if handles { 8.0 } else { 0.0 };
    let unit = -1.0..=1.0_f64;
    (
        any::<bool>(),
        prop::collection::vec(
            (
                0.0..100.0_f64,
                0.0..100.0_f64,
                unit.clone(),
                unit.clone(),
                unit.clone(),
                unit,
            ),
            2..6,
        ),
    )
        .prop_map(move |(closed, anchors)| PathSpec {
            closed,
            anchors: anchors
                .into_iter()
                .map(|(x, y, a, b, c, d)| (x, y, a * h, b * h, c * h, d * h))
                .collect(),
        })
}

fn build(specs: &[PathSpec]) -> (Document, Vec<NodeId>) {
    let d = Document::new(1);
    let mut n = 0_u64;
    let mut ids = Vec::new();
    // Two fixed anchors keep the group box at least 100 x 100 mm.
    let corner = |n: &mut u64, x: f64, y: f64| {
        *n += 1;
        NewAnchor::corner(AnchorId::new(1, *n), pt(x, y))
    };
    let a = corner(&mut n, 0.0, 0.0);
    let b = corner(&mut n, 100.0, 100.0);
    ids.push(d.create_path(&[a, b], false));
    for spec in specs {
        let anchors: Vec<NewAnchor> = spec
            .anchors
            .iter()
            .map(|&(x, y, ix, iy, ox, oy)| {
                n += 1;
                let mut anchor = NewAnchor::corner(AnchorId::new(1, n), pt(x, y));
                anchor.handle_in = Vec2::new(ix, iy);
                anchor.handle_out = Vec2::new(ox, oy);
                anchor
            })
            .collect();
        ids.push(d.create_path(&anchors, spec.closed));
    }
    (d, ids)
}

fn read(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn select_all(objects: &[ObjectSnapshot]) -> ObjectSelection {
    let mut s = ObjectSelection::new();
    s.set(&objects.iter().map(ObjectSnapshot::id).collect::<Vec<_>>());
    s
}

fn gesture(d: &Document, press: Point, to: Point, mods: Modifiers) -> Vec<ObjectSnapshot> {
    let objects = read(d);
    let mut selection = select_all(&objects);
    let mut tool = SelectTool::new();
    let mut minter = AnchorIdMinter::new(9);
    tool.pointer_down(
        &objects,
        &mut selection,
        press,
        Tolerance::from_mm(2.0),
        TransformHandleTolerances::at_scale(K),
        mods,
    );
    tool.pointer_moved(to, mods, &mut selection);
    tool.pointer_up(d, &objects, &mut selection, to, mods, &mut minter);
    read(d)
}

fn points(o: &ObjectSnapshot) -> Vec<(Point, Vec2, Vec2)> {
    match o {
        ObjectSnapshot::Path(p) => p
            .anchors
            .iter()
            .map(|a| (a.point, a.handle_in, a.handle_out))
            .collect(),
        ObjectSnapshot::Primitive(_) => unreachable!(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * (1.0 + a.abs().max(b.abs()))
}

fn area(o: &ObjectSnapshot) -> Option<f64> {
    let ObjectSnapshot::Path(p) = o else {
        return None;
    };
    if !p.closed
        || p.anchors
            .iter()
            .any(|a| a.handle_in != Vec2::ZERO || a.handle_out != Vec2::ZERO)
    {
        return None;
    }
    let pts: Vec<Point> = p.anchors.iter().map(|a| a.point).collect();
    let mut s = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    Some(s / 2.0)
}

fn rotated(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Criterion 1: the group box contains every anchor, and contains the box of
    /// each member.
    #[test]
    fn the_group_box_contains_every_anchor(specs in prop::collection::vec(path_spec(true), 1..4)) {
        let (d, _) = build(&specs);
        let objects = read(&d);
        let sel = select_all(&objects);
        let g = SelectTool::group_of(&objects, &sel).unwrap();
        let b = g.bounds();
        for o in &objects {
            for (p, _, _) in points(o) {
                prop_assert!(p.x >= b.min.x - 1e-9 && p.x <= b.max.x + 1e-9);
                prop_assert!(p.y >= b.min.y - 1e-9 && p.y <= b.max.y + 1e-9);
            }
        }
        prop_assert_eq!(b.angle.as_radians(), 0.0);
    }

    /// Criterion 16: a centre-handle drag translates every anchor by the pointer
    /// displacement and leaves handles and registers alone.
    #[test]
    fn a_move_translates_exactly(
        specs in prop::collection::vec(path_spec(true), 1..4),
        dx in -60.0..60.0_f64, dy in -60.0..60.0_f64,
    ) {
        prop_assume!(dx.hypot(dy) > 2.0);
        let (d, _) = build(&specs);
        let before = read(&d);
        let g = SelectTool::group_of(&before, &select_all(&before)).unwrap();
        let c = g.bounds().to_document(g.bounds().local_center());
        let after = gesture(&d, c, pt(c.x + dx, c.y + dy), Modifiers::NONE);
        for (a, b) in after.iter().zip(&before) {
            for ((pa, ia, oa), (pb, ib, ob)) in points(a).iter().zip(points(b)) {
                prop_assert!(close(pa.x, pb.x + dx) && close(pa.y, pb.y + dy));
                prop_assert_eq!(*ia, ib);
                prop_assert_eq!(*oa, ob);
            }
        }
    }

    /// Criteria 18 and 20: a corner scale is the exact linear image about the
    /// opposite corner, handle vectors scale per axis, registers stay.
    #[test]
    fn a_scale_is_the_exact_image(
        specs in prop::collection::vec(path_spec(true), 1..4),
        sx in 0.2..4.0_f64, sy in 0.2..4.0_f64,
    ) {
        prop_assume!((sx - 1.0).abs() > 0.05 || (sy - 1.0).abs() > 0.05);
        let (d, _) = build(&specs);
        let before = read(&d);
        let g = SelectTool::group_of(&before, &select_all(&before)).unwrap();
        let (lo, hi) = (g.bounds().min, g.bounds().max);
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        let after = gesture(&d, hi, pt(lo.x + w * sx, lo.y + h * sy), Modifiers::NONE);
        for (a, b) in after.iter().zip(&before) {
            for ((pa, ia, oa), (pb, ib, ob)) in points(a).iter().zip(points(b)) {
                prop_assert!(close(pa.x, lo.x + (pb.x - lo.x) * sx), "{} {}", pa.x, lo.x + (pb.x - lo.x) * sx);
                prop_assert!(close(pa.y, lo.y + (pb.y - lo.y) * sy));
                prop_assert!(close(ia.x, ib.x * sx) && close(ia.y, ib.y * sy));
                prop_assert!(close(oa.x, ob.x * sx) && close(oa.y, ob.y * sy));
            }
            prop_assert_eq!(a.rotation(), b.rotation());
        }
        // The dragged corner follows the pointer 1:1 (criterion 18: the box is
        // refit from the result; an affine image of a curve has the image bounds).
        let g2 = SelectTool::group_of(&after, &select_all(&after)).unwrap();
        prop_assert!(close(g2.bounds().max.x, lo.x + w * sx), "dragged corner x");
        prop_assert!(close(g2.bounds().max.y, lo.y + h * sy), "dragged corner y");
    }

    /// Criterion 26: a rotate is rigid: every pairwise anchor distance is
    /// unchanged, handle vectors turn by delta, `rotation` is increased by delta.
    #[test]
    fn a_rotate_is_rigid(
        specs in prop::collection::vec(path_spec(true), 1..4),
        degrees in prop_oneof![-170.0..-8.0_f64, 8.0..170.0_f64],
    ) {
        let (d, _) = build(&specs);
        let before = read(&d);
        let g = SelectTool::group_of(&before, &select_all(&before)).unwrap();
        let (lo, hi) = (g.bounds().min, g.bounds().max);
        let c = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let off = 32.0 / K / std::f64::consts::SQRT_2;
        let from = pt(hi.x + off, lo.y - off);
        let delta = degrees.to_radians();
        let to = rotated(from, c, delta);
        let after = gesture(&d, from, to, Modifiers::NONE);
        let flat = |os: &[ObjectSnapshot]| -> Vec<Point> {
            os.iter().flat_map(|o| points(o).into_iter().map(|(p, _, _)| p)).collect()
        };
        let (fa, fb) = (flat(&after), flat(&before));
        for (pa, pb) in fa.iter().zip(&fb) {
            let m = rotated(*pb, c, delta);
            prop_assert!(close(pa.x, m.x) && close(pa.y, m.y), "{pa:?} vs {m:?}");
        }
        for i in 0..fa.len().min(12) {
            for j in (i + 1)..fa.len().min(12) {
                let da = (fa[i].x - fa[j].x).hypot(fa[i].y - fa[j].y);
                let db = (fb[i].x - fb[j].x).hypot(fb[i].y - fb[j].y);
                prop_assert!(close(da, db), "distance {i} {j}");
            }
        }
        for (a, b) in after.iter().zip(&before) {
            for ((_, ia, oa), (_, ib, ob)) in points(a).iter().zip(points(b)) {
                let m = rotated(pt(ib.x, ib.y), pt(0.0, 0.0), delta);
                prop_assert!(close(ia.x, m.x) && close(ia.y, m.y));
                let m = rotated(pt(ob.x, ob.y), pt(0.0, 0.0), delta);
                prop_assert!(close(oa.x, m.x) && close(oa.y, m.y));
            }
            let want = (b.rotation().as_radians() + delta).rem_euclid(2.0 * std::f64::consts::PI);
            let got = a.rotation().as_radians().rem_euclid(2.0 * std::f64::consts::PI);
            let diff = (want - got).abs();
            prop_assert!(diff < 1e-9 || (2.0 * std::f64::consts::PI - diff) < 1e-9, "rotation {want} vs {got}");
        }
    }

    /// Criterion 41: a shear along the top handle keeps the bottom line and the
    /// signed area of every closed polygon of straight segments.
    #[test]
    fn a_shear_keeps_the_fixed_line_and_the_area(
        specs in prop::collection::vec(path_spec(false), 1..4),
        dx in prop_oneof![-80.0..-4.0_f64, 4.0..80.0_f64],
    ) {
        let (d, _) = build(&specs);
        let before = read(&d);
        let g = SelectTool::group_of(&before, &select_all(&before)).unwrap();
        let (lo, hi) = (g.bounds().min, g.bounds().max);
        let top = pt((lo.x + hi.x) / 2.0, lo.y - 16.0 / K);
        let after = gesture(&d, top, pt(top.x + dx, top.y), Modifiers::NONE);
        let t = dx / (hi.y - lo.y);
        for (a, b) in after.iter().zip(&before) {
            for ((pa, _, _), (pb, _, _)) in points(a).iter().zip(points(b)) {
                prop_assert!(close(pa.y, pb.y));
                prop_assert!(close(pa.x, pb.x + t * (hi.y - pb.y)), "x {} vs {}", pa.x, pb.x + t * (hi.y - pb.y));
            }
            if let (Some(sa), Some(sb)) = (area(a), area(b)) {
                prop_assert!(close(sa, sb), "area {sa} vs {sb}");
            }
            prop_assert_eq!(a.rotation(), b.rotation());
        }
    }
}
