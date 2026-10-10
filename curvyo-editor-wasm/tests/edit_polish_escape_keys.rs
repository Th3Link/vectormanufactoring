//! `Session`-level tests of Parts D and F of `specs/0010-edit-interaction-polish/
//! specification.md` for PR 1: the Escape cascade of every tool (criteria 42
//! to 49, 60), the key gate and key table (criteria 54, 55, 57, 60, 61), and
//! Split's selection (criteria 50 to 52). Driven through `Session`'s public
//! API only; the pure key table is also tested in `session/keys.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, PointCount, RectBounds, pack,
    unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};

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

fn path_document() -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(10.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(110.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(110.0, 80.0)),
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

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

fn press(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
}

fn key(session: &mut Session, key: &str) -> KeyOutcome {
    session.key_down(KeyInput {
        key,
        ..KeyInput::default()
    })
}

fn escape(session: &mut Session) -> EscapeStep {
    match key(session, "Escape") {
        KeyOutcome::Escape(step) => step,
        other => panic!("Escape gave {other:?}"),
    }
}

// ---------------------------------------------------------------------
// The Escape cascade: criteria 42 to 49
// ---------------------------------------------------------------------

/// Criterion 46: in the Select tool Escape clears the selection, then does
/// nothing; the tool stays Select.
#[test]
fn escape_in_the_select_tool_clears_the_selection_then_does_nothing() {
    let mut session = session_of(&rect_document());
    click(&mut session, pt(10.0, 50.0));
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    assert_eq!(session.selected_object_count(), 0);
    let bar = session.select_bar_state();
    assert!(
        bar.radius.is_none() && !bar.remove_rounding_shown && !bar.object_to_path,
        "no per-kind bar groups without a selection"
    );
    assert_eq!(escape(&mut session), EscapeStep::Nothing);
    assert_eq!(session.tool(), Tool::Select);
}

/// Criteria 42 and 49: during a Select-tool drag Escape cancels only the
/// drag (selection as after the press, nothing written); with the button
/// still held a second Escape does nothing more; after the release the next
/// Escape clears the selection.
#[test]
fn escape_during_a_select_drag_cancels_it_and_a_second_one_waits_for_the_release() {
    let document = rect_document();
    let mut session = session_of(&document);
    click(&mut session, pt(10.0, 50.0));
    let before = objects(&session);
    press(&mut session, pt(10.0, 50.0));
    session.pointer_hover(pt(60.0, 90.0), false, false);
    assert_eq!(escape(&mut session), EscapeStep::CancelledDrag);
    assert_eq!(
        session.selected_object_count(),
        1,
        "selection as after the press"
    );
    assert_eq!(
        escape(&mut session),
        EscapeStep::Nothing,
        "button still held"
    );
    assert_eq!(session.selected_object_count(), 1);
    session.pointer_up(pt(60.0, 90.0), false, false);
    assert_eq!(objects(&session), before, "nothing was written");
    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    assert_eq!(session.selected_object_count(), 0);
}

/// Criterion 42, step 1: an open entry chip closes first; nothing else
/// happens in that press.
#[test]
fn escape_closes_an_open_entry_before_anything_else() {
    let mut session = session_of(&rect_document());
    click(&mut session, pt(10.0, 50.0));
    assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
    assert!(session.transform_entry().is_some());
    assert_eq!(escape(&mut session), EscapeStep::ClosedEntry);
    assert!(session.transform_entry().is_none());
    assert_eq!(
        session.selected_object_count(),
        1,
        "the selection is untouched"
    );
    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
}

/// Criterion 43: Pen. Escape discards the unfinished path, a second Escape
/// switches to the Select tool, a third does nothing.
#[test]
fn escape_in_the_pen_discards_the_path_then_leaves_the_tool() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    for at in [pt(10.0, 10.0), pt(50.0, 10.0), pt(50.0, 40.0)] {
        click(&mut session, at);
    }
    assert_eq!(session.pen_in_progress().map(<[_]>::len), Some(3));
    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    assert!(session.pen_in_progress().is_none());
    assert!(
        objects(&session).is_empty(),
        "an unfinished path is never kept"
    );
    assert_eq!(session.tool(), Tool::Pen);
    assert_eq!(escape(&mut session), EscapeStep::LeftTool);
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(escape(&mut session), EscapeStep::Nothing);
}

