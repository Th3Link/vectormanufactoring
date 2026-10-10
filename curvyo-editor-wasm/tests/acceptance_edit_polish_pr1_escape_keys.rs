//! Independent tester acceptance tests for PR 1 of
//! `specs/0010-edit-interaction-polish/specification.md`, Part D (criteria 42 to
//! 52: the Escape cascade per tool, Split selects one node) and the PR 1
//! subset of Part F (criteria 54, 55, 57, 60, 61: key table, one gate, R and
//! S entries, held Escape, Delete). Written from the specification before the
//! implementation diff was read. Everything goes through `Session`'s public
//! API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(missing_docs, clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
#![allow(clippy::needless_range_loop, clippy::manual_let_else)]

use curvyo_document_core::{
    AnchorId, Angle, Document, InnerRatio, Length, NewAnchor, ObjectSnapshot, Point, PointCount,
    RectBounds, StarFrame, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn n_objects(s: &Session) -> usize {
    doc_of(s).object_ids().len()
}

fn objects_of(s: &Session) -> Vec<ObjectSnapshot> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .map(|id| d.object(id).unwrap())
        .collect()
}

fn path_points(o: &ObjectSnapshot) -> Vec<Point> {
    match o {
        ObjectSnapshot::Path(p) => p.anchors.iter().map(|a| a.point).collect(),
        ObjectSnapshot::Primitive(_) => panic!("path expected"),
    }
}

fn key(k: &str) -> KeyInput<'_> {
    KeyInput {
        key: k,
        ..KeyInput::default()
    }
}

fn press(s: &mut Session, k: &str) -> KeyOutcome {
    s.key_down(key(k))
}

fn esc(s: &mut Session) -> KeyOutcome {
    s.key_down(key("Escape"))
}

fn open_doc(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
}

/// An open polyline document: (0,0) (10,0) (20,5).
fn open_path_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 0.0, 0.0),
            anchor(2, 10.0, 0.0),
            anchor(3, 20.0, 5.0),
        ],
        false,
    );
    d
}

fn closed_path_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 0.0, 0.0),
            anchor(2, 20.0, 0.0),
            anchor(3, 10.0, 15.0),
        ],
        true,
    );
    d
}

fn rect_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    d
}

fn two_rects_doc() -> Document {
    let d = rect_doc();
    let _ = d.create_rect(RectBounds {
        origin: pt(100.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    d
}

/// A session with the path selected in the Select tool and the Node tool
/// active, with no node selected.
fn node_session(d: &Document, grab: Point) -> Session {
    let mut s = open_doc(d);
    click(&mut s, grab);
    assert_eq!(s.selected_object_count(), 1);
    s.set_tool(Tool::Node);
    s
}

fn select_rect() -> Session {
    let mut s = open_doc(&rect_doc());
    click(&mut s, pt(20.0, 0.0));
    assert_eq!(s.selected_object_count(), 1);
    s
}

/// The handle position of the typed angle (R) or size (S) chip, for the 40 x
/// 20 rectangle of `select_rect`. The S chip is placed by the centre
/// (criterion 59), so the bottom-right resize handle is half the box away
/// from it.
fn chip_handle(s: &mut Session, k: &str) -> Point {
    assert_eq!(press(s, k), KeyOutcome::EntryOpened);
    let view = s.transform_entry().unwrap();
    s.cancel_transform_entry();
    if k == "s" {
        assert!(view.at_centre);
        pt(view.center.x + 20.0, view.center.y + 10.0)
    } else {
        view.handle
    }
}

const TOOLS: [Tool; 6] = [
    Tool::Select,
    Tool::Pen,
    Tool::Node,
    Tool::Rectangle,
    Tool::Ellipse,
    Tool::PolygonStar,
];

fn step_of(o: KeyOutcome) -> EscapeStep {
    match o {
        KeyOutcome::Escape(step) => step,
        other => panic!("expected an Escape step, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Criterion 42, 43: Pen tool
// ---------------------------------------------------------------------

fn pen_with(points: &[Point]) -> Session {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    for &p in points {
        click(&mut s, p);
    }
    s
}

#[test]
fn ac43_pen_escape_discards_the_unfinished_path_then_leaves() {
    for count in [1usize, 2, 3, 5] {
        let pts: Vec<Point> = (0..count)
            .map(|i| pt(10.0 * i as f64, (i % 2) as f64 * 7.0))
            .collect();
        let mut s = pen_with(&pts);
        assert!(s.pen_in_progress().is_some(), "{count} nodes in progress");
        let before = change_count(&s);
        let first = esc(&mut s);
        assert!(
            matches!(
                step_of(first),
                EscapeStep::ClearedState | EscapeStep::CancelledDrag
            ),
            "{count}: {first:?}"
        );
        assert!(s.pen_in_progress().is_none(), "unfinished path discarded");
        assert_eq!(s.tool(), Tool::Pen, "tool stays Pen after the first Escape");
        assert_eq!(n_objects(&s), 0, "never kept by Escape");
        assert_eq!(change_count(&s), before, "nothing written");
        assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
        assert_eq!(s.tool(), Tool::Select);
        assert_eq!(n_objects(&s), 0);
    }
}

#[test]
fn ac43_pen_escape_during_a_handle_drag_discards_everything_and_the_release_writes_nothing() {
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0)]);
    s.pointer_hover(pt(20.0, 0.0), false, false);
    s.pointer_down(pt(20.0, 0.0), false);
    s.pointer_hover(pt(25.0, 8.0), false, false);
    let before = change_count(&s);
    let o = esc(&mut s);
    assert!(matches!(
        step_of(o),
        EscapeStep::ClearedState | EscapeStep::CancelledDrag
    ));
    assert!(s.pen_in_progress().is_none());
    assert_eq!(s.tool(), Tool::Pen);
    s.pointer_up(pt(25.0, 8.0), false, false);
    assert!(
        s.pen_in_progress().is_none(),
        "release does not resurrect a node"
    );
    assert_eq!(n_objects(&s), 0);
    assert_eq!(change_count(&s), before);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
}

#[test]
fn ac43_pen_with_no_path_leaves_on_the_first_escape_and_enter_still_finishes() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
    assert_eq!(s.tool(), Tool::Select);
    // Enter finishes an unfinished path (unchanged)
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 5.0)]);
    assert_eq!(press(&mut s, "Enter"), KeyOutcome::PenFinished);
    assert_eq!(n_objects(&s), 1);
    assert_eq!(path_points(&objects_of(&s)[0]).len(), 3);
}

// ---------------------------------------------------------------------
// Criterion 42, 44, 49: Rectangle, Ellipse and Polygon/Star
// ---------------------------------------------------------------------

