//! The frame budget of `specs/0019-multi-object-transform/` criterion 48 (the
//! architect's decision 3): a preview frame of a scale, rotate or skew of 200
//! selected objects (100 paths with 50 nodes, 100 rectangles) takes at most 1.1
//! times a plain move preview frame of the same selection, measured in the same
//! run. Run in release by the tester:
//! `cargo test --release -p curvyo-editor-wasm --test multi_object_transform_budgets -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, pack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::object_outline_bounds;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// 100 squares in a 10 x 10 grid and 100 closed 50-node paths below them.
fn scene(with_rects: bool) -> (Document, Vec<Point>) {
    let document = Document::new(1);
    let mut clicks = Vec::new();
    if with_rects {
        for i in 0..100u32 {
            let (x, y) = (f64::from(i % 10) * 25.0, f64::from(i / 10) * 25.0);
            let _ = document.create_rect(RectBounds {
                origin: pt(x, y),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            });
            clicks.push(pt(x, y + 5.0));
        }
    }
    for i in 0..100u32 {
        let (cx, cy) = (
            f64::from(i % 10) * 25.0 + 5.0,
            400.0 + f64::from(i / 10) * 25.0,
        );
        let anchors: Vec<NewAnchor> = (0..50u32)
            .map(|n| {
                let a = f64::from(n) / 50.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * 50 + u64::from(n)),
                    pt(cx + 8.0 * a.cos(), cy + 8.0 * a.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
        clicks.push(pt(cx + 8.0, cy));
    }
    (document, clicks)
}

fn selected_session(document: &Document, clicks: &[Point]) -> (Session, Point, Point) {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    // Zoom out so that all of it is on screen.
    for _ in 0..6 {
        session.wheel(0.0, 100.0, 600.0, 400.0, false, true);
    }
    let mut first = true;
    for at in clicks {
        session.pointer_hover(*at, !first, false);
        session.pointer_down(*at, !first);
        session.pointer_up(*at, !first, false);
        first = false;
    }
    let objects: Vec<ObjectSnapshot> = document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect();
    let (low, high) = objects
        .iter()
        .map(object_outline_bounds)
        .reduce(|(a, b), (c, d)| {
            (
                pt(a.x.min(c.x), a.y.min(c.y)),
                pt(b.x.max(d.x), b.y.max(d.y)),
            )
        })
        .unwrap();
    (session, low, high)
}

/// The mean time of `draw_list` over 30 pointer positions of a drag that started
/// at `press`, the pointer moving along `step` per frame; the best of three drags,
/// so that a busy machine does not decide the ratio.
fn frame_time(session: &mut Session, press: Point, step: (f64, f64)) -> Duration {
    (0..3)
        .map(|_| one_drag(session, press, step))
        .min()
        .unwrap_or_default()
}

fn one_drag(session: &mut Session, press: Point, step: (f64, f64)) -> Duration {
    session.pointer_hover(press, false, false);
    session.pointer_down(press, false);
    let frames = 30u32;
    // One warm-up frame.
    session.pointer_hover(pt(press.x + step.0, press.y + step.1), false, false);
    let _ = session.draw_list();
    let mut total = Duration::ZERO;
    for frame in 2..frames + 2 {
        let to = pt(
            press.x + step.0 * f64::from(frame),
            press.y + step.1 * f64::from(frame),
        );
        session.pointer_hover(to, false, false);
        let began = Instant::now();
        let _ = session.draw_list();
        total += began.elapsed();
    }
    session.escape();
    total / frames
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn a_group_scale_or_rotate_preview_costs_at_most_1_1_times_a_move_preview() {
    let (document, clicks) = scene(true);
    let (mut session, low, high) = selected_session(&document, &clicks);
    let k = session.view().scale();
    // A move from the first rectangle's outline.
    let move_frame = frame_time(&mut session, clicks[0], (0.4, 0.2));
    // A scale from the south-east corner handle.
    let scale_frame = frame_time(&mut session, pt(high.x, high.y), (0.4, 0.2));
    // A rotate from the north-east corner rotate handle (32 px out on the diagonal).
    let offset = 32.0 / k / std::f64::consts::SQRT_2;
    let rotate_frame = frame_time(
        &mut session,
        pt(high.x + offset, low.y - offset),
        (-0.4, 0.6),
    );
    println!(
        "200 objects: move {move_frame:?}, scale {scale_frame:?} ({:.2}x), rotate {rotate_frame:?} ({:.2}x)",
        scale_frame.as_secs_f64() / move_frame.as_secs_f64(),
        rotate_frame.as_secs_f64() / move_frame.as_secs_f64(),
    );
    if !cfg!(debug_assertions) {
        for (name, frame) in [("scale", scale_frame), ("rotate", rotate_frame)] {
            assert!(
                frame.as_secs_f64() <= 1.1 * move_frame.as_secs_f64(),
                "{name} {frame:?} against move {move_frame:?}"
            );
        }
    }
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn a_group_skew_preview_costs_at_most_1_1_times_a_move_preview() {
    // Skew exists for paths only: the 100 paths of the scene.
    let (document, clicks) = scene(false);
    let (mut session, low, high) = selected_session(&document, &clicks);
    let k = session.view().scale();
    let move_frame = frame_time(&mut session, clicks[0], (0.4, 0.2));
    // The top skew handle: 16 px above the middle of the top side.
    let top = pt(f64::midpoint(low.x, high.x), low.y - 16.0 / k);
    let skew_frame = frame_time(&mut session, top, (0.5, 0.0));
    println!(
        "100 paths: move {move_frame:?}, skew {skew_frame:?} ({:.2}x)",
        skew_frame.as_secs_f64() / move_frame.as_secs_f64()
    );
    if !cfg!(debug_assertions) {
        assert!(
            skew_frame.as_secs_f64() <= 1.1 * move_frame.as_secs_f64(),
            "skew {skew_frame:?} against move {move_frame:?}"
        );
    }
}

/// Reported, not gated (criterion 49, decision 3): at 10,000 selected objects (5,000
/// paths with 20 nodes, 5,000 rectangles) the frame at rest, a scale preview frame
/// and the pointer event that precedes it. A full-fidelity frame of this size needs the
/// draw-list cache (`docs/technical-debt.md`).
#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn report_the_frames_of_10000_selected_objects() {
    let document = Document::new(1);
    let columns = 100u32;
    for i in 0..5000u32 {
        let (cx, cy) = (f64::from(i % columns) * 30.0, f64::from(i / columns) * 30.0);
        let anchors: Vec<NewAnchor> = (0..20u32)
            .map(|n| {
                let a = f64::from(n) / 20.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * 20 + u64::from(n)),
                    pt(cx + 10.0 * a.cos(), cy + 10.0 * a.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
    }
    for i in 0..5000u32 {
        let (x, y) = (
            f64::from(i % columns) * 30.0,
            1600.0 + f64::from(i / columns) * 30.0,
        );
        let _ = document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(12.0),
            height: Length::from_mm(12.0),
        });
    }
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    for _ in 0..12 {
        session.wheel(0.0, 100.0, 600.0, 400.0, false, true);
    }
    // Select everything with one marquee that contains it all.
    let from = pt(-200.0, -200.0);
    let to = pt(3200.0, 3300.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
    println!("selected: {}", session.selected_object_count());
    let began = Instant::now();
    let _ = session.draw_list();
    println!("10,000 selected, frame at rest: {:?}", began.elapsed());
    let objects: Vec<ObjectSnapshot> = document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect();
    let (_, high) = objects
        .iter()
        .map(object_outline_bounds)
        .reduce(|(a, b), (c, d)| {
            (
                pt(a.x.min(c.x), a.y.min(c.y)),
                pt(b.x.max(d.x), b.y.max(d.y)),
            )
        })
        .unwrap();
    session.pointer_hover(high, false, false);
    session.pointer_down(high, false);
    let to = pt(high.x + 40.0, high.y + 20.0);
    let began = Instant::now();
    session.pointer_hover(to, false, false);
    println!("scale drag, pointer event: {:?}", began.elapsed());
    let began = Instant::now();
    let _ = session.draw_list();
    println!("scale drag, preview frame: {:?}", began.elapsed());
    session.escape();
}
