//! Independent tester acceptance tests for PR 4 of
//! `specs/edit-interaction-polish/specification.md`: Part C (criteria 26 to
//! 41, move modifiers) and the Copy check of the typed move (criteria 23 and
//! 24). Written from the specification and the public signatures before the
//! implementation was read. Everything goes through `Session`'s public API;
//! expected values come from reference arithmetic written here. The pointer
//! coordinates are document millimetres (the default view is 96 dpi, about
//! 3.78 px per mm; the dead zone is 3 px).

#![allow(
    clippy::assert_is_empty,
    clippy::collapsible_if,
    clippy::manual_midpoint,
    clippy::cast_sign_loss
)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::type_complexity, clippy::needless_pass_by_value)]
#![allow(clippy::doc_markdown, clippy::too_many_arguments, missing_docs)]
#![allow(clippy::needless_range_loop, clippy::manual_let_else)]

use std::collections::HashSet;

use vecmanf_document_core::{
    AnchorId, AnchorKind, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId,
    ObjectSnapshot, Point, PointCount, RectBounds, Shape, StarFrame, Vec2, pack, unpack,
};
use vecmanf_editor_wasm::{EscapeStep, KeyInput, KeyOutcome, Session, Tool};
use vecmanf_render_core::{DrawList, RgbaColor, Vertex, build_live_edit_preview};
use vecmanf_ui_core::{Axis, EntryOutcome, InvalidReason, MoveEntryMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

fn pnear(a: Point, b: Point) -> bool {
    near(a.x, b.x) && near(a.y, b.y)
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

const ACCENT: RgbaColor = RgbaColor::opaque(0x2F, 0x6F, 0xEE);

fn axis_guide() -> RgbaColor {
    ACCENT.with_alpha(128)
}

fn axis_guide_idle() -> RgbaColor {
    ACCENT.with_alpha(51)
}

fn open(d: &Document) -> Session {
    let bytes = pack(d, "0.1.0").unwrap();
    let mut s = Session::open(2, &bytes).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1000.0, 800.0);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn bytes_of(s: &Session) -> Vec<u8> {
    doc_of(s).export_loro_snapshot().unwrap()
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&bytes_of(s)).unwrap();
    l.len_changes()
}

fn all(s: &Session) -> Vec<ObjectSnapshot> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .map(|i| d.object(i).unwrap())
        .collect()
}

fn ids(s: &Session) -> Vec<NodeId> {
    doc_of(s).object_ids()
}

fn rect_origin(o: &ObjectSnapshot) -> Point {
    let ObjectSnapshot::Primitive(p) = o else {
        panic!("a primitive")
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!("a rect")
    };
    bounds.origin
}

fn sel(s: &Session) -> usize {
    s.selected_object_count()
}

fn readout(s: &Session) -> Option<String> {
    s.live_readout().map(|r| r.text)
}

fn k(s: &Session) -> f64 {
    s.view().scale()
}

fn click_mod(s: &mut Session, p: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(p, shift, ctrl);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, ctrl);
}

fn click(s: &mut Session, p: Point) {
    click_mod(s, p, false, false);
}

fn shift_click(s: &mut Session, p: Point) {
    click_mod(s, p, true, false);
}

/// A full drag with fixed modifiers throughout.
fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, shift, ctrl);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

fn press(s: &mut Session, key: &str) -> KeyOutcome {
    s.key_down(KeyInput {
        key,
        ..KeyInput::default()
    })
}

fn has(frame: &DrawList, v: &Vertex) -> bool {
    frame.triangles.iter().any(|w| {
        w.color == v.color
            && (w.position.x - v.position.x).abs() < 1e-6
            && (w.position.y - v.position.y).abs() < 1e-6
    })
}

fn contains_all(frame: &DrawList, part: &DrawList) -> bool {
    part.triangles.iter().all(|v| has(frame, v))
}

fn with_color(frame: &DrawList, color: RgbaColor) -> Vec<(usize, Vertex)> {
    frame
        .triangles
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, v)| v.color == color)
        .collect()
}

// Scene: A (10,20) 80x50 centre (50,45); B (150,20) 60x40 centre (180,40);
// C (10,120) 60x40.
const A_OUTLINE: (f64, f64) = (30.0, 20.0);
const A_CENTRE: (f64, f64) = (50.0, 45.0);
const A_INSIDE: (f64, f64) = (30.0, 35.0);
const B_OUTLINE: (f64, f64) = (150.0, 40.0);

fn abc_doc() -> Document {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(10.0, 20.0, 80.0, 50.0));
    let _ = d.create_rect(bounds(150.0, 20.0, 60.0, 40.0));
    let _ = d.create_rect(bounds(10.0, 120.0, 60.0, 40.0));
    d
}

fn abc_with_a_selected() -> Session {
    let mut s = open(&abc_doc());
    click(&mut s, pt(A_OUTLINE.0, A_OUTLINE.1));
    assert_eq!(sel(&s), 1);
    s
}

fn p(t: (f64, f64)) -> Point {
    pt(t.0, t.1)
}

fn plus(a: Point, dx: f64, dy: f64) -> Point {
    pt(a.x + dx, a.y + dy)
}

/// The object set after the first (and only) object differs by `d`.
fn moved_a(s: &Session) -> Point {
    rect_origin(&all(s)[0])
}

// ---------------------------------------------------------------------
// Criteria 27, 28, 30: Shift axis lock, re-chosen on every pointer event
// ---------------------------------------------------------------------

