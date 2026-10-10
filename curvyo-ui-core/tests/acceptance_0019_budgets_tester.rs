//! Independent tester budgets for `specs/0019-multi-object-transform/` criteria
//! 48 and 49 at the `curvyo-ui-core` level, where the specification defines the
//! costs (the gesture resolving function and the preview box; the group box;
//! the commit): own scene generators, own timing, release mode only:
//! `cargo nextest run --release -p curvyo-ui-core --test acceptance_0019_budgets_tester
//! --run-ignored only --no-capture`.

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
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, Modifiers, ObjectSelection, SelectTool, TransformHandleTolerances,
};

const K: f64 = 2.0; // px per mm

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn scene(paths: usize, nodes: usize, rects: usize) -> Document {
    let d = Document::new(1);
    let cell = |i: usize| (5.0 + (i % 100) as f64 * 12.0, 5.0 + (i / 100) as f64 * 12.0);
    for i in 0..paths {
        let (cx, cy) = cell(i);
        let h = 4.0;
        let corners = [(-h, -h), (h, -h), (h, h), (-h, h)];
        let per = nodes / 4;
        let extra = nodes - per * 4;
        let mut anchors = Vec::new();
        for side in 0..4 {
            let (ax, ay) = corners[side];
            let (bx, by) = corners[(side + 1) % 4];
            let n = per + usize::from(side == 0) * extra;
            for k in 0..n {
                let t = k as f64 / n as f64;
                anchors.push(NewAnchor::corner(
                    AnchorId::new(1, (i * nodes + anchors.len()) as u64 + 1),
                    pt(cx + ax + (bx - ax) * t, cy + ay + (by - ay) * t),
                ));
            }
        }
        let _ = d.create_path(&anchors, true);
    }
    for j in 0..rects {
        let (cx, cy) = cell(paths + j);
        let _ = d.create_rect(RectBounds {
            origin: pt(cx - 4.0, cy - 4.0),
            width: Length::from_mm(8.0),
            height: Length::from_mm(8.0),
        });
    }
    d
}

fn read(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn select_all(objects: &[ObjectSnapshot]) -> ObjectSelection {
    let ids: Vec<NodeId> = objects.iter().map(ObjectSnapshot::id).collect();
    let mut s = ObjectSelection::new();
    s.set(&ids);
    s
}

fn best<T>(reps: usize, mut f: impl FnMut() -> T) -> Duration {
    (0..reps)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(f());
            t.elapsed()
        })
        .min()
        .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum G {
    Move,
    Scale,
    Rotate,
    Skew,
}

fn gesture_points(lo: Point, hi: Point, g: G) -> (Point, Point) {
    let centre = pt(f64::midpoint(lo.x, hi.x), f64::midpoint(lo.y, hi.y));
    let d = 32.0 / K / std::f64::consts::SQRT_2;
    match g {
        G::Move => (centre, pt(centre.x + 30.0, centre.y + 18.0)),
        G::Scale => (hi, pt(hi.x + 40.0, hi.y + 20.0)),
        G::Rotate => (pt(hi.x + d, hi.y + d), pt(hi.x - 100.0, hi.y + 60.0)),
        G::Skew => {
            let top = pt(centre.x, lo.y - 16.0 / K);
            (top, pt(top.x + 50.0, top.y))
        }
    }
}

fn started(
    objects: &[ObjectSnapshot],
    selection: &mut ObjectSelection,
    press: Point,
    to: Point,
) -> SelectTool {
    let mut tool = SelectTool::new();
    tool.pointer_down(
        objects,
        selection,
        press,
        Tolerance::from_mm(4.0),
        TransformHandleTolerances::at_scale(K),
        Modifiers::NONE,
    );
    tool.pointer_moved(to, Modifiers::NONE, selection);
    tool
}

fn box_of(objects: &[ObjectSnapshot], selection: &ObjectSelection) -> (Point, Point) {
    let g = SelectTool::group_of(objects, selection).unwrap();
    (g.bounds().min, g.bounds().max)
}

