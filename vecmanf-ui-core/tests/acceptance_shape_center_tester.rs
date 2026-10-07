//! Independent tester acceptance tests for
//! `specs/shape-creation-from-center/specification.md`, written from the
//! specification (and the public tool API) before the implementation diff was
//! read. Everything goes through `RectangleTool`, `EllipseTool` and
//! `PolygonStarTool`. Expected boxes come from a reference model written here
//! straight from "The rule" section, never read back from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, missing_docs, clippy::doc_markdown)]

use proptest::prelude::*;
use vecmanf_document_core::{Document, Point, Shape};
use vecmanf_ui_core::{
    CreateOutcome, EllipseTool, Modifiers, PolyStarMode, PolygonStarTool, RectangleTool,
};

const EPS: f64 = 1e-9;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn m(shift: bool, ctrl: bool) -> Modifiers {
    Modifiers::new(shift, ctrl)
}

const ALL: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];

/// The spec's rule: returns the two opposite corners of the box and E.
fn ref_corners(a: Point, b: Point, shift: bool, ctrl: bool) -> (Point, Point, Point) {
    let e = if ctrl {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let mm = dx.abs().max(dy.abs());
        let sx = if dx < 0.0 { -1.0 } else { 1.0 };
        let sy = if dy < 0.0 { -1.0 } else { 1.0 };
        pt(a.x + sx * mm, a.y + sy * mm)
    } else {
        b
    };
    if shift {
        (pt(2.0 * a.x - e.x, 2.0 * a.y - e.y), e, e)
    } else {
        (a, e, e)
    }
}

/// (origin x, origin y, w, h) of the reference rectangle.
fn ref_rect(a: Point, b: Point, shift: bool, ctrl: bool) -> (f64, f64, f64, f64) {
    let (p, q, _) = ref_corners(a, b, shift, ctrl);
    (
        p.x.min(q.x),
        p.y.min(q.y),
        (p.x - q.x).abs(),
        (p.y - q.y).abs(),
    )
}

/// (cx, cy, rx, ry) of the reference ellipse.
fn ref_ellipse(a: Point, b: Point, shift: bool, ctrl: bool) -> (f64, f64, f64, f64) {
    let (p, q, _) = ref_corners(a, b, shift, ctrl);
    (
        f64::midpoint(p.x, q.x),
        f64::midpoint(p.y, q.y),
        (p.x - q.x).abs() / 2.0,
        (p.y - q.y).abs() / 2.0,
    )
}

fn rect_of(d: &Document, o: CreateOutcome) -> (f64, f64, f64, f64, f64, f64) {
    let CreateOutcome::Created(id) = o else {
        panic!("expected Created, got {o:?}");
    };
    let p = d.primitive(id).unwrap();
    let Shape::Rect {
        bounds,
        corner_radius,
    } = p.shape
    else {
        panic!("rect expected")
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
        corner_radius.as_mm(),
        p.rotation.as_radians(),
    )
}

fn ellipse_of(d: &Document, o: CreateOutcome) -> (f64, f64, f64, f64, f64) {
    let CreateOutcome::Created(id) = o else {
        panic!("expected Created, got {o:?}");
    };
    let p = d.primitive(id).unwrap();
    let Shape::Ellipse { frame } = p.shape else {
        panic!("ellipse expected")
    };
    (
        frame.center.x,
        frame.center.y,
        frame.rx.as_mm(),
        frame.ry.as_mm(),
        p.rotation.as_radians(),
    )
}

fn rect_drag(a: Point, b: Point, shift: bool, ctrl: bool) -> (f64, f64, f64, f64, f64, f64) {
    let d = Document::new(1);
    let mut t = RectangleTool::new();
    t.pointer_down(a);
    t.pointer_move(b, m(shift, ctrl));
    let o = t.pointer_up(&d, b, m(shift, ctrl));
    assert_eq!(d.object_ids().len(), 1);
    rect_of(&d, o)
}

