//! The Select tool's marquee, lasso and Alt-click cycle
//! (`specs/0014-advanced-selection/specification.md`, criteria 3 to 20) at the
//! `SelectTool` level: presses, moves and releases with explicit modifiers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, GestureKind, GestureShape, MarqueeMode, Modifiers, ObjectSelection,
    SelectPointerDownOutcome, SelectTool, SelectionCombine, TransformHandleTolerances,
};

/// Screen scale 4 px/mm: the 3 px dead zone is 0.75 mm.
const SCALE: f64 = 4.0;
const TOLERANCE_MM: f64 = 2.0;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::new(true, false);
const CTRL: Modifiers = Modifiers::new(false, true);
const ALT: Modifiers = Modifiers::NONE.with_alt(true);

struct Rig {
    document: Document,
    selection: ObjectSelection,
    tool: SelectTool,
    minter: AnchorIdMinter,
}

impl Rig {
    /// Three 10 mm squares in a row at x = 0, 20 and 40 (y 0 to 10).
    fn new() -> (Self, [NodeId; 3]) {
        let document = Document::new(1);
        let ids = [0.0, 20.0, 40.0].map(|x| {
            document.create_rect(RectBounds {
                origin: pt(x, 0.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            })
        });
        (
            Self {
                document,
                selection: ObjectSelection::new(),
                tool: SelectTool::new(),
                minter: AnchorIdMinter::new(9),
            },
            ids,
        )
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    fn press(&mut self, at: Point, modifiers: Modifiers) -> SelectPointerDownOutcome {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            Tolerance::from_mm(TOLERANCE_MM),
            TransformHandleTolerances::at_scale(SCALE),
            modifiers,
        )
    }

    fn moved(&mut self, to: Point, modifiers: Modifiers) {
        self.tool.pointer_moved(to, modifiers, &mut self.selection);
    }

    fn release(&mut self, at: Point, modifiers: Modifiers) {
        let objects = self.objects();
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            at,
            modifiers,
            &mut self.minter,
        );
    }

    /// A whole drag from `from` to `to`, released with `release_modifiers`.
    fn drag(&mut self, from: Point, to: Point, press: Modifiers, release: Modifiers) {
        self.press(from, press);
        self.moved(
            pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y)),
            release,
        );
        self.moved(to, release);
        self.release(to, release);
    }

    fn select(&mut self, ids: &[NodeId]) {
        self.selection.set(ids);
    }
}

// ---- marquee (criteria 8 to 15) ----

/// Criterion 8: inside the dead zone it is a click; a plain click clears, a
/// Shift or Ctrl click leaves the selection alone.
#[test]
fn a_click_on_empty_canvas_clears_only_without_shift_or_ctrl() {
    for (modifiers, cleared) in [(NONE, true), (SHIFT, false), (CTRL, false)] {
        let (mut rig, [a, ..]) = Rig::new();
        rig.select(&[a]);
        rig.press(pt(100.0, 100.0), modifiers);
        rig.moved(pt(100.3, 100.2), modifiers);
        rig.release(pt(100.3, 100.2), modifiers);
        assert_eq!(rig.selection.is_empty(), cleared, "{modifiers:?}");
    }
}

/// Criterion 9: leftward is touch (green) and selects what the box crosses.
#[test]
fn a_leftward_drag_selects_every_object_it_touches() {
    let (mut rig, [a, b, c]) = Rig::new();
    // From right of c back across b's right half.
    rig.drag(pt(60.0, 5.0), pt(25.0, 5.0), NONE, NONE);
    assert_eq!(rig.selection.ids(), &[b, c]);
    let _ = a;
}

/// Criterion 10: rightward is contain (red): only what lies fully inside.
#[test]
fn a_rightward_drag_selects_only_what_it_contains() {
    let (mut rig, [a, b, _]) = Rig::new();
    rig.drag(pt(-5.0, -5.0), pt(25.0, 15.0), NONE, NONE);
    assert_eq!(rig.selection.ids(), &[a], "b is only crossed");
    // Deselect first: a lone selected object has handles at its corners.
    rig.selection.clear();
    rig.drag(pt(-5.0, -5.0), pt(35.0, 15.0), NONE, NONE);
    assert_eq!(rig.selection.ids(), &[a, b]);
}