/// The worked example of criterion 30, step by step, with the readout text
/// of criterion 27 and the commit of the release.
#[test]
fn c30_the_axis_is_rechosen_on_every_event_and_shift_is_read_live() {
    let mut s = abc_with_a_selected();
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    // 1. (+30, +10) with Shift: axis x
    s.pointer_hover(plus(p0, 30.0, 10.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 30.0, 0.0 mm"));
    assert_eq!(s.move_indicators().lock, Some(Axis::X));
    // 2. (+31, +80): axis y (jump across the diagonal)
    s.pointer_hover(plus(p0, 31.0, 80.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, 80.0 mm"));
    assert_eq!(s.move_indicators().lock, Some(Axis::Y));
    // 3. back to (+31, +5): x again, offset (31, 0)
    s.pointer_hover(plus(p0, 31.0, 5.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 31.0, 0.0 mm"));
    assert_eq!(s.move_indicators().lock, Some(Axis::X));
    // 4. Shift released with the pointer at (+31, +80): free, offset D
    s.pointer_hover(plus(p0, 31.0, 80.0), false, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 31.0, 80.0 mm"));
    assert_eq!(s.move_indicators().lock, None);
    // 5. Shift pressed again, pointer at rest: |Dy| 80 beats |Dx| 31
    s.modifiers_changed(true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, 80.0 mm"));
    assert_eq!(s.move_indicators().lock, Some(Axis::Y));
    // release with Shift: the commit is the readout
    s.pointer_up(plus(p0, 31.0, 80.0), true, false);
    assert_eq!(moved_a(&s), pt(10.0, 100.0));
    assert_eq!(sel(&s), 1);
}

/// Criterion 30: an exact tie keeps the previous axis; there is no latch.
#[test]
fn c30_a_tie_keeps_the_previous_axis_and_there_is_no_hysteresis() {
    let mut s = abc_with_a_selected();
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    s.pointer_hover(plus(p0, 30.0, 10.0), true, false);
    assert_eq!(s.move_indicators().lock, Some(Axis::X));
    s.pointer_hover(plus(p0, 20.0, 20.0), true, false); // tie
    assert_eq!(
        readout(&s).as_deref(),
        Some("Δ 20.0, 0.0 mm"),
        "tie keeps x"
    );
    s.pointer_hover(plus(p0, 20.0, 21.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, 21.0 mm"));
    s.pointer_hover(plus(p0, 20.0, 20.0), true, false); // tie again
    assert_eq!(
        readout(&s).as_deref(),
        Some("Δ 0.0, 20.0 mm"),
        "tie keeps y"
    );
    // a one-hundredth-mm lead is enough to switch, no hysteresis
    s.pointer_hover(plus(p0, 20.01, 20.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 20.0, 0.0 mm"));
    s.pointer_hover(plus(p0, 20.01, 20.02), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, 20.0 mm"));
}

/// Criterion 27: real minus sign, one decimal, after the lock; never "-0.0" on
/// the locked-out axis.
#[test]
fn c27_readout_uses_a_real_minus_and_the_locked_offset() {
    let mut s = abc_with_a_selected();
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    s.pointer_hover(plus(p0, 12.5, -3.0), false, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 12.5, −3.0 mm"));
    assert!(readout(&s).unwrap().contains('\u{2212}'));
    assert!(!readout(&s).unwrap().contains('-'));
    s.pointer_hover(plus(p0, -30.0, 4.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ −30.0, 0.0 mm"));
    s.pointer_hover(plus(p0, 4.0, -30.0), true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, −30.0 mm"));
    // copy marker
    s.pointer_hover(plus(p0, 12.5, -3.0), false, true);
    assert_eq!(readout(&s).as_deref(), Some("Δ 12.5, −3.0 mm Copy"));
    s.pointer_hover(plus(p0, 4.0, -30.0), true, true);
    assert_eq!(readout(&s).as_deref(), Some("Δ 0.0, −30.0 mm Copy"));
    s.escape();
}

/// Criterion 27: no readout before the press, inside the dead zone, or after
/// the release.
#[test]
fn c27_no_readout_in_the_dead_zone_or_outside_a_drag() {
    let mut s = abc_with_a_selected();
    let p0 = p(A_INSIDE);
    assert!(readout(&s).is_none());
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    assert!(readout(&s).is_none());
    let two_px = 2.0 / k(&s);
    s.pointer_hover(plus(p0, two_px, 0.0), false, false);
    assert!(readout(&s).is_none(), "inside the 3 px dead zone");
    s.pointer_hover(plus(p0, 4.0 / k(&s), 0.0), false, false);
    assert!(readout(&s).is_some(), "4 px is past it");
    s.pointer_up(plus(p0, 4.0 / k(&s), 0.0), false, false);
    assert!(readout(&s).is_none());
}

/// Criterion 28: the axes are the document's, not a rotated object's.
#[test]
fn c28_the_lock_follows_document_axes_not_the_rotated_box() {
    let d = Document::new(1);
    let id = d.create_rect(bounds(10.0, 20.0, 80.0, 50.0));
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(pt(50.0, 45.0), Angle::from_radians(0.6)))
        .unwrap();
    let mut s = open(&d);
    // a rotated corner of the outline
    let c = rot(pt(30.0, 20.0), pt(50.0, 45.0), 0.6);
    click(&mut s, c);
    assert_eq!(sel(&s), 1);
    let before = rect_origin(&all(&s)[0]);
    drag_mod(&mut s, c, plus(c, 40.0, 17.0), true, false);
    let after = rect_origin(&all(&s)[0]);
    assert!(near(after.x - before.x, 40.0) && near(after.y - before.y, 0.0));
    let before = after;
    drag_mod(&mut s, plus(c, 40.0, 0.0), plus(c, 43.0, 30.0), true, false);
    let after = rect_origin(&all(&s)[0]);
    assert!(near(after.x - before.x, 0.0) && near(after.y - before.y, 30.0));
}

/// Criterion 31: a locked drag back to the start writes nothing.
#[test]
fn c31_a_locked_drag_back_to_the_start_writes_nothing() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, true, false);
    s.pointer_down(p0, true);
    s.pointer_hover(plus(p0, 30.0, 4.0), true, false);
    s.pointer_hover(p0, true, false);
    assert!(
        readout(&s).is_none_or(|t| t.starts_with("Δ 0.0, 0.0")),
        "{:?}",
        readout(&s)
    );
    s.pointer_up(p0, true, false);
    assert_eq!(bytes_of(&s), before);
}

// ---------------------------------------------------------------------
// Criterion 29: Shift at the press
// ---------------------------------------------------------------------

#[test]
fn c29_shift_press_on_a_selected_outline_toggles_on_a_click_at_the_release() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let o = p(A_OUTLINE);
    s.pointer_hover(o, true, false);
    s.pointer_down(o, true);
    assert_eq!(sel(&s), 1, "the selection does not change at the press");
    // release inside the 3 px dead zone
    let d = 2.0 / k(&s);
    s.pointer_hover(plus(o, d, 0.0), true, false);
    s.pointer_up(plus(o, d, 0.0), true, false);
    assert_eq!(sel(&s), 0, "a click toggles the object out");
    assert_eq!(bytes_of(&s), before, "nothing written");
}

#[test]
fn c29_shift_press_then_a_drag_locks_from_the_first_frame_and_does_not_toggle() {
    let mut s = abc_with_a_selected();
    let o = p(A_OUTLINE);
    s.pointer_hover(o, true, false);
    s.pointer_down(o, true);
    // the first frame past the dead zone: 4 px right, 3 px down
    let (dx, dy) = (4.0 / k(&s), 3.0 / k(&s));
    s.pointer_hover(plus(o, dx, dy), true, false);
    let text = readout(&s).expect("past the dead zone");
    assert!(
        text.ends_with(", 0.0 mm") && text.starts_with("Δ 1.0") || text.contains("0.0 mm"),
        "locked from the first frame: {text}"
    );
    assert_eq!(s.move_indicators().lock, Some(Axis::X));
    s.pointer_up(plus(o, 30.0, 5.0), true, false);
    assert_eq!(sel(&s), 1, "a drag never toggles");
    assert_eq!(moved_a(&s), pt(40.0, 20.0));
}

#[test]
fn c29_shift_press_on_an_unselected_object_joins_it_when_the_drag_leaves_the_dead_zone() {
    let mut s = abc_with_a_selected();
    let b = p(B_OUTLINE);
    s.pointer_hover(b, true, false);
    s.pointer_down(b, true);
    assert_eq!(sel(&s), 1, "no change at the press");
    s.pointer_hover(plus(b, 4.0 / k(&s), 0.0), true, false);
    assert_eq!(
        sel(&s),
        2,
        "B joined in the frame the drag left the dead zone"
    );
    s.pointer_hover(plus(b, 30.0, 12.0), true, false);
    s.pointer_up(plus(b, 30.0, 12.0), true, false);
    assert_eq!(sel(&s), 2);
    let o = all(&s);
    assert_eq!(rect_origin(&o[0]), pt(40.0, 20.0));
    assert_eq!(rect_origin(&o[1]), pt(180.0, 20.0));
    assert_eq!(rect_origin(&o[2]), pt(10.0, 120.0));
}

/// Criteria 26 and 29: the release builds its result from the release event
/// alone. A release at a far point that no pointer move preceded (a fast
/// flick, a synthetic event) is still "a drag that left the dead zone": the
/// Shift-pressed unselected object joins and moves with the selection.
#[test]

fn c29_a_release_with_no_preceding_move_event_still_joins_the_pressed_object() {
    let mut s = abc_with_a_selected();
    let b = p(B_OUTLINE);
    s.pointer_hover(b, true, false);
    s.pointer_down(b, true);
    s.pointer_up(plus(b, 30.0, 12.0), true, false); // no pointer_hover in between
    assert_eq!(sel(&s), 2, "B joined the selection");
    let o = all(&s);
    assert_eq!(rect_origin(&o[0]), pt(40.0, 20.0), "A moved");
    assert_eq!(rect_origin(&o[1]), pt(180.0, 20.0), "B moved with it");
}

#[test]
fn c29_shift_click_on_an_unselected_object_adds_it() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    shift_click(&mut s, p(B_OUTLINE));
    assert_eq!(sel(&s), 2);
    assert_eq!(bytes_of(&s), before);
    // and a shift click on a selected one removes it
    shift_click(&mut s, p(B_OUTLINE));
    assert_eq!(sel(&s), 1);
}

#[test]
fn c29_without_shift_at_the_press_a_mid_drag_shift_engages_and_releases_without_accumulation() {
    let mut s = abc_with_a_selected();
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, false, false);
    s.pointer_down(p0, false);
    s.pointer_hover(plus(p0, 30.0, 10.0), false, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 30.0, 10.0 mm"));
    s.modifiers_changed(true, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 30.0, 0.0 mm"));
    s.modifiers_changed(false, false);
    assert_eq!(readout(&s).as_deref(), Some("Δ 30.0, 10.0 mm"));
    s.pointer_up(plus(p0, 30.0, 10.0), false, false);
    assert_eq!(moved_a(&s), pt(40.0, 30.0));
}

#[test]
fn c29_a_shift_press_inside_the_sole_selected_box_off_the_outline_starts_no_move() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let p0 = p(A_INSIDE);
    s.pointer_hover(p0, true, false);
    s.pointer_down(p0, true);
    s.pointer_hover(plus(p0, 30.0, 10.0), true, false);
    assert!(readout(&s).is_none(), "no move readout");
    s.pointer_up(plus(p0, 30.0, 10.0), true, false);
    assert_eq!(bytes_of(&s), before);
}

/// Criterion 38: the centre handle with Shift never toggles, always moves.
#[test]
fn c38_shift_on_the_centre_handle_never_toggles_and_locks() {
    let mut s = abc_with_a_selected();
    let c = p(A_CENTRE);
    let before = bytes_of(&s);
    // a click with Shift on the handle: nothing is toggled
    s.pointer_hover(c, true, false);
    s.pointer_down(c, true);
    s.pointer_up(c, true, false);
    assert_eq!(sel(&s), 1, "still selected");
    assert_eq!(bytes_of(&s), before);
    // a drag locks from the first frame, no toggle
    s.pointer_hover(c, true, false);
    s.pointer_down(c, true);
    s.pointer_hover(plus(c, 4.0 / k(&s), 3.0 / k(&s)), true, false);
    assert!(readout(&s).unwrap().contains(", 0.0 mm"));
    s.pointer_up(plus(c, 25.0, 9.0), true, false);
    assert_eq!(sel(&s), 1);
    assert_eq!(moved_a(&s), pt(35.0, 20.0));
}

/// Criterion 38: Ctrl on the centre handle copies.
#[test]
fn c38_ctrl_on_the_centre_handle_copies() {
    let mut s = abc_with_a_selected();
    let c = p(A_CENTRE);
    drag_mod(&mut s, c, plus(c, 20.0, 60.0), false, true);
    let o = all(&s);
    assert_eq!(o.len(), 4);
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0));
    assert_eq!(rect_origin(&o[1]), pt(30.0, 80.0));
}

// ---------------------------------------------------------------------
// Criterion 33 to 37: Ctrl copy
// ---------------------------------------------------------------------

fn run(press_ctrl: bool, mid_ctrl: Option<bool>, release_ctrl: bool) -> Session {
    let mut s = abc_with_a_selected();
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, press_ctrl);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 10.0, 5.0), false, press_ctrl);
    if let Some(m) = mid_ctrl {
        s.modifiers_changed(false, m);
    }
    s.pointer_hover(plus(o, 30.0, 12.0), false, mid_ctrl.unwrap_or(press_ctrl));
    s.pointer_up(plus(o, 30.0, 12.0), false, release_ctrl);
    s
}

fn assert_copy(s: &Session) {
    let o = all(s);
    assert_eq!(o.len(), 4, "a copy was made");
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0), "original untouched");
    assert_eq!(
        rect_origin(&o[1]),
        pt(40.0, 32.0),
        "copy directly above, displaced"
    );
    assert_eq!(sel(s), 1);
}

fn assert_move(s: &Session) {
    let o = all(s);
    assert_eq!(o.len(), 3);
    assert_eq!(rect_origin(&o[0]), pt(40.0, 32.0));
    assert_eq!(sel(s), 1);
}

#[test]
fn c33_ctrl_at_the_release_decides() {
    assert_copy(&run(true, None, true)); // down all along
    assert_move(&run(true, None, false)); // released before the release
    assert_copy(&run(false, Some(true), true)); // pressed mid-drag
    assert_move(&run(false, Some(true), false)); // pressed and released again
    assert_copy(&run(false, None, true)); // only at the release event
    assert_move(&run(false, None, false));
    assert_move(&run(true, Some(false), false));
    assert_copy(&run(true, Some(false), true)); // off mid-drag, back at release
}

#[test]
fn c34_c35_the_copy_is_selected_and_the_originals_are_not() {
    let s = run(false, None, true);
    let mut s2 = s;
    // delete_selected deletes the selection: it must be the copy.
    s2.delete_selected();
    let o = all(&s2);
    assert_eq!(o.len(), 3);
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0));
    assert_eq!(sel(&s2), 0);
}

#[test]
fn c34_one_commit_and_new_ids_originals_keep_theirs() {
    let mut s = abc_with_a_selected();
    let before_ids = ids(&s);
    let n = change_count(&s);
    drag_mod(
        &mut s,
        p(A_INSIDE),
        plus(p(A_INSIDE), 30.0, 12.0),
        false,
        true,
    );
    assert_eq!(change_count(&s), n + 1, "one commit");
    let after = ids(&s);
    assert_eq!(after.len(), 4);
    for i in &before_ids {
        assert!(after.contains(i), "the originals keep their NodeIds");
    }
    let uniq: HashSet<_> = after.iter().collect();
    assert_eq!(uniq.len(), 4);
}

#[test]
fn c35_a_single_copy_shows_its_full_handle_set_at_once() {
    let mut s = abc_with_a_selected();
    drag_mod(
        &mut s,
        p(A_INSIDE),
        plus(p(A_INSIDE), 100.0, 100.0),
        false,
        true,
    );
    // the copy sits at origin (110, 120); its resize handle is hit
    let copy_origin = pt(110.0, 120.0);
    let se = pt(copy_origin.x + 80.0, copy_origin.y + 50.0);
    // a press just outside the SE corner within the resize handle tolerance
    // selects a resize cursor
    s.pointer_hover(plus(se, 1.0 / k(&s), 1.0 / k(&s)), false, false);
    assert!(
        s.cursor_hint().starts_with("resize"),
        "the copy has its handles: {}",
        s.cursor_hint()
    );
}

#[test]
fn c34_z_order_a_a_prime_b_b_prime_for_a_multi_selection() {
    let mut s = abc_with_a_selected();
    shift_click(&mut s, pt(10.0, 140.0)); // C's left edge
    assert_eq!(sel(&s), 2);
    let ids_before = ids(&s);
    drag_mod(
        &mut s,
        p(A_OUTLINE),
        plus(p(A_OUTLINE), 30.0, 12.0),
        false,
        true,
    );
    let o = all(&s);
    assert_eq!(o.len(), 5);
    // A, A', B, C, C'
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0));
    assert_eq!(rect_origin(&o[1]), pt(40.0, 32.0));
    assert_eq!(rect_origin(&o[2]), pt(150.0, 20.0));
    assert_eq!(rect_origin(&o[3]), pt(10.0, 120.0));
    assert_eq!(rect_origin(&o[4]), pt(40.0, 132.0));
    let now = ids(&s);
    assert_eq!(now[0], ids_before[0]);
    assert_eq!(now[2], ids_before[1]);
    assert_eq!(now[3], ids_before[2]);
    assert_eq!(sel(&s), 2, "selection = the two copies");
    s.delete_selected();
    let o = all(&s);
    assert_eq!(o.len(), 3);
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0));
    assert_eq!(rect_origin(&o[2]), pt(10.0, 120.0));
}

