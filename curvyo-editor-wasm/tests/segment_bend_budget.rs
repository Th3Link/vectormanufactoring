//! The frame and hover budgets of a segment bend on a path of 5000 nodes
//! (`specs/0031-segment-drag-bending` criterion 17; `adrs.md`, "performance"). `#[ignore]`d: the
//! numbers are for a release build on the host CPU, so run it on purpose:
//!
//! ```text
//! cargo test --release -p curvyo-editor-wasm --test segment_bend_budget -- --ignored --nocapture
//! ```
//!
//! Budgets: a bend frame's `draw_list` at most 12 ms after the first frame of the drag, which
//! reads the document once (headroom under the 20 ms of 50 frames per second for the `WebKitGTK`
//! upload); one hover hit test at most 2 ms, the document read excluded.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{AnchorId, AnchorKind, Document, NewAnchor, Point, Vec2, pack};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{NodeSelection, hit_test};

const NODES: usize = 5000;
const FRAME_BUDGET: Duration = Duration::from_millis(12);
const HOVER_BUDGET: Duration = Duration::from_millis(2);

fn long_path() -> Document {
    let document = Document::new(1);
    let anchors: Vec<NewAnchor> = (0..NODES)
        .map(|i| {
            let x = i as f64 * 2.0;
            NewAnchor {
                id: AnchorId::new(1, i as u64),
                point: Point::new(x, if i % 2 == 0 { 0.0 } else { 3.0 }),
                handle_in: Vec2::new(-0.5, 0.0),
                handle_out: Vec2::new(0.5, 0.0),
                kind: AnchorKind::Corner,
            }
        })
        .collect();
    let _ = document.create_path(&anchors, false);
    document
}

#[test]
#[ignore = "a timing budget for release builds; run on purpose"]
fn a_bend_frame_and_a_hover_stay_within_budget_on_five_thousand_nodes() {
    let document = long_path();
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session.set_tool(Tool::Node);
    // The middle of a segment far from the ends.
    let at = Point::new(4000.0 + 1.0, 1.5);
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);

    let mut worst = Duration::ZERO;
    for step in 1..=60 {
        let to = Point::new(at.x, at.y - f64::from(step));
        session.pointer_hover(to, false, false);
        let started = Instant::now();
        let list = session.draw_list();
        // The first frame of a drag reads the document once (the drag then reuses that read);
        // the budget is for every frame after it.
        if step > 1 {
            worst = worst.max(started.elapsed());
        } else {
            println!(
                "first frame of the drag (reads the document): {:?}",
                started.elapsed()
            );
        }
        assert!(list.triangle_count() > 0);
    }
    println!("bend frame, worst of 60: {worst:?} (budget {FRAME_BUDGET:?})");
    session.pointer_up(Point::new(at.x, at.y - 60.0), false, false);
    if !cfg!(debug_assertions) {
        assert!(worst <= FRAME_BUDGET, "{worst:?}");
    }

    let paths = vec![document.path(document.object_ids()[0]).unwrap()];
    let selection = NodeSelection::new();
    let tolerance = curvyo_document_core::Tolerance::from_mm(4.0 / 3.78);
    let mut worst_hover = Duration::ZERO;
    for step in 0..60 {
        let point = Point::new(4000.0 + f64::from(step), 1.5);
        let started = Instant::now();
        let _ = hit_test(&paths, &selection, point, tolerance, tolerance, tolerance);
        worst_hover = worst_hover.max(started.elapsed());
    }
    println!("hover hit test, worst of 60: {worst_hover:?} (budget {HOVER_BUDGET:?})");
    if !cfg!(debug_assertions) {
        assert!(worst_hover <= HOVER_BUDGET, "{worst_hover:?}");
    }
}