/// Criterion 43: also while a node's handle is dragged, the pen's step 2 and
/// 3 are one, nothing is written; and Enter still finishes a path.
#[test]
fn the_pen_discards_during_a_handle_drag_and_enter_finishes() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    click(&mut session, pt(10.0, 10.0));
    press(&mut session, pt(50.0, 10.0));
    session.pointer_hover(pt(70.0, 30.0), false, false);
    assert_eq!(escape(&mut session), EscapeStep::CancelledDrag);
    assert!(session.pen_in_progress().is_none());
    session.pointer_up(pt(70.0, 30.0), false, false);
    assert_eq!(objects(&session).len(), 0, "nothing was written");

    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    click(&mut session, pt(10.0, 10.0));
    click(&mut session, pt(50.0, 10.0));
    assert_eq!(key(&mut session, "Enter"), KeyOutcome::PenFinished);
    assert_eq!(objects(&session).len(), 1);
}

/// Criteria 44 and 49: the creation tools have no step 3. A cancelled
/// drag stays in the tool (a second Escape with the button held does
/// nothing); the first Escape without a drag leaves for Select with nothing
/// selected, where the next Escape does nothing (criterion 48).
#[test]
fn escape_in_a_creation_tool_cancels_the_drag_then_leaves_the_tool() {
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
        let mut session = Session::new(1);
        session.set_tool(tool);
        press(&mut session, pt(10.0, 10.0));
        session.pointer_hover(pt(40.0, 30.0), false, false);
        assert_eq!(escape(&mut session), EscapeStep::CancelledDrag, "{tool:?}");
        assert_eq!(session.tool(), tool);
        assert!(session.live_readout().is_none(), "no preview remains");
        assert_eq!(
            escape(&mut session),
            EscapeStep::Nothing,
            "button still held"
        );
        assert_eq!(session.tool(), tool);
        session.pointer_up(pt(40.0, 30.0), false, false);
        assert!(objects(&session).is_empty(), "nothing is written, no shape");
        assert_eq!(escape(&mut session), EscapeStep::LeftTool, "{tool:?}");
        assert_eq!(session.tool(), Tool::Select);
        assert_eq!(session.selected_object_count(), 0);
        assert_eq!(escape(&mut session), EscapeStep::Nothing);
    }
}

/// Criteria 45, 47, 49: in the Node tool a drag is cancelled with the node
/// selection kept; the next Escape clears the node selection and stays in the
/// tool; the next leaves for Select with the object selection as it was.
#[test]
fn escape_in_the_node_tool_takes_three_steps() {
    let mut session = session_of(&path_document());
    click(&mut session, pt(60.0, 20.0));
    assert_eq!(session.selected_object_count(), 1);
    session.set_tool(Tool::Node);
    click(&mut session, pt(10.0, 20.0));
    assert!(
        session.node_toolbar_state().can_delete,
        "a node is selected"
    );
    let before = objects(&session);

    press(&mut session, pt(10.0, 20.0));
    session.pointer_hover(pt(30.0, 40.0), false, false);
    assert_eq!(escape(&mut session), EscapeStep::CancelledDrag);
    assert!(
        session.node_toolbar_state().can_delete,
        "the selection is kept"
    );
    assert_eq!(
        escape(&mut session),
        EscapeStep::Nothing,
        "button still held"
    );
    assert!(session.node_toolbar_state().can_delete);
    session.pointer_up(pt(30.0, 40.0), false, false);
    assert_eq!(objects(&session), before, "the node never moved");

    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    assert_eq!(session.tool(), Tool::Node);
    assert!(!session.node_toolbar_state().can_delete);
    assert_eq!(escape(&mut session), EscapeStep::LeftTool);
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(
        session.selected_object_count(),
        1,
        "the path stays selected (criterion 47)"
    );
    assert_eq!(objects(&session), before);
}

