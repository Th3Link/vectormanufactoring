//! `Session`-level tests of `specs/object-transform-refinements/
//! specification.md`: commit counts (one per drag or confirmed entry, none
//! for a wobble under the 3 px dead zone), Shift reveal without pointer
//! movement, the double-click dispatch and the typed entry's lifecycle,
//! cursors, hints and readouts, save and reopen of a typed rotation, and
//! that a skew writes no `rotation`. The arithmetic itself is tested in
//! `curvyo-ui-core/tests/acceptance_object_transform_refinements.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    AnchorId, CURRENT_FORMAT_VERSION, Document, Length, NewAnchor, ObjectSnapshot, Point,
    RectBounds, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

/// Screen pixels per millimetre of a fresh session (96 dpi at 100 %).
const SCALE: f64 = 96.0 / 25.4;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn px(pixels: f64) -> f64 {
    pixels / SCALE
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
            NewAnchor::corner(AnchorId::new(1, 4), pt(10.0, 80.0)),
        ],
        true,
    );
    document
}

/// A session with the document's one object selected under the Select tool
/// (a click on its left edge).
fn selected(document: &Document) -> Session {
    let bytes = pack(document, "0.1.0").expect("pack");
    let mut session = Session::open(2, &bytes).expect("open");
    session.set_tool(Tool::Select);
    click(&mut session, pt(10.0, 50.0));
    session
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

/// A press, a move and a release; Shift and Ctrl are held from the move.
/// A whole double-click: the first press and release, then the second
/// press's `double_click`.
fn dbl(session: &mut Session, at: Point, shift: bool, ctrl: bool) {
    session.pointer_hover(at, shift, ctrl);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, ctrl);
    session.double_click(at, shift, ctrl);
}

fn drag(session: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, shift, ctrl);
    session.pointer_up(to, shift, ctrl);
}

fn object(session: &Session) -> ObjectSnapshot {
    let bytes = session.pack("0.1.0").expect("pack");
    let document = unpack(99, &bytes).expect("unpack");
    document.object(document.object_ids()[0]).expect("object")
}

/// How many Loro changes the session's document holds: a drag or an
/// entry that writes exactly once adds exactly one.
fn changes(session: &Session) -> usize {
    let bytes = session.pack("0.1.0").expect("pack");
    let document = unpack(99, &bytes).expect("unpack");
    let loro = loro::LoroDoc::new();
    loro.import(&document.export_loro_snapshot().expect("export"))
        .expect("import");
    loro.len_changes()
}

fn rotation_deg(session: &Session) -> f64 {
    object(session).rotation().as_radians().to_degrees()
}

/// Where the rect's handles are, in millimetres (box (10, 20)-(110, 80)).
const CORNER_OFFSET_PX: f64 = 32.0 / std::f64::consts::SQRT_2;

fn ne_rotate() -> Point {
    pt(110.0 + px(CORNER_OFFSET_PX), 20.0 - px(CORNER_OFFSET_PX))
}

fn top_side_rotate() -> Point {
    pt(60.0, 20.0 - px(32.0))
}

fn se_resize() -> Point {
    pt(110.0, 80.0)
}

fn top_skew() -> Point {
    pt(60.0, 20.0 - px(16.0))
}

// ---------------------------------------------------------------------
// Dead zone and commit counts: criteria 3, 41
// ---------------------------------------------------------------------

#[test]
fn a_press_and_wobble_under_three_pixels_writes_nothing_for_every_drag() {
    let mut rect = selected(&rect_document());
    let mut path = selected(&path_document());
    for session in [&mut rect, &mut path] {
        let before = changes(session);
        let snapshot = object(session);
        let mut starts = vec![pt(60.0, 50.0), se_resize(), ne_rotate()];
        if matches!(snapshot, ObjectSnapshot::Path(_)) {
            starts.push(top_skew());
        }
        for start in starts {
            drag(
                session,
                start,
                start.translated(Vec2::new(px(2.0), px(1.5))),
                false,
                false,
            );
            assert_eq!(changes(session), before, "a 2.5 px wobble from {start:?}");
            assert_eq!(object(session), snapshot);
        }
    }
}

