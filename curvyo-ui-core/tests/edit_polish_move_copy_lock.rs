//! `curvyo-ui-core`'s share of Part C of
//! `specs/edit-interaction-polish/specification.md`, PR 4: the axis lock, the
//! copy by Ctrl, Shift and Ctrl at the press, and the press classification the
//! plus badge shares with `pointer_down`. Session-level behaviour (readout,
//! badges, axes input, commit counts) is in
//! `curvyo-editor-wasm/tests/edit_polish_move_copy_lock.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Tolerance,
    Vec2,
};
use curvyo_ui_core::{
    AnchorIdMinter, Axis, Modifiers, ObjectSelection, PressTarget, SelectTool,
    TransformHandleTolerances, classify_press,
};

const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two screen pixels per millimetre: the 3 px dead zone is 1.5 mm and the
/// centre handle needs a 24 mm shorter side.
fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(2.0)
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::new(true, false);
const CTRL: Modifiers = Modifiers::new(false, true);
const BOTH: Modifiers = Modifiers::new(true, true);

struct Rig {
    document: Document,
    /// A 300 x 200 rectangle at the origin (an unfilled outline: it is hit on
    /// its edge), a 100 x 100 rectangle at (400, 0) and an open path.
    a: NodeId,
    b: NodeId,
    path: NodeId,
    selection: ObjectSelection,
    tool: SelectTool,
    minter: AnchorIdMinter,
}

impl Rig {
    fn new() -> Self {
        let document = Document::new(1);
        let a = document.create_rect(RectBounds {
            origin: pt(0.0, 0.0),
            width: Length::from_mm(300.0),
            height: Length::from_mm(200.0),
        });
        let b = document.create_rect(RectBounds {
            origin: pt(400.0, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(100.0),
        });
        let path = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 300.0)),
                NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 350.0)),
                NewAnchor::corner(AnchorId::new(1, 3), pt(200.0, 300.0)),
            ],
            false,
        );
        Self {
            document,
            a,
            b,
            path,
            selection: ObjectSelection::new(),
            tool: SelectTool::new(),
            minter: AnchorIdMinter::new(5),
        }
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    fn press(&mut self, at: Point, shift: bool) {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            at,
            SEGMENT_TOLERANCE,
            tolerances(),
            Modifiers::new(shift, false),
        );
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

    fn live(&self, at: Point, modifiers: Modifiers) -> Option<(Vec2, bool, Option<Axis>)> {
        self.tool
            .live_move(at, modifiers.shift, modifiers.ctrl)
            .map(|live| (live.offset, live.copy, live.axis))
    }

    fn origin_of(&self, id: NodeId) -> Point {
        match self.document.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => match p.shape {
                curvyo_document_core::Shape::Rect { bounds, .. } => bounds.origin,
                _ => panic!("a rectangle"),
            },
            ObjectSnapshot::Path(p) => p.anchors[0].point,
        }
    }

    /// A press on the top edge of rectangle A, selecting it.
    fn press_on_a(&mut self, shift: bool) -> Point {
        let at = pt(100.0, 0.0);
        self.press(at, shift);
        at
    }
}

/// Criterion 30, step by step: the axis is chosen again on every pointer
/// event with no latch, and releasing Shift gives the free offset again.
#[test]
fn the_axis_lock_is_rechosen_on_every_event_like_inkscape() {
    let mut rig = Rig::new();
    rig.press(pt(100.0, 0.0), false);
    // Shift down, pointer at (130, 10): axis x, offset (30, 0).
    rig.moved(pt(130.0, 10.0), SHIFT);
    assert_eq!(
        rig.live(pt(130.0, 10.0), SHIFT),
        Some((Vec2::new(30.0, 0.0), false, Some(Axis::X)))
    );
    // (131, 80): the axis has switched to y: offset (0, 80).
    rig.moved(pt(131.0, 80.0), SHIFT);
    assert_eq!(
        rig.live(pt(131.0, 80.0), SHIFT),
        Some((Vec2::new(0.0, 80.0), false, Some(Axis::Y)))
    );
    // Back to (131, 5): x again, offset (31, 0).
    rig.moved(pt(131.0, 5.0), SHIFT);
    assert_eq!(
        rig.live(pt(131.0, 5.0), SHIFT),
        Some((Vec2::new(31.0, 0.0), false, Some(Axis::X)))
    );
    // Shift released with the pointer at (131, 80): the free offset (31, 80).
    rig.moved(pt(131.0, 80.0), NONE);
    assert_eq!(
        rig.live(pt(131.0, 80.0), NONE),
        Some((Vec2::new(31.0, 80.0), false, None))
    );
    // Shift pressed again: |Dy| 80 beats |Dx| 31: offset (0, 80).
    rig.moved(pt(131.0, 80.0), SHIFT);
    assert_eq!(
        rig.live(pt(131.0, 80.0), SHIFT),
        Some((Vec2::new(0.0, 80.0), false, Some(Axis::Y)))
    );
    // The release commits exactly what the preview showed.
    rig.release(pt(131.0, 80.0), SHIFT);
    assert_eq!(rig.origin_of(rig.a), pt(0.0, 80.0));
}

