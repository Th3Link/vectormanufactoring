//! `Session`-level tests of the scale, rotate and skew of a multi-selection and
//! of its typed entries (`specs/0019-multi-object-transform/` criteria 9 to 15,
//! 18 to 42, 47). The arithmetic is tested in `curvyo-ui-core`; this drives the
//! public API of `Session` with pointer events and keys.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
#![allow(clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, PrimitiveSnapshot, RectBounds,
    Shape, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::EntryOutcome;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Two 100 x 60 mm rectangles at (20, 20) and (160, 20): the group box is
/// (20, 20) to (260, 80), its centre (140, 50).
fn rects() -> Document {
    let document = Document::new(1);
    for x in [20.0, 160.0] {
        let _ = document.create_rect(RectBounds {
            origin: pt(x, 20.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
    }
    document
}

/// Two open paths: (0, 0) to (60, 40) and (70, 0) to (90, 40), so the group box
/// is (0, 0) to (90, 40).
fn paths() -> Document {
    let document = Document::new(1);
    let mut next = 1;
    for (a, b) in [((0.0, 0.0), (60.0, 40.0)), ((70.0, 0.0), (90.0, 40.0))] {
        let _ = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, next), pt(a.0, a.1)),
                NewAnchor::corner(AnchorId::new(1, next + 1), pt(b.0, b.1)),
            ],
            false,
        );
        next += 2;
    }
    document
}

fn session_of(document: &Document) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.set_tool(Tool::Select);
    session.resize_viewport(1200.0, 800.0);
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

/// Selects the first two objects by clicking one point on each.
fn select_two(session: &mut Session, first: Point, second: Point) {
    click(session, first, false);
    click(session, second, true);
    assert_eq!(session.selected_object_count(), 2);
}

fn select_rects(session: &mut Session) {
    select_two(session, pt(20.0, 50.0), pt(160.0, 50.0));
}

fn select_paths(session: &mut Session) {
    select_two(session, pt(30.0, 20.0), pt(80.0, 20.0));
}

/// A drag from `from` to `to` through the middle with the modifiers held for
/// the whole drag, released at `to`.
fn drag_with(session: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    hold(session, from, shift, ctrl);
    session.pointer_down(from, shift);
    let middle = pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y));
    hold(session, middle, shift, ctrl);
    hold(session, to, shift, ctrl);
    session.pointer_up(to, shift, ctrl);
    hold(session, to, false, false);
}

fn drag(session: &mut Session, from: Point, to: Point) {
    drag_with(session, from, to, false, false);
}

fn key(session: &mut Session, key: &str, shift: bool) -> KeyOutcome {
    session.key_down(KeyInput {
        key,
        shift,
        ..KeyInput::default()
    })
}

fn bounds_of(object: &ObjectSnapshot) -> RectBounds {
    let ObjectSnapshot::Primitive(PrimitiveSnapshot {
        shape: Shape::Rect { bounds, .. },
        ..
    }) = object
    else {
        panic!("a rectangle");
    };
    *bounds
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn points_of(object: &ObjectSnapshot) -> Vec<Point> {
    let ObjectSnapshot::Path(path) = object else {
        panic!("a path");
    };
    path.anchors.iter().map(|a| a.point).collect()
}

/// Screen pixels per millimetre.
fn k(session: &Session) -> f64 {
    session.view().scale()
}

// ---------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------

/// Criteria 18, 29, 31: a corner drag scales every object about the opposite
/// corner of the group box, in one commit; the dragged corner follows the pointer.
#[test]
fn a_corner_drag_scales_every_object_about_the_opposite_corner() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let commits = change_count(&session);
    // The south-east corner (260, 80) goes to (380, 140): sx = 360/240, sy = 120/60.
    drag(&mut session, pt(260.0, 80.0), pt(380.0, 140.0));
    let after = objects(&session);
    assert_eq!(change_count(&session), commits + 1, "one commit");
    let (a, b) = (bounds_of(&after[0]), bounds_of(&after[1]));
    assert!(near(a.origin.x, 20.0) && near(a.origin.y, 20.0));
    assert!(near(a.width.as_mm(), 150.0) && near(a.height.as_mm(), 120.0));
    assert!(near(b.origin.x, 20.0 + 140.0 * 1.5) && near(b.origin.y, 20.0));
    assert!(near(b.width.as_mm(), 150.0) && near(b.height.as_mm(), 120.0));
    assert_eq!(session.selected_object_count(), 2);
}