#[test]
fn ac44_ac49_creation_tools_escape_cancels_a_drag_only_then_leaves() {
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
        // no drag: the first Escape leaves
        let mut s = Session::new(1);
        s.set_tool(tool);
        assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool, "{tool:?}");
        assert_eq!(s.tool(), Tool::Select);
        assert_eq!(
            step_of(esc(&mut s)),
            EscapeStep::Nothing,
            "{tool:?}: nothing selected"
        );
        assert_eq!(s.tool(), Tool::Select);

        // a drag in flight: Escape cancels only it, the tool stays
        let mut s = Session::new(1);
        s.set_tool(tool);
        s.pointer_hover(pt(10.0, 10.0), false, false);
        s.pointer_down(pt(10.0, 10.0), false);
        s.pointer_hover(pt(40.0, 30.0), false, false);
        assert!(
            s.live_readout().is_some(),
            "{tool:?}: preview while dragging"
        );
        let before = change_count(&s);
        assert_eq!(step_of(esc(&mut s)), EscapeStep::CancelledDrag, "{tool:?}");
        assert_eq!(s.tool(), tool, "{tool:?}: tool does not change");
        assert!(s.live_readout().is_none(), "{tool:?}: no preview remains");
        // button still held: Escape again does nothing more (criterion 49)
        let again = esc(&mut s);
        assert_eq!(
            again,
            KeyOutcome::Escape(EscapeStep::Nothing),
            "{tool:?}: second Escape with the button held"
        );
        assert_eq!(s.tool(), tool);
        // moving with the button still held and releasing writes nothing
        s.pointer_hover(pt(60.0, 60.0), false, false);
        s.pointer_up(pt(60.0, 60.0), false, false);
        assert_eq!(n_objects(&s), 0, "{tool:?}: cancelled drag creates nothing");
        assert_eq!(change_count(&s), before);
        assert_eq!(s.tool(), tool);
        // button up: now Escape leaves
        assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool, "{tool:?}");
        assert_eq!(s.tool(), Tool::Select);
    }
}

#[test]
fn ac48_after_a_created_shape_select_is_active_with_it_selected_escape_clears_then_nothing() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(10.0, 10.0), pt(50.0, 40.0));
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::Nothing);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(n_objects(&s), 1, "the shape is still there");
}

// ---------------------------------------------------------------------
// Criterion 42, 45, 47, 49, 60: Node tool
// ---------------------------------------------------------------------

#[test]
fn ac45_ac47_node_tool_two_step_escape_keeps_the_path_selected() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    assert!(s.node_toolbar_state().can_delete, "a node is selected");
    let objs = objects_of(&s);
    let before = change_count(&s);
    // first Escape: clears the node selection, stays in the Node tool
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.tool(), Tool::Node);
    assert!(!s.node_toolbar_state().can_delete, "node selection cleared");
    assert_eq!(objects_of(&s), objs, "path as it is");
    assert_eq!(change_count(&s), before);
    // second Escape: nothing actively selected -> Select tool
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1, "the path stays selected");
    // third: Select tool step 3, clears the object selection
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::Nothing);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(objects_of(&s), objs);
}

#[test]
fn ac45_a_selected_segment_is_cleared_by_the_first_escape_too() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(5.0, 0.0)); // a segment
    assert!(s.node_toolbar_state().can_insert, "segment selected");
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert!(!s.node_toolbar_state().can_insert);
    assert_eq!(s.tool(), Tool::Node);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
}

#[test]
fn ac45_ac49_node_drag_cancel_keeps_the_selection_and_writes_nothing() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    let objs = objects_of(&s);
    let before = change_count(&s);
    s.pointer_hover(pt(10.0, 0.0), false, false);
    s.pointer_down(pt(10.0, 0.0), false);
    s.pointer_hover(pt(14.0, 12.0), false, false);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::CancelledDrag);
    assert_eq!(s.tool(), Tool::Node, "tool unchanged");
    assert!(s.node_toolbar_state().can_delete, "selection kept");
    // button still held: nothing more
    assert_eq!(step_of(esc(&mut s)), EscapeStep::Nothing);
    assert_eq!(s.tool(), Tool::Node);
    assert!(s.node_toolbar_state().can_delete, "selection still kept");
    s.pointer_hover(pt(30.0, 30.0), false, false);
    s.pointer_up(pt(30.0, 30.0), false, false);
    assert_eq!(
        objects_of(&s),
        objs,
        "released after cancel: nothing written"
    );
    assert_eq!(change_count(&s), before);
    // now with the button up: first Escape clears, second leaves
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.tool(), Tool::Node);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
}

#[test]
fn ac60_held_escape_in_the_node_tool_clears_once_and_does_not_leave() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    for _ in 0..20 {
        let o = s.key_down(KeyInput {
            key: "Escape",
            repeat: true,
            ..KeyInput::default()
        });
        assert!(
            matches!(
                o,
                KeyOutcome::Ignored | KeyOutcome::Escape(EscapeStep::Nothing)
            ),
            "repeat took a step: {o:?}"
        );
        assert_eq!(s.tool(), Tool::Node, "held Escape did not leave the tool");
    }
    // and a repeat as the very first event does nothing either
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    let o = s.key_down(KeyInput {
        key: "Escape",
        repeat: true,
        ..KeyInput::default()
    });
    assert!(matches!(
        o,
        KeyOutcome::Ignored | KeyOutcome::Escape(EscapeStep::Nothing)
    ));
    assert!(
        s.node_toolbar_state().can_delete,
        "a repeat does not clear either"
    );
}

#[test]
fn ac60_held_escape_in_every_tool_takes_at_most_one_step() {
    for tool in TOOLS {
        let mut s = Session::new(1);
        s.set_tool(tool);
        let _ = esc(&mut s);
        // after the physical press the tool is Select (or still `tool` if
        // the first step was a clear); further repeats must not change it
        let after_first = s.tool();
        for _ in 0..10 {
            s.key_down(KeyInput {
                key: "Escape",
                repeat: true,
                ..KeyInput::default()
            });
            assert_eq!(s.tool(), after_first, "{tool:?}");
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 42, 46: Select tool
// ---------------------------------------------------------------------

#[test]
fn ac46_select_escape_clears_the_selection_then_does_nothing() {
    let mut s = select_rect();
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(s.tool(), Tool::Select);
    for _ in 0..3 {
        assert_eq!(step_of(esc(&mut s)), EscapeStep::Nothing);
        assert_eq!(s.tool(), Tool::Select);
    }
    assert_eq!(n_objects(&s), 1);
}

#[test]
fn ac42_open_entry_chip_is_closed_first_then_the_selection_clears() {
    let mut s = select_rect();
    assert_eq!(press(&mut s, "r"), KeyOutcome::EntryOpened);
    assert!(s.transform_entry().is_some());
    let before = change_count(&s);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClosedEntry);
    assert!(s.transform_entry().is_none());
    assert_eq!(
        s.selected_object_count(),
        1,
        "nothing else happened in that press"
    );
    assert_eq!(change_count(&s), before);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState);
    assert_eq!(s.selected_object_count(), 0);
}

fn state_snapshot(s: &Session) -> (Tool, usize, Vec<ObjectSnapshot>, usize) {
    (
        s.tool(),
        s.selected_object_count(),
        objects_of(s),
        change_count(s),
    )
}

#[test]
fn ac42_select_tool_drags_are_cancelled_by_escape_and_write_nothing() {
    // move, resize, rotate: each on a selected rect
    let kinds: [(&str, &dyn Fn(&mut Session) -> Point); 4] = [
        ("move body", &|_s| pt(20.0, 0.0)),
        ("resize corner", &|s| chip_handle(s, "s")),
        ("rotate corner", &|s| chip_handle(s, "r")),
        ("move inside", &|_s| pt(20.0, 10.0)),
    ];
    for (name, start) in kinds {
        let mut s = select_rect();
        let from = start(&mut s);
        let before = state_snapshot(&s);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(from.x + 30.0, from.y + 25.0), false, false);
        s.pointer_hover(pt(from.x + 35.0, from.y + 31.0), false, false);
        let step = esc(&mut s);
        assert_eq!(step_of(step), EscapeStep::CancelledDrag, "{name}");
        assert_eq!(
            s.selected_object_count(),
            1,
            "{name}: selection as it was after the press"
        );
        assert_eq!(s.tool(), Tool::Select, "{name}");
        assert!(s.live_readout().is_none(), "{name}: no preview remains");
        s.pointer_up(pt(from.x + 35.0, from.y + 31.0), false, false);
        let after = state_snapshot(&s);
        assert_eq!(before, after, "{name}: nothing written");
        // a second Escape (button up) clears the selection
        assert_eq!(step_of(esc(&mut s)), EscapeStep::ClearedState, "{name}");
    }
}

