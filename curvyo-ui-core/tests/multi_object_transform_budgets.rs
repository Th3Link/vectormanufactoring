//! The costs of `specs/0019-multi-object-transform/` criteria 48 and 49 (the
//! architect's decision 3) at 200 and 10,000 selected objects: the group box,
//! one preview step (resolving the gesture for every object plus the preview
//! box) and the commit of a move, scale, rotate and skew. Gated: the group box
//! (under 1 ms at 200 objects, under 20 ms at 10,000), one preview step (under
//! 50 ms at 10,000) and every commit (within 5 s). Reported, not gated: the copy
//! commit and the bytes a scale adds to the saved file. Run in release by the
//! tester: `cargo test --release -p curvyo-ui-core --test
//! multi_object_transform_budgets -- --ignored --nocapture --test-threads 1`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    missing_docs
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, GroupSelection, Modifiers, ObjectSelection, SelectTool,
    TransformHandleTolerances,
};

const SCALE: f64 = 1.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// `paths` closed paths of `nodes` anchors on a grid, then `rects` squares.
fn build(paths: u32, nodes: u32, rects: u32) -> Document {
    let document = Document::new(1);
    let columns = 100u32;
    for i in 0..paths {
        let (cx, cy) = (f64::from(i % columns) * 30.0, f64::from(i / columns) * 30.0);
        let anchors: Vec<NewAnchor> = (0..nodes)
            .map(|n| {
                let a = f64::from(n) / f64::from(nodes) * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * u64::from(nodes) + u64::from(n)),
                    pt(cx + 10.0 * a.cos(), cy + 10.0 * a.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
    }
    for i in 0..rects {
        let (x, y) = (
            f64::from(i % columns) * 30.0,
            f64::from(paths.div_ceil(columns) + 1 + i / columns) * 30.0,
        );
        let _ = document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(12.0),
            height: Length::from_mm(12.0),
        });
    }
    document
}

