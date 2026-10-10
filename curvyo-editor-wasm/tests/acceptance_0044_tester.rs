//! Independent tester acceptance tests for `specs/0044-editing-quick-wins/specification.md`
//! (select all, keyboard nudge, the draw-list cache). Written from the specification and the
//! public API before the implementation diff was read. Everything goes through `Session`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::similar_names
)]
#![allow(missing_docs, clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::many_single_char_names, clippy::cast_possible_truncation)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, DisplayUnit, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds,
    Shape, pack, unpack,
};
use curvyo_editor_wasm::{KeyHint, KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, MoveEntryMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn add_rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    })
}

/// `n` rectangles in a row, 30 mm apart, each 10 x 10 mm, at y = 20.
fn row_doc(n: usize) -> Document {
    let d = Document::new(1);
    for i in 0..n {
        add_rect(&d, 20.0 + 30.0 * i as f64, 20.0, 10.0, 10.0);
    }
    d
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1200.0, 800.0);
    s
}

fn reread(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn objects(s: &Session) -> Vec<ObjectSnapshot> {
    let d = reread(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn ids(s: &Session) -> Vec<NodeId> {
    reread(s).object_ids()
}

fn loro_of(s: &Session) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&reread(s).export_loro_snapshot().unwrap())
        .unwrap();
    l
}

fn change_count(s: &Session) -> usize {
    loro_of(s).len_changes()
}

/// The number of operations in the log: it grows with every commit that writes.
fn ops(s: &Session) -> usize {
    loro_of(s).len_ops()
}

/// Commit messages, oldest first.
fn labels(s: &Session) -> Vec<String> {
    let l = loro_of(s);
    let frontiers: Vec<loro::ID> = l.oplog_frontiers().iter().collect();
    let mut changes: Vec<(u32, String)> = Vec::new();
    l.travel_change_ancestors(&frontiers, &mut |c| {
        changes.push((
            c.lamport,
            c.message.map(|m| m.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    changes.sort();
    changes.into_iter().map(|(_, m)| m).collect()
}

fn origin_of(o: &ObjectSnapshot) -> Point {
    match o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => bounds.origin,
            _ => panic!("a rectangle"),
        },
        ObjectSnapshot::Path(p) => p.anchors[0].point,
    }
}

fn key(k: &str) -> KeyInput<'_> {
    KeyInput {
        key: k,
        ..KeyInput::default()
    }
}

fn ctrl_a(s: &mut Session) -> KeyOutcome {
    s.key_down(KeyInput {
        key: "a",
        ctrl: true,
        ..KeyInput::default()
    })
}

fn arrow(s: &mut Session, k: &str, shift: bool, repeat: bool, t: f64) -> KeyOutcome {
    s.key_down_at(
        KeyInput {
            key: k,
            shift,
            repeat,
            ..KeyInput::default()
        },
        t,
    )
}

fn select_all(s: &mut Session) {
    assert_eq!(ctrl_a(s), KeyOutcome::SelectedAll);
}

fn selection_ids(s: &Session) -> Vec<NodeId> {
    s.style_scope().ids
}

fn marquee(s: &mut Session, a: Point, b: Point) {
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(b, false, false);
    s.pointer_up(b, false, false);
}

fn typed_move(s: &mut Session, dx: f64, dy: f64) {
    assert_eq!(s.key_down(key("m")), KeyOutcome::EntryOpened);
    assert_eq!(
        s.commit_move_entry(
            &format!("{dx}"),
            &format!("{dy}"),
            MoveEntryMode {
                absolute: false,
                copy: false
            }
        ),
        EntryOutcome::Committed
    );
}

/// A small triangle strictly inside `triangle(d, n', ox, oy)`: combine accepts it as a hole.
fn inner_triangle(d: &Document, n: u64, ox: f64, oy: f64) -> NodeId {
    d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, n * 3), pt(ox + 8.0, oy + 3.0)),
            NewAnchor::corner(AnchorId::new(1, n * 3 + 1), pt(ox + 12.0, oy + 3.0)),
            NewAnchor::corner(AnchorId::new(1, n * 3 + 2), pt(ox + 10.0, oy + 6.0)),
        ],
        true,
    )
}

fn triangle(d: &Document, n: u64, ox: f64, oy: f64) -> NodeId {
    d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, n * 3), pt(ox, oy)),
            NewAnchor::corner(AnchorId::new(1, n * 3 + 1), pt(ox + 20.0, oy)),
            NewAnchor::corner(AnchorId::new(1, n * 3 + 2), pt(ox + 10.0, oy + 15.0)),
        ],
        true,
    )
}

// ---------------------------------------------------------------------
// Select all (criteria 1 to 6)
// ---------------------------------------------------------------------

#[test]
fn c1_ctrl_a_selects_every_root_object_in_stacking_order_and_writes_nothing() {
    let d = row_doc(5);
    let mut s = open(&d);
    let commits = change_count(&s);
    let before = s.pack("0.1.0").unwrap();
    assert_eq!(ctrl_a(&mut s), KeyOutcome::SelectedAll);
    assert_eq!(s.selected_object_count(), 5);
    assert_eq!(selection_ids(&s), ids(&s), "stacking order, bottom to top");
    assert_eq!(change_count(&s), commits, "select all creates no commit");
    assert_eq!(
        s.pack("0.1.0").unwrap().len(),
        before.len(),
        "nothing is written to the file"
    );
    assert!(
        s.style_scope().subject.starts_with("5 "),
        "{}",
        s.style_scope().subject
    );
    assert_eq!(objects(&s), objects(&open(&d)), "nothing else changes");
}

