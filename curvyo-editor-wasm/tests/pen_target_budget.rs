//! The hover budget of the Pen's targets with 5000 open paths (`specs/0034-pen-path-extension`
//! criterion 24). `#[ignore]`d: the numbers are for a release build on the host CPU:
//!
//! ```text
//! cargo test --release -p curvyo-editor-wasm --test pen_target_budget -- --ignored --nocapture
//! ```
//!
//! A hover query with the end-node cache warm must take at most 2 ms. The first query after a
//! document change rebuilds the index with one read of every path; that cost is reported, not
//! budgeted (the same read every frame already pays).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{AnchorId, Document, NewAnchor, Point, pack};
use curvyo_editor_wasm::{Session, Tool};

const PATHS: usize = 5000;
const BUDGET: Duration = Duration::from_millis(2);

#[test]
#[ignore = "a timing budget for release builds; run on purpose"]
fn a_warm_hover_query_stays_within_budget_with_five_thousand_open_paths() {
    let document = Document::new(1);
    for i in 0..PATHS {
        let (x, y) = ((i % 100) as f64 * 30.0, (i / 100) as f64 * 30.0);
        let base = (i * 2) as u64;
        let _ = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, base), Point::new(x, y)),
                NewAnchor::corner(AnchorId::new(1, base + 1), Point::new(x + 10.0, y)),
            ],
            false,
        );
    }
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session.set_tool(Tool::Pen);

    session.pointer_hover(Point::new(5.0, 5.0), false, false);
    let started = Instant::now();
    let _ = session.pen_target();
    println!("first query (builds the index): {:?}", started.elapsed());

    let mut worst = Duration::ZERO;
    for step in 0..200 {
        session.pointer_hover(Point::new(f64::from(step) * 7.0, 3.0), false, false);
        let started = Instant::now();
        let _ = session.pen_target();
        worst = worst.max(started.elapsed());
    }
    println!("warm query, worst of 200: {worst:?} (budget {BUDGET:?})");
    if !cfg!(debug_assertions) {
        assert!(worst <= BUDGET, "{worst:?}");
    }

    // A commit changes the version: the next query rebuilds the index.
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(5000.0, 5000.0), false);
    session.pointer_up(Point::new(5000.0, 5000.0), false, false);
    session.pointer_down(Point::new(5100.0, 5000.0), false);
    session.pointer_up(Point::new(5100.0, 5000.0), false, false);
    session.finish_pen();
    session.pointer_hover(Point::new(5.0, 5.0), false, false);
    let started = Instant::now();
    let _ = session.pen_target();
    println!(
        "first query after a commit (rebuild): {:?}",
        started.elapsed()
    );
}
