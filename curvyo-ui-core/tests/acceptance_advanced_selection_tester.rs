//! Independent tester acceptance tests for `specs/advanced-selection/`
//! (criteria 1 to 20) at the `curvyo-ui-core` level. Written from the
//! specification; the expected values come from reference models written
//! here (rectangle distance, bounding-box overlap, the modifier table), never
//! from the code under test. Scale is 4 px/mm, so the 8 px Select tolerance
//! is 2 mm and the 3 px dead zone is 0.75 mm.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::type_complexity)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::manual_let_else, missing_docs)]
#![allow(clippy::cast_lossless, clippy::needless_range_loop)]

use std::sync::mpsc;
use std::time::Duration;

use curvyo_document_core::{
    AnchorId, Document, FillMode, FillModeTarget, Length, NewAnchor, NodeId, ObjectSnapshot, Point,
    RectBounds, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, GestureShape, MarqueeMode, Modifiers, ObjectSelection,
    SelectPointerDownOutcome, SelectTool, SelectionCombine, TransformHandleTolerances,
    hit_test_object, hit_test_objects, hit_test_objects_along, objects_in_marquee,
};
use proptest::prelude::*;

const SCALE: f64 = 4.0;
const TOL: f64 = 2.0;

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::new(true, false);
const CTRL: Modifiers = Modifiers::new(false, true);
const SHIFT_CTRL: Modifiers = Modifiers::new(true, true);
const ALT: Modifiers = Modifiers::NONE.with_alt(true);
const ALT_SHIFT: Modifiers = Modifiers::new(true, false).with_alt(true);
const ALT_CTRL: Modifiers = Modifiers::new(false, true).with_alt(true);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tol() -> Tolerance {
    Tolerance::from_mm(TOL)
}

// ---------------------------------------------------------------------
// Rig
// ---------------------------------------------------------------------

struct Rig {
    document: Document,
    selection: ObjectSelection,
    tool: SelectTool,
    minter: AnchorIdMinter,
    next_anchor: u64,
}

impl Rig {
    fn empty() -> Self {
        Self {
            document: Document::new(1),
            selection: ObjectSelection::new(),
            tool: SelectTool::new(),
            minter: AnchorIdMinter::new(9),
            next_anchor: 1,
        }
    }

    fn rect(&self, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        self.document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        })
    }

    fn line(&mut self, a: Point, b: Point) -> NodeId {
        let n = self.next_anchor;
        self.next_anchor += 2;
        self.document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(7, n), a),
                NewAnchor::corner(AnchorId::new(7, n + 1), b),
            ],
            false,
        )
    }

    fn fill(&self, id: NodeId) {
        self.document
            .set_fill_mode(
                FillMode::Solid,
                &[FillModeTarget {
                    id,
                    seed_stops: vec![],
                }],
            )
            .unwrap();
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    fn doc_fingerprint(&self) -> String {
        format!("{:?}", self.objects())
    }

    fn press(&mut self, at: Point, m: Modifiers) -> SelectPointerDownOutcome {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            tol(),
            TransformHandleTolerances::at_scale(SCALE),
            m,
        )
    }

    fn moved(&mut self, to: Point, m: Modifiers) {
        self.tool.pointer_moved(to, m, &mut self.selection);
    }

    fn release(&mut self, at: Point, m: Modifiers) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            m,
            &mut self.minter,
        );
    }

    /// A whole drag: press with `press`, move through the midpoint and the end
    /// with `release`'s modifiers, release with them.
    fn drag(&mut self, from: Point, to: Point, press: Modifiers, release: Modifiers) {
        self.press(from, press);
        self.moved(
            pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y)),
            release,
        );
        self.moved(to, release);
        self.release(to, release);
    }

    fn click(&mut self, at: Point, m: Modifiers) {
        self.press(at, m);
        self.release(at, m);
    }

    fn select(&mut self, ids: &[NodeId]) {
        self.selection.set(ids);
    }

    fn sel(&self) -> Vec<NodeId> {
        let mut v = self.selection.ids().to_vec();
        v.sort_by_key(key);
        v
    }
}

fn sorted(mut v: Vec<NodeId>) -> Vec<NodeId> {
    v.sort_by_key(key);
    v
}

/// Squares A, B, C at x 0/20/40 (10 mm, y 0..10) and a wide bar D (0..50 x 20..30).
fn scene() -> (Rig, [NodeId; 4]) {
    let rig = Rig::empty();
    let a = rig.rect(0.0, 0.0, 10.0, 10.0);
    let b = rig.rect(20.0, 0.0, 10.0, 10.0);
    let c = rig.rect(40.0, 0.0, 10.0, 10.0);
    let d = rig.rect(0.0, 20.0, 50.0, 10.0);
    (rig, [a, b, c, d])
}

/// Within `secs`, or the test fails (a hang is a defect, not a timeout of the harness).
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("did not finish in time (hang or pathological cost)")
}

// ---------------------------------------------------------------------
// AC 1: 8 px outline tolerance (pure hit test, 2 mm at 4 px/mm)
// ---------------------------------------------------------------------

#[test]
fn ac1_outline_hit_within_tolerance_and_not_beyond() {
    let (rig, [a, ..]) = scene();
    let objs = rig.objects();
    // Left edge of A at x = 0, mid-height.
    for (dx, hit) in [
        (0.0, true),
        (-1.9, true),
        (-2.1, false),
        (1.9, true),
        (2.1, false),
    ] {
        let got = hit_test_object(&objs, pt(dx, 5.0), tol());
        assert_eq!(got == Some(a), hit, "dx {dx}");
    }
}

#[test]
fn ac1_path_segment_within_tolerance() {
    let mut rig = Rig::empty();
    let l = rig.line(pt(0.0, 0.0), pt(30.0, 0.0));
    let objs = rig.objects();
    assert_eq!(hit_test_object(&objs, pt(15.0, 1.99), tol()), Some(l));
    assert_eq!(hit_test_object(&objs, pt(15.0, -1.99), tol()), Some(l));
    assert_eq!(hit_test_object(&objs, pt(15.0, 2.01), tol()), None);
    // Past the end by more than the tolerance: the ends are round, not extended.
    assert_eq!(hit_test_object(&objs, pt(32.5, 0.0), tol()), None);
    assert_eq!(hit_test_object(&objs, pt(31.9, 0.0), tol()), Some(l));
}

// ---------------------------------------------------------------------
// AC 3 to 7: candidates and Alt-click cycling
// ---------------------------------------------------------------------

/// Three vertical lines at x = 10, 11, 12 and a far cluster at 30, 31, 32.
fn cycle_scene() -> (Rig, [NodeId; 3], [NodeId; 3]) {
    let mut rig = Rig::empty();
    let near = [10.0, 11.0, 12.0].map(|x| rig.line(pt(x, 0.0), pt(x, 10.0)));
    let far = [30.0, 31.0, 32.0].map(|x| rig.line(pt(x, 0.0), pt(x, 10.0)));
    (rig, near, far)
}

