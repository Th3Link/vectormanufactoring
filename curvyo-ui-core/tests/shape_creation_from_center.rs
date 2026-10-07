//! Acceptance tests for `specs/shape-creation-from-center/specification.md`
//! (criteria 1 to 7, 9, 10, 12, 13, 17), against the public API of the
//! rectangle, ellipse and polygon/star tools. The numbers are the spec's.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{Document, Point, PointCount, Shape};
use curvyo_ui_core::{CreateOutcome, EllipseTool, Modifiers, PolygonStarTool, RectangleTool};
use proptest::prelude::*;

const TOL: f64 = 1e-9;

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::new(true, false);
const CTRL: Modifiers = Modifiers::new(false, true);
const BOTH: Modifiers = Modifiers::new(true, true);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

/// Drags a rectangle from `a` to `b` under `m` and returns (origin x, origin
/// y, width, height, corner radius); asserts that the commit equals the last
/// preview.
fn drag_rect(a: Point, b: Point, m: Modifiers) -> (f64, f64, f64, f64, f64) {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(a);
    tool.pointer_move(b, m);
    let preview = tool.live_shape().expect("a preview").shape;
    let CreateOutcome::Created(id) = tool.pointer_up(&document, b, m) else {
        panic!("expected Created");
    };
    let shape = document.primitive(id).unwrap().shape;
    assert_eq!(shape, preview, "AC 9: the commit is the last preview");
    let Shape::Rect {
        bounds,
        corner_radius,
    } = shape
    else {
        panic!("a rectangle");
    };
    assert_eq!(document.object_ids().len(), 1, "AC 13: one object");
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
        corner_radius.as_mm(),
    )
}

/// The same for an ellipse: (centre x, centre y, rx, ry).
fn drag_ellipse(a: Point, b: Point, m: Modifiers) -> (f64, f64, f64, f64) {
    let document = Document::new(1);
    let mut tool = EllipseTool::new();
    tool.pointer_down(a);
    tool.pointer_move(b, m);
    let preview = tool.live_shape().expect("a preview").shape;
    let CreateOutcome::Created(id) = tool.pointer_up(&document, b, m) else {
        panic!("expected Created");
    };
    let shape = document.primitive(id).unwrap().shape;
    assert_eq!(shape, preview, "AC 9: the commit is the last preview");
    let Shape::Ellipse { frame } = shape else {
        panic!("an ellipse");
    };
    (
        frame.center.x,
        frame.center.y,
        frame.rx.as_mm(),
        frame.ry.as_mm(),
    )
}

fn rect_is(got: (f64, f64, f64, f64, f64), x: f64, y: f64, w: f64, h: f64) -> bool {
    near(got.0, x) && near(got.1, y) && near(got.2, w) && near(got.3, h) && near(got.4, 0.0)
}

fn ellipse_is(got: (f64, f64, f64, f64), cx: f64, cy: f64, rx: f64, ry: f64) -> bool {
    near(got.0, cx) && near(got.1, cy) && near(got.2, rx) && near(got.3, ry)
}

#[test]
fn ac1_shift_draws_a_rectangle_around_the_press_point() {
    let got = drag_rect(pt(100.0, 50.0), pt(130.0, 40.0), SHIFT);
    assert!(rect_is(got, 70.0, 40.0, 60.0, 20.0), "{got:?}");
}

#[test]
fn ac2_the_side_of_the_press_point_does_not_matter() {
    for b in [pt(70.0, 60.0), pt(70.0, 40.0), pt(130.0, 60.0)] {
        let got = drag_rect(pt(100.0, 50.0), b, SHIFT);
        assert!(rect_is(got, 70.0, 40.0, 60.0, 20.0), "{b:?}: {got:?}");
    }
}

#[test]
fn ac3_shift_ctrl_makes_a_square_around_the_press_point() {
    for b in [pt(130.0, 40.0), pt(110.0, 20.0)] {
        let got = drag_rect(pt(100.0, 50.0), b, BOTH);
        assert!(rect_is(got, 70.0, 20.0, 60.0, 60.0), "{b:?}: {got:?}");
    }
}

#[test]
fn ac4_ctrl_alone_and_no_modifier_are_unchanged() {
    let got = drag_rect(pt(100.0, 50.0), pt(130.0, 40.0), CTRL);
    assert!(rect_is(got, 100.0, 20.0, 30.0, 30.0), "{got:?}");
    let got = drag_rect(pt(100.0, 50.0), pt(130.0, 40.0), NONE);
    assert!(rect_is(got, 100.0, 40.0, 30.0, 10.0), "{got:?}");
}

