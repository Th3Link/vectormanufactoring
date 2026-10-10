//! `Session`-level tests of `specs/0019-multi-object-transform/`: the group box and
//! its handles, moving a selection, the typed entries and the transforms
//! through the public API of `Session`. The rules themselves are tested in
//! `curvyo-ui-core` and `curvyo-document-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
#![allow(clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Shape, pack,
    unpack,
};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const ACCENT: (u8, u8, u8) = (0x2F, 0x6F, 0xEE);
const MEMBER_ALPHA: u8 = 153;

/// Two 100 x 60 mm rectangles at (20, 20) and (160, 20), so the group box is
/// (20, 20) to (260, 80) with its centre handle at (140, 50); a small
/// unfilled rectangle between them; and a path far below.
fn document() -> Document {
    let document = Document::new(1);
    for x in [20.0, 160.0] {
        let _ = document.create_rect(RectBounds {
            origin: pt(x, 20.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
    }
    let _ = document.create_rect(RectBounds {
        origin: pt(130.0, 30.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(20.0),
    });
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(20.0, 150.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(80.0, 190.0)),
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

fn hold(session: &mut Session, at: Point, shift: bool, ctrl: bool) {
    session.modifiers_changed(shift, ctrl, false);
    session.pointer_hover(at, shift, ctrl);
}

fn click(session: &mut Session, at: Point, shift: bool) {
    hold(session, at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
    hold(session, at, false, false);
}

/// Selects the two big rectangles: a click on the first one's left edge and a
/// Shift-click on the second one's.
fn select_both(session: &mut Session) {
    click(session, pt(20.0, 50.0), false);
    click(session, pt(160.0, 50.0), true);
    assert_eq!(session.selected_object_count(), 2);
}

fn drag(session: &mut Session, from: Point, to: Point) {
    hold(session, from, false, false);
    session.pointer_down(from, false);
    let middle = pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y));
    hold(session, middle, false, false);
    hold(session, to, false, false);
    session.pointer_up(to, false, false);
    hold(session, to, false, false);
}

fn count_colour(session: &Session, rgb: (u8, u8, u8), alpha: u8) -> usize {
    session
        .draw_list()
        .triangles
        .iter()
        .filter(|v| (v.color.r, v.color.g, v.color.b, v.color.a) == (rgb.0, rgb.1, rgb.2, alpha))
        .count()
}

fn key(session: &mut Session, key: &str, shift: bool) -> KeyOutcome {
    session.key_down(KeyInput {
        key,
        shift,
        ..KeyInput::default()
    })
}

/// Whether an `--accent` triangle of full opacity has a vertex within 1 mm of
/// `at`.
fn accent_ink_near(session: &Session, at: Point) -> bool {
    session.draw_list().triangles.iter().any(|v| {
        (v.color.r, v.color.g, v.color.b, v.color.a) == (ACCENT.0, ACCENT.1, ACCENT.2, 255)
            && (v.position.x - at.x).abs() < 1.0
            && (v.position.y - at.y).abs() < 1.0
    })
}

/// Criteria 1 and 4: two selected objects draw the group box in `--accent`
/// around both (its top edge spans the gap between them) and the lighter
/// member boxes; one object draws its own box and no member box.
#[test]
fn a_multi_selection_draws_a_group_box_with_lighter_member_boxes() {
    let mut session = session();
    click(&mut session, pt(20.0, 50.0), false);
    assert_eq!(
        count_colour(&session, ACCENT, MEMBER_ALPHA),
        0,
        "one object"
    );
    assert!(
        !accent_ink_near(&session, pt(140.0, 20.0)),
        "no box over the gap"
    );
    click(&mut session, pt(160.0, 50.0), true);
    assert!(
        count_colour(&session, ACCENT, MEMBER_ALPHA) > 0,
        "member boxes"
    );
    let top = (0..40).any(|i| accent_ink_near(&session, pt(121.0 + f64::from(i), 20.0)));
    assert!(top, "the group box spans the gap between the two objects");
}

/// Criteria 6 and 52: a selection that drops from two to one shows the box and
/// handles of one object again.
#[test]
fn a_selection_dropping_to_one_object_has_no_group_box() {
    let mut session = session();
    select_both(&mut session);
    click(&mut session, pt(160.0, 50.0), true);
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(count_colour(&session, ACCENT, MEMBER_ALPHA), 0);
}

/// Criteria 16 and 17: a press on the centre handle moves the whole selection
/// with the pointer, in one commit, keeping the relative positions.
#[test]
fn the_centre_handle_moves_the_selection_in_one_commit() {
    let mut session = session();
    select_both(&mut session);
    let before = objects(&session);
    let commits = change_count(&session);
    // The group box (20, 20) to (260, 80): the centre handle at (140, 50).
    assert_eq!(session.cursor_hint(), "default");
    hold(&mut session, pt(140.0, 50.0), false, false);
    assert_eq!(session.cursor_hint(), "move");
    drag(&mut session, pt(140.0, 50.0), pt(160.0, 90.0));
    let after = objects(&session);
    assert_eq!(change_count(&session), commits + 1, "one commit");
    for index in [0, 1] {
        let (a, b) = (origin_of(&before[index]), origin_of(&after[index]));
        assert_eq!((b.x - a.x, b.y - a.y), (20.0, 40.0));
    }
    assert_eq!(origin_of(&before[2]), origin_of(&after[2]), "not selected");
    assert_eq!(session.selected_object_count(), 2);
}

/// Criterion 16: a drag from a selected object's outline moves the selection.
#[test]
fn a_selected_objects_outline_moves_the_selection() {
    let mut session = session();
    select_both(&mut session);
    let before = objects(&session);
    drag(&mut session, pt(20.0, 50.0), pt(30.0, 50.0));
    let after = objects(&session);
    for index in [0, 1] {
        assert_eq!(
            origin_of(&after[index]).x - origin_of(&before[index]).x,
            10.0
        );
    }
}

/// Criterion 45: a press on empty canvas inside the group box does not move
/// the selection: a click clears it, a drag is a marquee.
#[test]
fn the_empty_interior_of_the_group_box_clears_or_marquees() {
    let mut session = session();
    select_both(&mut session);
    let commits = change_count(&session);
    // (125, 70) is inside the group box, on no object and on no handle.
    click(&mut session, pt(125.0, 70.0), false);
    assert_eq!(session.selected_object_count(), 0);
    assert_eq!(change_count(&session), commits);

    let mut session = self::session();
    select_both(&mut session);
    drag(&mut session, pt(125.0, 70.0), pt(135.0, 76.0));
    assert_eq!(change_count(&session), commits, "a marquee writes nothing");
}

/// Criterion 43 step 6: a plain press on an unselected object inside the group
/// box selects that object.
#[test]
fn a_press_on_an_unselected_object_inside_the_box_selects_it() {
    let mut session = session();
    select_both(&mut session);
    // The small rectangle's left edge, at (130, 40), is inside the group box.
    click(&mut session, pt(130.0, 40.0), false);
    assert_eq!(session.selected_object_count(), 1);
}

/// Criterion 17: Shift locks the move to one axis, and the origin axes pass
/// through the group box centre.
#[test]
fn shift_locks_a_group_move_to_one_axis() {
    let mut session = session();
    select_both(&mut session);
    let before = objects(&session);
    hold(&mut session, pt(140.0, 50.0), true, false);
    session.pointer_down(pt(140.0, 50.0), true);
    hold(&mut session, pt(170.0, 60.0), true, false);
    assert!(
        session
            .live_readout()
            .unwrap()
            .text
            .starts_with("Δ 30.0, 0.0")
    );
    assert!(count_colour(&session, ACCENT, 128) > 0, "the origin axes");
    session.pointer_up(pt(170.0, 60.0), true, false);
    let after = objects(&session);
    let (a, b) = (origin_of(&before[0]), origin_of(&after[0]));
    assert_eq!((b.x - a.x, b.y - a.y), (30.0, 0.0));
}

/// Criterion 17: Ctrl copies every selected object and selects the copies.
#[test]
fn ctrl_copies_the_whole_selection() {
    let mut session = session();
    select_both(&mut session);
    let before = ids(&session).len();
    hold(&mut session, pt(140.0, 50.0), false, true);
    session.pointer_down(pt(140.0, 50.0), false);
    hold(&mut session, pt(140.0, 120.0), false, true);
    session.pointer_up(pt(140.0, 120.0), false, true);
    assert_eq!(ids(&session).len(), before + 2);
    assert_eq!(session.selected_object_count(), 2);
}

/// Criterion 35: M opens the move chip; Absolute puts the top-left corner of
/// the group box at (X, Y).
#[test]
fn the_typed_move_uses_the_group_box_top_left() {
    let mut session = session();
    select_both(&mut session);
    assert_eq!(key(&mut session, "m", false), KeyOutcome::EntryOpened);
    let view = session.move_entry().expect("a move chip");
    assert_eq!(
        view.absolute_prefill,
        ["20.0".to_string(), "20.0".to_string()]
    );
    assert_eq!(view.center, pt(140.0, 50.0));
    let before = objects(&session);
    let outcome = session.commit_move_entry(
        "100",
        "40",
        curvyo_ui_core::MoveEntryMode {
            absolute: true,
            copy: false,
        },
    );
    assert_eq!(outcome, curvyo_ui_core::EntryOutcome::Committed);
    let after = objects(&session);
    for index in [0, 1] {
        let (a, b) = (origin_of(&before[index]), origin_of(&after[index]));
        assert_eq!((b.x - a.x, b.y - a.y), (80.0, 20.0));
    }
}

/// Criterion 50: the moved selection is the same after saving and reopening.
#[test]
fn a_moved_selection_survives_a_save_and_reopen() {
    let mut session = session();
    select_both(&mut session);
    drag(&mut session, pt(140.0, 50.0), pt(150.0, 60.0));
    let saved = session.pack("0.1.0").unwrap();
    let reopened = Session::open(3, &saved).unwrap();
    let again = reopened.pack("0.1.0").unwrap();
    assert_eq!(objects(&session), {
        let document = unpack(99, &again).unwrap();
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect::<Vec<_>>()
    });
}

/// Criterion 38: the centre handle's hint chip lines.
#[test]
fn the_centre_handle_has_the_selection_hint() {
    let mut session = session();
    select_both(&mut session);
    hold(&mut session, pt(140.0, 50.0), false, false);
    assert_eq!(session.handle_hint(), "group");
    assert_eq!(
        session.corner_hint_lines(),
        [
            "Move selection",
            "Shift: keep one axis",
            "Ctrl: copy",
            "Double-click or M: type an offset"
        ]
    );
    hold(&mut session, pt(125.0, 70.0), false, false);
    assert_eq!(session.handle_hint(), "");
}