#[test]
fn ac3_hit_test_objects_lists_every_candidate_nearest_first() {
    let (rig, [l10, l11, l12], _) = cycle_scene();
    let objs = rig.objects();
    // At x = 10.6: l11 0.4, l10 0.6, l12 1.4.
    assert_eq!(
        hit_test_objects(&objs, pt(10.6, 5.0), tol()),
        vec![l11, l10, l12]
    );
    assert_eq!(hit_test_object(&objs, pt(10.6, 5.0), tol()), Some(l11));
    assert!(hit_test_objects(&objs, pt(100.0, 100.0), tol()).is_empty());
}

#[test]
fn ac3_exact_tie_favours_the_topmost() {
    let mut rig = Rig::empty();
    let under = rig.line(pt(10.0, 0.0), pt(10.0, 10.0));
    let over = rig.line(pt(12.0, 0.0), pt(12.0, 10.0));
    let objs = rig.objects();
    assert_eq!(hit_test_object(&objs, pt(11.0, 5.0), tol()), Some(over));
    assert_eq!(
        hit_test_objects(&objs, pt(11.0, 5.0), tol()),
        vec![over, under]
    );
}

#[test]
fn ac3_to_5_plain_click_then_alt_clicks_cycle_and_wrap() {
    let (mut rig, [l10, l11, l12], _) = cycle_scene();
    let p = pt(10.6, 5.0);
    rig.click(p, NONE);
    assert_eq!(rig.selection.ids(), &[l11], "plain click: nearest");
    let expect = [l10, l12, l11, l10, l12, l11];
    for (n, id) in expect.into_iter().enumerate() {
        rig.click(p, ALT);
        assert_eq!(rig.selection.ids(), &[id], "alt-click #{}", n + 1);
    }
}

#[test]
fn ac5_n_alt_clicks_return_to_the_first() {
    let (mut rig, ..) = cycle_scene();
    let p = pt(10.6, 5.0);
    rig.click(p, NONE);
    let first = rig.sel();
    for _ in 0..3 {
        rig.click(p, ALT);
    }
    assert_eq!(rig.sel(), first);
}

#[test]
fn ac4_alt_click_replaces_the_previous_selection() {
    let (mut rig, [l10, l11, _], far) = cycle_scene();
    rig.select(&[far[0], far[1]]);
    rig.click(pt(10.6, 5.0), NONE);
    assert_eq!(rig.selection.ids(), &[l11]);
    rig.click(pt(10.6, 5.0), ALT);
    assert_eq!(rig.selection.ids(), &[l10], "only one, replacing");
}

#[test]
fn ac6_the_cycle_resets_at_a_point_beyond_tolerance() {
    let (mut rig, [_, l11, _], [l30, l31, l32]) = cycle_scene();
    rig.click(pt(10.6, 5.0), NONE);
    rig.click(pt(10.6, 5.0), ALT); // l10
    // A far point: candidates at 31.6 are l32 (0.4), l31 (0.6), l30 (1.6).
    rig.click(pt(31.6, 5.0), ALT);
    assert_eq!(
        rig.selection.ids(),
        &[l32],
        "starts at the nearest of the new point"
    );
    rig.click(pt(31.6, 5.0), ALT);
    assert_eq!(rig.selection.ids(), &[l31]);
    // Back to the first cluster: a fresh start there too.
    rig.click(pt(10.6, 5.0), ALT);
    assert_eq!(
        rig.selection.ids(),
        &[l11],
        "nearest again, not where the old cycle left"
    );
    let _ = l30;
}

#[test]
fn ac6_a_plain_click_elsewhere_resets_the_cycle() {
    let (mut rig, [l10, l11, _], _) = cycle_scene();
    let p = pt(10.6, 5.0);
    rig.click(p, NONE);
    rig.click(p, ALT);
    assert_eq!(rig.selection.ids(), &[l10]);
    rig.click(pt(100.0, 100.0), NONE); // empty canvas
    assert!(rig.selection.is_empty());
    rig.click(p, ALT);
    assert_eq!(
        rig.selection.ids(),
        &[l11],
        "a new cycle starts at the nearest"
    );
}

#[test]
fn ac6_small_jitter_inside_tolerance_keeps_cycling() {
    let (mut rig, [l10, l11, l12], _) = cycle_scene();
    rig.click(pt(10.6, 5.0), NONE);
    rig.click(pt(10.62, 5.1), ALT);
    assert_eq!(rig.selection.ids(), &[l10]);
    rig.click(pt(10.58, 4.9), ALT);
    assert_eq!(rig.selection.ids(), &[l12]);
    let _ = l11;
}

#[test]
fn ac7_a_single_candidate_stays_selected() {
    let (mut rig, _, [l30, ..]) = cycle_scene();
    rig.click(pt(27.0, 5.0), NONE); // 3 mm from l30: out of tolerance; nothing
    assert!(rig.selection.is_empty());
    let mut rig2 = Rig::empty();
    let only = rig2.line(pt(0.0, 0.0), pt(0.0, 10.0));
    rig2.click(pt(0.5, 5.0), NONE);
    for _ in 0..4 {
        rig2.click(pt(0.5, 5.0), ALT);
        assert_eq!(rig2.selection.ids(), &[only]);
    }
    let _ = l30;
}

#[test]
fn ac4_alt_click_without_a_prior_plain_click_starts_at_the_nearest() {
    let (mut rig, [_, l11, _], _) = cycle_scene();
    rig.click(pt(10.6, 5.0), ALT);
    assert_eq!(rig.selection.ids(), &[l11]);
}

#[test]
fn ac8_alt_click_jitter_within_3px_cycles_and_does_not_lasso() {
    let (mut rig, [l10, ..], _) = cycle_scene();
    let p = pt(10.6, 5.0);
    rig.click(p, NONE);
    // 2.8 px of hand jitter at 4 px/mm = 0.7 mm.
    rig.press(p, ALT);
    rig.moved(pt(10.6 + 0.7, 5.0), ALT);
    rig.release(pt(10.6 + 0.7, 5.0), ALT);
    assert_eq!(
        rig.selection.ids(),
        &[l10],
        "one candidate, not every one along a line"
    );
}

#[test]
fn ac8_alt_drag_past_3px_is_a_lasso_not_a_cycle_step() {
    let (mut rig, [l10, l11, l12], _) = cycle_scene();
    rig.drag(pt(9.0, 5.0), pt(13.5, 5.0), ALT, ALT);
    assert_eq!(
        rig.sel(),
        sorted(vec![l10, l11, l12]),
        "everything the line touches"
    );
}

// ---------------------------------------------------------------------
// AC 8 to 15: marquee
// ---------------------------------------------------------------------