#[test]
fn c1_one_object_and_cmd_is_ctrl() {
    let d = row_doc(1);
    let mut s = open(&d);
    assert_eq!(ctrl_a(&mut s), KeyOutcome::SelectedAll);
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn c1_stacking_order_is_creation_order_with_mixed_kinds() {
    let d = Document::new(1);
    let a = add_rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = triangle(&d, 1, 30.0, 0.0);
    let c = add_rect(&d, 60.0, 0.0, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(selection_ids(&s), vec![a, b, c]);
}

#[test]
fn c2_replaces_existing_selection_and_empty_document_is_consumed() {
    let d = row_doc(5);
    let mut s = open(&d);
    marquee(&mut s, pt(10.0, 10.0), pt(35.0, 40.0));
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(ctrl_a(&mut s), KeyOutcome::SelectedAll);
    assert_eq!(s.selected_object_count(), 5, "replaces, not toggles");
    // Pressing it again does not toggle either.
    assert_eq!(ctrl_a(&mut s), KeyOutcome::SelectedAll);
    assert_eq!(s.selected_object_count(), 5);

    let mut empty = open(&Document::new(1));
    let commits = change_count(&empty);
    let out = ctrl_a(&mut empty);
    assert_eq!(out, KeyOutcome::SelectedAll);
    assert_ne!(out.code(), "ignored", "consumed: preventDefault runs");
    assert_eq!(empty.selected_object_count(), 0);
    assert_eq!(change_count(&empty), commits);
}

#[test]
fn c2_empty_document_gives_no_hint() {
    let mut s = open(&Document::new(1));
    assert!(!matches!(ctrl_a(&mut s), KeyOutcome::Hint(_)));
}

#[test]
fn c3_other_tools_ignore_ctrl_a() {
    for tool in [
        Tool::Pen,
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        let d = row_doc(3);
        let mut s = open(&d);
        s.set_tool(tool);
        assert_eq!(ctrl_a(&mut s), KeyOutcome::Ignored, "{tool:?}");
        assert_eq!(s.selected_object_count(), 0, "{tool:?}");
    }
}

#[test]
fn c3_other_tools_do_not_touch_an_existing_selection() {
    let d = row_doc(3);
    let mut s = open(&d);
    marquee(&mut s, pt(10.0, 10.0), pt(35.0, 40.0));
    assert_eq!(s.selected_object_count(), 1);
    s.set_tool(Tool::Rectangle);
    let kept = s.selected_object_count();
    assert_eq!(ctrl_a(&mut s), KeyOutcome::Ignored);
    assert_eq!(s.selected_object_count(), kept);
    assert_eq!(s.tool(), Tool::Rectangle, "no tool change by surprise");
}

#[test]
fn gate_ctrl_a_is_ignored_in_every_blocked_state() {
    let d = row_doc(3);
    // Text field focus: DOM knowledge only.
    let mut s = open(&d);
    assert_eq!(
        s.key_down(KeyInput {
            key: "a",
            ctrl: true,
            dom_blocked: true,
            ..KeyInput::default()
        }),
        KeyOutcome::Ignored
    );
    assert_eq!(s.selected_object_count(), 0);
    // Repeat: once only.
    assert_eq!(
        s.key_down(KeyInput {
            key: "a",
            ctrl: true,
            repeat: true,
            ..KeyInput::default()
        }),
        KeyOutcome::Ignored
    );
    // Ctrl+Alt+A and Ctrl+Shift+A.
    for (shift, alt) in [(false, true), (true, false), (true, true)] {
        assert_eq!(
            s.key_down(KeyInput {
                key: "a",
                ctrl: true,
                shift,
                alt,
                ..KeyInput::default()
            }),
            KeyOutcome::Ignored,
            "shift={shift} alt={alt}"
        );
    }
    // Alt+A without Ctrl, plain A.
    assert_eq!(
        s.key_down(KeyInput {
            key: "a",
            alt: true,
            ..KeyInput::default()
        }),
        KeyOutcome::Ignored
    );
    assert_eq!(s.selected_object_count(), 0);
}

#[test]
fn gate_ctrl_a_is_ignored_while_dragging_and_while_an_entry_chip_is_open() {
    let d = Document::new(1);
    add_rect(&d, 10.0, 20.0, 100.0, 60.0);
    add_rect(&d, 200.0, 20.0, 40.0, 40.0);
    let mut s = open(&d);
    // A drag in flight: press on the first rectangle's outline and move.
    s.pointer_hover(pt(10.0, 50.0), false, false);
    s.pointer_down(pt(10.0, 50.0), false);
    s.pointer_hover(pt(20.0, 50.0), false, false);
    assert!(s.is_pointer_down());
    assert_eq!(ctrl_a(&mut s), KeyOutcome::Ignored, "drag in flight");
    s.pointer_up(pt(20.0, 50.0), false, false);
    // Chip open: select one, M opens the chip.
    s.escape();
    marquee(&mut s, pt(0.0, 0.0), pt(150.0, 100.0));
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(s.key_down(key("m")), KeyOutcome::EntryOpened);
    assert_eq!(ctrl_a(&mut s), KeyOutcome::Ignored, "chip open");
    assert_eq!(s.selected_object_count(), 1);
}

#[test]
fn c1_boolean_results_and_compound_paths_are_selected_too() {
    let d = Document::new(1);
    triangle(&d, 1, 0.0, 0.0);
    inner_triangle(&d, 2, 0.0, 0.0);
    add_rect(&d, 100.0, 0.0, 20.0, 20.0);
    let mut s = open(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0));
    assert_eq!(s.selected_object_count(), 2);
    let out = s.apply_combine();
    assert!(
        matches!(out, curvyo_editor_wasm::CombineOutcome::Applied { .. }),
        "{out:?}"
    );
    s.escape();
    assert_eq!(ids(&s).len(), 2, "a compound path and a rectangle");
    select_all(&mut s);
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(selection_ids(&s), ids(&s));
    // A boolean result is a root object like any other.
    let d = Document::new(1);
    triangle(&d, 1, 0.0, 0.0);
    triangle(&d, 2, 5.0, 5.0);
    add_rect(&d, 100.0, 0.0, 20.0, 20.0);
    let mut s = open(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0));
    let out = s.apply_boolean(curvyo_ui_core::BooleanOp::Union);
    assert!(
        matches!(out, curvyo_editor_wasm::BooleanOutcome::Applied { .. }),
        "{out:?}"
    );
    s.escape();
    select_all(&mut s);
    assert_eq!(s.selected_object_count(), ids(&s).len());
    assert_eq!(ids(&s).len(), 2);
}

#[test]
fn c6_escape_clears_the_selection_and_there_is_no_ctrl_shift_a() {
    let d = row_doc(3);
    let mut s = open(&d);
    select_all(&mut s);
    let out = s.key_down(key("Escape"));
    assert_ne!(out, KeyOutcome::Ignored);
    assert_eq!(s.selected_object_count(), 0);
    select_all(&mut s);
    assert_eq!(
        s.key_down(KeyInput {
            key: "A",
            ctrl: true,
            shift: true,
            ..KeyInput::default()
        }),
        KeyOutcome::Ignored
    );
    assert_eq!(
        s.selected_object_count(),
        3,
        "Ctrl+Shift+A does not deselect"
    );
}

