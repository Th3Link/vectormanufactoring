//! `Session`-level tests of `specs/shape-creation-from-center/specification.md`
//! for the criteria that need the session: the readout text (11), a modifier
//! change with the pointer at rest (8), the release event's modifiers (10),
//! Escape (14), panning mid-drag (15), what is stored (13), and that a Shift
//! press is still a create-drag (16, 17). Driven through `Session`'s public
//! API, the way the frontend calls it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{CURRENT_FORMAT_VERSION, Point, Shape, unpack};
use curvyo_editor_wasm::{EscapeStep, Session, Tool};
use curvyo_ui_core::PolyStarMode;

const TOL: f64 = 1e-9;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// What the frontend does on a key event: cache the state, then re-send the
/// hover at the last pointer position.
fn key_change(session: &mut Session, at: Point, shift: bool, ctrl: bool) {
    session.modifiers_changed(shift, ctrl);
    session.pointer_hover(at, shift, ctrl);
}

fn readout(session: &Session) -> Option<(String, Point)> {
    session.live_readout().map(|r| (r.text, r.anchor))
}

fn started(tool: Tool) -> Session {
    let mut session = Session::new(1);
    session.set_tool(tool);
    session.pointer_down(pt(100.0, 50.0), false);
    session
}

fn shapes(session: &Session) -> Vec<Shape> {
    let bytes = session.pack("0.1.0").expect("pack");
    let document = unpack(99, &bytes).expect("unpack");
    document
        .object_ids()
        .into_iter()
        .map(|id| document.primitive(id).expect("a primitive").shape)
        .collect()
}

fn rect_of(shape: Shape) -> (f64, f64, f64, f64) {
    let Shape::Rect { bounds, .. } = shape else {
        panic!("a rectangle");
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

fn assert_rect(shape: Shape, want: (f64, f64, f64, f64)) {
    let got = rect_of(shape);
    assert!(
        (got.0 - want.0).abs() < TOL
            && (got.1 - want.1).abs() < TOL
            && (got.2 - want.2).abs() < TOL
            && (got.3 - want.3).abs() < TOL,
        "{got:?} != {want:?}"
    );
}

/// AC 11: the readout reports the final shape in each mode, anchored at the
/// effective endpoint.
#[test]
fn ac11_readouts_report_the_final_shape_in_every_mode() {
    let b = pt(130.0, 40.0);
    let cases = [
        (Tool::Rectangle, false, false, "30.0 × 10.0 mm", b),
        (
            Tool::Rectangle,
            false,
            true,
            "30.0 × 30.0 mm",
            pt(130.0, 20.0),
        ),
        (Tool::Rectangle, true, false, "60.0 × 20.0 mm", b),
        (
            Tool::Rectangle,
            true,
            true,
            "60.0 × 60.0 mm",
            pt(130.0, 20.0),
        ),
        (Tool::Ellipse, true, false, "30.0 × 10.0 mm", b),
        (Tool::Ellipse, true, true, "30.0 × 30.0 mm", pt(130.0, 20.0)),
        (Tool::Ellipse, false, false, "15.0 × 5.0 mm", b),
    ];
    for (tool, shift, ctrl, text, anchor) in cases {
        let mut session = started(tool);
        session.pointer_hover(b, shift, ctrl);
        assert_eq!(
            readout(&session),
            Some((text.to_string(), anchor)),
            "{tool:?} shift {shift} ctrl {ctrl}"
        );
    }
}

/// AC 8: with the pointer held still, pressing and releasing the keys in
/// any order changes the readout and the drawn preview, and the commit is
/// what the last preview showed.
#[test]
fn ac8_every_modifier_change_with_the_pointer_at_rest_updates_preview_and_readout() {
    let b = pt(130.0, 40.0);
    let sequence = [
        (false, false, "30.0 × 10.0 mm"),
        (true, false, "60.0 × 20.0 mm"),
        (true, true, "60.0 × 60.0 mm"),
        (false, true, "30.0 × 30.0 mm"),
        (true, true, "60.0 × 60.0 mm"),
        (true, false, "60.0 × 20.0 mm"),
        (false, false, "30.0 × 10.0 mm"),
    ];
    let mut session = started(Tool::Rectangle);
    let mut previous = None;
    for (shift, ctrl, text) in sequence {
        key_change(&mut session, b, shift, ctrl);
        assert_eq!(readout(&session).map(|r| r.0).as_deref(), Some(text));
        let drawn = session.draw_list();
        assert_ne!(Some(&drawn), previous.as_ref(), "the preview changed");
        previous = Some(drawn);
    }
    key_change(&mut session, b, true, true);
    session.pointer_up(b, true, true);
    assert_rect(shapes(&session)[0], (70.0, 20.0, 60.0, 60.0));
}

/// AC 10: the release event's state wins over the last preview's.
#[test]
fn ac10_the_release_modifiers_win() {
    let mut session = started(Tool::Rectangle);
    session.pointer_hover(pt(130.0, 40.0), true, false);
    session.pointer_up(pt(130.0, 40.0), false, false);
    assert_rect(shapes(&session)[0], (100.0, 40.0, 30.0, 10.0));

    let mut session = started(Tool::Ellipse);
    session.pointer_hover(pt(130.0, 40.0), false, false);
    session.pointer_up(pt(130.0, 40.0), true, true);
    let Shape::Ellipse { frame } = shapes(&session)[0] else {
        panic!("an ellipse");
    };
    assert!((frame.center.x - 100.0).abs() < TOL && (frame.center.y - 50.0).abs() < TOL);
    assert!((frame.rx.as_mm() - 30.0).abs() < TOL && (frame.ry.as_mm() - 30.0).abs() < TOL);
}

/// AC 12: at the press point nothing is drawn or created, in any state.
#[test]
fn ac12_the_press_point_shows_and_creates_nothing_whatever_the_modifiers() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let mut session = started(tool);
        let idle = session.draw_list();
        for (shift, ctrl) in [(false, false), (true, false), (false, true), (true, true)] {
            key_change(&mut session, pt(100.0, 50.0), shift, ctrl);
            assert_eq!(readout(&session), None);
            assert_eq!(session.draw_list(), idle);
        }
        session.pointer_up(pt(100.0, 50.0), true, true);
        assert_eq!(shapes(&session).len(), 0);
        assert_eq!(session.tool(), tool);
    }
}