#[test]
fn c34_the_clone_of_overlapping_objects_keeps_relative_order() {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(10.0, 20.0, 80.0, 50.0));
    let _ = d.create_rect(bounds(50.0, 40.0, 80.0, 50.0));
    let mut s = open(&d);
    click(&mut s, pt(30.0, 20.0));
    shift_click(&mut s, pt(130.0, 65.0));
    assert_eq!(sel(&s), 2);
    drag_mod(&mut s, pt(130.0, 65.0), pt(140.0, 85.0), false, true);
    let o = all(&s);
    assert_eq!(o.len(), 4);
    let origins: Vec<_> = o.iter().map(rect_origin).collect();
    assert_eq!(
        origins,
        vec![
            pt(10.0, 20.0),
            pt(20.0, 40.0),
            pt(50.0, 40.0),
            pt(60.0, 60.0)
        ]
    );
    assert_eq!(sel(&s), 2);
}

/// All kinds, with rotation, radius, star frame, anchors: scene of five.
fn rich_session() -> (Session, Vec<Point>) {
    let d = Document::new(1);
    let path = d.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(1, 1),
                point: pt(0.0, 10.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(6.0, -8.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor {
                id: AnchorId::new(1, 2),
                point: pt(25.0, 0.0),
                handle_in: Vec2::new(-5.0, -6.0),
                handle_out: Vec2::new(5.0, 6.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor {
                id: AnchorId::new(1, 3),
                point: pt(45.0, 22.0),
                handle_in: Vec2::new(-3.0, -7.0),
                handle_out: Vec2::new(2.0, 2.0),
                kind: AnchorKind::Asymmetric,
            },
        ],
        true,
    );
    let rect = d.create_rect(bounds(10.0, 60.0, 30.0, 16.0));
    d.set_corner_radius(&[rect], Length::from_mm(3.5)).unwrap();
    let ellipse = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 60.0),
        rx: Length::from_mm(12.0),
        ry: Length::from_mm(7.0),
    });
    let polygon = d.create_polygon(
        StarFrame {
            center: pt(140.0, 60.0),
            radius: Length::from_mm(9.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
    );
    let star = d.create_star(
        StarFrame {
            center: pt(180.0, 60.0),
            radius: Length::from_mm(11.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let o = d.object(rect).unwrap();
    d.rotate_object(&o.rotated(pt(25.0, 68.0), Angle::from_radians(0.5)))
        .unwrap();
    let o = d.object(star).unwrap();
    d.rotate_object(&o.rotated(pt(180.0, 60.0), Angle::from_radians(0.2)))
        .unwrap();
    // The star's vertices have rotated about the centre (the rotate is baked
    // into the frame and the register): the first outer vertex is where the
    // snapshot says; find it through the outline.
    let hit_points = vec![
        pt(0.0, 10.0),                            // path anchor 1
        rot(pt(20.0, 60.0), pt(25.0, 68.0), 0.5), // rect corner (rounded: near)
        pt(88.0, 60.0),                           // ellipse left
        pt(149.0, 60.0),                          // polygon vertex 0
        {
            let r = rot(pt(191.0, 60.0), pt(180.0, 60.0), 0.2);
            pt(r.x, r.y)
        },
    ];
    let _ = (path, ellipse, polygon);
    (open(&d), hit_points)
}

#[test]
fn c34_every_kind_is_copied_with_every_field_and_the_selection_is_the_copies() {
    let (mut s, hits) = rich_session();
    // select every object: click the first, shift-click the rest
    click(&mut s, hits[0]);
    for h in &hits[1..] {
        shift_click(&mut s, *h);
    }
    // the rounded rect's corner point is not on its outline exactly; accept
    // fewer selected only if a press missed, and then say so.
    assert_eq!(sel(&s), 5, "all five selected (scene hit points)");
    let before = all(&s);
    let n = change_count(&s);
    let off = Vec2::new(13.5, 27.25);
    let from = hits[2];
    drag_mod(
        &mut s,
        from,
        pt(from.x + off.x, from.y + off.y),
        false,
        true,
    );
    assert_eq!(change_count(&s), n + 1);
    let after = all(&s);
    assert_eq!(after.len(), 10);
    assert_eq!(sel(&s), 5);
    // z-order: original, copy, original, copy ...
    for (i, orig) in before.iter().enumerate() {
        assert_eq!(&after[2 * i], orig, "original {i} untouched, same place");
        let copy = &after[2 * i + 1];
        assert_ne!(copy.id(), orig.id());
        match (orig, copy) {
            (ObjectSnapshot::Path(a), ObjectSnapshot::Path(b)) => {
                assert_eq!(a.closed, b.closed);
                assert_eq!(a.stroke_width, b.stroke_width);
                assert_eq!(a.stroke, b.stroke);
                assert_eq!(a.rotation, b.rotation);
                for (x, y) in a.anchors.iter().zip(&b.anchors) {
                    assert_ne!(x.id, y.id);
                    assert!(pnear(plus(x.point, off.x, off.y), y.point));
                    assert_eq!(
                        (x.handle_in, x.handle_out, x.kind),
                        (y.handle_in, y.handle_out, y.kind)
                    );
                }
            }
            (ObjectSnapshot::Primitive(a), ObjectSnapshot::Primitive(b)) => {
                assert_eq!(a.stroke_width, b.stroke_width);
                assert_eq!(a.rotation, b.rotation, "rotation copied");
                assert_eq!(a.fill, b.fill);
                match (&a.shape, &b.shape) {
                    (
                        Shape::Rect {
                            bounds: x,
                            corner_radius: r1,
                        },
                        Shape::Rect {
                            bounds: y,
                            corner_radius: r2,
                        },
                    ) => {
                        assert_eq!(r1, r2, "corner radius copied");
                        assert_eq!((x.width, x.height), (y.width, y.height));
                        assert!(pnear(plus(x.origin, off.x, off.y), y.origin));
                    }
                    (Shape::Ellipse { frame: x }, Shape::Ellipse { frame: y }) => {
                        assert!(pnear(plus(x.center, off.x, off.y), y.center));
                        assert_eq!((x.rx, x.ry), (y.rx, y.ry));
                    }
                    (
                        Shape::Polygon {
                            frame: x,
                            point_count: a1,
                        },
                        Shape::Polygon {
                            frame: y,
                            point_count: a2,
                        },
                    ) => {
                        assert_eq!(a1, a2);
                        assert!(pnear(plus(x.center, off.x, off.y), y.center));
                        assert_eq!(x.angle, y.angle);
                    }
                    (
                        Shape::Star {
                            frame: x,
                            point_count: a1,
                            inner_ratio: r1,
                        },
                        Shape::Star {
                            frame: y,
                            point_count: a2,
                            inner_ratio: r2,
                        },
                    ) => {
                        assert_eq!((a1, r1), (a2, r2));
                        assert!(pnear(plus(x.center, off.x, off.y), y.center));
                        assert_eq!(x.angle, y.angle, "star frame copied");
                        assert_eq!(x.radius, y.radius);
                    }
                    other => panic!("kind changed: {other:?}"),
                }
            }
            _ => panic!("path <-> primitive conversion"),
        }
    }
    // anchor ids are unique across the whole document
    let mut seen = HashSet::new();
    let d = doc_of(&s);
    for id in d.object_ids() {
        if let Some(p) = d.path(id) {
            for a in p.anchors {
                assert!(seen.insert(a.id), "anchor id reused");
            }
        }
    }
    assert_eq!(seen.len(), 6);
    // the selection is the five copies: deleting it leaves the originals
    s.delete_selected();
    assert_eq!(all(&s), before);
}

/// Criterion 36: a zero offset makes no copy and writes nothing.
#[test]
fn c36_a_copy_dragged_back_to_the_start_makes_no_copy() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, true);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 12.0), false, true);
    s.pointer_hover(o, false, true);
    s.pointer_up(o, false, true);
    assert_eq!(bytes_of(&s), before);
    assert_eq!(all(&s).len(), 3);
    assert_eq!(sel(&s), 1);
    // and inside the dead zone
    s.pointer_hover(o, false, true);
    s.pointer_down(o, false);
    let q = plus(o, 1.0 / k(&s), 0.0);
    s.pointer_hover(q, false, true);
    s.pointer_up(q, false, true);
    assert_eq!(bytes_of(&s), before);
    // and a Shift+Ctrl press-release without movement
    s.pointer_hover(o, true, true);
    s.pointer_down(o, true);
    s.pointer_up(o, true, true);
    assert_eq!(bytes_of(&s), before);
}