fn ellipse_drag(a: Point, b: Point, shift: bool, ctrl: bool) -> (f64, f64, f64, f64, f64) {
    let d = Document::new(1);
    let mut t = EllipseTool::new();
    t.pointer_down(a);
    t.pointer_move(b, m(shift, ctrl));
    let o = t.pointer_up(&d, b, m(shift, ctrl));
    assert_eq!(d.object_ids().len(), 1);
    ellipse_of(&d, o)
}

fn close(got: (f64, f64, f64, f64), want: (f64, f64, f64, f64)) {
    assert!(
        (got.0 - want.0).abs() < EPS
            && (got.1 - want.1).abs() < EPS
            && (got.2 - want.2).abs() < EPS
            && (got.3 - want.3).abs() < EPS,
        "got {got:?}, want {want:?}"
    );
}

const A: (f64, f64) = (100.0, 50.0);

// ---------------------------------------------------------------------
// Rectangle criteria 1-4
// ---------------------------------------------------------------------

#[test]
fn ac01_rect_shift_worked_example() {
    let r = rect_drag(pt(A.0, A.1), pt(130.0, 40.0), true, false);
    close((r.0, r.1, r.2, r.3), (70.0, 40.0, 60.0, 20.0));
    assert!(r.4.abs() < EPS, "corner radius 0");
    assert_eq!(r.5, 0.0, "rotation 0");
}

#[test]
fn ac02_rect_shift_is_independent_of_the_side_of_a() {
    for b in [
        (70.0, 60.0),
        (70.0, 40.0),
        (130.0, 60.0),
        (130.0, 40.0),
        (70.0, 40.0),
    ] {
        let r = rect_drag(pt(A.0, A.1), pt(b.0, b.1), true, false);
        close((r.0, r.1, r.2, r.3), (70.0, 40.0, 60.0, 20.0));
    }
}

#[test]
fn ac03_rect_shift_ctrl_square_larger_extent_wins() {
    let r = rect_drag(pt(A.0, A.1), pt(130.0, 40.0), true, true);
    close((r.0, r.1, r.2, r.3), (70.0, 20.0, 60.0, 60.0));
    let r = rect_drag(pt(A.0, A.1), pt(110.0, 20.0), true, true);
    close((r.0, r.1, r.2, r.3), (70.0, 20.0, 60.0, 60.0));
    // all four quadrants
    for b in [(70.0, 60.0), (70.0, 40.0), (130.0, 60.0), (110.0, 80.0)] {
        let r = rect_drag(pt(A.0, A.1), pt(b.0, b.1), true, true);
        assert!((r.2 - r.3).abs() < EPS, "square");
        assert!(((r.0 + r.2 / 2.0) - 100.0).abs() < EPS);
        assert!(((r.1 + r.3 / 2.0) - 50.0).abs() < EPS);
    }
}

#[test]
fn ac04_rect_ctrl_alone_and_plain_unchanged_golden() {
    let r = rect_drag(pt(A.0, A.1), pt(130.0, 40.0), false, true);
    close((r.0, r.1, r.2, r.3), (100.0, 20.0, 30.0, 30.0));
    let r = rect_drag(pt(A.0, A.1), pt(130.0, 40.0), false, false);
    close((r.0, r.1, r.2, r.3), (100.0, 40.0, 30.0, 10.0));
    // pre-feature golden numbers of primitive-shapes
    let r = rect_drag(pt(10.0, 10.0), pt(30.0, 25.0), false, false);
    close((r.0, r.1, r.2, r.3), (10.0, 10.0, 20.0, 15.0));
    let r = rect_drag(pt(0.0, 0.0), pt(10.0, 30.0), false, true);
    close((r.0, r.1, r.2, r.3), (0.0, 0.0, 30.0, 30.0));
    // Ctrl alone, drag up-left: square grows towards the pointer's quadrant
    let r = rect_drag(pt(0.0, 0.0), pt(-10.0, 30.0), false, true);
    close((r.0, r.1, r.2, r.3), (-30.0, 0.0, 30.0, 30.0));
}