#[test]
fn ac5_shift_draws_an_ellipse_around_the_press_point() {
    for b in [
        pt(130.0, 40.0),
        pt(70.0, 60.0),
        pt(70.0, 40.0),
        pt(130.0, 60.0),
    ] {
        let got = drag_ellipse(pt(100.0, 50.0), b, SHIFT);
        assert!(ellipse_is(got, 100.0, 50.0, 30.0, 10.0), "{b:?}: {got:?}");
    }
}

#[test]
fn ac6_shift_ctrl_makes_a_circle_around_the_press_point() {
    for b in [pt(130.0, 40.0), pt(110.0, 20.0)] {
        let got = drag_ellipse(pt(100.0, 50.0), b, BOTH);
        assert!(ellipse_is(got, 100.0, 50.0, 30.0, 30.0), "{b:?}: {got:?}");
    }
}

#[test]
fn ac7_ctrl_alone_and_no_modifier_ellipses_are_unchanged() {
    let got = drag_ellipse(pt(100.0, 50.0), pt(130.0, 40.0), NONE);
    assert!(ellipse_is(got, 115.0, 45.0, 15.0, 5.0), "{got:?}");
    let got = drag_ellipse(pt(100.0, 50.0), pt(130.0, 40.0), CTRL);
    assert!(ellipse_is(got, 115.0, 35.0, 15.0, 15.0), "{got:?}");
}

/// AC 10: the release event's modifiers decide, not the last move's.
#[test]
fn ac10_the_release_modifiers_win_over_the_last_preview() {
    let document = Document::new(1);
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(100.0, 50.0));
    tool.pointer_move(pt(130.0, 40.0), SHIFT);
    let CreateOutcome::Created(id) = tool.pointer_up(&document, pt(130.0, 40.0), NONE) else {
        panic!("expected Created");
    };
    let Shape::Rect { bounds, .. } = document.primitive(id).unwrap().shape else {
        panic!("a rectangle");
    };
    assert!(near(bounds.origin.x, 100.0) && near(bounds.width.as_mm(), 30.0));

    let mut tool = EllipseTool::new();
    tool.pointer_down(pt(100.0, 50.0));
    tool.pointer_move(pt(130.0, 40.0), NONE);
    let CreateOutcome::Created(id) = tool.pointer_up(&document, pt(130.0, 40.0), BOTH) else {
        panic!("expected Created");
    };
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("an ellipse");
    };
    assert!(near(frame.center.x, 100.0) && near(frame.rx.as_mm(), 30.0));
}

/// AC 8 at tool level: a modifier change with the pointer at rest is a
/// `pointer_move` at the same point, and changes the preview.
#[test]
fn ac8_a_modifier_change_at_the_same_point_changes_the_preview() {
    let mut tool = RectangleTool::new();
    tool.pointer_down(pt(100.0, 50.0));
    let at = pt(130.0, 40.0);
    let mut widths = Vec::new();
    for m in [NONE, CTRL, BOTH, SHIFT, NONE] {
        tool.pointer_move(at, m);
        let Shape::Rect { bounds, .. } = tool.live_shape().unwrap().shape else {
            panic!("a rectangle");
        };
        widths.push((bounds.width.as_mm(), bounds.height.as_mm()));
    }
    assert_eq!(
        widths,
        vec![
            (30.0, 10.0),
            (30.0, 30.0),
            (60.0, 60.0),
            (60.0, 20.0),
            (30.0, 10.0)
        ]
    );
}

/// AC 12: at the press point there is no preview and no commit, whatever the
/// modifiers.
#[test]
fn ac12_a_pointer_at_the_press_point_shows_and_creates_nothing() {
    for m in [NONE, CTRL, SHIFT, BOTH] {
        let document = Document::new(1);
        let a = pt(100.0, 50.0);
        let mut rect = RectangleTool::new();
        rect.pointer_down(a);
        rect.pointer_move(a, m);
        assert_eq!(rect.live_shape(), None);
        assert_eq!(rect.pointer_up(&document, a, m), CreateOutcome::NoOp);
        let mut ellipse = EllipseTool::new();
        ellipse.pointer_down(a);
        ellipse.pointer_move(a, m);
        assert_eq!(ellipse.live_shape(), None);
        assert_eq!(ellipse.pointer_up(&document, a, m), CreateOutcome::NoOp);
        assert_eq!(document.object_ids().len(), 0);
    }
}