/// Criterion 10: a drag with no net horizontal movement is touch.
#[test]
fn a_vertical_drag_counts_as_leftward() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.drag(pt(5.0, 50.0), pt(5.0, -20.0), NONE, NONE);
    assert_eq!(rig.selection.ids(), &[a]);
    let _ = (b, c);
}

/// Criterion 11: Alt at release inverts the mode of the same drag.
#[test]
fn alt_at_release_inverts_the_mode() {
    let (mut rig, [a, b, _]) = Rig::new();
    // Rightward would contain only `a`; inverted it touches `a` and `b`.
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(25.0, 15.0), ALT);
    rig.release(pt(25.0, 15.0), ALT);
    assert_eq!(rig.selection.ids(), &[a, b]);
}

/// Criterion 12: Shift adds to the selection, even objects outside the box.
#[test]
fn shift_adds_the_result_to_the_selection() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[c]);
    rig.drag(pt(-5.0, -5.0), pt(15.0, 15.0), SHIFT, SHIFT);
    assert_eq!(rig.selection.ids(), &[c, a]);
    let _ = b;
}

/// Criterion 13: Ctrl strictly removes: an object the drag touches that was
/// not selected is never added.
#[test]
fn ctrl_removes_the_result_and_never_adds() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[a, c]);
    rig.drag(pt(-5.0, -5.0), pt(25.0, 15.0), CTRL, CTRL);
    assert_eq!(rig.selection.ids(), &[c], "a removed, b not added");
    let _ = b;
}

/// Criterion 13: Shift and Ctrl together remove (Ctrl wins).
#[test]
fn shift_with_ctrl_removes() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[a, b, c]);
    let both = Modifiers::new(true, true);
    rig.drag(pt(-5.0, -5.0), pt(15.0, 15.0), both, both);
    assert_eq!(rig.selection.ids(), &[b, c]);
}

/// Criteria 12 and 13 read the modifiers at the release, not at the press.
#[test]
fn the_combine_is_the_one_held_at_release() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[b, c]);
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(15.0, 15.0), SHIFT);
    rig.release(pt(15.0, 15.0), SHIFT);
    assert_eq!(rig.selection.ids(), &[b, c, a]);
}

/// A plain empty press clears at the press (as slice 4 always did), and the
/// marquee then replaces: touching nothing leaves nothing selected.
#[test]
fn a_plain_marquee_that_finds_nothing_leaves_nothing_selected() {
    let (mut rig, [a, ..]) = Rig::new();
    rig.select(&[a]);
    rig.drag(pt(100.0, 100.0), pt(120.0, 120.0), NONE, NONE);
    assert!(rig.selection.is_empty());
}

/// Criterion 14: the live gesture follows direction and all three
/// modifiers with the pointer at rest, and is absent inside the dead zone.
#[test]
fn the_live_gesture_follows_direction_and_modifiers() {
    let (mut rig, _) = Rig::new();
    let start = pt(10.0, 30.0);
    assert_eq!(
        rig.press(start, NONE),
        SelectPointerDownOutcome::Marquee,
        "an empty press arms a marquee"
    );
    assert_eq!(rig.tool.gesture_kind(), Some(GestureKind::Marquee));
    assert!(rig.tool.live_gesture(pt(10.2, 30.1), NONE).is_none());

    let right = pt(30.0, 50.0);
    rig.moved(right, NONE);
    let live = rig
        .tool
        .live_gesture(right, NONE)
        .expect("past the dead zone");
    assert_eq!(live.combine, SelectionCombine::Replace);
    assert_eq!(
        live.shape,
        GestureShape::Box {
            from: start,
            to: right,
            mode: MarqueeMode::Contain
        }
    );
    let inverted = rig.tool.live_gesture(right, SHIFT.with_alt(true)).unwrap();
    assert_eq!(inverted.combine, SelectionCombine::Add);
    assert!(matches!(
        inverted.shape,
        GestureShape::Box {
            mode: MarqueeMode::Touch,
            ..
        }
    ));
    let left = pt(-5.0, 50.0);
    let reversed = rig.tool.live_gesture(left, CTRL).unwrap();
    assert_eq!(reversed.combine, SelectionCombine::Remove);
    assert!(matches!(
        reversed.shape,
        GestureShape::Box {
            mode: MarqueeMode::Touch,
            ..
        }
    ));
}

