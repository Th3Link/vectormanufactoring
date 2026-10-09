//! Combine and Break apart in the editor session (`specs/0035-combine-and-break-apart` criteria
//! 1, 3, 7, 9, 10, 12, 13, 15, 16, 17, 18, 19 golden): what the Path card shows, one commit per
//! command with its pinned label, the selection afterwards, refusals that change nothing and
//! outline their offenders, and the round trip through the file.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    Document, Length, NodeId, ObjectSnapshot, Point, RectBounds, pack, unpack,
};
use curvyo_editor_wasm::{BreakApartOutcome, CombineOutcome, Session, Tool};
use curvyo_ui_core::{BooleanAvailability, BreakApartRefusal, CombineRefusal};

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

/// A marquee over everything, from outside the artwork.
fn select_all(session: &mut Session, to: Point) {
    let from = pt(-200.0, -200.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

fn doc(session: &Session) -> Document {
    unpack(3, &session.pack("0.1.0").unwrap()).unwrap()
}

fn json(session: &Session) -> Vec<u8> {
    doc(session).export_json().unwrap()
}

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

fn paths(session: &Session) -> Vec<curvyo_document_core::PathSnapshot> {
    let document = doc(session);
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| match document.object(id) {
            Some(ObjectSnapshot::Path(path)) => Some(path),
            _ => None,
        })
        .collect()
}

/// A plate (40 mm), a hole (20 mm) and an island (10 mm) inside it, all selected.
fn plate_hole_island() -> Session {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let _ = rect(&document, 15.0, 15.0, 10.0);
    let mut session = session_of(&document);
    select_all(&mut session, pt(60.0, 60.0));
    assert_eq!(session.selected_object_count(), 3);
    session
}

/// Criterion 1: the buttons follow the selection and the tool.
#[test]
fn the_path_card_follows_the_selection_and_the_tool() {
    let mut session = plate_hole_island();
    let ready = session.path_availability();
    assert_eq!(ready.combine, BooleanAvailability::Ready);
    assert!(!ready.break_apart, "no compound path selected");
    session.set_tool(Tool::Node);
    let other = session.path_availability();
    assert_eq!(other.combine, BooleanAvailability::NeedsTwo);
    assert!(!other.break_apart);
    session.set_tool(Tool::Select);
    let _ = session.apply_combine();
    let one = session.path_availability();
    assert_eq!(one.combine, BooleanAvailability::NeedsTwo);
    assert!(one.break_apart, "the compound path enables Break apart");
}

/// Criteria 3, 7, 15, 17, 18: Combine, Break apart, Combine again.
#[test]
fn combine_and_break_apart_round_trip_with_one_commit_each() {
    let mut session = plate_hole_island();
    let before = commit_labels(&session).len();

    let outcome = session.apply_combine();
    assert_eq!(
        outcome,
        CombineOutcome::Applied {
            operands: 3,
            holes: 1,
            styles_differ: false
        }
    );
    assert_eq!(session.tool(), Tool::Select);
    assert_eq!(session.selected_object_count(), 1);
    let labels = commit_labels(&session);
    assert_eq!(labels.len(), before + 1, "one commit");
    assert_eq!(labels.last().unwrap(), "combine_paths");
    let combined = paths(&session);
    assert_eq!(combined.len(), 1);
    assert_eq!(combined[0].extra_subpaths.len(), 2, "three outlines in all");
    let outlines_before: Vec<Vec<Point>> = combined[0]
        .subpaths()
        .map(|s| s.anchors.iter().map(|a| a.point).collect())
        .collect();

    let outcome = session.apply_break_apart();
    assert_eq!(
        outcome,
        BreakApartOutcome::Applied {
            compounds: 1,
            pieces: 2,
            with_holes: true,
            left_alone: 0
        }
    );
    let labels = commit_labels(&session);
    assert_eq!(labels.len(), before + 2);
    assert_eq!(labels.last().unwrap(), "break_apart");
    assert_eq!(
        session.selected_object_count(),
        2,
        "all pieces are selected"
    );
    let pieces = paths(&session);
    assert_eq!(pieces.len(), 2);
    assert!(pieces[0].is_compound() && !pieces[1].is_compound());

    // Criterion 18: Break apart then Combine gives one compound path with the same outlines.
    assert_eq!(
        session.apply_combine(),
        CombineOutcome::Applied {
            operands: 2,
            holes: 1,
            styles_differ: false
        }
    );
    let again = paths(&session);
    assert_eq!(again.len(), 1);
    let outlines_after: Vec<Vec<Point>> = again[0]
        .subpaths()
        .map(|s| s.anchors.iter().map(|a| a.point).collect())
        .collect();
    assert_eq!(outlines_after, outlines_before);
}

/// Criteria 9, 10, 12: a refusal changes nothing, outlines its offenders, and repeats.
#[test]
fn touching_shapes_are_refused_and_outlined_without_changing_anything() {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 20.0);
    let b = rect(&document, 10.0, 10.0, 20.0);
    let _far = rect(&document, 100.0, 0.0, 20.0);
    let mut session = session_of(&document);
    select_all(&mut session, pt(200.0, 200.0));
    assert_eq!(session.selected_object_count(), 3);
    let before = json(&session);
    let labels = commit_labels(&session);
    let plain = session.draw_list().triangle_count();

    let first = session.apply_combine();
    assert_eq!(
        first,
        CombineOutcome::Refused(CombineRefusal::Touching {
            offenders: vec![a, b],
            of: 3
        })
    );
    assert_eq!(json(&session), before, "nothing changed");
    assert_eq!(commit_labels(&session), labels, "no commit");
    assert_eq!(session.selected_object_count(), 3);
    assert_eq!(session.tool(), Tool::Select);
    assert!(session.draw_list().triangle_count() > plain, "outlined");
    assert_eq!(session.apply_combine(), first, "a second press: the same");

    session.clear_boolean_refusal();
    assert_eq!(session.draw_list().triangle_count(), plain);
}