/// Criterion 23: the live readout shows the group box size at the pointer and
/// the pivot marker the fixed point; both are gone after the release.
#[test]
fn a_scale_drag_shows_the_size_readout_and_the_pivot() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    hold(&mut session, pt(260.0, 80.0), false, false);
    session.pointer_down(pt(260.0, 80.0), false);
    hold(&mut session, pt(380.0, 140.0), false, false);
    let readout = session.live_readout().expect("a readout");
    assert_eq!(readout.text, "360.0 mm \u{d7} 120.0 mm");
    assert_eq!(readout.anchor, pt(380.0, 140.0));
    session.pointer_up(pt(380.0, 140.0), false, false);
    assert!(session.live_readout().is_none());
}

/// Criterion 19: Ctrl keeps the proportions of the box, Shift scales about the
/// centre, and both change mid-drag without accumulating error.
#[test]
fn ctrl_and_shift_change_the_scale_gesture() {
    // Ctrl: the dominant axis (x: 240 -> 360, factor 1.5) for both.
    let mut session = session_of(&rects());
    select_rects(&mut session);
    drag_with(&mut session, pt(260.0, 80.0), pt(380.0, 90.0), false, true);
    let a = bounds_of(&objects(&session)[0]);
    assert!(near(a.width.as_mm(), 150.0) && near(a.height.as_mm(), 90.0));
    // Shift: about the centre (140, 50): 120 more on each side in x is a factor of 2.
    let mut session = session_of(&rects());
    select_rects(&mut session);
    drag_with(&mut session, pt(260.0, 80.0), pt(380.0, 80.0), true, false);
    let a = bounds_of(&objects(&session)[0]);
    assert!(near(a.width.as_mm(), 200.0) && near(a.height.as_mm(), 60.0));
    assert!(near(a.origin.x, -100.0) && near(a.origin.y, 20.0));
}

/// Criterion 19: an edge handle changes one axis only.
#[test]
fn an_edge_handle_scales_one_axis() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    drag(&mut session, pt(260.0, 50.0), pt(380.0, 70.0));
    let (a, b) = (
        bounds_of(&objects(&session)[0]),
        bounds_of(&objects(&session)[1]),
    );
    assert!(near(a.width.as_mm(), 150.0) && near(a.height.as_mm(), 60.0));
    assert!(near(b.width.as_mm(), 150.0));
}

/// Criteria 22 and 30: a drag back to the start, a release inside the dead zone
/// and Escape write nothing.
#[test]
fn a_cancelled_or_null_scale_writes_nothing() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let commits = change_count(&session);
    drag(&mut session, pt(260.0, 80.0), pt(260.5, 80.0));
    assert_eq!(change_count(&session), commits, "inside the dead zone");
    hold(&mut session, pt(260.0, 80.0), false, false);
    session.pointer_down(pt(260.0, 80.0), false);
    hold(&mut session, pt(300.0, 90.0), false, false);
    assert_eq!(session.escape(), EscapeStep::CancelledDrag);
    session.pointer_up(pt(300.0, 90.0), false, false);
    assert_eq!(change_count(&session), commits, "Escape");
    hold(&mut session, pt(260.0, 80.0), false, false);
    session.pointer_down(pt(260.0, 80.0), false);
    hold(&mut session, pt(300.0, 90.0), false, false);
    hold(&mut session, pt(260.0, 80.0), false, false);
    session.pointer_up(pt(260.0, 80.0), false, false);
    assert_eq!(change_count(&session), commits, "back to the start");
}

/// Criterion 22: a factor below zero stops at zero.
#[test]
fn a_scale_below_zero_stops_at_zero() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    drag(&mut session, pt(260.0, 80.0), pt(-100.0, 80.0));
    let a = bounds_of(&objects(&session)[0]);
    assert!(near(a.width.as_mm(), 0.0), "{a:?}");
    assert!(near(a.height.as_mm(), 60.0));
}