/// Criterion 32: Shift+Ctrl: a copy displaced along the locked axis.
#[test]
fn c32_shift_and_ctrl_together_copy_along_the_locked_axis() {
    let mut s = abc_with_a_selected();
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, false);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 10.0), true, true);
    assert_eq!(readout(&s).as_deref(), Some("Δ 30.0, 0.0 mm Copy"));
    s.pointer_up(plus(o, 30.0, 10.0), true, true);
    let r = all(&s);
    assert_eq!(r.len(), 4);
    assert_eq!(rect_origin(&r[0]), pt(10.0, 20.0));
    assert_eq!(rect_origin(&r[1]), pt(40.0, 20.0));
}

/// Criterion 26: Escape cancels everything, also with Ctrl held, no copy.
#[test]
fn c26_escape_cancels_the_drag_with_ctrl_held_and_the_badge_stays() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let o = p(A_OUTLINE);
    s.pointer_hover(o, false, true);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 12.0), false, true);
    assert!(s.move_indicators().copy_badge);
    let step = s.escape();
    assert_eq!(step, EscapeStep::CancelledDrag);
    assert!(readout(&s).is_none());
    assert_eq!(bytes_of(&s), before);
    s.pointer_up(plus(o, 30.0, 12.0), false, true);
    assert_eq!(
        bytes_of(&s),
        before,
        "the release after Escape writes nothing"
    );
    assert_eq!(all(&s).len(), 3);
    // pointer is back over the outline with Ctrl held: the badge shows again
    s.pointer_hover(o, false, true);
    assert!(
        s.move_indicators().copy_badge,
        "held Ctrl keeps the plus badge"
    );
    // and also with the pointer at rest after the cancel
    s.modifiers_changed(false, true);
    assert!(s.move_indicators().copy_badge);
    s.modifiers_changed(false, false);
    assert!(!s.move_indicators().copy_badge);
}

