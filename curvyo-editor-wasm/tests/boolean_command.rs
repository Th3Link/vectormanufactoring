//! The boolean command in the editor session (`specs/0016-boolean-operations` criteria 1, 15 to
//! 19, 21 to 23, 28, 38): what the rail buttons show, one commit per operation, the selection
//! afterwards, refusals that change nothing and outline their offenders, and the compound path
//! in the Node tool.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, pack, unpack,
};
use curvyo_editor_wasm::{BooleanOutcome, DoubleClickHint, Session, Tool};
use curvyo_ui_core::{BooleanAvailability, BooleanOp, BooleanRefusal};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(document: &Document, x: f64, y: f64, side: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(side),
        height: Length::from_mm(side),
    })
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    session
}

fn click(session: &mut Session, at: Point, shift: bool) {
    session.pointer_hover(at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
}

/// Two overlapping squares of 20 mm, A at the origin (below) and B at (10, 10); returns the
/// session with both selected.
fn two_squares() -> (Session, NodeId, NodeId) {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 20.0);
    let b = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    click(&mut session, pt(30.0, 20.0), false); // B's right edge
    click(&mut session, pt(0.0, 10.0), true); // A's left edge
    assert_eq!(session.selected_object_count(), 2);
    let ids = unpack(3, &session.pack("0.1.0").unwrap())
        .unwrap()
        .object_ids();
    assert_eq!(ids.len(), 2);
    let _ = (a, b);
    (session, ids[0], ids[1])
}

fn json(session: &Session) -> Vec<u8> {
    unpack(3, &session.pack("0.1.0").unwrap())
        .unwrap()
        .export_json()
        .unwrap()
}

fn doc(session: &Session) -> Document {
    unpack(3, &session.pack("0.1.0").unwrap()).unwrap()
}

/// Criterion 1: the buttons are enabled only with the Select tool and two or more objects.
#[test]
fn the_buttons_follow_the_selection_and_the_tool() {
    let (mut session, _, _) = two_squares();
    assert_eq!(session.boolean_availability(), BooleanAvailability::Ready);
    session.set_tool(Tool::Node);
    assert_eq!(
        session.boolean_availability(),
        BooleanAvailability::NeedsTwo,
        "another tool: the selection is not drawn, so the buttons are dimmed"
    );
    session.set_tool(Tool::Select);
    click(&mut session, pt(500.0, 500.0), false);
    assert_eq!(
        session.boolean_availability(),
        BooleanAvailability::NeedsTwo
    );
}

/// Criteria 19 to 23, 28: the operands become one object that is the only selection; the tool
/// stays Select.
#[test]
fn an_operation_replaces_the_operands_and_selects_the_result() {
    let (mut session, _, _) = two_squares();
    let outcome = session.apply_boolean(BooleanOp::Union);
    assert_eq!(
        outcome,
        BooleanOutcome::Applied {
            operands: 2,
            compound: false
        }
    );
    let document = doc(&session);
    let ids = document.object_ids();
    assert_eq!(ids.len(), 1);
    let ObjectSnapshot::Path(path) = document.object(ids[0]).unwrap() else {
        panic!("a path")
    };
    assert!(!path.is_compound());
    assert_eq!(path.anchors.len(), 8, "two offset squares unite to 8 nodes");
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(session.tool(), Tool::Select);
}

/// A ring is one compound path object (criterion 20).
#[test]
fn a_difference_with_a_hole_is_one_compound_path() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    click(&mut session, pt(40.0, 20.0), false);
    click(&mut session, pt(10.0, 20.0), true);
    assert_eq!(
        session.apply_boolean(BooleanOp::Difference),
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
    let after = doc(&session);
    let ids = after.object_ids();
    assert_eq!(ids.len(), 1);
    assert!(after.path(ids[0]).unwrap().is_compound());
    // The Node tool says why it shows no node (criterion 38).
    session.set_tool(Tool::Node);
    assert!(session.node_toolbar_state().compound_only);
    session.set_tool(Tool::Select);
    session.set_tool(Tool::Node);
    click(&mut session, pt(0.0, 0.0), false);
    assert!(
        !session.node_toolbar_state().can_delete,
        "no node to delete"
    );
}