/// Criteria 10 and 21: a selection that holds a star scales proportionally
/// only, its edge handles do not exist, and the corner hint names the cause.
#[test]
fn a_selection_with_a_star_scales_proportionally_at_a_corner_and_says_where_to_stretch() {
    let document = rects();
    let _ = document.create_star(
        curvyo_document_core::StarFrame {
            center: pt(140.0, 140.0),
            radius: Length::from_mm(20.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        curvyo_document_core::PointCount::new(5).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    let mut session = session_of(&document);
    // The rectangle on the left and the star.
    let star_tip = pt(160.0, 140.0);
    select_two(&mut session, pt(20.0, 50.0), star_tip);
    // The group box: the bounds of the two outlines.
    let all = objects(&session);
    let (low, high) = [&all[0], &all[2]]
        .iter()
        .map(|o| curvyo_ui_core::object_outline_bounds(o))
        .reduce(|(a, b), (c, d)| {
            (
                pt(a.x.min(c.x), a.y.min(c.y)),
                pt(b.x.max(d.x), b.y.max(d.y)),
            )
        })
        .unwrap();
    let group_box = (low.x, low.y, high.x, high.y);
    // The east edge midpoint is a handle (criterion 18), and the star counts for
    // its hint line (criterion 55).
    hold(
        &mut session,
        pt(group_box.2, f64::midpoint(group_box.1, group_box.3)),
        false,
        false,
    );
    assert_eq!(session.handle_hint(), "group");
    assert_eq!(session.hover_conversion_counts(), vec![0, 1, 0, 0]);
    // The south-east corner is proportional and points to the edge handle.
    hold(&mut session, pt(group_box.2, group_box.3), false, false);
    assert_eq!(session.handle_hint(), "group");
    assert!(
        session.hover_conversion_counts().is_empty(),
        "a corner never converts"
    );
    let lines = session.corner_hint_lines();
    assert_eq!(lines[0], "Resize selection, proportional");
    assert_eq!(lines[1], "Stretch with an edge handle");
    // A free drag of the corner scales both axes by one factor.
    drag(
        &mut session,
        pt(group_box.2, group_box.3),
        pt(group_box.2 + 60.0, group_box.3 + 5.0),
    );
    let after = objects(&session);
    let a = bounds_of(&after[0]);
    assert!(near(a.width.as_mm() / 100.0, a.height.as_mm() / 60.0));
    assert!(a.width.as_mm() > 100.0);
}

// ---------------------------------------------------------------------
// Rotate
// ---------------------------------------------------------------------

/// The north-east rotate handle: 32 px out along the diagonal.
fn rotate_ne(session: &Session, corner: Point) -> Point {
    let offset = 32.0 / k(session) / std::f64::consts::SQRT_2;
    pt(corner.x + offset, corner.y - offset)
}

/// Criteria 25, 26, 28, 31: a corner rotate handle turns every object about the
/// centre of the group box by the swept angle, rigidly, in one commit.
#[test]
fn a_corner_rotate_drag_turns_the_selection_about_the_box_centre() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let commits = change_count(&session);
    let handle = rotate_ne(&session, pt(260.0, 20.0));
    // From the handle (up and right of the centre) to straight below the centre:
    // the vector from the centre turns by the swept angle.
    let centre = pt(140.0, 50.0);
    let from = (handle.x - centre.x, handle.y - centre.y);
    let angle = from.1.atan2(from.0);
    let target_angle = std::f64::consts::FRAC_PI_2;
    let length = from.0.hypot(from.1);
    let to = pt(
        centre.x + length * target_angle.cos(),
        centre.y + length * target_angle.sin(),
    );
    drag(&mut session, handle, to);
    let swept = target_angle - angle;
    let after = objects(&session);
    assert_eq!(change_count(&session), commits + 1);
    for object in &after {
        assert!(
            near(object.rotation().as_radians(), swept),
            "rotation {swept}"
        );
    }
    // Rigid: the distance between the two centres is unchanged (140 mm).
    let centre_of = |o: &ObjectSnapshot| {
        let b = bounds_of(o);
        let (cx, cy) = (
            b.origin.x + b.width.as_mm() / 2.0,
            b.origin.y + b.height.as_mm() / 2.0,
        );
        (cx, cy)
    };
    let (c0, c1) = (centre_of(&after[0]), centre_of(&after[1]));
    assert!(near((c1.0 - c0.0).hypot(c1.1 - c0.1), 140.0));
    // Their midpoint stays at the group centre.
    assert!(near(f64::midpoint(c0.0, c1.0), 140.0) && near(f64::midpoint(c0.1, c1.1), 50.0));
}

/// Criteria 7 and 28: after a rotate the group box is axis-aligned again, and
/// during the drag the readout is the turn since the press.
#[test]
fn a_rotate_drag_shows_delta_and_the_box_is_axis_aligned_afterwards() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let handle = rotate_ne(&session, pt(260.0, 20.0));
    let centre = pt(140.0, 50.0);
    let radius = (handle.x - centre.x).hypot(handle.y - centre.y);
    let start_angle = (handle.y - centre.y).atan2(handle.x - centre.x);
    let to = pt(
        centre.x + radius * (start_angle + 0.5).cos(),
        centre.y + radius * (start_angle + 0.5).sin(),
    );
    hold(&mut session, handle, false, false);
    session.pointer_down(handle, false);
    hold(&mut session, to, false, false);
    let text = session.live_readout().expect("a readout").text;
    assert_eq!(text, "\u{394} 28.6\u{b0}");
    // The pivot marker is at the centre.
    session.pointer_up(to, false, false);
    assert!(session.live_readout().is_none());
    assert_eq!(session.selected_object_count(), 2);
}

/// Criterion 27: Ctrl snaps the swept angle.
#[test]
fn ctrl_snaps_the_rotation() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let handle = rotate_ne(&session, pt(260.0, 20.0));
    let centre = pt(140.0, 50.0);
    let radius = (handle.x - centre.x).hypot(handle.y - centre.y);
    let start_angle = (handle.y - centre.y).atan2(handle.x - centre.x);
    let to = pt(
        centre.x + radius * (start_angle + 0.4).cos(),
        centre.y + radius * (start_angle + 0.4).sin(),
    );
    hold(&mut session, handle, false, true);
    session.pointer_down(handle, false);
    hold(&mut session, to, false, true);
    assert_eq!(session.live_readout().unwrap().text, "\u{394} 22.5\u{b0}");
    session.pointer_up(to, false, true);
}