/// Criterion 30: an exact tie keeps the previous axis (x if there was none).
#[test]
fn an_exact_tie_keeps_the_previous_axis() {
    let mut rig = Rig::new();
    rig.press(pt(100.0, 0.0), false);
    // A tie with no axis chosen yet: x.
    rig.moved(pt(110.0, 10.0), SHIFT);
    assert_eq!(rig.live(pt(110.0, 10.0), SHIFT).unwrap().2, Some(Axis::X));
    // y wins, then a tie keeps y.
    rig.moved(pt(105.0, 40.0), SHIFT);
    rig.moved(pt(140.0, 40.0), SHIFT);
    assert_eq!(
        rig.live(pt(140.0, 40.0), SHIFT),
        Some((Vec2::new(0.0, 40.0), false, Some(Axis::Y)))
    );
    // A tie also across the other diagonal.
    rig.moved(pt(60.0, 40.0), SHIFT);
    assert_eq!(rig.live(pt(60.0, 40.0), SHIFT).unwrap().2, Some(Axis::Y));
}

/// Criterion 28: the other offset is exactly zero and the objects follow the
/// pointer one to one along the axis.
#[test]
fn a_locked_move_has_exactly_zero_on_the_other_axis() {
    let mut rig = Rig::new();
    rig.press(pt(100.0, 0.0), false);
    rig.moved(pt(137.25, 9.5), SHIFT);
    let (offset, _, axis) = rig.live(pt(137.25, 9.5), SHIFT).unwrap();
    assert_eq!((offset.x, offset.y, axis), (37.25, 0.0, Some(Axis::X)));
    rig.release(pt(137.25, 9.5), SHIFT);
    assert_eq!(rig.origin_of(rig.a), pt(37.25, 0.0));
}

/// Criterion 29: Shift at the press locks from the first frame past the dead
/// zone, and the selection did not change at the press.
#[test]
fn shift_at_the_press_locks_from_the_first_frame_past_the_dead_zone() {
    let mut rig = Rig::new();
    rig.press_on_a(true);
    assert!(rig.selection.is_empty(), "a Shift press changes nothing");
    // 1 mm is inside the 1.5 mm dead zone: no move yet.
    rig.moved(pt(101.0, 0.5), SHIFT);
    assert_eq!(rig.live(pt(101.0, 0.5), SHIFT), None);
    // 4 mm: past it, locked on the first frame.
    rig.moved(pt(104.0, 1.0), SHIFT);
    assert_eq!(
        rig.live(pt(104.0, 1.0), SHIFT),
        Some((Vec2::new(4.0, 0.0), false, Some(Axis::X)))
    );
    // The unselected object joined the selection in that frame.
    assert_eq!(rig.selection.ids(), &[rig.a]);
}

/// Criterion 29: a Shift press released without leaving the dead zone is a
/// click that toggles, on an unselected and on a selected object.
#[test]
fn a_shift_click_toggles_on_the_release() {
    let mut rig = Rig::new();
    rig.selection.select_single(rig.b);
    rig.press(pt(100.0, 0.0), true);
    assert_eq!(rig.selection.ids(), &[rig.b]);
    rig.moved(pt(100.4, 0.2), SHIFT);
    rig.release(pt(100.4, 0.2), SHIFT);
    assert_eq!(rig.selection.ids(), &[rig.b, rig.a], "added");
    // A selected object is removed by the same click.
    rig.press(pt(100.0, 0.0), true);
    rig.release(pt(100.0, 0.0), SHIFT);
    assert_eq!(rig.selection.ids(), &[rig.b], "removed");
}

