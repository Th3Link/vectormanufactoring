//! Session-level tests of PR 2 of `specs/rectangle-corner-radii/`: the "Link
//! corners" switch and its lifetime, per-corner drags with Shift, the live
//! readout texts, the typed entry's scope row, accessible names and "max"
//! notice, the knob hint lines and the bar's "Mixed" data. The pure rules are
//! tested in `curvyo-ui-core`; the DOM texts in `frontend/`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Corner, CornerRadii, Document, Length, NodeId, ObjectSnapshot, Point, RectBounds, Shape, pack,
    unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::{BarValue, EntryOutcome};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: Length::from_mm(tl),
        tr: Length::from_mm(tr),
        br: Length::from_mm(br),
        bl: Length::from_mm(bl),
    }
}

/// A 100 x 60 mm rectangle at (10, 20).
const RECT: (f64, f64, f64, f64) = (10.0, 20.0, 100.0, 60.0);

fn session_with(stored: CornerRadii) -> (Session, NodeId) {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(RECT.0, RECT.1),
        width: Length::from_mm(RECT.2),
        height: Length::from_mm(RECT.3),
    });
    document.set_corner_radii(&[(id, stored)]).unwrap();
    let bytes = pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    click(&mut session, pt(RECT.0, RECT.1 + 30.0));
    (session, id)
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

/// The knob of `corner` for the radius it shows (small radii only, so that the
/// diagonal cap does not apply): on its diagonal at `15 + ρ·L(s)` pixels.
fn knob(session: &Session, corner: Corner, radius_mm: f64) -> Point {
    let scale = session.view().scale();
    let (x, y, w, h) = RECT;
    let shorter_px = w.min(h) * scale;
    let rho = radius_mm / (w.min(h) / 2.0);
    let travel = (shorter_px - 14.0) / std::f64::consts::SQRT_2 - 15.0;
    let along = (15.0 + rho * travel) / scale / std::f64::consts::SQRT_2;
    match corner {
        Corner::Tl => pt(x + along, y + along),
        Corner::Tr => pt(x + w - along, y + along),
        Corner::Br => pt(x + w - along, y + h - along),
        Corner::Bl => pt(x + along, y + h - along),
    }
}

/// `at` moved by `delta` millimetres along the inward diagonal of `corner`.
fn along(at: Point, corner: Corner, delta_mm: f64) -> Point {
    let d = corner.inward_diagonal();
    pt(at.x + d.x * delta_mm, at.y + d.y * delta_mm)
}

fn stored(session: &Session) -> CornerRadii {
    let document = unpack(99, &session.pack("0.1.0").unwrap()).unwrap();
    let ObjectSnapshot::Primitive(p) = document.object(document.object_ids()[0]).unwrap() else {
        panic!("a primitive");
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!("a rectangle");
    };
    corner_radii
}

fn drag_with(session: &mut Session, from: Point, to: Point, shift: bool) {
    session.pointer_hover(from, shift, false);
    session.pointer_down(from, shift);
    session.pointer_hover(to, shift, false);
    session.pointer_up(to, shift, false);
}

fn readout(session: &Session) -> Option<String> {
    session.live_readout().map(|r| r.text)
}

/// What the chip shows of an open entry.
struct OpenedEntry {
    kind: &'static str,
    scope: Option<&'static str>,
    name: &'static str,
    prefill: String,
}