// ---------------------------------------------------------------------
// Ellipse criteria 5-7
// ---------------------------------------------------------------------

#[test]
fn ac05_ellipse_shift_worked_example_and_sides() {
    for b in [(130.0, 40.0), (70.0, 60.0), (70.0, 40.0), (130.0, 60.0)] {
        let e = ellipse_drag(pt(A.0, A.1), pt(b.0, b.1), true, false);
        close((e.0, e.1, e.2, e.3), (100.0, 50.0, 30.0, 10.0));
        assert_eq!(e.4, 0.0, "rotation 0");
    }
}

#[test]
fn ac06_ellipse_shift_ctrl_circle() {
    let e = ellipse_drag(pt(A.0, A.1), pt(130.0, 40.0), true, true);
    close((e.0, e.1, e.2, e.3), (100.0, 50.0, 30.0, 30.0));
    let e = ellipse_drag(pt(A.0, A.1), pt(110.0, 20.0), true, true);
    close((e.0, e.1, e.2, e.3), (100.0, 50.0, 30.0, 30.0));
}

#[test]
fn ac07_ellipse_ctrl_alone_and_plain_unchanged_golden() {
    let e = ellipse_drag(pt(A.0, A.1), pt(130.0, 40.0), false, false);
    close((e.0, e.1, e.2, e.3), (115.0, 45.0, 15.0, 5.0));
    // Ctrl alone: E = (130, 20) -> box (100,20)-(130,50) -> centre (115,35), r 15
    let e = ellipse_drag(pt(A.0, A.1), pt(130.0, 40.0), false, true);
    close((e.0, e.1, e.2, e.3), (115.0, 35.0, 15.0, 15.0));
}

// ---------------------------------------------------------------------
// Whole matrix against the reference model, incl. zero axis
// ---------------------------------------------------------------------

