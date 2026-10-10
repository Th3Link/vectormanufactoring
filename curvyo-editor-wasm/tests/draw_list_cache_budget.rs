//! The cost of a frame at rest with many objects (`specs/0044-editing-quick-wins/` criterion 5
//! and `docs/technical-debt.md`, "Canvas performance on Linux/WebKitGTK"). `#[ignore]`d: the
//! numbers are for a release build on the host CPU:
//!
//! ```text
//! cargo test --release -p curvyo-editor-wasm --test draw_list_cache_budget -- --ignored --nocapture
//! ```
//!
//! `specs/0044-editing-quick-wins/` criterion 5: with 5,000 objects Ctrl+A is drawn within 100 ms.
//! The nudge report is the cost of one arrow-key event with 1,000 objects selected.
//!
//! Half the objects are rectangles, half are closed paths of eight nodes. The frame at rest is
//! `Session::draw_list()` with nothing changed since the previous frame.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{AnchorId, Document, Length, NewAnchor, Point, RectBounds, pack};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};

const COLUMNS: usize = 100;
const PITCH_MM: f64 = 20.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session_with(count: usize) -> Session {
    let document = Document::new(1);
    let mut anchor = 0_u64;
    for i in 0..count {
        let (x, y) = (
            (i % COLUMNS) as f64 * PITCH_MM,
            (i / COLUMNS) as f64 * PITCH_MM,
        );
        if i % 2 == 0 {
            let _ = document.create_rect(RectBounds {
                origin: pt(x, y),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            });
        } else {
            let anchors: Vec<NewAnchor> = (0..8_u32)
                .map(|n| {
                    let angle = f64::from(n) / 8.0 * std::f64::consts::TAU;
                    anchor += 1;
                    NewAnchor::corner(
                        AnchorId::new(1, anchor),
                        pt(x + 5.0 + 5.0 * angle.cos(), y + 5.0 + 5.0 * angle.sin()),
                    )
                })
                .collect();
            let _ = document.create_path(&anchors, true);
        }
    }
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1600.0, 900.0);
    session.set_tool(Tool::Select);
    session
}

/// The mean time of `frames` consecutive `draw_list` calls.
fn frame_at_rest(session: &Session, frames: u32) -> Duration {
    let started = Instant::now();
    for _ in 0..frames {
        let _ = session.draw_list();
    }
    started.elapsed() / frames
}

#[test]
#[ignore = "a timing report for release builds; run on purpose"]
fn frame_at_rest_by_object_count() {
    for count in [200, 5000, 10_000] {
        let session = session_with(count);
        let first = Instant::now();
        let _ = session.draw_list();
        let first = first.elapsed();
        let rest = frame_at_rest(&session, 5);
        println!("{count:>6} objects, nothing selected: first frame {first:?}, at rest {rest:?}");
        let mut session = session;
        let edge = (COLUMNS as f64 + 1.0) * PITCH_MM;
        session.pointer_down(pt(-5.0, -5.0), false);
        session.pointer_hover(pt(edge, edge * 100.0), false, false);
        session.pointer_up(pt(edge, edge * 100.0), false, false);
        assert_eq!(session.selected_object_count(), count);
        let first = Instant::now();
        let _ = session.draw_list();
        let first = first.elapsed();
        let rest = frame_at_rest(&session, 5);
        println!("{count:>6} objects, all selected: first frame {first:?}, at rest {rest:?}");
    }
}

/// Everything the frontend reads from the session after a key press (`syncFromSession`, the
/// Style panel, the rail's command availability and the readouts), and what that took.
fn reads_after_a_key(session: &Session) -> Duration {
    let mut total = Duration::ZERO;
    let mut time = |name: &str, read: &dyn Fn()| {
        let started = Instant::now();
        read();
        let took = started.elapsed();
        if took >= Duration::from_millis(5) {
            println!("    read {name}: {took:?}");
        }
        total += took;
    };
    time("selected_object_count", &|| {
        let _ = session.selected_object_count();
    });
    time("select_bar_state", &|| {
        let _ = session.select_bar_state();
    });
    time("transform_entry", &|| {
        let _ = session.transform_entry();
    });
    time("move_entry", &|| {
        let _ = session.move_entry();
    });
    time("move_indicators", &|| {
        let _ = session.move_indicators();
    });
    time("style_panel_view", &|| {
        let _ = session.style_panel_view();
    });
    time("boolean_availability", &|| {
        let _ = session.boolean_availability();
    });
    time("path_availability", &|| {
        let _ = session.path_availability();
    });
    time("close_path_state", &|| {
        let _ = session.close_path_state();
    });
    time("node_toolbar_state", &|| {
        let _ = session.node_toolbar_state();
    });
    time("live_readout", &|| {
        let _ = session.live_readout();
    });
    time("nudge_readout", &|| {
        let _ = session.nudge_readout();
    });
    time("cursor_hint", &|| {
        let _ = session.cursor_hint();
    });
    time("handle_hint", &|| {
        let _ = session.handle_hint();
    });
    total
}

const SELECT_ALL_BUDGET: Duration = Duration::from_millis(100);

#[test]
#[ignore = "a timing budget for release builds; run on purpose"]
fn select_all_is_drawn_within_budget_with_five_thousand_objects() {
    let mut session = session_with(5000);
    let _ = session.draw_list();
    let started = Instant::now();
    let outcome = session.key_down(KeyInput {
        key: "a",
        ctrl: true,
        ..KeyInput::default()
    });
    let key = started.elapsed();
    let _ = session.draw_list();
    let drawn = started.elapsed();
    let reads = reads_after_a_key(&session);
    assert_eq!(outcome, KeyOutcome::SelectedAll);
    assert_eq!(session.selected_object_count(), 5000);
    println!(
        "Ctrl+A with 5000 objects: key {key:?}, key and first frame {drawn:?}, \
         the frontend's reads after the key {reads:?}"
    );
    println!(
        "all selected, frame at rest {:?}",
        frame_at_rest(&session, 5)
    );
    if !cfg!(debug_assertions) {
        assert!(
            drawn <= SELECT_ALL_BUDGET,
            "{drawn:?} (budget {SELECT_ALL_BUDGET:?})"
        );
    }
}

#[test]
#[ignore = "a timing report for release builds; run on purpose"]
fn one_nudge_event_with_a_thousand_objects_selected() {
    let mut session = session_with(1000);
    let _ = session.key_down(KeyInput {
        key: "a",
        ctrl: true,
        ..KeyInput::default()
    });
    let _ = session.draw_list();
    let mut worst = Duration::ZERO;
    let mut frame = Duration::ZERO;
    let mut reads = Duration::ZERO;
    for step in 0..20_u32 {
        let started = Instant::now();
        let _ = session.key_down(KeyInput {
            key: "ArrowRight",
            repeat: step > 0,
            time_ms: f64::from(step) * 33.0,
            ..KeyInput::default()
        });
        worst = worst.max(started.elapsed());
        reads = reads.max(reads_after_a_key(&session));
        let started = Instant::now();
        let _ = session.draw_list();
        frame = frame.max(started.elapsed());
    }
    println!(
        "nudge of 1000 objects: worst key event {worst:?}, worst reads after it {reads:?}, worst frame after it {frame:?}"
    );
}