#[test]
fn a_drag_past_the_dead_zone_is_exactly_one_commit_for_every_gesture() {
    for path in [false, true] {
        let document = if path {
            path_document()
        } else {
            rect_document()
        };
        let mut gestures: Vec<(Point, Point)> = vec![
            (pt(60.0, 50.0), pt(80.0, 55.0)),
            (se_resize(), pt(130.0, 95.0)),
            (ne_rotate(), pt(150.0, 40.0)),
        ];
        if path {
            gestures.push((top_skew(), pt(80.0, 20.0 - px(16.0))));
        }
        for (from, to) in gestures {
            let mut session = selected(&document);
            let before = changes(&session);
            let snapshot = object(&session);
            drag(&mut session, from, to, false, false);
            assert_ne!(object(&session), snapshot, "{from:?} wrote");
            assert_eq!(changes(&session), before + 1, "one commit for {from:?}");
        }
    }
}

// ---------------------------------------------------------------------
// Shift reveal without pointer movement: criterion 6
// ---------------------------------------------------------------------

#[test]
fn shift_reveals_the_side_rotate_handles_with_no_pointer_movement() {
    let mut session = selected(&rect_document());
    session.pointer_hover(top_side_rotate(), false, false);
    assert_eq!(
        session.cursor_hint(),
        "default",
        "no handle there without Shift"
    );
    session.modifiers_changed(true, false);
    assert_eq!(
        session.cursor_hint(),
        "rotate",
        "revealed in the frame Shift goes down"
    );
    session.modifiers_changed(false, false);
    assert_eq!(
        session.cursor_hint(),
        "default",
        "gone in the frame it goes up"
    );
}

// ---------------------------------------------------------------------
// Cursors and hints: criteria 4, 10, 48, 54
// ---------------------------------------------------------------------

#[test]
fn cursors_and_hints_follow_the_handle_under_the_pointer() {
    let mut session = selected(&path_document());
    let probe = |session: &mut Session, at: Point| {
        session.pointer_hover(at, false, false);
        (session.cursor_hint(), session.handle_hint())
    };
    assert_eq!(
        probe(&mut session, pt(60.0, 50.0)),
        ("move".into(), "move".into())
    );
    assert_eq!(
        probe(&mut session, ne_rotate()),
        ("rotate".into(), "rotate-corner".into())
    );
    assert_eq!(
        probe(&mut session, se_resize()),
        ("resize:45.0".into(), "resize-corner".into())
    );
    assert_eq!(
        probe(&mut session, pt(60.0, 20.0)),
        ("resize:90.0".into(), "resize-edge".into())
    );
    assert_eq!(
        probe(&mut session, top_skew()),
        ("skew:0.0".into(), "skew".into())
    );
    assert_eq!(
        probe(&mut session, pt(10.0 - px(16.0), 50.0)),
        ("skew:90.0".into(), "skew-y".into())
    );
    session.modifiers_changed(true, false);
    session.pointer_hover(top_side_rotate(), true, false);
    assert_eq!(session.cursor_hint(), "rotate");
    assert_eq!(session.handle_hint(), "rotate-side");
    session.modifiers_changed(false, false);
    assert_eq!(
        probe(&mut session, pt(900.0, 900.0)),
        ("default".into(), String::new())
    );
}

