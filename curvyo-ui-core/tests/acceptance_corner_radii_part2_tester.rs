//! Independent black-box tests of `curvyo-ui-core`'s share of PART 2 of
//! `specs/rectangle-corner-radii/` (criteria 1 to 8, 12, 14 and 23 as far as
//! they are pure tool behaviour): knob layout and the diagonal cap, the Link
//! switch and Shift as an exclusive-or, the per-corner drag and its limit, the
//! effective-value writes on a shrunk rectangle, the 72 px threshold and the
//! handle tiers, preview == commit, hostile pointer values, and the body
//! reachability question of `adrs.md` decision 9.
//!
//! Written from the specification before the implementation was read. The
//! oracles (`css_effective`, `expected_knob_local`, the drag gain) are
//! restated here from the specification text, not taken from the code.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::suboptimal_flops,
    clippy::needless_range_loop,
    missing_docs
)]

use std::f64::consts::SQRT_2;

use curvyo_document_core::{
    Angle, CornerRadii, Document, Length, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot,
    RectBounds, Shape, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, Corner, CornerLinking, EditHandle, Modifiers, ObjectSelection, ParamHandle,
    SelectPointerDownOutcome, SelectTool, TransformHandleTolerances,
};

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);
const CORNERS: [Corner; 4] = [Corner::Tl, Corner::Tr, Corner::Br, Corner::Bl];

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn radii(r: [f64; 4]) -> CornerRadii {
    CornerRadii {
        tl: mm(r[0]),
        tr: mm(r[1]),
        br: mm(r[2]),
        bl: mm(r[3]),
    }
}

fn arr(r: CornerRadii) -> [f64; 4] {
    [r.tl.as_mm(), r.tr.as_mm(), r.br.as_mm(), r.bl.as_mm()]
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

/// The CSS rule of criterion 9, restated: `[tl, tr, br, bl]`.
fn css_effective(w: f64, h: f64, r: [f64; 4]) -> [f64; 4] {
    let mut f = 1.0_f64;
    for (num, den) in [
        (w, r[0] + r[1]),
        (w, r[3] + r[2]),
        (h, r[0] + r[3]),
        (h, r[1] + r[2]),
    ] {
        if den > 0.0 {
            f = f.min(num / den);
        }
    }
    r.map(|x| x * f)
}

/// The neighbours of a corner in `[tl, tr, br, bl]` indices: (shares the
/// horizontal side, shares the vertical side), criterion 4.
fn neighbours(i: usize) -> (usize, usize) {
    match i {
        0 => (1, 3),
        1 => (0, 2),
        2 => (3, 1),
        _ => (2, 0),
    }
}

/// Inward diagonal unit vector of corner `i` (document space, unrotated).
fn inward(i: usize) -> (f64, f64) {
    let s = 1.0 / SQRT_2;
    match i {
        0 => (s, s),
        1 => (-s, s),
        2 => (-s, -s),
        _ => (s, -s),
    }
}

/// The drawn position of corner `i`'s knob in the box's local frame
/// (millimetres, origin at the box's top-left), from the specification text of
/// criterion 1 and the design-system row "Parameter handle layout".
fn expected_knob_local(w: f64, h: f64, scale: f64, eff: [f64; 4], i: usize) -> (f64, f64) {
    let shorter = w.min(h);
    let s_px = shorter * scale;
    let long_px = w.max(h) * scale;
    let travel_px = (s_px - 14.0) / SQRT_2 - 15.0;
    let rho = |k: usize| eff[k] / (shorter / 2.0);
    let sigma = 2.0 + SQRT_2 * (long_px - s_px) / travel_px;
    let partner = (i + 2) % 4;
    let own = rho(i);
    let shown = if own <= 1.0 {
        own
    } else {
        own.min((sigma - rho(partner)).max(1.0))
    };
    let along_px = 15.0 + shown * travel_px;
    let along_mm = along_px / scale / SQRT_2;
    let (cx, cy) = match i {
        0 => (0.0, 0.0),
        1 => (w, 0.0),
        2 => (w, h),
        _ => (0.0, h),
    };
    let (dx, dy) = inward(i);
    (cx + dx.signum() * along_mm, cy + dy.signum() * along_mm)
}

/// Where a pointer must be so that the radius is exactly 0 again: 15 px from
/// the corner on the diagonal.
fn zero_position(origin: (f64, f64), w: f64, h: f64, scale: f64, i: usize) -> Point {
    let (lx, ly) = expected_knob_local(w, h, scale, [0.0; 4], i);
    pt(origin.0 + lx, origin.1 + ly)
}

struct Rig {
    document: Document,
    id: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
    scale: f64,
    w: f64,
    h: f64,
    origin: (f64, f64),
}

impl Rig {
    fn new(w: f64, h: f64, stored: [f64; 4], scale: f64) -> Self {
        let origin = (10.0, 20.0);
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: pt(origin.0, origin.1),
            width: mm(w),
            height: mm(h),
        });
        document.set_corner_radii(&[(id, radii(stored))]).unwrap();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        Self {
            document,
            id,
            selection,
            tool: SelectTool::new(),
            scale,
            w,
            h,
            origin,
        }
    }

    /// Rotates the rectangle about its own centre by `degrees`.
    fn rotate(&mut self, degrees: f64) {
        let o = self.object();
        let centre = pt(self.origin.0 + self.w / 2.0, self.origin.1 + self.h / 2.0);
        let rotated = o.rotated(centre, Angle::from_radians(degrees.to_radians()));
        self.document.rotate_object(&rotated).unwrap();
    }

    fn tol(&self) -> TransformHandleTolerances {
        TransformHandleTolerances::at_scale(self.scale)
    }

    fn object(&self) -> ObjectSnapshot {
        self.document.object(self.id).unwrap()
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        vec![self.object()]
    }

    fn handles(&self, side_rotate: bool) -> Vec<(EditHandle, Point)> {
        SelectTool::transform_handles(&self.objects(), &self.selection, self.tol(), side_rotate)
    }

    fn knob(&self, corner: Corner) -> Point {
        let wanted = EditHandle::Param(ParamHandle::CornerRadius(corner));
        self.handles(false)
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .expect("the knob is drawn")
            .1
    }

    fn press(&mut self, at: Point, shift: bool) -> SelectPointerDownOutcome {
        let objects = self.objects();
        let tol = self.tol();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            SEGMENT_TOLERANCE,
            tol,
            Modifiers::new(shift, false),
        )
    }

    fn preview(&self, at: Point, shift: bool, ctrl: bool) -> Option<ObjectSnapshot> {
        self.tool.live_transform(at, shift, ctrl)
    }

    fn release(&mut self, at: Point, shift: bool) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            Modifiers {
                shift,
                ..Modifiers::NONE
            },
            &mut AnchorIdMinter::new(99),
        );
    }

    /// The knob of `corner` (as drawn), moved `delta_mm` along its diagonal.
    fn target(&self, corner: Corner, delta_mm: f64) -> Point {
        let i = index(corner);
        let from = self.knob(corner);
        let (dx, dy) = inward(i);
        pt(from.x + dx * delta_mm, from.y + dy * delta_mm)
    }

    fn drag(&mut self, corner: Corner, shift_press: bool, shift_release: bool, delta_mm: f64) {
        let from = self.knob(corner);
        let to = self.target(corner, delta_mm);
        self.press(from, shift_press);
        self.release(to, shift_release);
    }

    fn stored(&self) -> [f64; 4] {
        arr(rect_radii(&self.object()))
    }

    fn effective_stored(&self) -> [f64; 4] {
        css_effective(self.w, self.h, self.stored())
    }
}