#[test]
fn ac8_click_vs_drag_threshold_at_3_px() {
    // 2.9 px (0.725 mm): a click, clears. 3.2 px (0.8 mm): a marquee.
    for (dx, clears_via_click) in [(0.725, true), (0.8, false)] {
        let (mut rig, [a, ..]) = scene();
        rig.select(&[a]);
        rig.press(pt(100.0, 100.0), NONE);
        rig.moved(pt(100.0 + dx, 100.0), NONE);
        let g = rig.tool.live_gesture(pt(100.0 + dx, 100.0), NONE);
        assert_eq!(g.is_none(), clears_via_click, "dx {dx}: live gesture");
        rig.release(pt(100.0 + dx, 100.0), NONE);
        // Both end with an empty selection (an empty marquee replaces too).
        assert!(rig.selection.is_empty(), "dx {dx}");
    }
}

#[test]
fn ac8_ctrl_or_shift_click_on_empty_canvas_leaves_the_selection() {
    for m in [SHIFT, CTRL, SHIFT_CTRL] {
        let (mut rig, [a, b, ..]) = scene();
        rig.select(&[a, b]);
        rig.click(pt(100.0, 100.0), m);
        assert_eq!(rig.sel(), sorted(vec![a, b]), "{m:?}");
    }
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    rig.click(pt(100.0, 100.0), NONE);
    assert!(rig.selection.is_empty());
}

#[test]
fn ac8_empty_press_is_a_marquee_not_a_move_even_over_objects() {
    let (mut rig, [..]) = scene();
    let before = rig.doc_fingerprint();
    // Press outside every tolerance, sweep across all four objects.
    let out = rig.press(pt(-6.0, -6.0), NONE);
    assert_eq!(out, SelectPointerDownOutcome::Marquee);
    rig.moved(pt(60.0, 35.0), NONE);
    rig.release(pt(60.0, 35.0), NONE);
    assert_eq!(rig.doc_fingerprint(), before, "AC 15: nothing moved");
    assert_eq!(rig.selection.ids().len(), 4);
}

/// The modifier table of the specification, row by row.
/// (modifiers, rightward, expected mode, expected combine)
fn table() -> Vec<(Modifiers, bool, MarqueeMode, SelectionCombine)> {
    use MarqueeMode::{Contain, Touch};
    use SelectionCombine::{Add, Remove, Replace};
    vec![
        (NONE, false, Touch, Replace),
        (NONE, true, Contain, Replace),
        (ALT, false, Contain, Replace),
        (ALT, true, Touch, Replace),
        (SHIFT, false, Touch, Add),
        (SHIFT, true, Contain, Add),
        (CTRL, false, Touch, Remove),
        (CTRL, true, Contain, Remove),
        (ALT_SHIFT, false, Contain, Add),
        (ALT_SHIFT, true, Touch, Add),
        (ALT_CTRL, false, Contain, Remove),
        (ALT_CTRL, true, Touch, Remove),
        (SHIFT_CTRL, false, Touch, Remove),
        (SHIFT_CTRL, true, Contain, Remove),
        (
            Modifiers::new(true, true).with_alt(true),
            false,
            Contain,
            Remove,
        ),
        (
            Modifiers::new(true, true).with_alt(true),
            true,
            Touch,
            Remove,
        ),
    ]
}

/// Reference model: bounds of A, B, C, D as (x0, y0, x1, y1).
const BOUNDS: [(f64, f64, f64, f64); 4] = [
    (0.0, 0.0, 10.0, 10.0),
    (20.0, 0.0, 30.0, 10.0),
    (40.0, 0.0, 50.0, 10.0),
    (0.0, 20.0, 50.0, 30.0),
];

fn model_marquee(b: (f64, f64, f64, f64), from: Point, to: Point, mode: MarqueeMode) -> bool {
    let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
    let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
    match mode {
        MarqueeMode::Contain => b.0 >= x0 && b.2 <= x1 && b.1 >= y0 && b.3 <= y1,
        MarqueeMode::Touch => b.0 <= x1 && b.2 >= x0 && b.1 <= y1 && b.3 >= y0,
    }
}

fn model_apply(initial: &[NodeId], result: &[NodeId], combine: SelectionCombine) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = match combine {
        SelectionCombine::Replace => result.to_vec(),
        SelectionCombine::Add => {
            let mut v = initial.to_vec();
            for r in result {
                if !v.contains(r) {
                    v.push(*r);
                }
            }
            v
        }
        SelectionCombine::Remove => initial
            .iter()
            .copied()
            .filter(|i| !result.contains(i))
            .collect(),
    };
    out.sort_by_key(key);
    out
}

#[test]
fn ac9_to_14_the_full_modifier_table_for_a_marquee_drag() {
    // Box (-5,-5)..(25,15): crosses B, contains A only.
    let corners = (pt(-5.0, -5.0), pt(25.0, 15.0));
    for initial_ix in [vec![1, 2], vec![0, 2], vec![3], vec![]] {
        for (m, rightward, mode, combine) in table() {
            let (mut rig, ids) = scene();
            let initial: Vec<NodeId> = initial_ix.iter().map(|&i| ids[i]).collect();
            rig.select(&initial);
            let (from, to) = if rightward {
                corners
            } else {
                (corners.1, corners.0)
            };
            // Press: modifiers at press may differ; the table is about release.
            rig.press(from, NONE);
            rig.moved(to, m);
            // Live read (AC 14).
            let live = rig.tool.live_gesture(to, m).expect("marquee runs");
            assert_eq!(
                live.combine, combine,
                "live combine {m:?} right={rightward}"
            );
            match live.shape {
                GestureShape::Box {
                    mode: live_mode, ..
                } => {
                    assert_eq!(live_mode, mode, "live mode {m:?} right={rightward}");
                }
                GestureShape::Line(_) => panic!("a box expected"),
            }
            rig.release(to, m);
            let result: Vec<NodeId> = (0..4)
                .filter(|&i| model_marquee(BOUNDS[i], from, to, mode))
                .map(|i| ids[i])
                .collect();
            let want = model_apply(&initial, &result, combine);
            assert_eq!(
                rig.sel(),
                want,
                "initial {initial_ix:?} mods {m:?} rightward {rightward}"
            );
        }
    }
}