/// Criteria 15 and 18: an open path is refused; nothing changes, the offender is outlined in
/// red, the buttons stay usable and a second activation gives the same result.
#[test]
fn an_open_path_is_refused_and_outlined_without_changing_anything() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 20.0);
    let open = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(100.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(130.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(160.0, 0.0)),
        ],
        false,
    );
    let mut session = session_of(&document);
    click(&mut session, pt(0.0, 10.0), false);
    click(&mut session, pt(130.0, 20.0), true);
    assert_eq!(session.selected_object_count(), 2);
    assert_eq!(
        session.boolean_availability(),
        BooleanAvailability::OpenPaths { open: 1, of: 2 }
    );
    let before = json(&session);
    let plain = session.draw_list().triangle_count();

    let first = session.apply_boolean(BooleanOp::Union);
    assert_eq!(
        first,
        BooleanOutcome::Refused(BooleanRefusal::OpenPaths {
            offenders: vec![open],
            of: 2
        })
    );
    assert_eq!(json(&session), before, "nothing changed");
    assert_eq!(session.selected_object_count(), 2, "the selection stays");
    assert_eq!(session.tool(), Tool::Select);
    assert!(
        session.draw_list().triangle_count() > plain,
        "the offender is outlined"
    );
    assert_eq!(
        session.apply_boolean(BooleanOp::Union),
        first,
        "a second time, the same"
    );

    // The outline ends with the selection ...
    click(&mut session, pt(500.0, 500.0), false);
    click(&mut session, pt(0.0, 10.0), false);
    assert_eq!(session.selected_object_count(), 1);
    let one_selected = session.draw_list().triangle_count();
    click(&mut session, pt(0.0, 10.0), false);
    session.clear_boolean_refusal();
    assert_eq!(
        session.draw_list().triangle_count(),
        one_selected,
        "a changed selection shows no outline"
    );
}

/// The outline is not stored, selects nothing, and goes with the tool.
#[test]
fn the_refusal_outline_ends_with_the_selection_the_tool_and_the_host() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 20.0);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(100.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(130.0, 20.0)),
        ],
        false,
    );
    let mut session = session_of(&document);
    click(&mut session, pt(0.0, 10.0), false);
    click(&mut session, pt(115.0, 10.0), true);
    let plain = session.draw_list().triangle_count();
    let _ = session.apply_boolean(BooleanOp::Intersection);
    let outlined = session.draw_list().triangle_count();
    assert!(outlined > plain);
    session.clear_boolean_refusal();
    assert_eq!(
        session.draw_list().triangle_count(),
        plain,
        "the host cleared it"
    );
    let _ = session.apply_boolean(BooleanOp::Intersection);
    session.set_tool(Tool::Node);
    session.set_tool(Tool::Select);
    assert_eq!(
        session.draw_list().triangle_count(),
        plain,
        "a tool change cleared it"
    );
}

/// Criteria 16 and 17: an empty result is refused with a code and changes nothing.
#[test]
fn an_empty_result_is_refused() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 10.0);
    let _ = rect(&document, 100.0, 0.0, 10.0);
    let mut session = session_of(&document);
    click(&mut session, pt(0.0, 5.0), false);
    click(&mut session, pt(110.0, 5.0), true);
    let before = json(&session);
    assert_eq!(
        session.apply_boolean(BooleanOp::Intersection),
        BooleanOutcome::Refused(BooleanRefusal::Empty)
    );
    assert_eq!(json(&session), before);
    assert_eq!(session.selected_object_count(), 2);
}

/// The command is ignored outside the Select tool, and with fewer than two objects it is a
/// refusal that outlines nothing.
#[test]
fn the_command_is_ignored_outside_the_select_tool() {
    let (mut session, _, _) = two_squares();
    let before = json(&session);
    session.set_tool(Tool::Node);
    assert_eq!(
        session.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Ignored
    );
    assert_eq!(json(&session), before);
    session.set_tool(Tool::Select);
    click(&mut session, pt(500.0, 500.0), false);
    assert_eq!(
        session.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Refused(BooleanRefusal::NeedsTwo)
    );
}

/// Criterion 38: a double-click on a compound path stays in the Select tool and asks for the
/// sentence.
#[test]
fn a_double_click_on_a_compound_path_asks_for_the_sentence() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    click(&mut session, pt(40.0, 20.0), false);
    click(&mut session, pt(10.0, 20.0), true);
    let _ = session.apply_boolean(BooleanOp::Difference);
    // The stroke of the hole, away from every handle of the selection box.
    let at = pt(10.0, 15.0);
    click(&mut session, at, false);
    let hint = session.double_click_hint(at, false, false);
    assert_eq!(hint, DoubleClickHint::CompoundPath);
    assert_eq!(hint.code(), "compound_path");
    assert_eq!(session.tool(), Tool::Select);
}