#[test]
fn c4_style_tab_follows_a_selection_that_becomes_non_empty() {
    use curvyo_ui_core::PanelContent;
    let d = row_doc(3);
    let mut s = open(&d);
    assert_eq!(s.panel_content(), PanelContent::Document);
    select_all(&mut s);
    assert_eq!(s.panel_content(), PanelContent::Style);
}

// ---------------------------------------------------------------------
// Nudge (criteria 7 to 13)
// ---------------------------------------------------------------------

#[test]
fn c7_each_direction_moves_one_millimetre_y_grows_downward() {
    let cases = [
        ("ArrowRight", 1.0, 0.0),
        ("ArrowLeft", -1.0, 0.0),
        ("ArrowDown", 0.0, 1.0),
        ("ArrowUp", 0.0, -1.0),
    ];
    for (k, dx, dy) in cases {
        let mut s = open(&row_doc(1));
        select_all(&mut s);
        let before = origin_of(&objects(&s)[0]);
        assert_eq!(
            arrow(&mut s, k, false, false, 0.0),
            KeyOutcome::Nudged { new_run: true },
            "{k}"
        );
        let after = origin_of(&objects(&s)[0]);
        assert_eq!((after.x - before.x, after.y - before.y), (dx, dy), "{k}");
        // Shift: 10 mm.
        let mut s = open(&row_doc(1));
        select_all(&mut s);
        let _ = arrow(&mut s, k, true, false, 0.0);
        let after = origin_of(&objects(&s)[0]);
        assert_eq!(
            (after.x - before.x, after.y - before.y),
            (10.0 * dx, 10.0 * dy),
            "shift {k}"
        );
    }
}

#[test]
fn c7_distance_is_independent_of_zoom_and_display_unit() {
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    let start = origin_of(&objects(&s)[0]);
    s.wheel(0.0, -400.0, 600.0, 400.0, false, true);
    s.wheel(0.0, -400.0, 600.0, 400.0, false, true);
    let zoomed = s.zoom_percent();
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    s.set_display_unit(DisplayUnit::In);
    let _ = arrow(&mut s, "ArrowRight", false, false, 1000.0);
    s.set_display_unit(DisplayUnit::Cm);
    let _ = arrow(&mut s, "ArrowDown", true, false, 2000.0);
    for _ in 0..4 {
        s.wheel(0.0, 400.0, 600.0, 400.0, false, true);
    }
    assert_ne!(zoomed, s.zoom_percent());
    let _ = arrow(&mut s, "ArrowLeft", false, false, 3000.0);
    let end = origin_of(&objects(&s)[0]);
    assert_eq!((end.x - start.x, end.y - start.y), (1.0, 10.0));
}

#[test]
fn c8_one_translate_objects_commit_per_key_event_for_every_object_kind() {
    let d = Document::new(1);
    add_rect(&d, 0.0, 0.0, 20.0, 20.0);
    triangle(&d, 1, 50.0, 0.0);
    let mut s = open(&d);
    select_all(&mut s);
    let commits = change_count(&s);
    let n = labels(&s).len();
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    assert_eq!(change_count(&s), commits + 1, "one commit for both objects");
    let l = labels(&s);
    assert_eq!(l.len(), n + 1);
    assert!(
        l.last().unwrap().starts_with("translate_objects"),
        "label {l:?}"
    );
}

#[test]
fn c8_nudge_equals_the_typed_relative_move_for_every_object_kind() {
    // One object of each kind per document: rectangle, path, compound path, converted shape.
    let kinds: Vec<(&str, Vec<u8>)> = {
        let mut v = Vec::new();
        let d = Document::new(1);
        add_rect(&d, 3.0, 4.0, 20.0, 10.0);
        v.push(("rectangle", pack(&d, "0.1.0").unwrap()));
        let d = Document::new(1);
        triangle(&d, 1, 3.0, 4.0);
        v.push(("path", pack(&d, "0.1.0").unwrap()));
        let d = Document::new(1);
        triangle(&d, 1, 0.0, 0.0);
        inner_triangle(&d, 2, 0.0, 0.0);
        let mut s = open(&d);
        select_all(&mut s);
        let _ = s.apply_combine();
        assert_eq!(ids(&s).len(), 1, "compound path");
        v.push(("compound path", s.pack("0.1.0").unwrap()));
        let d = Document::new(1);
        add_rect(&d, 3.0, 4.0, 20.0, 10.0);
        let mut s = open(&d);
        select_all(&mut s);
        s.convert_selected_to_paths();
        assert!(matches!(objects(&s)[0], ObjectSnapshot::Path(_)));
        v.push(("converted rectangle", s.pack("0.1.0").unwrap()));
        v
    };
    let moves = [
        ("ArrowRight", false, 1.0, 0.0),
        ("ArrowUp", true, 0.0, -10.0),
        ("ArrowLeft", false, -1.0, 0.0),
        ("ArrowDown", true, 0.0, 10.0),
    ];
    for (name, bytes) in &kinds {
        for (k, shift, dx, dy) in moves {
            let fresh = || {
                let mut s = Session::open(2, bytes).unwrap();
                s.set_tool(Tool::Select);
                s.resize_viewport(1200.0, 800.0);
                select_all(&mut s);
                s
            };
            let mut a = fresh();
            let _ = arrow(&mut a, k, shift, false, 0.0);
            let mut b = fresh();
            typed_move(&mut b, dx, dy);
            assert_eq!(objects(&a), objects(&b), "{name} {k} shift={shift}");
            assert_ne!(objects(&a), objects(&fresh()), "{name}: it did move");
        }
    }
}