/// AC 13: one object, the same stored fields as without modifiers, and the
/// saved file keeps its `format_version`.
#[test]
fn ac13_one_object_and_nothing_records_the_modifiers() {
    let mut centred = started(Tool::Rectangle);
    centred.pointer_hover(pt(130.0, 40.0), true, false);
    centred.pointer_up(pt(130.0, 40.0), true, false);
    let mut plain = started(Tool::Rectangle);
    plain.pointer_hover(pt(130.0, 40.0), false, false);
    plain.pointer_up(pt(130.0, 40.0), false, false);
    assert_eq!(shapes(&centred).len(), 1);

    let json = |session: &Session| {
        let bytes = session.pack("0.1.0").expect("pack");
        let doc = unpack(99, &bytes).expect("unpack");
        let value: serde_json::Value =
            serde_json::from_slice(&doc.export_json().expect("json")).expect("parse");
        value
    };
    let (a, b) = (json(&centred), json(&plain));
    assert_eq!(a["format_version"], CURRENT_FORMAT_VERSION);
    let keys = |v: &serde_json::Value| {
        v["objects"][0]
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&a), keys(&b));
    assert_eq!(
        a["objects"][0]["shape"]
            .as_object()
            .map(|o| o.keys().count()),
        b["objects"][0]["shape"]
            .as_object()
            .map(|o| o.keys().count())
    );
}

/// AC 14: Escape cancels in every modifier state and later key changes do not
/// bring the drag back.
#[test]
fn ac14_escape_cancels_and_modifier_changes_do_not_revive_it() {
    for (shift, ctrl) in [(true, false), (false, true), (true, true)] {
        let mut session = started(Tool::Rectangle);
        key_change(&mut session, pt(130.0, 40.0), shift, ctrl);
        assert!(readout(&session).is_some());
        assert_eq!(session.escape(), EscapeStep::CancelledDrag);
        let idle = Session::new(1).draw_list().triangle_count();
        for (s, c) in [(false, false), (true, false), (true, true), (false, true)] {
            key_change(&mut session, pt(130.0, 40.0), s, c);
            assert_eq!(readout(&session), None);
            assert_eq!(session.draw_list().triangle_count(), idle);
        }
        session.pointer_up(pt(130.0, 40.0), true, true);
        assert_eq!(shapes(&session).len(), 0);
    }
}

/// AC 15: panning or zooming mid-drag leaves A fixed in document space, still
/// the centre.
#[test]
fn ac15_pan_and_zoom_mid_drag_keep_the_press_point_the_centre() {
    let mut session = started(Tool::Rectangle);
    session.pointer_hover(pt(130.0, 40.0), true, false);
    session.begin_pan(10.0, 10.0);
    session.pan_to(210.0, 90.0);
    session.end_pan();
    session.wheel(0.0, -120.0, 50.0, 50.0, false, true);
    session.pointer_hover(pt(130.0, 40.0), true, false);
    assert_eq!(
        readout(&session).map(|r| r.0).as_deref(),
        Some("60.0 × 20.0 mm")
    );
    session.pointer_up(pt(130.0, 40.0), true, false);
    assert_rect(shapes(&session)[0], (70.0, 40.0, 60.0, 20.0));
}