#[test]
fn ac10_no_horizontal_movement_is_touch() {
    let (mut rig, [a, b, ..]) = scene();
    // Straight down: x equal; touch: crossing counts.
    rig.press(pt(5.0, -5.0), NONE); // (5,-5) is 5 mm from A: empty
    rig.moved(pt(5.0, 15.0), NONE);
    let live = rig.tool.live_gesture(pt(5.0, 15.0), NONE).unwrap();
    assert!(matches!(
        live.shape,
        GestureShape::Box {
            mode: MarqueeMode::Touch,
            ..
        }
    ));
    rig.release(pt(5.0, 15.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[a],
        "a zero-width touch box crosses A"
    );
    let _ = b;
}

#[test]
fn ac11_alt_pressed_mid_drag_inverts_and_released_reverts() {
    let (mut rig, [a, b, ..]) = scene();
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), NONE);
    let mode =
        |rig: &Rig, m: Modifiers| match rig.tool.live_gesture(pt(25.0, 15.0), m).unwrap().shape {
            GestureShape::Box { mode, .. } => mode,
            GestureShape::Line(_) => panic!("a box"),
        };
    assert_eq!(mode(&rig, NONE), MarqueeMode::Contain);
    assert_eq!(
        mode(&rig, ALT),
        MarqueeMode::Touch,
        "Alt inverts, still a box"
    );
    assert_eq!(mode(&rig, NONE), MarqueeMode::Contain, "released: reverts");
    // Alt at release: inverted.
    rig.moved(pt(25.0, 15.0), ALT);
    rig.release(pt(25.0, 15.0), ALT);
    assert_eq!(rig.sel(), sorted(vec![a, b]));
}

#[test]
fn ac11_alt_pressed_then_released_before_release_applies_the_default() {
    let (mut rig, [a, ..]) = scene();
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), ALT);
    rig.moved(pt(25.0, 15.0), NONE);
    rig.release(pt(25.0, 15.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[a],
        "contain, the un-inverted default"
    );
}

#[test]
fn ac11_a_direction_reversal_flips_the_mode_live() {
    let (mut rig, ..) = scene();
    rig.press(pt(15.0, 40.0), NONE);
    rig.moved(pt(30.0, 5.0), NONE);
    let mode_at = |rig: &Rig, p: Point| match rig.tool.live_gesture(p, NONE).unwrap().shape {
        GestureShape::Box { mode, .. } => mode,
        GestureShape::Line(_) => panic!("a box"),
    };
    assert_eq!(mode_at(&rig, pt(30.0, 5.0)), MarqueeMode::Contain);
    assert_eq!(mode_at(&rig, pt(0.0, 5.0)), MarqueeMode::Touch);
    assert_eq!(
        mode_at(&rig, pt(15.0, 5.0)),
        MarqueeMode::Touch,
        "equal x: touch"
    );
    assert_eq!(mode_at(&rig, pt(15.001, 5.0)), MarqueeMode::Contain);
}

#[test]
fn ac12_13_modifiers_are_read_at_release_not_at_press() {
    // Shift at the press only: release is a replace.
    let (mut rig, [a, b, c, _]) = scene();
    rig.select(&[c]);
    rig.press(pt(-5.0, -5.0), SHIFT);
    rig.moved(pt(25.0, 15.0), NONE);
    rig.release(pt(25.0, 15.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[a],
        "Shift only at press: replaces (contain)"
    );
    // Ctrl at the press only.
    let (mut rig, [a, _, c, _]) = scene();
    rig.select(&[a, c]);
    rig.press(pt(-5.0, -5.0), CTRL);
    rig.moved(pt(25.0, 15.0), NONE);
    rig.release(pt(25.0, 15.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[a],
        "Ctrl only at press: replace, not remove"
    );
    // Shift only at release.
    let (mut rig, [a, _, c, _]) = scene();
    rig.select(&[c]);
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), NONE);
    rig.release(pt(25.0, 15.0), SHIFT);
    assert_eq!(rig.sel(), sorted(vec![a, c]));
    let _ = b;
}

#[test]
fn ac13_ctrl_never_adds_an_unselected_object() {
    let (mut rig, [a, b, c, d]) = scene();
    rig.select(&[c, d]);
    rig.drag(pt(25.0, 15.0), pt(-5.0, -5.0), NONE, CTRL); // touches A and B only
    assert_eq!(
        rig.sel(),
        sorted(vec![c, d]),
        "nothing added, nothing else removed"
    );
    let _ = (a, b);
}

#[test]
fn ac13_shift_and_ctrl_together_remove() {
    let (mut rig, [a, _, c, _]) = scene();
    rig.select(&[a, c]);
    rig.drag(pt(-5.0, -5.0), pt(25.0, 15.0), NONE, SHIFT_CTRL);
    assert_eq!(rig.selection.ids(), &[c]);
}

#[test]
fn ac9_marquee_replace_with_an_empty_result_clears() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    rig.drag(pt(100.0, 100.0), pt(120.0, 120.0), NONE, NONE);
    assert!(rig.selection.is_empty());
}

#[test]
fn ac9_marquee_uses_the_drawn_bounds_so_a_bulging_curve_is_not_contained() {
    // Anchors y 0..20 but the curve bulges to about y -9..29.
    let rig = Rig::empty();
    let id = rig.document.create_path(
        &[
            NewAnchor {
                handle_out: curvyo_document_core::Vec2::new(10.0, -36.0),
                ..NewAnchor::corner(AnchorId::new(7, 1), pt(10.04, 0.0))
            },
            NewAnchor {
                handle_in: curvyo_document_core::Vec2::new(-10.0, 36.0),
                ..NewAnchor::corner(AnchorId::new(7, 2), pt(50.0, 20.0))
            },
        ],
        false,
    );
    let objs = rig.objects();
    let ids = |mode| {
        objects_in_marquee(
            &objs,
            pt(0.0, -1.0),
            pt(60.0, 21.0),
            mode,
            Tolerance::from_mm(0.0),
        )
    };
    assert!(
        ids(MarqueeMode::Contain).is_empty(),
        "the curve leaves the box"
    );
    assert_eq!(ids(MarqueeMode::Touch), vec![id]);
    let big = objects_in_marquee(
        &objs,
        pt(-5.0, -30.0),
        pt(70.0, 40.0),
        MarqueeMode::Contain,
        Tolerance::from_mm(0.0),
    );
    assert_eq!(big, vec![id]);
}

#[test]
fn ac15_a_marquee_started_over_a_far_object_never_moves_it_even_via_escape() {
    let (mut rig, [a, ..]) = scene();
    let before = rig.doc_fingerprint();
    rig.press(pt(-6.0, -6.0), NONE);
    rig.moved(pt(40.0, 40.0), NONE);
    rig.tool.escape();
    rig.release(pt(40.0, 40.0), NONE);
    assert_eq!(rig.doc_fingerprint(), before);
    assert!(rig.selection.is_empty());
    let _ = a;
}

#[test]
fn ac14_escape_cancels_a_marquee_and_keeps_the_selection() {
    let (mut rig, [a, b, ..]) = scene();
    rig.select(&[b]);
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), NONE);
    assert!(rig.tool.live_gesture(pt(25.0, 15.0), NONE).is_some());
    rig.tool.escape();
    assert!(
        rig.tool.live_gesture(pt(25.0, 15.0), NONE).is_none(),
        "overlay gone"
    );
    rig.release(pt(25.0, 15.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[b],
        "a cancelled drag changes nothing"
    );
    let _ = a;
}