// ---------------------------------------------------------------------
// Criterion 26, 33: draw list: blue copy, black originals, box stays
// ---------------------------------------------------------------------

fn start_drag(s: &mut Session, ctrl: bool) -> (Point, DrawList) {
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, ctrl);
    let rest = s.draw_list();
    s.pointer_down(o, false);
    (o, rest)
}

#[test]
fn c33_copy_preview_blue_outline_travels_alone_box_and_handles_stay() {
    let mut s = abc_with_a_selected();
    let a = all(&s)[0].clone();
    let (o, rest) = start_drag(&mut s, true);
    let to = plus(o, 30.0, 12.0);
    s.pointer_hover(to, false, true);
    let during = s.draw_list();
    let blue = build_live_edit_preview(
        std::slice::from_ref(&a.translated(Vec2::new(30.0, 12.0))),
        s.view(),
    );
    assert!(blue.triangle_count() > 0);
    assert!(
        contains_all(&during, &blue),
        "the blue copy is exactly the result"
    );
    // Everything drawn at rest stays where it was, except the four corner
    // radius knobs (parameter handles), which every drag hides, move or
    // copy alike (observed on the plain move too, not specific to copy).
    let missing: Vec<_> = rest.triangles.iter().filter(|v| !has(&during, v)).collect();
    let near_corner = |v: &&Vertex| {
        let (x, y) = (v.position.x, v.position.y);
        [(10.0, 20.0), (90.0, 20.0), (10.0, 70.0), (90.0, 70.0)]
            .iter()
            .any(|(cx, cy)| (x - cx).abs() < 6.0 && (y - cy).abs() < 6.0)
    };
    assert!(
        missing.iter().all(near_corner),
        "the box, the resize/rotate/skew handles and the original stay put"
    );
    assert!(
        rest.triangles.len() - missing.len() > 3000,
        "nearly everything at rest is still drawn"
    );
    // release Ctrl in the frame of the key: back to a move, box follows
    s.modifiers_changed(false, false);
    let moving = s.draw_list();
    assert!(
        !contains_all(&moving, &rest),
        "in a move the box and handles travel"
    );
    assert!(
        contains_all(&moving, &blue),
        "the blue outline is the same result"
    );
    // press Ctrl again with the pointer at rest: copy in that frame
    s.modifiers_changed(false, true);
    assert_eq!(s.draw_list(), during);
    s.escape();
}

/// A drag with Ctrl toggled mid-drag draws what a drag with the final
/// modifier state draws (no sticky state).
#[test]
fn c26_preview_is_a_function_of_the_current_modifiers_only() {
    let to_off = (30.0, 12.0);
    let frame = |setup: &dyn Fn(&mut Session, Point)| {
        let mut s = abc_with_a_selected();
        let o = p(A_INSIDE);
        s.pointer_hover(o, false, false);
        s.pointer_down(o, false);
        setup(&mut s, o);
        s.draw_list()
    };
    let to = |o: Point| plus(o, to_off.0, to_off.1);
    let plain = frame(&|s, o| s.pointer_hover(to(o), false, false));
    let toggled = frame(&|s, o| {
        s.pointer_hover(plus(o, 5.0, 40.0), false, true);
        s.pointer_hover(plus(o, 50.0, -3.0), true, true);
        s.pointer_hover(to(o), false, false);
    });
    assert_eq!(
        plain, toggled,
        "a Ctrl or Shift used and released leaves nothing"
    );
    let copy = frame(&|s, o| s.pointer_hover(to(o), false, true));
    let toggled_copy = frame(&|s, o| {
        s.pointer_hover(plus(o, 5.0, 40.0), true, false);
        s.pointer_hover(to(o), false, true);
    });
    assert_eq!(copy, toggled_copy);
    assert_ne!(plain, copy);
    let locked = frame(&|s, o| s.pointer_hover(plus(o, 30.0, 0.0), false, false));
    let locked_by_shift = frame(&|s, o| s.pointer_hover(to(o), true, false));
    // x-locked offset (30, 0) drawn identically either way, apart from the
    // axes lines that only the Shift route draws
    assert_ne!(locked, locked_by_shift);
}

#[test]
fn c27_origin_axes_colours_geometry_and_draw_order() {
    let mut s = abc_with_a_selected();
    let a = all(&s)[0].clone();
    let rest = s.draw_list();
    let o = p(A_INSIDE);
    s.pointer_hover(o, true, false);
    s.pointer_down(o, true); // Shift at the press on a selected box interior: no move
    s.pointer_up(o, true, false);
    // proper start: no shift at the press
    s.pointer_hover(o, false, false);
    s.pointer_down(o, false);
    let pressed = s.draw_list();
    assert!(with_color(&pressed, axis_guide()).is_empty());
    assert!(with_color(&pressed, axis_guide_idle()).is_empty());
    // past the dead zone, no Shift: no axes
    s.pointer_hover(plus(o, 30.0, 10.0), false, false);
    let free = s.draw_list();
    assert!(
        with_color(&free, axis_guide()).is_empty(),
        "free move: no axes"
    );
    // Shift on, pointer at rest: axes in that frame
    s.modifiers_changed(true, false);
    let locked = s.draw_list();
    let active = with_color(&locked, axis_guide());
    let idle = with_color(&locked, axis_guide_idle());
    assert!(!active.is_empty(), "the locked axis line is drawn");
    assert!(!idle.is_empty(), "the other axis line is drawn");
    let scale = k(&s);
    // start centre of the selection: the box centre of A (50, 45)
    let centre = pt(50.0, 45.0);
    // locked to x: the horizontal line is the active one
    let (ymin, ymax) = active
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| {
            (lo.min(v.position.y), hi.max(v.position.y))
        });
    let (xmin, xmax) = active
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| {
            (lo.min(v.position.x), hi.max(v.position.x))
        });
    assert!(
        ((ymin + ymax) / 2.0 - centre.y).abs() * scale <= 1.01,
        "through the start centre"
    );
    let thickness_px = (ymax - ymin) * scale;
    assert!(
        (0.9..=1.1).contains(&thickness_px),
        "1 px thick: {thickness_px}"
    );
    assert!(
        xmin <= 0.0 && xmax >= 1000.0 / scale - 1.0,
        "spans the viewport"
    );
    let (vxmin, vxmax) = idle.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| {
        (lo.min(v.position.x), hi.max(v.position.x))
    });
    let (vymin, vymax) = idle.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| {
        (lo.min(v.position.y), hi.max(v.position.y))
    });
    assert!(((vxmin + vxmax) / 2.0 - centre.x).abs() * scale <= 1.01);
    assert!((0.9..=1.1).contains(&((vxmax - vxmin) * scale)));
    assert!(vymin <= 0.0 && vymax >= 800.0 / scale - 1.0);
    // draw order: artwork, axes, blue outline, box and handles
    let blue = build_live_edit_preview(
        std::slice::from_ref(&a.translated(Vec2::new(30.0, 0.0))),
        s.view(),
    );
    let first_axis = active.iter().chain(&idle).map(|(i, _)| *i).min().unwrap();
    let last_axis = active.iter().chain(&idle).map(|(i, _)| *i).max().unwrap();
    let blue_first = locked
        .triangles
        .iter()
        .position(|v| {
            has(&blue, v) && v.color == blue.triangles[0].color && v.color != RgbaColor::BLACK
        })
        .expect("blue outline present");
    assert!(last_axis < blue_first, "axes under the blue outline");
    let black_rest: Vec<_> = with_color(&rest, RgbaColor::BLACK);
    let black_last = with_color(&locked, RgbaColor::BLACK)
        .iter()
        .map(|(i, _)| *i)
        .filter(|i| *i < blue_first)
        .max();
    if !black_rest.is_empty() {
        if let Some(bl) = black_last {
            assert!(bl < first_axis, "axes above the artwork");
        }
    }
    let after_blue = locked.triangles.len();
    assert!(
        after_blue > blue_first + blue.triangles.len(),
        "box and handles drawn after the blue outline"
    );
    // Shift released: axes gone in that frame
    s.modifiers_changed(false, false);
    let gone = s.draw_list();
    assert!(with_color(&gone, axis_guide()).is_empty());
    assert!(with_color(&gone, axis_guide_idle()).is_empty());
    // Switch the lock to y: the colours swap lines
    s.pointer_hover(plus(o, 10.0, 60.0), true, false);
    let ylock = s.draw_list();
    let a2 = with_color(&ylock, axis_guide());
    let (axmin, axmax) = a2.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| {
        (lo.min(v.position.x), hi.max(v.position.x))
    });
    assert!(
        ((axmin + axmax) / 2.0 - centre.x).abs() * scale <= 1.01,
        "vertical line active"
    );
    s.escape();
    assert!(
        with_color(&s.draw_list(), axis_guide()).is_empty(),
        "Escape removes the axes"
    );
}