#[test]
fn a_polygons_resize_hint_has_no_proportional_line() {
    let document = Document::new(1);
    let _ = document.create_polygon(
        curvyo_document_core::StarFrame {
            center: pt(60.0, 50.0),
            radius: Length::from_mm(40.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        curvyo_document_core::PointCount::new(6).unwrap(),
    );
    let bytes = pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    // A vertex of the hexagon (angle 0) is on its outline: select by it.
    click(&mut session, pt(100.0, 50.0));
    session.pointer_hover(pt(100.0, 10.0), false, false);
    assert_eq!(session.handle_hint(), "resize-corner-uniform");
}

#[test]
fn the_skew_handle_cursor_turns_with_a_rotated_path() {
    let mut session = selected(&path_document());
    // Rotate by typing 30 degrees.
    session.pointer_hover(ne_rotate(), false, false);
    dbl(&mut session, ne_rotate(), false, false);
    let _ = session.commit_transform_entry("30", "", 0);
    let bytes = session.pack("0.1.0").unwrap();
    let document = unpack(99, &bytes).unwrap();
    assert!(
        (document
            .object(document.object_ids()[0])
            .unwrap()
            .rotation()
            .as_radians()
            .to_degrees()
            - 30.0)
            .abs()
            < 1e-9
    );
    // Find the top skew handle on the rotated box by scanning for the cursor.
    let mut found = None;
    for yi in 0..=1200 {
        for xi in 0..=1200 {
            let at = pt(f64::from(xi) * 0.1, f64::from(yi) * 0.1);
            session.pointer_hover(at, false, false);
            let hint = session.cursor_hint();
            if hint.starts_with("skew:") {
                found = Some(hint);
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let hint = found.expect("a skew handle exists");
    assert!(
        hint == "skew:30.0" || hint == "skew:120.0",
        "top/bottom skew along u (30), left/right along v (120): {hint}"
    );
}

// ---------------------------------------------------------------------
// Double-click dispatch and the entry lifecycle: criteria 3, 18-24, 31, 32, 49
// ---------------------------------------------------------------------

#[test]
fn a_double_click_inside_the_box_only_hints_and_on_a_handle_opens_the_entry() {
    // `unified-object-editing` criteria 32, 33: a primitive has no tool of its
    // own to hand off to, so the centre handle and the inside of the box change
    // nothing (the hint is asked for); a handle opens its entry.
    let mut session = selected(&rect_document());
    let before = session.pack("0.1.0").expect("pack");
    dbl(&mut session, pt(60.0, 50.0), false, false);
    assert_eq!(session.tool(), Tool::Select, "centre handle: no handoff");
    assert!(
        session.move_entry().is_some(),
        "the drawn centre handle opens the typed move (`edit-interaction-polish` criterion 15)"
    );

    let mut session = selected(&rect_document());
    dbl(&mut session, pt(40.0, 60.0), false, false);
    assert_eq!(session.tool(), Tool::Select, "anywhere inside the box");
    assert_eq!(
        session.pack("0.1.0").expect("pack"),
        before,
        "nothing written"
    );

    for at in [ne_rotate(), se_resize()] {
        let mut session = selected(&rect_document());
        dbl(&mut session, at, false, false);
        assert_eq!(session.tool(), Tool::Select, "{at:?}: no handoff");
        assert!(
            session.transform_entry().is_some(),
            "{at:?}: the entry opens"
        );
    }

    let mut session = selected(&path_document());
    dbl(&mut session, top_skew(), false, false);
    assert_eq!(session.tool(), Tool::Select, "skew handle: no handoff");
    assert_eq!(
        session.transform_entry().map(|entry| entry.kind),
        Some("skew"),
        "skew handle: the skew entry opens (`edit-interaction-polish` criterion 9)"
    );
}

#[test]
fn the_angle_entry_commits_one_change_and_survives_save_and_reopen() {
    let mut session = selected(&rect_document());
    let before = changes(&session);
    dbl(&mut session, ne_rotate(), false, false);
    let view = session.transform_entry().expect("entry");
    assert_eq!(view.kind, "angle");
    assert_eq!(view.fields.len(), 1);
    assert_eq!(view.fields[0].prefill, "0");
    assert_eq!(changes(&session), before, "opening writes nothing");

    assert_eq!(
        session.commit_transform_entry("45", "", 0),
        curvyo_ui_core::EntryOutcome::Committed
    );
    assert_eq!(changes(&session), before + 1, "one commit");
    assert!(session.transform_entry().is_none(), "the entry closes");
    assert!((rotation_deg(&session) - 45.0).abs() < 1e-9);

    // Save, close, reopen: the rotation is what was typed, still a rect.
    let bytes = session.pack("0.1.0").unwrap();
    let reopened = Session::open(3, &bytes).unwrap();
    assert!((rotation_deg(&reopened) - 45.0).abs() < 1e-9);
    let ObjectSnapshot::Primitive(p) = object(&reopened) else {
        panic!("still a primitive")
    };
    assert!(matches!(p.shape, curvyo_document_core::Shape::Rect { .. }));
    assert_eq!(CURRENT_FORMAT_VERSION, 5, "no format change");
}

#[test]
fn an_untouched_equal_or_invalid_entry_writes_nothing() {
    let mut session = selected(&rect_document());
    let before = changes(&session);
    dbl(&mut session, ne_rotate(), false, false);
    assert_eq!(
        session.commit_transform_entry("0", "", 0),
        curvyo_ui_core::EntryOutcome::Unchanged
    );
    assert!(session.transform_entry().is_none());
    dbl(&mut session, ne_rotate(), false, false);
    assert!(matches!(
        session.commit_transform_entry("abc", "", 0),
        curvyo_ui_core::EntryOutcome::Invalid { field: 0, .. }
    ));
    assert!(
        session.transform_entry().is_some(),
        "an invalid value keeps it open"
    );
    session.cancel_transform_entry();
    assert!(session.transform_entry().is_none());
    assert_eq!(changes(&session), before);
}

#[test]
fn the_size_entry_commits_one_change_with_the_hand_drags_fixed_point() {
    let mut session = selected(&rect_document());
    let before = changes(&session);
    dbl(&mut session, se_resize(), false, false);
    let view = session.transform_entry().expect("entry");
    assert_eq!(view.kind, "size");
    assert_eq!(
        view.fields
            .iter()
            .map(|f| f.prefill.as_str())
            .collect::<Vec<_>>(),
        ["100.0", "60.0"]
    );
    assert!(!view.linked);
    session.commit_transform_entry("150", "60.0", 0);
    assert_eq!(changes(&session), before + 1);
    let ObjectSnapshot::Primitive(p) = object(&session) else {
        panic!()
    };
    let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!((bounds.origin.x - 10.0).abs() < 1e-9 && (bounds.origin.y - 20.0).abs() < 1e-9);
    assert!((bounds.width.as_mm() - 150.0).abs() < 1e-9);
}

#[test]
fn ctrl_at_the_second_press_links_width_and_height() {
    let mut session = selected(&rect_document());
    dbl(&mut session, se_resize(), false, true);
    let view = session.transform_entry().expect("entry");
    assert!(view.linked);
    assert_eq!(
        session.transform_entry_linked(0, "200").as_deref(),
        Some("120.0")
    );
}

#[test]
fn every_way_of_leaving_closes_the_entry_without_writing() {
    // (label, action) pairs; each starts from a freshly opened angle entry.
    type Action = fn(&mut Session);
    let actions: Vec<(&str, Action)> = vec![
        ("escape", |s| {
            let _ = s.escape();
        }),
        ("tool switch", |s| s.set_tool(Tool::Pen)),
        ("press elsewhere", |s| {
            s.pointer_hover(pt(900.0, 900.0), false, false);
            s.pointer_down(pt(900.0, 900.0), false);
            s.pointer_up(pt(900.0, 900.0), false, false);
        }),
        ("delete", Session::delete_selected),
        ("object to path", Session::convert_selected_to_paths),
        ("stroke switch", |s| s.set_scale_stroke_width(true)),
        ("cancel", Session::cancel_transform_entry),
    ];
    for (label, action) in actions {
        let mut session = selected(&rect_document());
        let before = changes(&session);
        dbl(&mut session, ne_rotate(), false, false);
        assert!(session.transform_entry().is_some(), "{label}: opened");
        action(&mut session);
        assert!(session.transform_entry().is_none(), "{label}: closed");
        // Convert/delete write by themselves; everything else writes nothing.
        if !matches!(label, "delete" | "object to path") {
            assert_eq!(changes(&session), before, "{label}: nothing written");
        }
    }
}

#[test]
fn the_press_that_closes_the_entry_is_processed_normally() {
    // A second object: pressing it while the entry is open selects it.
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(60.0),
    });
    let second = document.create_rect(RectBounds {
        origin: pt(300.0, 20.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(40.0),
    });
    let bytes = pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    click(&mut session, pt(10.0, 50.0));
    dbl(&mut session, ne_rotate(), false, false);
    assert!(session.transform_entry().is_some());
    click(&mut session, pt(300.0, 40.0));
    assert!(session.transform_entry().is_none());
    // The second object is now the selection: a double-click on its Se
    // resize handle opens a size entry for it.
    dbl(&mut session, pt(340.0, 60.0), false, false);
    let view = session.transform_entry().expect("second object's entry");
    assert_eq!(view.fields[0].prefill, "40.0");
    let _ = second;
}

// ---------------------------------------------------------------------
// Skew and readouts: criteria 35, 40, 44-46, 51
// ---------------------------------------------------------------------

#[test]
fn a_skew_writes_anchors_only_and_shows_its_readout() {
    let mut session = selected(&path_document());
    let before = object(&session);
    let before_changes = changes(&session);
    let start = top_skew();
    session.pointer_hover(start, false, false);
    session.pointer_down(start, false);
    let to = pt(start.x + px(40.0), start.y);
    session.pointer_hover(to, false, false);
    let readout = session.live_readout().expect("skew readout").text;
    assert!(
        readout.starts_with("Skew x +"),
        "x skew from the top, positive to the right: {readout}"
    );
    assert!(readout.ends_with('°'));
    let left = pt(start.x - px(40.0), start.y);
    session.pointer_hover(left, false, false);
    let readout = session.live_readout().expect("skew readout").text;
    assert!(
        readout.starts_with("Skew x \u{2212}"),
        "a real minus: {readout}"
    );
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
    let after = object(&session);
    assert_ne!(after, before);
    assert_eq!(changes(&session), before_changes + 1);
    assert_eq!(after.rotation(), before.rotation(), "rotation untouched");
    let (ObjectSnapshot::Path(a), ObjectSnapshot::Path(b)) = (&after, &before) else {
        panic!()
    };
    assert_eq!(a.stroke_width, b.stroke_width);
    assert_eq!(a.closed, b.closed);
    assert_eq!(a.anchors.len(), b.anchors.len());
}

#[test]
fn a_skew_under_ctrl_reads_a_snapped_angle() {
    let mut session = selected(&path_document());
    let start = top_skew();
    session.pointer_hover(start, false, false);
    session.pointer_down(start, false);
    // Lever = box height 60 mm: 22.5 degrees is a 24.85 mm displacement.
    let to = pt(start.x + 60.0 * 20.0_f64.to_radians().tan(), start.y);
    session.pointer_hover(to, false, true);
    assert_eq!(session.live_readout().unwrap().text, "Skew x +22.5°");
    session.escape();
    assert!(session.live_readout().is_none());
}

#[test]
fn a_rotate_readout_shows_one_decimal_on_a_snap_stop() {
    let mut session = selected(&rect_document());
    let handle = ne_rotate();
    session.pointer_hover(handle, false, false);
    session.pointer_down(handle, false);
    let center = pt(60.0, 50.0);
    let v = center.vector_to(handle);
    let angle = v.y.atan2(v.x) + 24.0_f64.to_radians();
    let to = pt(
        center.x + v.length() * angle.cos(),
        center.y + v.length() * angle.sin(),
    );
    session.pointer_hover(to, false, true);
    assert_eq!(session.live_readout().unwrap().text, "22.5°");
    session.escape();
}

#[test]
fn a_primitive_never_skews() {
    let mut session = selected(&rect_document());
    let before = object(&session);
    let before_changes = changes(&session);
    drag(
        &mut session,
        top_skew(),
        pt(100.0, 20.0 - px(16.0)),
        false,
        false,
    );
    assert_eq!(object(&session), before);
    assert_eq!(changes(&session), before_changes);
    assert_eq!(session.tool(), Tool::Select);
}

#[test]
fn double_clicking_the_n_edge_of_an_unselected_rectangle_only_hints() {
    let bytes = pack(&rect_document(), "0.1.0").expect("pack");
    let mut session = Session::open(2, &bytes).expect("open");
    session.set_tool(Tool::Select);
    // The top-edge midpoint is where the N resize handle will appear: the
    // first click selects, the second press must hand off.
    let on_edge = pt(60.0, 20.0);
    session.pointer_hover(on_edge, false, false);
    session.pointer_down(on_edge, false);
    session.pointer_up(on_edge, false, false);
    assert!(session.double_click(on_edge, false, false), "the edit hint");
    assert_eq!(session.tool(), Tool::Select);
    assert!(session.transform_entry().is_none());
}