/// Criterion 29: a Shift press on an already selected object keeps the
/// selection for a drag and moves all of it.
#[test]
fn a_shift_drag_of_a_selected_object_keeps_the_whole_selection() {
    let mut rig = Rig::new();
    rig.selection.set(&[rig.a, rig.b]);
    rig.press(pt(100.0, 0.0), true);
    rig.moved(pt(150.0, 3.0), SHIFT);
    rig.release(pt(150.0, 3.0), SHIFT);
    assert_eq!(rig.selection.ids(), &[rig.a, rig.b]);
    assert_eq!(rig.origin_of(rig.a), pt(50.0, 0.0));
    assert_eq!(rig.origin_of(rig.b), pt(450.0, 0.0));
}

/// Criterion 38: a press on the drawn centre handle with Shift or Ctrl is a
/// move press and never toggles.
#[test]
fn a_press_on_the_centre_handle_with_a_modifier_never_toggles() {
    for modifiers in [SHIFT, CTRL, BOTH] {
        let mut rig = Rig::new();
        rig.selection.select_single(rig.a);
        let centre = pt(150.0, 100.0);
        rig.press(centre, modifiers.shift);
        assert!(rig.tool.move_in_flight(), "{modifiers:?}");
        rig.release(centre, modifiers);
        assert_eq!(rig.selection.ids(), &[rig.a], "{modifiers:?}: no toggle");
        assert_eq!(rig.origin_of(rig.a), pt(0.0, 0.0));
    }
}

/// Criterion 38: the centre handle's Shift or Ctrl are move modifiers: Shift
/// locks the axis, Ctrl makes a copy.
#[test]
fn the_modifiers_of_a_centre_handle_press_are_move_modifiers() {
    let mut rig = Rig::new();
    rig.selection.select_single(rig.a);
    let centre = pt(150.0, 100.0);
    rig.press(centre, true);
    rig.moved(centre.translated(Vec2::new(20.0, 5.0)), SHIFT);
    assert_eq!(
        rig.live(centre.translated(Vec2::new(20.0, 5.0)), SHIFT),
        Some((Vec2::new(20.0, 0.0), false, Some(Axis::X)))
    );
    rig.tool.escape();

    rig.press(centre, false);
    rig.moved(centre.translated(Vec2::new(20.0, 5.0)), CTRL);
    assert_eq!(
        rig.live(centre.translated(Vec2::new(20.0, 5.0)), CTRL),
        Some((Vec2::new(20.0, 5.0), true, None))
    );
}

/// Criteria 33, 35: Ctrl at the release copies, the original stays, the
/// selection is the copy, one commit.
#[test]
fn ctrl_at_the_release_makes_a_copy_and_selects_it() {
    let mut rig = Rig::new();
    rig.press_on_a(false);
    let objects_before = rig.document.object_ids();
    let original = rig.document.object(rig.a).unwrap();
    let changes_before = rig.document.export_loro_snapshot().unwrap();
    rig.moved(pt(150.0, 30.0), CTRL);
    rig.release(pt(150.0, 30.0), CTRL);

    let objects = rig.document.object_ids();
    assert_eq!(objects.len(), objects_before.len() + 1);
    assert_eq!(rig.document.object(rig.a).unwrap(), original, "untouched");
    let copy = rig.selection.ids()[0];
    assert_eq!(rig.selection.ids().len(), 1);
    assert!(!objects_before.contains(&copy));
    assert_eq!(rig.origin_of(copy), pt(50.0, 30.0));
    // Directly above its original.
    let position = objects.iter().position(|id| *id == rig.a).unwrap();
    assert_eq!(objects[position + 1], copy);
    assert_ne!(rig.document.export_loro_snapshot().unwrap(), changes_before);
}