/// The commit messages of a document, oldest first (the labels the undo slice keys on).
fn commit_labels(session: &Session) -> Vec<String> {
    let loro = loro::LoroDoc::new();
    loro.import(&doc(session).export_loro_snapshot().unwrap())
        .unwrap();
    let ids: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut changes: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&ids, &mut |change| {
        changes.push((
            change.lamport,
            change.message.map(|m| m.to_string()).unwrap_or_default(),
        ));
        std::ops::ControlFlow::Continue(())
    })
    .unwrap();
    changes.sort();
    changes.into_iter().map(|(_, label)| label).collect()
}

/// Criterion 28: each operation writes exactly one commit labelled `boolean_<op>`, and a
/// refusal writes none. (A small result: a big one is split into several Loro changes with the
/// same message, so the count of changes is no measure of operations there.)
#[test]
fn every_operation_writes_one_labelled_commit_and_a_refusal_none() {
    for (op, label) in [
        (BooleanOp::Union, "boolean_union"),
        (BooleanOp::Difference, "boolean_difference"),
        (BooleanOp::Intersection, "boolean_intersection"),
        (BooleanOp::Exclusion, "boolean_exclusion"),
        (BooleanOp::ReverseDifference, "boolean_reverse_difference"),
    ] {
        let (mut session, _, _) = two_squares();
        let before = commit_labels(&session);
        assert!(matches!(
            session.apply_boolean(op),
            BooleanOutcome::Applied { .. }
        ));
        let after = commit_labels(&session);
        assert_eq!(after.len(), before.len() + 1, "{label}: one commit");
        assert_eq!(after.last().map(String::as_str), Some(label));
    }

    // A refusal, and a call that is ignored, write nothing.
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 20.0);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(100.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(130.0, 20.0)),
        ],
        false,
    );
    let mut session = session_of(&document);
    click(&mut session, pt(0.0, 10.0), false);
    click(&mut session, pt(115.0, 10.0), true);
    let before = commit_labels(&session);
    assert!(matches!(
        session.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Refused(_)
    ));
    session.set_tool(Tool::Node);
    assert_eq!(
        session.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Ignored
    );
    assert_eq!(commit_labels(&session), before, "no commit");
}

/// Criterion 38: in the Node tool a compound path shows no node, handle or segment overlay, so
/// the frame is the artwork alone (the one the Select tool draws with nothing selected).
#[test]
fn the_node_tool_draws_no_overlay_for_a_compound_path() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    click(&mut session, pt(40.0, 20.0), false);
    click(&mut session, pt(10.0, 20.0), true);
    let _ = session.apply_boolean(BooleanOp::Difference);
    // The artwork alone: nothing selected in the Select tool.
    click(&mut session, pt(500.0, 500.0), false);
    let artwork = session.draw_list().triangle_count();
    click(&mut session, pt(0.0, 20.0), false);
    assert_eq!(session.selected_object_count(), 1);
    session.set_tool(Tool::Node);
    assert!(session.node_toolbar_state().compound_only);
    assert_eq!(session.draw_list().triangle_count(), artwork);
}

/// Copy of a ring made by a boolean through the typed move entry's Copy check (the other route to
/// `duplicate_objects` next to Ctrl-move): both compound paths keep both outlines, and every
/// anchor id in the document is unique.
#[test]
fn a_typed_copy_of_a_ring_keeps_every_outline_with_fresh_anchor_ids() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    click(&mut session, pt(40.0, 20.0), false);
    click(&mut session, pt(10.0, 20.0), true);
    assert!(matches!(
        session.apply_boolean(BooleanOp::Difference),
        BooleanOutcome::Applied { compound: true, .. }
    ));
    let opened = session.key_down(curvyo_editor_wasm::KeyInput {
        key: "m",
        ..curvyo_editor_wasm::KeyInput::default()
    });
    assert_ne!(opened, curvyo_editor_wasm::KeyOutcome::Ignored);
    assert!(session.move_entry().is_some(), "the move entry is open");
    session.commit_move_entry(
        "60",
        "0",
        curvyo_ui_core::MoveEntryMode {
            absolute: false,
            copy: true,
        },
    );
    let after = doc(&session);
    let rings: Vec<_> = after
        .object_ids()
        .into_iter()
        .filter_map(|id| after.path(id))
        .filter(curvyo_document_core::PathSnapshot::is_compound)
        .collect();
    assert_eq!(rings.len(), 2, "the original and the copy");
    let mut ids = std::collections::HashSet::new();
    for ring in &rings {
        assert_eq!(ring.extra_subpaths.len(), 1, "both outlines");
        assert_eq!(ring.all_anchors().count(), 8);
        ids.extend(ring.all_anchors().map(|anchor| anchor.id));
    }
    assert_eq!(ids.len(), 16, "no anchor id is shared");
}