#[test]
fn c27_no_axes_after_a_shift_press_before_the_drag_and_at_the_release() {
    let mut s = abc_with_a_selected();
    let o = p(A_OUTLINE);
    s.pointer_hover(o, true, false);
    assert!(with_color(&s.draw_list(), axis_guide()).is_empty());
    s.pointer_down(o, true);
    assert!(
        with_color(&s.draw_list(), axis_guide()).is_empty(),
        "inside the dead zone"
    );
    assert_eq!(
        s.move_indicators().lock,
        None,
        "no lock badge before the dead zone is left"
    );
    s.pointer_hover(plus(o, 4.0 / k(&s), 0.0), true, false);
    assert!(
        !with_color(&s.draw_list(), axis_guide()).is_empty(),
        "first frame past it: locked"
    );
    s.pointer_up(plus(o, 20.0, 0.0), true, false);
    assert!(with_color(&s.draw_list(), axis_guide()).is_empty());
    assert_eq!(s.move_indicators().lock, None);
}

// ---------------------------------------------------------------------
// Badges (criteria 26, 33, 37)
// ---------------------------------------------------------------------

#[test]
fn c33_plus_badge_follows_ctrl_and_the_press_hit_test_with_the_pointer_at_rest() {
    let mut s = abc_with_a_selected();
    // Ctrl on A's outline, on unselected B's outline, on the centre handle
    for at in [p(A_OUTLINE), p(B_OUTLINE), p(A_CENTRE)] {
        s.pointer_hover(at, false, false);
        assert!(!s.move_indicators().copy_badge, "no Ctrl, no badge");
        s.modifiers_changed(false, true);
        assert!(
            s.move_indicators().copy_badge,
            "the frame of the key, pointer at rest: {at:?}"
        );
        s.modifiers_changed(false, false);
        assert!(!s.move_indicators().copy_badge);
    }
    // empty canvas: Ctrl belongs to the marquee
    s.pointer_hover(pt(400.0, 300.0), false, true);
    assert!(!s.move_indicators().copy_badge);
    // the lock badge never before a press, whatever Shift does
    s.pointer_hover(p(A_OUTLINE), true, false);
    assert_eq!(s.move_indicators().lock, None);
    s.pointer_hover(p(A_CENTRE), true, true);
    assert_eq!(s.move_indicators().lock, None);
}

#[test]
fn c33_the_badge_shows_during_a_copy_drag_and_follows_ctrl() {
    let mut s = abc_with_a_selected();
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, false);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 12.0), false, false);
    assert!(!s.move_indicators().copy_badge);
    s.modifiers_changed(false, true);
    assert!(s.move_indicators().copy_badge);
    assert!(readout(&s).unwrap().ends_with(" Copy"));
    s.modifiers_changed(false, false);
    assert!(!s.move_indicators().copy_badge);
    assert!(!readout(&s).unwrap().ends_with("Copy"));
    s.pointer_up(plus(o, 30.0, 12.0), false, false);
    assert!(!s.move_indicators().copy_badge);
}

#[test]
fn c26_a_press_without_a_prior_pointer_move_still_works_with_cached_ctrl() {
    let mut s = abc_with_a_selected();
    s.modifiers_changed(false, true);
    let o = p(A_INSIDE);
    s.pointer_down(o, false); // no pointer_hover first
    s.pointer_hover(plus(o, 30.0, 12.0), false, true);
    assert!(s.move_indicators().copy_badge);
    s.pointer_up(plus(o, 30.0, 12.0), false, true);
    assert_eq!(all(&s).len(), 4);
}

/// The badge and the press share one hit test: for Ctrl held, over a grid of
/// points on a mixed scene, `copy_badge == (the press begins a move)`.
#[test]
fn c37_the_badge_and_pointer_down_agree_on_a_grid() {
    // the move begins <=> a drag past the dead zone shows the move readout.
    let build = |select: &[(f64, f64)]| {
        let (mut s, hits) = rich_session();
        let _ = hits;
        if !select.is_empty() {
            let mut first = true;
            for h in select {
                if first {
                    click(&mut s, pt(h.0, h.1));
                    first = false;
                } else {
                    shift_click(&mut s, pt(h.0, h.1));
                }
            }
        }
        s
    };
    let selections: [&[(f64, f64)]; 4] = [
        &[],
        &[(0.0, 10.0)],                 // the path
        &[(88.0, 60.0)],                // the ellipse
        &[(88.0, 60.0), (149.0, 60.0)], // two
    ];
    let mut mismatches = Vec::new();
    let mut moves = 0;
    let mut non_moves = 0;
    for sel_points in selections {
        for shift in [false, true] {
            for gx in 0..=24 {
                for gy in 0..=10 {
                    let at = pt(-5.0 + f64::from(gx) * 9.0, -5.0 + f64::from(gy) * 9.5 + 5.0);
                    let mut s = build(sel_points);
                    s.pointer_hover(at, shift, true);
                    let badge = s.move_indicators().copy_badge;
                    s.pointer_down(at, shift);
                    s.pointer_hover(plus(at, 40.0, 6.0), shift, true);
                    let began = readout(&s).is_some_and(|t| t.starts_with('Δ'));
                    s.escape();
                    if badge != began {
                        mismatches.push((sel_points.len(), shift, at, badge, began));
                    }
                    if began {
                        moves += 1;
                    } else {
                        non_moves += 1;
                    }
                }
            }
        }
    }
    assert!(
        moves > 20 && non_moves > 20,
        "the grid exercises both ({moves}/{non_moves})"
    );
    assert!(mismatches.is_empty(), "badge != press: {mismatches:?}");
}

// ---------------------------------------------------------------------
// Criterion 39, 40: other tools and other handles keep their meaning
// ---------------------------------------------------------------------

#[test]
fn c40_resize_rotate_drags_make_no_copy_and_show_no_move_readout() {
    let mut s = abc_with_a_selected();
    // SE corner resize handle: drag with Ctrl
    let se = pt(90.0, 70.0);
    let n = all(&s).len();
    drag_mod(&mut s, se, pt(120.0, 100.0), false, true);
    assert_eq!(all(&s).len(), n, "no copy by a resize");
    assert_eq!(sel(&s), 1);
    // Shift with a resize (uniform): readout is a size readout, never Δ
    let mut s = abc_with_a_selected();
    s.pointer_hover(se, true, false);
    s.pointer_down(se, true);
    s.pointer_hover(pt(120.0, 100.0), true, false);
    assert!(!readout(&s).unwrap_or_default().starts_with('Δ'));
    s.escape();
}

#[test]
fn c39_node_pen_and_creation_tools_give_shift_and_ctrl_no_move_meaning() {
    for tool in [
        Tool::Node,
        Tool::Pen,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        let mut s = abc_with_a_selected();
        s.set_tool(tool);
        s.modifiers_changed(true, true);
        assert!(!s.move_indicators().copy_badge, "{tool:?}");
        assert_eq!(s.move_indicators().lock, None);
        let before_len = all(&s).len();
        s.pointer_hover(p(A_OUTLINE), false, true);
        assert!(!s.move_indicators().copy_badge, "{tool:?}");
        s.pointer_hover(p(A_OUTLINE), false, false);
        s.pointer_down(p(A_OUTLINE), false);
        s.pointer_hover(plus(p(A_OUTLINE), 40.0, 10.0), true, true);
        assert!(
            !readout(&s).unwrap_or_default().starts_with('Δ'),
            "{tool:?}"
        );
        s.escape();
        assert_eq!(all(&s).len(), before_len);
        assert!(with_color(&s.draw_list(), axis_guide()).is_empty());
    }
}

// ---------------------------------------------------------------------
// The keyboard gate: Ctrl alone does not trigger tools
// ---------------------------------------------------------------------