proptest! {
    #[test]
    fn rect_matches_reference_for_all_modes(
        ax in -500.0f64..500.0, ay in -500.0f64..500.0,
        bx in -500.0f64..500.0, by in -500.0f64..500.0,
        mode in 0usize..4,
    ) {
        let (shift, ctrl) = ALL[mode];
        let (a, b) = (pt(ax, ay), pt(bx, by));
        let (p, q, _) = ref_corners(a, b, shift, ctrl);
        prop_assume!((p.x - q.x).abs() > 1e-6 || (p.y - q.y).abs() > 1e-6);
        let d = Document::new(1);
        let mut t = RectangleTool::new();
        t.pointer_down(a);
        t.pointer_move(b, m(shift, ctrl));
        let o = t.pointer_up(&d, b, m(shift, ctrl));
        let got = rect_of(&d, o);
        let want = ref_rect(a, b, shift, ctrl);
        prop_assert!((got.0 - want.0).abs() < 1e-7 && (got.1 - want.1).abs() < 1e-7);
        prop_assert!((got.2 - want.2).abs() < 1e-7 && (got.3 - want.3).abs() < 1e-7);
        if shift {
            prop_assert!(((got.0 + got.2 / 2.0) - ax).abs() < 1e-7);
            prop_assert!(((got.1 + got.3 / 2.0) - ay).abs() < 1e-7);
        }
    }

    #[test]
    fn ellipse_matches_reference_for_all_modes(
        ax in -500.0f64..500.0, ay in -500.0f64..500.0,
        bx in -500.0f64..500.0, by in -500.0f64..500.0,
        mode in 0usize..4,
    ) {
        let (shift, ctrl) = ALL[mode];
        let (a, b) = (pt(ax, ay), pt(bx, by));
        let (p, q, _) = ref_corners(a, b, shift, ctrl);
        prop_assume!((p.x - q.x).abs() > 1e-6 || (p.y - q.y).abs() > 1e-6);
        let d = Document::new(1);
        let mut t = EllipseTool::new();
        t.pointer_down(a);
        t.pointer_move(b, m(shift, ctrl));
        let o = t.pointer_up(&d, b, m(shift, ctrl));
        let got = ellipse_of(&d, o);
        let want = ref_ellipse(a, b, shift, ctrl);
        prop_assert!((got.0 - want.0).abs() < 1e-7 && (got.1 - want.1).abs() < 1e-7);
        prop_assert!((got.2 - want.2).abs() < 1e-7 && (got.3 - want.3).abs() < 1e-7);
        if shift {
            prop_assert!((got.0 - ax).abs() < 1e-7 && (got.1 - ay).abs() < 1e-7);
        }
    }

    /// Criterion 9: for any sequence of moves and modifier changes, the live
    /// preview equals what a release at the same point/modifiers commits.
    #[test]
    fn rect_preview_equals_commit(
        ax in -200.0f64..200.0, ay in -200.0f64..200.0,
        seq in proptest::collection::vec((-300.0f64..300.0, -300.0f64..300.0, 0usize..4), 1..8),
    ) {
        let a = pt(ax, ay);
        let mut t = RectangleTool::new();
        t.pointer_down(a);
        let mut last = (a, 0usize);
        let mut preview = None;
        for (x, y, mode) in &seq {
            let b = pt(*x, *y);
            let (s, c) = ALL[*mode];
            t.pointer_move(b, m(s, c));
            preview = t.live_shape();
            last = (b, *mode);
        }
        let (s, c) = ALL[last.1];
        let d = Document::new(1);
        let o = t.pointer_up(&d, last.0, m(s, c));
        match (preview, o) {
            (None, CreateOutcome::NoOp) => {}
            (Some(p), CreateOutcome::Created(id)) => {
                let committed = d.primitive(id).unwrap().shape;
                let (Shape::Rect { bounds: pb, .. }, Shape::Rect { bounds: cb, .. }) = (p.shape, committed) else {
                    panic!("rect expected")
                };
                prop_assert!((pb.origin.x - cb.origin.x).abs() < EPS);
                prop_assert!((pb.origin.y - cb.origin.y).abs() < EPS);
                prop_assert!((pb.width.as_mm() - cb.width.as_mm()).abs() < EPS);
                prop_assert!((pb.height.as_mm() - cb.height.as_mm()).abs() < EPS);
            }
            (p, o) => prop_assert!(false, "preview {:?} vs commit {:?}", p.is_some(), o),
        }
    }

    #[test]
    fn ellipse_preview_equals_commit(
        ax in -200.0f64..200.0, ay in -200.0f64..200.0,
        seq in proptest::collection::vec((-300.0f64..300.0, -300.0f64..300.0, 0usize..4), 1..8),
    ) {
        let a = pt(ax, ay);
        let mut t = EllipseTool::new();
        t.pointer_down(a);
        let mut last = (a, 0usize);
        let mut preview = None;
        for (x, y, mode) in &seq {
            let b = pt(*x, *y);
            let (s, c) = ALL[*mode];
            t.pointer_move(b, m(s, c));
            preview = t.live_shape();
            last = (b, *mode);
        }
        let (s, c) = ALL[last.1];
        let d = Document::new(1);
        let o = t.pointer_up(&d, last.0, m(s, c));
        match (preview, o) {
            (None, CreateOutcome::NoOp) => {}
            (Some(p), CreateOutcome::Created(id)) => {
                let committed = d.primitive(id).unwrap().shape;
                let (Shape::Ellipse { frame: pf }, Shape::Ellipse { frame: cf }) = (p.shape, committed) else {
                    panic!("ellipse expected")
                };
                prop_assert!((pf.center.x - cf.center.x).abs() < EPS);
                prop_assert!((pf.center.y - cf.center.y).abs() < EPS);
                prop_assert!((pf.rx.as_mm() - cf.rx.as_mm()).abs() < EPS);
                prop_assert!((pf.ry.as_mm() - cf.ry.as_mm()).abs() < EPS);
            }
            (p, o) => prop_assert!(false, "preview {:?} vs commit {:?}", p.is_some(), o),
        }
    }
}