#[test]
fn ac8_plain_press_on_empty_canvas_then_escape_keeps_the_selection() {
    // Spec: the clear belongs to the click (press + release). Cancelling in
    // between must not have cleared it.
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    rig.press(pt(100.0, 100.0), NONE);
    rig.tool.escape();
    assert_eq!(rig.selection.ids(), &[a], "Escape after a bare press");
}

#[test]
fn ac8_clearing_happens_at_release_not_at_press() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    rig.press(pt(100.0, 100.0), NONE);
    assert_eq!(
        rig.selection.ids(),
        &[a],
        "still selected between press and release"
    );
    rig.release(pt(100.0, 100.0), NONE);
    assert!(rig.selection.is_empty());
}

// ---------------------------------------------------------------------
// AC 16 to 20: lasso
// ---------------------------------------------------------------------

#[test]
fn ac16_alt_press_arms_a_lasso_anywhere_and_changes_nothing_until_release() {
    let (mut rig, [a, b, ..]) = scene();
    rig.select(&[b]);
    let out = rig.press(pt(100.0, 100.0), ALT);
    assert_eq!(out, SelectPointerDownOutcome::Lasso);
    rig.moved(pt(60.0, 5.0), ALT);
    rig.moved(pt(-10.0, 5.0), ALT);
    assert_eq!(
        rig.selection.ids(),
        &[b],
        "no selection change during the drag"
    );
    let live = rig.tool.live_gesture(pt(-10.0, 5.0), ALT).expect("a line");
    match live.shape {
        GestureShape::Line(points) => assert!(points.len() >= 2),
        GestureShape::Box { .. } => panic!("a lasso is a line"),
    }
    rig.release(pt(-10.0, 5.0), ALT);
    assert!(rig.selection.ids().contains(&a));
}

#[test]
fn ac16_inside_the_dead_zone_there_is_no_line_yet() {
    let (mut rig, ..) = scene();
    rig.press(pt(100.0, 100.0), ALT);
    rig.moved(pt(100.5, 100.0), ALT); // 2 px
    assert!(rig.tool.live_gesture(pt(100.5, 100.0), ALT).is_none());
}

#[test]
fn ac17_alt_press_on_an_object_never_moves_it() {
    let (mut rig, [a, ..]) = scene();
    let before = rig.doc_fingerprint();
    let out = rig.press(pt(0.0, 5.0), ALT); // on A's outline
    assert_eq!(out, SelectPointerDownOutcome::Lasso);
    rig.moved(pt(-20.0, 5.0), ALT);
    rig.moved(pt(-20.0, 50.0), ALT);
    rig.release(pt(-20.0, 50.0), ALT);
    assert_eq!(rig.doc_fingerprint(), before, "A did not move");
    assert_eq!(rig.selection.ids(), &[a], "the line started on A's outline");
}

#[test]
fn ac17_alt_press_on_a_selected_objects_interior_or_handle_still_lassos() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    let before = rig.doc_fingerprint();
    // A's corner handle region (outside the box by a little, within 16 px).
    assert_eq!(
        rig.press(pt(-1.0, -1.0), ALT),
        SelectPointerDownOutcome::Lasso
    );
    rig.moved(pt(-30.0, -30.0), ALT);
    rig.release(pt(-30.0, -30.0), ALT);
    assert_eq!(rig.doc_fingerprint(), before);
    // Interior of the selected box.
    rig.select(&[a]);
    assert_eq!(
        rig.press(pt(5.0, 5.0), ALT),
        SelectPointerDownOutcome::Lasso
    );
    rig.moved(pt(5.0, 6.0), ALT);
    rig.moved(pt(5.0, 8.0), ALT);
    rig.release(pt(5.0, 8.0), ALT);
    assert_eq!(rig.doc_fingerprint(), before);
}

#[test]
fn ac18_a_line_selects_every_outline_it_comes_within_tolerance_of() {
    let (mut rig, [a, b, c, d]) = scene();
    rig.drag(pt(-10.0, 5.0), pt(60.0, 5.0), ALT, ALT);
    assert_eq!(rig.sel(), sorted(vec![a, b, c]));
    // Passing 1.9 mm below the tops of A, B, C: touches; 2.1: nothing.
    let (mut rig, [a, b, c, _]) = scene();
    rig.drag(pt(-10.0, -1.9), pt(60.0, -1.9), ALT, ALT);
    assert_eq!(rig.sel(), sorted(vec![a, b, c]));
    let (mut rig, ..) = scene();
    rig.select(&[d]);
    rig.drag(pt(-10.0, -2.1), pt(60.0, -2.1), ALT, ALT);
    assert!(
        rig.selection.is_empty(),
        "AC 20: plain Alt-drag touching nothing clears"
    );
}

#[test]
fn ac18_a_sparse_line_does_not_miss_a_thin_crossing() {
    // Only press, one far move and release: a 600 mm stretch. The crossing of
    // a 0 width outline must be found, not stepped over.
    let mut rig = Rig::empty();
    let l = rig.line(pt(300.0, -50.0), pt(300.0, 50.0));
    rig.press(pt(0.0, 0.0), ALT);
    rig.moved(pt(600.0, 0.3), ALT);
    rig.release(pt(600.0, 0.3), ALT);
    assert_eq!(rig.selection.ids(), &[l]);
}

#[test]
fn ac20_a_line_inside_a_primitive_that_never_crosses_its_outline_selects_nothing() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    rig.drag(pt(3.5, 5.0), pt(6.5, 5.0), ALT, ALT); // 3.5 mm from the nearest edges
    assert!(rig.selection.is_empty(), "cleared: it touched nothing");
}

#[test]
fn ac20_a_filled_shape_is_still_outline_only_for_the_lasso() {
    let (mut rig, [a, ..]) = scene();
    rig.fill(a);
    rig.drag(pt(3.5, 5.0), pt(6.5, 5.0), ALT, ALT);
    assert!(rig.selection.is_empty(), "spec: outline proximity only");
}