/// Criterion 16: a ring alone is one piece; the refusal draws no outline and writes nothing.
#[test]
fn a_ring_alone_is_one_piece_and_the_defensive_refusal_needs_no_compound() {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 10.0, 10.0, 20.0);
    let mut session = session_of(&document);
    select_all(&mut session, pt(60.0, 60.0));
    let _ = session.apply_combine();
    let ids = doc(&session).object_ids();
    let before = json(&session);
    let labels = commit_labels(&session);
    let plain = session.draw_list().triangle_count();

    assert_eq!(
        session.apply_break_apart(),
        BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds: 1 })
    );
    assert_eq!(json(&session), before);
    assert_eq!(commit_labels(&session), labels);
    assert_eq!(doc(&session).object_ids(), ids, "the object keeps its ids");
    assert_eq!(session.selected_object_count(), 1);
    assert_eq!(session.draw_list().triangle_count(), plain, "no outline");

    click(&mut session, pt(500.0, 500.0), false);
    assert_eq!(
        session.apply_break_apart(),
        BreakApartOutcome::Refused(BreakApartRefusal::NoCompound)
    );
}

/// The commands are ignored outside the Select tool.
#[test]
fn the_commands_are_ignored_outside_the_select_tool() {
    let mut session = plate_hole_island();
    session.set_tool(Tool::Node);
    assert_eq!(session.apply_combine(), CombineOutcome::Ignored);
    assert_eq!(session.apply_break_apart(), BreakApartOutcome::Ignored);
}

/// Criterion 19, golden file: the ring with an island, saved after Combine, opens and breaks
/// apart into the ring and the disc. Regenerate with
/// `CURVYO_WRITE_FIXTURES=1 cargo test -p curvyo-editor-wasm --test combine_break_apart`.
#[test]
fn the_golden_ring_with_an_island_breaks_apart_into_two_objects() {
    const GOLDEN: &str = "combine_ring_island.curvyo";
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(GOLDEN);
    if std::env::var_os("CURVYO_WRITE_FIXTURES").is_some() {
        let document = Document::new(1);
        let _ = rect(&document, 0.0, 0.0, 20.0);
        let _ = rect(&document, 5.0, 5.0, 10.0);
        let _ = rect(&document, 7.5, 7.5, 5.0);
        let mut session = session_of(&document);
        select_all(&mut session, pt(60.0, 60.0));
        assert!(matches!(
            session.apply_combine(),
            CombineOutcome::Applied { .. }
        ));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, session.pack("0.1.0").unwrap()).unwrap();
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
    assert_eq!(doc(&session).object_ids().len(), 1);
    select_all(&mut session, pt(60.0, 60.0));
    assert_eq!(
        session.apply_break_apart(),
        BreakApartOutcome::Applied {
            compounds: 1,
            pieces: 2,
            with_holes: true,
            left_alone: 0
        }
    );
    let pieces = paths(&session);
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].extra_subpaths.len(), 1, "the ring with its hole");
    assert_eq!(pieces[1].anchors.len(), 4, "the island");
}