// ---------------------------------------------------------------------
// Zero axis (criterion 12), no movement
// ---------------------------------------------------------------------

#[test]
fn ac12_zero_movement_creates_nothing_and_no_preview_in_all_modes() {
    for (s, c) in ALL {
        let d = Document::new(1);
        let mut r = RectangleTool::new();
        r.pointer_down(pt(5.0, 5.0));
        r.pointer_move(pt(5.0, 5.0), m(s, c));
        assert!(r.live_shape().is_none(), "rect preview {s} {c}");
        assert_eq!(r.pointer_up(&d, pt(5.0, 5.0), m(s, c)), CreateOutcome::NoOp);
        let mut e = EllipseTool::new();
        e.pointer_down(pt(5.0, 5.0));
        e.pointer_move(pt(5.0, 5.0), m(s, c));
        assert!(e.live_shape().is_none(), "ellipse preview {s} {c}");
        assert_eq!(e.pointer_up(&d, pt(5.0, 5.0), m(s, c)), CreateOutcome::NoOp);
        assert_eq!(d.object_ids().len(), 0);
    }
}

#[test]
fn ac12_back_to_a_after_moving_clears_the_preview_and_commits_nothing() {
    for (s, c) in ALL {
        let d = Document::new(1);
        let mut r = RectangleTool::new();
        r.pointer_down(pt(5.0, 5.0));
        r.pointer_move(pt(25.0, 15.0), m(s, c));
        assert!(r.live_shape().is_some());
        r.pointer_move(pt(5.0, 5.0), m(s, c));
        assert!(r.live_shape().is_none());
        assert_eq!(r.pointer_up(&d, pt(5.0, 5.0), m(s, c)), CreateOutcome::NoOp);
        assert_eq!(d.object_ids().len(), 0);
    }
}

#[test]
fn ac12_one_axis_drag_is_created_as_before() {
    // plain: zero height rect is created (spec "Out of scope")
    let r = rect_drag(pt(0.0, 0.0), pt(10.0, 0.0), false, false);
    close((r.0, r.1, r.2, r.3), (0.0, 0.0, 10.0, 0.0));
    // Shift without Ctrl: other dimension is zero
    let r = rect_drag(pt(0.0, 0.0), pt(10.0, 0.0), true, false);
    close((r.0, r.1, r.2, r.3), (-10.0, 0.0, 20.0, 0.0));
    let r = rect_drag(pt(0.0, 0.0), pt(0.0, -10.0), true, false);
    close((r.0, r.1, r.2, r.3), (0.0, -10.0, 0.0, 20.0));
    // Shift+Ctrl with one axis: zero axis counts positive, still a square
    let r = rect_drag(pt(0.0, 0.0), pt(10.0, 0.0), true, true);
    close((r.0, r.1, r.2, r.3), (-10.0, -10.0, 20.0, 20.0));
    let e = ellipse_drag(pt(0.0, 0.0), pt(10.0, 0.0), true, false);
    close((e.0, e.1, e.2, e.3), (0.0, 0.0, 10.0, 0.0));
    let e = ellipse_drag(pt(0.0, 0.0), pt(10.0, 0.0), true, true);
    close((e.0, e.1, e.2, e.3), (0.0, 0.0, 10.0, 10.0));
}

// ---------------------------------------------------------------------
// Criterion 9/10: release modifiers win over the last move's
// ---------------------------------------------------------------------