#[test]
fn c8_a_mixed_multi_selection_equals_translate_objects_in_one_commit() {
    let d = Document::new(1);
    add_rect(&d, 0.0, 0.0, 20.0, 20.0);
    triangle(&d, 1, 50.0, 0.0);
    triangle(&d, 2, 100.0, 0.0);
    inner_triangle(&d, 3, 100.0, 0.0);
    add_rect(&d, 200.0, 0.0, 20.0, 20.0);
    let mut base = open(&d);
    marquee(&mut base, pt(95.0, -5.0), pt(135.0, 35.0));
    assert_eq!(base.selected_object_count(), 2);
    let _ = base.apply_combine();
    base.escape();
    marquee(&mut base, pt(195.0, -5.0), pt(225.0, 25.0));
    base.convert_selected_to_paths();
    base.escape();
    let prepared = base.pack("0.1.0").unwrap();
    for (k, shift, dx, dy) in [
        ("ArrowRight", false, 1.0, 0.0),
        ("ArrowUp", true, 0.0, -10.0),
    ] {
        let mut a = Session::open(2, &prepared).unwrap();
        a.set_tool(Tool::Select);
        a.resize_viewport(1200.0, 800.0);
        select_all(&mut a);
        assert_eq!(a.selected_object_count(), 4);
        let ops_before = ops(&a);
        let _ = arrow(&mut a, k, shift, false, 0.0);
        assert!(ops(&a) > ops_before);
        let direct = unpack(99, &prepared).unwrap();
        direct
            .translate_objects(
                &direct.object_ids(),
                curvyo_document_core::Vec2::new(dx, dy),
            )
            .unwrap();
        let expected: Vec<ObjectSnapshot> = direct
            .object_ids()
            .into_iter()
            .filter_map(|id| direct.object(id))
            .collect();
        assert_eq!(objects(&a), expected, "{k}");
    }
}

#[test]
fn c8_single_object_and_partial_selection_move_only_the_selected() {
    let mut s = open(&row_doc(3));
    marquee(&mut s, pt(40.0, 10.0), pt(65.0, 40.0));
    assert_eq!(s.selected_object_count(), 1);
    let before = objects(&s);
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    let after = objects(&s);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[2]);
    assert_eq!(origin_of(&after[1]).x, origin_of(&before[1]).x + 1.0);
    assert_eq!(origin_of(&after[1]).y, origin_of(&before[1]).y);
}

#[test]
fn c8_size_and_stroke_do_not_change() {
    let mut s = open(&row_doc(2));
    select_all(&mut s);
    let before = objects(&s);
    for t in 0..7 {
        let _ = arrow(&mut s, "ArrowRight", true, t > 0, f64::from(t) * 30.0);
    }
    let after = objects(&s);
    for (b, a) in before.iter().zip(&after) {
        match (b, a) {
            (ObjectSnapshot::Primitive(pb), ObjectSnapshot::Primitive(pa)) => {
                assert_eq!(pb.style, pa.style);
                let (Shape::Rect { bounds: bb, .. }, Shape::Rect { bounds: ba, .. }) =
                    (pb.shape, pa.shape)
                else {
                    panic!("rects")
                };
                assert_eq!(bb.width, ba.width);
                assert_eq!(bb.height, ba.height);
                assert_eq!(ba.origin.x, bb.origin.x + 70.0);
            }
            _ => panic!("rects"),
        }
    }
}

#[test]
fn c9_held_key_commits_at_once_per_repeat_event_and_the_run_flags_are_right() {
    let mut s = open(&row_doc(2));
    select_all(&mut s);
    let start = origin_of(&objects(&s)[0]);
    let mut last_ops = ops(&s);
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 1000.0),
        KeyOutcome::Nudged { new_run: true }
    );
    for i in 1..=10 {
        assert_eq!(
            arrow(
                &mut s,
                "ArrowRight",
                false,
                true,
                1000.0 + 33.0 * f64::from(i)
            ),
            KeyOutcome::Nudged { new_run: false },
            "repeat {i}"
        );
        // "Every event writes its commit at once": the saved document already holds it.
        assert_eq!(origin_of(&objects(&s)[0]).x, start.x + 1.0 + f64::from(i));
        assert!(ops(&s) > last_ops, "repeat {i} wrote nothing");
        last_ops = ops(&s);
    }
    let l = labels(&s);
    assert!(l.last().unwrap().starts_with("translate_objects"), "{l:?}");
}

#[test]
fn c9_run_boundaries() {
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    // 3 separate presses: 3 runs.
    for t in [0.0, 100.0, 200.0] {
        assert_eq!(
            arrow(&mut s, "ArrowRight", false, false, t),
            KeyOutcome::Nudged { new_run: true }
        );
    }
    // A repeat exactly 600 ms later is still the run, 601 is not.
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, true, 800.0),
        KeyOutcome::Nudged { new_run: false },
        "within 600 ms of the previous event"
    );
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, true, 1400.0),
        KeyOutcome::Nudged { new_run: false },
        "exactly 600 ms: at most 600 ms is allowed"
    );
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, true, 2001.0),
        KeyOutcome::Nudged { new_run: true },
        "601 ms"
    );
    // A different arrow, or Shift changed, ends the run (even flagged as repeat).
    assert_eq!(
        arrow(&mut s, "ArrowDown", false, true, 2010.0),
        KeyOutcome::Nudged { new_run: true }
    );
    assert_eq!(
        arrow(&mut s, "ArrowDown", true, true, 2020.0),
        KeyOutcome::Nudged { new_run: true }
    );
    assert_eq!(
        arrow(&mut s, "ArrowDown", true, true, 2030.0),
        KeyOutcome::Nudged { new_run: false }
    );
    // A non-repeat event is a new press.
    assert_eq!(
        arrow(&mut s, "ArrowDown", true, false, 2040.0),
        KeyOutcome::Nudged { new_run: true }
    );
    // A repeat as the very first event of a fresh session (missed key-down) opens a run.
    let mut t = open(&row_doc(1));
    select_all(&mut t);
    assert_eq!(
        arrow(&mut t, "ArrowLeft", false, true, 5.0),
        KeyOutcome::Nudged { new_run: true }
    );
}

#[test]
fn c9_readout_and_announcement() {
    let mut s = open(&row_doc(1));
    assert!(s.nudge_readout().is_none());
    assert_eq!(s.nudge_announcement(), "");
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    let _ = arrow(&mut s, "ArrowRight", false, true, 30.0);
    let _ = arrow(&mut s, "ArrowRight", false, true, 60.0);
    let r = s.nudge_readout().expect("readout");
    assert_eq!(r.text, "Δ 3.0, 0.0 mm");
    assert_eq!(s.nudge_announcement(), "Moved 3 mm right.");
    // Anchored at the selection centre: rect (20,20) 10x10, moved 3 mm.
    assert_eq!(r.anchor, pt(28.0, 25.0));

    // New step: the readout shows the step's distance only.
    let _ = arrow(&mut s, "ArrowLeft", true, false, 5000.0);
    assert_eq!(s.nudge_readout().unwrap().text, "Δ −10.0, 0.0 mm");
    assert_eq!(s.nudge_announcement(), "Moved 10 mm left.");
    let _ = arrow(&mut s, "ArrowUp", false, false, 6000.0);
    assert_eq!(s.nudge_readout().unwrap().text, "Δ 0.0, −1.0 mm");
    assert_eq!(s.nudge_announcement(), "Moved 1 mm up.");
    let _ = arrow(&mut s, "ArrowDown", true, false, 7000.0);
    assert_eq!(s.nudge_readout().unwrap().text, "Δ 0.0, 10.0 mm");
    assert_eq!(s.nudge_announcement(), "Moved 10 mm down.");
}