/// Criterion 15: a marquee over objects moves and writes nothing.
#[test]
fn a_marquee_never_moves_anything() {
    let (mut rig, _) = Rig::new();
    let before = rig.objects();
    rig.drag(pt(-5.0, -5.0), pt(60.0, 15.0), NONE, NONE);
    assert_eq!(rig.objects(), before);
}

/// Escape cancels the marquee and writes nothing; the release then does
/// nothing either.
#[test]
fn escape_cancels_a_marquee() {
    let (mut rig, [a, ..]) = Rig::new();
    rig.press(pt(-5.0, -5.0), NONE);
    rig.moved(pt(15.0, 15.0), NONE);
    assert!(rig.tool.drag_in_flight());
    rig.tool.escape();
    assert!(!rig.tool.drag_in_flight());
    rig.release(pt(15.0, 15.0), NONE);
    assert!(rig.selection.is_empty());
    let _ = a;
}

// ---- lasso (criteria 16 to 20) ----

/// Criteria 16 and 17: Alt at the press arms a lasso anywhere, also on an
/// object (never a move of it) and on a drawn handle.
#[test]
fn alt_at_the_press_arms_a_lasso_even_on_an_object() {
    let (mut rig, [a, ..]) = Rig::new();
    let before = rig.objects();
    assert_eq!(
        rig.press(pt(0.0, 5.0), ALT),
        SelectPointerDownOutcome::Lasso,
        "on a's outline"
    );
    assert_eq!(rig.tool.gesture_kind(), Some(GestureKind::Lasso));
    assert!(!rig.tool.move_in_flight());
    rig.moved(pt(0.0, 25.0), ALT);
    rig.release(pt(0.0, 25.0), ALT);
    assert_eq!(rig.objects(), before, "nothing moved");
    assert_eq!(rig.selection.ids(), &[a]);
}

/// Criterion 16: the line is live feedback once past the dead zone, and the
/// selection has not changed yet.
#[test]
fn the_lasso_line_is_live_and_changes_nothing_until_release() {
    let (mut rig, [a, ..]) = Rig::new();
    rig.select(&[a]);
    rig.press(pt(100.0, 100.0), ALT);
    assert!(rig.tool.live_gesture(pt(100.2, 100.0), ALT).is_none());
    rig.moved(pt(110.0, 100.0), ALT);
    rig.moved(pt(120.0, 110.0), ALT);
    let live = rig.tool.live_gesture(pt(120.0, 110.0), ALT).unwrap();
    match live.shape {
        GestureShape::Line(points) => {
            assert_eq!(points.first(), Some(&pt(100.0, 100.0)));
            assert_eq!(points.last(), Some(&pt(120.0, 110.0)));
        }
        other @ GestureShape::Box { .. } => panic!("a line, not {other:?}"),
    }
    assert_eq!(rig.selection.ids(), &[a], "unchanged until release");
}

/// Criterion 18: every outline the line comes near is selected, replacing.
#[test]
fn a_lasso_selects_every_outline_it_crosses() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[c]);
    // Across a and b, below c.
    rig.drag(pt(-5.0, 5.0), pt(25.0, 5.0), ALT, ALT);
    assert_eq!(rig.selection.ids(), &[a, b]);
}

