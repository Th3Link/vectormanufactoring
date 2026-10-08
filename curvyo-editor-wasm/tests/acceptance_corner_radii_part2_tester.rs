//! Independent black-box tests of PART 2 of `specs/rectangle-corner-radii/`
//! through `Session`'s public API: the Link switch and its lifetime, Shift as
//! an exclusive-or, per-corner drags and their commits, readouts, followers,
//! the typed entry, the bar's Radius field, Remove rounding, the Scale-corner-
//! radius interplay, hint lines, peers, old files, hostile values and "only
//! rectangles". Written from the specification before the implementation was
//! read. Knob positions come from the formula of criterion 1 restated here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines, clippy::similar_names, clippy::doc_markdown)]
#![allow(clippy::many_single_char_names, clippy::cast_precision_loss)]
#![allow(missing_docs, clippy::needless_range_loop, clippy::type_complexity)]

use std::f64::consts::SQRT_2;
use std::io::{Cursor, Write};

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, CornerRadii, Document, EllipseFrame,
    Length, NodeId, ObjectSnapshot, Point, PointCount, RectBounds, Shape, StarFrame, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::RgbaColor;
use curvyo_ui_core::{BarValue, EntryOutcome};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn radii(r: [f64; 4]) -> CornerRadii {
    CornerRadii {
        tl: mm(r[0]),
        tr: mm(r[1]),
        br: mm(r[2]),
        bl: mm(r[3]),
    }
}

fn arr(r: CornerRadii) -> [f64; 4] {
    [r.tl.as_mm(), r.tr.as_mm(), r.br.as_mm(), r.bl.as_mm()]
}

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

// -------------------- the specification's formulas, restated ----------------

fn css(w: f64, h: f64, r: [f64; 4]) -> [f64; 4] {
    let mut f = 1.0_f64;
    for (n, d) in [
        (w, r[0] + r[1]),
        (w, r[3] + r[2]),
        (h, r[0] + r[3]),
        (h, r[1] + r[2]),
    ] {
        if d > 0.0 {
            f = f.min(n / d);
        }
    }
    r.map(|x| x * f)
}

fn inward(i: usize) -> (f64, f64) {
    let s = 1.0 / SQRT_2;
    match i {
        0 => (s, s),
        1 => (-s, s),
        2 => (-s, -s),
        _ => (s, -s),
    }
}

fn knob_local(w: f64, h: f64, scale: f64, eff: [f64; 4], i: usize) -> (f64, f64) {
    let shorter = w.min(h);
    let s_px = shorter * scale;
    let long_px = w.max(h) * scale;
    let travel = (s_px - 14.0) / SQRT_2 - 15.0;
    let rho = |k: usize| eff[k] / (shorter / 2.0);
    let sigma = 2.0 + SQRT_2 * (long_px - s_px) / travel;
    let own = rho(i);
    let shown = if own <= 1.0 {
        own
    } else {
        own.min((sigma - rho((i + 2) % 4)).max(1.0))
    };
    let along = (15.0 + shown * travel) / scale / SQRT_2;
    let c = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)][i];
    let d = inward(i);
    (c.0 + d.0.signum() * along, c.1 + d.1.signum() * along)
}

// -------------------------------- fixtures ---------------------------------

const ORIGIN: (f64, f64) = (10.0, 20.0);

struct Fx {
    s: Session,
    w: f64,
    h: f64,
}

fn build(w: f64, h: f64, stored: [f64; 4]) -> (Vec<u8>, NodeId) {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(ORIGIN.0, ORIGIN.1),
        width: mm(w),
        height: mm(h),
    });
    d.set_corner_radii(&[(id, radii(stored))]).unwrap();
    (pack(&d, "0.1.0").unwrap(), id)
}

fn fx_peer(peer: u64, w: f64, h: f64, stored: [f64; 4]) -> Fx {
    let (bytes, _) = build(w, h, stored);
    let mut s = Session::open(peer, &bytes).unwrap();
    s.set_tool(Tool::Select);
    click(&mut s, pt(ORIGIN.0, ORIGIN.1 + h / 2.0), false);
    Fx { s, w, h }
}

/// A 100 x 60 mm rectangle.
fn fx(stored: [f64; 4]) -> Fx {
    fx_peer(2, 100.0, 60.0, stored)
}

fn click(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, false);
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn stored_of(s: &Session) -> [f64; 4] {
    let d = doc_of(s);
    let ObjectSnapshot::Primitive(p) = d.object(d.object_ids()[0]).unwrap() else {
        panic!("a primitive");
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!("a rectangle");
    };
    arr(corner_radii)
}