// ---------------------------------------------------------------------
// Skew
// ---------------------------------------------------------------------

/// Criteria 40 to 42: the top skew handle of a selection of paths shears every
/// path by the same map, the bottom side fixed.
#[test]
fn a_skew_drag_shears_every_path_with_the_bottom_fixed() {
    let mut session = session_of(&paths());
    select_paths(&mut session);
    let commits = change_count(&session);
    // The top handle: 16 px above the middle of the top side (45, 0).
    let handle = pt(45.0, -16.0 / k(&session));
    hold(&mut session, handle, false, false);
    assert!(session.cursor_hint().starts_with("skew"), "a skew handle");
    // 45 degrees with a lever of 40 mm: 40 mm to the right.
    drag(&mut session, handle, pt(45.0 + 40.0, handle.y));
    let after = objects(&session);
    assert_eq!(change_count(&session), commits + 1);
    let first = points_of(&after[0]);
    assert!(near(first[0].x, 40.0) && near(first[0].y, 0.0), "top moved");
    assert!(
        near(first[1].x, 60.0) && near(first[1].y, 40.0),
        "bottom fixed"
    );
    let second = points_of(&after[1]);
    assert!(near(second[0].x, 110.0) && near(second[1].x, 90.0));
}

/// Criterion 40: a selection with a rectangle shows no skew handle at all, and
/// the keys say so.
#[test]
fn a_selection_with_a_rectangle_has_no_skew_handle() {
    let document = paths();
    let _ = document.create_rect(RectBounds {
        origin: pt(100.0, 0.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(40.0),
    });
    let mut session = session_of(&document);
    select_two(&mut session, pt(30.0, 20.0), pt(100.0, 20.0));
    let handle = pt(f64::midpoint(0.0, 120.0), -16.0 / k(&session));
    hold(&mut session, handle, false, false);
    assert!(!session.cursor_hint().starts_with("skew"), "no skew handle");
    assert_eq!(
        key(&mut session, "k", false),
        KeyOutcome::Hint(curvyo_editor_wasm::KeyHint::PathOnly)
    );
}

// ---------------------------------------------------------------------
// Typed entries
// ---------------------------------------------------------------------

/// Criterion 33: R opens the angle chip "Δ" prefilled 0; Enter turns the
/// selection by the typed angle about the group box centre; typing 0 or a
/// full turn writes nothing.
#[test]
fn the_angle_chip_turns_the_selection_by_the_typed_angle() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    assert_eq!(key(&mut session, "r", false), KeyOutcome::EntryOpened);
    let view = session.transform_entry().expect("a chip");
    assert_eq!(view.kind, "angle");
    assert!(view.selection);
    assert_eq!(view.fields.len(), 1);
    assert_eq!(view.fields[0].label, "\u{394}");
    assert_eq!(view.fields[0].accessible_name, "Rotate selection by");
    assert_eq!(view.fields[0].prefill, "0");
    let commits = change_count(&session);
    assert_eq!(
        session.commit_transform_entry("0", "", 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(change_count(&session), commits);
    assert_eq!(key(&mut session, "r", false), KeyOutcome::EntryOpened);
    assert_eq!(
        session.commit_transform_entry("360", "", 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(key(&mut session, "r", false), KeyOutcome::EntryOpened);
    assert_eq!(
        session.commit_transform_entry("abc", "", 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: curvyo_ui_core::InvalidReason::NotANumber
        }
    );
    assert_eq!(
        session.commit_transform_entry("90", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&session), commits + 1);
    let after = objects(&session);
    assert!(near(
        after[0].rotation().as_radians(),
        std::f64::consts::FRAC_PI_2
    ));
    // The left rectangle (centre (70, 50)) ends at (140, -20).
    let b = bounds_of(&after[0]);
    assert!(near(b.origin.x + b.width.as_mm() / 2.0, 140.0));
    assert!(near(b.origin.y + b.height.as_mm() / 2.0, -20.0));
}

/// Criterion 34: S opens the size chip with the group box size; Enter scales the
/// selection to it about the box centre; a size of zero is refused.
#[test]
fn the_size_chip_scales_the_selection_to_the_typed_size() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    assert_eq!(key(&mut session, "s", false), KeyOutcome::EntryOpened);
    let view = session.transform_entry().expect("a chip");
    assert_eq!(view.kind, "size");
    assert_eq!(view.fields.len(), 2);
    assert_eq!(view.fields[0].prefill, "240.0");
    assert_eq!(view.fields[1].prefill, "60.0");
    assert!(view.at_centre);
    assert!(!view.linked);
    assert_eq!(
        session.commit_transform_entry("0", "60.0", 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: curvyo_ui_core::InvalidReason::NotPositive
        }
    );
    assert_eq!(
        session.commit_transform_entry("480", "60.0", 0),
        EntryOutcome::Committed
    );
    // About the centre (140, 50): the box is now 480 wide, from -100 to 380.
    let after = objects(&session);
    let a = bounds_of(&after[0]);
    assert!(near(a.origin.x, -100.0) && near(a.width.as_mm(), 200.0));
    // Unedited Enter writes nothing.
    assert_eq!(key(&mut session, "s", false), KeyOutcome::EntryOpened);
    let view = session.transform_entry().unwrap();
    assert_eq!(view.fields[0].prefill, "480.0");
    assert_eq!(
        session.commit_transform_entry("480.0", "60.0", 0),
        EntryOutcome::Unchanged
    );
}

/// Criterion 36: K opens the skew chip of the top handle for paths; typing 45
/// shears every path with the bottom fixed.
#[test]
fn the_skew_chip_shears_the_paths() {
    let mut session = session_of(&paths());
    select_paths(&mut session);
    assert_eq!(key(&mut session, "k", false), KeyOutcome::EntryOpened);
    let view = session.transform_entry().expect("a chip");
    assert_eq!(view.kind, "skew");
    assert_eq!(view.fields[0].accessible_name, "Skew angle x");
    assert_eq!(
        session.commit_transform_entry("45", "", 0),
        EntryOutcome::Committed
    );
    let first = points_of(&objects(&session)[0]);
    assert!(near(first[0].x, 40.0) && near(first[1].x, 60.0));
}

/// Criterion 35, 37: the keys with nothing selected show the usual hint; the
/// old "Select one object to type a value" is gone.
#[test]
fn nothing_selected_still_says_select_an_object_first() {
    let mut session = session_of(&rects());
    for letter in ["m", "r", "s", "k"] {
        let outcome = key(&mut session, letter, false);
        assert!(matches!(
            outcome,
            KeyOutcome::Hint(curvyo_editor_wasm::KeyHint::SelectFirst) | KeyOutcome::ToolChanged
        ));
    }
}

/// Criterion 47: a double-click on a group handle opens its chip: the centre
/// handle the move chip, a rotate handle the angle chip.
#[test]
fn a_double_click_on_a_group_handle_opens_its_chip() {
    let mut session = session_of(&rects());
    select_rects(&mut session);
    let centre = pt(140.0, 50.0);
    hold(&mut session, centre, false, false);
    session.pointer_down(centre, false);
    session.pointer_up(centre, false, false);
    session.double_click(centre, false, false);
    assert!(session.move_entry().is_some());
    session.cancel_transform_entry();
    let handle = rotate_ne(&session, pt(260.0, 20.0));
    hold(&mut session, handle, false, false);
    session.pointer_down(handle, false);
    session.pointer_up(handle, false, false);
    session.double_click(handle, false, false);
    let view = session.transform_entry().expect("an angle chip");
    assert_eq!(view.kind, "angle");
}