/// Criterion 48: 200 objects: the group box under 1 ms; scale and rotate
/// preview steps within 1.1 times a move preview step (same run, same inputs);
/// skew on 200 paths against a move of the same.
#[test]
#[ignore = "release-mode benchmark"]
fn budget_200() {
    let d = scene(100, 50, 100);
    let objects = read(&d);
    let mut sel = select_all(&objects);
    let gb = best(50, || SelectTool::group_of(&objects, &sel));
    println!("200: group box {gb:?}");
    assert!(gb < Duration::from_millis(1), "{gb:?}");
    let (lo, hi) = box_of(&objects, &sel);
    let step = |g: G, objects: &[ObjectSnapshot], sel: &mut ObjectSelection, lo, hi| {
        let (press, to) = gesture_points(lo, hi, g);
        let tool = started(objects, sel, press, to);
        let t = best(60, || {
            let live = tool
                .live_edit(objects, sel, to, false, false)
                .expect("live edit");
            SelectTool::group_of(&live.objects, sel)
        });
        let mut tool = tool;
        tool.escape();
        t
    };
    let mv = step(G::Move, &objects, &mut sel, lo, hi);
    let sc = step(G::Scale, &objects, &mut sel, lo, hi);
    let ro = step(G::Rotate, &objects, &mut sel, lo, hi);
    println!(
        "200 mixed: move {mv:?} scale {sc:?} ({:.2}x) rotate {ro:?} ({:.2}x)",
        sc.as_secs_f64() / mv.as_secs_f64(),
        ro.as_secs_f64() / mv.as_secs_f64()
    );
    // Reported, not gated here: criterion 48 gates the whole preview *frame*
    // (resolve plus draw list), which the Session-level budget test
    // `curvyo-editor-wasm/tests/acceptance_0019_budgets_tester.rs` measures.
    // This is the bare resolving function.
    let dp = scene(200, 50, 0);
    let objects = read(&dp);
    let mut sel = select_all(&objects);
    let (lo, hi) = box_of(&objects, &sel);
    let mv = step(G::Move, &objects, &mut sel, lo, hi);
    let sk = step(G::Skew, &objects, &mut sel, lo, hi);
    println!(
        "200 paths: move {mv:?} skew {sk:?} ({:.2}x)",
        sk.as_secs_f64() / mv.as_secs_f64()
    );
}

/// Criterion 49: 10,000 objects: group box under 20 ms; one preview step under
/// 50 ms for scale, rotate (and skew on paths); every commit within 5 s.
#[test]
#[ignore = "release-mode benchmark"]
fn budget_10000() {
    let d = scene(5000, 20, 5000);
    let objects = read(&d);
    let mut sel = select_all(&objects);
    let gb = best(5, || SelectTool::group_of(&objects, &sel));
    println!("10000: group box {gb:?}");
    assert!(gb < Duration::from_millis(20), "{gb:?}");
    let (lo, hi) = box_of(&objects, &sel);
    for g in [G::Move, G::Scale, G::Rotate] {
        let (press, to) = gesture_points(lo, hi, g);
        let mut tool = started(&objects, &mut sel, press, to);
        let t = best(5, || {
            let live = tool
                .live_edit(&objects, &sel, to, false, false)
                .expect("live edit");
            SelectTool::group_of(&live.objects, &sel)
        });
        println!("10000 {g:?}: preview step {t:?}");
        if !matches!(g, G::Move) {
            assert!(t < Duration::from_millis(50), "{g:?} preview step {t:?}");
        }
        tool.escape();
    }
    for g in [G::Move, G::Scale, G::Rotate] {
        let fresh = scene(5000, 20, 5000);
        let objects = read(&fresh);
        let mut sel = select_all(&objects);
        let (lo, hi) = box_of(&objects, &sel);
        let (press, to) = gesture_points(lo, hi, g);
        let mut tool = started(&objects, &mut sel, press, to);
        let mut minter = AnchorIdMinter::new(9);
        let t = Instant::now();
        tool.pointer_up(&fresh, &objects, &mut sel, to, Modifiers::NONE, &mut minter);
        let commit = t.elapsed();
        println!("10000 {g:?}: commit {commit:?}");
        assert!(commit < Duration::from_secs(5), "{g:?} commit {commit:?}");
        // Something happened.
        let after = read(&fresh);
        assert!(
            after.iter().zip(&objects).any(|(a, b)| a != b),
            "{g:?} wrote nothing"
        );
    }
    // Skew: paths only.
    let fresh = scene(10_000, 20, 0);
    let objects = read(&fresh);
    let mut sel = select_all(&objects);
    let (lo, hi) = box_of(&objects, &sel);
    let (press, to) = gesture_points(lo, hi, G::Skew);
    let mut tool = started(&objects, &mut sel, press, to);
    let t = best(5, || {
        let live = tool
            .live_edit(&objects, &sel, to, false, false)
            .expect("live edit");
        SelectTool::group_of(&live.objects, &sel)
    });
    println!("10000 paths skew: preview step {t:?}");
    assert!(t < Duration::from_millis(50), "skew preview step {t:?}");
    let mut minter = AnchorIdMinter::new(9);
    let t = Instant::now();
    tool.pointer_up(&fresh, &objects, &mut sel, to, Modifiers::NONE, &mut minter);
    let commit = t.elapsed();
    println!("10000 paths skew: commit {commit:?}");
    assert!(commit < Duration::from_secs(5), "skew commit {commit:?}");
}
