//! Independent tester frame budget of the converting stretch
//! (`specs/0019-multi-object-transform/` criterion 48, second case) in a release
//! build: `cargo nextest run --release -p curvyo-editor-wasm --test
//! acceptance_0019_stretch_budgets_tester --run-ignored only --no-capture`.
//! 200 objects, 100 paths of 50 nodes and 100 rectangles; a frame (hover plus
//! draw list) of the corner stretch of the unrotated selection and of the edge
//! stretch of the same selection with the rectangles turned by 30 degrees (all
//! convert), each against a plain move frame of the same selection, five runs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]
#![allow(clippy::too_many_lines, missing_docs, clippy::cast_possible_truncation)]
#![allow(clippy::manual_midpoint)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Angle, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, pack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::object_outline_bounds;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn scene(turn_rects: bool) -> Document {
    let d = Document::new(1);
    let cell = |i: usize| (10.0 + (i % 20) as f64 * 14.0, 10.0 + (i / 20) as f64 * 14.0);
    for i in 0..100usize {
        let (cx, cy) = cell(i);
        let anchors: Vec<NewAnchor> = (0..50usize)
            .map(|n| {
                let a = n as f64 / 50.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, (i * 50 + n) as u64),
                    pt(cx + 5.0 * a.cos(), cy + 5.0 * a.sin()),
                )
            })
            .collect();
        let _ = d.create_path(&anchors, true);
    }
    for j in 0..100usize {
        let (cx, cy) = cell(100 + j);
        let id = d.create_rect(RectBounds {
            origin: pt(cx - 5.0, cy - 4.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(8.0),
        });
        if turn_rects {
            let o = d.object(id).unwrap();
            d.rotate_object(&o.rotated(pt(cx, cy), Angle::from_radians(30.0_f64.to_radians())))
                .unwrap();
        }
    }
    d
}

fn selected(d: &Document) -> (Session, Point, Point) {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1600.0, 1000.0);
    s.pointer_hover(pt(-20.0, -20.0), false, false);
    s.pointer_down(pt(-20.0, -20.0), false);
    s.pointer_hover(pt(900.0, 900.0), false, false);
    s.pointer_up(pt(900.0, 900.0), false, false);
    assert_eq!(s.selected_object_count(), 200);
    let os: Vec<ObjectSnapshot> = d
        .object_ids()
        .into_iter()
        .filter_map(|i| d.object(i))
        .collect();
    let (lo, hi) = os
        .iter()
        .map(object_outline_bounds)
        .reduce(|(a, b), (c, e)| {
            (
                pt(a.x.min(c.x), a.y.min(c.y)),
                pt(b.x.max(e.x), b.y.max(e.y)),
            )
        })
        .unwrap();
    (s, lo, hi)
}

/// Mean frame (hover plus draw list) of a drag from `press` moving `step` per frame.
fn drag_frame(s: &mut Session, press: Point, step: (f64, f64), readout: bool) -> Duration {
    s.pointer_hover(press, false, false);
    s.pointer_down(press, false);
    s.pointer_hover(
        pt(press.x + step.0 * 20.0, press.y + step.1 * 20.0),
        false,
        false,
    );
    assert!(!readout || s.live_readout().is_some(), "the stretch runs");
    let _ = s.draw_list();
    let frames = 30u32;
    let mut total = Duration::ZERO;
    for f in 21..frames + 21 {
        let to = pt(
            press.x + step.0 * f64::from(f),
            press.y + step.1 * f64::from(f),
        );
        let t = Instant::now();
        s.pointer_hover(to, false, false);
        std::hint::black_box(s.draw_list());
        total += t.elapsed();
    }
    s.escape();
    s.pointer_up(press, false, false);
    total / frames
}

fn best(s: &mut Session, press: Point, step: (f64, f64), readout: bool) -> Duration {
    (0..3)
        .map(|_| drag_frame(s, press, step, readout))
        .min()
        .unwrap()
}

#[test]
#[ignore = "release-mode benchmark"]
fn stretch_frames_against_move_frames() {
    for (name, turn, edge) in [
        ("corner stretch, rectangles unrotated", false, false),
        (
            "edge stretch, rectangles turned 30 degrees (100 convert)",
            true,
            true,
        ),
    ] {
        let d = scene(turn);
        let (mut s, lo, hi) = selected(&d);
        let centre = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let mut ratios = Vec::new();
        for _ in 0..11 {
            let mv = best(&mut s, centre, (0.2, 0.1), false);
            let (press, step) = if edge {
                (pt(hi.x, centre.y), (0.3, 0.0))
            } else {
                (hi, (0.3, 0.15))
            };
            let st = best(&mut s, press, step, true);
            ratios.push(st.as_secs_f64() / mv.as_secs_f64());
            println!(
                "{name}: move {mv:?}, stretch {st:?}, ratio {:.3}",
                ratios.last().unwrap()
            );
        }
        println!("{name}: ratios {ratios:.3?}");
        // The gate is the median of the interleaved rounds (a single ratio of two
        // 4 ms timings is inside machine noise).
        ratios.sort_by(f64::total_cmp);
        let median = ratios[ratios.len() / 2];
        println!("{name}: median ratio {median:.3}");
        if !cfg!(debug_assertions) {
            assert!(median <= 1.1, "{name}: median ratio {median:.3}");
        }
    }
}