/// Criterion 33: Ctrl is read at the release, not at the press.
#[test]
fn ctrl_pressed_and_released_mid_drag_follows_the_state_of_the_release() {
    let mut rig = Rig::new();
    rig.press_on_a(false);
    rig.moved(pt(150.0, 30.0), CTRL);
    assert!(rig.live(pt(150.0, 30.0), CTRL).unwrap().1);
    // Released again before the button: a plain move.
    rig.moved(pt(150.0, 30.0), NONE);
    assert!(!rig.live(pt(150.0, 30.0), NONE).unwrap().1);
    let count = rig.document.object_ids().len();
    rig.release(pt(150.0, 30.0), NONE);
    assert_eq!(rig.document.object_ids().len(), count, "no copy");
    assert_eq!(rig.origin_of(rig.a), pt(50.0, 30.0), "a move");

    // Held at the press and released before the button: also a move.
    let mut rig = Rig::new();
    rig.press_on_a(false);
    rig.moved(pt(150.0, 30.0), CTRL);
    rig.release(pt(150.0, 30.0), NONE);
    assert_eq!(rig.origin_of(rig.a), pt(50.0, 30.0));
    assert_eq!(rig.document.object_ids().len(), 3);

    // Not held at the press, pressed before the button comes up: a copy.
    let mut rig = Rig::new();
    rig.press_on_a(false);
    rig.moved(pt(150.0, 30.0), NONE);
    rig.release(pt(150.0, 30.0), CTRL);
    assert_eq!(rig.origin_of(rig.a), pt(0.0, 0.0));
    assert_eq!(rig.document.object_ids().len(), 4);
}

/// Criterion 32: Shift and Ctrl together copy along the locked axis.
#[test]
fn shift_and_ctrl_together_copy_along_the_axis() {
    let mut rig = Rig::new();
    rig.press_on_a(false);
    rig.moved(pt(160.0, 12.0), BOTH);
    assert_eq!(
        rig.live(pt(160.0, 12.0), BOTH),
        Some((Vec2::new(60.0, 0.0), true, Some(Axis::X)))
    );
    rig.release(pt(160.0, 12.0), BOTH);
    let copy = rig.selection.ids()[0];
    assert_ne!(copy, rig.a);
    assert_eq!(rig.origin_of(copy), pt(60.0, 0.0));
    assert_eq!(rig.origin_of(rig.a), pt(0.0, 0.0));
}

/// Criteria 31, 36: a drag back to the start writes nothing, with and without
/// Ctrl; a press and release inside the dead zone write nothing.
#[test]
fn a_zero_offset_writes_nothing_with_and_without_ctrl() {
    for modifiers in [NONE, CTRL, SHIFT, BOTH] {
        let mut rig = Rig::new();
        rig.press_on_a(false);
        let before = rig.document.export_loro_snapshot().unwrap();
        rig.moved(pt(160.0, 12.0), modifiers);
        assert!(rig.live(pt(160.0, 12.0), modifiers).is_some());
        // Back exactly to the start.
        rig.moved(pt(100.0, 0.0), modifiers);
        let live = rig.tool.live_edit(
            &rig.objects(),
            &rig.selection,
            pt(100.0, 0.0),
            modifiers.shift,
            modifiers.ctrl,
        );
        assert_eq!(live, None, "{modifiers:?}: nothing to preview");
        rig.release(pt(100.0, 0.0), modifiers);
        assert_eq!(
            rig.document.export_loro_snapshot().unwrap(),
            before,
            "{modifiers:?}"
        );
    }
    // A press and release inside the dead zone.
    let mut rig = Rig::new();
    rig.press_on_a(false);
    let before = rig.document.export_loro_snapshot().unwrap();
    rig.moved(pt(100.5, 0.5), CTRL);
    rig.release(pt(100.5, 0.5), CTRL);
    assert_eq!(rig.document.export_loro_snapshot().unwrap(), before);
}