#[test]
fn ac19_20_lasso_combine_rules_and_empty_results() {
    let (_, ids0) = scene();
    let _ = ids0;
    for (m, rightish, expected) in [
        (ALT, vec![0usize], vec![0, 1, 2]), // replace: result only
        (ALT_SHIFT, vec![0usize], vec![0, 1, 2, 3]), // add to {D, A}
        (ALT_CTRL, vec![0usize], vec![3]),  // remove from {D, A}
    ] {
        let (mut rig, ids) = scene();
        rig.select(&[ids[3], ids[rightish[0]]]);
        // A line through A, B, C.
        rig.drag(pt(-10.0, 5.0), pt(60.0, 5.0), m, m);
        let want: Vec<NodeId> = {
            let result = [ids[0], ids[1], ids[2]];
            let initial = [ids[3], ids[rightish[0]]];
            let combine = SelectionCombine::from_modifiers(m.shift, m.ctrl);
            model_apply(&initial, &result, combine)
        };
        let _ = expected;
        assert_eq!(rig.sel(), want, "{m:?}");
    }
    // Ctrl wins over Shift for the lasso.
    let (mut rig, ids) = scene();
    rig.select(&[ids[0], ids[3]]);
    let both = Modifiers::new(true, true).with_alt(true);
    rig.drag(pt(-10.0, 5.0), pt(60.0, 5.0), both, both);
    assert_eq!(rig.selection.ids(), &[ids[3]]);
    // Empty result: plain Alt clears, Alt+Shift / Alt+Ctrl leave it alone.
    for (m, cleared) in [(ALT, true), (ALT_SHIFT, false), (ALT_CTRL, false)] {
        let (mut rig, ids) = scene();
        rig.select(&[ids[0], ids[3]]);
        rig.drag(pt(100.0, 100.0), pt(120.0, 120.0), m, m);
        assert_eq!(rig.selection.is_empty(), cleared, "{m:?}");
        if !cleared {
            assert_eq!(rig.sel(), sorted(vec![ids[0], ids[3]]));
        }
    }
}

#[test]
fn ac19_lasso_modifiers_are_read_at_release() {
    let (mut rig, ids) = scene();
    rig.select(&[ids[3]]);
    rig.press(pt(-10.0, 5.0), ALT);
    rig.moved(pt(25.0, 5.0), ALT);
    rig.moved(pt(60.0, 5.0), ALT_SHIFT);
    rig.release(pt(60.0, 5.0), ALT_SHIFT);
    assert_eq!(rig.sel(), sorted(vec![ids[0], ids[1], ids[2], ids[3]]));
}

#[test]
fn ac16_releasing_alt_mid_lasso_does_not_turn_it_into_a_box() {
    let (mut rig, [a, b, c, _]) = scene();
    rig.press(pt(-10.0, 5.0), ALT);
    rig.moved(pt(25.0, 5.0), NONE); // Alt released
    let live = rig
        .tool
        .live_gesture(pt(25.0, 5.0), NONE)
        .expect("still a gesture");
    assert!(matches!(live.shape, GestureShape::Line(_)), "still a lasso");
    rig.moved(pt(60.0, 5.0), NONE);
    rig.release(pt(60.0, 5.0), NONE);
    assert_eq!(rig.sel(), sorted(vec![a, b, c]));
}

#[test]
fn alt_pressed_after_a_box_started_does_not_make_it_a_lasso() {
    let (mut rig, ..) = scene();
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), ALT);
    let live = rig.tool.live_gesture(pt(25.0, 15.0), ALT).unwrap();
    assert!(matches!(live.shape, GestureShape::Box { .. }));
}

// ---------------------------------------------------------------------
// Interplay with transform handles and the press order
// ---------------------------------------------------------------------

#[test]
fn press_order_plain_inside_the_sole_selected_box_moves_shift_arms_the_add_marquee() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    assert_eq!(
        rig.press(pt(5.0, 5.0), NONE),
        SelectPointerDownOutcome::Selected,
        "move"
    );
    rig.tool.escape();
    assert_eq!(
        rig.press(pt(5.0, 5.0), SHIFT),
        SelectPointerDownOutcome::Marquee
    );
    rig.tool.escape();
    // Outside the box, well clear of the handles.
    assert_eq!(
        rig.press(pt(70.0, 70.0), NONE),
        SelectPointerDownOutcome::Marquee
    );
    rig.tool.escape();
}

#[test]
fn press_on_a_resize_handle_is_a_handle_drag_not_a_marquee() {
    let (mut rig, [a, ..]) = scene();
    rig.select(&[a]);
    // Bottom-right corner of A is (10, 10).
    assert_eq!(
        rig.press(pt(10.0, 10.0), NONE),
        SelectPointerDownOutcome::Handle
    );
    rig.tool.escape();
}

#[test]
fn a_press_on_another_objects_outline_within_8_px_selects_it_not_a_marquee() {
    let (mut rig, [_, b, ..]) = scene();
    // 1.9 mm left of B's left edge (x = 20).
    assert_eq!(
        rig.press(pt(18.1, 5.0), NONE),
        SelectPointerDownOutcome::Selected
    );
    rig.release(pt(18.1, 5.0), NONE);
    assert_eq!(rig.selection.ids(), &[b]);
    // 2.1 mm: empty canvas.
    let (mut rig, ..) = scene();
    assert_eq!(
        rig.press(pt(17.9 - 0.2, 5.0), NONE),
        SelectPointerDownOutcome::Marquee
    );
}

#[test]
fn moving_still_works_for_a_plain_press_and_shift_ctrl_variants() {
    let (mut rig, [a, ..]) = scene();
    rig.drag(pt(0.0, 5.0), pt(0.0, 25.0), NONE, NONE); // down 20 mm onto... D's top; fine
    let moved = rig.doc_fingerprint();
    let (rig2, _) = scene();
    assert_ne!(
        moved,
        rig2.doc_fingerprint(),
        "a plain drag on an outline moves"
    );
    let _ = a;
}

// ---------------------------------------------------------------------
// Pure functions: combine, mode, properties
// ---------------------------------------------------------------------

#[test]
fn selection_combine_from_modifiers_ctrl_wins() {
    assert_eq!(
        SelectionCombine::from_modifiers(false, false),
        SelectionCombine::Replace
    );
    assert_eq!(
        SelectionCombine::from_modifiers(true, false),
        SelectionCombine::Add
    );
    assert_eq!(
        SelectionCombine::from_modifiers(false, true),
        SelectionCombine::Remove
    );
    assert_eq!(
        SelectionCombine::from_modifiers(true, true),
        SelectionCombine::Remove
    );
}

#[test]
fn marquee_mode_for_drag_matches_the_spec() {
    let s = pt(10.0, 10.0);
    assert_eq!(
        MarqueeMode::for_drag(s, pt(20.0, 0.0), false),
        MarqueeMode::Contain
    );
    assert_eq!(
        MarqueeMode::for_drag(s, pt(0.0, 99.0), false),
        MarqueeMode::Touch
    );
    assert_eq!(
        MarqueeMode::for_drag(s, pt(10.0, 99.0), false),
        MarqueeMode::Touch
    );
    assert_eq!(
        MarqueeMode::for_drag(s, pt(20.0, 0.0), true),
        MarqueeMode::Touch
    );
    assert_eq!(
        MarqueeMode::for_drag(s, pt(0.0, 0.0), true),
        MarqueeMode::Contain
    );
    assert_eq!(
        MarqueeMode::for_drag(s, pt(10.0, 0.0), true),
        MarqueeMode::Contain
    );
}

