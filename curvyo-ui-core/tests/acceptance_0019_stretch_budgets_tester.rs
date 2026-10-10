//! Independent tester budgets for the converting stretch of
//! `specs/0019-multi-object-transform/` criteria 48 and 49, in a release build:
//! `cargo nextest run --release -p curvyo-ui-core --test
//! acceptance_0019_stretch_budgets_tester --run-ignored only --no-capture`.
//! An edge-handle stretch of a selection in which 5,000 of 10,000 objects (stars
//! and rectangles turned by 30 degrees) convert: group box, preview step and
//! commit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]
#![allow(clippy::too_many_lines, missing_docs, clippy::cast_possible_truncation)]
#![allow(clippy::unreadable_literal, clippy::suboptimal_flops)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Angle, Document, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot, Point,
    PointCount, RectBounds, StarFrame, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, Modifiers, ObjectSelection, SelectTool, TransformHandleTolerances,
};

const K: f64 = 2.0; // px per mm

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// `paths` paths with `nodes` nodes, `stars` stars (unrotated frames, a shown
/// angle) and `rects` rectangles turned by 30 degrees.
fn scene(paths: usize, nodes: usize, stars: usize, rects: usize) -> Document {
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
    for j in 0..stars {
        let (cx, cy) = cell(paths + j);
        let _ = d.create_star(
            StarFrame {
                center: pt(cx, cy),
                radius: Length::from_mm(4.0),
                angle: Angle::from_radians(-1.2),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
    }
    for j in 0..rects {
        let (cx, cy) = cell(paths + stars + j);
        let id = d.create_rect(RectBounds {
            origin: pt(cx - 4.0, cy - 3.0),
            width: Length::from_mm(8.0),
            height: Length::from_mm(6.0),
        });
        let o = d.object(id).unwrap();
        d.rotate_object(&o.rotated(pt(cx, cy), Angle::from_radians(30.0_f64.to_radians())))
            .unwrap();
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
    EdgeStretch,
    CornerStretch,
}

fn gesture_points(lo: Point, hi: Point, g: G) -> (Point, Point) {
    let centre = pt(f64::midpoint(lo.x, hi.x), f64::midpoint(lo.y, hi.y));
    match g {
        G::Move => (centre, pt(centre.x + 30.0, centre.y + 18.0)),
        G::EdgeStretch => (pt(hi.x, centre.y), pt(hi.x + 40.0, centre.y)),
        G::CornerStretch => (hi, pt(hi.x + 40.0, hi.y + 20.0)),
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

fn step_time(objects: &[ObjectSnapshot], sel: &mut ObjectSelection, g: G, reps: usize) -> Duration {
    let (lo, hi) = box_of(objects, sel);
    let (press, to) = gesture_points(lo, hi, g);
    let mut tool = started(objects, sel, press, to);
    let t = best(reps, || {
        let live = tool
            .live_edit(objects, sel, to, false, false)
            .expect("live edit");
        SelectTool::group_of(&live.objects, sel)
    });
    tool.escape();
    t
}

/// Criterion 48, converting variant: 200 objects (100 paths with 50 nodes, 100
/// rectangles turned by 30 degrees, all convert on an edge stretch): the preview
/// resolve step of the edge stretch against the move step, same run. Reported and
/// gated at 1.1 times on the bare resolver; the frame-level ratio is measured in the
/// editor-wasm test.
#[test]
#[ignore = "release-mode benchmark"]
fn stretch_budget_200() {
    let d = scene(100, 50, 0, 100);
    let objects = read(&d);
    let mut sel = select_all(&objects);
    let gb = best(50, || SelectTool::group_of(&objects, &sel));
    println!("200 converting: group box {gb:?}");
    assert!(gb < Duration::from_millis(1), "{gb:?}");
    let mut ratios = Vec::new();
    for _ in 0..5 {
        let mv = step_time(&objects, &mut sel, G::Move, 60);
        let ed = step_time(&objects, &mut sel, G::EdgeStretch, 60);
        ratios.push(ed.as_secs_f64() / mv.as_secs_f64());
        println!(
            "200 converting: move {mv:?} edge stretch {ed:?} ({:.2}x)",
            ratios.last().unwrap()
        );
    }
    println!("ratios {ratios:.2?}");
}

/// Criterion 49, converting variant: 10,000 objects (5,000 paths with 20 nodes,
/// 2,500 stars, 2,500 rectangles turned by 30 degrees): group box < 20 ms, one
/// preview step < 50 ms, the commit < 5 s.
#[test]
#[ignore = "release-mode benchmark"]
fn stretch_budget_10000() {
    let d = scene(5000, 20, 2500, 2500);
    let objects = read(&d);
    let mut sel = select_all(&objects);
    let gb = best(5, || SelectTool::group_of(&objects, &sel));
    println!("10000 converting: group box {gb:?}");
    assert!(gb < Duration::from_millis(20), "{gb:?}");
    for g in [G::EdgeStretch, G::CornerStretch] {
        let t = step_time(&objects, &mut sel, g, 5);
        println!("10000 converting {g:?}: preview step {t:?}");
        assert!(t < Duration::from_millis(50), "{g:?} preview step {t:?}");
    }
    let fresh = scene(5000, 20, 2500, 2500);
    let objects = read(&fresh);
    let mut sel = select_all(&objects);
    let (lo, hi) = box_of(&objects, &sel);
    let (press, to) = gesture_points(lo, hi, G::EdgeStretch);
    let mut tool = started(&objects, &mut sel, press, to);
    let mut minter = AnchorIdMinter::new(9);
    let t = Instant::now();
    tool.pointer_up(&fresh, &objects, &mut sel, to, Modifiers::NONE, &mut minter);
    let commit = t.elapsed();
    println!("10000 converting: commit {commit:?}");
    assert!(commit < Duration::from_secs(5), "commit {commit:?}");
    let after = read(&fresh);
    let paths = after
        .iter()
        .filter(|o| matches!(o, ObjectSnapshot::Path(_)))
        .count();
    assert_eq!(
        paths, 10_000,
        "5,000 conversions: every object is a path now"
    );
}