fn read(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn selection_of(objects: &[ObjectSnapshot]) -> ObjectSelection {
    let ids: Vec<NodeId> = objects.iter().map(ObjectSnapshot::id).collect();
    let mut selection = ObjectSelection::new();
    selection.set(&ids);
    selection
}

fn mean<T>(runs: u32, mut f: impl FnMut() -> T) -> Duration {
    let began = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    began.elapsed() / runs
}

/// The handle spots of a group of `objects` at 1 px per mm.
struct Spots {
    low: Point,
    high: Point,
}

fn spots(objects: &[ObjectSnapshot], selection: &ObjectSelection) -> Spots {
    let group = SelectTool::group_of(objects, selection).unwrap();
    let b = group.bounds();
    Spots {
        low: b.min,
        high: b.max,
    }
}

/// One gesture: press at `press`, move to `to`, release; the release's wall time.
fn gesture(
    document: &Document,
    objects: &[ObjectSnapshot],
    selection: &mut ObjectSelection,
    press: Point,
    to: Point,
    modifiers: Modifiers,
) -> Duration {
    let tolerances = TransformHandleTolerances::at_scale(SCALE);
    let mut tool = SelectTool::new();
    let mut minter = AnchorIdMinter::new(9);
    tool.pointer_down(
        objects,
        selection,
        press,
        Tolerance::from_mm(2.0),
        tolerances,
        modifiers,
    );
    tool.pointer_moved(to, modifiers, selection);
    let began = Instant::now();
    tool.pointer_up(document, objects, selection, to, modifiers, &mut minter);
    began.elapsed()
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn the_group_box_of_200_objects_takes_under_1_ms() {
    let document = build(100, 50, 100);
    let objects = read(&document);
    let selection = selection_of(&objects);
    let took = mean(200, || SelectTool::group_of(&objects, &selection));
    println!("group box of 200 objects: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took < Duration::from_millis(1), "{took:?}");
    }
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn the_group_box_of_10000_objects_takes_under_20_ms() {
    let document = build(5000, 20, 5000);
    let objects = read(&document);
    let selection = selection_of(&objects);
    let took = mean(10, || {
        GroupSelection::from_objects(&objects, selection.ids())
    });
    println!("group box of 10,000 objects: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took < Duration::from_millis(20), "{took:?}");
    }
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn one_preview_step_of_10000_objects_takes_under_50_ms() {
    let document = build(5000, 20, 5000);
    let objects = read(&document);
    let mut selection = selection_of(&objects);
    let at = spots(&objects, &selection);
    let tolerances = TransformHandleTolerances::at_scale(SCALE);
    let offset = 32.0 / SCALE / std::f64::consts::SQRT_2;
    for (name, press, to) in [
        ("scale", at.high, pt(at.high.x + 40.0, at.high.y + 25.0)),
        (
            "rotate",
            pt(at.high.x + offset, at.low.y - offset),
            pt(at.high.x - 100.0, at.low.y - 150.0),
        ),
    ] {
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            press,
            Tolerance::from_mm(2.0),
            tolerances,
            Modifiers::NONE,
        );
        tool.pointer_moved(to, Modifiers::NONE, &mut selection);
        let took = mean(5, || {
            let live = tool
                .live_edit(&objects, &selection, to, false, false)
                .expect("a live edit");
            SelectTool::group_of(&live.objects, &selection)
        });
        println!("{name}: one preview step of 10,000 objects: {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took < Duration::from_millis(50), "{name}: {took:?}");
        }
        tool.escape();
    }
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn the_commits_of_10000_objects_finish_within_5_s() {
    let document = build(5000, 20, 5000);
    let before = document.export_loro_snapshot().unwrap().len();
    let offset = 32.0 / SCALE / std::f64::consts::SQRT_2;
    let mut results = Vec::new();
    for gesture_name in ["move", "scale", "rotate", "copy"] {
        let objects = read(&document);
        let mut selection = selection_of(&objects);
        let at = spots(&objects, &selection);
        let centre = pt(
            f64::midpoint(at.low.x, at.high.x),
            f64::midpoint(at.low.y, at.high.y),
        );
        let (press, to, modifiers) = match gesture_name {
            "move" => (
                centre,
                pt(centre.x + 30.0, centre.y + 20.0),
                Modifiers::NONE,
            ),
            "copy" => (
                centre,
                pt(centre.x + 30.0, centre.y + 20.0),
                Modifiers::new(false, true),
            ),
            "scale" => (
                at.high,
                pt(at.high.x + 40.0, at.high.y + 25.0),
                Modifiers::NONE,
            ),
            _ => (
                pt(at.high.x + offset, at.low.y - offset),
                pt(at.high.x - 100.0, at.low.y - 150.0),
                Modifiers::NONE,
            ),
        };
        let took = gesture(&document, &objects, &mut selection, press, to, modifiers);
        results.push((gesture_name, took));
    }
    let after = document.export_loro_snapshot().unwrap().len();
    // Skew exists for paths only: 5,000 paths with 20 nodes each.
    let paths = build(5000, 20, 0);
    let objects = read(&paths);
    let mut selection = selection_of(&objects);
    let at = spots(&objects, &selection);
    let top = pt(f64::midpoint(at.low.x, at.high.x), at.low.y - 16.0 / SCALE);
    let skew = gesture(
        &paths,
        &objects,
        &mut selection,
        top,
        pt(top.x + 50.0, top.y),
        Modifiers::NONE,
    );
    results.push(("skew (5,000 paths)", skew));
    for (name, took) in &results {
        println!("commit of {name}: {took:?}");
        if !cfg!(debug_assertions) && *name != "copy" {
            assert!(*took < Duration::from_secs(5), "{name}: {took:?}");
        }
    }
    println!("saved file: {before} bytes before, {after} bytes after the four commits");
}