#[test]
fn ac10_release_modifiers_win_over_the_last_preview_state() {
    for (ps, pc) in ALL {
        for (rs, rc) in ALL {
            let a = pt(A.0, A.1);
            let b = pt(130.0, 40.0);
            let d = Document::new(1);
            let mut t = RectangleTool::new();
            t.pointer_down(a);
            t.pointer_move(b, m(ps, pc));
            let o = t.pointer_up(&d, b, m(rs, rc));
            let got = rect_of(&d, o);
            close((got.0, got.1, got.2, got.3), ref_rect(a, b, rs, rc));
        }
    }
}

#[test]
fn ac09_preview_changes_with_modifiers_at_a_still_pointer_and_matches_golden() {
    let a = pt(A.0, A.1);
    let b = pt(130.0, 40.0);
    let mut t = RectangleTool::new();
    t.pointer_down(a);
    let mut seen = Vec::new();
    // any order of pressing and releasing: re-send the move with new modifiers
    for (s, c) in [
        (false, false),
        (true, false),
        (true, true),
        (false, true),
        (false, false),
        (true, true),
        (true, false),
        (false, false),
    ] {
        t.pointer_move(b, m(s, c));
        let p = t.live_shape().unwrap();
        let Shape::Rect { bounds, .. } = p.shape else {
            panic!()
        };
        let want = ref_rect(a, b, s, c);
        close(
            (
                bounds.origin.x,
                bounds.origin.y,
                bounds.width.as_mm(),
                bounds.height.as_mm(),
            ),
            want,
        );
        // readout anchor is E
        let (_, _, e) = ref_corners(a, b, s, c);
        assert!((p.anchor.x - e.x).abs() < EPS && (p.anchor.y - e.y).abs() < EPS);
        seen.push((s, c));
    }
    assert_eq!(seen.len(), 8);
}

// ---------------------------------------------------------------------
// Escape (criterion 14)
// ---------------------------------------------------------------------

#[test]
fn ac14_escape_cancels_for_both_tools_and_modifier_changes_do_not_revive() {
    for (s, c) in ALL {
        let d = Document::new(1);
        let mut r = RectangleTool::new();
        r.pointer_down(pt(0.0, 0.0));
        r.pointer_move(pt(10.0, 20.0), m(s, c));
        assert!(r.escape());
        assert!(r.live_shape().is_none());
        r.pointer_move(pt(10.0, 20.0), m(!s, !c));
        assert!(r.live_shape().is_none());
        assert_eq!(
            r.pointer_up(&d, pt(10.0, 20.0), m(s, c)),
            CreateOutcome::NoOp
        );
        let mut e = EllipseTool::new();
        e.pointer_down(pt(0.0, 0.0));
        e.pointer_move(pt(10.0, 20.0), m(s, c));
        assert!(e.escape());
        assert!(e.live_shape().is_none());
        e.pointer_move(pt(10.0, 20.0), m(!s, !c));
        assert!(e.live_shape().is_none());
        assert_eq!(
            e.pointer_up(&d, pt(10.0, 20.0), m(s, c)),
            CreateOutcome::NoOp
        );
        assert_eq!(d.object_ids().len(), 0);
    }
}

// ---------------------------------------------------------------------
// Criterion 13: defaults identical across modes; exactly one object
// ---------------------------------------------------------------------

#[test]
fn ac13_stroke_fill_defaults_identical_in_all_modes() {
    let mut reference = None;
    for (s, c) in ALL {
        let d = Document::new(1);
        let mut t = RectangleTool::new();
        t.pointer_down(pt(0.0, 0.0));
        let CreateOutcome::Created(id) = t.pointer_up(&d, pt(10.0, 5.0), m(s, c)) else {
            panic!()
        };
        let p = d.primitive(id).unwrap();
        let key = (p.stroke_width, p.stroke, p.fill, p.rotation);
        assert!(p.fill.is_none());
        if let Some(r) = reference {
            assert_eq!(r, key);
        } else {
            reference = Some(key);
        }
        let d2 = Document::new(1);
        let mut e = EllipseTool::new();
        e.pointer_down(pt(0.0, 0.0));
        let CreateOutcome::Created(id) = e.pointer_up(&d2, pt(10.0, 5.0), m(s, c)) else {
            panic!()
        };
        let p = d2.primitive(id).unwrap();
        assert_eq!(
            (p.stroke_width, p.stroke, p.fill, p.rotation),
            reference.unwrap()
        );
    }
}

