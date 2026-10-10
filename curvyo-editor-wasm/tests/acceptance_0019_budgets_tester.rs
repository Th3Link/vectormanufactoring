//! Independent tester budgets for `specs/0019-multi-object-transform/`
//! criteria 48 and 49, in a release build:
//! `cargo nextest run --release -p curvyo-editor-wasm --test
//! acceptance_0019_budgets_tester --run-ignored only --no-capture`.
//! Own scene generators, own timing; the numbers are printed so that the lead
//! can quote them. All tests are `#[ignore]` (they need `--release`).

// Test code: byte buffers are compared with `assert!(a == b, "msg")` on purpose
// (a failing `assert_eq!` would print the whole snapshot), and the arithmetic is
// written the way the specification states it.
#![allow(
    clippy::manual_assert_eq,
    clippy::manual_midpoint,
    clippy::collapsible_if
)]
#![allow(clippy::unneeded_wildcard_pattern, clippy::unreadable_literal)]
#![allow(clippy::used_underscore_binding, clippy::useless_conversion)]
#![allow(clippy::suboptimal_flops, clippy::imprecise_flops)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]
#![allow(clippy::too_many_lines, missing_docs, clippy::cast_possible_truncation)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, pack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{ObjectSelection, SelectTool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const SIDE: f64 = 6.0;
const PITCH: f64 = 10.0;

/// A closed path with `nodes` anchors walking the perimeter of a square of
/// side 6 mm whose corners are all nodes: the bounds are exactly the square.
fn square_path(d: &Document, cx: f64, cy: f64, nodes: usize, first_id: u64) {
    let per_side = nodes / 4;
    let extra = nodes - per_side * 4;
    let h = SIDE / 2.0;
    let corners = [(-h, -h), (h, -h), (h, h), (-h, h)];
    let mut pts = Vec::with_capacity(nodes);
    for side in 0..4 {
        let (ax, ay) = corners[side];
        let (bx, by) = corners[(side + 1) % 4];
        let n = per_side + usize::from(side == 0) * extra;
        for k in 0..n {
            let t = k as f64 / n as f64;
            pts.push((cx + ax + (bx - ax) * t, cy + ay + (by - ay) * t));
        }
    }
    let anchors: Vec<NewAnchor> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, first_id + i as u64), pt(*x, *y)))
        .collect();
    let _ = d.create_path(&anchors, true);
}

/// `paths` paths with `nodes` nodes and `rects` squares on a grid of 100 columns.
fn scene(paths: usize, nodes: usize, rects: usize) -> Document {
    let d = Document::new(1);
    let cell = |i: usize| {
        (
            5.0 + (i % 100) as f64 * PITCH,
            5.0 + (i / 100) as f64 * PITCH,
        )
    };
    for i in 0..paths {
        let (cx, cy) = cell(i);
        square_path(&d, cx, cy, nodes, (i * nodes) as u64 + 1);
    }
    for j in 0..rects {
        let (cx, cy) = cell(paths + j);
        let _ = d.create_rect(RectBounds {
            origin: pt(cx - SIDE / 2.0, cy - SIDE / 2.0),
            width: Length::from_mm(SIDE),
            height: Length::from_mm(SIDE),
        });
    }
    d
}

fn extent(n: usize) -> (Point, Point) {
    let cell = |i: usize| {
        (
            5.0 + (i % 100) as f64 * PITCH,
            5.0 + (i / 100) as f64 * PITCH,
        )
    };
    let (x1, _) = cell(n.min(100) - 1);
    let rows = (n - 1) / 100;
    let (_, y1) = cell(rows * 100);
    (
        pt(5.0 - SIDE / 2.0, 5.0 - SIDE / 2.0),
        pt(x1 + SIDE / 2.0, y1 + SIDE / 2.0),
    )
}

fn session_with_everything_selected(d: &Document, n: usize) -> (Session, Point, Point, f64) {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1600.0, 1000.0);
    s.pointer_hover(pt(-20.0, -20.0), false, false);
    s.pointer_down(pt(-20.0, -20.0), false);
    s.pointer_hover(pt(1300.0, 1300.0), false, false);
    s.pointer_up(pt(1300.0, 1300.0), false, false);
    assert_eq!(s.selected_object_count(), n);
    let (lo, hi) = extent(n);
    let k = s_scale(&s);
    (s, lo, hi, k)
}