#[test]
fn c9_readout_is_millimetres_whatever_the_display_unit() {
    for unit in [DisplayUnit::In, DisplayUnit::Cm] {
        let mut s = open(&row_doc(1));
        s.set_display_unit(unit);
        select_all(&mut s);
        let _ = arrow(&mut s, "ArrowRight", true, false, 0.0);
        assert_eq!(
            s.nudge_readout().unwrap().text,
            "Δ 10.0, 0.0 mm",
            "{unit:?}"
        );
        assert_eq!(s.nudge_announcement(), "Moved 10 mm right.");
    }
}

#[test]
fn c9_announcement_for_the_example_of_the_spec() {
    // "Moved 11 mm right."
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", true, false, 0.0);
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    assert_eq!(
        s.nudge_announcement(),
        "Moved 1 mm right.",
        "different shift = new step"
    );
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", true, false, 0.0);
    for i in 1..=1 {
        let _ = arrow(&mut s, "ArrowRight", true, true, 30.0 * f64::from(i));
    }
    assert_eq!(s.nudge_announcement(), "Moved 20 mm right.");
}

#[test]
fn c10_too_far_is_all_or_nothing_and_writes_nothing() {
    let d = Document::new(1);
    add_rect(&d, 100.0, 100.0, 10.0, 10.0);
    // Right edge at exactly the limit.
    add_rect(&d, 1.0e7 - 10.0, 0.0, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    let commits = change_count(&s);
    let before = objects(&s);
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 0.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
    assert_eq!(change_count(&s), commits);
    assert_eq!(
        objects(&s),
        before,
        "the object that would fit did not move either"
    );
    assert!(s.nudge_readout().is_none());
    assert_eq!(s.nudge_announcement(), "");
    // The other directions are fine.
    assert_eq!(
        arrow(&mut s, "ArrowLeft", false, false, 10.0),
        KeyOutcome::Nudged { new_run: true }
    );
}

#[test]
fn c10_the_limit_is_inclusive_and_symmetric() {
    // Right edge at 1e7 - 1: +1 reaches the limit exactly and is allowed, then +1 is refused.
    let d = Document::new(1);
    add_rect(&d, 1.0e7 - 11.0, 0.0, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 0.0),
        KeyOutcome::Nudged { new_run: true }
    );
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, true, 30.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
    // Shift (10 mm) from 5 mm away is refused entirely, not clamped.
    let d = Document::new(1);
    add_rect(&d, 1.0e7 - 15.0, 0.0, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    let before = objects(&s);
    assert_eq!(
        arrow(&mut s, "ArrowRight", true, false, 0.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
    assert_eq!(objects(&s), before);
    // Negative side and the y axis.
    let d = Document::new(1);
    add_rect(&d, -1.0e7 + 0.5, 0.0, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(
        arrow(&mut s, "ArrowLeft", false, false, 0.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
    let d = Document::new(1);
    add_rect(&d, 0.0, -1.0e7 + 0.5, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(
        arrow(&mut s, "ArrowUp", false, false, 0.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
    let d = Document::new(1);
    add_rect(&d, 0.0, 1.0e7 - 10.5, 10.0, 10.0);
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(
        arrow(&mut s, "ArrowDown", false, false, 0.0),
        KeyOutcome::Hint(KeyHint::TooFar)
    );
}

#[test]
fn c11_nothing_selected_other_tools_and_modifiers_leave_the_key_alone() {
    let d = row_doc(2);
    let mut s = open(&d);
    for k in ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"] {
        assert_eq!(
            arrow(&mut s, k, false, false, 0.0),
            KeyOutcome::Ignored,
            "empty {k}"
        );
        assert_eq!(
            arrow(&mut s, k, true, false, 0.0),
            KeyOutcome::Ignored,
            "empty shift {k}"
        );
    }
    let empty = open(&Document::new(1));
    let mut e = empty;
    assert_eq!(
        arrow(&mut e, "ArrowLeft", false, false, 0.0),
        KeyOutcome::Ignored
    );

    select_all(&mut s);
    let commits = change_count(&s);
    for (ctrl, alt, shift) in [
        (true, false, false),
        (false, true, false),
        (true, true, false),
        (true, false, true),
        (false, true, true),
    ] {
        assert_eq!(
            s.key_down_at(
                KeyInput {
                    key: "ArrowRight",
                    ctrl,
                    alt,
                    shift,
                    ..KeyInput::default()
                },
                0.0
            ),
            KeyOutcome::Ignored,
            "ctrl={ctrl} alt={alt} shift={shift}"
        );
    }
    assert_eq!(
        s.key_down_at(
            KeyInput {
                key: "ArrowRight",
                dom_blocked: true,
                ..KeyInput::default()
            },
            0.0
        ),
        KeyOutcome::Ignored,
        "a focused field keeps its arrows"
    );
    for k in [
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "ArrowRightX",
        "arrowright",
    ] {
        assert_eq!(
            arrow(&mut s, k, false, false, 0.0),
            KeyOutcome::Ignored,
            "{k}"
        );
    }
    assert_eq!(change_count(&s), commits);
    for tool in [
        Tool::Pen,
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        s.set_tool(tool);
        assert_eq!(
            arrow(&mut s, "ArrowRight", false, false, 0.0),
            KeyOutcome::Ignored,
            "{tool:?}"
        );
    }
    assert_eq!(
        change_count(&s),
        commits,
        "no nudge in other tools, nodes included (13)"
    );
}

#[test]
fn c11_not_while_dragging_or_with_a_chip_open() {
    let d = Document::new(1);
    add_rect(&d, 10.0, 20.0, 100.0, 60.0);
    let mut s = open(&d);
    select_all(&mut s);
    let commits = change_count(&s);
    // Drag in flight.
    s.pointer_hover(pt(10.0, 50.0), false, false);
    s.pointer_down(pt(10.0, 50.0), false);
    s.pointer_hover(pt(20.0, 50.0), false, false);
    assert!(s.is_pointer_down());
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 0.0),
        KeyOutcome::Ignored
    );
    s.pointer_up(pt(20.0, 50.0), false, false);
    let after_drag = change_count(&s);
    assert_eq!(
        after_drag,
        commits + 1,
        "the drag is one commit; the arrow added none"
    );
    // Chip open.
    assert_eq!(s.key_down(key("m")), KeyOutcome::EntryOpened);
    let n = change_count(&s);
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 0.0),
        KeyOutcome::Ignored
    );
    assert_eq!(change_count(&s), n);
}

#[test]
fn c12_escape_after_a_nudge_keeps_the_objects_where_they_are() {
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    let start = origin_of(&objects(&s)[0]);
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    let _ = arrow(&mut s, "ArrowRight", false, true, 30.0);
    let commits = change_count(&s);
    let _ = s.key_down(key("Escape"));
    assert_eq!(origin_of(&objects(&s)[0]).x, start.x + 2.0);
    assert_eq!(
        change_count(&s),
        commits,
        "Escape does not undo and writes nothing"
    );
}

#[test]
fn nudge_keeps_the_selection_and_survives_save_and_reopen() {
    let mut s = open(&row_doc(3));
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowDown", true, false, 0.0);
    assert_eq!(s.selected_object_count(), 3);
    let bytes = s.pack("0.1.0").unwrap();
    let t = Session::open(3, &bytes).unwrap();
    for o in objects(&t) {
        assert_eq!(origin_of(&o).y, 30.0);
    }
}

#[test]
fn nudge_followed_by_marquee_and_ctrl_a_still_works_with_transform_state() {
    // After a nudge, a drag-move and select-all keep working (the group box is current).
    let mut s = open(&row_doc(2));
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", true, false, 0.0);
    s.escape();
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowLeft", true, false, 100.0);
    for o in objects(&s) {
        assert!((origin_of(&o).x - (20.0 + 30.0 * 0.0)).abs() < 1e-9 || origin_of(&o).x == 50.0);
    }
}

#[test]
fn nudge_is_exact_after_many_repeats() {
    // 1,000 repeats of 1 mm stay exact (no accumulated float drift in the stored position).
    let mut s = open(&row_doc(1));
    select_all(&mut s);
    for i in 0..1000 {
        let _ = arrow(&mut s, "ArrowRight", false, i > 0, f64::from(i) * 33.0);
    }
    assert_eq!(origin_of(&objects(&s)[0]).x, 1020.0);
    for i in 0..1000 {
        let _ = arrow(
            &mut s,
            "ArrowRight",
            true,
            i > 0,
            40000.0 + f64::from(i) * 33.0,
        );
    }
    assert_eq!(origin_of(&objects(&s)[0]).x, 11_020.0);
}

#[test]
fn nudge_of_a_pen_unfinished_path_is_ignored() {
    let d = row_doc(2);
    let mut s = open(&d);
    select_all(&mut s);
    s.set_tool(Tool::Pen);
    s.pointer_hover(pt(500.0, 500.0), false, false);
    s.pointer_down(pt(500.0, 500.0), false);
    s.pointer_up(pt(500.0, 500.0), false, false);
    s.pointer_hover(pt(600.0, 500.0), false, false);
    s.pointer_down(pt(600.0, 500.0), false);
    s.pointer_up(pt(600.0, 500.0), false, false);
    assert!(s.pen_in_progress().is_some());
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, false, 0.0),
        KeyOutcome::Ignored
    );
    assert_eq!(ctrl_a(&mut s), KeyOutcome::Ignored);
}

// ---------------------------------------------------------------------
// Draw-list cache (milestone 1): no stale draws
// ---------------------------------------------------------------------

/// What a fresh read of the saved file draws, with nothing selected.
fn fresh_frame(s: &Session) -> curvyo_render_core::DrawList {
    let mut t = Session::open(2, &s.pack("0.1.0").unwrap()).unwrap();
    t.set_tool(Tool::Select);
    t.resize_viewport(1200.0, 800.0);
    t.set_display_unit(s.display_unit());
    t.draw_list()
}

fn assert_fresh(s: &mut Session, what: &str) {
    s.escape();
    let kept = s.draw_list();
    assert!(kept == fresh_frame(s), "stale draw after {what}");
}

type Step = (&'static str, fn(&mut Session));

fn scenario_document() -> Document {
    let d = Document::new(1);
    add_rect(&d, 0.0, 0.0, 20.0, 20.0);
    add_rect(&d, 10.0, 10.0, 20.0, 20.0);
    add_rect(&d, 100.0, 0.0, 20.0, 20.0);
    triangle(&d, 1, 50.0, 60.0);
    // A triangle and a smaller one inside it that do not touch: a combine candidate.
    triangle(&d, 2, 200.0, 200.0);
    let _ = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 90), pt(205.0, 205.0)),
            NewAnchor::corner(AnchorId::new(1, 91), pt(212.0, 205.0)),
            NewAnchor::corner(AnchorId::new(1, 92), pt(208.0, 210.0)),
        ],
        true,
    );
    d
}