fn open_entry(session: &mut Session, at: Point, shift: bool) -> OpenedEntry {
    session.pointer_hover(at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
    session.double_click(at, shift, false);
    let entry = session.transform_entry().expect("the entry is open");
    OpenedEntry {
        kind: entry.kind,
        scope: entry.scope,
        name: entry.fields[0].accessible_name,
        prefill: entry.fields[0].prefill.clone(),
    }
}

/// Criterion 2: on by default and in every new session or opened file; the
/// state is never written and a toggle changes no radius.
#[test]
fn the_switch_starts_on_everywhere_and_toggling_writes_nothing() {
    assert!(Session::new(1).link_corners());
    let (mut session, _) = session_with(radii(1.0, 2.0, 3.0, 4.0));
    assert!(session.link_corners());
    let before = session.pack("0.1.0").unwrap();
    session.set_link_corners(false);
    assert!(!session.link_corners());
    session.set_link_corners(true);
    assert_eq!(stored(&session), radii(1.0, 2.0, 3.0, 4.0));
    assert_eq!(unpack(5, &before).unwrap().object_ids().len(), 1);
    // A reopened file starts on again.
    let mut off = Session::open(3, &session.pack("0.1.0").unwrap()).unwrap();
    assert!(off.link_corners());
    off.set_link_corners(false);
    let fresh = Session::open(4, &off.pack("0.1.0").unwrap()).unwrap();
    assert!(fresh.link_corners(), "Open resets the switch");
}

/// Criteria 2 and 3 through the session: switch on, no Shift: all four; Shift:
/// this corner; the other way round with the switch off.
#[test]
fn a_knob_drag_changes_all_four_or_its_own_corner() {
    for (linked, shift, one_corner) in [
        (true, false, false),
        (true, true, true),
        (false, false, true),
        (false, true, false),
    ] {
        let (mut session, _) = session_with(radii(5.0, 0.0, 12.0, 3.0));
        session.set_link_corners(linked);
        let from = knob(&session, Corner::Tl, 5.0);
        drag_with(&mut session, from, along(from, Corner::Tl, 8.0), shift);
        let after = stored(&session);
        assert!(after.tl.as_mm() > 5.0, "linked {linked} shift {shift}");
        if one_corner {
            assert_eq!(
                (after.tr, after.br, after.bl),
                (
                    Length::from_mm(0.0),
                    Length::from_mm(12.0),
                    Length::from_mm(3.0)
                )
            );
        } else {
            assert_eq!(after, CornerRadii::uniform(after.tl));
        }
    }
}

/// Criterion 7: "r 3.5 mm", the suffix " max" at a limit and " · all corners"
/// for a linked drag that overwrites unequal radii.
#[test]
fn the_readout_names_the_radius_the_limit_and_the_overwrite() {
    // Equal radii, linked: plain.
    let (mut session, _) = session_with(CornerRadii::uniform(Length::from_mm(5.0)));
    let from = knob(&session, Corner::Tl, 5.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(along(from, Corner::Tl, 3.0), false, false);
    let text = readout(&session).expect("a readout");
    assert!(text.starts_with("r ") && text.ends_with(" mm"), "{text}");
    session.pointer_hover(along(from, Corner::Tl, 500.0), false, false);
    assert_eq!(readout(&session).unwrap(), "r 30.0 mm max");
    session.escape();

    // Unequal radii, linked: the press finds them unequal.
    let (mut session, _) = session_with(radii(5.0, 0.0, 12.0, 3.0));
    let from = knob(&session, Corner::Tl, 5.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(along(from, Corner::Tl, 500.0), false, false);
    assert_eq!(
        readout(&session).unwrap(),
        "r 30.0 mm max \u{b7} all corners"
    );
    session.pointer_hover(along(from, Corner::Tl, 3.0), false, false);
    let text = readout(&session).unwrap();
    assert!(
        text.ends_with(" \u{b7} all corners") && !text.contains("max"),
        "{text}"
    );
    session.escape();
    assert_eq!(
        stored(&session),
        radii(5.0, 0.0, 12.0, 3.0),
        "Escape writes nothing"
    );

    // Unlinked: the limit is the neighbours' (min(100 - 40, 60 - 30) = 30).
    let (mut session, _) = session_with(radii(0.0, 40.0, 0.0, 30.0));
    session.set_link_corners(false);
    let from = knob(&session, Corner::Tl, 0.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(along(from, Corner::Tl, 500.0), false, false);
    assert_eq!(readout(&session).unwrap(), "r 30.0 mm max");
}

/// Criterion 6: the scope row, the accessible names, the corner's own value and
/// the "max" notice for a limited value.
#[test]
fn the_typed_entry_has_a_scope_row_names_and_a_max_notice() {
    let (mut session, _) = session_with(radii(0.0, 40.0, 0.0, 30.0));
    session.set_link_corners(false);
    let at = knob(&session, Corner::Tl, 0.0);
    let entry = open_entry(&mut session, at, false);
    assert_eq!(entry.kind, "corner-radius");
    assert_eq!(entry.scope, Some("This corner only"));
    assert_eq!(entry.name, "Top-left corner radius");
    assert_eq!(entry.prefill, "0.0");
    let outcome = session.commit_transform_entry("55", "", 0);
    assert_eq!(outcome, EntryOutcome::Committed);
    assert_eq!(stored(&session), radii(30.0, 40.0, 0.0, 30.0));
    let notice = session.live_readout().expect("the limit is not silent");
    assert_eq!(notice.text, "r 30.0 mm max");
    session.clear_limit_notice();
    assert!(session.live_readout().is_none());

    // Linked: all four, named so; a press clears a notice too.
    let (mut session, _) = session_with(radii(0.0, 0.0, 0.0, 0.0));
    let at = knob(&session, Corner::Br, 0.0);
    let entry = open_entry(&mut session, at, false);
    assert_eq!(entry.scope, Some("All four corners"));
    assert_eq!(entry.name, "Corner radius, all corners");
    assert_eq!(
        session.commit_transform_entry("99", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(
        stored(&session),
        CornerRadii::uniform(Length::from_mm(30.0))
    );
    assert_eq!(session.live_readout().unwrap().text, "r 30.0 mm max");
    session.pointer_down(pt(500.0, 500.0), false);
    session.pointer_up(pt(500.0, 500.0), false, false);
    assert!(session.live_readout().is_none());
}

/// Criterion 8: a click on the switch closes an open entry without writing.
#[test]
fn clicking_the_switch_closes_the_entry_and_writes_nothing() {
    let (mut session, _) = session_with(radii(1.0, 2.0, 3.0, 4.0));
    let at = knob(&session, Corner::Tl, 1.0);
    let _ = open_entry(&mut session, at, false);
    session.set_link_corners(false);
    assert!(session.transform_entry().is_none());
    assert_eq!(stored(&session), radii(1.0, 2.0, 3.0, 4.0));
}

/// Criterion 23: the hint lines depend on the switch (not on Shift) and a
/// limited corner adds its notes.
#[test]
fn the_knob_hint_lines_follow_the_switch_and_a_limited_corner() {
    let (mut session, _) = session_with(radii(5.0, 0.0, 12.0, 3.0));
    let at = knob(&session, Corner::Tl, 5.0);
    session.pointer_hover(at, false, false);
    assert_eq!(
        session.corner_hint_lines(),
        [
            "Corner radius, all four",
            "Shift: this corner only",
            "Double-click: type a value"
        ]
    );
    session.set_link_corners(false);
    assert_eq!(
        session.corner_hint_lines(),
        [
            "Corner radius, this corner",
            "Shift: all four corners",
            "Double-click: type a value"
        ]
    );
    // Off the knob: no lines.
    session.pointer_hover(pt(60.0, 50.0), false, false);
    assert!(session.corner_hint_lines().is_empty());

    // A shrunk rectangle: TL 30 + BL 30 + TR 30 on 100 x 40 -> shown 20.
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(10.0, 20.0),
        width: Length::from_mm(100.0),
        height: Length::from_mm(60.0),
    });
    document
        .set_corner_radii(&[(id, radii(50.0, 50.0, 0.0, 50.0))])
        .unwrap();
    let bytes = pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    click(&mut session, pt(10.0, 50.0));
    let at = knob(&session, Corner::Tl, 30.0);
    session.pointer_hover(at, false, false);
    let lines = session.corner_hint_lines();
    assert_eq!(lines[0], "Limited by the size. Stored 50 mm, shown 30 mm.");
    assert_eq!(
        lines.len(),
        4,
        "linked: no 'editing one corner' note: {lines:?}"
    );
    session.set_link_corners(false);
    let lines = session.corner_hint_lines();
    assert_eq!(
        lines[1],
        "Editing one corner fixes the other three at their shown size."
    );
    assert_eq!(lines.len(), 5);
}

/// Criterion 22: one rectangle with unequal corners reads "Mixed" and carries
/// the four effective radii for its tooltip.
#[test]
fn the_bar_reads_mixed_with_the_four_values() {
    let (session, _) = session_with(radii(12.0, 0.0, 12.0, 0.0));
    let bar = session.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Mixed));
    let corners = bar.radius_corners.expect("the four values");
    assert_eq!(
        [corners.tl, corners.tr, corners.br, corners.bl].map(Length::as_mm),
        [12.0, 0.0, 12.0, 0.0]
    );
    assert!(bar.remove_rounding_enabled);
}

/// Criterion 8: "Remove rounding" and the bar field write all four, whatever
/// the switch says.
#[test]
fn remove_rounding_and_the_bar_field_ignore_the_switch() {
    let (mut session, _) = session_with(radii(5.0, 0.0, 12.0, 3.0));
    session.set_link_corners(false);
    session.remove_corner_rounding();
    assert_eq!(stored(&session), radii(0.0, 0.0, 0.0, 0.0));
    assert_eq!(
        session.set_selected_radius_text("7"),
        EntryOutcome::Committed
    );
    assert_eq!(stored(&session), CornerRadii::uniform(Length::from_mm(7.0)));
}