#[test]
fn ac13_second_pointer_up_creates_nothing_more() {
    let d = Document::new(1);
    let mut t = RectangleTool::new();
    t.pointer_down(pt(0.0, 0.0));
    assert!(matches!(
        t.pointer_up(&d, pt(10.0, 5.0), m(true, false)),
        CreateOutcome::Created(_)
    ));
    assert_eq!(
        t.pointer_up(&d, pt(10.0, 5.0), m(true, false)),
        CreateOutcome::NoOp
    );
    assert_eq!(d.object_ids().len(), 1);
}

// ---------------------------------------------------------------------
// Hostile pointer values
// ---------------------------------------------------------------------

/// Hostile (NaN / infinite / extreme) pointer values must never panic. The
/// stored geometry should stay finite; non-finite results are collected per
/// mode so a pre-existing gap (plain drag, no modifier) is told apart from a
/// regression in the new modes.
#[test]
fn hostile_pointer_values_do_not_panic() {
    let bad = hostile_run();
    // Pre-existing gap, not caused by this feature: the plain drag (no
    // modifier) stores non-finite geometry for NaN/inf pointers too. See
    // the ignored strict test below.
    assert!(
        bad.contains_key(&(false, false, "rect")),
        "expected the pre-existing plain-drag gap to be what the probe sees"
    );
}

#[test]
#[ignore = "pre-existing gap: NaN/inf pointer values reach the document in every mode, plain drag included"]
fn hostile_pointer_values_never_store_non_finite() {
    let bad = hostile_run();
    assert!(bad.is_empty(), "non-finite geometry stored: {bad:?}");
}

