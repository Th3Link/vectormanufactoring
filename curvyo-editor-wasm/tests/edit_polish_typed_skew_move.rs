//! `Session`-level tests of Part B of `specs/0010-edit-interaction-polish/
//! specification.md` and the keys M, K and Shift+K of Part F, for PR 3: the
//! typed skew and the typed move, by double-click and by key. Driven through
//! `Session`'s public API only; the rules themselves are tested in
//! `curvyo-ui-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyHint, KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, MoveEntryMode, object_outline_bounds};

/// Screen pixels per millimetre of a fresh session (96 dpi at 100 %).
const SCALE: f64 = 96.0 / 25.4;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect_document() -> Document {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(60.0),
    });
    document
}

fn path_document(size: f64) -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0 + size, 20.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(10.0 + size, 20.0 + size / 2.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(10.0, 20.0 + size / 2.0)),
        ],
        true,
    );
    document
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session
}

fn objects(session: &Session) -> Vec<ObjectSnapshot> {
    let document = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn change_count(session: &Session) -> usize {
    let document = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    doc.len_changes()
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

fn double_click(session: &mut Session, at: Point, shift: bool) -> bool {
    session.pointer_hover(at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
    session.double_click(at, shift, false)
}

fn key(session: &mut Session, key: &str, shift: bool) -> KeyOutcome {
    session.key_down(KeyInput {
        key,
        shift,
        ..KeyInput::default()
    })
}

fn selected(document: &Document, at: Point) -> Session {
    let mut session = session_of(document);
    click(&mut session, at);
    assert_eq!(session.selected_object_count(), 1);
    session
}

fn top_left(session: &Session) -> Point {
    object_outline_bounds(&objects(session)[0]).0
}

fn near(a: Point, x: f64, y: f64) -> bool {
    (a.x - x).abs() < 1e-6 && (a.y - y).abs() < 1e-6
}

/// Criterion 56: M, 5, Tab, -3, Enter moves the object by (5, -3): one
/// commit, the selection and the tool unchanged, the chip closed.
#[test]
fn m_opens_the_move_chip_and_a_relative_entry_is_one_commit() {
    let mut session = selected(&rect_document(), pt(40.0, 20.0));
    let before = top_left(&session);
    assert_eq!(key(&mut session, "m", false), KeyOutcome::EntryOpened);
    let chip = session.move_entry().expect("the move chip");
    assert!(near(chip.center, 60.0, 50.0), "the box centre");
    assert_eq!(chip.relative_prefill, ["0".to_string(), "0".to_string()]);
    assert_eq!(
        chip.absolute_prefill,
        ["10.0".to_string(), "20.0".to_string()]
    );
    assert!(session.transform_entry().is_none());
    let commits = change_count(&session);
    assert_eq!(
        session.commit_move_entry(
            "5",
            "-3",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&session), commits + 1);
    assert!(near(top_left(&session), before.x + 5.0, before.y - 3.0));
    assert!(session.move_entry().is_none(), "the chip closed");
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(session.selected_object_count(), 1);
}

/// Criterion 56: M, 100, Tab, 50, Tab, Space, Enter puts the top-left of the
/// object's bounds at (100, 50).
#[test]
fn an_absolute_entry_puts_the_top_left_of_the_bounds_there() {
    let mut session = selected(&rect_document(), pt(40.0, 20.0));
    key(&mut session, "m", false);
    assert_eq!(
        session.commit_move_entry(
            "100",
            "50",
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
    assert!(near(top_left(&session), 100.0, 50.0));
}

/// Criterion 22: an invalid field keeps the chip open and writes nothing;
/// criterion 25: an unedited chip and a result equal to the position write
/// nothing and close.
#[test]
fn invalid_and_no_op_entries_write_nothing() {
    let mut session = selected(&rect_document(), pt(40.0, 20.0));
    let before = session.pack("0.1.0").unwrap();
    key(&mut session, "m", false);
    assert!(matches!(
        session.commit_move_entry(
            "1,2,3",
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Invalid { field: 0, .. }
    ));
    assert!(session.move_entry().is_some(), "an invalid chip stays open");
    assert_eq!(
        session.commit_move_entry(
            "0",
            "0",
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert!(session.move_entry().is_none());
    key(&mut session, "m", false);
    assert_eq!(
        session.commit_move_entry(
            "10.0",
            "20.0",
            MoveEntryMode {
                absolute: true,
                copy: false
            }
        ),
        EntryOutcome::Unchanged
    );
    assert_eq!(session.pack("0.1.0").unwrap(), before);
}

/// Criterion 15: a double-click on the drawn centre handle opens the move
/// chip for a path and a primitive, writes nothing and changes no tool, hint
/// or selection; the hint chip is not asked for.
#[test]
fn a_double_click_on_the_centre_handle_opens_the_move_chip() {
    for (document, centre) in [
        (rect_document(), pt(60.0, 50.0)),
        (path_document(100.0), pt(60.0, 45.0)),
    ] {
        let mut session = selected(&document, pt(40.0, 20.0));
        let before = session.pack("0.1.0").unwrap();
        assert!(session.move_entry().is_none());
        let hint_requested = double_click(&mut session, centre, false);
        assert!(!hint_requested, "no hint chip");
        assert!(session.move_entry().is_some());
        assert_eq!(session.tool(), Tool::Select);
        assert_eq!(session.selected_object_count(), 1);
        assert_eq!(
            session.pack("0.1.0").unwrap(),
            before,
            "the presses write nothing"
        );
    }
}

/// Criterion 16: a double-click inside the box away from the centre handle, and
/// where the handle is not drawn, keeps the old behaviour: a path hands off to
/// the Node tool, a primitive asks for the hint chip.
#[test]
fn elsewhere_in_the_box_the_double_click_keeps_its_behaviour() {
    let mut rect = selected(&rect_document(), pt(40.0, 20.0));
    assert!(double_click(&mut rect, pt(30.0, 70.0), false));
    assert!(rect.move_entry().is_none());
    let mut path = selected(&path_document(100.0), pt(40.0, 20.0));
    assert!(!double_click(&mut path, pt(30.0, 60.0), false));
    assert_eq!(path.tool(), Tool::Node);
    assert!(path.move_entry().is_none());
    // A box under 48 px: the centre handle is not drawn, so no typed move.
    let tiny = Document::new(1);
    let _ = tiny.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(8.0),
        height: Length::from_mm(8.0),
    });
    let mut small = selected(&tiny, pt(10.0, 24.0));
    assert!(double_click(&mut small, pt(14.0, 24.0), false));
    assert!(small.move_entry().is_none());
    // M opens it all the same, with the chip where the handle would be.
    assert_eq!(key(&mut small, "m", false), KeyOutcome::EntryOpened);
    assert!(near(small.move_entry().unwrap().center, 14.0, 24.0));
}

/// Criterion 9: K opens the skew entry of the top handle, Shift+K that of the
/// right handle, with the accessible names, the prefill "0" and the chip at
/// the handle's position even for a path far too small to draw it; one commit
/// of the skew by the typed angle.
#[test]
fn k_and_shift_k_open_the_skew_chips_and_enter_skews_in_one_commit() {
    for size in [4.0, 100.0] {
        for (shift, name) in [(false, "Skew angle x"), (true, "Skew angle y")] {
            let mut session = selected(&path_document(size), pt(10.0 + size / 2.0, 20.0));
            assert_eq!(
                key(&mut session, "k", shift),
                KeyOutcome::EntryOpened,
                "{size}"
            );
            let chip = session.transform_entry().expect("a skew chip");
            assert_eq!(chip.kind, "skew");
            assert_eq!(chip.fields.len(), 1);
            assert_eq!(chip.fields[0].accessible_name, name);
            assert_eq!(chip.fields[0].prefill, "0");
            assert_eq!(chip.fields[0].label, "");
            assert!(!chip.linked);
            // The handle sits outside the box, at the middle of its side.
            if shift {
                assert!((chip.handle.y - (20.0 + size / 4.0)).abs() < 1e-6);
                assert!(chip.handle.x > 10.0 + size);
            } else {
                assert!((chip.handle.x - (10.0 + size / 2.0)).abs() < 1e-6);
                assert!(chip.handle.y < 20.0);
            }
            let commits = change_count(&session);
            let before = objects(&session);
            assert_eq!(
                session.commit_transform_entry("30", "", 0),
                EntryOutcome::Committed
            );
            assert_eq!(change_count(&session), commits + 1);
            assert_ne!(objects(&session), before);
            assert!(session.transform_entry().is_none());
            assert_eq!(session.tool(), Tool::Select);
        }
    }
}

/// Criterion 9, 10: the skew chip by double-click on the skew handle, and the
/// typed 45 degrees of the worked example (20 x 10 path, top handle: the top
/// edge moves +10 mm in x).
#[test]
fn a_double_click_on_a_skew_handle_opens_the_chip_and_the_worked_example_holds() {
    let mut session = selected(&path_document(20.0), pt(20.0, 20.0));
    key(&mut session, "k", false);
    let handle = session.transform_entry().unwrap().handle;
    session.cancel_transform_entry();
    assert!(!double_click(&mut session, handle, false));
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(session.transform_entry().unwrap().kind, "skew");
    assert_eq!(
        session.commit_transform_entry("45", "", 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Path(path) = &objects(&session)[0] else {
        panic!("a path");
    };
    let xs: Vec<f64> = path.anchors.iter().map(|a| a.point.x).collect();
    for (got, want) in xs.iter().zip([20.0, 40.0, 30.0, 10.0]) {
        assert!((got - want).abs() < 1e-9, "{xs:?}");
    }
}

/// Criterion 11: the skew chip's refusals go over the string the chip reads.
#[test]
fn the_skew_chip_refuses_out_of_range_text_and_writes_nothing() {
    let mut session = selected(&path_document(100.0), pt(60.0, 20.0));
    let before = session.pack("0.1.0").unwrap();
    key(&mut session, "k", false);
    for text in ["90", "-90", "abc", ""] {
        assert!(
            matches!(
                session.commit_transform_entry(text, "", 0),
                EntryOutcome::Invalid { field: 0, .. }
            ),
            "{text:?}"
        );
        assert!(
            session.transform_entry().is_some(),
            "{text:?} keeps it open"
        );
    }
    assert_eq!(
        session.commit_transform_entry("0", "", 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(session.pack("0.1.0").unwrap(), before);
    assert!(session.transform_entry().is_none());
}

/// Criterion 59: the entry keys that cannot act show a hint and change
/// nothing: nothing selected and M or K outside the Select tool "select
/// first", several objects "select one", a non-path "paths only".
#[test]
fn the_entry_keys_that_cannot_act_give_the_hints() {
    // Nothing selected.
    let mut session = session_of(&rect_document());
    for (k, shift) in [("m", false), ("k", false), ("k", true)] {
        assert_eq!(
            key(&mut session, k, shift),
            KeyOutcome::Hint(KeyHint::SelectFirst),
            "{k}"
        );
    }
    assert_eq!(session.tool(), Tool::Select);
    // Another tool, even with an object selected in the Select tool before.
    let mut session = selected(&rect_document(), pt(40.0, 20.0));
    session.set_tool(Tool::Rectangle);
    for k in ["m", "k"] {
        let before = session.pack("0.1.0").unwrap();
        assert_eq!(
            key(&mut session, k, false),
            KeyOutcome::Hint(KeyHint::SelectFirst)
        );
        assert_eq!(session.tool(), Tool::Rectangle, "no tool change");
        assert_eq!(session.pack("0.1.0").unwrap(), before);
    }
    // A non-path.
    let mut session = selected(&rect_document(), pt(40.0, 20.0));
    for shift in [false, true] {
        assert_eq!(
            key(&mut session, "k", shift),
            KeyOutcome::Hint(KeyHint::PathOnly)
        );
        assert!(session.transform_entry().is_none());
    }
    assert_eq!(session.selected_object_count(), 1);
    // Several objects.
    let document = rect_document();
    let _ = document.create_rect(RectBounds {
        origin: pt(200.0, 20.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let mut session = session_of(&document);
    click(&mut session, pt(40.0, 20.0));
    session.pointer_hover(pt(220.0, 20.0), true, false);
    session.pointer_down(pt(220.0, 20.0), true);
    session.pointer_up(pt(220.0, 20.0), true, false);
    assert_eq!(session.selected_object_count(), 2);
    for (k, shift) in [("m", false), ("k", false), ("k", true)] {
        assert_eq!(
            key(&mut session, k, shift),
            KeyOutcome::Hint(KeyHint::SelectOne)
        );
    }
}

/// Criterion 12 and 25: Escape closes the chip first and writes nothing; a
/// click elsewhere, a tool switch and a selection change close it without
/// writing; the press that closed it is not swallowed.
#[test]
fn every_way_of_closing_writes_nothing() {
    for k in ["m", "k"] {
        let doc = if k == "k" {
            path_document(100.0)
        } else {
            rect_document()
        };
        let _ = doc.create_rect(RectBounds {
            origin: pt(200.0, 20.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(40.0),
        });
        let at = pt(40.0, 20.0);
        let open = |session: &mut Session| {
            click(session, at);
            assert_eq!(key(session, k, false), KeyOutcome::EntryOpened);
        };
        let chip_open = |session: &Session| {
            session.move_entry().is_some() || session.transform_entry().is_some()
        };
        let mut session = session_of(&doc);
        let before = session.pack("0.1.0").unwrap();
        open(&mut session);
        assert!(chip_open(&session));
        assert_eq!(
            session.key_down(KeyInput {
                key: "Escape",
                ..KeyInput::default()
            }),
            KeyOutcome::Escape(EscapeStep::ClosedEntry)
        );
        assert!(!chip_open(&session));
        assert_eq!(
            session.selected_object_count(),
            1,
            "Escape closed only the chip"
        );
        // A click on another object closes the chip and selects that object.
        open(&mut session);
        click(&mut session, pt(220.0, 20.0));
        assert!(!chip_open(&session));
        assert_eq!(session.selected_object_count(), 1);
        // A tool switch closes it.
        click(&mut session, at);
        assert_eq!(key(&mut session, k, false), KeyOutcome::EntryOpened);
        session.set_tool(Tool::Node);
        assert!(!chip_open(&session));
        assert_eq!(
            session.pack("0.1.0").unwrap(),
            before,
            "{k}: nothing written"
        );
    }
}

/// Criterion 59: while the M, K or Shift+K chip is open the Shift-revealed
/// side rotate handles are hidden; the key never reveals one.
#[test]
fn an_open_entry_chip_shows_no_side_rotate_handle() {
    let mut session = selected(&path_document(100.0), pt(40.0, 20.0));
    let plain = session.draw_list().triangle_count();
    session.modifiers_changed(true, false, false);
    let shifted = session.draw_list().triangle_count();
    assert_ne!(plain, shifted, "Shift reveals the side rotate handles");
    session.modifiers_changed(false, false, false);
    key(&mut session, "m", false);
    let no_shift = session.draw_list().triangle_count();
    session.modifiers_changed(true, false, false);
    assert_eq!(
        session.draw_list().triangle_count(),
        no_shift,
        "Shift reveals nothing while the chip is open"
    );
}

/// Criterion 25: the hint of the centre handle and the cursor stay `move`.
#[test]
fn the_hint_names_stay_for_the_centre_and_split_for_the_skew_sides() {
    let mut session = selected(&path_document(100.0), pt(40.0, 20.0));
    session.pointer_hover(pt(60.0, 45.0), false, false);
    assert_eq!(session.handle_hint(), "move");
    let top = pt(60.0, 20.0 - 16.0 / SCALE);
    session.pointer_hover(top, false, false);
    assert_eq!(session.handle_hint(), "skew");
    let right = pt(110.0 + 16.0 / SCALE, 45.0);
    session.pointer_hover(right, false, false);
    assert_eq!(session.handle_hint(), "skew-y");
}