fn bounds_of(s: &Session) -> (f64, f64, f64, f64) {
    let d = doc_of(s);
    let ObjectSnapshot::Primitive(p) = d.object(d.object_ids()[0]).unwrap() else {
        panic!();
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!();
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

fn changes(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn vv(s: &Session) -> loro::VersionVector {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.oplog_vv()
}

fn corner_key_writes(s: &Session, from: &loro::VersionVector) -> [usize; 4] {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    let ops = format!("{:?}", l.export_json_updates(from, &l.oplog_vv()));
    ["tl", "tr", "br", "bl"].map(|c| ops.matches(&format!("\"corner_radius_{c}\"")).count())
}

/// `p` moved by `d` millimetres along corner `i`'s inward diagonal.
fn along(p: Point, i: usize, d: f64) -> Point {
    let v = inward(i);
    pt(p.x + v.0 * d, p.y + v.1 * d)
}

impl Fx {
    fn scale(&self) -> f64 {
        self.s.view().scale()
    }

    fn eff(&self) -> [f64; 4] {
        css(self.w, self.h, stored_of(&self.s))
    }

    fn knob(&self, i: usize) -> Point {
        let (lx, ly) = knob_local(self.w, self.h, self.scale(), self.eff(), i);
        pt(ORIGIN.0 + lx, ORIGIN.1 + ly)
    }

    fn press(&mut self, i: usize, shift: bool) -> Point {
        let at = self.knob(i);
        self.s.pointer_hover(at, shift, false);
        self.s.pointer_down(at, shift);
        at
    }

    /// One whole drag: press (with `shift_press`), move, release (with `shift_end`).
    fn drag(&mut self, i: usize, shift_press: bool, shift_end: bool, delta_mm: f64) {
        let from = self.press(i, shift_press);
        let to = along(from, i, delta_mm);
        self.s.pointer_hover(to, shift_end, false);
        self.s.pointer_up(to, shift_end, false);
    }

    fn readout(&self) -> Option<String> {
        self.s.live_readout().map(|r| r.text)
    }
}

// ------------------------------- 1. the switch ------------------------------

#[test]
fn the_switch_defaults_to_linked_survives_selection_and_tool_changes_and_is_reset_by_new_and_open()
{
    assert!(Session::new(1).link_corners(), "New: linked");
    let mut f = fx([1.0, 2.0, 3.0, 4.0]);
    assert!(f.s.link_corners(), "a freshly opened file: linked");
    f.s.set_link_corners(false);
    assert!(!f.s.link_corners());
    // Session state across selection and tool changes.
    click(&mut f.s, pt(500.0, 500.0), false);
    f.s.set_tool(Tool::Pen);
    f.s.set_tool(Tool::Select);
    assert!(!f.s.link_corners(), "kept for the session");
    // Never written to the project: a saved file reopens linked.
    let reopened = Session::open(7, &f.s.pack("0.1.0").unwrap()).unwrap();
    assert!(reopened.link_corners(), "Open resets it");
    // A fresh session: New.
    let mut other = Session::new(8);
    other.set_link_corners(false);
    assert!(Session::new(9).link_corners());
}

#[test]
fn toggling_the_switch_writes_nothing_and_changes_no_radius() {
    let mut f = fx([1.0, 2.0, 3.0, 4.0]);
    let (n, v) = (changes(&f.s), vv(&f.s));
    for on in [false, true, false, true, true] {
        f.s.set_link_corners(on);
    }
    assert_eq!(changes(&f.s), n);
    assert_eq!(corner_key_writes(&f.s, &v), [0; 4]);
    assert_eq!(stored_of(&f.s), [1.0, 2.0, 3.0, 4.0]);
    // The other switches are not touched by it.
    assert!(!f.s.scale_corner_radius());
}

#[test]
fn a_click_on_the_switch_mid_drag_changes_nothing_until_the_next_drag() {
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = f.press(0, false);
    f.s.set_link_corners(false);
    let to = along(from, 0, 6.0);
    f.s.pointer_hover(to, false, false);
    f.s.pointer_up(to, false, false);
    let a = stored_of(&f.s);
    assert!(
        a.iter().all(|v| *v == a[0]),
        "started linked: all four: {a:?}"
    );
    // The next drag is unlinked.
    let before = stored_of(&f.s);
    f.drag(0, false, false, -3.0);
    let after = stored_of(&f.s);
    assert!(after[0] < before[0]);
    assert_eq!(&after[1..], &before[1..]);
}

// ------------------------- 2. Shift as exclusive-or --------------------------

#[test]
fn switch_and_shift_make_four_equal_or_one_corner_for_every_corner() {
    for linked in [true, false] {
        for shift in [false, true] {
            for i in 0..4 {
                let start = [5.0, 0.0, 12.0, 3.0];
                let mut f = fx(start);
                f.s.set_link_corners(linked);
                // Shift only at the press; released before the pointer-up.
                f.drag(i, shift, false, 5.0);
                let a = stored_of(&f.s);
                assert!(a[i] > start[i] + 0.5, "corner {i} grows");
                if linked ^ shift {
                    assert!(
                        a.iter().all(|v| *v == a[i]),
                        "linked {linked} shift {shift} corner {i}: {a:?}"
                    );
                } else {
                    for k in 0..4 {
                        if k != i {
                            assert_eq!(
                                a[k], start[k],
                                "linked {linked} shift {shift} corner {i} k {k}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn shift_pressed_or_released_mid_drag_changes_nothing() {
    // Linked, press without Shift, Shift down for the rest of the drag.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = f.press(0, false);
    f.s.modifiers_changed(true, false);
    let to = along(from, 0, 6.0);
    f.s.pointer_hover(to, true, false);
    f.s.pointer_up(to, true, false);
    let a = stored_of(&f.s);
    assert!(a.iter().all(|v| *v == a[0]), "{a:?}");
    // Linked, press with Shift (one corner), Shift released mid-drag.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = f.press(0, true);
    f.s.modifiers_changed(false, false);
    let to = along(from, 0, 6.0);
    f.s.pointer_hover(to, false, false);
    f.s.pointer_up(to, false, false);
    let a = stored_of(&f.s);
    assert_eq!(&a[1..], &[0.0, 12.0, 3.0]);
    assert!(a[0] > 5.0);
}

// ------------------------- one commit per interaction ------------------------

#[test]
fn every_interaction_is_one_commit_and_writes_only_the_registers_that_change() {
    // Unlinked drag from f = 1: exactly one commit, one register.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let (n, v) = (changes(&f.s), vv(&f.s));
    f.drag(2, false, false, 4.0);
    assert_eq!(changes(&f.s), n + 1);
    assert_eq!(corner_key_writes(&f.s, &v), [0, 0, 1, 0]);

    // Linked drag on equal radii: one commit, four registers.
    let mut f = fx([5.0; 4]);
    let (n, v) = (changes(&f.s), vv(&f.s));
    f.drag(0, false, false, 4.0);
    assert_eq!(changes(&f.s), n + 1);
    assert_eq!(corner_key_writes(&f.s, &v), [1, 1, 1, 1]);

    // Linked drag on a start that already has one of the values: that register is not rewritten.
    let mut f = fx([0.0, 0.0, 0.0, 0.0]);
    let (n, v) = (changes(&f.s), vv(&f.s));
    f.drag(0, false, false, 4.0);
    assert_eq!(changes(&f.s), n + 1);
    assert_eq!(corner_key_writes(&f.s, &v), [1, 1, 1, 1]);

    // A drag that ends where it began, Escape, and a press without movement write nothing.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let (n, v) = (changes(&f.s), vv(&f.s));
    f.drag(1, false, false, 0.0);
    let from = f.press(1, false);
    f.s.pointer_hover(along(from, 1, 8.0), false, false);
    let _ = f.s.escape();
    f.s.pointer_up(along(from, 1, 8.0), false, false);
    assert_eq!(changes(&f.s), n);
    assert_eq!(corner_key_writes(&f.s, &v), [0; 4]);
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
    assert!(f.readout().is_none(), "no readout after Escape");
}

#[test]
fn an_unlinked_drag_on_a_shrunk_rectangle_is_one_commit_that_fixes_the_other_three() {
    let mut f = fx([50.0, 50.0, 0.0, 50.0]); // 100 x 60: f = 0.6 -> drawn 30, 30, 0, 30
    f.s.set_link_corners(false);
    let drawn = f.eff();
    assert!(close(drawn[0], 30.0, 1e-9));
    let n = changes(&f.s);
    f.drag(2, false, false, 4.0);
    assert_eq!(changes(&f.s), n + 1);
    let a = stored_of(&f.s);
    assert!(
        close(a[0], 30.0, 1e-9) && close(a[1], 30.0, 1e-9) && close(a[3], 30.0, 1e-9),
        "{a:?}"
    );
    assert!(a[2] > 0.0);
    // Nothing moved on screen: the effective values of the others are what they were.
    let e = f.eff();
    assert!(close(e[0], 30.0, 1e-9) && close(e[1], 30.0, 1e-9) && close(e[3], 30.0, 1e-9));
}

// --------------------------------- readouts ----------------------------------

#[test]
fn readout_texts_follow_the_specification() {
    // "r 12.3 mm" with one decimal, "max" at the unlinked limit, " · all corners".
    let mut f = fx([0.0, 40.0, 0.0, 30.0]);
    f.s.set_link_corners(false);
    let from = f.press(0, false);
    assert!(
        f.readout().is_none(),
        "no readout before the pointer leaves the dead zone"
    );
    f.s.pointer_hover(along(from, 0, 3.0), false, false);
    let t = f.readout().unwrap();
    assert!(
        t.starts_with("r ") && t.ends_with(" mm") && !t.contains("max") && !t.contains('\u{b7}'),
        "{t}"
    );
    let num: f64 = t[2..t.len() - 3].parse().unwrap();
    assert!(
        num > 0.0 && t.split('.').nth(1).unwrap().trim_end_matches(" mm").len() == 1,
        "{t}"
    );
    f.s.pointer_hover(along(from, 0, 900.0), false, false);
    // limit = min(100 - 40, 60 - 30) = 30
    assert_eq!(f.readout().unwrap(), "r 30.0 mm max");
    f.s.pointer_hover(along(from, 0, 3.0), false, false);
    assert!(
        !f.readout().unwrap().contains("max"),
        "max goes away when the limit does"
    );
    f.s.pointer_up(along(from, 0, 3.0), false, false);
    assert!(
        f.readout().is_none(),
        "the readout is gone after the release"
    );

    // Linked, equal start: no suffix; far: "max" at half the shorter side.
    let mut f = fx([5.0; 4]);
    let from = f.press(1, false);
    f.s.pointer_hover(along(from, 1, 900.0), false, false);
    assert_eq!(f.readout().unwrap(), "r 30.0 mm max");
    f.s.pointer_hover(along(from, 1, 2.0), false, false);
    let t = f.readout().unwrap();
    assert!(
        !t.contains('\u{b7}'),
        "equal at the press: no overwrite note: {t}"
    );
    let _ = f.s.escape();

    // Linked, unequal start: " · all corners", joined to "max".
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = f.press(0, false);
    f.s.pointer_hover(along(from, 0, 2.0), false, false);
    assert!(
        f.readout().unwrap().ends_with(" mm \u{b7} all corners")
            || f.readout().unwrap().ends_with("mm \u{b7} all corners"),
        "{:?}",
        f.readout()
    );
    f.s.pointer_hover(along(from, 0, 900.0), false, false);
    assert_eq!(f.readout().unwrap(), "r 30.0 mm max \u{b7} all corners");
    let _ = f.s.escape();

    // Switch off + Shift = all four: the overwrite note applies; Shift with switch on: it does not.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    f.s.set_link_corners(false);
    let from = f.press(0, true);
    f.s.pointer_hover(along(from, 0, 2.0), true, false);
    assert!(
        f.readout().unwrap().contains("all corners"),
        "{:?}",
        f.readout()
    );
    let _ = f.s.escape();
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = f.press(0, true);
    f.s.pointer_hover(along(from, 0, 2.0), true, false);
    assert!(
        !f.readout().unwrap().contains("all corners"),
        "{:?}",
        f.readout()
    );
    let _ = f.s.escape();
}

fn hover_ground() -> RgbaColor {
    RgbaColor {
        r: 0x2F,
        g: 0x6F,
        b: 0xEE,
        a: 51,
    }
}

/// How many hover-ground vertices the picture holds (each hovered knob is one disc).
fn hover_vertices(s: &Session) -> usize {
    s.draw_list()
        .triangles
        .iter()
        .filter(|v| v.color == hover_ground())
        .count()
}

#[test]
fn followers_take_the_hover_ground_only_in_a_drag_that_changes_all_four() {
    // One hovered knob is one disc of hover ground: the unit.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let idle = hover_vertices(&f.s);
    let at = f.knob(1);
    f.s.pointer_hover(at, false, false);
    let unit = hover_vertices(&f.s) - idle;
    assert!(unit > 0, "a hovered knob shows the hover ground");
    f.s.pointer_hover(pt(60.0, 50.0), false, false);

    for (linked, shift, all) in [
        (true, false, true),
        (true, true, false),
        (false, true, true),
        (false, false, false),
    ] {
        let mut f = fx([5.0, 0.0, 12.0, 3.0]);
        f.s.set_link_corners(linked);
        let base = hover_vertices(&f.s);
        let from = f.press(0, shift);
        f.s.pointer_hover(along(from, 0, 5.0), shift, false);
        let during = hover_vertices(&f.s) - base;
        // The dragged knob is solid; the three others are followers or idle.
        if all {
            assert_eq!(
                during,
                3 * unit,
                "linked {linked} shift {shift}: three followers"
            );
        } else {
            assert_eq!(
                during, 0,
                "linked {linked} shift {shift}: the others stay idle"
            );
        }
        let _ = f.s.escape();
    }
}

fn black_vertices(s: &Session) -> Vec<(u64, u64)> {
    let mut v: Vec<(u64, u64)> = s
        .draw_list()
        .triangles
        .iter()
        .filter(|v| v.color == RgbaColor::BLACK)
        .map(|v| (v.position.x.to_bits(), v.position.y.to_bits()))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn the_black_original_stays_while_the_drag_previews_and_escape_restores_the_picture() {
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let before = black_vertices(&f.s);
    assert!(!before.is_empty());
    let from = f.press(0, false);
    f.s.pointer_hover(along(from, 0, 8.0), false, false);
    assert_eq!(
        black_vertices(&f.s),
        before,
        "the original is drawn unchanged under the preview"
    );
    let _ = f.s.escape();
    assert_eq!(black_vertices(&f.s), before);
    // After a real release the black outline is the new one.
    f.drag(0, false, false, 8.0);
    assert_ne!(black_vertices(&f.s), before);
}

// ---------------------------------- hints -----------------------------------

#[test]
fn hint_lines_follow_the_switch_never_shift_and_name_no_corner() {
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let at = f.knob(0);
    f.s.pointer_hover(at, false, false);
    let on = f.s.corner_hint_lines();
    assert_eq!(
        on,
        [
            "Corners differ. Dragging sets all four to one value.",
            "Corner radius, all four",
            "Shift: this corner only",
            "Double-click: type a value"
        ]
    );
    // Shift held while hovering: unchanged (no live tracking).
    f.s.pointer_hover(at, true, false);
    f.s.modifiers_changed(true, false);
    assert_eq!(f.s.corner_hint_lines(), on);
    f.s.modifiers_changed(false, false);
    f.s.set_link_corners(false);
    assert_eq!(
        f.s.corner_hint_lines(),
        [
            "Corner radius, this corner",
            "Shift: all four corners",
            "Double-click: type a value"
        ]
    );
    for line in f.s.corner_hint_lines() {
        for word in ["top", "bottom", "left", "right", "Top", "Bottom"] {
            assert!(!line.contains(word), "{line}");
        }
    }
    // Off a knob: none.
    f.s.pointer_hover(pt(60.0, 50.0), false, false);
    assert!(f.s.corner_hint_lines().is_empty());
}

#[test]
fn a_limited_corner_adds_its_own_values_and_the_fix_note_only_when_the_next_drag_is_one_corner() {
    // 100 x 60, 50/50/0/50 -> shown 30 for TL, TR, BL, stored 50.
    let mut f = fx([50.0, 50.0, 0.0, 50.0]);
    for i in [0usize, 1, 3] {
        let at = f.knob(i);
        f.s.pointer_hover(at, false, false);
        let l = f.s.corner_hint_lines();
        assert_eq!(
            l[0], "Limited by the size. Stored 50 mm, shown 30 mm.",
            "corner {i}"
        );
        assert!(
            !l.iter().any(|x| x.contains("fixes the other three")),
            "linked: {l:?}"
        );
        // Limited note, the corners-differ warning (linked), then the three lines.
        assert_eq!(l.len(), 5);
        assert!(l[1].starts_with("Corners differ."), "{l:?}");
    }
    f.s.set_link_corners(false);
    let at = f.knob(0);
    f.s.pointer_hover(at, false, false);
    let l = f.s.corner_hint_lines();
    assert!(
        l.contains(&"Editing one corner fixes the other three at their shown size.".to_string()),
        "{l:?}"
    );
    // BR (stored 0, shown 0) is not limited: no limited line even though the neighbours are.
    let at = f.knob(2);
    f.s.pointer_hover(at, false, false);
    let l = f.s.corner_hint_lines();
    assert!(!l.iter().any(|x| x.starts_with("Limited")), "{l:?}");
    // A rectangle that is not shrunk has no limited line.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let at = f.knob(0);
    f.s.pointer_hover(at, false, false);
    let l = f.s.corner_hint_lines();
    // Unequal corners, linked: only the corners-differ warning is added.
    assert_eq!(l.len(), 4);
    assert!(l[0].starts_with("Corners differ."), "{l:?}");
}

// -------------------------------- typed entry --------------------------------

struct Opened {
    kind: &'static str,
    scope: Option<&'static str>,
    name: &'static str,
    prefill: String,
    fields: usize,
    label: &'static str,
}

fn open_entry(f: &mut Fx, i: usize, shift_second_press: bool) -> Opened {
    let at = f.knob(i);
    f.s.pointer_hover(at, false, false);
    f.s.pointer_down(at, false);
    f.s.pointer_up(at, false, false);
    f.s.pointer_hover(at, shift_second_press, false);
    f.s.double_click(at, shift_second_press, false);
    let e = f.s.transform_entry().expect("the entry is open");
    Opened {
        kind: e.kind,
        scope: e.scope,
        name: e.fields[0].accessible_name,
        prefill: e.fields[0].prefill.clone(),
        fields: e.fields.len(),
        label: e.fields[0].label,
    }
}

#[test]
fn the_entry_has_one_field_r_a_scope_row_and_the_right_accessible_name() {
    let names = [
        "Top-left corner radius",
        "Top-right corner radius",
        "Bottom-right corner radius",
        "Bottom-left corner radius",
    ];
    for i in 0..4 {
        for (linked, shift) in [(true, false), (true, true), (false, false), (false, true)] {
            let mut f = fx([5.0, 10.0, 15.0, 20.0]);
            f.s.set_link_corners(linked);
            let o = open_entry(&mut f, i, shift);
            assert_eq!(o.fields, 1);
            assert_eq!(o.label, "r");
            assert_eq!(o.kind, "corner-radius");
            let one = linked == shift;
            let _ = one;
            let unlinked = linked == shift;
            if unlinked {
                assert_eq!(o.scope, Some("This corner only"));
                assert_eq!(o.name, names[i]);
            } else {
                assert_eq!(o.scope, Some("All four corners"));
                assert_eq!(o.name, "Corner radius, all corners");
            }
            // The field shows the corner's own effective radius.
            let want = [5.0, 10.0, 15.0, 20.0][i];
            let got: f64 = o.prefill.parse().unwrap();
            assert!(close(got, want, 0.05), "prefill {} for {want}", o.prefill);
        }
    }
}

#[test]
fn enter_applies_to_the_scope_the_entry_was_opened_with() {
    // One corner.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let n = changes(&f.s);
    let _ = open_entry(&mut f, 1, false);
    assert_eq!(
        f.s.commit_transform_entry("7", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [5.0, 7.0, 15.0, 20.0]);
    assert_eq!(changes(&f.s), n + 1);
    // All four.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 1, false);
    assert_eq!(
        f.s.commit_transform_entry("7", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [7.0; 4]);
    // Zero is valid.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let _ = open_entry(&mut f, 0, false);
    assert_eq!(
        f.s.commit_transform_entry("0", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [0.0, 10.0, 15.0, 20.0]);
    // Shift at the second press inverts the scope; the switch changed afterwards closes the entry.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 3, true);
    assert_eq!(
        f.s.commit_transform_entry("4", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 4.0]);
}

#[test]
fn a_value_over_the_limit_is_limited_written_and_never_silent() {
    // Unlinked TL with TR 40 and BL 30: limit min(100 - 40, 60 - 30) = 30.
    let mut f = fx([0.0, 40.0, 0.0, 30.0]);
    f.s.set_link_corners(false);
    let _ = open_entry(&mut f, 0, false);
    assert_eq!(
        f.s.commit_transform_entry("55", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [30.0, 40.0, 0.0, 30.0]);
    assert_eq!(f.readout().expect("a notice"), "r 30.0 mm max");
    f.s.clear_limit_notice();
    assert!(f.readout().is_none());
    // Linked: half the shorter side.
    let mut f = fx([0.0; 4]);
    let _ = open_entry(&mut f, 2, false);
    assert_eq!(
        f.s.commit_transform_entry("999", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(stored_of(&f.s), [30.0; 4]);
    assert_eq!(f.readout().unwrap(), "r 30.0 mm max");
    // A value that is inside the limit shows no notice.
    let mut f = fx([0.0; 4]);
    let _ = open_entry(&mut f, 2, false);
    assert_eq!(
        f.s.commit_transform_entry("12", "", 0),
        EntryOutcome::Committed
    );
    assert!(f.readout().is_none());
}

#[test]
fn invalid_text_keeps_the_entry_open_and_writes_nothing_untouched_enter_and_escape_close_without_writing()
 {
    for bad in ["", "abc", "-3", "NaN", "inf", "1e999", "1,2,3", "--1"] {
        let mut f = fx([5.0, 10.0, 15.0, 20.0]);
        let n = changes(&f.s);
        let _ = open_entry(&mut f, 0, false);
        let out = f.s.commit_transform_entry(bad, "", 0);
        assert!(
            matches!(out, EntryOutcome::Invalid { .. }),
            "{bad:?}: {out:?}"
        );
        assert!(f.s.transform_entry().is_some(), "{bad:?}: stays open");
        assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
        assert_eq!(changes(&f.s), n);
    }
    // Untouched Enter: closes without writing.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let n = changes(&f.s);
    let o = open_entry(&mut f, 0, false);
    let out = f.s.commit_transform_entry(&o.prefill, "", 0);
    assert_eq!(out, EntryOutcome::Unchanged);
    assert!(f.s.transform_entry().is_none());
    assert_eq!(changes(&f.s), n);
    // Escape.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 0, false);
    let _ = f.s.escape();
    assert!(f.s.transform_entry().is_none());
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
    // cancel (blur).
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 0, false);
    f.s.cancel_transform_entry();
    assert!(f.s.transform_entry().is_none());
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
}

#[test]
fn a_press_elsewhere_a_tool_switch_and_the_switches_close_the_entry_without_writing() {
    // A press elsewhere is not swallowed: it deselects.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 0, false);
    click(&mut f.s, pt(500.0, 500.0), false);
    assert!(f.s.transform_entry().is_none());
    assert_eq!(
        f.s.selected_object_count(),
        0,
        "the closing press was processed"
    );
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
    // Tool switch.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    let _ = open_entry(&mut f, 0, false);
    f.s.set_tool(Tool::Pen);
    assert!(f.s.transform_entry().is_none());
    assert_eq!(stored_of(&f.s), [5.0, 10.0, 15.0, 20.0]);
    // Each of the three switches.
    for which in 0..3 {
        let mut f = fx([5.0, 10.0, 15.0, 20.0]);
        let n = changes(&f.s);
        let _ = open_entry(&mut f, 0, false);
        match which {
            0 => f.s.set_link_corners(false),
            1 => f.s.set_scale_corner_radius(true),
            _ => f.s.set_scale_stroke_width(true),
        }
        assert!(f.s.transform_entry().is_none(), "switch {which}");
        assert_eq!(changes(&f.s), n);
    }
    // The entry that was opened keeps its scope: a click on the switch after Enter is moot, but
    // the scope row of a freshly opened chip follows the new switch value.
    let mut f = fx([5.0, 10.0, 15.0, 20.0]);
    f.s.set_link_corners(false);
    let o = open_entry(&mut f, 0, false);
    assert_eq!(o.scope, Some("This corner only"));
}

#[test]
fn a_typed_entry_on_a_shrunk_rectangle_fixes_the_other_three_at_their_shown_size() {
    let mut f = fx([50.0, 50.0, 0.0, 50.0]);
    f.s.set_link_corners(false);
    let o = open_entry(&mut f, 0, false);
    let shown: f64 = o.prefill.parse().unwrap();
    assert!(close(shown, 30.0, 0.05), "{}", o.prefill);
    let n = changes(&f.s);
    assert_eq!(
        f.s.commit_transform_entry("10", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(changes(&f.s), n + 1);
    let a = stored_of(&f.s);
    assert!(
        close(a[0], 10.0, 1e-9)
            && close(a[1], 30.0, 1e-9)
            && close(a[2], 0.0, 1e-9)
            && close(a[3], 30.0, 1e-9),
        "{a:?}"
    );
}

// ------------------------------- the bar field --------------------------------

fn two_rects(a: [f64; 4], b: [f64; 4]) -> (Session, NodeId, NodeId) {
    let d = Document::new(1);
    let ia = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(100.0),
        height: mm(60.0),
    });
    let ib = d.create_rect(RectBounds {
        origin: pt(200.0, 0.0),
        width: mm(40.0),
        height: mm(40.0),
    });
    d.set_corner_radii(&[(ia, radii(a)), (ib, radii(b))])
        .unwrap();
    let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    click(&mut s, pt(0.0, 30.0), false);
    click(&mut s, pt(200.0, 20.0), true);
    (s, ia, ib)
}

fn stored_all(s: &Session) -> Vec<[f64; 4]> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .map(|id| {
            let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
                panic!()
            };
            let Shape::Rect { corner_radii, .. } = p.shape else {
                panic!()
            };
            arr(corner_radii)
        })
        .collect()
}

#[test]
fn the_bar_field_reads_mixed_with_four_values_uniform_with_a_limited_tag_and_several_rectangles() {
    let f = fx([12.0, 0.0, 12.0, 0.0]);
    let bar = f.s.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Mixed));
    let c = bar.radius_corners.expect("four values for the tooltip");
    assert_eq!(
        [c.tl, c.tr, c.br, c.bl].map(Length::as_mm),
        [12.0, 0.0, 12.0, 0.0]
    );
    assert!(bar.radius_limited.is_none(), "Mixed shows no tag");
    assert!(bar.remove_rounding_shown && bar.remove_rounding_enabled);

    // Uniform: the value; "limited" only when a stored radius is above its effective one.
    let f = fx([8.0; 4]);
    let bar = f.s.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Uniform(mm(8.0))));
    assert!(bar.radius_limited.is_none() && bar.radius_corners.is_none());
    let f = fx([40.0; 4]);
    let bar = f.s.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Uniform(mm(30.0))));
    assert_eq!(bar.radius_limited, Some(mm(40.0)));

    // Unequal stored but equal effective after the clamp (all corners limited alike): Uniform.
    let f = fx([40.0, 40.0, 50.0, 50.0]);
    let bar = f.s.select_bar_state();
    // f = min(60/80, 60/100... ) -> 0.6: effective 24, 24, 30, 30: unequal -> Mixed.
    assert_eq!(bar.radius, Some(BarValue::Mixed));

    // Several rectangles: Uniform if all effective radii of all are equal, else Mixed; no corners list.
    let (s, _, _) = two_rects([5.0; 4], [5.0; 4]);
    assert_eq!(
        s.select_bar_state().radius,
        Some(BarValue::Uniform(mm(5.0)))
    );
    let (s, _, _) = two_rects([5.0; 4], [5.0, 5.0, 5.0, 4.0]);
    let bar = s.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Mixed));
    assert!(
        bar.radius_corners.is_none(),
        "the four values are for one rectangle"
    );
}

#[test]
fn typing_in_the_bar_field_sets_all_four_ignores_the_switch_and_limits_each_rectangle() {
    for linked in [true, false] {
        let mut f = fx([5.0, 0.0, 12.0, 3.0]);
        f.s.set_link_corners(linked);
        let n = changes(&f.s);
        assert_eq!(f.s.set_selected_radius_text("7"), EntryOutcome::Committed);
        assert_eq!(stored_of(&f.s), [7.0; 4]);
        assert_eq!(changes(&f.s), n + 1);
        assert_eq!(f.s.set_selected_radius_text("999"), EntryOutcome::Committed);
        assert_eq!(
            stored_of(&f.s),
            [30.0; 4],
            "limited to half the shorter side"
        );
        assert_eq!(f.s.set_selected_radius_text("0"), EntryOutcome::Committed);
        assert_eq!(stored_of(&f.s), [0.0; 4]);
    }
    // Invalid input writes nothing.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let n = changes(&f.s);
    for bad in ["", "x", "-2", "NaN", "inf"] {
        let out = f.s.set_selected_radius_text(bad);
        assert!(
            matches!(out, EntryOutcome::Invalid { .. }),
            "{bad:?}: {out:?}"
        );
    }
    assert_eq!(changes(&f.s), n);
    assert_eq!(stored_of(&f.s), [5.0, 0.0, 12.0, 3.0]);
    // Two rectangles: each limited by its own shorter side (30 and 20), one commit.
    let (mut s, _, _) = two_rects([5.0, 0.0, 12.0, 3.0], [1.0; 4]);
    s.set_link_corners(false);
    let n = changes(&s);
    assert_eq!(s.set_selected_radius_text("25"), EntryOutcome::Committed);
    assert_eq!(changes(&s), n + 1);
    let all = stored_all(&s);
    assert_eq!(all.len(), 2);
    let mut got: Vec<f64> = all.iter().map(|r| r[0]).collect();
    got.sort_by(f64::total_cmp);
    assert_eq!(got, [20.0, 25.0]);
    assert!(all.iter().all(|r| r.iter().all(|v| *v == r[0])));
}

#[test]
fn remove_rounding_zeroes_every_selected_rectangle_in_one_commit_whatever_the_switch() {
    for linked in [true, false] {
        let (mut s, ..) = two_rects([5.0, 0.0, 12.0, 3.0], [4.0; 4]);
        s.set_link_corners(linked);
        let (n, v) = (changes(&s), vv(&s));
        assert!(s.select_bar_state().remove_rounding_enabled);
        s.remove_corner_rounding();
        assert_eq!(changes(&s), n + 1);
        assert!(stored_all(&s).iter().all(|r| *r == [0.0; 4]));
        let w = corner_key_writes(&s, &v);
        assert!(
            w.iter().all(|c| *c <= 2),
            "each register at most once per rectangle: {w:?}"
        );
        assert!(
            !s.select_bar_state().remove_rounding_enabled,
            "nothing left to remove"
        );
        // A second press changes nothing.
        let n = changes(&s);
        s.remove_corner_rounding();
        assert_eq!(changes(&s), n);
    }
    // One corner only rounded.
    let mut f = fx([0.0, 0.0, 9.0, 0.0]);
    assert!(f.s.select_bar_state().remove_rounding_enabled);
    f.s.remove_corner_rounding();
    assert_eq!(stored_of(&f.s), [0.0; 4]);
}

// -------------------------- resize and the Scale switch ----------------------

fn resize_drag(f: &mut Fx, to: Point, shift: bool, ctrl: bool) {
    // The BR corner resize handle sits on the corner.
    let from = pt(ORIGIN.0 + f.w, ORIGIN.1 + f.h);
    f.s.pointer_hover(from, shift, ctrl);
    f.s.pointer_down(from, shift);
    f.s.pointer_hover(to, shift, ctrl);
    f.s.pointer_up(to, shift, ctrl);
}

#[test]
fn a_resize_keeps_every_radius_by_default_and_scales_all_four_by_the_square_root_when_on() {
    let start = [5.0, 0.0, 12.0, 3.0];
    for (target, shift, ctrl) in [
        ((210.0, 140.0), false, false),
        ((210.0, 80.0), false, false),
        ((60.0, 40.0), false, false),
        ((210.0, 140.0), true, false),
        ((210.0, 140.0), false, true),
    ] {
        // Keep (the default): the stored radii are exactly as before.
        let mut f = fx(start);
        assert!(!f.s.scale_corner_radius(), "off at program start");
        resize_drag(&mut f, pt(target.0, target.1), shift, ctrl);
        assert_eq!(stored_of(&f.s), start, "Keep {target:?}");
        let (_, _, w, h) = bounds_of(&f.s);
        assert!(w != 100.0 || h != 60.0, "the drag resized");
        // Proportional.
        let mut f = fx(start);
        f.s.set_scale_corner_radius(true);
        resize_drag(&mut f, pt(target.0, target.1), shift, ctrl);
        let (_, _, w, h) = bounds_of(&f.s);
        let factor = ((w / 100.0) * (h / 60.0)).sqrt();
        let got = stored_of(&f.s);
        for k in 0..4 {
            assert!(
                close(got[k], start[k] * factor, 1e-9),
                "On {target:?} shift {shift} ctrl {ctrl} k {k}: {got:?} factor {factor}"
            );
        }
        assert_eq!(got[1], 0.0, "a sharp corner stays sharp");
    }
}

#[test]
fn the_two_scale_switches_are_independent_and_the_corner_switch_is_read_at_the_press() {
    // Corner switch off, stroke switch on: radii still untouched.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    f.s.set_scale_stroke_width(true);
    resize_drag(&mut f, pt(210.0, 140.0), false, false);
    assert_eq!(stored_of(&f.s), [5.0, 0.0, 12.0, 3.0]);
    // Corner switch on, stroke switch off: radii scale (factor 2 here).
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    f.s.set_scale_corner_radius(true);
    f.s.set_scale_stroke_width(false);
    resize_drag(&mut f, pt(210.0, 140.0), false, false);
    assert_eq!(stored_of(&f.s), [10.0, 0.0, 24.0, 6.0]);
    // Read once at the press: a change mid-drag applies from the next drag.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    f.s.set_scale_corner_radius(true);
    let from = pt(ORIGIN.0 + 100.0, ORIGIN.1 + 60.0);
    f.s.pointer_hover(from, false, false);
    f.s.pointer_down(from, false);
    f.s.set_scale_corner_radius(false);
    f.s.pointer_hover(pt(210.0, 140.0), false, false);
    f.s.pointer_up(pt(210.0, 140.0), false, false);
    assert_eq!(
        stored_of(&f.s),
        [10.0, 0.0, 24.0, 6.0],
        "the press saw the switch on"
    );
}

#[test]
fn shrinking_then_enlarging_with_keep_gives_the_stored_radii_back() {
    let start = [5.0, 0.0, 12.0, 3.0];
    let mut f = fx(start);
    resize_drag(&mut f, pt(30.0, 28.0), false, false); // 20 x 8 mm: radii exceed the size
    assert_eq!(stored_of(&f.s), start);
    f.w = 20.0;
    f.h = 8.0;
    let (_, _, w, h) = bounds_of(&f.s);
    assert!(w < 25.0 && h < 10.0);
    resize_drag(&mut f, pt(110.0, 80.0), false, false);
    assert_eq!(stored_of(&f.s), start);
}

#[test]
fn a_typed_size_follows_the_switch_as_read_when_the_entry_opens() {
    for on in [false, true] {
        let mut f = fx([5.0, 0.0, 12.0, 3.0]);
        f.s.set_scale_corner_radius(on);
        let from = pt(ORIGIN.0 + 100.0, ORIGIN.1 + 60.0);
        f.s.pointer_hover(from, false, false);
        f.s.pointer_down(from, false);
        f.s.pointer_up(from, false, false);
        f.s.double_click(from, false, false);
        let e = f.s.transform_entry().expect("the size entry");
        assert_eq!(e.kind, "size");
        let out = f.s.commit_transform_entry("200", "120", 0);
        assert_eq!(out, EntryOutcome::Committed);
        let want = if on {
            [10.0, 0.0, 24.0, 6.0]
        } else {
            [5.0, 0.0, 12.0, 3.0]
        };
        assert_eq!(stored_of(&f.s), want, "switch {on}");
    }
    // A click on the switch while the size entry is open closes it without writing.
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    let from = pt(ORIGIN.0 + 100.0, ORIGIN.1 + 60.0);
    f.s.pointer_hover(from, false, false);
    f.s.pointer_down(from, false);
    f.s.pointer_up(from, false, false);
    f.s.double_click(from, false, false);
    assert!(f.s.transform_entry().is_some());
    f.s.set_scale_corner_radius(true);
    assert!(f.s.transform_entry().is_none());
    assert_eq!(bounds_of(&f.s).2, 100.0);
}

#[test]
fn a_move_leaves_the_radii_alone() {
    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
    f.s.set_scale_corner_radius(true);
    let a = pt(ORIGIN.0 + 50.0, ORIGIN.1 + 30.0);
    f.s.pointer_hover(a, false, false);
    f.s.pointer_down(a, false);
    f.s.pointer_hover(pt(a.x + 40.0, a.y + 25.0), false, false);
    f.s.pointer_up(pt(a.x + 40.0, a.y + 25.0), false, false);
    assert_eq!(stored_of(&f.s), [5.0, 0.0, 12.0, 3.0]);
    assert!(bounds_of(&f.s).0 > ORIGIN.0 + 30.0, "it moved");
}

// ---------------------------------- peers -----------------------------------

fn merged(a: &Session, b: &Session) -> Document {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(a).export_loro_snapshot().unwrap())
        .unwrap();
    l.import(&doc_of(b).export_loro_snapshot().unwrap())
        .unwrap();
    l.commit();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester-merge",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("manifest.json", o).unwrap();
    w.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    w.start_file("document.loro", o).unwrap();
    w.write_all(&l.export(loro::ExportMode::Snapshot).unwrap())
        .unwrap();
    w.start_file("document.json", o).unwrap();
    w.write_all(b"{}").unwrap();
    unpack(9, &w.finish().unwrap().into_inner()).unwrap()
}

fn radii_of_doc(d: &Document) -> [f64; 4] {
    let ObjectSnapshot::Primitive(p) = d.object(d.object_ids()[0]).unwrap() else {
        panic!()
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!()
    };
    arr(corner_radii)
}

#[test]
fn two_peers_unlinking_two_different_corners_both_survive_in_either_merge_order() {
    let start = [5.0, 10.0, 15.0, 20.0];
    let mut a = fx_peer(2, 100.0, 60.0, start);
    let mut b = fx_peer(3, 100.0, 60.0, start);
    a.s.set_link_corners(false);
    b.s.set_link_corners(false);
    a.drag(0, false, false, 4.0); // TL grows
    b.drag(2, false, false, -3.0); // BR shrinks
    let (ta, tb) = (stored_of(&a.s), stored_of(&b.s));
    assert!(ta[0] > 5.0 && tb[2] < 15.0);
    for swap in [false, true] {
        let m = if swap {
            merged(&b.s, &a.s)
        } else {
            merged(&a.s, &b.s)
        };
        let r = radii_of_doc(&m);
        assert_eq!(r[0], ta[0], "swap {swap}");
        assert_eq!(r[2], tb[2], "swap {swap}");
        assert_eq!((r[1], r[3]), (10.0, 20.0));
    }
}

#[test]
fn a_linked_edit_and_an_unlinked_edit_on_the_same_file_converge() {
    let start = [5.0, 10.0, 15.0, 20.0];
    let mut a = fx_peer(2, 100.0, 60.0, start);
    let mut b = fx_peer(3, 100.0, 60.0, start);
    b.s.set_link_corners(false);
    a.drag(0, false, false, 4.0); // all four
    b.drag(1, false, false, -4.0); // TR only
    let ab = radii_of_doc(&merged(&a.s, &b.s));
    let ba = radii_of_doc(&merged(&b.s, &a.s));
    assert_eq!(ab, ba);
    for v in ab {
        assert!(v.is_finite() && v >= 0.0);
    }
}

// ------------------------------- old files ----------------------------------

#[test]
fn a_version_5_file_opens_as_four_equal_radii_and_edits_per_corner_without_a_rewrite_on_open() {
    let bytes =
        std::fs::read("../curvyo-document-core/tests/fixtures/legacy_corner_radius_v5.curvyo")
            .unwrap();
    let d = unpack(1, &bytes).unwrap();
    let ids = d.object_ids();
    // Find a rectangle with a positive legacy radius.
    let mut pick = None;
    for id in ids {
        let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
            continue;
        };
        let Shape::Rect {
            bounds,
            corner_radii,
        } = p.shape
        else {
            continue;
        };
        let r = arr(corner_radii);
        assert!(
            r.iter().all(|v| *v == r[0]),
            "four equal radii from the legacy value"
        );
        if r[0] > 0.0 {
            pick = Some((id, bounds, r[0]));
        }
    }
    let (id, bounds, r0) = pick.expect("a rounded rectangle in the fixture");
    let mut s = Session::open(2, &bytes).unwrap();
    // Open writes nothing.
    assert_eq!(
        corner_key_writes(&s, &vv(&Session::open(3, &bytes).unwrap())),
        [0; 4]
    );
    s.set_tool(Tool::Select);
    let (w, h) = (bounds.width.as_mm(), bounds.height.as_mm());
    click(
        &mut s,
        pt(bounds.origin.x, bounds.origin.y + h / 2.0),
        false,
    );
    assert_eq!(s.selected_object_count(), 1);
    let scale = s.view().scale();
    if w.min(h) * scale >= 72.0 {
        let eff = css(w, h, [r0; 4]);
        let (lx, ly) = knob_local(w, h, scale, eff, 2);
        let at = pt(bounds.origin.x + lx, bounds.origin.y + ly);
        s.set_link_corners(false);
        let v = vv(&s);
        s.pointer_hover(at, false, false);
        s.pointer_down(at, false);
        let to = pt(at.x - 0.7, at.y - 0.7);
        s.pointer_hover(to, false, false);
        s.pointer_up(to, false, false);
        let w_ = corner_key_writes(&s, &v);
        assert_eq!(
            w_,
            [0, 0, 1, 0],
            "one register written, the others fall back to the legacy value"
        );
        let d2 = doc_of(&s);
        let ObjectSnapshot::Primitive(p) = d2.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect { corner_radii, .. } = p.shape else {
            panic!()
        };
        let r = arr(corner_radii);
        assert_eq!((r[0], r[1], r[3]), (r0, r0, r0));
        assert_ne!(r[2], r0);
    }
}

// --------------------------------- hostile ----------------------------------

#[test]
fn hostile_pointer_values_during_a_radius_drag_and_hover_never_write_bad_radii() {
    let bad_values = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
        -1e300,
        f64::MAX,
    ];
    for linked in [true, false] {
        for bad in bad_values {
            for i in 0..4 {
                for shape in 0..3 {
                    let p = [pt(bad, 0.0), pt(0.0, bad), pt(bad, bad)][shape];
                    let mut f = fx([5.0, 0.0, 12.0, 3.0]);
                    f.s.set_link_corners(linked);
                    let _ = f.press(i, false);
                    f.s.pointer_hover(p, false, false);
                    let _ = f.s.live_readout();
                    let _ = f.s.draw_list();
                    f.s.pointer_up(p, false, false);
                    let a = stored_of(&f.s);
                    for v in a {
                        assert!(
                            v.is_finite() && (0.0..=100.0).contains(&v),
                            "{bad} -> {a:?}"
                        );
                    }
                    // The file saved still opens.
                    Session::open(5, &f.s.pack("0.1.0").unwrap()).unwrap();
                }
            }
        }
    }
    // Hover and double-click with hostile points on a selected rectangle: no panic.
    let mut f = fx([5.0; 4]);
    for bad in bad_values {
        f.s.pointer_hover(pt(bad, bad), false, false);
        let _ = f.s.corner_hint_lines();
        f.s.double_click(pt(bad, bad), true, true);
        f.s.cancel_transform_entry();
    }
}

#[test]
fn a_rectangle_with_absurd_radii_still_lets_the_maker_edit_it() {
    let mut f = fx([1e9, 1e9, 1e9, 1e9]);
    let bar = f.s.select_bar_state();
    assert!(
        matches!(bar.radius, Some(BarValue::Uniform(v)) if close(v.as_mm(), 30.0, 1e-6)),
        "{:?}",
        bar.radius
    );
    f.s.set_link_corners(false);
    let at = f.knob(0);
    f.s.pointer_hover(at, false, false);
    let _ = f.s.corner_hint_lines();
    assert_eq!(f.s.set_selected_radius_text("4"), EntryOutcome::Committed);
    assert_eq!(stored_of(&f.s), [4.0; 4]);
}

// ------------------------------ rectangle only ------------------------------

#[test]
fn ellipse_polygon_and_star_have_no_corner_controls_and_ignore_the_switch() {
    let d = Document::new(1);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(50.0, 50.0),
        rx: mm(40.0),
        ry: mm(30.0),
    });
    let _ = d.create_polygon(
        StarFrame {
            center: pt(250.0, 50.0),
            radius: mm(40.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        PointCount::new(6).unwrap(),
    );
    let bytes = pack(&d, "0.1.0").unwrap();
    for (x, y) in [(10.0, 50.0), (210.0, 50.0)] {
        let mut s = Session::open(2, &bytes).unwrap();
        s.set_tool(Tool::Select);
        click(&mut s, pt(x, y), false);
        let sel = s.selected_object_count();
        let bar = s.select_bar_state();
        assert_eq!(bar.radius, None);
        assert!(!bar.remove_rounding_shown && !bar.remove_rounding_enabled);
        let (n, before) = (changes(&s), s.pack("0.1.0").unwrap().len());
        s.set_link_corners(false);
        s.remove_corner_rounding();
        let _ = s.set_selected_radius_text("5");
        assert_eq!(
            changes(&s),
            n,
            "nothing written for a non-rectangle ({sel} selected)"
        );
        let _ = before;
        assert!(s.corner_hint_lines().is_empty());
    }
}

#[test]
fn a_rotated_rectangle_names_and_edits_its_corners_in_its_own_frame() {
    use curvyo_document_core::Angle;
    let (w, h) = (100.0, 60.0);
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(ORIGIN.0, ORIGIN.1),
        width: mm(w),
        height: mm(h),
    });
    d.set_corner_radii(&[(id, radii([5.0, 10.0, 15.0, 20.0]))])
        .unwrap();
    let centre = pt(ORIGIN.0 + w / 2.0, ORIGIN.1 + h / 2.0);
    let rotated = d
        .object(id)
        .unwrap()
        .rotated(centre, Angle::from_radians(std::f64::consts::FRAC_PI_2));
    d.rotate_object(&rotated).unwrap();
    let bytes = pack(&d, "0.1.0").unwrap();
    let names = [
        "Top-left corner radius",
        "Top-right corner radius",
        "Bottom-right corner radius",
        "Bottom-left corner radius",
    ];
    for i in 0..4 {
        let mut s = Session::open(2, &bytes).unwrap();
        s.set_tool(Tool::Select);
        click(&mut s, pt(centre.x, centre.y + 49.0), false); // inside the rotated 60 x 100 box
        assert_eq!(s.selected_object_count(), 1);
        s.set_link_corners(false);
        let scale = s.view().scale();
        let (lx, ly) = knob_local(w, h, scale, [5.0, 10.0, 15.0, 20.0], i);
        let (dx, dy) = (ORIGIN.0 + lx - centre.x, ORIGIN.1 + ly - centre.y);
        let at = pt(centre.x - dy, centre.y + dx); // 90 degrees clockwise on screen
        s.pointer_hover(at, false, false);
        s.pointer_down(at, false);
        s.pointer_up(at, false, false);
        s.double_click(at, false, false);
        let e = s.transform_entry().expect("an entry at the rotated knob");
        assert_eq!(e.fields[0].accessible_name, names[i], "corner {i}");
        assert_eq!(e.scope, Some("This corner only"));
        assert_eq!(
            s.commit_transform_entry("2", "", 0),
            EntryOutcome::Committed
        );
        let mut want = [5.0, 10.0, 15.0, 20.0];
        want[i] = 2.0;
        assert_eq!(stored_of(&s), want, "own-frame corner {i}");
    }
}

#[test]
fn a_stars_inner_radius_drag_ignores_the_link_switch_and_shift() {
    use curvyo_document_core::{Angle, InnerRatio};
    use curvyo_ui_core::{
        EditHandle, ObjectSelection, ParamHandle, SelectTool, TransformHandleTolerances,
    };
    let d = Document::new(1);
    let id = d.create_star(
        StarFrame {
            center: pt(100.0, 100.0),
            radius: mm(60.0),
            angle: Angle::from_radians(-std::f64::consts::FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let bytes = pack(&d, "0.1.0").unwrap();
    let mut ratios = Vec::new();
    for (linked, shift) in [(true, false), (true, true), (false, false), (false, true)] {
        let mut s = Session::open(2, &bytes).unwrap();
        s.set_tool(Tool::Select);
        click(&mut s, pt(100.0, 40.0), false); // the first outer vertex
        assert_eq!(s.selected_object_count(), 1);
        s.set_link_corners(linked);
        let objects = vec![doc_of(&s).object(id).unwrap()];
        let mut sel = ObjectSelection::new();
        sel.select_single(id);
        let tol = TransformHandleTolerances::at_scale(s.view().scale());
        let handle = SelectTool::transform_handles(&objects, &sel, tol, false)
            .into_iter()
            .find(|(h, _)| *h == EditHandle::Param(ParamHandle::InnerRadius))
            .expect("the inner-radius handle")
            .1;
        let to = pt(handle.x, handle.y + 12.0);
        s.pointer_hover(handle, shift, false);
        s.pointer_down(handle, shift);
        s.pointer_hover(to, shift, false);
        s.pointer_up(to, shift, false);
        let doc = doc_of(&s);
        let ObjectSnapshot::Primitive(p) = doc.object(id).unwrap() else {
            panic!()
        };
        ratios.push(format!("{:?}", p.shape));
    }
    assert!(ratios.windows(2).all(|w| w[0] == w[1]), "{ratios:?}");
    assert!(
        !ratios[0].contains("0.5"),
        "the ratio changed: {}",
        ratios[0]
    );
}
