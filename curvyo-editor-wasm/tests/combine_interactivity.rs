//! Combine and Break apart at the size of the budgets (`specs/0035-combine-and-break-apart`
//! criterion 19): four generated inputs, each command through the session in under 2 s with the
//! expected counts. Budgets are asserted in release builds only; a debug build runs a tenth of the
//! size and prints the numbers.
//! Run: `cargo test --release -p curvyo-editor-wasm --test combine_interactivity -- --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, Vec2,
    pack,
};
use curvyo_editor_wasm::{BreakApartOutcome, CombineOutcome, Session, Tool};
use curvyo_ui_core::BreakApartRefusal;

const SCALE: usize = if cfg!(debug_assertions) { 10 } else { 1 };
const BUDGET: Duration = Duration::from_secs(2);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

fn select_all(session: &mut Session, to: Point) {
    let from = pt(-200.0, -200.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

fn check(label: &str, took: Duration) {
    println!("{label}: {took:?} (budget {BUDGET:?})");
    if !cfg!(debug_assertions) {
        assert!(took <= BUDGET, "{label}: {took:?} is over {BUDGET:?}");
    }
}

/// Anchor ids from one counter, so no two paths share one.
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> AnchorId {
        self.0 += 1;
        AnchorId::new(1, self.0)
    }
}

/// A circle of four smooth nodes.
fn circle(document: &Document, ids: &mut Ids, cx: f64, cy: f64, r: f64) {
    let k = r * 0.552_284_749_8;
    let node = |ids: &mut Ids, x: f64, y: f64, hin: Vec2, hout: Vec2| NewAnchor {
        id: ids.next(),
        point: pt(x, y),
        handle_in: hin,
        handle_out: hout,
        kind: AnchorKind::Symmetric,
    };
    let anchors = [
        node(ids, cx + r, cy, Vec2::new(0.0, -k), Vec2::new(0.0, k)),
        node(ids, cx, cy + r, Vec2::new(k, 0.0), Vec2::new(-k, 0.0)),
        node(ids, cx - r, cy, Vec2::new(0.0, k), Vec2::new(0.0, -k)),
        node(ids, cx, cy - r, Vec2::new(-k, 0.0), Vec2::new(k, 0.0)),
    ];
    let _ = document.create_path(&anchors, true);
}

fn square(document: &Document, ids: &mut Ids, x: f64, y: f64, side: f64) {
    let anchors = [(x, y), (x + side, y), (x + side, y + side), (x, y + side)]
        .map(|(px, py)| NewAnchor::corner(ids.next(), pt(px, py)));
    let _ = document.create_path(&anchors, true);
}

fn only_path(session: &Session) -> curvyo_document_core::PathSnapshot {
    let document = unpacked(session);
    let ids = document.object_ids();
    assert_eq!(ids.len(), 1);
    let Some(ObjectSnapshot::Path(path)) = document.object(ids[0]) else {
        panic!("a path");
    };
    path
}

fn unpacked(session: &Session) -> Document {
    curvyo_document_core::unpack(3, &session.pack("0.1.0").unwrap()).unwrap()
}

/// (a) One rectangle with 1,000 circles inside: Combine gives one compound path of 1,001
/// outlines; Break apart of it is one piece with its holes and is refused.
#[test]
fn a_rectangle_with_a_thousand_circles() {
    let count = 1000 / SCALE;
    let document = Document::new(1);
    let mut ids = Ids(0);
    let _ = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(500.0),
        height: Length::from_mm(500.0),
    });
    for i in 0..count {
        let (x, y) = (20.0 + (i % 40) as f64 * 12.0, 20.0 + (i / 40) as f64 * 12.0);
        circle(&document, &mut ids, x, y, 4.0);
    }
    let mut session = session_of(&document);
    select_all(&mut session, pt(600.0, 600.0));
    assert_eq!(session.selected_object_count(), count + 1);

    let started = Instant::now();
    let outcome = session.apply_combine();
    check("combine (a)", started.elapsed());
    assert!(matches!(outcome, CombineOutcome::Applied { holes, .. } if holes == count));
    assert_eq!(only_path(&session).subpaths().count(), count + 1);

    let started = Instant::now();
    let outcome = session.apply_break_apart();
    check("break apart (a)", started.elapsed());
    assert_eq!(
        outcome,
        BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds: 1 })
    );
}