/// Criterion 60: a held Escape acts once: the key repeat does nothing, in
/// the Node tool it clears the selection once and does not go on to leave.
#[test]
fn a_held_escape_acts_once() {
    let mut session = session_of(&path_document());
    session.set_tool(Tool::Node);
    click(&mut session, pt(10.0, 20.0));
    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    for _ in 0..5 {
        let outcome = session.key_down(KeyInput {
            key: "Escape",
            repeat: true,
            ..KeyInput::default()
        });
        assert_eq!(outcome, KeyOutcome::Ignored);
    }
    assert_eq!(session.tool(), Tool::Node);
}

/// A lost pointer cancels the drag and forgets the button; the following
/// Escape then steps on.
#[test]
fn a_cancelled_pointer_cancels_the_drag_and_releases_the_button() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Rectangle);
    press(&mut session, pt(10.0, 10.0));
    session.pointer_hover(pt(40.0, 30.0), false, false);
    session.pointer_cancelled();
    assert!(session.live_readout().is_none());
    assert_eq!(escape(&mut session), EscapeStep::LeftTool);
}

// ---------------------------------------------------------------------
// The key gate and key table: criteria 54, 55, 57, 61
// ---------------------------------------------------------------------

/// Criterion 55: during a Select-tool move drag no key changes a tool,
/// deletes or opens anything, and the drag commits exactly what the same drag
/// without the key commits.
#[test]
fn no_key_acts_during_a_drag() {
    let run = |keys: &[&str]| {
        let mut session = session_of(&rect_document());
        click(&mut session, pt(10.0, 50.0));
        press(&mut session, pt(10.0, 50.0));
        session.pointer_hover(pt(40.0, 70.0), false, false);
        for k in keys {
            assert_eq!(
                key(&mut session, k),
                KeyOutcome::Ignored,
                "{k} during a drag"
            );
            assert_eq!(session.tool(), Tool::Select);
        }
        session.pointer_up(pt(40.0, 70.0), false, false);
        objects(&session)
    };
    let with = run(&[
        "r",
        "e",
        "b",
        "n",
        "s",
        "m",
        "k",
        "*",
        "Delete",
        "Backspace",
        "R",
    ]);
    let without = run(&[]);
    assert_eq!(with, without);
    assert_eq!(with.len(), 1, "nothing was deleted");
}

/// Criterion 55 for the other tools' drags: Node, creation tools.
#[test]
fn no_key_acts_during_a_node_or_create_drag() {
    let mut session = session_of(&path_document());
    session.set_tool(Tool::Node);
    press(&mut session, pt(10.0, 20.0));
    session.pointer_hover(pt(20.0, 25.0), false, false);
    for k in ["b", "e", "r", "s", "Delete"] {
        assert_eq!(key(&mut session, k), KeyOutcome::Ignored, "{k}");
    }
    assert_eq!(session.tool(), Tool::Node);
    session.pointer_up(pt(20.0, 25.0), false, false);

    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
        let mut session = Session::new(1);
        session.set_tool(tool);
        press(&mut session, pt(5.0, 5.0));
        session.pointer_hover(pt(30.0, 20.0), false, false);
        for k in ["b", "n", "e", "r", "s", "*"] {
            assert_eq!(key(&mut session, k), KeyOutcome::Ignored, "{k} in {tool:?}");
        }
        assert_eq!(session.tool(), tool);
    }
}