/// AC 16 (the part that survives `unified-object-editing` 25): a Shift press
/// on an existing rectangle's outline in the Rectangle tool starts a
/// create-drag around the press point and changes nothing else.
#[test]
fn ac16_a_shift_press_on_an_existing_outline_creates_around_the_press_point() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Rectangle);
    session.pointer_down(pt(0.0, 0.0), false);
    session.pointer_up(pt(40.0, 30.0), false, false);
    let first = shapes(&session)[0];

    session.set_tool(Tool::Rectangle);
    session.pointer_down(pt(0.0, 15.0), true);
    session.pointer_hover(pt(10.0, 25.0), true, false);
    session.pointer_up(pt(10.0, 25.0), true, false);
    let all = shapes(&session);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0], first);
    assert_rect(all[1], (-10.0, 5.0, 20.0, 20.0));
}

/// AC 17: Shift has no effect on a polygon or star create-drag; Ctrl keeps
/// the angle snap of `edit-interaction-polish` criterion 4.
#[test]
fn ac17_shift_changes_nothing_for_polygon_and_star() {
    for star in [false, true] {
        let make = |shift: bool, ctrl: bool| {
            let mut session = started(Tool::PolygonStar);
            session.set_poly_star_mode(if star {
                PolyStarMode::Star
            } else {
                PolyStarMode::Polygon
            });
            session.pointer_hover(pt(130.0, 40.0), shift, ctrl);
            let text = readout(&session);
            session.pointer_up(pt(130.0, 40.0), shift, ctrl);
            (shapes(&session)[0], text)
        };
        assert_eq!(make(false, false), make(true, false));
        assert_eq!(make(false, true), make(true, true));
        assert_ne!(make(false, false), make(false, true), "Ctrl still snaps");
    }
}

/// UX review: while Shift is down in a rectangle or ellipse create-drag the
/// pivot marker is drawn at the press point (`docs/design-system.md`,
/// "Modifiers in a rectangle or ellipse create-drag"), and only then. The
/// reference is the same outline made by a plain corner-to-corner drag, which
/// has no marker.
#[test]
fn the_press_point_is_marked_while_shift_is_down_in_a_rectangle_or_ellipse_create_drag() {
    let b = pt(130.0, 40.0);
    let corner_drag = |tool: Tool, from: Point, to: Point| {
        let mut session = Session::new(1);
        session.set_tool(tool);
        session.pointer_down(from, false);
        session.pointer_hover(to, false, false);
        session.draw_list().triangle_count()
    };
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let mut session = started(tool);
        // (shift, ctrl, the equivalent plain corner drag, marker expected)
        let cases = [
            (false, false, (pt(100.0, 50.0), pt(130.0, 40.0)), false),
            (false, true, (pt(100.0, 50.0), pt(130.0, 20.0)), false),
            (true, false, (pt(70.0, 60.0), pt(130.0, 40.0)), true),
            (true, true, (pt(70.0, 80.0), pt(130.0, 20.0)), true),
            (false, false, (pt(100.0, 50.0), pt(130.0, 40.0)), false),
        ];
        for (shift, ctrl, (from, to), marker) in cases {
            key_change(&mut session, b, shift, ctrl);
            let reference = corner_drag(tool, from, to);
            let drawn = session.draw_list().triangle_count();
            assert_eq!(
                drawn > reference,
                marker,
                "{tool:?} shift {shift} ctrl {ctrl}: {drawn} vs {reference}"
            );
        }
        key_change(&mut session, b, true, false);
        assert_eq!(session.escape(), EscapeStep::CancelledDrag);
        assert_eq!(
            session.draw_list(),
            Session::new(1).draw_list(),
            "Escape: nothing drawn"
        );
    }
}

/// The marker goes with the release: the committed shape is drawn exactly as
/// the same shape made without Shift, and polygon/star never get the marker.
#[test]
fn the_marker_is_gone_after_release_and_never_drawn_for_polygon_and_star() {
    let b = pt(130.0, 40.0);
    let mut centred = started(Tool::Rectangle);
    key_change(&mut centred, b, true, false);
    centred.pointer_up(b, true, false);
    let mut plain = Session::new(1);
    plain.set_tool(Tool::Rectangle);
    plain.pointer_down(pt(70.0, 40.0), false);
    plain.pointer_up(pt(130.0, 60.0), false, false);
    // Same pointer position and keys afterwards, so only the marker could differ.
    key_change(&mut centred, b, false, false);
    key_change(&mut plain, b, false, false);
    assert_eq!(centred.draw_list(), plain.draw_list());

    let mut star = started(Tool::PolygonStar);
    key_change(&mut star, b, false, false);
    let without = star.draw_list();
    key_change(&mut star, b, true, false);
    assert_eq!(star.draw_list(), without);
}