/// (b) 2,000 disjoint squares: one compound path of 2,000 outlines, then 2,000 objects.
#[test]
fn two_thousand_disjoint_squares() {
    let count = 2000 / SCALE;
    let document = Document::new(1);
    let mut ids = Ids(0);
    for i in 0..count {
        square(
            &document,
            &mut ids,
            (i % 50) as f64 * 12.0,
            (i / 50) as f64 * 12.0,
            10.0,
        );
    }
    let mut session = session_of(&document);
    select_all(&mut session, pt(700.0, 700.0));
    assert_eq!(session.selected_object_count(), count);

    let started = Instant::now();
    assert!(matches!(
        session.apply_combine(),
        CombineOutcome::Applied { .. }
    ));
    check("combine (b)", started.elapsed());
    assert_eq!(only_path(&session).subpaths().count(), count);

    let started = Instant::now();
    let outcome = session.apply_break_apart();
    check("break apart (b)", started.elapsed());
    assert!(matches!(outcome, BreakApartOutcome::Applied { pieces, .. } if pieces == count));
    assert_eq!(unpacked(&session).object_ids().len(), count);
}

/// (c) 500 nested squares: one compound path of 500 outlines with alternating windings, then 250
/// regions of a square and its hole.
#[test]
fn five_hundred_nested_squares() {
    let count = 500 / SCALE;
    let document = Document::new(1);
    let mut ids = Ids(0);
    for i in 0..count {
        let inset = i as f64 * 1.0;
        square(
            &document,
            &mut ids,
            inset,
            inset,
            2.0 * count as f64 - 2.0 * inset,
        );
    }
    let mut session = session_of(&document);
    select_all(
        &mut session,
        pt(2.0 * count as f64 + 50.0, 2.0 * count as f64 + 50.0),
    );
    assert_eq!(session.selected_object_count(), count);

    let started = Instant::now();
    assert!(matches!(
        session.apply_combine(),
        CombineOutcome::Applied { .. }
    ));
    check("combine (c)", started.elapsed());
    let path = only_path(&session);
    let signs: Vec<bool> = path
        .subpaths()
        .map(|s| {
            let n = s.anchors.len();
            let area: f64 = (0..n)
                .map(|k| {
                    let (a, b) = (s.anchors[k].point, s.anchors[(k + 1) % n].point);
                    a.x * b.y - b.x * a.y
                })
                .sum();
            area > 0.0
        })
        .collect();
    assert_eq!(signs.len(), count);
    assert!(
        signs
            .iter()
            .enumerate()
            .all(|(k, &shape)| shape == (k % 2 == 0))
    );

    let started = Instant::now();
    let outcome = session.apply_break_apart();
    check("break apart (c)", started.elapsed());
    assert!(matches!(outcome, BreakApartOutcome::Applied { pieces, .. } if pieces == count / 2));
}

/// (d) A compound path of 5,000 tiny squares breaks apart into 5,000 closed paths.
#[test]
fn a_compound_path_of_five_thousand_squares() {
    let count = 5000 / SCALE;
    let big = Document::new(1);
    let mut ids = Ids(0);
    let outlines: Vec<(Vec<NewAnchor>, bool)> = (0..count)
        .map(|i| {
            let (x, y) = ((i % 80) as f64 * 3.0, (i / 80) as f64 * 3.0);
            let anchors = [(x, y), (x + 1.0, y), (x + 1.0, y + 1.0), (x, y + 1.0)]
                .map(|(px, py)| NewAnchor::corner(ids.next(), pt(px, py)))
                .to_vec();
            (anchors, true)
        })
        .collect();
    let base = big.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    });
    let _ = big
        .replace_with_path(&[base], base, &outlines, "combine_paths")
        .unwrap();
    let mut session = session_of(&big);
    select_all(&mut session, pt(400.0, 400.0));
    assert_eq!(session.selected_object_count(), 1);

    let started = Instant::now();
    let outcome = session.apply_break_apart();
    check("break apart (d)", started.elapsed());
    assert!(matches!(outcome, BreakApartOutcome::Applied { pieces, .. } if pieces == count));
    assert_eq!(unpacked(&session).object_ids().len(), count);
}