/// Criterion 55: a pan is an operation too.
#[test]
fn no_key_acts_during_a_pan() {
    let mut session = Session::new(1);
    session.begin_pan(10.0, 10.0);
    assert_eq!(key(&mut session, "b"), KeyOutcome::Ignored);
    assert_eq!(session.tool(), Tool::Select);
    session.end_pan();
    assert_eq!(key(&mut session, "b"), KeyOutcome::ToolChanged);
}

/// Criterion 55: Ctrl+R, Ctrl+S, Cmd+R and Alt+E change no tool; the page's
/// default is left alone (`Ignored`); the key repeat and the DOM flag do the
/// same.
#[test]
fn modified_repeated_and_dom_blocked_letters_do_nothing() {
    let mut session = Session::new(1);
    for (k, ctrl, alt, repeat, dom_blocked) in [
        ("r", true, false, false, false),
        ("s", true, false, false, false),
        ("e", false, true, false, false),
        ("b", false, false, true, false),
        ("Delete", false, false, true, false),
        ("n", false, false, false, true),
    ] {
        let outcome = session.key_down(KeyInput {
            key: k,
            shift: false,
            ctrl,
            alt,
            repeat,
            dom_blocked,
        });
        assert_eq!(outcome, KeyOutcome::Ignored, "{k}");
        assert_eq!(session.tool(), Tool::Select);
    }
}

/// Criterion 55: with a Pen path of three nodes open, B, N, E, R, S, `*` and
/// Delete do nothing; Enter finishes it.
#[test]
fn an_open_pen_path_blocks_the_tool_letters() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    for at in [pt(10.0, 10.0), pt(50.0, 10.0), pt(50.0, 40.0)] {
        click(&mut session, at);
    }
    for k in ["b", "n", "e", "r", "s", "*", "Delete", "Backspace"] {
        assert_eq!(key(&mut session, k), KeyOutcome::Ignored, "{k}");
        assert_eq!(session.tool(), Tool::Pen);
        assert_eq!(session.pen_in_progress().map(<[_]>::len), Some(3));
    }
    assert_eq!(key(&mut session, "Enter"), KeyOutcome::PenFinished);
    assert_eq!(objects(&session).len(), 1);
    // With no path the letters work again.
    assert_eq!(key(&mut session, "n"), KeyOutcome::ToolChanged);
    assert_eq!(session.tool(), Tool::Node);
}

/// Criterion 54: the tool letters, case-insensitively, `*` with Shift.
#[test]
fn the_tool_letters_switch_tools() {
    let mut session = Session::new(1);
    for (k, shift, tool) in [
        ("b", false, Tool::Pen),
        ("N", false, Tool::Node),
        ("e", false, Tool::Ellipse),
        ("r", false, Tool::Rectangle),
        ("*", true, Tool::PolygonStar),
        ("s", false, Tool::Select),
    ] {
        let outcome = session.key_down(KeyInput {
            key: k,
            shift,
            ..KeyInput::default()
        });
        assert_eq!(outcome, KeyOutcome::ToolChanged, "{k}");
        assert_eq!(session.tool(), tool, "{k}");
    }
    // Shift with a letter does nothing.
    let outcome = session.key_down(KeyInput {
        key: "B",
        shift: true,
        ..KeyInput::default()
    });
    assert_eq!(outcome, KeyOutcome::Ignored);
    assert_eq!(session.tool(), Tool::Select);
}