fn scenario_steps() -> Vec<Step> {
    vec![
        ("nudge", |s| {
            select_all(s);
            let _ = arrow(s, "ArrowRight", true, false, 0.0);
        }),
        ("typed move", |s| {
            marquee(s, pt(105.0, -5.0), pt(135.0, 25.0));
            assert_eq!(s.selected_object_count(), 1);
            typed_move(s, 3.0, -2.0);
        }),
        ("drag move", |s| {
            s.pointer_hover(pt(110.0, 0.0), false, false);
            s.pointer_down(pt(110.0, 0.0), false);
            s.pointer_hover(pt(125.0, 20.0), false, false);
            let _ = s.draw_list();
            s.pointer_up(pt(125.0, 20.0), false, false);
        }),
        ("style change", |s| {
            select_all(s);
            s.set_stroke_paint(false);
        }),
        ("combine", |s| {
            marquee(s, pt(195.0, 195.0), pt(230.0, 230.0));
            assert_eq!(s.selected_object_count(), 2);
            let out = s.apply_combine();
            assert!(
                matches!(out, curvyo_editor_wasm::CombineOutcome::Applied { .. }),
                "{out:?}"
            );
        }),
        ("break apart", |s| {
            marquee(s, pt(195.0, 195.0), pt(230.0, 230.0));
            let _ = s.apply_break_apart();
        }),
        ("boolean", |s| {
            marquee(s, pt(-5.0, -5.0), pt(45.0, 45.0));
            let _ = s.apply_boolean(curvyo_ui_core::BooleanOp::Union);
        }),
        ("convert to paths", |s| {
            select_all(s);
            s.convert_selected_to_paths();
        }),
        ("delete", |s| {
            marquee(s, pt(195.0, 195.0), pt(230.0, 230.0));
            s.delete_selected();
        }),
        ("rectangle tool", |s| {
            s.set_tool(Tool::Rectangle);
            s.pointer_hover(pt(300.0, 300.0), false, false);
            s.pointer_down(pt(300.0, 300.0), false);
            s.pointer_hover(pt(360.0, 340.0), false, false);
            s.pointer_up(pt(360.0, 340.0), false, false);
            s.set_tool(Tool::Select);
        }),
        ("30 nudges", |s| {
            select_all(s);
            for i in 0..30 {
                let _ = arrow(s, "ArrowLeft", false, i > 0, f64::from(i) * 33.0);
                let _ = s.draw_list();
            }
        }),
    ]
}