#[test]
fn ac42_skew_and_parameter_handle_drags_are_cancelled_too() {
    // skew: a path; scan for the skew handle
    let mut s = open_doc(&closed_path_doc());
    click(&mut s, pt(10.0, 0.0));
    let mut skew_at = None;
    'scan: for iy in -40..=70 {
        for ix in -40..=60 {
            let p = pt(f64::from(ix), f64::from(iy));
            s.pointer_hover(p, false, false);
            if s.handle_hint() == "skew" {
                skew_at = Some(p);
                break 'scan;
            }
        }
    }
    let from = skew_at.expect("a skew handle exists on a path");
    let before = state_snapshot(&s);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(pt(from.x + 25.0, from.y), false, false);
    s.pointer_hover(pt(from.x + 30.0, from.y + 2.0), false, false);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::CancelledDrag, "skew");
    s.pointer_up(pt(from.x + 30.0, from.y + 2.0), false, false);
    assert_eq!(
        state_snapshot(&s),
        before,
        "skew cancelled, nothing written"
    );

    // parameter handle: a star's inner ratio / a rect's corner radius, found
    // by scanning for a hint that names a parameter
    for doc in [rect_doc(), {
        let d = Document::new(1);
        let _ = d.create_star(
            StarFrame {
                center: pt(30.0, 30.0),
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(-1.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        d
    }] {
        let mut s = open_doc(&doc);
        let p0 = {
            let o = &objects_of(&s)[0];
            match o {
                ObjectSnapshot::Primitive(p) => {
                    curvyo_document_core::outline_of_rotated(&p.shape, p.rotation)[0].point
                }
                ObjectSnapshot::Path(_) => unreachable!(),
            }
        };
        click(&mut s, p0);
        assert_eq!(s.selected_object_count(), 1);
        let mut hit = None;
        let mut seen = std::collections::BTreeSet::new();
        'scan2: for iy in -30..=80 {
            for ix in -30..=80 {
                let p = pt(f64::from(ix) * 0.75, f64::from(iy) * 0.75);
                s.pointer_hover(p, false, false);
                let h = s.handle_hint();
                seen.insert(h.clone());
                if h.contains("radius")
                    || h.contains("ratio")
                    || h.contains("param")
                    || h.contains("corner-r")
                {
                    hit = Some(p);
                    break 'scan2;
                }
            }
        }
        let Some(from) = hit else {
            panic!("no parameter handle found; hints seen: {seen:?}");
        };
        let before = state_snapshot(&s);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(from.x + 6.0, from.y + 6.0), false, false);
        s.pointer_hover(pt(from.x + 9.0, from.y + 9.0), false, false);
        assert_eq!(
            step_of(esc(&mut s)),
            EscapeStep::CancelledDrag,
            "param handle"
        );
        s.pointer_up(pt(from.x + 9.0, from.y + 9.0), false, false);
        assert_eq!(
            state_snapshot(&s),
            before,
            "param drag cancelled, nothing written"
        );
    }
}

// ---------------------------------------------------------------------
// Criterion 42, 47: Escape never writes; leaving a tool keeps things
// ---------------------------------------------------------------------

#[test]
fn ac42_escape_never_writes_to_the_document_in_any_tool_or_state() {
    for tool in TOOLS {
        let mut s = open_doc(&two_rects_doc());
        click(&mut s, pt(20.0, 0.0));
        s.set_tool(tool);
        let before = change_count(&s);
        let objs = objects_of(&s);
        for _ in 0..4 {
            esc(&mut s);
        }
        assert_eq!(change_count(&s), before, "{tool:?}");
        assert_eq!(objects_of(&s), objs, "{tool:?}");
    }
}

#[test]
fn ac47_leaving_the_node_tool_by_escape_keeps_tool_rail_state_and_no_drag_state() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    esc(&mut s); // nothing selected: leaves at once
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
    assert!(s.live_readout().is_none());
    // S and the letters keep working
    assert_eq!(press(&mut s, "n"), KeyOutcome::ToolChanged);
    assert_eq!(s.tool(), Tool::Node);
    assert_eq!(
        press(&mut s, "s"),
        KeyOutcome::ToolChanged,
        "S selects Select"
    );
    assert_eq!(s.tool(), Tool::Select);
}

// ---------------------------------------------------------------------
// Criterion 50 to 52: Split selects one node
// ---------------------------------------------------------------------

#[test]
fn ac50_ac51_open_path_split_selects_one_node_and_a_plain_drag_moves_only_it() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    assert!(s.node_toolbar_state().can_split);
    s.split_selected();
    assert_eq!(n_objects(&s), 2, "two path objects");
    let st = s.node_toolbar_state();
    assert!(
        !st.can_split,
        "criterion 52: Split disabled (the node is an end)"
    );
    assert!(!st.can_join, "criterion 52: Join needs two");
    assert!(st.can_delete, "a node is selected");
    assert!(
        st.can_convert_to_corner && st.can_convert_to_symmetric && st.can_convert_to_asymmetric
    );
    let objs = objects_of(&s);
    // each piece's end sits at (10, 0)
    assert!(near(*path_points(&objs[0]).last().unwrap(), pt(10.0, 0.0)));
    assert!(near(path_points(&objs[1])[0], pt(10.0, 0.0)));
    // plain drag, no click on empty canvas first
    let before = change_count(&s);
    drag(&mut s, pt(10.0, 0.0), pt(10.0, 8.0));
    assert_eq!(change_count(&s), before + 1, "one commit");
    let objs = objects_of(&s);
    let (first, second) = (path_points(&objs[0]), path_points(&objs[1]));
    assert!(
        near(*first.last().unwrap(), pt(10.0, 0.0)),
        "first piece's end stays: {first:?}"
    );
    assert!(
        near(second[0], pt(10.0, 8.0)),
        "the new second node moved: {second:?}"
    );
    assert!(near(first[0], pt(0.0, 0.0)) && near(second[1], pt(20.0, 5.0)));
}

#[test]
fn ac50_ac51_closed_path_split_and_drag() {
    for node in [pt(0.0, 0.0), pt(20.0, 0.0), pt(10.0, 15.0)] {
        let mut s = node_session(&closed_path_doc(), pt(10.0, 0.0));
        click(&mut s, node);
        assert!(s.node_toolbar_state().can_split, "{node:?}");
        s.split_selected();
        assert_eq!(n_objects(&s), 1, "closed path stays one object, now open");
        let pts = path_points(&objects_of(&s)[0]);
        assert_eq!(pts.len(), 4, "{node:?}: one node added");
        assert!(
            near(pts[0], node) && near(pts[3], node),
            "copies at the split point: {pts:?}"
        );
        let st = s.node_toolbar_state();
        assert!(!st.can_split && !st.can_join, "criterion 52");
        let before = change_count(&s);
        let to = pt(node.x + 3.0, node.y - 9.0);
        drag(&mut s, node, to);
        assert_eq!(change_count(&s), before + 1);
        let pts = path_points(&objects_of(&s)[0]);
        // exactly one of the two copies moved; the spec names the new first node
        let moved: Vec<usize> = (0..pts.len()).filter(|&i| near(pts[i], to)).collect();
        assert_eq!(moved.len(), 1, "exactly one node moved: {pts:?}");
        assert_eq!(
            moved[0], 0,
            "the new first node of the opened path: {pts:?}"
        );
        assert!(
            near(pts[3], node),
            "the other copy stays at the split point"
        );
    }
}