fn hostile_run() -> std::collections::BTreeMap<(bool, bool, &'static str), usize> {
    let mut bad: std::collections::BTreeMap<(bool, bool, &str), usize> =
        std::collections::BTreeMap::new();
    let vals = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        -0.0,
        1e308,
        1e-320,
    ];
    for (s, c) in ALL {
        for &x in &vals {
            for &y in &vals {
                for a in [pt(0.0, 0.0), pt(100.0, 50.0), pt(x, y)] {
                    let d = Document::new(1);
                    let mut r = RectangleTool::new();
                    r.pointer_down(a);
                    r.pointer_move(pt(x, y), m(s, c));
                    let _ = r.live_shape();
                    let o = r.pointer_up(&d, pt(x, y), m(s, c));
                    if let CreateOutcome::Created(id) = o {
                        let Shape::Rect { bounds, .. } = d.primitive(id).unwrap().shape else {
                            panic!()
                        };
                        if !(bounds.origin.x.is_finite()
                            && bounds.origin.y.is_finite()
                            && bounds.width.as_mm().is_finite()
                            && bounds.height.as_mm().is_finite())
                        {
                            *bad.entry((s, c, "rect")).or_default() += 1;
                        }
                    }
                    let d = Document::new(1);
                    let mut e = EllipseTool::new();
                    e.pointer_down(a);
                    e.pointer_move(pt(x, y), m(s, c));
                    let _ = e.live_shape();
                    if let CreateOutcome::Created(id) = e.pointer_up(&d, pt(x, y), m(s, c)) {
                        let Shape::Ellipse { frame } = d.primitive(id).unwrap().shape else {
                            panic!()
                        };
                        if !(frame.center.x.is_finite()
                            && frame.center.y.is_finite()
                            && frame.rx.as_mm().is_finite()
                            && frame.ry.as_mm().is_finite())
                        {
                            *bad.entry((s, c, "ellipse")).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    bad
}

#[test]
fn huge_but_finite_extents_stay_centred_on_a() {
    // 2A - E may overflow only beyond f64::MAX; 1e300 is fine
    let r = rect_drag(pt(0.0, 0.0), pt(1e300, -1e300), true, false);
    assert!((r.0 + r.2 / 2.0).abs() < 1e290);
    assert!((r.2 - 2e300).abs() < 1e290);
}

// ---------------------------------------------------------------------
// Criterion 17: polygon / star unchanged by Shift; Ctrl keeps its snap
// ---------------------------------------------------------------------

#[test]
fn ac17_polygon_and_star_shift_changes_nothing() {
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        let make = |shift: bool| {
            let d = Document::new(1);
            let mut t = PolygonStarTool::new();
            t.set_mode(mode);
            t.pointer_down(pt(100.0, 50.0));
            t.pointer_move(pt(113.0, 41.0), m(shift, false));
            let live = t.live_shape().map(|p| (p.shape, p.anchor));
            let o = t.pointer_up(&d, pt(113.0, 41.0), m(shift, false));
            let CreateOutcome::Created(id) = o else {
                panic!("created")
            };
            let p = d.primitive(id).unwrap();
            (live, p.shape, p.rotation)
        };
        let plain = make(false);
        let shifted = make(true);
        assert_eq!(plain, shifted, "Shift has no effect on {mode:?}");
        // centre is the press point
        let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = plain.1 else {
            panic!()
        };
        assert!((frame.center.x - 100.0).abs() < EPS && (frame.center.y - 50.0).abs() < EPS);
    }
}

// ---------------------------------------------------------------------
// White-box edges seen in the diff (`2A - E` arithmetic, degenerate test)
// ---------------------------------------------------------------------

#[test]
fn whitebox_tiny_extents_under_shift_are_consistent_between_preview_and_commit() {
    for d in [1e-15, 1e-12, 1e-9, 1e-7, 1e-4] {
        for (s, c) in ALL {
            let a = pt(100.0, 50.0);
            let b = pt(100.0 + d, 50.0 + d);
            let doc = Document::new(1);
            let mut t = RectangleTool::new();
            t.pointer_down(a);
            t.pointer_move(b, m(s, c));
            let preview = t.live_shape();
            let o = t.pointer_up(&doc, b, m(s, c));
            assert_eq!(
                preview.is_some(),
                matches!(o, CreateOutcome::Created(_)),
                "d={d} s={s} c={c}"
            );
            if let (Some(p), CreateOutcome::Created(id)) = (preview, o) {
                assert_eq!(p.shape, doc.primitive(id).unwrap().shape);
            }
        }
    }
}

#[test]
#[ignore = "tool-level only: Session clamps pointer coordinates (MAX_POINTER_COORDINATE_MM), and a plain drag between the same extremes overflows the width too (pre-existing)"]
fn whitebox_shift_mirror_overflow_does_not_store_infinity_for_finite_pointers() {
    // A and B finite, 2A - E overflows f64: the stored box must not be inf.
    let a = pt(1e308, 0.0);
    let b = pt(-1e308, 10.0);
    let doc = Document::new(1);
    let mut t = RectangleTool::new();
    t.pointer_down(a);
    t.pointer_move(b, m(true, false));
    if let CreateOutcome::Created(id) = t.pointer_up(&doc, b, m(true, false)) {
        let Shape::Rect { bounds, .. } = doc.primitive(id).unwrap().shape else {
            panic!()
        };
        assert!(
            bounds.origin.x.is_finite() && bounds.width.as_mm().is_finite(),
            "overflow stored: {bounds:?}"
        );
    }
}