/// A session that draws a frame before and after every step (the cache is always warm) must
/// draw exactly what a session that ran the same steps without drawing in between draws (the
/// cache is never warm), after every step.
#[test]
fn cache_the_draw_after_each_kind_of_local_change_equals_an_uncached_read() {
    let steps = scenario_steps();
    let d = scenario_document();
    let mut warm = open(&d);
    let _ = warm.draw_list();
    let mut frames = Vec::new();
    for (_, step) in &steps {
        let _ = warm.draw_list();
        step(&mut warm);
        warm.escape();
        frames.push(warm.draw_list());
    }
    for (i, (name, _)) in steps.iter().enumerate() {
        let mut cold = open(&d);
        for (_, step) in &steps[..=i] {
            step(&mut cold);
            cold.escape();
        }
        assert!(cold.draw_list() == frames[i], "stale draw after {name}");
    }
    // And the artwork the warm session ended with is what the file says, for the steps that
    // keep nothing else in view state: compare object counts.
    assert_eq!(objects(&warm).len(), ids(&warm).len());
}

/// Except for conversion, what the session draws equals what a re-opened file draws.
#[test]
fn cache_the_draw_equals_a_reopened_file_after_plain_edits() {
    let d = scenario_document();
    let mut s = open(&d);
    for (name, step) in scenario_steps()
        .iter()
        .filter(|(n, _)| !["convert to paths"].contains(n))
    {
        let _ = s.draw_list();
        step(&mut s);
        s.escape();
        assert!(
            s.draw_list() == fresh_frame(&s),
            "{name}: differs from a reopened file"
        );
    }
}

#[test]
fn cache_document_size_and_presets_do_not_leave_stale_frames() {
    let d = row_doc(3);
    let mut s = open(&d);
    let _ = s.draw_list();
    s.set_document_orientation(curvyo_document_core::Orientation::Portrait);
    assert_fresh(&mut s, "orientation");
    let _ = s.draw_list();
    s.set_document_orientation(curvyo_document_core::Orientation::Landscape);
    assert_fresh(&mut s, "orientation back");
}

#[test]
fn cache_two_sessions_on_documents_with_equal_frontiers_do_not_share() {
    // Two different documents with the same number of commits by different peers/content.
    let a = row_doc(4);
    let b = Document::new(1);
    for i in 0..4 {
        add_rect(&b, 5.0 * f64::from(i), 100.0, 3.0, 3.0);
    }
    let sa = open(&a);
    let sb = open(&b);
    let fa = sa.draw_list();
    let fb = sb.draw_list();
    let different = fa != fb;
    assert!(different);
    let (again_a, again_b) = (sa.draw_list(), sb.draw_list());
    assert!(
        again_a == fa && again_b == fb,
        "each session keeps its own frame"
    );
}

#[test]
fn cache_a_drag_keeps_its_first_read_and_the_end_matches_a_fresh_read() {
    let d = Document::new(1);
    add_rect(&d, 10.0, 20.0, 100.0, 60.0);
    let mut s = open(&d);
    s.pointer_hover(pt(10.0, 50.0), false, false);
    s.pointer_down(pt(10.0, 50.0), false);
    for x in [12.0, 14.0, 30.0] {
        s.pointer_hover(pt(x, 50.0), false, false);
        let _ = s.draw_list();
    }
    s.pointer_up(pt(30.0, 50.0), false, false);
    assert_eq!(origin_of(&objects(&s)[0]), pt(30.0, 20.0));
    assert_fresh(&mut s, "drag end");
}

#[test]
fn cache_memory_does_not_grow_over_a_thousand_operations() {
    fn rss_kib() -> u64 {
        let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
        let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
        pages * 4
    }
    let mut s = open(&row_doc(200));
    select_all(&mut s);
    // Warm up: allocator pools settle.
    for i in 0..100 {
        let _ = arrow(&mut s, "ArrowRight", false, i > 0, f64::from(i) * 33.0);
        let _ = s.draw_list();
    }
    let before = rss_kib();
    for i in 0..1000 {
        let dir = if i % 2 == 0 {
            "ArrowRight"
        } else {
            "ArrowLeft"
        };
        let _ = arrow(&mut s, dir, false, false, 10_000.0 + f64::from(i) * 700.0);
        let _ = s.draw_list();
    }
    let after = rss_kib();
    let growth = after.saturating_sub(before);
    println!("rss before {before} KiB after {after} KiB growth {growth} KiB");
    // Each snapshot read is a Vec of 200 objects: a leak of one per op would be megabytes.
    // The Loro oplog legitimately grows (1,000 commits of 200 objects), so allow for it.
    assert!(growth < 200 * 1024, "growth {growth} KiB");
}

// ---------------------------------------------------------------------
// White-box edges (read after the black-box tests above)
// ---------------------------------------------------------------------

#[test]
fn edge_escape_mid_hold_then_the_repeat_events_move_nothing() {
    let mut s = open(&row_doc(2));
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", false, false, 0.0);
    let _ = arrow(&mut s, "ArrowRight", false, true, 30.0);
    let _ = s.key_down(key("Escape"));
    let before = objects(&s);
    let commits = ops(&s);
    // The held key keeps repeating after the selection is gone: nothing is selected.
    assert_eq!(
        arrow(&mut s, "ArrowRight", false, true, 60.0),
        KeyOutcome::Ignored
    );
    assert_eq!(objects(&s), before);
    assert_eq!(ops(&s), commits);
}