/// Criterion 19: Shift adds, Ctrl removes, Ctrl wins over Shift.
#[test]
fn the_lasso_combines_with_shift_and_ctrl() {
    let (mut rig, [a, b, c]) = Rig::new();
    rig.select(&[c]);
    let alt_shift = SHIFT.with_alt(true);
    rig.drag(pt(-5.0, 5.0), pt(25.0, 5.0), alt_shift, alt_shift);
    assert_eq!(rig.selection.ids(), &[c, a, b]);
    let alt_ctrl = CTRL.with_alt(true);
    rig.drag(pt(-5.0, 5.0), pt(15.0, 5.0), alt_ctrl, alt_ctrl);
    assert_eq!(rig.selection.ids(), &[c, b]);
    let all = Modifiers::new(true, true).with_alt(true);
    rig.drag(pt(15.0, 5.0), pt(25.0, 5.0), all, all);
    assert_eq!(rig.selection.ids(), &[c]);
}

/// Criterion 19: Alt released mid-drag has no effect on a lasso.
#[test]
fn releasing_alt_mid_lasso_keeps_it_a_lasso() {
    let (mut rig, [a, b, _]) = Rig::new();
    rig.press(pt(-5.0, 5.0), ALT);
    rig.moved(pt(10.0, 5.0), NONE);
    rig.moved(pt(25.0, 5.0), NONE);
    assert_eq!(rig.tool.gesture_kind(), Some(GestureKind::Lasso));
    rig.release(pt(25.0, 5.0), NONE);
    assert_eq!(rig.selection.ids(), &[a, b]);
}

/// Criterion 20: a line that touches nothing clears on a plain Alt-drag and
/// leaves the selection with Alt+Shift or Alt+Ctrl.
#[test]
fn a_lasso_that_touches_nothing_clears_only_when_plain() {
    for (modifiers, cleared) in [
        (ALT, true),
        (SHIFT.with_alt(true), false),
        (CTRL.with_alt(true), false),
    ] {
        let (mut rig, [a, ..]) = Rig::new();
        rig.select(&[a]);
        rig.drag(pt(100.0, 100.0), pt(120.0, 130.0), modifiers, modifiers);
        assert_eq!(rig.selection.is_empty(), cleared, "{modifiers:?}");
    }
}

/// Criterion 20: through an unfilled interior, without crossing the outline.
#[test]
fn a_lasso_inside_an_unfilled_shape_selects_nothing() {
    let (mut rig, _) = Rig::new();
    rig.drag(pt(3.0, 3.0), pt(7.0, 7.0), ALT, ALT);
    assert!(rig.selection.is_empty());
}

// ---- the Alt-click cycle (criteria 3 to 7) ----

/// Three horizontal lines stacked 0.5 mm apart: a point at y = 0 has them
/// all within tolerance, nearest the last drawn.
fn cycle_rig() -> (Rig, [NodeId; 3]) {
    let document = Document::new(1);
    let make = |n: u64, y: f64| {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 2 * n), pt(0.0, y)),
                NewAnchor::corner(AnchorId::new(1, 2 * n + 1), pt(100.0, y)),
            ],
            false,
        )
    };
    let far = make(1, -1.0);
    let mid = make(2, -0.5);
    let near = make(3, -0.1);
    (
        Rig {
            document,
            selection: ObjectSelection::new(),
            tool: SelectTool::new(),
            minter: AnchorIdMinter::new(9),
        },
        [near, mid, far],
    )
}

fn click(rig: &mut Rig, at: Point, modifiers: Modifiers) {
    rig.press(at, modifiers);
    rig.release(at, modifiers);
}

/// Criteria 3 to 5: a plain click takes the nearest; each Alt-click at the
/// same point takes the next, wrapping after the last.
#[test]
fn alt_clicks_step_through_the_candidates_and_wrap() {
    let (mut rig, [near, mid, far]) = cycle_rig();
    let at = pt(50.0, 0.0);
    click(&mut rig, at, NONE);
    assert_eq!(rig.selection.ids(), &[near]);
    for expected in [mid, far, near, mid] {
        click(&mut rig, at, ALT);
        assert_eq!(rig.selection.ids(), &[expected]);
    }
}

