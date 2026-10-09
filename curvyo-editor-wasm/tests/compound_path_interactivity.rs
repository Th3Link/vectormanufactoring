//! A big compound path stays interactive in the editor session
//! (`specs/0016-boolean-operations` criterion 47): the cost of one frame during a move, a
//! rotate and a resize drag, of a hover, and of the release that commits. Budgets are asserted
//! in release builds only; a debug build runs a fifth of the outlines and prints the numbers.
//! Run: `cargo test --release -p curvyo-editor-wasm --test compound_path_interactivity --
//! --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, Point, RectBounds, StyleEdit, pack,
};
use curvyo_editor_wasm::{Session, Tool};

/// Outlines of four anchors each: 1,000 (4,000 anchors, the size of a big boolean result) in
/// release, a fifth of that in debug.
const OUTLINES: usize = if cfg!(debug_assertions) { 200 } else { 1_000 };

/// A guard against a slow path, not a target: a frame of a drag measures 13 to 36 ms on the
/// development machine, and the budget leaves a shared CI runner twice that. The cost is the
/// tessellation of the whole object on every frame; a quadratic bug would take seconds.
const FRAME_BUDGET: Duration = Duration::from_millis(100);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session() -> Session {
    let document = Document::new(1);
    let seed = document.create_rect(RectBounds {
        origin: pt(900.0, 900.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    });
    let outlines: Vec<_> = (0..OUTLINES)
        .map(|i| {
            let (x, y) = ((i % 40) as f64 * 3.0, (i / 40) as f64 * 3.0);
            let anchors = [(x, y), (x + 2.0, y), (x + 2.0, y + 2.0), (x, y + 2.0)]
                .into_iter()
                .enumerate()
                .map(|(k, (cx, cy))| {
                    NewAnchor::corner(AnchorId::new(5, (i * 4 + k) as u64), pt(cx, cy))
                })
                .collect();
            (anchors, true)
        })
        .collect();
    let id = document
        .replace_with_path(&[seed], seed, &outlines, "boolean_union")
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

fn check(label: &str, took: Duration) {
    println!("{label}: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(
            took < FRAME_BUDGET,
            "{label} took {took:?}, budget {FRAME_BUDGET:?}"
        );
    }
}

/// Mean time of the frames in `moves`, each followed by a draw.
fn frame_time(session: &mut Session, moves: &[Point], shift: bool) -> Duration {
    let started = Instant::now();
    for &to in moves {
        session.pointer_hover(to, shift, false);
        let _ = session.draw_list();
    }
    started.elapsed() / u32::try_from(moves.len()).unwrap()
}

#[test]
fn moving_rotating_and_resizing_a_big_compound_path_stays_interactive() {
    let mut s = session();
    let on_fill = pt(1.0, 1.0);

    // Select, then a hover frame.
    s.pointer_hover(on_fill, false, false);
    s.pointer_down(on_fill, false);
    s.pointer_up(on_fill, false, false);
    assert_eq!(s.selected_object_count(), 1);
    check(
        "hover frame",
        frame_time(&mut s, &[pt(2.0, 2.0), pt(3.0, 3.0), pt(4.0, 4.0)], false),
    );

    // A move drag: press, a few frames, release.
    s.pointer_down(on_fill, false);
    let steps: Vec<Point> = (1..=5).map(|k| pt(1.0 + f64::from(k) * 2.0, 1.0)).collect();
    check("move drag frame", frame_time(&mut s, &steps, false));
    let started = Instant::now();
    s.pointer_up(pt(11.0, 1.0), false, false);
    check("move release (commit)", started.elapsed());

    // The box of the 40-column grid of 2 mm squares on a 3 mm pitch.
    let rows = (OUTLINES - 1) / 40;
    let corner_se = pt(117.0, rows as f64 * 3.0 + 2.0);
    let corner_ne = pt(117.0, 0.0);
    let first_extra_corner = |s: &Session| {
        let bytes = s.pack("0.1.0").unwrap();
        let document = curvyo_document_core::unpack(9, &bytes).unwrap();
        let id = *document.object_ids().last().unwrap();
        document.path(id).unwrap().extra_subpaths[0].anchors[0].point
    };

    // A resize drag from the south-east corner.
    s.pointer_hover(corner_se, false, false);
    s.pointer_down(corner_se, false);
    let steps: Vec<Point> = (1..=5)
        .map(|k| {
            pt(
                corner_se.x + f64::from(k) * 4.0,
                corner_se.y + f64::from(k) * 2.0,
            )
        })
        .collect();
    check("resize drag frame", frame_time(&mut s, &steps, false));
    let started = Instant::now();
    s.pointer_up(*steps.last().unwrap(), false, false);
    check("resize release (commit)", started.elapsed());
    let resized = first_extra_corner(&s);
    assert!(resized.x > 3.0, "the resize committed: {resized:?}");

    // A rotate drag from just outside the north-east corner of the resized box.
    let d = 32.0 / s.view().scale() / std::f64::consts::SQRT_2;
    // The resize drag moved the south-east corner 20 mm to the right.
    let box_ne = pt(corner_ne.x + 20.0, corner_ne.y);
    let handle = pt(box_ne.x + d, box_ne.y - d);
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    let steps: Vec<Point> = (1..=5)
        .map(|k| pt(handle.x + f64::from(k) * 5.0, handle.y + f64::from(k) * 5.0))
        .collect();
    check("rotate drag frame", frame_time(&mut s, &steps, false));
    let started = Instant::now();
    s.pointer_up(*steps.last().unwrap(), false, false);
    check("rotate release (commit)", started.elapsed());
    assert_ne!(first_extra_corner(&s), resized, "the rotation committed");
}