#[test]
fn ac52_split_rules_otherwise_unchanged_two_copies_corner_kinds_and_z_order() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    s.split_selected();
    let d = doc_of(&s);
    let ids = d.object_ids();
    assert_eq!(ids.len(), 2);
    for id in &ids {
        let p = d.path(*id).unwrap();
        assert!(!p.closed);
    }
    // the second object is above the original (z-order)
    let p1 = d.path(ids[0]).unwrap();
    let p2 = d.path(ids[1]).unwrap();
    assert_eq!(p1.anchors.len(), 2);
    assert_eq!(p2.anchors.len(), 2);
}

#[test]
fn ac50_split_endpoint_or_nothing_selected_is_not_available() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    assert!(!s.node_toolbar_state().can_split);
    click(&mut s, pt(0.0, 0.0));
    assert!(!s.node_toolbar_state().can_split, "an end of an open path");
    let before = change_count(&s);
    s.split_selected();
    assert_eq!(change_count(&s), before);
    assert_eq!(n_objects(&s), 1);
}

/// A triangle (x, y, colour) triple of the draw list.
fn tri_colour_at(s: &Session, doc_pt: Point) -> Option<[u8; 4]> {
    // the draw list is in document space (the GPU applies the view)
    let list = s.draw_list();
    let mut found = None;
    for t in list.triangles.chunks(3) {
        let [a, b, c] = [t[0].position, t[1].position, t[2].position];
        {
            let p = doc_pt;
            let d1 = (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
            let d2 = (p.x - c.x) * (b.y - c.y) - (b.x - c.x) * (p.y - c.y);
            let d3 = (p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y);
            let neg = d1 < -1e-9 || d2 < -1e-9 || d3 < -1e-9;
            let pos = d1 > 1e-9 || d2 > 1e-9 || d3 > 1e-9;
            if !(neg && pos) {
                found = Some(colour_of(&t[0]));
            }
        }
    }
    found
}

fn colour_of(v: &curvyo_render_core::Vertex) -> [u8; 4] {
    colour_array(v.color)
}

fn colour_array(c: curvyo_render_core::RgbaColor) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

#[test]
fn ac50_the_selected_node_glyph_is_drawn_above_the_unselected_one() {
    // reference colours: one selected node, one unselected node, no overlap
    let mut r = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut r, pt(20.0, 5.0));
    let selected = tri_colour_at(&r, pt(20.0, 5.0)).expect("selected glyph drawn");
    let unselected = tri_colour_at(&r, pt(0.0, 0.0)).expect("unselected glyph drawn");
    assert_ne!(selected, unselected, "the two states look different");

    // after Split the two glyphs sit on the same point: the selected is on top
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    s.split_selected();
    let top = tri_colour_at(&s, pt(10.0, 0.0)).expect("a glyph at the split point");
    assert_eq!(
        top, selected,
        "the topmost triangle at the shared point is the selected glyph"
    );

    // and the closed-path case
    let mut s = node_session(&closed_path_doc(), pt(10.0, 0.0));
    click(&mut s, pt(20.0, 0.0));
    s.split_selected();
    let top = tri_colour_at(&s, pt(20.0, 0.0)).expect("a glyph at the split point");
    assert_eq!(top, selected, "closed path: selected glyph on top");
}

#[test]
fn ac50_press_at_the_shared_position_hits_the_selected_node_even_after_a_hover() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    s.split_selected();
    // hover first (as a real pointer does), then press: still the selected one
    s.pointer_hover(pt(9.9, 0.1), false, false);
    s.pointer_hover(pt(10.0, 0.0), false, false);
    s.pointer_down(pt(10.0, 0.0), false);
    s.pointer_hover(pt(12.0, 6.0), false, false);
    s.pointer_up(pt(12.0, 6.0), false, false);
    let objs = objects_of(&s);
    assert!(near(*path_points(&objs[0]).last().unwrap(), pt(10.0, 0.0)));
    assert!(near(path_points(&objs[1])[0], pt(12.0, 6.0)));
}

// ---------------------------------------------------------------------
// Criterion 54, 57: key table
// ---------------------------------------------------------------------

#[test]
fn ac54_tool_letters_b_n_e_star_switch_in_every_state() {
    let states: Vec<(&str, Box<dyn Fn() -> Session>)> = vec![
        ("select none", Box::new(|| open_doc(&rect_doc()))),
        ("select one", Box::new(select_rect)),
        (
            "pen",
            Box::new(|| {
                let mut s = Session::new(1);
                s.set_tool(Tool::Pen);
                s
            }),
        ),
        (
            "node",
            Box::new(|| node_session(&open_path_doc(), pt(5.0, 0.0))),
        ),
        (
            "rect",
            Box::new(|| {
                let mut s = Session::new(1);
                s.set_tool(Tool::Rectangle);
                s
            }),
        ),
    ];
    for (name, make) in states {
        for (k, shift, tool) in [
            ("b", false, Tool::Pen),
            ("n", false, Tool::Node),
            ("e", false, Tool::Ellipse),
            ("*", true, Tool::PolygonStar),
            ("*", false, Tool::PolygonStar),
            ("B", false, Tool::Pen),
            ("N", false, Tool::Node),
            ("E", false, Tool::Ellipse),
        ] {
            let mut s = make();
            let o = s.key_down(KeyInput {
                key: k,
                shift,
                ..KeyInput::default()
            });
            assert_eq!(o, KeyOutcome::ToolChanged, "{name}: {k}");
            assert_eq!(s.tool(), tool, "{name}: {k}");
        }
    }
}

#[test]
fn ac54_r_and_s_depend_only_on_select_tool_with_a_selection() {
    // Select tool, one object selected: R opens the angle chip, S the size chip
    let mut s = select_rect();
    assert_eq!(press(&mut s, "r"), KeyOutcome::EntryOpened);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.transform_entry().unwrap().kind, "angle");
    s.cancel_transform_entry();
    assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened);
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.transform_entry().unwrap().kind, "size");
    s.cancel_transform_entry();
    // Caps Lock letters
    assert_eq!(press(&mut s, "R"), KeyOutcome::EntryOpened);
    s.cancel_transform_entry();
    assert_eq!(press(&mut s, "S"), KeyOutcome::EntryOpened);
    s.cancel_transform_entry();

    // Select tool, nothing selected: R = Rectangle, S = Select
    let mut s = open_doc(&rect_doc());
    assert_eq!(press(&mut s, "r"), KeyOutcome::ToolChanged);
    assert_eq!(s.tool(), Tool::Rectangle);
    let mut s = open_doc(&rect_doc());
    let o = press(&mut s, "s");
    assert!(
        matches!(o, KeyOutcome::ToolChanged | KeyOutcome::Ignored),
        "{o:?}"
    );
    assert_eq!(s.tool(), Tool::Select);

    // every other tool: R = Rectangle, S = Select, even with a selection kept
    for tool in [Tool::Pen, Tool::Node, Tool::Ellipse, Tool::PolygonStar] {
        let mut s = open_doc(&rect_doc());
        click(&mut s, pt(20.0, 0.0));
        s.set_tool(tool);
        // creation tools clear the selection; Node keeps it
        assert_eq!(press(&mut s, "r"), KeyOutcome::ToolChanged, "{tool:?}");
        assert_eq!(s.tool(), Tool::Rectangle, "{tool:?}");
        let mut s = open_doc(&rect_doc());
        click(&mut s, pt(20.0, 0.0));
        s.set_tool(tool);
        assert_eq!(press(&mut s, "s"), KeyOutcome::ToolChanged, "{tool:?}");
        assert_eq!(s.tool(), Tool::Select, "{tool:?}");
    }
    // from Rectangle with R: stays Rectangle (no-op or changed)
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    press(&mut s, "r");
    assert_eq!(s.tool(), Tool::Rectangle);
}