/// Criterion 4: the step replaces the selection, whatever it was.
#[test]
fn an_alt_click_replaces_the_selection() {
    let (mut rig, [near, mid, far]) = cycle_rig();
    let at = pt(50.0, 0.0);
    click(&mut rig, at, NONE);
    rig.selection.set(&[near, far]);
    click(&mut rig, at, ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
}

/// Criterion 4 with jitter: an Alt-click a pixel off the first point, and a
/// release inside the dead zone, still continue the cycle.
#[test]
fn the_cycle_survives_pointer_jitter() {
    let (mut rig, [near, mid, _]) = cycle_rig();
    click(&mut rig, pt(50.0, 0.0), NONE);
    rig.press(pt(50.4, 0.2), ALT);
    rig.moved(pt(50.6, 0.3), ALT);
    rig.release(pt(50.6, 0.3), ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
    let _ = near;
}

/// Criterion 6: a click or Alt-click farther than the tolerance starts over
/// at the nearest candidate of the new point.
#[test]
fn a_far_click_resets_the_cycle() {
    let (mut rig, [near, mid, _]) = cycle_rig();
    click(&mut rig, pt(10.0, 0.0), NONE);
    click(&mut rig, pt(10.0, 0.0), ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
    // Alt-click far away (not a plain click first): starts at the nearest.
    click(&mut rig, pt(80.0, 0.0), ALT);
    assert_eq!(rig.selection.ids(), &[near]);
    click(&mut rig, pt(80.0, 0.0), ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
    // A plain click far away starts a new cycle there.
    click(&mut rig, pt(30.0, 0.0), NONE);
    assert_eq!(rig.selection.ids(), &[near]);
    click(&mut rig, pt(30.0, 0.0), ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
}

/// Criterion 7: one candidate is a no-op, never an error.
#[test]
fn alt_clicking_a_single_candidate_keeps_it_selected() {
    let (mut rig, [a, ..]) = Rig::new();
    let on_a = pt(0.0, 5.0);
    click(&mut rig, on_a, NONE);
    for _ in 0..3 {
        click(&mut rig, on_a, ALT);
        assert_eq!(rig.selection.ids(), &[a]);
    }
}

/// Alt-click on empty canvas changes nothing (nothing to step to).
#[test]
fn an_alt_click_on_nothing_leaves_the_selection() {
    let (mut rig, [a, ..]) = Rig::new();
    rig.select(&[a]);
    click(&mut rig, pt(100.0, 100.0), ALT);
    assert_eq!(rig.selection.ids(), &[a]);
}

/// Shift or Ctrl with a stationary Alt-click is still a cycle step; Escape,
/// a Shift-click or a miss drops the cycle.
#[test]
fn the_cycle_ends_on_escape_shift_click_and_a_miss() {
    for end in 0..3 {
        let (mut rig, [near, mid, _]) = cycle_rig();
        let at = pt(50.0, 0.0);
        click(&mut rig, at, NONE);
        match end {
            0 => rig.tool.escape(),
            1 => click(&mut rig, at, SHIFT),
            _ => click(&mut rig, pt(500.0, 500.0), NONE),
        }
        rig.selection.set(&[near]);
        click(&mut rig, at, SHIFT.with_alt(true));
        // With the cycle gone the Alt-click starts again at the nearest.
        assert_eq!(rig.selection.ids(), &[near], "end {end}");
        click(&mut rig, at, CTRL.with_alt(true));
        assert_eq!(rig.selection.ids(), &[mid], "end {end}: a second step");
    }
}

/// A marquee or lasso drag drops the cycle.
#[test]
fn a_gesture_drops_the_cycle() {
    let (mut rig, [near, mid, _]) = cycle_rig();
    let at = pt(50.0, 0.0);
    click(&mut rig, at, NONE);
    rig.drag(pt(60.0, 30.0), pt(70.0, 40.0), ALT, ALT);
    rig.selection.set(&[near]);
    click(&mut rig, at, ALT);
    assert_eq!(
        rig.selection.ids(),
        &[near],
        "a new cycle starts at the nearest"
    );
    click(&mut rig, at, ALT);
    assert_eq!(rig.selection.ids(), &[mid]);
}
