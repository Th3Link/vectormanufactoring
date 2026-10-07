//! `Session`-level tests of Part C of `specs/edit-interaction-polish/
//! specification.md` (PR 4): the axis lock, the copy, their readout, axes and
//! badges, and the Copy check of the typed move. Driven through `Session`'s
//! public API; the rules themselves are tested in `vecmanf-ui-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use vecmanf_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Shape, pack,
    unpack,
};
use vecmanf_editor_wasm::{EscapeStep, MoveIndicators, Session, Tool};
use vecmanf_ui_core::{Axis, EntryOutcome, MoveEntryMode};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A 100 x 60 rectangle at (10, 20) (hit on its outline, its centre handle at
/// (60, 50)), a second one at (200, 20) and a path below them.
fn document() -> Document {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(60.0),
    });
    let _ = document.create_rect(RectBounds {
        origin: pt(200.0, 20.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 150.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(60.0, 190.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(110.0, 150.0)),
        ],
        false,
    );
    document
}

fn session() -> Session {
    let mut session = Session::open(2, &pack(&document(), "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

fn reread(session: &Session) -> Document {
    unpack(99, &session.pack("0.1.0").unwrap()).unwrap()
}

fn objects(session: &Session) -> Vec<ObjectSnapshot> {
    let document = reread(session);
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn ids(session: &Session) -> Vec<NodeId> {
    reread(session).object_ids()
}

fn change_count(session: &Session) -> usize {
    let doc = loro::LoroDoc::new();
    doc.import(&reread(session).export_loro_snapshot().unwrap())
        .unwrap();
    doc.len_changes()
}

fn origin_of(object: &ObjectSnapshot) -> Point {
    match object {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => bounds.origin,
            _ => panic!("a rectangle"),
        },
        ObjectSnapshot::Path(path) => path.anchors[0].point,
    }
}

/// A modifier state as the host reports it: the cached state and a hover at
/// the pointer, as `applyModifiers` does.
fn hold(session: &mut Session, at: Point, shift: bool, ctrl: bool) {
    session.modifiers_changed(shift, ctrl);
    session.pointer_hover(at, shift, ctrl);
}

/// The press on the top edge of the first rectangle, with the modifiers of
/// the press.
fn press_on_edge(session: &mut Session, shift: bool, ctrl: bool) -> Point {
    let at = pt(40.0, 20.0);
    hold(session, at, shift, ctrl);
    session.pointer_down(at, shift);
    at
}

fn colour_count(session: &Session, a: u8) -> usize {
    session
        .draw_list()
        .triangles
        .iter()
        .filter(|v| (v.color.r, v.color.g, v.color.b, v.color.a) == (0x2F, 0x6F, 0xEE, a))
        .count()
}

const AXIS_GUIDE_ALPHA: u8 = 128;
const AXIS_GUIDE_IDLE_ALPHA: u8 = 51;

/// Criterion 27: the readout shows the offset a release would commit, after
/// any lock, with a real minus sign and " Copy" in copy mode; none before the
/// dead zone is left.
#[test]
fn the_move_readout_shows_the_committed_offset() {
    let mut session = session();
    let start = press_on_edge(&mut session, false, false);
    assert!(session.live_readout().is_none(), "before the dead zone");
    hold(
        &mut session,
        pt(start.x + 12.5, start.y - 3.0),
        false,
        false,
    );
    assert_eq!(
        session.live_readout().unwrap().text,
        "Δ 12.5, \u{2212}3.0 mm"
    );
    hold(&mut session, pt(start.x + 30.0, start.y + 4.0), true, false);
    assert_eq!(session.live_readout().unwrap().text, "Δ 30.0, 0.0 mm");
    hold(&mut session, pt(start.x + 30.0, start.y + 4.0), true, true);
    assert_eq!(session.live_readout().unwrap().text, "Δ 30.0, 0.0 mm Copy");
    // The readout is anchored at the pointer.
    assert_eq!(
        session.live_readout().unwrap().anchor,
        pt(start.x + 30.0, start.y + 4.0)
    );
    session.pointer_up(pt(start.x + 30.0, start.y + 4.0), true, true);
    assert!(session.live_readout().is_none(), "gone after the release");
}

/// Criterion 27: the origin axes appear only while the lock is engaged past
/// the dead zone, and are gone in the frame Shift is released.
#[test]
fn the_origin_axes_exist_only_while_the_lock_is_engaged() {
    let mut session = session();
    let start = press_on_edge(&mut session, true, false);
    let axes = |s: &Session| {
        (
            colour_count(s, AXIS_GUIDE_ALPHA),
            colour_count(s, AXIS_GUIDE_IDLE_ALPHA),
        )
    };
    let before = axes(&session);
    // Inside the dead zone: nothing.
    hold(&mut session, pt(start.x + 0.2, start.y), true, false);
    assert_eq!(axes(&session), before);
    // Past it: a strong and a faint line.
    hold(&mut session, pt(start.x + 20.0, start.y + 4.0), true, false);
    let (strong, faint) = axes(&session);
    assert!(strong >= before.0 + 6 && faint >= before.1 + 6, "two quads");
    // Shift released: gone in the same frame.
    hold(
        &mut session,
        pt(start.x + 20.0, start.y + 4.0),
        false,
        false,
    );
    assert_eq!(axes(&session), before);
    // Shift again, and the axis flips with the pointer: the same two lines,
    // the strong one swapped.
    hold(&mut session, pt(start.x + 4.0, start.y + 20.0), true, false);
    let (strong2, faint2) = axes(&session);
    assert!(strong2 >= before.0 + 6 && faint2 >= before.1 + 6);
    session.pointer_up(pt(start.x + 4.0, start.y + 20.0), true, false);
    assert_eq!(axes(&session), before, "gone after the release");
}

/// Criterion 33: the selection box and the handles stay on the originals
/// while a copy is dragged, and follow the blue outline in a move.
#[test]
fn the_box_stays_on_the_original_in_a_copy_and_travels_in_a_move() {
    // The decoration input is checked in `select_view`'s own unit tests; here
    // the drawn result: the centre handle (a white square with an accent
    // outline) sits at the original in copy mode and at the target in a move.
    let mut session = session();
    let start = press_on_edge(&mut session, false, false);
    let target = pt(start.x + 70.0, start.y + 10.0);
    let near = |list: &vecmanf_render_core::DrawList, at: Point| {
        list.triangles
            .iter()
            .any(|v| (v.position.x - at.x).abs() < 9.0 && (v.position.y - at.y).abs() < 9.0)
    };
    // In a move the centre handle travels (60, 50) -> (130, 60).
    hold(&mut session, target, false, false);
    let moved = session.draw_list();
    assert!(near(&moved, pt(130.0, 60.0)), "handle at the target");
    // In a copy it stays at (60, 50).
    hold(&mut session, target, false, true);
    let copy = session.draw_list();
    assert!(near(&copy, pt(60.0, 50.0)), "handle on the original");
    assert!(
        !copy
            .triangles
            .iter()
            .any(|v| (v.position.x - 130.0).abs() < 4.0
                && (v.position.y - 60.0).abs() < 4.0
                && v.color.a == 255
                && v.color.r == 255),
        "no white handle square at the copy"
    );
}

/// Criteria 33, 34, 35: a copy drag is one commit, the original is untouched,
/// the selection is the copy (a delete removes the copy and leaves the
/// original), and there is no copy for a zero offset.
#[test]
fn a_copy_drag_is_one_commit_and_selects_the_copy() {
    let mut session = session();
    let before_objects = objects(&session);
    let before_ids = ids(&session);
    let commits = change_count(&session);
    let start = press_on_edge(&mut session, false, true);
    let to = pt(start.x + 50.0, start.y + 30.0);
    hold(&mut session, to, false, true);
    session.pointer_up(to, false, true);

    let after = objects(&session);
    assert_eq!(after.len(), before_objects.len() + 1);
    assert_eq!(change_count(&session), commits + 1, "one commit");
    assert_eq!(after[0], before_objects[0], "the original is untouched");
    assert_eq!(origin_of(&after[1]), pt(60.0, 50.0), "the copy above it");
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(session.tool(), Tool::Select);
    // The selected object is the copy: deleting the selection removes it.
    session.delete_selected();
    assert_eq!(ids(&session), before_ids);

    // A drag back to the start writes nothing.
    let mut session = self::session();
    let commits = change_count(&session);
    let start = press_on_edge(&mut session, false, true);
    hold(&mut session, pt(start.x + 20.0, start.y), false, true);
    hold(&mut session, start, false, true);
    session.pointer_up(start, false, true);
    assert_eq!(change_count(&session), commits);
}

/// Criterion 26: Escape cancels the drag and writes nothing, including no
/// copy, and a Ctrl that is still held keeps predicting.
#[test]
fn escape_cancels_a_copy_drag_and_ctrl_keeps_showing_the_badge() {
    let mut session = session();
    let commits = change_count(&session);
    let start = press_on_edge(&mut session, false, true);
    hold(&mut session, pt(start.x + 20.0, start.y + 9.0), false, true);
    assert_eq!(
        session.move_indicators(),
        MoveIndicators {
            copy_badge: true,
            lock: None
        }
    );
    assert_eq!(session.escape(), EscapeStep::CancelledDrag);
    session.pointer_up(pt(start.x + 20.0, start.y + 9.0), false, true);
    assert_eq!(change_count(&session), commits, "nothing written");
    // The pointer is now off the outline, Ctrl still down: no badge...
    hold(&mut session, pt(300.0, 300.0), false, true);
    assert!(!session.move_indicators().copy_badge);
    // ... back over the outline with Ctrl still down: the badge again.
    hold(&mut session, start, false, true);
    assert!(session.move_indicators().copy_badge);
}

/// Criteria 26, 33, 37: the plus badge shows wherever a press with Ctrl would
/// start a move, follows the key with the pointer at rest, and never shows on
/// empty canvas or over a resize handle.
#[test]
fn the_plus_badge_follows_ctrl_and_the_press_classification() {
    let mut session = session();
    let on_edge = pt(40.0, 20.0);
    // No Ctrl: no badge, wherever the pointer is.
    hold(&mut session, on_edge, false, false);
    assert!(!session.move_indicators().copy_badge);
    // Ctrl pressed with the pointer at rest: the badge appears at once.
    session.modifiers_changed(false, true);
    assert!(session.move_indicators().copy_badge);
    // Released: it is gone in the same frame, with no pointer motion.
    session.modifiers_changed(false, false);
    assert!(!session.move_indicators().copy_badge);
    // Empty canvas: Ctrl belongs to the marquee.
    hold(&mut session, pt(500.0, 500.0), false, true);
    assert!(!session.move_indicators().copy_badge);
    // An unselected object: yes.
    hold(&mut session, pt(220.0, 20.0), false, true);
    assert!(session.move_indicators().copy_badge);

    // With the first rectangle selected: the centre handle (a move), but not
    // a corner resize handle.
    let mut session = self::session();
    hold(&mut session, on_edge, false, false);
    session.pointer_down(on_edge, false);
    session.pointer_up(on_edge, false, false);
    hold(&mut session, pt(60.0, 50.0), false, true);
    assert!(session.move_indicators().copy_badge, "the centre handle");
    hold(&mut session, pt(110.0, 80.0), false, true);
    assert!(!session.move_indicators().copy_badge, "a resize handle");
}

/// Criterion 33: the badge shows exactly where a real press with Ctrl held
/// begins a move, over a grid of points and several selections.
#[test]
fn the_plus_badge_agrees_with_a_real_press_over_a_grid() {
    for pre_selected in [None, Some(pt(40.0, 20.0)), Some(pt(220.0, 20.0))] {
        for shift in [false, true] {
            let mut checked = 0;
            for x in (0..260).step_by(11) {
                for y in (0..210).step_by(11) {
                    let at = pt(f64::from(x), f64::from(y));
                    let mut session = session();
                    if let Some(select) = pre_selected {
                        hold(&mut session, select, false, false);
                        session.pointer_down(select, false);
                        session.pointer_up(select, false, false);
                    }
                    hold(&mut session, at, shift, true);
                    let badge = session.move_indicators().copy_badge;
                    session.pointer_down(at, shift);
                    hold(&mut session, at, shift, true);
                    let moving = session.move_indicators().copy_badge;
                    // After a real press with Ctrl held, the badge reads "a
                    // copy drag runs" exactly when a move began.
                    assert_eq!(
                        badge, moving,
                        "{pre_selected:?} shift {shift} at ({x}, {y})"
                    );
                    checked += usize::from(badge);
                    session.pointer_up(at, shift, true);
                }
            }
            assert!(checked > 0, "{pre_selected:?}: some points start a move");
        }
    }
}

/// Criterion 27: the lock badge shows only while a move past the dead zone is
/// locked, with its axis, and never before the press.
#[test]
fn the_lock_badge_shows_only_during_a_locked_move() {
    let mut session = session();
    let at = pt(40.0, 20.0);
    hold(&mut session, at, true, false);
    assert_eq!(session.move_indicators().lock, None, "not before the press");
    session.pointer_down(at, true);
    hold(&mut session, pt(40.3, 20.1), true, false);
    assert_eq!(session.move_indicators().lock, None, "inside the dead zone");
    hold(&mut session, pt(70.0, 24.0), true, false);
    assert_eq!(session.move_indicators().lock, Some(Axis::X));
    hold(&mut session, pt(42.0, 60.0), true, false);
    assert_eq!(session.move_indicators().lock, Some(Axis::Y));
    hold(&mut session, pt(42.0, 60.0), false, false);
    assert_eq!(session.move_indicators().lock, None, "Shift released");
}

/// Criterion 29: a Shift press changes nothing, an unselected object joins the
/// selection when the drag leaves the dead zone, and a release inside it
/// toggles.
#[test]
fn shift_at_the_press_toggles_on_release_and_joins_on_a_drag() {
    // A click toggles.
    let mut session = session();
    let a = pt(40.0, 20.0);
    hold(&mut session, a, false, false);
    session.pointer_down(a, false);
    session.pointer_up(a, false, false);
    let b = pt(220.0, 20.0);
    hold(&mut session, b, true, false);
    session.pointer_down(b, true);
    assert_eq!(session.selected_object_count(), 1, "nothing at the press");
    session.pointer_up(b, true, false);
    assert_eq!(session.selected_object_count(), 2, "added on the release");

    // A drag joins it and moves both along one axis.
    let mut session = self::session();
    hold(&mut session, a, false, false);
    session.pointer_down(a, false);
    session.pointer_up(a, false, false);
    hold(&mut session, b, true, false);
    session.pointer_down(b, true);
    hold(&mut session, pt(b.x + 30.0, b.y + 5.0), true, false);
    assert_eq!(
        session.selected_object_count(),
        2,
        "joined past the dead zone"
    );
    session.pointer_up(pt(b.x + 30.0, b.y + 5.0), true, false);
    let after = objects(&session);
    assert_eq!(origin_of(&after[0]), pt(40.0, 20.0));
    assert_eq!(origin_of(&after[1]), pt(230.0, 20.0));
}

/// Criterion 23: Ctrl at the second press of the double-click opens the move
/// chip with the Copy check on; the typed copy leaves the original, creates
/// one copy displaced as typed and selects it.
#[test]
fn the_typed_move_copies_with_the_copy_check() {
    let mut session = session();
    let centre = pt(60.0, 50.0);
    // Select the rectangle, then double-click its centre with Ctrl at the
    // second press.
    let edge = pt(40.0, 20.0);
    hold(&mut session, edge, false, false);
    session.pointer_down(edge, false);
    session.pointer_up(edge, false, false);
    hold(&mut session, centre, false, false);
    session.pointer_down(centre, false);
    session.pointer_up(centre, false, false);
    session.pointer_down(centre, false);
    session.pointer_up(centre, false, true);
    session.double_click(centre, false, true);
    let chip = session.move_entry().expect("the move chip");
    assert!(chip.copy_preset, "Ctrl at the second press");

    let before = objects(&session);
    let commits = change_count(&session);
    assert_eq!(
        session.commit_move_entry(
            "5",
            "-3",
            MoveEntryMode {
                absolute: false,
                copy: true
            }
        ),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&session), commits + 1);
    let after = objects(&session);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after[0], before[0]);
    assert_eq!(origin_of(&after[1]), pt(15.0, 17.0));
    assert_eq!(session.selected_object_count(), 1);
    assert!(session.move_entry().is_none());

    // The key M cannot carry a Ctrl: the check opens off.
    let mut session = self::session();
    hold(&mut session, edge, false, false);
    session.pointer_down(edge, false);
    session.pointer_up(edge, false, false);
    session.key_down(vecmanf_editor_wasm::KeyInput {
        key: "m",
        ..vecmanf_editor_wasm::KeyInput::default()
    });
    assert!(!session.move_entry().expect("the chip").copy_preset);
}

/// A plain drag (no modifier) is still one `translate_objects` commit: the
/// same registers as before this change.
#[test]
fn a_plain_drag_is_still_one_move_commit() {
    let mut session = session();
    let commits = change_count(&session);
    let start = press_on_edge(&mut session, false, false);
    let to = pt(start.x + 10.0, start.y + 5.0);
    hold(&mut session, to, false, false);
    session.pointer_up(to, false, false);
    assert_eq!(change_count(&session), commits + 1);
    assert_eq!(origin_of(&objects(&session)[0]), pt(20.0, 25.0));
    assert_eq!(objects(&session).len(), 3);
}

/// Criterion 41: a copy of 200 selected objects (100 paths with 50 nodes
/// each, 100 rectangles): the live preview stays within the 8 ms budget per
/// `draw_list()`, and the commit is measured, not budgeted. Run in release:
/// `cargo test --release -p vecmanf-editor-wasm --test edit_polish_move_copy_lock -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn a_200_object_copy_previews_within_the_frame_budget_and_the_commit_is_measured() {
    let document = Document::new(1);
    let mut clicks = Vec::new();
    for i in 0..100u32 {
        let (x, y) = (f64::from(i % 10) * 25.0, f64::from(i / 10) * 25.0);
        let _ = document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        clicks.push(pt(x, y + 5.0));
    }
    for i in 0..100u32 {
        let (cx, cy) = (
            f64::from(i % 10) * 25.0 + 5.0,
            400.0 + f64::from(i / 10) * 25.0,
        );
        let anchors: Vec<NewAnchor> = (0..50u32)
            .map(|n| {
                let a = f64::from(n) / 50.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * 50 + u64::from(n)),
                    pt(cx + 8.0 * a.cos(), cy + 8.0 * a.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
        clicks.push(pt(cx + 8.0, cy));
    }
    let mut session = Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    for (index, at) in clicks.iter().enumerate() {
        let shift = index > 0;
        session.pointer_hover(*at, shift, false);
        session.pointer_down(*at, shift);
        session.pointer_up(*at, shift, false);
    }
    assert_eq!(session.selected_object_count(), 200);

    let start = clicks[0];
    hold(&mut session, start, false, true);
    session.pointer_down(start, false);
    let frames = 30u32;
    let mut drawing = std::time::Duration::ZERO;
    let mut triangles = 0;
    for frame in 0..frames {
        let to = pt(start.x + 5.0 + f64::from(frame), start.y + 3.0);
        hold(&mut session, to, false, true);
        let began = std::time::Instant::now();
        triangles += session.draw_list().triangle_count();
        drawing += began.elapsed();
    }
    let per_frame = drawing / frames;
    println!(
        "200-object copy preview: draw_list {per_frame:?} per frame, {} triangles",
        triangles / frames as usize
    );
    println!(
        "8 ms budget: {}",
        if per_frame < std::time::Duration::from_millis(8) {
            "met"
        } else {
            "missed"
        }
    );
    let to = pt(start.x + 40.0, start.y + 3.0);
    let began = std::time::Instant::now();
    session.pointer_up(to, false, true);
    println!("200-object copy commit: {:?}", began.elapsed());
    assert_eq!(objects(&session).len(), 400, "200 copies");
    assert_eq!(session.selected_object_count(), 200);
    if !cfg!(debug_assertions) {
        assert!(
            per_frame < std::time::Duration::from_millis(20),
            "{per_frame:?} per frame is under 50 frames per second"
        );
    }
}