/// Criteria 34, 35: a selection of several kinds is copied the same way; a
/// path's copy has fresh anchor ids; a primitive's copy is a primitive; the
/// relative z-order is kept (A, B becomes A, A', B, B').
#[test]
fn a_selection_of_several_kinds_is_copied_as_one_commit() {
    let mut rig = Rig::new();
    rig.selection.set(&[rig.path, rig.a]);
    rig.press(pt(100.0, 0.0), false);
    rig.moved(pt(130.0, 40.0), CTRL);
    rig.release(pt(130.0, 40.0), CTRL);

    let copies = rig.selection.ids().to_vec();
    assert_eq!(copies.len(), 2);
    let ids = rig.document.object_ids();
    let at = |id| ids.iter().position(|x| *x == id).unwrap();
    // `ids` was [a, b, path]: the copies sit directly above their originals.
    assert_eq!(at(copies[1]), at(rig.a) + 1, "A' above A");
    assert_eq!(at(copies[0]), at(rig.path) + 1, "path' above the path");
    let ObjectSnapshot::Path(original) = rig.document.object(rig.path).unwrap() else {
        panic!("a path")
    };
    let ObjectSnapshot::Path(copy) = rig.document.object(copies[0]).unwrap() else {
        panic!("a path")
    };
    for (o, c) in original.anchors.iter().zip(&copy.anchors) {
        assert_ne!(o.id, c.id, "fresh anchor ids");
        assert_eq!(c.point, o.point.translated(Vec2::new(30.0, 40.0)));
    }
    assert!(matches!(
        rig.document.object(copies[1]).unwrap(),
        ObjectSnapshot::Primitive(_)
    ));
}

/// Criterion 33: the blue outline is the translated copies, marked as a copy
/// so the box and handles stay on the originals; a move is not.
#[test]
fn the_live_edit_says_whether_it_is_a_copy() {
    let mut rig = Rig::new();
    rig.press_on_a(false);
    rig.moved(pt(150.0, 30.0), NONE);
    let objects = rig.objects();
    let moved = rig
        .tool
        .live_edit(&objects, &rig.selection, pt(150.0, 30.0), false, false)
        .unwrap();
    assert!(!moved.copy);
    let copied = rig
        .tool
        .live_edit(&objects, &rig.selection, pt(150.0, 30.0), false, true)
        .unwrap();
    assert!(copied.copy);
    assert_eq!(copied.objects, moved.objects);
    assert_eq!(copied.objects.len(), 1);
    assert_eq!(copied.objects[0].id(), rig.a, "carries the original's id");
}

/// Escape cancels the drag and writes nothing, including no copy.
#[test]
fn escape_cancels_a_copy_drag_and_writes_nothing() {
    let mut rig = Rig::new();
    rig.press_on_a(false);
    let before = rig.document.export_loro_snapshot().unwrap();
    rig.moved(pt(150.0, 30.0), CTRL);
    rig.tool.escape();
    assert!(!rig.tool.move_in_flight());
    assert_eq!(rig.live(pt(150.0, 30.0), CTRL), None);
    rig.release(pt(150.0, 30.0), CTRL);
    assert_eq!(rig.document.export_loro_snapshot().unwrap(), before);
}

/// Criterion 33 (the plus badge's rule): for a grid of points over a mixed
/// scene, the press classification says "a move begins" exactly when a real
/// `pointer_down` begins a move drag, with and without Shift.
#[test]
fn the_press_classification_agrees_with_pointer_down_everywhere() {
    for selected in [vec![], vec![0_usize], vec![1], vec![0, 1], vec![2]] {
        for shift in [false, true] {
            let mut rig = Rig::new();
            let ids = [rig.a, rig.b, rig.path];
            let selection: Vec<NodeId> = selected.iter().map(|&i| ids[i]).collect();
            rig.selection.set(&selection);
            let objects = rig.objects();
            let mut begins_a_move = 0;
            for x in (-20..=520).step_by(13) {
                for y in (-20..=380).step_by(13) {
                    let at = pt(f64::from(x), f64::from(y));
                    // A press writes nothing to the document: one document
                    // serves every probe, with a fresh tool and selection.
                    rig.tool = SelectTool::new();
                    rig.selection.set(&selection);
                    let target = classify_press(
                        &objects,
                        &rig.selection,
                        at,
                        SEGMENT_TOLERANCE,
                        tolerances(),
                        Modifiers::new(shift, false),
                    );
                    rig.tool.pointer_down(
                        &objects,
                        &mut rig.selection,
                        at,
                        SEGMENT_TOLERANCE,
                        tolerances(),
                        Modifiers::new(shift, false),
                    );
                    assert_eq!(
                        target.begins_move(),
                        rig.tool.move_in_flight(),
                        "{selected:?} shift {shift} at ({x}, {y}): {target:?}"
                    );
                    if matches!(target, PressTarget::Handle(_)) {
                        assert!(!rig.tool.move_in_flight());
                    }
                    begins_a_move += usize::from(target.begins_move());
                }
            }
            assert!(begins_a_move > 0, "{selected:?}: the grid reaches objects");
        }
    }
}