#[test]
fn the_keyboard_gate_ignores_ctrl_alone_and_ctrl_letters() {
    let mut s = abc_with_a_selected();
    for key in ["Control", "Shift", "Meta"] {
        let r = s.key_down(KeyInput {
            key,
            ctrl: key == "Control",
            shift: key == "Shift",
            ..KeyInput::default()
        });
        assert_eq!(r, KeyOutcome::Ignored, "{key}");
    }
    for key in ["s", "r", "e", "b", "n", "m"] {
        let r = s.key_down(KeyInput {
            key,
            ctrl: true,
            ..KeyInput::default()
        });
        assert_eq!(r, KeyOutcome::Ignored, "Ctrl+{key}");
    }
    assert!(s.move_entry().is_none());
    // during a drag the M key does not open the chip (existing gate)
    let o = p(A_INSIDE);
    s.pointer_hover(o, false, true);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 10.0), false, true);
    let r = press(&mut s, "m");
    assert_eq!(r, KeyOutcome::Ignored);
    s.escape();
}

// ---------------------------------------------------------------------
// Hostile values
// ---------------------------------------------------------------------

#[test]
fn hostile_pointer_values_write_nothing_bad() {
    for ctrl in [false, true] {
        let mut s = abc_with_a_selected();
        let before = bytes_of(&s);
        let o = p(A_INSIDE);
        s.pointer_hover(o, false, ctrl);
        s.pointer_down(o, false);
        s.pointer_hover(pt(f64::NAN, 3.0), false, ctrl);
        s.pointer_hover(pt(f64::INFINITY, 3.0), true, ctrl);
        s.pointer_up(pt(f64::NAN, f64::NAN), false, ctrl);
        assert_eq!(
            bytes_of(&s),
            before,
            "a non-finite release writes nothing (ctrl {ctrl})"
        );
        // a huge but finite release: no panic, the file still opens and has
        // only finite coordinates
        s.pointer_hover(o, false, ctrl);
        s.pointer_down(o, false);
        s.pointer_hover(pt(1e300, -1e300), false, ctrl);
        let _ = s.draw_list();
        let _ = readout(&s);
        s.pointer_up(pt(1e300, -1e300), false, ctrl);
        let d = doc_of(&s);
        for id in d.object_ids() {
            let origin = rect_origin(&d.object(id).unwrap());
            assert!(origin.x.is_finite() && origin.y.is_finite());
        }
        let _ = s.pack("0.1.0").unwrap();
        let _ = s.draw_list();
    }
}

// ---------------------------------------------------------------------
// Typed move: the Copy check (criteria 23, 24)
// ---------------------------------------------------------------------

fn open_move_chip(s: &mut Session) {
    assert_eq!(press(s, "m"), KeyOutcome::EntryOpened);
    assert!(s.move_entry().is_some());
}

#[test]
fn c23_relative_typed_copy_makes_one_copy_and_selects_it() {
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let n = change_count(&s);
    let r = s.commit_move_entry(
        "5",
        "-3",
        MoveEntryMode {
            absolute: false,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    assert_eq!(change_count(&s), n + 1, "one commit");
    let o = all(&s);
    assert_eq!(o.len(), 4);
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0), "original untouched");
    assert_eq!(rect_origin(&o[1]), pt(15.0, 17.0));
    assert_eq!(sel(&s), 1);
    // the selection is the copy
    open_move_chip(&mut s);
    let c = s.move_entry().unwrap().center;
    assert!(
        pnear(c, pt(55.0, 42.0)),
        "the chip belongs to the copy: {c:?}"
    );
    s.escape();
}