/// Criterion 54: R and S open the entries in the Select tool with one object
/// selected, and the example of the criterion: after a rectangle is drawn R
/// opens its angle entry, and to draw the next one the maker presses Escape
/// (the selection clears) and then R.
#[test]
fn r_after_drawing_a_rectangle_opens_the_angle_entry_and_escape_then_r_draws() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Rectangle);
    press(&mut session, pt(10.0, 10.0));
    session.pointer_hover(pt(60.0, 40.0), false, false);
    session.pointer_up(pt(60.0, 40.0), false, false);
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(session.selected_object_count(), 1);

    assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
    let entry = session.transform_entry().expect("the angle entry");
    assert_eq!(entry.kind, "angle");
    assert_eq!(escape(&mut session), EscapeStep::ClosedEntry);
    assert_eq!(key(&mut session, "s"), KeyOutcome::EntryOpened);
    assert_eq!(session.transform_entry().expect("size entry").kind, "size");
    assert_eq!(escape(&mut session), EscapeStep::ClosedEntry);
    // A typed letter while a chip is open changes nothing.
    assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
    assert_eq!(key(&mut session, "b"), KeyOutcome::Ignored);
    assert_eq!(escape(&mut session), EscapeStep::ClosedEntry);

    assert_eq!(escape(&mut session), EscapeStep::ClearedState);
    assert_eq!(key(&mut session, "r"), KeyOutcome::ToolChanged);
    assert_eq!(session.tool(), Tool::Rectangle);
}

/// Criterion 57: the key route opens the same entry as the double-click
/// route and Enter commits one write; R, 0, Enter stands a star shown at -15
/// back to 0 (first tip straight right).
#[test]
fn r_zero_enter_stands_a_star_back_to_zero() {
    let mut session = Session::new(1);
    session.set_tool(Tool::PolygonStar);
    session.set_poly_star_mode(curvyo_ui_core::PolyStarMode::Star);
    session.set_poly_star_point_count(PointCount::new(5).unwrap());
    session.pointer_hover(pt(100.0, 50.0), false, false);
    session.pointer_down(pt(100.0, 50.0), false);
    session.pointer_hover(pt(110.0, 48.0), false, true);
    session.pointer_up(pt(110.0, 48.0), false, true);

    assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
    let entry = session.transform_entry().expect("angle entry");
    assert_eq!(entry.fields[0].prefill, "-15");
    assert_eq!(
        session.commit_transform_entry("0", "", 0),
        curvyo_ui_core::EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(star) = objects(&session).remove(0) else {
        panic!("a star");
    };
    assert!(
        ObjectSnapshot::Primitive(star)
            .orientation()
            .as_radians()
            .abs()
            < 1e-9
    );
    assert!(session.transform_entry().is_none(), "the entry closed");
}

/// Criterion 57 for an object too small to draw every handle: the corner
/// handles always exist, so R and S still open their entries.
#[test]
fn r_and_s_open_for_a_tiny_object() {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: Length::from_mm(15.0 / SCALE),
        height: Length::from_mm(15.0 / SCALE),
    });
    let mut session = session_of(&document);
    click(&mut session, pt(10.0, 10.0 + 7.0 / SCALE));
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(key(&mut session, "r"), KeyOutcome::EntryOpened);
    assert!(session.transform_entry().is_some());
    assert_eq!(escape(&mut session), EscapeStep::ClosedEntry);
    assert_eq!(key(&mut session, "s"), KeyOutcome::EntryOpened);
    assert!(session.transform_entry().is_some());
}

