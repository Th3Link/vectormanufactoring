//! The boolean command at the size of the budgets (`specs/0016-boolean-operations` criteria
//! 46 and 47): Union of 1,000 rectangles including the document update, and the time from the
//! press to the repainted canvas for two operands of 1,000 nodes each. Budgets are asserted in
//! release builds only; a debug build runs a fifth of the size and prints the numbers.
//! Run: `cargo test --release -p curvyo-editor-wasm --test boolean_interactivity -- --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{AnchorId, Document, Length, NewAnchor, Point, RectBounds, pack};
use curvyo_editor_wasm::{BooleanOutcome, Session, Tool};
use curvyo_ui_core::BooleanOp;

const RECTANGLES: usize = if cfg!(debug_assertions) { 200 } else { 1_000 };
const NODES: usize = if cfg!(debug_assertions) { 200 } else { 1_000 };

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

/// A marquee over everything, from outside the artwork.
fn select_all(session: &mut Session, to: Point) {
    let from = pt(-200.0, -200.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

fn check(label: &str, took: Duration, budget: Duration) {
    println!("{label}: {took:?} (budget {budget:?})");
    if !cfg!(debug_assertions) {
        assert!(took <= budget, "{label}: {took:?} is over {budget:?}");
    }
}

/// Criterion 46: 1,000 rectangles in a grid, neighbours overlapping, one Union within 1 s.
#[test]
fn union_of_a_thousand_rectangles_including_the_document_update() {
    let document = Document::new(1);
    for i in 0..RECTANGLES {
        let (x, y) = ((i % 40) as f64 * 8.0, (i / 40) as f64 * 8.0);
        let _ = document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
    }
    let mut session = session_of(&document);
    select_all(&mut session, pt(400.0, 400.0));
    assert_eq!(session.selected_object_count(), RECTANGLES);

    let started = Instant::now();
    let outcome = session.apply_boolean(BooleanOp::Union);
    let took = started.elapsed();
    assert!(
        matches!(outcome, BooleanOutcome::Applied { operands, .. } if operands == RECTANGLES),
        "{outcome:?}"
    );
    check("union of the rectangles", took, Duration::from_secs(1));
}

/// Criterion 47: two operands of 1,000 nodes each, press to repainted canvas within 150 ms.
#[test]
fn two_big_operands_are_repainted_within_the_budget() {
    let document = Document::new(1);
    for (k, centre) in [pt(0.0, 0.0), pt(40.0, 0.0)].into_iter().enumerate() {
        let anchors: Vec<_> = (0..NODES)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / NODES as f64;
                NewAnchor::corner(
                    AnchorId::new(10 + k as u64, i as u64),
                    pt(centre.x + 50.0 * angle.cos(), centre.y + 50.0 * angle.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
    }
    let mut session = session_of(&document);
    select_all(&mut session, pt(200.0, 200.0));
    assert_eq!(session.selected_object_count(), 2);

    let started = Instant::now();
    let outcome = session.apply_boolean(BooleanOp::Union);
    let _ = session.draw_list();
    let took = started.elapsed();
    assert!(
        matches!(outcome, BooleanOutcome::Applied { .. }),
        "{outcome:?}"
    );
    check("press to repaint", took, Duration::from_millis(150));
}