fn ids_n(n: usize) -> Vec<NodeId> {
    let d = Document::new(1);
    (0..n)
        .map(|i| {
            d.create_rect(RectBounds {
                origin: pt(i as f64 * 12.0, 0.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            })
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_apply_algebra(init in prop::collection::vec(any::<bool>(), 8), res in prop::collection::vec(any::<bool>(), 8)) {
        let ids = ids_n(8);
        let initial: Vec<NodeId> = ids.iter().zip(&init).filter(|&(_, &b)| b).map(|(i, _)| *i).collect();
        let result: Vec<NodeId> = ids.iter().zip(&res).filter(|&(_, &b)| b).map(|(i, _)| *i).collect();
        for combine in [SelectionCombine::Replace, SelectionCombine::Add, SelectionCombine::Remove] {
            let mut s = ObjectSelection::new();
            s.set(&initial);
            s.apply(combine, &result);
            let mut got = s.ids().to_vec();
            got.sort_by_key(key);
            prop_assert_eq!(got, model_apply(&initial, &result, combine));
        }
        // Add then Remove of the same result leaves initial minus result.
        let mut s = ObjectSelection::new();
        s.set(&initial);
        s.apply(SelectionCombine::Add, &result);
        s.apply(SelectionCombine::Remove, &result);
        let mut got = s.ids().to_vec();
        got.sort_by_key(key);
        prop_assert_eq!(got, model_apply(&initial, &result, SelectionCombine::Remove));
        // Remove is idempotent; no duplicates ever.
        let mut s2 = ObjectSelection::new();
        s2.set(&initial);
        s2.apply(SelectionCombine::Add, &result);
        s2.apply(SelectionCombine::Add, &result);
        let mut v = s2.ids().to_vec();
        let n = v.len();
        v.sort_by_key(key);
        v.dedup();
        prop_assert_eq!(v.len(), n);
    }

    #[test]
    fn prop_marquee_matches_the_bbox_model(
        rects in prop::collection::vec((-20i32..60, -20i32..60, 1i32..20, 1i32..20), 1..14),
        bx in (-30i32..70, -30i32..70, -30i32..70, -30i32..70),
    ) {
        let doc = Document::new(1);
        let ids: Vec<NodeId> = rects.iter().map(|&(x, y, w, h)| doc.create_rect(RectBounds {
            origin: pt(f64::from(x), f64::from(y)),
            width: Length::from_mm(f64::from(w)),
            height: Length::from_mm(f64::from(h)),
        })).collect();
        let objs: Vec<ObjectSnapshot> = ids.iter().filter_map(|&i| doc.object(i)).collect();
        // Box edges at quarter offsets: never exactly on an integer edge.
        let from = pt(f64::from(bx.0) + 0.25, f64::from(bx.1) + 0.25);
        let to = pt(f64::from(bx.2) + 0.25, f64::from(bx.3) + 0.25);
        for mode in [MarqueeMode::Touch, MarqueeMode::Contain] {
            let got = objects_in_marquee(&objs, from, to, mode, Tolerance::from_mm(0.0));
            let want: Vec<NodeId> = rects.iter().zip(&ids)
                .filter(|&(&(x, y, w, h), _)| model_marquee(
                    (f64::from(x), f64::from(y), f64::from(x + w), f64::from(y + h)), from, to, mode))
                .map(|(_, &i)| i).collect();
            prop_assert_eq!(&got, &want, "{:?}", mode);
            // Symmetric in the corner order.
            prop_assert_eq!(objects_in_marquee(&objs, to, from, mode, Tolerance::from_mm(0.0)), got);
        }
        let touch = objects_in_marquee(&objs, from, to, MarqueeMode::Touch, Tolerance::from_mm(0.0));
        let contain = objects_in_marquee(&objs, from, to, MarqueeMode::Contain, Tolerance::from_mm(0.0));
        prop_assert!(contain.iter().all(|c| touch.contains(c)), "contain is a subset of touch");
    }

    #[test]
    fn prop_candidates_match_the_distance_model(
        rects in prop::collection::vec((-20i32..60, -20i32..60, 2i32..20, 2i32..20), 1..10),
        p in (-30i32..70, -30i32..70),
    ) {
        let doc = Document::new(1);
        let ids: Vec<NodeId> = rects.iter().map(|&(x, y, w, h)| doc.create_rect(RectBounds {
            origin: pt(f64::from(x), f64::from(y)),
            width: Length::from_mm(f64::from(w)),
            height: Length::from_mm(f64::from(h)),
        })).collect();
        let objs: Vec<ObjectSnapshot> = ids.iter().filter_map(|&i| doc.object(i)).collect();
        let point = pt(f64::from(p.0) + 0.37, f64::from(p.1) + 0.41);
        let dist = |&(x, y, w, h): &(i32, i32, i32, i32)| -> f64 {
            let (x0, y0, x1, y1) = (f64::from(x), f64::from(y), f64::from(x + w), f64::from(y + h));
            let dx = (x0 - point.x).max(point.x - x1).max(0.0);
            let dy = (y0 - point.y).max(point.y - y1).max(0.0);
            if dx > 0.0 || dy > 0.0 {
                dx.hypot(dy)
            } else {
                (point.x - x0).min(x1 - point.x).min(point.y - y0).min(y1 - point.y)
            }
        };
        let got = hit_test_objects(&objs, point, tol());
        let mut want: Vec<(f64, usize)> = rects.iter().enumerate()
            .map(|(i, r)| (dist(r), i)).filter(|(d, _)| *d <= TOL).collect();
        want.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
        let want_ids: Vec<NodeId> = want.iter().map(|&(_, i)| ids[i]).collect();
        // Same set; same order (distance-sorted; ties are topmost first).
        prop_assert_eq!(sorted(got.clone()), sorted(want_ids.clone()));
        let dists: Vec<f64> = got.iter().map(|g| dist(&rects[ids.iter().position(|i| i == g).unwrap()])).collect();
        prop_assert!(dists.windows(2).all(|w| w[0] <= w[1] + 1e-9), "nearest first: {dists:?}");
        prop_assert_eq!(hit_test_object(&objs, point, tol()), got.first().copied());
        // No duplicates.
        let mut d = got.clone();
        d.sort_by_key(key);
        d.dedup();
        prop_assert_eq!(d.len(), got.len());
        // The lasso with one point is the same set.
        let along = hit_test_objects_along(&objs, &[point], tol());
        prop_assert_eq!(sorted(along), sorted(got));
    }

    #[test]
    fn prop_lasso_matches_dense_sampling(
        rects in prop::collection::vec((-20i32..60, -20i32..60, 2i32..20, 2i32..20), 1..8),
        line in prop::collection::vec((-40i32..80, -40i32..80), 2..6),
    ) {
        let doc = Document::new(1);
        let ids: Vec<NodeId> = rects.iter().map(|&(x, y, w, h)| doc.create_rect(RectBounds {
            origin: pt(f64::from(x), f64::from(y)),
            width: Length::from_mm(f64::from(w)),
            height: Length::from_mm(f64::from(h)),
        })).collect();
        let objs: Vec<ObjectSnapshot> = ids.iter().filter_map(|&i| doc.object(i)).collect();
        let poly: Vec<Point> = line.iter().map(|&(x, y)| pt(f64::from(x) + 0.13, f64::from(y) + 0.29)).collect();
        let got = hit_test_objects_along(&objs, &poly, tol());
        let dist = |&(x, y, w, h): &(i32, i32, i32, i32), p: Point| -> f64 {
            let (x0, y0, x1, y1) = (f64::from(x), f64::from(y), f64::from(x + w), f64::from(y + h));
            let dx = (x0 - p.x).max(p.x - x1).max(0.0);
            let dy = (y0 - p.y).max(p.y - y1).max(0.0);
            if dx > 0.0 || dy > 0.0 { dx.hypot(dy) } else { (p.x - x0).min(x1 - p.x).min(p.y - y0).min(y1 - p.y) }
        };
        for (i, r) in rects.iter().enumerate() {
            let mut min_d = f64::INFINITY;
            for pair in poly.windows(2) {
                let n = 4000;
                for k in 0..=n {
                    let t = f64::from(k) / f64::from(n);
                    let p = pt(pair[0].x + (pair[1].x - pair[0].x) * t, pair[0].y + (pair[1].y - pair[0].y) * t);
                    min_d = min_d.min(dist(r, p));
                }
            }
            let has = got.contains(&ids[i]);
            if min_d <= TOL - 0.05 { prop_assert!(has, "rect {i} should be touched (min {min_d})"); }
            if min_d > TOL + 0.05 { prop_assert!(!has, "rect {i} should not be touched (min {min_d})"); }
        }
        // Results are in z-order.
        let order: Vec<usize> = got.iter().map(|g| ids.iter().position(|i| i == g).unwrap()).collect();
        prop_assert!(order.windows(2).all(|w| w[0] < w[1]));
    }
}

// ---------------------------------------------------------------------
// Degenerate and hostile input
// ---------------------------------------------------------------------

#[test]
fn zero_size_marquee_after_a_real_drag_back_to_the_press_point_does_not_panic() {
    let (mut rig, [a, ..]) = scene();
    rig.press(pt(5.0, 5.0 + 0.0), SHIFT); // inside A's bounds; A is unselected: Shift on interior of an unfilled rect is empty canvas
    rig.moved(pt(60.0, 60.0), SHIFT);
    rig.moved(pt(5.0, 5.0), SHIFT);
    rig.release(pt(5.0, 5.0), SHIFT);
    // No assertion on the outcome beyond sanity: either A (point inside the
    // bounds touches it) or unchanged.
    assert!(rig.selection.ids().len() <= 1);
    let _ = a;
}

#[test]
fn nan_and_infinite_corners_do_not_panic_in_the_pure_functions() {
    let (rig, _) = scene();
    let objs = rig.objects();
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
        -1e300,
        f64::MAX,
    ] {
        for mode in [MarqueeMode::Touch, MarqueeMode::Contain] {
            let _ = objects_in_marquee(&objs, pt(bad, 0.0), pt(10.0, bad), mode, tol());
            let _ = objects_in_marquee(&objs, pt(bad, bad), pt(bad, bad), mode, tol());
        }
        let _ = hit_test_objects(&objs, pt(bad, 1.0), tol());
        let _ = hit_test_objects(&objs, pt(1.0, bad), tol());
        let _ = hit_test_object(&objs, pt(bad, bad), tol());
    }
}

#[test]
fn huge_lasso_lines_finish_quickly_and_nan_points_do_not_panic() {
    let (rig, [a, ..]) = scene();
    let objs = rig.objects();
    let got = within(30, move || {
        let long = hit_test_objects_along(&objs, &[pt(-1e9, 5.0), pt(1e9, 5.0)], tol());
        let _ = hit_test_objects_along(&objs, &[pt(-1e300, 5.0), pt(1e300, 5.0)], tol());
        let _ = hit_test_objects_along(&objs, &[pt(f64::NAN, 5.0), pt(1.0, 5.0)], tol());
        let _ = hit_test_objects_along(&objs, &[pt(f64::INFINITY, 5.0), pt(1.0, 5.0)], tol());
        let _ = hit_test_objects_along(&objs, &[], tol());
        long
    });
    assert!(
        got.contains(&a),
        "a 2e9 mm line through y = 5 still touches A"
    );
}

#[test]
fn thousands_of_objects_marquee_lasso_and_candidates_stay_fast() {
    let rig = Rig::empty();
    for r in 0..40 {
        for c in 0..100 {
            let _ = rig.rect(f64::from(c) * 12.0, f64::from(r) * 12.0, 10.0, 10.0);
        }
    }
    let objs = rig.objects();
    assert_eq!(objs.len(), 4000);
    let (n_touch, n_contain, n_row, n_cand) = within(120, move || {
        let t = objects_in_marquee(
            &objs,
            pt(-1.0, -1.0),
            pt(2000.0, 2000.0),
            MarqueeMode::Touch,
            tol(),
        );
        let c = objects_in_marquee(
            &objs,
            pt(-1.0, -1.0),
            pt(2000.0, 2000.0),
            MarqueeMode::Contain,
            tol(),
        );
        let row = hit_test_objects_along(&objs, &[pt(-5.0, 5.0), pt(1300.0, 5.0)], tol());
        let cand = hit_test_objects(&objs, pt(10.9, 5.0), tol());
        (t.len(), c.len(), row.len(), cand.len())
    });
    assert_eq!(n_touch, 4000);
    assert_eq!(n_contain, 4000);
    assert_eq!(n_row, 100, "one row of 100 squares");
    assert!(
        n_cand >= 2,
        "the gap between two squares is two candidates, got {n_cand}"
    );
}

#[test]
fn many_pointer_moves_in_one_lasso_are_cheap() {
    let (mut rig, ..) = scene();
    rig.press(pt(-10.0, 100.0), ALT);
    within(60, move || {
        for i in 0..20_000 {
            let x = -10.0 + f64::from(i) * 0.01;
            rig.moved(pt(x, 100.0 + (f64::from(i) * 0.1).sin()), ALT);
            if i % 2000 == 0 {
                let _ = rig.tool.live_gesture(pt(x, 100.0), ALT);
            }
        }
        rig.release(pt(200.0, 100.0), ALT);
        assert!(rig.selection.is_empty());
    });
}

#[test]
fn a_stale_object_vanishing_mid_marquee_does_not_panic() {
    let (mut rig, [a, b, ..]) = scene();
    rig.select(&[a, b]);
    rig.press(pt(70.0, 70.0), SHIFT);
    rig.moved(pt(-5.0, -5.0), SHIFT);
    rig.document.delete_objects(&[a]).ok();
    rig.release(pt(-5.0, -5.0), SHIFT);
    assert!(!rig.selection.ids().contains(&a) || rig.selection.ids().len() <= 4);
}

fn key(id: &NodeId) -> String {
    format!("{id:?}")
}