fn index(c: Corner) -> usize {
    CORNERS.iter().position(|x| *x == c).unwrap()
}

fn rect_radii(o: &ObjectSnapshot) -> CornerRadii {
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Rect { corner_radii, .. },
        ..
    }) = o
    else {
        panic!("a rectangle");
    };
    *corner_radii
}

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

/// `G(s) = (s/2)/L(s)` in mm of radius per mm of pointer travel.
fn gain(shorter_mm: f64, scale: f64) -> f64 {
    let travel_mm = ((shorter_mm * scale - 14.0) / SQRT_2 - 15.0) / scale;
    (shorter_mm / 2.0) / travel_mm
}

// ---------------------------------------------------------------------------
// Criterion 1: knob layout, the diagonal cap and the 4 px glyph gap
// ---------------------------------------------------------------------------

/// A radius set for a random box: a mixture that includes the awkward cases
/// (diagonal pair large, one corner at the shorter side, zeros, huge sums).
fn random_stored(rng: &mut Rng, w: f64, h: f64) -> [f64; 4] {
    let shorter = w.min(h);
    match (rng.next() * 6.0) as u32 {
        0 => [0.0; 4].map(|_| rng.range(0.0, shorter / 2.0)),
        1 => {
            // TL = BR large, the others small or zero.
            let big = rng.range(0.5, 1.0) * shorter;
            [big, rng.range(0.0, 0.2) * shorter, big, 0.0]
        }
        2 => {
            // A lone corner up to the shorter side.
            let mut r = [0.0; 4];
            r[(rng.next() * 4.0) as usize % 4] = shorter;
            r
        }
        3 => [0.0; 4].map(|_| rng.range(0.0, 3.0) * shorter),
        4 => [0.0; 4].map(|_| rng.range(0.0, 1.0) * shorter),
        _ => {
            let big = rng.range(0.55, 1.0) * shorter;
            [0.0, big, 0.0, big]
        }
    }
}