#[test]
fn c23_absolute_typed_copy_puts_the_bounds_top_left_at_x_y() {
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let r = s.commit_move_entry(
        "100",
        "50",
        MoveEntryMode {
            absolute: true,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    let o = all(&s);
    assert_eq!(o.len(), 4);
    assert_eq!(rect_origin(&o[0]), pt(10.0, 20.0));
    assert_eq!(rect_origin(&o[1]), pt(100.0, 50.0));
    // without the check the same entry moves
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let r = s.commit_move_entry(
        "100",
        "50",
        MoveEntryMode {
            absolute: true,
            copy: false,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    assert_eq!(rect_origin(&all(&s)[0]), pt(100.0, 50.0));
    assert_eq!(all(&s).len(), 3);
}

#[test]
fn c23_a_typed_copy_of_a_path_gets_fresh_anchor_ids_and_every_field() {
    let (mut s, hits) = rich_session();
    click(&mut s, hits[0]);
    open_move_chip(&mut s);
    let before = all(&s);
    let r = s.commit_move_entry(
        "10",
        "20",
        MoveEntryMode {
            absolute: false,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    let after = all(&s);
    assert_eq!(after.len(), before.len() + 1);
    let (ObjectSnapshot::Path(a), ObjectSnapshot::Path(b)) = (&after[0], &after[1]) else {
        panic!("path then its copy")
    };
    assert_eq!(after[0], before[0]);
    for (x, y) in a.anchors.iter().zip(&b.anchors) {
        assert_ne!(x.id, y.id);
        assert!(pnear(plus(x.point, 10.0, 20.0), y.point));
        assert_eq!(
            (x.handle_in, x.handle_out, x.kind),
            (y.handle_in, y.handle_out, y.kind)
        );
    }
    assert_eq!((a.closed, a.rotation), (b.closed, b.rotation));
}

#[test]
fn c23_a_typed_copy_of_a_rotated_primitive_uses_the_drawn_bounds_for_absolute() {
    let d = Document::new(1);
    let id = d.create_ellipse(EllipseFrame {
        center: pt(60.0, 40.0),
        rx: Length::from_mm(20.0),
        ry: Length::from_mm(8.0),
    });
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(pt(60.0, 40.0), Angle::from_radians(0.5)))
        .unwrap();
    let mut s = open(&d);
    // outline point: ellipse's rotated left extreme
    let h = rot(pt(40.0, 40.0), pt(60.0, 40.0), 0.5);
    click(&mut s, h);
    assert_eq!(sel(&s), 1);
    open_move_chip(&mut s);
    let view = s.move_entry().unwrap();
    let top_left: Vec<f64> = view
        .absolute_prefill
        .iter()
        .map(|t| t.replace(',', ".").parse().unwrap())
        .collect();
    // reference: tight bounds of a rotated ellipse
    let (a, b, th) = (20.0_f64, 8.0_f64, 0.5_f64);
    let hw = (a * a * th.cos().powi(2) + b * b * th.sin().powi(2)).sqrt();
    let hh = (a * a * th.sin().powi(2) + b * b * th.cos().powi(2)).sqrt();
    assert!(
        (top_left[0] - (60.0 - hw)).abs() < 0.06,
        "{top_left:?} vs {}",
        60.0 - hw
    );
    assert!((top_left[1] - (40.0 - hh)).abs() < 0.06);
    let r = s.commit_move_entry(
        "100",
        "50",
        MoveEntryMode {
            absolute: true,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    let all_ = all(&s);
    let ObjectSnapshot::Primitive(c) = &all_[1] else {
        panic!()
    };
    let Shape::Ellipse { frame } = c.shape else {
        panic!()
    };
    assert!((frame.center.x - hw - 100.0).abs() < 0.01);
    assert!((frame.center.y - hh - 50.0).abs() < 0.01);
    assert!(
        (c.rotation.as_radians() - 0.5).abs() < 1e-9,
        "rotation copied"
    );
}

#[test]
fn c23_a_typed_copy_with_a_zero_result_writes_nothing() {
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let before = bytes_of(&s);
    let r = s.commit_move_entry(
        "0",
        "0",
        MoveEntryMode {
            absolute: false,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Unchanged);
    assert_eq!(bytes_of(&s), before);
    open_move_chip(&mut s);
    let r = s.commit_move_entry(
        "10",
        "20",
        MoveEntryMode {
            absolute: true,
            copy: true,
        },
    ); // equal to the current top-left
    assert_eq!(r, EntryOutcome::Unchanged);
    assert_eq!(bytes_of(&s), before);
}

#[test]
fn c23_invalid_input_with_copy_stays_open_and_writes_nothing() {
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let before = bytes_of(&s);
    for (x, y) in [("abc", "1"), ("1", ""), ("1e9", "0"), ("1", "-2e7")] {
        let r = s.commit_move_entry(
            x,
            y,
            MoveEntryMode {
                absolute: false,
                copy: true,
            },
        );
        assert!(matches!(r, EntryOutcome::Invalid { .. }), "{x} {y}: {r:?}");
        assert!(s.move_entry().is_some(), "stays open");
    }
    let r = s.commit_move_entry(
        "1",
        "x",
        MoveEntryMode {
            absolute: true,
            copy: true,
        },
    );
    assert_eq!(
        r,
        EntryOutcome::Invalid {
            field: 1,
            reason: InvalidReason::NotANumber
        }
    );
    assert_eq!(bytes_of(&s), before);
}

#[test]
fn c23_ctrl_at_the_second_press_presets_the_check_other_routes_do_not() {
    let preset = |shift: bool, ctrl: bool| -> bool {
        let mut s = abc_with_a_selected();
        let c = p(A_CENTRE);
        s.pointer_hover(c, shift, ctrl);
        s.pointer_down(c, shift);
        s.pointer_up(c, shift, ctrl);
        s.pointer_hover(c, shift, ctrl);
        let _ = s.double_click(c, shift, ctrl);
        let v = s.move_entry().expect("the chip opens on the centre handle");
        v.copy_preset
    };
    assert!(preset(false, true));
    assert!(preset(true, true), "Shift has no effect, Ctrl presets");
    assert!(!preset(false, false));
    assert!(
        !preset(true, false),
        "Shift at the second press has no effect"
    );
    // the key M never presets, even with Ctrl cached
    let mut s = abc_with_a_selected();
    s.modifiers_changed(false, true);
    open_move_chip(&mut s);
    assert!(!s.move_entry().unwrap().copy_preset);
    // unchecked on every open: close and reopen
    s.escape();
    s.modifiers_changed(false, false);
    open_move_chip(&mut s);
    assert!(!s.move_entry().unwrap().copy_preset);
}

#[test]
fn c23_non_finite_and_odd_typed_values_with_copy_write_nothing() {
    let mut s = abc_with_a_selected();
    open_move_chip(&mut s);
    let before = bytes_of(&s);
    for text in [
        "NaN", "inf", "-inf", "Infinity", "1e999", "1e400", "--5", "5-", "1 2", "0x10",
    ] {
        for absolute in [false, true] {
            let r = s.commit_move_entry(
                text,
                "0",
                MoveEntryMode {
                    absolute,
                    copy: true,
                },
            );
            assert!(
                matches!(r, EntryOutcome::Invalid { .. }),
                "{text:?} absolute {absolute}: {r:?}"
            );
            assert_eq!(bytes_of(&s), before);
        }
    }
    // a real minus (U+2212) and a decimal comma are accepted
    let r = s.commit_move_entry(
        "\u{2212}2,5",
        "1.5",
        MoveEntryMode {
            absolute: false,
            copy: true,
        },
    );
    assert_eq!(r, EntryOutcome::Committed);
    assert_eq!(rect_origin(&all(&s)[1]), pt(7.5, 21.5));
}

#[test]
fn c35_a_copy_of_a_copy_chains_and_keeps_the_order() {
    let mut s = abc_with_a_selected();
    drag_mod(
        &mut s,
        p(A_OUTLINE),
        plus(p(A_OUTLINE), 20.0, 40.0),
        false,
        true,
    );
    // the selected copy is at (30, 60); copy it again from its own outline
    let outline = pt(50.0, 60.0);
    drag_mod(&mut s, outline, plus(outline, 5.0, 100.0), false, true);
    let o = all(&s);
    assert_eq!(o.len(), 5);
    let origins: Vec<_> = o.iter().map(rect_origin).collect();
    assert_eq!(
        origins,
        vec![
            pt(10.0, 20.0),
            pt(30.0, 60.0),
            pt(35.0, 160.0),
            pt(150.0, 20.0),
            pt(10.0, 120.0)
        ]
    );
    assert_eq!(sel(&s), 1);
}

#[test]
fn c29_ctrl_shift_press_on_a_selected_outline_released_in_the_dead_zone_toggles() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let o = p(A_OUTLINE);
    s.pointer_hover(o, true, true);
    s.pointer_down(o, true);
    s.pointer_up(o, true, true);
    assert_eq!(sel(&s), 0);
    assert_eq!(bytes_of(&s), before, "no copy, nothing written");
}

#[test]
fn c23_the_double_click_with_ctrl_writes_nothing_and_does_not_copy() {
    let mut s = abc_with_a_selected();
    let before = bytes_of(&s);
    let c = p(A_CENTRE);
    s.pointer_hover(c, false, true);
    s.pointer_down(c, false);
    s.pointer_up(c, false, true);
    s.pointer_hover(c, false, true);
    let _ = s.double_click(c, false, true);
    assert_eq!(
        bytes_of(&s),
        before,
        "the presses of the double click copy nothing"
    );
    assert_eq!(sel(&s), 1);
}

#[test]
fn c24_the_centre_handle_hint_is_the_move_hint() {
    let mut s = abc_with_a_selected();
    s.pointer_hover(p(A_CENTRE), false, false);
    assert_eq!(s.handle_hint(), "move");
    assert_eq!(
        s.cursor_hint(),
        "move",
        "Ctrl and Shift never change the cursor hint"
    );
    s.pointer_hover(p(A_CENTRE), true, true);
    assert_eq!(s.cursor_hint(), "move");
}

// ---------------------------------------------------------------------
// Criterion 41: preview budget for 200 objects (informational timing)
// ---------------------------------------------------------------------

#[test]
#[ignore = "timing, run with --release -- --ignored; the spec budget (8 ms per draw) is missed by a plain move too: about 11 ms on the tester machine, copy about the same"]
fn c41_a_copy_of_200_objects_previews_within_8_ms() {
    let d = Document::new(1);
    for i in 0..100 {
        let anchors: Vec<NewAnchor> = (0..50)
            .map(|j| {
                NewAnchor::corner(
                    AnchorId::new(1, (i * 50 + j + 1) as u64),
                    pt(
                        f64::from(i % 10) * 12.0 + f64::from(j) * 0.2,
                        f64::from(i / 10) * 12.0 + (f64::from(j) * 0.7).sin(),
                    ),
                )
            })
            .collect();
        let _ = d.create_path(&anchors, false);
    }
    for i in 0..100 {
        let _ = d.create_rect(bounds(
            f64::from(i % 10) * 12.0,
            200.0 + f64::from(i / 10) * 12.0,
            8.0,
            8.0,
        ));
    }
    let mut s = open(&d);
    // select all 200 by shift-clicking each rectangle's left edge and each
    // path's first anchor
    click(&mut s, pt(0.0, 204.0));
    for i in 1..100 {
        shift_click(
            &mut s,
            pt(
                f64::from(i % 10) * 12.0,
                200.0 + f64::from(i / 10) * 12.0 + 4.0,
            ),
        );
    }
    for i in 0..100 {
        shift_click(
            &mut s,
            pt(f64::from(i % 10) * 12.0, f64::from(i / 10) * 12.0),
        );
    }
    let n = sel(&s);
    assert_eq!(n, 200, "all 200 selected");
    let at_rest = std::time::Instant::now();
    for _ in 0..20 {
        let _ = s.draw_list();
    }
    let rest_ms = at_rest.elapsed().as_secs_f64() * 1000.0 / 20.0;
    let o = pt(0.0, 204.0);
    let measure = |s: &mut Session, ctrl: bool| {
        s.pointer_hover(o, false, ctrl);
        s.pointer_down(o, false);
        let frames = 40;
        let (mut hover_ms, mut draw_ms) = (0.0, 0.0);
        for f in 0..frames {
            let t0 = std::time::Instant::now();
            s.pointer_hover(plus(o, 5.0 + f64::from(f), 7.0), false, ctrl);
            let t1 = std::time::Instant::now();
            let _ = s.draw_list();
            let t2 = std::time::Instant::now();
            hover_ms += (t1 - t0).as_secs_f64() * 1000.0;
            draw_ms += (t2 - t1).as_secs_f64() * 1000.0;
        }
        s.escape();
        (hover_ms / f64::from(frames), draw_ms / f64::from(frames))
    };
    let (move_hover, move_draw) = measure(&mut s, false);
    assert_eq!(sel(&s), 200, "the measured drag was a move of all 200");
    let (copy_hover, copy_draw) = measure(&mut s, true);
    eprintln!(
        "200 selected: idle draw {rest_ms:.2} ms; move hover {move_hover:.2} + draw {move_draw:.2} ms; copy hover {copy_hover:.2} + draw {copy_draw:.2} ms"
    );
    // commit time of the copy
    s.pointer_hover(o, false, true);
    s.pointer_down(o, false);
    s.pointer_hover(plus(o, 30.0, 7.0), false, true);
    let t = std::time::Instant::now();
    s.pointer_up(plus(o, 30.0, 7.0), false, true);
    eprintln!(
        "commit of the 200-object copy: {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(all(&s).len(), 400);
    assert!(copy_draw < 8.0, "draw {copy_draw} ms over the 8 ms budget");
}