/// Criterion 37: on empty canvas a press starts no move, so no copy and no
/// badge; Ctrl there changes nothing about the selection either.
#[test]
fn a_press_on_empty_canvas_starts_no_move() {
    let mut rig = Rig::new();
    rig.selection.select_single(rig.b);
    rig.press(pt(700.0, 700.0), false);
    assert!(!rig.tool.move_in_flight());
    // The click clears at the release (`advanced-selection` criterion 8).
    rig.release(pt(700.0, 700.0), NONE);
    assert!(rig.selection.is_empty(), "a plain click clears");
    rig.selection.select_single(rig.b);
    rig.press(pt(700.0, 700.0), true);
    assert!(!rig.tool.move_in_flight());
    rig.release(pt(700.0, 700.0), Modifiers::new(true, false));
    assert_eq!(rig.selection.ids(), &[rig.b], "Shift keeps the selection");
}

// ---------------------------------------------------------------------
// The Copy check of the typed move (criterion 23)
// ---------------------------------------------------------------------

use curvyo_ui_core::{
    EntryKey, EntryOutcome, MoveEntryMode, SelectDoubleClickOutcome, parse_entry_number,
};

impl Rig {
    /// A double-click on the centre of rectangle A with the given modifiers
    /// at the second press, leaving the move chip open.
    fn open_move_chip(&mut self, second_press: (bool, bool)) {
        self.selection.select_single(self.a);
        let centre = pt(150.0, 100.0);
        self.press(centre, false);
        self.release(centre, NONE);
        let objects = self.objects();
        let outcome = self.tool.double_click(
            &objects,
            &self.selection,
            centre,
            SEGMENT_TOLERANCE,
            tolerances(),
            second_press,
        );
        assert_eq!(outcome, SelectDoubleClickOutcome::EntryOpened);
    }

    fn commit_chip(&mut self, texts: [&str; 2], absolute: bool, copy: bool) -> EntryOutcome {
        self.tool.commit_move_entry(
            &self.document,
            &mut self.selection,
            &mut self.minter,
            texts,
            MoveEntryMode { absolute, copy },
        )
    }
}

/// Criterion 23: Ctrl at the second press opens the chip with Copy on; Shift
/// has no effect; the key M cannot carry a Ctrl and opens it off.
#[test]
fn ctrl_at_the_second_press_presets_the_copy_check() {
    let mut rig = Rig::new();
    rig.open_move_chip((false, true));
    assert!(rig.tool.move_entry().unwrap().copy_preset());
    let mut rig = Rig::new();
    rig.open_move_chip((true, false));
    assert!(!rig.tool.move_entry().unwrap().copy_preset());
    let mut rig = Rig::new();
    rig.open_move_chip((false, false));
    assert!(!rig.tool.move_entry().unwrap().copy_preset());
    let mut rig = Rig::new();
    rig.selection.select_single(rig.a);
    let objects = rig.objects();
    rig.tool
        .open_entry_for_key(&objects, &rig.selection, EntryKey::Move)
        .unwrap();
    assert!(!rig.tool.move_entry().unwrap().copy_preset());
}

/// Criterion 23, relative: one copy displaced by (X, Y), the original
/// untouched, the selection is the copy, the chip closes.
#[test]
fn a_typed_relative_copy_displaces_the_copy_and_selects_it() {
    let mut rig = Rig::new();
    rig.open_move_chip((false, false));
    let before = rig.document.object(rig.a).unwrap();
    let count = rig.document.object_ids().len();
    assert_eq!(
        rig.commit_chip(["5", "-3"], false, true),
        EntryOutcome::Committed
    );
    assert_eq!(rig.document.object(rig.a).unwrap(), before);
    assert_eq!(rig.document.object_ids().len(), count + 1);
    let copy = rig.selection.ids()[0];
    assert_ne!(copy, rig.a);
    assert_eq!(rig.selection.ids().len(), 1);
    assert_eq!(rig.origin_of(copy), pt(5.0, -3.0));
    assert!(rig.tool.move_entry().is_none(), "the chip closed");
}