fn s_scale(s: &Session) -> f64 {
    s.view().scale()
}

fn best<F: FnMut() -> Duration>(mut f: F, reps: usize) -> Duration {
    (0..reps).map(|_| f()).min().unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Gesture {
    Move,
    Scale,
    Rotate,
    Skew,
}

/// One preview step of `gesture`: the press, one hover to a different point,
/// optionally a draw list, and the Escape that cancels.
fn preview_step(
    s: &mut Session,
    lo: Point,
    hi: Point,
    k: f64,
    gesture: Gesture,
    with_draw: bool,
) -> Duration {
    let centre = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    let (from, to) = match gesture {
        Gesture::Move => (centre, pt(centre.x + 12.0, centre.y + 7.0)),
        Gesture::Scale => (hi, pt(hi.x + 15.0, hi.y + 9.0)),
        Gesture::Rotate => {
            let d = 32.0 / std::f64::consts::SQRT_2 / k;
            (pt(hi.x + d, hi.y + d), pt(hi.x + d + 25.0, hi.y + d - 10.0))
        }
        Gesture::Skew => {
            let top = pt(centre.x, lo.y - 16.0 / k);
            (top, pt(top.x + 14.0, top.y))
        }
    };
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    let t = Instant::now();
    s.pointer_hover(to, false, false);
    if with_draw {
        std::hint::black_box(s.draw_list());
    }
    let dt = t.elapsed();
    s.escape();
    s.pointer_up(to, false, false);
    dt
}

/// Criterion 48: 200 objects (100 paths with 50 nodes, 100 rectangles): a
/// preview frame of scale and rotate is at most 1.1 times a move frame; skew on
/// 200 paths against a move of the same 200 paths. The group box computes in
/// under 1 ms.
#[test]
#[ignore = "release-mode benchmark"]
fn budget_200_objects() {
    let d = scene(100, 50, 100);
    let (mut s, lo, hi, k) = session_with_everything_selected(&d, 200);
    // Warm up.
    for g in [Gesture::Move, Gesture::Scale, Gesture::Rotate] {
        preview_step(&mut s, lo, hi, k, g, true);
    }
    let mv = best(|| preview_step(&mut s, lo, hi, k, Gesture::Move, true), 15);
    let sc = best(|| preview_step(&mut s, lo, hi, k, Gesture::Scale, true), 15);
    let ro = best(
        || preview_step(&mut s, lo, hi, k, Gesture::Rotate, true),
        15,
    );
    println!(
        "200 mixed: move {mv:?}, scale {sc:?} ({:.2}x), rotate {ro:?} ({:.2}x)",
        sc.as_secs_f64() / mv.as_secs_f64(),
        ro.as_secs_f64() / mv.as_secs_f64()
    );
    assert!(
        sc.as_secs_f64() <= mv.as_secs_f64() * 1.1,
        "scale {sc:?} vs move {mv:?}"
    );
    assert!(
        ro.as_secs_f64() <= mv.as_secs_f64() * 1.1,
        "rotate {ro:?} vs move {mv:?}"
    );
    // The hover step alone (no draw list), which is what the gesture resolver costs.
    let mv0 = best(|| preview_step(&mut s, lo, hi, k, Gesture::Move, false), 15);
    let sc0 = best(
        || preview_step(&mut s, lo, hi, k, Gesture::Scale, false),
        15,
    );
    let ro0 = best(
        || preview_step(&mut s, lo, hi, k, Gesture::Rotate, false),
        15,
    );
    println!("200 mixed, resolve only: move {mv0:?}, scale {sc0:?}, rotate {ro0:?}");

    let dp = scene(200, 50, 0);
    let (mut s, lo, hi, k) = session_with_everything_selected(&dp, 200);
    for g in [Gesture::Move, Gesture::Skew] {
        preview_step(&mut s, lo, hi, k, g, true);
    }
    let mv = best(|| preview_step(&mut s, lo, hi, k, Gesture::Move, true), 15);
    let sk = best(|| preview_step(&mut s, lo, hi, k, Gesture::Skew, true), 15);
    println!(
        "200 paths: move {mv:?}, skew {sk:?} ({:.2}x)",
        sk.as_secs_f64() / mv.as_secs_f64()
    );
    assert!(
        sk.as_secs_f64() <= mv.as_secs_f64() * 1.1,
        "skew {sk:?} vs move {mv:?}"
    );

    // Group box under 1 ms.
    let objects: Vec<ObjectSnapshot> = d
        .object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect();
    let mut sel = ObjectSelection::new();
    sel.set(&d.object_ids());
    let t = best(
        || {
            let t = Instant::now();
            std::hint::black_box(SelectTool::group_of(&objects, &sel));
            t.elapsed()
        },
        20,
    );
    println!("200: group box {t:?}");
    assert!(t < Duration::from_millis(1), "group box {t:?}");
}

/// Criterion 49: 10,000 objects (5,000 paths with 20 nodes, 5,000 rectangles):
/// the group box under 20 ms, one preview step under 50 ms, each commit within
/// 5 s; for skew 10,000 paths.
#[test]
#[ignore = "release-mode benchmark"]
fn budget_10000_objects() {
    let d = scene(5000, 20, 5000);
    let objects: Vec<ObjectSnapshot> = d
        .object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect();
    let mut sel = ObjectSelection::new();
    sel.set(&d.object_ids());
    let t = best(
        || {
            let t = Instant::now();
            std::hint::black_box(SelectTool::group_of(&objects, &sel));
            t.elapsed()
        },
        3,
    );
    println!("10000: group box {t:?}");
    assert!(t < Duration::from_millis(20), "group box {t:?}");

    for g in [Gesture::Move, Gesture::Scale, Gesture::Rotate] {
        let (mut s, lo, hi, k) = session_with_everything_selected(&d, 10_000);
        let step = best(|| preview_step(&mut s, lo, hi, k, g, false), 5);
        // Reported, not gated: the Session step includes reading all 10,000
        // objects out of the document (criterion 49 gates the ui-core step in
        // `curvyo-ui-core/tests/acceptance_0019_budgets_tester.rs`).
        println!("10000 mixed {g:?}: Session preview step (end to end, no draw) {step:?}");
        // Commit.
        let (mut s, lo, hi, k) = session_with_everything_selected(&d, 10_000);
        let centre = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let (from, to) = match g {
            Gesture::Move => (centre, pt(centre.x + 12.0, centre.y + 7.0)),
            Gesture::Scale => (hi, pt(hi.x + 15.0, hi.y + 9.0)),
            _ => {
                let dd = 32.0 / std::f64::consts::SQRT_2 / k;
                (
                    pt(hi.x + dd, hi.y + dd),
                    pt(hi.x + dd + 25.0, hi.y + dd - 10.0),
                )
            }
        };
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(to, false, false);
        let slow = s.release_is_slow();
        let t = Instant::now();
        s.pointer_up(to, false, false);
        let commit = t.elapsed();
        println!("10000 mixed {g:?}: commit {commit:?}, release_is_slow {slow}");
        assert!(commit < Duration::from_secs(5), "{g:?} commit {commit:?}");
        assert!(slow, "{g:?}: the host must be told the release is slow");
    }

    let dp = scene(10_000, 20, 0);
    let (mut s, lo, hi, k) = session_with_everything_selected(&dp, 10_000);
    let step = best(|| preview_step(&mut s, lo, hi, k, Gesture::Skew, false), 5);
    println!("10000 paths skew: Session preview step (end to end, no draw) {step:?}");
    let (mut s, lo, _hi, k) = session_with_everything_selected(&dp, 10_000);
    let top = pt((lo.x + _hi.x) / 2.0, lo.y - 16.0 / k);
    s.pointer_hover(top, false, false);
    s.pointer_down(top, false);
    s.pointer_hover(pt(top.x + 14.0, top.y), false, false);
    let t = Instant::now();
    s.pointer_up(pt(top.x + 14.0, top.y), false, false);
    let commit = t.elapsed();
    println!("10000 paths skew: commit {commit:?}");
    assert!(commit < Duration::from_secs(5), "skew commit {commit:?}");
}