/// Superseded by `multi-object-transform` (criteria 33 and 34): several objects
/// selected, R and S open the entries of the selection and change no tool.
#[test]
fn r_and_s_with_several_objects_selected_open_the_selection_entries() {
    let document = rect_document();
    let _ = document.create_rect(RectBounds {
        origin: pt(200.0, 20.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let mut session = session_of(&document);
    click(&mut session, pt(10.0, 50.0));
    session.pointer_hover(pt(200.0, 40.0), true, false);
    session.pointer_down(pt(200.0, 40.0), true);
    session.pointer_up(pt(200.0, 40.0), true, false);
    assert_eq!(session.selected_object_count(), 2);
    for k in ["r", "s"] {
        assert_eq!(key(&mut session, k), KeyOutcome::EntryOpened);
        assert_eq!(session.tool(), Tool::Select);
        assert_eq!(session.selected_object_count(), 2);
        session.cancel_transform_entry();
    }
}

/// Criterion 61: Delete and Backspace delete the selection in the Select
/// tool and the selected nodes in the Node tool; in the Pen tool nothing.
#[test]
fn delete_keys_delete_the_selection_in_select_and_node() {
    let mut session = session_of(&rect_document());
    click(&mut session, pt(10.0, 50.0));
    assert_eq!(key(&mut session, "Delete"), KeyOutcome::Deleted);
    assert_eq!(objects(&session).len(), 0, "nothing was written");

    let mut session = session_of(&rect_document());
    click(&mut session, pt(10.0, 50.0));
    assert_eq!(key(&mut session, "Backspace"), KeyOutcome::Deleted);
    assert_eq!(objects(&session).len(), 0, "nothing was written");

    let mut session = session_of(&path_document());
    session.set_tool(Tool::Node);
    click(&mut session, pt(10.0, 20.0));
    assert_eq!(key(&mut session, "Delete"), KeyOutcome::Deleted);
    let ObjectSnapshot::Path(path) = objects(&session).remove(0) else {
        panic!("a path");
    };
    assert_eq!(path.anchors.len(), 2, "the selected node is gone");

    let mut session = session_of(&rect_document());
    click(&mut session, pt(10.0, 50.0));
    session.set_tool(Tool::Pen);
    let _ = key(&mut session, "Delete");
    assert_eq!(objects(&session).len(), 1, "the Pen deletes nothing");
}

// ---------------------------------------------------------------------
// Split: criteria 50 to 52 through the Session
// ---------------------------------------------------------------------

/// Criteria 50 and 51: after Split one node is selected, and a drag from the
/// shared position moves that node only, with no click on empty canvas.
#[test]
fn split_selects_one_node_and_the_drag_moves_one_end() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(20.0, 0.0)),
        ],
        false,
    );
    let mut session = session_of(&document);
    session.set_tool(Tool::Node);
    click(&mut session, pt(10.0, 0.0));
    assert!(session.node_toolbar_state().can_split);
    session.split_selected();
    let state = session.node_toolbar_state();
    assert!(
        !state.can_split && !state.can_join,
        "one end node is selected"
    );
    assert!(state.can_convert_to_corner && state.can_delete);

    press(&mut session, pt(10.0, 0.0));
    session.pointer_hover(pt(10.0, 8.0), false, false);
    session.pointer_up(pt(10.0, 8.0), false, false);

    let ends: Vec<Point> = objects(&session)
        .into_iter()
        .map(|object| {
            let ObjectSnapshot::Path(path) = object else {
                panic!("a path");
            };
            // The end at the split: the last anchor of the first piece, the
            // first anchor of the second.
            if path.anchors.len() == 2 && path.anchors[0].point.x == 0.0 {
                path.anchors[1].point
            } else {
                path.anchors[0].point
            }
        })
        .collect();
    assert_eq!(ends.len(), 2);
    assert!(ends.contains(&pt(10.0, 0.0)), "one end stayed: {ends:?}");
    assert!(ends.contains(&pt(10.0, 8.0)), "the other moved: {ends:?}");
}

/// A window blur with no button down loses nothing: an unfinished Pen path
/// stays. A pointer lost during a Pen handle drag (button down) still
/// discards the path, as before.
#[test]
fn pointer_cancelled_keeps_a_pen_path_unless_a_button_was_down() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    click(&mut session, pt(10.0, 10.0));
    click(&mut session, pt(50.0, 10.0));
    session.pointer_cancelled();
    assert_eq!(
        session.pen_in_progress().map(<[_]>::len),
        Some(2),
        "no button was down: the path stays"
    );

    press(&mut session, pt(50.0, 40.0));
    session.pointer_hover(pt(70.0, 60.0), false, false);
    session.pointer_cancelled();
    assert!(
        session.pen_in_progress().is_none(),
        "button down during the drag: the path is discarded"
    );
    assert_eq!(objects(&session).len(), 0, "nothing was written");
}