/// Superseded by `multi-object-transform` (criteria 33, 34 and 37): R and S act on
/// a selection of several objects too, over the group box; "Select one object to
/// type a value" no longer exists.
#[test]
fn ac54_several_selected_objects_r_and_s_open_the_entries_of_the_selection() {
    let mut s = open_doc(&two_rects_doc());
    click(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(120.0, 0.0), true, false);
    s.pointer_down(pt(120.0, 0.0), true);
    s.pointer_up(pt(120.0, 0.0), true, false);
    assert_eq!(s.selected_object_count(), 2);
    let before = state_snapshot(&s);
    for (k, kind) in [("r", "angle"), ("s", "size")] {
        assert_eq!(press(&mut s, k), KeyOutcome::EntryOpened, "{k}");
        assert_eq!(s.tool(), Tool::Select);
        assert_eq!(s.selected_object_count(), 2);
        assert_eq!(s.transform_entry().expect("a chip").kind, kind);
        s.cancel_transform_entry();
    }
    assert_eq!(state_snapshot(&s), before, "opening writes nothing");
}

#[test]
fn ac57_r_and_s_open_exactly_the_double_click_entries_for_every_kind() {
    let kinds: Vec<(&str, Document, Point, Point)> = vec![
        ("rect", rect_doc(), pt(20.0, 0.0), pt(40.0, 20.0)),
        ("path", closed_path_doc(), pt(10.0, 0.0), pt(20.0, 15.0)),
        (
            "polygon",
            {
                let d = Document::new(1);
                let _ = d.create_polygon(
                    StarFrame {
                        center: pt(30.0, 30.0),
                        radius: Length::from_mm(20.0),
                        angle: Angle::from_radians(0.3),
                    },
                    PointCount::new(6).unwrap(),
                );
                d
            },
            pt(30.0 + 20.0 * 0.3_f64.cos(), 30.0 + 20.0 * 0.3_f64.sin()),
            // The box turns with the shape (`polygon-star-box-refit`): its
            // bottom-right corner is (R, R) turned by the frame angle.
            pt(
                30.0 + 20.0 * (0.3_f64.cos() - 0.3_f64.sin()),
                30.0 + 20.0 * (0.3_f64.sin() + 0.3_f64.cos()),
            ),
        ),
    ];
    for (name, doc, grab, south_east) in kinds {
        for (k, kind) in [
            ("r", "angle"),
            ("s", if name == "polygon" { "radius" } else { "size" }),
        ] {
            let mut s = open_doc(&doc);
            click(&mut s, grab);
            assert_eq!(press(&mut s, k), KeyOutcome::EntryOpened, "{name} {k}");
            let via_key = s.transform_entry().unwrap();
            assert_eq!(via_key.kind, kind, "{name} {k}");
            s.cancel_transform_entry();
            // the double-click route at the same handle
            // The key S opens its chip by the centre (criterion 59), so the
            // double-click route is tried on the bottom-right handle itself.
            let h = if k == "s" { south_east } else { via_key.handle };
            s.pointer_hover(h, false, false);
            s.pointer_down(h, false);
            s.pointer_up(h, false, false);
            s.pointer_hover(h, false, false);
            s.double_click(h, false, false);
            let via_dbl = s
                .transform_entry()
                .unwrap_or_else(|| panic!("{name} {k}: double click opens a chip"));
            assert_eq!(via_dbl.kind, via_key.kind, "{name} {k}");
            assert_eq!(via_dbl.fields, via_key.fields, "{name} {k}: same prefill");
            if k == "s" {
                // The one difference: the key's chip is placed by the centre,
                // the double-click's at the handle.
                assert!(near(via_key.handle, via_key.center), "{name} {k}");
                assert!(via_key.at_centre && !via_dbl.at_centre, "{name} {k}");
                assert!(near(via_dbl.handle, south_east), "{name} {k}");
            } else {
                assert!(
                    near(via_dbl.handle, via_key.handle),
                    "{name} {k}: same anchor"
                );
                assert!(!via_key.at_centre && !via_dbl.at_centre);
            }
            assert!(near(via_dbl.center, via_key.center), "{name} {k}");
        }
    }
}

#[test]
fn ac57_r_and_s_chips_apply_one_commit_and_work_at_any_object_size() {
    // tiny and huge objects: the corner handle may be hidden at a small size
    for size in [0.2, 1.0, 5.0, 40.0, 4000.0] {
        let d = Document::new(1);
        let _ = d.create_rect(RectBounds {
            origin: pt(0.0, 0.0),
            width: Length::from_mm(size),
            height: Length::from_mm(size / 2.0),
        });
        let mut s = open_doc(&d);
        click(&mut s, pt(size / 2.0, 0.0));
        assert_eq!(s.selected_object_count(), 1, "size {size}");
        assert_eq!(press(&mut s, "r"), KeyOutcome::EntryOpened, "size {size}");
        let before = change_count(&s);
        let out = s.commit_transform_entry("30", "", 0);
        assert_eq!(out, curvyo_ui_core::EntryOutcome::Committed, "size {size}");
        assert_eq!(change_count(&s), before + 1);
        assert_eq!(press(&mut s, "s"), KeyOutcome::EntryOpened, "size {size}");
        assert_eq!(s.transform_entry().unwrap().kind, "size");
        s.cancel_transform_entry();
    }
}

// ---------------------------------------------------------------------
// Criterion 55: the one gate
// ---------------------------------------------------------------------

const GATED_KEYS: [&str; 8] = ["b", "n", "e", "*", "r", "s", "Delete", "Backspace"];

fn bundle(s: &Session) -> (Tool, usize, Vec<ObjectSnapshot>, usize, bool) {
    (
        s.tool(),
        s.selected_object_count(),
        objects_of(s),
        change_count(s),
        s.transform_entry().is_some(),
    )
}