/// Criterion 23, absolute: the copy's bounds' top-left lies at (X, Y).
#[test]
fn a_typed_absolute_copy_puts_the_copys_top_left_there() {
    let mut rig = Rig::new();
    rig.open_move_chip((false, false));
    assert_eq!(
        rig.commit_chip(["100", "50"], true, true),
        EntryOutcome::Committed
    );
    let copy = rig.selection.ids()[0];
    assert_eq!(rig.origin_of(copy), pt(100.0, 50.0));
    assert_eq!(rig.origin_of(rig.a), pt(0.0, 0.0));
}

/// Criteria 23, 25: a copy at no offset, or an invalid field, writes nothing;
/// an invalid chip stays open and the selection stays.
#[test]
fn a_typed_copy_with_no_offset_or_a_bad_field_writes_nothing() {
    let mut rig = Rig::new();
    rig.open_move_chip((false, false));
    let before = rig.document.export_loro_snapshot().unwrap();
    assert_eq!(
        rig.commit_chip(["abc", "0"], false, true),
        EntryOutcome::Invalid {
            field: 0,
            reason: curvyo_ui_core::InvalidReason::NotANumber
        }
    );
    assert!(rig.tool.move_entry().is_some(), "stays open");
    assert_eq!(rig.selection.ids(), &[rig.a]);
    assert_eq!(
        rig.commit_chip(["0", "0"], false, true),
        EntryOutcome::Unchanged
    );
    assert_eq!(rig.document.export_loro_snapshot().unwrap(), before);
    assert_eq!(rig.selection.ids(), &[rig.a]);
}

/// Flag 6: the minus of the move readout (U+2212) parses.
#[test]
fn the_real_minus_sign_of_the_readout_parses() {
    assert_eq!(parse_entry_number("\u{2212}3.5", false), Some(-3.5));
    assert_eq!(parse_entry_number("\u{2212} 12,5", false), Some(-12.5));
    assert_eq!(parse_entry_number("-3.5", false), Some(-3.5));
    assert_eq!(parse_entry_number("\u{2212}", false), None);
    assert_eq!(parse_entry_number("\u{2212}\u{2212}1", false), None);
    assert_eq!(parse_entry_number("\u{2212}15\u{b0}", true), Some(-15.0));
}

/// Criterion 29: a release outside the dead zone with no `pointer_moved`
/// before it (a flick, a touch, a pen) still joins the Shift-pressed
/// unselected object, for a move and for a copy.
#[test]
fn a_release_with_no_move_event_still_joins_the_pressed_object() {
    for modifiers in [SHIFT, BOTH] {
        let mut rig = Rig::new();
        rig.selection.select_single(rig.b);
        rig.press(pt(100.0, 0.0), true);
        rig.release(pt(130.0, 4.0), modifiers);
        if modifiers.ctrl {
            let copies = rig.selection.ids().to_vec();
            assert_eq!(copies.len(), 2, "both objects were copied");
            assert_eq!(rig.origin_of(rig.a), pt(0.0, 0.0));
            assert_eq!(rig.origin_of(rig.b), pt(400.0, 0.0));
        } else {
            assert_eq!(rig.selection.ids(), &[rig.b, rig.a]);
            assert_eq!(rig.origin_of(rig.a), pt(30.0, 0.0), "the pressed one moved");
            assert_eq!(
                rig.origin_of(rig.b),
                pt(430.0, 0.0),
                "the old selection too"
            );
        }
    }
}

/// UX review of PR 4: in a move drag Shift means "lock", so the four
/// Shift-revealed side rotate handles are not shown, whatever the Shift state
/// at the press; idle they still follow Shift.
#[test]
fn no_side_rotate_handles_show_during_a_move_drag() {
    let mut rig = Rig::new();
    rig.selection.select_single(rig.a);
    assert!(rig.tool.side_rotate_revealed(true), "idle: Shift reveals");
    assert!(!rig.tool.side_rotate_revealed(false));
    rig.press(pt(100.0, 0.0), true);
    rig.moved(pt(140.0, 3.0), SHIFT);
    assert!(rig.tool.move_in_flight());
    assert!(!rig.tool.side_rotate_revealed(true), "Shift at the press");
    rig.release(pt(140.0, 3.0), SHIFT);
    assert!(rig.tool.side_rotate_revealed(true), "idle again");
}