#[test]
fn knobs_are_drawn_where_the_specification_formula_puts_them() {
    let mut rng = Rng(0x00C0_FFEE_1234_5678);
    let mut checked = 0;
    for case in 0..1500 {
        let scale = rng.range(0.5, 6.0);
        let s_px = rng.range(72.0, 400.0);
        let aspect = if rng.next() < 0.3 {
            1.0
        } else {
            rng.range(1.0, 8.0)
        };
        let (w, h) = if rng.next() < 0.5 {
            (s_px / scale, s_px * aspect / scale)
        } else {
            (s_px * aspect / scale, s_px / scale)
        };
        let stored = random_stored(&mut rng, w, h);
        let mut rig = Rig::new(w, h, stored, scale);
        let rotation = if case % 4 == 0 {
            rng.range(0.0, 360.0)
        } else {
            0.0
        };
        if rotation != 0.0 {
            rig.rotate(rotation);
        }
        let eff = css_effective(w, h, stored);
        let centre = pt(rig.origin.0 + w / 2.0, rig.origin.1 + h / 2.0);
        for (i, corner) in CORNERS.iter().enumerate() {
            let (lx, ly) = expected_knob_local(w, h, scale, eff, i);
            let mut want = pt(rig.origin.0 + lx, rig.origin.1 + ly);
            if rotation != 0.0 {
                let (sn, cs) = rotation.to_radians().sin_cos();
                let (dx, dy) = (want.x - centre.x, want.y - centre.y);
                want = pt(centre.x + dx * cs - dy * sn, centre.y + dx * sn + dy * cs);
            }
            let got = rig.knob(*corner);
            assert!(
                close(got.x, want.x, 1e-6 / scale) && close(got.y, want.y, 1e-6 / scale),
                "case {case} {corner:?} got {got:?} want {want:?} (w {w} h {h} stored {stored:?})"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 6000);
}

/// Circumscribed radius (px) of every glyph kind for the clearance rule: the
/// parameter knob 10 px diameter, resize 8 x 8 px, rotate 12 px.
fn glyph_radius_px(handle: EditHandle) -> Option<f64> {
    match handle {
        EditHandle::Param(_) => Some(5.0),
        EditHandle::Resize(_) => Some(4.0 * SQRT_2),
        EditHandle::Rotate(_) => Some(6.0),
        // The centre glyph yields to a nearby knob (criterion 1 / design system).
        EditHandle::Move | EditHandle::Skew(_) => None,
    }
}

#[test]
fn no_two_drawn_glyphs_are_closer_than_four_pixels_for_independent_radii() {
    let mut rng = Rng(0x0BAD_5EED_9999_0001);
    let mut worst = f64::MAX;
    for case in 0..3000 {
        let scale = rng.range(0.5, 6.0);
        let s_px = if case % 7 == 0 {
            72.0
        } else {
            rng.range(72.0, 400.0)
        };
        let aspect = match case % 5 {
            0 => 1.0,
            1 => 1.8,
            _ => rng.range(1.0, 8.0),
        };
        let (w, h) = if rng.next() < 0.5 {
            (s_px / scale, s_px * aspect / scale)
        } else {
            (s_px * aspect / scale, s_px / scale)
        };
        let stored = random_stored(&mut rng, w, h);
        let mut rig = Rig::new(w, h, stored, scale);
        if case % 3 == 0 {
            rig.rotate(rng.range(0.0, 360.0));
        }
        for side_rotate in [false, true] {
            let drawn: Vec<(EditHandle, Point)> = rig
                .handles(side_rotate)
                .into_iter()
                .filter(|(hd, _)| glyph_radius_px(*hd).is_some())
                .collect();
            for a in 0..drawn.len() {
                for b in (a + 1)..drawn.len() {
                    let (ha, pa) = drawn[a];
                    let (hb, pb) = drawn[b];
                    let dist_px = (pa.x - pb.x).hypot(pa.y - pb.y) * scale;
                    let gap = dist_px - glyph_radius_px(ha).unwrap() - glyph_radius_px(hb).unwrap();
                    worst = worst.min(gap);
                    assert!(
                        gap >= 4.0 - 1e-6,
                        "case {case}: {ha:?} and {hb:?} are {gap:.3} px apart (w {w:.2} h {h:.2} scale {scale:.2} stored {stored:?} side_rotate {side_rotate})"
                    );
                }
            }
        }
    }
    eprintln!("smallest glyph gap over the sweep: {worst:.4} px");
}

#[test]
fn the_diagonal_cap_has_the_fixed_cases_of_the_specification() {
    // Square at exactly 72 px, TL = BR = 0.6 s: both at rho = 1.
    let scale = 2.0;
    let s = 36.0;
    let rig = Rig::new(s, s, [0.6 * s, 0.0, 0.6 * s, 0.0], scale);
    let tl = rig.knob(Corner::Tl);
    let br = rig.knob(Corner::Br);
    let d = (tl.x - br.x).hypot(tl.y - br.y) * scale;
    assert!(d >= 14.0 * SQRT_2 - 1e-6, "diagonal knobs {d:.3} px apart");
    let own = (rig.knob(Corner::Tl).x - rig.origin.0) * scale;
    // rho = 1 on a square: (s - 14)/2 px from each side.
    assert!(close(own, (72.0 - 14.0) / 2.0, 1e-6), "{own}");
    // The stored values are untouched by the cap.
    assert!(
        rig.stored()
            .iter()
            .zip([21.6, 0.0, 21.6, 0.0])
            .all(|(a, b)| close(*a, b, 1e-12))
    );

    // One lone corner reaches rho = 2 (the radius equals the shorter side).
    let rig = Rig::new(s, s, [s, 0.0, 0.0, 0.0], scale);
    let (lx, _) = expected_knob_local(s, s, scale, [s, 0.0, 0.0, 0.0], 0);
    assert!(close(rig.knob(Corner::Tl).x - rig.origin.0, lx, 1e-9));
    let travel = (72.0 - 14.0) / SQRT_2 - 15.0;
    assert!(close(lx * scale * SQRT_2, 15.0 + 2.0 * travel, 1e-6));

    // A wide box, 180 x 72 px: the sum is above 2 so no knob is pulled back.
    let (w, h) = (90.0, 36.0);
    let rig = Rig::new(w, h, [0.6 * h, 0.0, 0.6 * h, 0.0], scale);
    let eff = css_effective(w, h, rig.stored());
    let (lx, _) = expected_knob_local(w, h, scale, eff, 0);
    let travel = (72.0 - 14.0) / SQRT_2 - 15.0;
    // rho = 1.2 (own radius), not capped to 1.
    assert!(close(lx * scale * SQRT_2, 15.0 + 1.2 * travel, 1e-6));
    assert!(close(rig.knob(Corner::Tl).x - rig.origin.0, lx, 1e-9));

    // A knob at or below rho = 1 is at its own radius whatever its partner is.
    let rig = Rig::new(s, s, [0.5 * s, 0.0, s, 0.0], scale);
    let eff = rig.effective_stored();
    let (lx, _) = expected_knob_local(s, s, scale, eff, 0);
    assert!(close(rig.knob(Corner::Tl).x - rig.origin.0, lx, 1e-9));
    // rho = 1 exactly for TL: its own radius position, although BR (rho 2) is large.
    assert!(close(lx * scale * SQRT_2, 15.0 + travel_of(72.0), 1e-6));
}

fn travel_of(s_px: f64) -> f64 {
    (s_px - 14.0) / SQRT_2 - 15.0
}

#[test]
fn drawing_the_knobs_never_changes_a_stored_radius() {
    let rig = Rig::new(36.0, 36.0, [30.0, 0.0, 30.0, 0.0], 2.0);
    let before = rig.stored();
    let _ = rig.handles(true);
    let _ = rig.handles(false);
    assert_eq!(rig.stored(), before);
}

// ---------------------------------------------------------------------------
// Criteria 2, 3: Link switch and Shift as an exclusive-or, frozen at the press
// ---------------------------------------------------------------------------

#[test]
fn the_switch_and_shift_decide_one_corner_or_all_four() {
    for (unlinked_switch, shift) in [(false, false), (false, true), (true, false), (true, true)] {
        for start in [[5.0, 0.0, 12.0, 3.0], [8.0; 4]] {
            for corner in CORNERS {
                let mut rig = Rig::new(100.0, 60.0, start, 3.0);
                if unlinked_switch {
                    rig.tool.set_corner_linking(CornerLinking::Unlinked);
                }
                let i = index(corner);
                rig.drag(corner, shift, shift, 6.0);
                let after = rig.stored();
                let one_corner = unlinked_switch != shift;
                assert!(after[i] > start[i] + 1.0, "{corner:?} grew");
                if one_corner {
                    for k in 0..4 {
                        if k != i {
                            assert_eq!(
                                after[k], start[k],
                                "switch off {unlinked_switch} shift {shift} {corner:?} k {k}"
                            );
                        }
                    }
                } else {
                    assert!(after.iter().all(|v| *v == after[i]), "{after:?}");
                }
            }
        }
    }
}

#[test]
fn shift_is_read_once_at_the_press_and_frozen() {
    // Linked, press without Shift, Shift down at preview and release: still all four.
    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
    let from = rig.knob(Corner::Tl);
    let to = rig.target(Corner::Tl, 6.0);
    rig.press(from, false);
    let live = rig.preview(to, true, false).expect("a preview");
    let l = arr(rect_radii(&live));
    assert!(l.iter().all(|v| *v == l[0]), "{l:?}");
    rig.release(to, true);
    let a = rig.stored();
    assert!(a.iter().all(|v| *v == a[0]), "{a:?}");

    // Linked, press with Shift (one corner), Shift released mid-drag: still one corner.
    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
    let from = rig.knob(Corner::Tl);
    let to = rig.target(Corner::Tl, 6.0);
    rig.press(from, true);
    let live = arr(rect_radii(&rig.preview(to, false, false).unwrap()));
    assert_eq!(&live[1..], &[0.0, 12.0, 3.0]);
    rig.release(to, false);
    let a = rig.stored();
    assert_eq!(&a[1..], &[0.0, 12.0, 3.0]);
    assert!(a[0] > 5.0);

    // A click on the switch mid-drag changes nothing until the next drag.
    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
    let from = rig.knob(Corner::Tl);
    let to = rig.target(Corner::Tl, 6.0);
    rig.press(from, false);
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    rig.release(to, false);
    let a = rig.stored();
    assert!(a.iter().all(|v| *v == a[0]), "drag started linked: {a:?}");
    // The next drag is unlinked.
    let before = rig.stored();
    rig.drag(Corner::Tl, false, false, -3.0);
    let after = rig.stored();
    assert!(after[0] < before[0]);
    assert_eq!(&after[1..], &before[1..]);
}

// ---------------------------------------------------------------------------
// Criteria 4, 5: the per-corner limit, exact zero
// ---------------------------------------------------------------------------

#[test]
fn an_unlinked_drag_stops_at_the_neighbours_limit_and_moves_nothing_else() {
    let mut rng = Rng(0x1357_9BDF_0246_8ACE);
    for case in 0..400 {
        let scale = rng.range(0.8, 5.0);
        let w = rng.range(72.0 / scale, 300.0);
        let h = rng.range(72.0 / scale, 300.0);
        // A start with f = 1 (the sums on the sides within the sides).
        let stored = loop {
            let candidate = [0.0; 4].map(|_| rng.range(0.0, 0.7) * w.min(h));
            if css_effective(w, h, candidate) == candidate {
                break candidate;
            }
        };
        for corner in CORNERS {
            let i = index(corner);
            let mut rig = Rig::new(w, h, stored, scale);
            rig.tool.set_corner_linking(CornerLinking::Unlinked);
            let from = rig.knob(corner);
            let far = rig.target(corner, 10_000.0);
            rig.press(from, false);
            // Every step of the way the other three effective radii are unchanged.
            for step_px in [8.0, 20.0, 60.0, 400.0, 10_000.0] {
                let step = step_px / scale;
                let at = rig.target(corner, step);
                let live = rig.preview(at, false, false).unwrap();
                let eff = css_effective(w, h, arr(rect_radii(&live)));
                for k in 0..4 {
                    if k != i {
                        assert!(
                            close(eff[k], stored[k], 1e-9),
                            "case {case} {corner:?} step {step} k {k}: {eff:?} vs {stored:?}"
                        );
                    }
                }
            }
            rig.release(far, false);
            let (hh, vv) = neighbours(i);
            let limit = (w - stored[hh]).min(h - stored[vv]);
            let after = rig.stored();
            assert!(
                close(after[i], limit, 1e-9),
                "case {case} {corner:?}: {} vs limit {limit} (stored {stored:?}, {w}x{h})",
                after[i]
            );
            for k in 0..4 {
                if k != i {
                    assert_eq!(after[k], stored[k]);
                }
            }
        }
    }
}

#[test]
fn a_linked_drag_stops_at_half_the_shorter_side() {
    for (w, h) in [(100.0, 60.0), (60.0, 100.0), (80.0, 80.0)] {
        let mut rig = Rig::new(w, h, [3.0, 1.0, 9.0, 0.0], 3.0);
        rig.drag(Corner::Br, false, false, 10_000.0);
        assert_eq!(rig.stored(), [f64::min(w, h) / 2.0; 4]);
    }
}

#[test]
fn the_handle_stays_under_the_pointer_by_the_gain_of_the_specification() {
    for (w, h, scale) in [
        (100.0_f64, 60.0_f64, 3.0),
        (200.0, 120.0, 1.0),
        (30.0, 30.0, 4.0),
    ] {
        let shorter = w.min(h);
        for corner in CORNERS {
            let mut rig = Rig::new(w, h, [0.0; 4], scale);
            let from = rig.knob(corner);
            let delta = 4.0 / scale; // 4 px of pointer travel along the diagonal
            let to = rig.target(corner, delta);
            rig.press(from, false);
            let live = rect_radii(&rig.preview(to, false, false).unwrap());
            let want = delta * gain(shorter, scale);
            for v in arr(live) {
                assert!(
                    close(v, want, 1e-9),
                    "{corner:?}: {v} vs {want} (w {w} scale {scale})"
                );
            }
            rig.escape_drag();
        }
    }
}

impl Rig {
    fn escape_drag(&mut self) {
        self.tool.escape();
    }
}

#[test]
fn returning_to_or_beyond_the_zero_position_makes_the_corner_exactly_sharp() {
    let (w, h, scale) = (100.0, 60.0, 3.0);
    for linked in [true, false] {
        for corner in CORNERS {
            let i = index(corner);
            let mut rig = Rig::new(w, h, [10.0, 12.0, 14.0, 16.0], scale);
            if !linked {
                rig.tool.set_corner_linking(CornerLinking::Unlinked);
            }
            let from = rig.knob(corner);
            let zero = zero_position(rig.origin, w, h, scale, i);
            let (dx, dy) = inward(i);
            // At the zero position, and 20 mm outward of it (beyond, towards the corner).
            for beyond in [0.0, 20.0] {
                let to = pt(zero.x - dx * beyond, zero.y - dy * beyond);
                rig.press(from, false);
                let live = arr(rect_radii(&rig.preview(to, false, false).unwrap()));
                let exact = beyond > 0.0;
                let zero_ok = |v: f64| if exact { v == 0.0 } else { v.abs() <= 1e-9 };
                if linked {
                    assert!(
                        live.iter().all(|v| zero_ok(*v)),
                        "linked {corner:?} beyond {beyond}: {live:?}"
                    );
                } else {
                    assert!(zero_ok(live[i]), "{corner:?} beyond {beyond}: {}", live[i]);
                }
                rig.escape_drag();
            }
            // Past the corner altogether (the pointer left the box on the far side).
            let to = pt(zero.x - dx * 500.0, zero.y - dy * 500.0);
            rig.press(from, false);
            rig.release(to, false);
            assert_eq!(rig.stored()[i], 0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Criterion 3 (f < 1): the effective-value writes
// ---------------------------------------------------------------------------

#[test]
fn an_unlinked_drag_on_a_shrunk_rectangle_keeps_the_others_as_drawn_and_writes_them() {
    // 100 x 40: 30, 30, 0, 30 -> f = 2/3, drawn 20, 20, 0, 20.
    let stored = [30.0, 30.0, 0.0, 30.0];
    for corner in CORNERS {
        let i = index(corner);
        let mut rig = Rig::new(100.0, 40.0, stored, 4.0);
        rig.tool.set_corner_linking(CornerLinking::Unlinked);
        let drawn = rig.effective_stored();
        assert!(close(drawn[0], 20.0, 1e-9) && close(drawn[2], 0.0, 1e-12));
        let from = rig.knob(corner);
        rig.press(from, false);
        // At the press (no movement past the dead zone): no preview, nothing differs.
        // Then at every step the other three are unchanged as drawn.
        let sign = if drawn[i] > 0.0 { -1.0 } else { 1.0 };
        for step_px in [8.0, 20.0, 60.0] {
            let step = sign * step_px / 4.0;
            let at = rig.target(corner, step);
            let live = rig.preview(at, false, false).unwrap();
            let eff = css_effective(100.0, 40.0, arr(rect_radii(&live)));
            for k in 0..4 {
                if k != i {
                    assert!(
                        close(eff[k], drawn[k], 1e-9),
                        "{corner:?} step {step} k {k}: {eff:?} vs {drawn:?}"
                    );
                }
            }
        }
        let to = rig.target(corner, sign * 4.0);
        rig.release(to, false);
        let after = rig.stored();
        for k in 0..4 {
            if k != i {
                assert!(
                    close(after[k], drawn[k], 1e-9),
                    "{corner:?} stored k {k}: {after:?} vs {drawn:?} (written at the effective value)"
                );
            }
        }
        // And the sum is within the sides: no f < 1 left, so nothing moves on screen.
        assert_eq!(css_effective(100.0, 40.0, after), after);
    }
}

#[test]
fn a_linked_drag_on_a_shrunk_rectangle_writes_four_equal_radii() {
    let mut rig = Rig::new(100.0, 40.0, [30.0, 30.0, 0.0, 30.0], 4.0);
    rig.drag(Corner::Br, false, false, 5.0);
    let a = rig.stored();
    assert!(a.iter().all(|v| *v == a[0]) && a[0] > 0.0);
}

#[test]
fn a_press_and_release_on_a_knob_without_moving_writes_nothing() {
    for stored in [[30.0, 30.0, 0.0, 30.0], [5.0, 0.0, 12.0, 3.0]] {
        for unlinked in [false, true] {
            let mut rig = Rig::new(100.0, 40.0, stored, 4.0);
            if unlinked {
                rig.tool.set_corner_linking(CornerLinking::Unlinked);
            }
            let from = rig.knob(Corner::Tr);
            rig.press(from, false);
            rig.release(from, false);
            assert_eq!(rig.stored(), stored);
        }
    }
}

// ---------------------------------------------------------------------------
// Criterion 7 and 23: facts for the readout and the followers
// ---------------------------------------------------------------------------

#[test]
fn the_drag_reports_limit_overwrite_and_followers() {
    // Linked, unequal start: overwrite; far away: limited at half the shorter side.
    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(true));
    let near = rig.target(Corner::Tl, 3.0);
    let info = rig.tool.live_param_drag(near).unwrap();
    assert!(info.overwrites_unequal && !info.limited);
    let far = rig.target(Corner::Tl, 10_000.0);
    let info = rig.tool.live_param_drag(far).unwrap();
    assert!(info.limited);
    rig.escape_drag();
    assert_eq!(rig.tool.corner_drag_changes_all(), None);

    // Linked, equal start: no overwrite suffix.
    let mut rig = Rig::new(100.0, 60.0, [7.0; 4], 3.0);
    let from = rig.knob(Corner::Tl);
    rig.press(from, false);
    let info = rig
        .tool
        .live_param_drag(rig.target(Corner::Tl, 3.0))
        .unwrap();
    assert!(!info.overwrites_unequal);
    rig.escape_drag();

    // One-corner (Shift): no followers, no overwrite note, limited by the neighbours.
    let mut rig = Rig::new(100.0, 60.0, [5.0, 40.0, 12.0, 30.0], 3.0);
    let from = rig.knob(Corner::Tl);
    rig.press(from, true);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(false));
    let info = rig
        .tool
        .live_param_drag(rig.target(Corner::Tl, 10_000.0))
        .unwrap();
    assert!(!info.overwrites_unequal && info.limited);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(false));
    rig.escape_drag();

    // Switch off, Shift: all four (decided at the press).
    let mut rig = Rig::new(100.0, 60.0, [5.0, 40.0, 12.0, 30.0], 3.0);
    rig.tool.set_corner_linking(CornerLinking::Unlinked);
    let from = rig.knob(Corner::Tl);
    rig.press(from, true);
    assert_eq!(rig.tool.corner_drag_changes_all(), Some(true));
    rig.escape_drag();
}

#[test]
fn escape_cancels_the_drag_and_writes_nothing() {
    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
    let from = rig.knob(Corner::Tl);
    let to = rig.target(Corner::Tl, 8.0);
    rig.press(from, false);
    assert!(rig.preview(to, false, false).is_some());
    rig.escape_drag();
    assert!(rig.preview(to, false, false).is_none());
    rig.release(to, false);
    assert_eq!(rig.stored(), [5.0, 0.0, 12.0, 3.0]);
}

// ---------------------------------------------------------------------------
// Preview == commit
// ---------------------------------------------------------------------------

#[test]
fn the_preview_is_exactly_what_the_release_commits() {
    let mut rng = Rng(0xFEED_FACE_0BAD_CAFE);
    for case in 0..300 {
        let scale = rng.range(1.0, 4.0);
        let (w, h) = (
            rng.range(72.0 / scale, 200.0),
            rng.range(72.0 / scale, 200.0),
        );
        let stored = if case % 3 == 0 {
            [0.0; 4].map(|_| rng.range(0.0, 1.5) * w.min(h))
        } else {
            [0.0; 4].map(|_| rng.range(0.0, 0.4) * w.min(h))
        };
        let corner = CORNERS[(rng.next() * 4.0) as usize % 4];
        let unlinked = rng.next() < 0.5;
        let shift = rng.next() < 0.5;
        let mut rig = Rig::new(w, h, stored, scale);
        if unlinked {
            rig.tool.set_corner_linking(CornerLinking::Unlinked);
        }
        let from = rig.knob(corner);
        let to = rig.target(corner, rng.range(-20.0, 80.0));
        rig.press(from, shift);
        let live = rig.preview(to, shift, false);
        rig.release(to, shift);
        let after = rig.object();
        match live {
            Some(l) => assert_eq!(arr(rect_radii(&l)), arr(rect_radii(&after)), "case {case}"),
            None => assert_eq!(rig.stored(), stored),
        }
    }
}

// ---------------------------------------------------------------------------
// Hostile pointer values
// ---------------------------------------------------------------------------

#[test]
fn hostile_pointer_values_never_write_a_non_finite_or_negative_radius() {
    for unlinked in [false, true] {
        for corner in CORNERS {
            for bad in [
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                1e300,
                -1e300,
                f64::MAX,
                f64::MIN,
            ] {
                for (px, py) in [(bad, bad), (bad, 0.0), (0.0, bad)] {
                    let mut rig = Rig::new(100.0, 60.0, [5.0, 0.0, 12.0, 3.0], 3.0);
                    if unlinked {
                        rig.tool.set_corner_linking(CornerLinking::Unlinked);
                    }
                    let from = rig.knob(corner);
                    rig.press(from, false);
                    let _ = rig.preview(pt(px, py), false, false);
                    rig.release(pt(px, py), false);
                    let a = rig.stored();
                    let (w, h) = (100.0_f64, 60.0_f64);
                    for v in a {
                        assert!(v.is_finite() && v >= 0.0, "{corner:?} {px} {py}: {a:?}");
                        assert!(v <= w.max(h) + 1e-9, "{a:?}");
                    }
                    // A reopened copy of the document still opens: the file is valid.
                    let bytes = curvyo_document_core::pack(&rig.document, "t").unwrap();
                    curvyo_document_core::unpack(5, &bytes).expect("the saved file opens");
                }
            }
        }
    }
}

#[test]
fn hostile_stored_radii_and_boxes_do_not_panic_the_layout() {
    for stored in [
        [1e300, 1e300, 1e300, 1e300],
        [f64::MAX / 4.0, 0.0, f64::MAX / 4.0, 0.0],
        [0.0; 4],
        [1e-300; 4],
    ] {
        for (w, h) in [
            (100.0, 60.0),
            (1e-6, 1e-6),
            (1e9, 1e9),
            (0.0, 50.0),
            (50.0, 0.0),
        ] {
            let document = Document::new(1);
            let id = document.create_rect(RectBounds {
                origin: pt(0.0, 0.0),
                width: mm(w),
                height: mm(h),
            });
            let _ = document.set_corner_radii(&[(id, radii(stored))]);
            let mut selection = ObjectSelection::new();
            selection.select_single(id);
            let objects = vec![document.object(id).unwrap()];
            let tol = TransformHandleTolerances::at_scale(3.0);
            let hs = SelectTool::transform_handles(&objects, &selection, tol, true);
            for (_, p) in hs {
                assert!(p.x.is_finite() && p.y.is_finite(), "{stored:?} {w}x{h}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The 72 px threshold and the tiers
// ---------------------------------------------------------------------------

#[test]
fn the_tiers_and_the_72_px_threshold_hold_at_every_scale() {
    for scale in [0.5, 1.0, 2.0, 3.7, 6.0] {
        for (px, corner_only, edge, centre, param) in [
            (23.9, true, false, false, false),
            (24.0, true, true, false, false),
            (47.9, true, true, false, false),
            (48.0, true, true, true, false),
            (71.9, true, true, true, false),
            (72.0, true, true, true, true),
            (72.1, true, true, true, true),
            (200.0, true, true, true, true),
        ] {
            let side = px / scale;
            let rig = Rig::new(side * 1.5, side, [0.0; 4], scale);
            let hs = rig.handles(false);
            let bx = curvyo_ui_core::oriented_bounds(&rig.object());
            let tol = rig.tol();
            let count = |f: fn(&EditHandle) -> bool| hs.iter().filter(|(h, _)| f(h)).count();
            let drawn_resize: Vec<EditHandle> = hs
                .iter()
                .map(|(h, _)| *h)
                .filter(|h| matches!(h, EditHandle::Resize(_)))
                .filter(|h| curvyo_ui_core::is_drawn_handle(*h, &bx, &tol))
                .collect();
            assert_eq!(
                drawn_resize.len() > 4,
                edge,
                "{px} px at scale {scale}: edge resize"
            );
            assert!(drawn_resize.len() >= 4, "{px} px: corner resize always");
            let _ = corner_only;
            assert_eq!(
                hs.iter().any(|(h, _)| matches!(h, EditHandle::Move)),
                centre,
                "{px} px at scale {scale}: centre"
            );
            let knobs = count(|h| matches!(h, EditHandle::Param(ParamHandle::CornerRadius(_))));
            assert_eq!(
                knobs,
                if param { 4 } else { 0 },
                "{px} px at scale {scale}: knobs"
            );
            // Always four corner rotate handles.
            assert_eq!(count(|h| matches!(h, EditHandle::Rotate(_))), 4);
        }
    }
}

#[test]
fn below_72_px_a_press_where_a_knob_would_be_is_not_a_radius_press() {
    let scale = 2.0;
    let side = 71.5 / scale;
    let mut rig = Rig::new(side * 2.0, side, [0.0; 4], scale);
    let at = zero_position(rig.origin, side * 2.0, side, scale, 0);
    let outcome = rig.press(at, false);
    assert_ne!(
        rig.tool
            .dragging_handle()
            .map(|h| matches!(h, EditHandle::Param(_))),
        Some(true),
        "{outcome:?}"
    );
    rig.escape_drag();
}

// ---------------------------------------------------------------------------
// Body reachability on wide boxes (adrs.md decision 9)
// ---------------------------------------------------------------------------

fn press_kind(rig: &Rig, objects: &[ObjectSnapshot], at: Point) -> &'static str {
    let hit = SelectTool::handle_at(objects, &rig.selection, at, rig.tol(), false);
    match hit {
        Some((_, _, EditHandle::Param(_))) => "radius",
        Some(_) => "handle",
        None => "body",
    }
}

/// Fraction of the interior that is a move press, and whether the named probes
/// are moves, for a box of `s_px` and `aspect` with `rho` on all four knobs.
fn scan(s_px: f64, aspect: f64, stored_rho: [f64; 4]) -> (f64, Vec<(&'static str, &'static str)>) {
    let scale = 2.0;
    let (h, w) = (s_px / scale, s_px * aspect / scale);
    let shorter = h;
    let stored = stored_rho.map(|r| r * shorter / 2.0);
    let rig = Rig::new(w, h, stored, scale);
    let objects = rig.objects();
    let mut body = 0usize;
    let mut total = 0usize;
    let wpx = (w * scale) as usize;
    let hpx = (h * scale) as usize;
    for ix in (1..wpx).step_by(2) {
        for iy in (1..hpx).step_by(2) {
            let at = pt(
                rig.origin.0 + ix as f64 / scale,
                rig.origin.1 + iy as f64 / scale,
            );
            total += 1;
            if press_kind(&rig, &objects, at) == "body" {
                body += 1;
            }
        }
    }
    let c = (rig.origin.0 + w / 2.0, rig.origin.1 + h / 2.0);
    let probes = [
        ("centre", (c.0, c.1)),
        ("halfway to top edge midpoint", (c.0, c.1 - h / 4.0)),
        ("halfway to bottom edge midpoint", (c.0, c.1 + h / 4.0)),
        ("halfway to left edge midpoint", (c.0 - w / 4.0, c.1)),
        ("halfway to right edge midpoint", (c.0 + w / 4.0, c.1)),
    ];
    let named = probes
        .iter()
        .map(|(n, (x, y))| (*n, press_kind(&rig, &objects, pt(*x, *y))))
        .collect();
    (body as f64 / total as f64, named)
}

#[test]
fn a_wide_rectangle_can_always_be_moved_by_pressing_its_body_somewhere() {
    for aspect in [1.0, 1.4, 1.8, 2.5, 4.0, 8.0] {
        for s_px in [72.0, 80.0, 100.0, 150.0] {
            for rho in [0.0, 0.5, 1.0] {
                let (fraction, probes) = scan(s_px, aspect, [rho; 4]);
                eprintln!(
                    "aspect {aspect} s {s_px} rho {rho}: move area {:.1} % {probes:?}",
                    fraction * 100.0
                );
                assert!(
                    probes.iter().any(|(_, k)| *k == "body") || fraction > 0.02,
                    "no reachable body at aspect {aspect} s {s_px} rho {rho}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Other shapes are untouched
// ---------------------------------------------------------------------------

#[test]
fn only_rectangles_get_corner_knobs() {
    use curvyo_document_core::{EllipseFrame, PointCount, StarFrame};
    let document = Document::new(1);
    let ellipse = document.create_ellipse(EllipseFrame {
        center: pt(50.0, 50.0),
        rx: mm(40.0),
        ry: mm(30.0),
    });
    let polygon = document.create_polygon(
        StarFrame {
            center: pt(150.0, 50.0),
            radius: mm(40.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(6).unwrap(),
    );
    for id in [ellipse, polygon] {
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let objects = vec![
            document.object(id).unwrap(),
            document.object(ellipse).unwrap(),
        ];
        let tol = TransformHandleTolerances::at_scale(3.0);
        let hs = SelectTool::transform_handles(&objects, &selection, tol, false);
        assert!(
            !hs.iter()
                .any(|(h, _)| matches!(h, EditHandle::Param(ParamHandle::CornerRadius(_)))),
            "{id:?} has corner knobs"
        );
    }
}