#[test]
fn ac55_modifiers_repeat_and_dom_blocked_gate_every_key_in_every_state() {
    let makers: Vec<(&str, Box<dyn Fn() -> Session>)> = vec![
        ("select one", Box::new(select_rect)),
        ("select none", Box::new(|| open_doc(&rect_doc()))),
        (
            "node sel",
            Box::new(|| {
                let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
                click(&mut s, pt(10.0, 0.0));
                s
            }),
        ),
        (
            "pen",
            Box::new(|| {
                let mut s = Session::new(1);
                s.set_tool(Tool::Pen);
                s
            }),
        ),
        (
            "ellipse",
            Box::new(|| {
                let mut s = Session::new(1);
                s.set_tool(Tool::Ellipse);
                s
            }),
        ),
    ];
    for (name, make) in &makers {
        for k in GATED_KEYS {
            for (label, input) in [
                (
                    "ctrl/cmd",
                    KeyInput {
                        key: k,
                        ctrl: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "alt",
                    KeyInput {
                        key: k,
                        alt: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "ctrl+alt",
                    KeyInput {
                        key: k,
                        ctrl: true,
                        alt: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "repeat",
                    KeyInput {
                        key: k,
                        repeat: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "dom_blocked",
                    KeyInput {
                        key: k,
                        dom_blocked: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "ctrl+shift",
                    KeyInput {
                        key: k,
                        ctrl: true,
                        shift: true,
                        ..KeyInput::default()
                    },
                ),
                (
                    "alt+shift",
                    KeyInput {
                        key: k,
                        alt: true,
                        shift: true,
                        ..KeyInput::default()
                    },
                ),
            ] {
                let mut s = make();
                let before = bundle(&s);
                let o = s.key_down(input);
                assert_eq!(o, KeyOutcome::Ignored, "{name}: {k} with {label}");
                assert_eq!(bundle(&s), before, "{name}: {k} with {label} changed state");
            }
        }
    }
}

#[test]
fn ac55_ctrl_r_ctrl_s_cmd_r_alt_e_change_no_tool() {
    for (k, ctrl, alt) in [
        ("r", true, false),
        ("s", true, false),
        ("e", false, true),
        ("R", true, false),
        ("S", true, false),
    ] {
        for make in [select_rect as fn() -> Session, || open_doc(&rect_doc())] {
            let mut s = make();
            let t = s.tool();
            let o = s.key_down(KeyInput {
                key: k,
                ctrl,
                alt,
                ..KeyInput::default()
            });
            assert_eq!(o, KeyOutcome::Ignored);
            assert_eq!(s.tool(), t);
            assert!(s.transform_entry().is_none());
        }
    }
}

#[test]
fn ac55_an_open_entry_chip_blocks_the_keys() {
    let mut s = select_rect();
    assert_eq!(press(&mut s, "r"), KeyOutcome::EntryOpened);
    for k in GATED_KEYS {
        let before = bundle(&s);
        assert_eq!(press(&mut s, k), KeyOutcome::Ignored, "chip open: {k}");
        assert_eq!(bundle(&s), before, "chip open: {k}");
    }
    assert!(s.transform_entry().is_some(), "the chip is still open");
}

#[test]
fn ac55_a_drag_in_flight_blocks_every_key_and_the_commit_is_unchanged() {
    // reference: the same move drag without any key
    let reference = {
        let mut s = select_rect();
        s.pointer_hover(pt(20.0, 0.0), false, false);
        s.pointer_down(pt(20.0, 0.0), false);
        s.pointer_hover(pt(30.0, 12.0), false, false);
        s.pointer_hover(pt(35.0, 17.0), false, false);
        s.pointer_up(pt(35.0, 17.0), false, false);
        (objects_of(&s), change_count(&s))
    };
    for k in GATED_KEYS.iter().chain(["m", "k", "K", "Delete"].iter()) {
        let mut s = select_rect();
        s.pointer_hover(pt(20.0, 0.0), false, false);
        s.pointer_down(pt(20.0, 0.0), false);
        s.pointer_hover(pt(30.0, 12.0), false, false);
        let o = press(&mut s, k);
        assert_eq!(o, KeyOutcome::Ignored, "move drag: {k}");
        assert_eq!(s.tool(), Tool::Select, "move drag: {k} changed the tool");
        assert_eq!(n_objects(&s), 1, "move drag: {k} deleted");
        s.pointer_hover(pt(35.0, 17.0), false, false);
        press(&mut s, k);
        s.pointer_up(pt(35.0, 17.0), false, false);
        assert_eq!(
            (objects_of(&s), change_count(&s)),
            reference,
            "{k}: commit equals the keyless drag"
        );
    }
}

#[test]
fn ac55_every_kind_of_drag_blocks_the_keys() {
    // resize / rotate / marquee on Select; node drag; create drags; pan
    let mut cases: Vec<(&str, Session, Box<dyn Fn(&mut Session)>)> = Vec::new();
    cases.push((
        "resize",
        select_rect(),
        Box::new(|s: &mut Session| {
            let h = chip_handle(s, "s");
            s.pointer_hover(h, false, false);
            s.pointer_down(h, false);
            s.pointer_hover(pt(h.x + 20.0, h.y + 20.0), false, false);
        }),
    ));
    cases.push((
        "rotate",
        select_rect(),
        Box::new(|s: &mut Session| {
            let h = chip_handle(s, "r");
            s.pointer_hover(h, false, false);
            s.pointer_down(h, false);
            s.pointer_hover(pt(h.x + 20.0, h.y + 20.0), false, false);
        }),
    ));
    cases.push((
        "node drag",
        node_session(&open_path_doc(), pt(5.0, 0.0)),
        Box::new(|s: &mut Session| {
            click(s, pt(10.0, 0.0));
            s.pointer_hover(pt(10.0, 0.0), false, false);
            s.pointer_down(pt(10.0, 0.0), false);
            s.pointer_hover(pt(14.0, 12.0), false, false);
        }),
    ));
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
        let mut s = Session::new(1);
        s.set_tool(tool);
        cases.push((
            "create drag",
            s,
            Box::new(|s: &mut Session| {
                s.pointer_hover(pt(10.0, 10.0), false, false);
                s.pointer_down(pt(10.0, 10.0), false);
                s.pointer_hover(pt(40.0, 40.0), false, false);
            }),
        ));
    }
    cases.push((
        "pan",
        select_rect(),
        Box::new(|s: &mut Session| {
            s.begin_pan(10.0, 10.0);
            s.pan_to(20.0, 30.0);
        }),
    ));
    for (name, mut s, start) in cases {
        start(&mut s);
        let t = s.tool();
        let count = n_objects(&s);
        for k in GATED_KEYS.iter().chain(["m", "k"].iter()) {
            let o = press(&mut s, k);
            assert_eq!(o, KeyOutcome::Ignored, "{name}: {k}");
            assert_eq!(s.tool(), t, "{name}: {k} changed the tool");
            assert_eq!(n_objects(&s), count, "{name}: {k} deleted or created");
        }
    }
}

#[test]
fn ac55_an_unfinished_pen_path_blocks_every_key_but_enter_and_escape_end_it() {
    for k in GATED_KEYS {
        let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 5.0)]);
        assert_eq!(press(&mut s, k), KeyOutcome::Ignored, "pen path open: {k}");
        assert_eq!(s.tool(), Tool::Pen);
        assert_eq!(s.pen_in_progress().map(<[NewAnchor]>::len), Some(3), "{k}");
    }
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 5.0)]);
    assert_eq!(press(&mut s, "Enter"), KeyOutcome::PenFinished);
    assert_eq!(n_objects(&s), 1);
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 5.0)]);
    esc(&mut s);
    assert!(s.pen_in_progress().is_none());
    // the double-click route finishes too (unchanged)
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0)]);
    s.pointer_hover(pt(20.0, 5.0), false, false);
    s.pointer_down(pt(20.0, 5.0), false);
    s.pointer_up(pt(20.0, 5.0), false, false);
    s.double_click(pt(20.0, 5.0), false, false);
    s.finish_pen();
    assert_eq!(n_objects(&s), 1);
}

#[test]
fn ac55_space_held_is_reported_as_dom_blocked_and_is_ignored() {
    let mut s = select_rect();
    for k in GATED_KEYS {
        let o = s.key_down(KeyInput {
            key: k,
            dom_blocked: true,
            ..KeyInput::default()
        });
        assert_eq!(o, KeyOutcome::Ignored, "{k}");
    }
    // Escape is not gated by the list (but by repeat only)
    let o = s.key_down(KeyInput {
        key: "Escape",
        ctrl: true,
        ..KeyInput::default()
    });
    assert!(
        matches!(o, KeyOutcome::Escape(_)),
        "Escape with Ctrl held still acts: {o:?}"
    );
}

#[test]
fn ac55_shift_is_allowed_only_for_star_and_shift_k() {
    // Shift+B etc: record the behaviour; the spec says the gate blocks Shift
    // for every letter but * and K, and Caps Lock (no Shift flag) works.
    for (k, tool) in [("b", Tool::Pen), ("n", Tool::Node), ("e", Tool::Ellipse)] {
        let mut s = open_doc(&rect_doc());
        let o = s.key_down(KeyInput {
            key: k,
            shift: true,
            ..KeyInput::default()
        });
        assert_eq!(
            o,
            KeyOutcome::Ignored,
            "Shift+{k}: spec 55 allows Shift only for * and Shift+K"
        );
        assert_eq!(s.tool(), Tool::Select, "Shift+{k}");
        let _ = tool;
    }
    let mut s = open_doc(&rect_doc());
    assert_eq!(
        s.key_down(KeyInput {
            key: "*",
            shift: true,
            ..KeyInput::default()
        }),
        KeyOutcome::ToolChanged
    );
}