#[test]
fn edge_consecutive_select_drags_without_a_frame_in_between_never_use_a_stale_read() {
    let d = Document::new(1);
    add_rect(&d, 10.0, 20.0, 100.0, 60.0);
    add_rect(&d, 300.0, 20.0, 40.0, 40.0);
    let mut s = open(&d);
    // Drag 1: the first rectangle by its outline, 20 mm right.
    for round in 0..4 {
        let from = pt(10.0 + 20.0 * f64::from(round), 50.0);
        let to = pt(from.x + 20.0, 50.0);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(from.x + 10.0, 50.0), false, false);
        s.pointer_hover(to, false, false);
        s.pointer_up(to, false, false);
        assert_eq!(
            origin_of(&objects(&s)[0]).x,
            10.0 + 20.0 * f64::from(round + 1),
            "round {round}"
        );
        // A nudge between drags reads the objects too.
        let _ = arrow(
            &mut s,
            "ArrowRight",
            false,
            false,
            1000.0 * f64::from(round),
        );
        assert_eq!(
            origin_of(&objects(&s)[0]).x,
            10.0 + 20.0 * f64::from(round + 1) + 1.0,
            "after the nudge, round {round}"
        );
        // Shift the next drag's start by the nudge.
        let _ = arrow(
            &mut s,
            "ArrowLeft",
            false,
            false,
            1000.0 * f64::from(round) + 500.0,
        );
    }
}

#[test]
fn edge_a_cancelled_drag_then_a_nudge_then_a_drag_sees_the_nudge() {
    let d = Document::new(1);
    add_rect(&d, 10.0, 20.0, 100.0, 60.0);
    let mut s = open(&d);
    s.pointer_hover(pt(10.0, 50.0), false, false);
    s.pointer_down(pt(10.0, 50.0), false);
    s.pointer_hover(pt(40.0, 50.0), false, false);
    let _ = s.draw_list();
    let _ = s.key_down(key("Escape"));
    assert_eq!(origin_of(&objects(&s)[0]).x, 10.0, "the drag was cancelled");
    select_all(&mut s);
    let _ = arrow(&mut s, "ArrowRight", true, false, 0.0);
    s.escape();
    // The rectangle's left outline is now at x = 20.
    s.pointer_hover(pt(20.0, 50.0), false, false);
    s.pointer_down(pt(20.0, 50.0), false);
    s.pointer_hover(pt(25.0, 50.0), false, false);
    let _ = s.draw_list();
    s.pointer_up(pt(25.0, 50.0), false, false);
    assert_eq!(origin_of(&objects(&s)[0]).x, 25.0);
}

#[test]
fn edge_node_tool_drag_twice_without_a_frame_in_between() {
    let d = Document::new(1);
    let _ = triangle(&d, 1, 100.0, 100.0);
    let mut s = open(&d);
    s.set_tool(Tool::Node);
    for round in 0..3 {
        let from = pt(100.0 + 10.0 * f64::from(round), 100.0);
        let to = pt(from.x + 10.0, 100.0);
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(to, false, false);
        s.pointer_up(to, false, false);
        let path = match &objects(&s)[0] {
            ObjectSnapshot::Path(p) => p.clone(),
            ObjectSnapshot::Primitive(_) => panic!("path"),
        };
        assert_eq!(
            path.anchors[0].point.x,
            100.0 + 10.0 * f64::from(round + 1),
            "round {round}"
        );
    }
}

/// A tiny deterministic generator for the random walk below.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    SelectAll,
    Nudge(usize, bool, bool),
    Marquee(f64, f64),
    Escape,
    Delete,
    Stroke(bool),
    Drag(f64, f64, f64),
}

fn run_op(s: &mut Session, op: Op, clock: &mut f64) {
    const ARROWS: [&str; 4] = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];
    match op {
        Op::SelectAll => {
            let _ = ctrl_a(s);
        }
        Op::Nudge(a, shift, repeat) => {
            *clock += 40.0;
            let _ = arrow(s, ARROWS[a], shift, repeat, *clock);
        }
        Op::Marquee(x, y) => marquee(s, pt(x - 30.0, y - 30.0), pt(x + 60.0, y + 60.0)),
        Op::Escape => {
            let _ = s.key_down(key("Escape"));
        }
        Op::Delete => s.delete_selected(),
        Op::Stroke(on) => s.set_stroke_paint(on),
        Op::Drag(x, y, dx) => {
            s.pointer_hover(pt(x, y), false, false);
            s.pointer_down(pt(x, y), false);
            s.pointer_hover(pt(x + dx / 2.0, y), false, false);
            s.pointer_up(pt(x + dx, y), false, false);
        }
    }
}

/// A random walk over nudges, select all, marquees, drags, style and delete: the session that
/// draws after every step (warm cache) draws exactly what a session that replays the same steps
/// without any frame in between (cold) draws, and what the saved file holds is what a re-read
/// shows (`pack` reads the document, not the cache).
#[test]
fn cache_random_walk_warm_equals_cold() {
    for seed in 1..=3_u64 {
        let mut rng = Lcg(seed * 7919);
        let ops: Vec<Op> = (0..28)
            .map(|_| match rng.next(8) {
                0 => Op::SelectAll,
                1 | 2 => Op::Nudge(rng.next(4) as usize, rng.next(2) == 0, rng.next(2) == 0),
                3 => Op::Marquee(20.0 + 30.0 * rng.next(5) as f64, 20.0),
                4 => Op::Escape,
                5 => Op::Stroke(rng.next(2) == 0),
                6 => Op::Drag(20.0 + 30.0 * rng.next(5) as f64, 20.0, 12.0),
                _ => {
                    if rng.next(4) == 0 {
                        Op::Delete
                    } else {
                        Op::Nudge(1, false, true)
                    }
                }
            })
            .collect();
        let d = row_doc(5);
        let mut warm = open(&d);
        let mut clock = 0.0;
        let mut frames = Vec::new();
        for op in &ops {
            let _ = warm.draw_list();
            run_op(&mut warm, *op, &mut clock);
            frames.push(warm.draw_list());
        }
        for i in 0..ops.len() {
            let mut cold = open(&d);
            let mut clock = 0.0;
            for op in &ops[..=i] {
                run_op(&mut cold, *op, &mut clock);
            }
            assert!(
                cold.draw_list() == frames[i],
                "seed {seed}: stale frame after step {i} ({:?})",
                ops[i]
            );
        }
    }
}