/// AC 12: a one-axis drag is created as today; under Shift the other
/// dimension is zero and the first twice as long.
#[test]
fn ac12_a_one_axis_drag_is_created_as_before() {
    let got = drag_rect(pt(100.0, 50.0), pt(130.0, 50.0), SHIFT);
    assert!(rect_is(got, 70.0, 50.0, 60.0, 0.0), "{got:?}");
    let got = drag_rect(pt(100.0, 50.0), pt(130.0, 50.0), NONE);
    assert!(rect_is(got, 100.0, 50.0, 30.0, 0.0), "{got:?}");
}

/// AC 17: Shift has no effect on a polygon's create-drag.
#[test]
fn ac17_shift_does_not_change_a_polygon_create_drag() {
    let make = |m: Modifiers| {
        let document = Document::new(1);
        let mut tool = PolygonStarTool::new();
        tool.set_point_count(PointCount::new(5).unwrap());
        tool.pointer_down(pt(100.0, 50.0));
        tool.pointer_move(pt(130.0, 40.0), m);
        let preview = tool.live_shape().unwrap().shape;
        let CreateOutcome::Created(id) = tool.pointer_up(&document, pt(130.0, 40.0), m) else {
            panic!("expected Created");
        };
        assert_eq!(document.primitive(id).unwrap().shape, preview);
        preview
    };
    assert_eq!(make(NONE), make(SHIFT));
    assert_eq!(make(CTRL), make(BOTH));
}

fn modifiers_strategy() -> impl Strategy<Value = Modifiers> {
    (any::<bool>(), any::<bool>()).prop_map(|(s, c)| Modifiers::new(s, c))
}

fn coord() -> impl Strategy<Value = f64> {
    -1000.0..1000.0_f64
}

proptest! {
    /// AC 1, 5: under Shift without Ctrl the press point is the centre of
    /// whatever is created, for any pointer position.
    #[test]
    fn shift_makes_the_press_point_the_centre(
        ax in coord(), ay in coord(), bx in coord(), by in coord(), ctrl in any::<bool>(),
    ) {
        let m = Modifiers::new(true, ctrl);
        let (a, b) = (pt(ax, ay), pt(bx, by));
        prop_assume!(a != b);

        let (ex, ey) = (b.x - a.x, b.y - a.y);
        let extent = ex.abs().max(ey.abs());
        let (w, h) = if ctrl { (extent, extent) } else { (ex.abs(), ey.abs()) };

        let (cx, cy, rx, ry) = drag_ellipse(a, b, m);
        prop_assert!((cx - ax).abs() < TOL && (cy - ay).abs() < TOL);
        prop_assert!((rx - w).abs() < TOL && (ry - h).abs() < TOL);

        let (ox, oy, rw, rh, _) = drag_rect(a, b, m);
        prop_assert!((ox + rw / 2.0 - ax).abs() < TOL && (oy + rh / 2.0 - ay).abs() < TOL);
        prop_assert!((rw - 2.0 * w).abs() < TOL && (rh - 2.0 * h).abs() < TOL);
    }

    /// AC 9, 10: after any sequence of moves and modifier changes, the last
    /// preview is the shape that a release at the same point with the same
    /// modifiers commits.
    #[test]
    fn the_last_preview_is_what_release_commits(
        a in (coord(), coord()),
        moves in prop::collection::vec(((coord(), coord()), modifiers_strategy()), 1..8),
    ) {
        let a = pt(a.0, a.1);
        let document = Document::new(1);
        let mut rect = RectangleTool::new();
        let mut ellipse = EllipseTool::new();
        rect.pointer_down(a);
        ellipse.pointer_down(a);
        let mut last = (a, Modifiers::NONE);
        for ((x, y), m) in moves {
            let p = pt(x, y);
            rect.pointer_move(p, m);
            ellipse.pointer_move(p, m);
            last = (p, m);
        }
        let (p, m) = last;
        let rect_preview = rect.live_shape();
        let ellipse_preview = ellipse.live_shape();
        let rect_out = rect.pointer_up(&document, p, m);
        let ellipse_out = ellipse.pointer_up(&document, p, m);
        for (preview, out) in [(rect_preview, rect_out), (ellipse_preview, ellipse_out)] {
            match (preview, out) {
                (None, CreateOutcome::NoOp) => {}
                (Some(preview), CreateOutcome::Created(id)) => {
                    prop_assert_eq!(document.primitive(id).unwrap().shape, preview.shape);
                }
                other => prop_assert!(false, "preview and commit disagree: {:?}", other),
            }
        }
    }
}