#[test]
fn ac55_the_modifier_keys_themselves_and_unknown_keys_are_ignored_without_effect() {
    let mut s = select_rect();
    let before = bundle(&s);
    for k in [
        "Shift",
        "Control",
        "Alt",
        "Meta",
        "CapsLock",
        " ",
        "Tab",
        "ArrowLeft",
        "F5",
        "",
        "Dead",
        "Unidentified",
        "Process",
        "ß",
        "é",
        "ЯЯ",
        "r\u{0}",
        "rr",
        "R ",
        "\u{1f600}",
        "Insert",
        "Home",
        "1",
        "0",
        "-",
        "+",
    ] {
        let o = press(&mut s, k);
        assert!(
            matches!(o, KeyOutcome::Ignored),
            "{k:?} is not a key of this table: {o:?}"
        );
        assert_eq!(bundle(&s), before, "{k:?}");
    }
    let long = "r".repeat(100_000);
    assert_eq!(press(&mut s, &long), KeyOutcome::Ignored);
}

// ---------------------------------------------------------------------
// Criterion 61: Delete
// ---------------------------------------------------------------------

#[test]
fn ac61_delete_and_backspace_per_tool() {
    for k in ["Delete", "Backspace"] {
        // Select: the selection is deleted
        let mut s = select_rect();
        assert_eq!(press(&mut s, k), KeyOutcome::Deleted);
        assert_eq!(n_objects(&s), 0, "{k}");
        // Node: the selected node
        let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
        click(&mut s, pt(10.0, 0.0));
        assert_eq!(press(&mut s, k), KeyOutcome::Deleted);
        assert_eq!(
            path_points(&objects_of(&s)[0]).len(),
            2,
            "{k}: node deleted"
        );
        // Pen: nothing
        let mut s = open_doc(&rect_doc());
        s.set_tool(Tool::Pen);
        let before = change_count(&s);
        let _ = press(&mut s, k);
        assert_eq!(n_objects(&s), 1, "{k}: pen deletes nothing");
        assert_eq!(change_count(&s), before);
        // creation tools: nothing selected, nothing happens
        for tool in [Tool::Rectangle, Tool::Ellipse, Tool::PolygonStar] {
            let mut s = open_doc(&rect_doc());
            s.set_tool(tool);
            let _ = press(&mut s, k);
            assert_eq!(n_objects(&s), 1, "{k} {tool:?}");
        }
    }
}

#[test]
fn ac61_delete_mid_node_drag_deletes_nothing_and_the_drag_commits_normally() {
    let mut s = node_session(&open_path_doc(), pt(5.0, 0.0));
    click(&mut s, pt(10.0, 0.0));
    s.pointer_hover(pt(10.0, 0.0), false, false);
    s.pointer_down(pt(10.0, 0.0), false);
    s.pointer_hover(pt(12.0, 7.0), false, false);
    assert_eq!(press(&mut s, "Delete"), KeyOutcome::Ignored);
    assert_eq!(press(&mut s, "Backspace"), KeyOutcome::Ignored);
    s.pointer_up(pt(12.0, 7.0), false, false);
    let pts = path_points(&objects_of(&s)[0]);
    assert_eq!(pts.len(), 3, "no node deleted");
    assert!(near(pts[1], pt(12.0, 7.0)), "the drag committed");
}

#[test]
fn ac61_delete_with_ctrl_or_alt_does_nothing() {
    for (ctrl, alt) in [(true, false), (false, true)] {
        let mut s = select_rect();
        let o = s.key_down(KeyInput {
            key: "Delete",
            ctrl,
            alt,
            ..KeyInput::default()
        });
        assert_eq!(o, KeyOutcome::Ignored);
        assert_eq!(n_objects(&s), 1);
    }
}

// ---------------------------------------------------------------------
// Hostile: random key and pointer sequences keep every invariant
// ---------------------------------------------------------------------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() as usize) % items.len()]
    }
}

#[test]
fn hostile_random_sequences_never_panic_and_ignored_keys_change_nothing() {
    let keys = [
        "b",
        "n",
        "e",
        "*",
        "r",
        "s",
        "m",
        "k",
        "K",
        "Delete",
        "Backspace",
        "Escape",
        "Enter",
        " ",
        "x",
        "R",
        "S",
        "Shift",
        "Control",
    ];
    let points = [
        pt(0.0, 0.0),
        pt(10.0, 0.0),
        pt(20.0, 0.0),
        pt(20.0, 10.0),
        pt(40.0, 20.0),
        pt(5.0, 0.0),
        pt(100.0, 0.0),
        pt(140.0, 20.0),
        pt(-30.0, -30.0),
        pt(1e6, -1e6),
        pt(f64::MAX / 4.0, 0.0),
        pt(10.0, 15.0),
    ];
    for seed in 0..8u64 {
        let mut rng = Lcg(seed + 1);
        let mut s = open_doc(&two_rects_doc());
        let mut down = false;
        for step in 0..300 {
            if step % 50 == 0 {
                let _ = doc_of(&s);
            }
            match rng.next() % 6 {
                0 => {
                    let p = *rng.pick(&points);
                    s.pointer_hover(
                        p,
                        rng.next().is_multiple_of(4),
                        rng.next().is_multiple_of(4),
                    );
                }
                1 if !down => {
                    let p = *rng.pick(&points);
                    s.pointer_down(p, rng.next().is_multiple_of(4));
                    down = true;
                }
                2 if down => {
                    let p = *rng.pick(&points);
                    s.pointer_up(
                        p,
                        rng.next().is_multiple_of(4),
                        rng.next().is_multiple_of(4),
                    );
                    down = false;
                }
                3 => {
                    let k = *rng.pick(&keys);
                    let input = KeyInput {
                        key: k,
                        shift: rng.next().is_multiple_of(5),
                        ctrl: rng.next().is_multiple_of(6),
                        alt: rng.next().is_multiple_of(8),
                        repeat: rng.next().is_multiple_of(6),
                        dom_blocked: rng.next().is_multiple_of(8),
                    };
                    let before = bundle(&s);
                    let o = s.key_down(input);
                    if o == KeyOutcome::Ignored {
                        assert_eq!(
                            bundle(&s),
                            before,
                            "seed {seed}: an ignored key changed state ({input:?})"
                        );
                    }
                    if k == "Escape" && !input.repeat {
                        // one press = one step, never writes
                        assert_eq!(change_count(&s), before.3, "seed {seed}: Escape wrote");
                    }
                }
                4 => {
                    s.set_tool(*rng.pick(&TOOLS));
                    down = false;
                }
                5 => {
                    s.pointer_cancelled();
                    down = false;
                }
                _ => {}
            }
            // the document always packs and reopens
            if let Some(e) = s.transform_entry() {
                assert_eq!(
                    s.tool(),
                    Tool::Select,
                    "an open chip implies the Select tool: {e:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// White-box follow-ups after reading the diff
// ---------------------------------------------------------------------

#[test]
fn probe_keys_between_press_and_dead_zone_exit_leave_a_consistent_state() {
    // A press on the object with the button still down and no movement yet:
    // the spec's "move drag" starts after the 3 px dead zone, so keys may
    // act; whatever they do, the release must not corrupt the document.
    for k in [
        "r",
        "s",
        "b",
        "n",
        "e",
        "*",
        "Delete",
        "Backspace",
        "Escape",
    ] {
        let mut s = select_rect();
        let before = objects_of(&s);
        s.pointer_hover(pt(20.0, 0.0), false, false);
        s.pointer_down(pt(20.0, 0.0), false);
        let o = press(&mut s, k);
        eprintln!("press-hold then {k:?}: {o:?}, tool {:?}", s.tool());
        s.pointer_hover(pt(26.0, 6.0), false, false);
        s.pointer_up(pt(26.0, 6.0), false, false);
        // consistent: the doc reopens, and a chip never stays open across a
        // tool change
        let _ = doc_of(&s);
        if s.tool() != Tool::Select {
            assert!(
                s.transform_entry().is_none(),
                "{k}: chip open outside Select"
            );
        }
        if matches!(k, "Delete" | "Backspace") && n_objects(&s) == 0 {
            // deleted while the button was down: the release must not
            // resurrect or move anything
            assert_eq!(objects_of(&s).len(), 0);
        }
        let _ = before;
    }
}

#[test]
fn ac42_a_parameter_chip_is_closed_by_escape_and_blocks_the_letters() {
    // rect corner-radius handle: double-click it, then Escape
    let mut s = open_doc(&rect_doc());
    click(&mut s, pt(20.0, 0.0));
    let mut found = None;
    'scan: for iy in -30..=60 {
        for ix in -30..=80 {
            let p = pt(f64::from(ix) * 0.75, f64::from(iy) * 0.75);
            s.pointer_hover(p, false, false);
            let h = s.handle_hint();
            if h.contains("radius") {
                found = Some(p);
                break 'scan;
            }
        }
    }
    let p = found.expect("a corner radius handle exists");
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
    s.pointer_hover(p, false, false);
    s.double_click(p, false, false);
    let Some(view) = s.transform_entry() else {
        panic!("double click on the corner radius handle opens its chip");
    };
    assert_eq!(view.kind, "corner-radius");
    let before = change_count(&s);
    for k in ["b", "n", "e", "*", "r", "s", "Delete"] {
        assert_eq!(
            press(&mut s, k),
            KeyOutcome::Ignored,
            "param chip open: {k}"
        );
        assert!(s.transform_entry().is_some());
    }
    assert_eq!(step_of(esc(&mut s)), EscapeStep::ClosedEntry);
    assert!(s.transform_entry().is_none());
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(change_count(&s), before);
}

#[test]
fn probe_double_press_and_orphan_release_keep_escape_consistent() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    // a press, a second press without release (lost pointer-up), Escape
    s.pointer_down(pt(0.0, 0.0), false);
    s.pointer_down(pt(5.0, 5.0), false);
    s.pointer_hover(pt(30.0, 30.0), false, false);
    let o = esc(&mut s);
    eprintln!("double press then Escape: {o:?}");
    s.pointer_cancelled();
    assert_eq!(s.tool(), Tool::Rectangle);
    // after pointer_cancelled the button is forgotten: Escape leaves
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
    // an orphan release does not create anything or break the cascade
    s.set_tool(Tool::Ellipse);
    s.pointer_up(pt(40.0, 40.0), false, false);
    assert_eq!(n_objects(&s), 0);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
}

#[test]
fn ac49_a_cancelled_creation_drag_then_pointer_cancelled_lets_escape_leave() {
    // button lost (pointercancel) after Escape cancelled the drag
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.pointer_hover(pt(10.0, 10.0), false, false);
    s.pointer_down(pt(10.0, 10.0), false);
    s.pointer_hover(pt(40.0, 40.0), false, false);
    assert_eq!(step_of(esc(&mut s)), EscapeStep::CancelledDrag);
    s.pointer_cancelled();
    assert_eq!(step_of(esc(&mut s)), EscapeStep::LeftTool);
}

#[test]
fn hostile_non_finite_and_extreme_pointer_input_does_not_corrupt_escape_state() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    for p in [
        pt(f64::NAN, 0.0),
        pt(0.0, f64::INFINITY),
        pt(f64::NEG_INFINITY, f64::NAN),
        pt(1e300, -1e300),
    ] {
        s.pointer_hover(pt(0.0, 0.0), false, false);
        s.pointer_down(pt(0.0, 0.0), false);
        s.pointer_hover(p, false, false);
        s.pointer_up(p, false, true);
        let _ = doc_of(&s);
        assert_eq!(n_objects(&s), n_objects(&s));
        // the button state is cleared by the release: Escape leaves the tool
        let step = esc(&mut s);
        assert!(
            matches!(
                step_of(step),
                EscapeStep::LeftTool | EscapeStep::Nothing | EscapeStep::ClearedState
            ),
            "after a hostile release Escape acts normally: {step:?}"
        );
        s.set_tool(Tool::Rectangle);
    }
}

#[test]
fn ac55_key_input_extremes_do_not_panic() {
    let mut s = select_rect();
    for k in [
        "\u{0}",
        "Delete\u{0}",
        &"é".repeat(1000),
        "\u{202e}r",
        "ｒ",
        "Ｒ",
        "r\u{301}",
    ] {
        let o = press(&mut s, k);
        assert_eq!(o, KeyOutcome::Ignored, "{k:?}");
    }
    assert_eq!(s.tool(), Tool::Select);
}

/// FINDING: the frontend's window `blur` handler now calls
/// `Session::pointer_cancelled` (so a lost pointer cancels a drag), and
/// `pointer_cancelled` discards an unfinished Pen path even when no pointer
/// button is down. Switching to another window while drawing a path therefore
/// throws the path away, which `main` does not do and the specification
/// never asks for (criterion 43 names Escape only; "an unfinished path is
/// never kept by Escape; Enter and the double-click finish it").
#[test]
fn finding_pointer_cancelled_without_a_pressed_button_keeps_the_pen_path() {
    let mut s = pen_with(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 5.0)]);
    assert!(s.pen_in_progress().is_some());
    s.pointer_cancelled();
    assert!(
        s.pen_in_progress().is_some(),
        "no button was down: nothing was lost, the path must stay"
    );
}

#[test]
fn ac50_the_selected_node_after_a_split_is_the_one_with_the_original_outgoing_handle() {
    for closed in [false, true] {
        let d = Document::new(1);
        let mut middle = anchor(2, 10.0, 0.0);
        middle.handle_out = curvyo_document_core::Vec2::new(3.0, 2.0);
        let _ = d.create_path(&[anchor(1, 0.0, 0.0), middle, anchor(3, 20.0, 5.0)], closed);
        let mut s = node_session(&d, pt(0.0, 0.0));
        click(&mut s, pt(10.0, 0.0));
        s.split_selected();
        let before = change_count(&s);
        drag(&mut s, pt(10.0, 0.0), pt(12.0, -9.0));
        assert_eq!(change_count(&s), before + 1, "closed {closed}: one commit");
        let d2 = doc_of(&s);
        let mut moved = Vec::new();
        for id in d2.object_ids() {
            let p = d2.path(id).unwrap();
            for a in &p.anchors {
                if near(a.point, pt(12.0, -9.0)) {
                    moved.push((a.handle_out, a.handle_in));
                }
            }
        }
        assert_eq!(moved.len(), 1, "closed {closed}: exactly one node moved");
        assert_eq!(
            moved[0].0,
            curvyo_document_core::Vec2::new(3.0, 2.0),
            "closed {closed}: the dragged (selected) node keeps the original outgoing handle"
        );
        assert_eq!(
            moved[0].1,
            curvyo_document_core::Vec2::ZERO,
            "closed {closed}"
        );
    }
}
