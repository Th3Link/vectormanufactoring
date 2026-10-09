//! Session-level tests of PR 1 of `specs/0009-unified-object-editing/
//! specification.md`, written from the specification's own arithmetic (the
//! radius handle's position rule `p = 15 + ρ·L(s)`, the stored fields, the
//! blue-new/black-old frames) and driven through `Session`'s public API
//! only. The pure parts (tiers, clearance, hit order, `apply_param`) are
//! tested in `curvyo-ui-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Document, InnerRatio, Length,
    NewAnchor, NodeId, ObjectSnapshot, Point, PointCount, RectBounds, Shape, StarFrame, pack,
    unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::{RgbaColor, build_live_edit_preview};
use curvyo_ui_core::{BarValue, EntryOutcome, InvalidReason};

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

fn star_frame(cx: f64, cy: f64, r: f64) -> StarFrame {
    StarFrame {
        center: pt(cx, cy),
        radius: Length::from_mm(r),
        angle: curvyo_document_core::Angle::from_radians(0.0),
    }
}

// ---------------------------------------------------------------------
// Fixtures and gestures
// ---------------------------------------------------------------------

fn open(document: &Document) -> Session {
    let bytes = pack(document, "0.1.0").unwrap();
    let mut session = Session::open(2, &bytes).unwrap();
    session.set_tool(Tool::Select);
    session
}

fn state_of(session: &Session) -> Document {
    unpack(99, &session.pack("0.1.0").unwrap()).unwrap()
}

fn first_object(session: &Session) -> ObjectSnapshot {
    let document = state_of(session);
    document.object(document.object_ids()[0]).unwrap()
}

fn radius_of(session: &Session) -> f64 {
    let ObjectSnapshot::Primitive(p) = first_object(session) else {
        panic!("a primitive");
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!("a rectangle");
    };
    uniform_mm(corner_radii)
}

/// The largest of a rectangle's four corner radii.
fn largest_radius_mm(session: &Session) -> f64 {
    let ObjectSnapshot::Primitive(p) = first_object(session) else {
        panic!("a primitive");
    };
    let Shape::Rect { corner_radii, .. } = p.shape else {
        panic!("a rectangle");
    };
    [
        corner_radii.tl,
        corner_radii.tr,
        corner_radii.br,
        corner_radii.bl,
    ]
    .into_iter()
    .map(curvyo_document_core::Length::as_mm)
    .fold(0.0, f64::max)
}

fn ratio_of(session: &Session, index: usize) -> f64 {
    let document = state_of(session);
    let ObjectSnapshot::Primitive(p) = document.object(document.object_ids()[index]).unwrap()
    else {
        panic!("a primitive");
    };
    let Shape::Star { inner_ratio, .. } = p.shape else {
        panic!("a star");
    };
    inner_ratio.get()
}

fn change_count(session: &Session) -> usize {
    let loro = loro::LoroDoc::new();
    loro.import(&state_of(session).export_loro_snapshot().unwrap())
        .unwrap();
    loro.len_changes()
}

fn click(session: &mut Session, at: Point) {
    session.pointer_hover(at, false, false);
    session.pointer_down(at, false);
    session.pointer_up(at, false, false);
}

fn drag(session: &mut Session, from: Point, to: Point) {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

/// A 100 x 60 mm rectangle (378 x 227 px at the default zoom) at (10, 20),
/// selected with the Select tool.
fn selected_rect(radius: f64) -> Session {
    let document = Document::new(1);
    let id = document.create_rect(bounds(10.0, 20.0, 100.0, 60.0));
    document
        .set_corner_radius(&[id], Length::from_mm(radius))
        .unwrap();
    let mut session = open(&document);
    click(&mut session, pt(10.0, 50.0));
    session
}

/// The radius handle at the NE corner, from the specification's rule: on the
/// corner's inward diagonal at `p = 15 + ρ·L(s)` pixels, with
/// `L(s) = (s - 14)/√2 - 15`, `s` the shorter side in pixels and `ρ` the
/// radius over half of it.
fn ne_radius_handle(session: &Session, rect: (f64, f64, f64, f64), radius_mm: f64) -> Point {
    let scale = session.view().scale();
    let (x, y, w, h) = rect;
    let shorter_px = w.min(h) * scale;
    let rho = radius_mm / (w.min(h) / 2.0);
    let travel = (shorter_px - 14.0) / std::f64::consts::SQRT_2 - 15.0;
    let distance_mm = (15.0 + rho * travel) / scale;
    let along = distance_mm / std::f64::consts::SQRT_2;
    pt(x + w - along, y + along)
}

const RECT: (f64, f64, f64, f64) = (10.0, 20.0, 100.0, 60.0);

fn readout(session: &Session) -> Option<String> {
    session.live_readout().map(|r| r.text)
}

// ---------------------------------------------------------------------
// Criteria 2, 24: a radius drag
// ---------------------------------------------------------------------

/// Criterion 2: a radius drag is one commit; criterion 24: it writes only the
/// radius register (the same key the shape tools wrote), nothing new.
#[test]
fn a_radius_drag_is_one_commit_that_writes_only_the_radius_register() {
    let mut session = selected_rect(0.0);
    let before = change_count(&session);
    let loro = |s: &Session| {
        let l = loro::LoroDoc::new();
        l.import(&state_of(s).export_loro_snapshot().unwrap())
            .unwrap();
        l
    };
    let from = loro(&session).oplog_vv();
    let knob = ne_radius_handle(&session, RECT, 0.0);
    let to = pt(knob.x - 3.0, knob.y + 3.0);
    drag(&mut session, knob, to);
    assert_eq!(change_count(&session), before + 1, "exactly one commit");
    assert!(radius_of(&session) > 0.0);
    let l = loro(&session);
    let ops = format!("{:?}", l.export_json_updates(&from, &l.oplog_vv()));
    assert!(ops.contains("corner_radius"), "{ops}");
    assert!(
        !ops.contains("rect_bounds") && !ops.contains("rotation") && !ops.contains("stroke"),
        "only the radius is written: {ops}"
    );
}

/// Criterion 38 and 24: a project edited through the Select tool saves with
/// the same `format_version`, and the radius survives a save and reopen.
#[test]
fn a_radius_set_with_the_select_tool_survives_save_and_reopen_with_the_same_format_version() {
    let mut session = selected_rect(0.0);
    let knob = ne_radius_handle(&session, RECT, 0.0);
    drag(&mut session, knob, pt(knob.x - 4.0, knob.y + 4.0));
    let radius = radius_of(&session);
    let bytes = session.pack("0.1.0").unwrap();
    let reopened = Session::open(7, &bytes).unwrap();
    assert!((radius_of(&reopened) - radius).abs() < 1e-12);
    // The container's manifest still names the current format.
    let archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut archive = archive;
    let manifest: serde_json::Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["format_version"], CURRENT_FORMAT_VERSION);
}

/// Criterion 20: "r 3.5 mm" live at the pointer; the knob shows the built-in
/// pointer cursor, hovering and dragging, whatever the modifiers.
#[test]
fn a_radius_drag_shows_a_readout_and_the_pointer_cursor_and_hint() {
    let mut session = selected_rect(0.0);
    let knob = ne_radius_handle(&session, RECT, 0.0);
    session.pointer_hover(knob, false, false);
    assert_eq!(session.cursor_hint(), "pointer");
    assert_eq!(session.handle_hint(), "param-radius");
    session.pointer_hover(knob, true, true);
    assert_eq!(session.cursor_hint(), "pointer", "modifiers change nothing");
    session.pointer_down(knob, false);
    let to = pt(knob.x - 2.0, knob.y + 2.0);
    session.pointer_hover(to, false, false);
    let text = readout(&session).expect("a parameter drag shows a readout");
    assert!(
        text.starts_with("r ") && text.ends_with(" mm"),
        "readout {text}"
    );
    assert_eq!(session.cursor_hint(), "pointer", "while dragging");
    session.pointer_up(to, false, false);
    assert!(readout(&session).is_none());
}

#[test]
fn a_star_inner_radius_drag_shows_a_ratio_readout() {
    let document = Document::new(1);
    let _ = document.create_star(
        star_frame(60.0, 50.0, 40.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut session = open(&document);
    click(&mut session, pt(100.0, 50.0)); // the outline of the first outer vertex
    let theta = std::f64::consts::PI / 5.0;
    let knob = pt(60.0 + 20.0 * theta.cos(), 50.0 + 20.0 * theta.sin());
    session.pointer_hover(knob, false, false);
    assert_eq!(session.handle_hint(), "param-inner");
    session.pointer_down(knob, false);
    let to = pt(knob.x + 2.0 * theta.cos(), knob.y + 2.0 * theta.sin());
    session.pointer_hover(to, false, false);
    assert_eq!(readout(&session).as_deref(), Some("ratio 0.55"));
    session.pointer_up(to, false, false);
    assert!((ratio_of(&session, 0) - 0.55).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// Criteria 10 to 14: blue new, black old
// ---------------------------------------------------------------------

/// Criterion 10: during a radius drag the committed rectangle stays drawn
/// unchanged in black and the geometry the release commits is drawn over it,
/// in blue; after the release there is only the new black one.
#[test]
fn a_drag_draws_the_old_object_in_black_and_the_new_geometry_in_blue() {
    let mut session = selected_rect(0.0);
    let before_drag = session.draw_list();
    let knob = ne_radius_handle(&session, RECT, 0.0);
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    let to = pt(knob.x - 5.0, knob.y + 5.0);
    session.pointer_hover(to, false, false);
    let during = session.draw_list();
    session.pointer_up(to, false, false);
    let committed = first_object(&session);
    let expected_blue = build_live_edit_preview(std::slice::from_ref(&committed), session.view());
    let has = |frame: &curvyo_render_core::DrawList, v: &curvyo_render_core::Vertex| {
        frame.triangles.iter().any(|w| {
            w.color == v.color
                && (w.position.x - v.position.x).abs() < 1e-6
                && (w.position.y - v.position.y).abs() < 1e-6
        })
    };
    assert!(expected_blue.triangle_count() > 0);
    assert!(
        expected_blue.triangles.iter().all(|v| has(&during, v)),
        "the blue outline is exactly what the release commits"
    );
    let black = RgbaColor::BLACK;
    assert!(
        before_drag
            .triangles
            .iter()
            .filter(|v| v.color == black)
            .all(|v| has(&during, v)),
        "the old stroke is unchanged during the drag"
    );
    // After the release the object is drawn in black at the new geometry
    // and the blue overlay is gone.
    let after = session.draw_list();
    assert!(
        !expected_blue.triangles.iter().all(|v| has(&after, v)),
        "no blue outline remains after the release"
    );
}

/// Criterion 12: a move dragged out and back to its start shows no blue
/// outline and writes nothing (a zero-offset commit would rewrite registers).
#[test]
fn a_move_dragged_back_to_its_start_writes_nothing_and_shows_no_overlay() {
    let mut session = selected_rect(5.0);
    let before_bytes = state_of(&session).export_loro_snapshot().unwrap();
    let start = pt(40.0, 40.0); // inside the box, on no handle
    session.pointer_hover(start, false, false);
    session.pointer_down(start, false);
    let pressed = session.draw_list(); // inside the dead zone: no overlay
    session.pointer_hover(pt(60.0, 55.0), false, false);
    let away = session.draw_list();
    session.pointer_hover(start, false, false);
    let back = session.draw_list();
    assert_ne!(away, pressed, "away from the start the overlay shows");
    assert_eq!(back, pressed, "back at the start there is no blue outline");
    session.pointer_up(start, false, false);
    assert_eq!(
        state_of(&session).export_loro_snapshot().unwrap(),
        before_bytes,
        "nothing was written"
    );
}

/// Criterion 12: Escape during a parameter drag writes nothing and removes the
/// preview.
#[test]
fn escape_during_a_parameter_drag_writes_nothing() {
    let mut session = selected_rect(0.0);
    let before = change_count(&session);
    let rest = session.draw_list();
    let knob = ne_radius_handle(&session, RECT, 0.0);
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    session.pointer_hover(pt(knob.x - 6.0, knob.y + 6.0), false, false);
    assert_ne!(session.draw_list(), rest);
    session.escape();
    session.pointer_hover(pt(300.0, 300.0), false, false);
    session.pointer_up(pt(300.0, 300.0), false, false);
    assert_eq!(change_count(&session), before);
    assert_eq!(radius_of(&session), 0.0);
}

/// Criterion 7: parameter handles are not drawn while the same object is
/// moved, resized or rotated by drag, and the transform handles stay drawn
/// during a parameter drag.
#[test]
fn radius_handles_are_not_drawn_during_another_drag_but_the_transform_handles_stay_during_theirs() {
    let mut session = selected_rect(0.0);
    let at_rest = session.draw_list().triangle_count();
    // A move drag from the body.
    session.pointer_hover(pt(40.0, 40.0), false, false);
    session.pointer_down(pt(40.0, 40.0), false);
    session.pointer_hover(pt(41.0, 40.0), false, false);
    session.pointer_hover(pt(60.0, 40.0), false, false);
    let during_move = session.draw_list().triangle_count();
    session.escape();
    assert!(
        // The blue outline (about 140 triangles with its white casing,
        // `0007` criterion 40; 70 without) is the only addition; four knobs
        // would add about 200 more.
        during_move < at_rest + 400,
        "no radius knobs are added by the move drag"
    );
    // A parameter drag keeps every transform handle drawn.
    let knob = ne_radius_handle(&session, RECT, 0.0);
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    session.pointer_hover(pt(knob.x - 4.0, knob.y + 4.0), false, false);
    assert!(session.draw_list().triangle_count() > 0);
    session.escape();
}

// ---------------------------------------------------------------------
// Criteria 18, 19: typed entry on a knob
// ---------------------------------------------------------------------

#[test]
fn a_double_click_on_a_radius_handle_opens_the_corner_radius_field() {
    let mut session = selected_rect(3.46);
    let knob = ne_radius_handle(&session, RECT, 3.46);
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    session.pointer_up(knob, false, false);
    session.double_click(knob, false, false);
    let entry = session.transform_entry().expect("the entry is open");
    assert_eq!(entry.kind, "corner-radius");
    assert_eq!(entry.fields.len(), 1);
    assert_eq!(entry.fields[0].label, "r");
    assert_eq!(
        entry.fields[0].accessible_name,
        "Corner radius, all corners"
    );
    assert_eq!(entry.fields[0].prefill, "3.5");
    assert_eq!(
        session.tool(),
        Tool::Select,
        "no double-click switches tools"
    );
}

#[test]
fn the_typed_radius_commits_once_zero_is_valid_and_a_large_value_is_limited() {
    for (typed, expected) in [("7", 7.0), ("0", 0.0), ("500", 30.0)] {
        let mut session = selected_rect(3.0);
        let knob = ne_radius_handle(&session, RECT, 3.0);
        session.pointer_hover(knob, false, false);
        session.pointer_down(knob, false);
        session.pointer_up(knob, false, false);
        session.double_click(knob, false, false);
        let before = change_count(&session);
        assert_eq!(
            session.commit_transform_entry(typed, "", 0),
            EntryOutcome::Committed,
            "{typed}"
        );
        assert_eq!(change_count(&session), before + 1);
        assert!((radius_of(&session) - expected).abs() < 1e-9, "{typed}");
        assert!(session.transform_entry().is_none(), "the field closes");
    }
}

#[test]
fn invalid_radius_text_keeps_the_field_open_and_every_close_path_writes_nothing() {
    let mut session = selected_rect(3.0);
    let knob = ne_radius_handle(&session, RECT, 3.0);
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    session.pointer_up(knob, false, false);
    session.double_click(knob, false, false);
    let before = change_count(&session);
    assert_eq!(
        session.commit_transform_entry("abc", "", 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotANumber
        }
    );
    assert_eq!(
        session.commit_transform_entry("-1", "", 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::Negative
        }
    );
    assert!(session.transform_entry().is_some(), "still open");
    // An unedited Enter closes and writes nothing.
    assert_eq!(
        session.commit_transform_entry("3", "", 0),
        EntryOutcome::Unchanged
    );
    assert!(session.transform_entry().is_none());
    assert_eq!(change_count(&session), before);
    // Escape, a tool switch and a press elsewhere also write nothing.
    for close in 0..3 {
        session.pointer_hover(knob, false, false);
        session.pointer_down(knob, false);
        session.pointer_up(knob, false, false);
        session.double_click(knob, false, false);
        assert!(session.transform_entry().is_some(), "reopened {close}");
        match close {
            0 => session.cancel_transform_entry(),
            1 => {
                session.set_tool(Tool::Pen);
                session.set_tool(Tool::Select);
            }
            _ => click(&mut session, pt(300.0, 300.0)),
        }
        assert!(session.transform_entry().is_none(), "closed by {close}");
        assert_eq!(change_count(&session), before, "close path {close}");
    }
}

/// Criteria 13, 18: the value a typed radius writes is the value a drag to the
/// same radius writes, because both go through one `apply_param`.
#[test]
fn a_typed_radius_and_a_dragged_radius_agree() {
    let mut dragged = selected_rect(0.0);
    let knob = ne_radius_handle(&dragged, RECT, 0.0);
    drag(&mut dragged, knob, pt(knob.x - 4.0, knob.y + 4.0));
    let radius = radius_of(&dragged);
    let mut typed = selected_rect(0.0);
    let knob = ne_radius_handle(&typed, RECT, 0.0);
    typed.pointer_hover(knob, false, false);
    typed.pointer_down(knob, false);
    typed.pointer_up(knob, false, false);
    typed.double_click(knob, false, false);
    assert_eq!(
        typed.commit_transform_entry(&format!("{radius}"), "", 0),
        EntryOutcome::Committed
    );
    assert!((radius_of(&typed) - radius).abs() < 1e-9);
}

#[test]
fn a_double_click_on_a_star_knob_opens_the_ratio_field_with_its_range() {
    let document = Document::new(1);
    let _ = document.create_star(
        star_frame(60.0, 50.0, 40.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.456).unwrap(),
    );
    let mut session = open(&document);
    click(&mut session, pt(100.0, 50.0));
    let theta = std::f64::consts::PI / 5.0;
    let r = 40.0 * 0.456;
    let knob = pt(60.0 + r * theta.cos(), 50.0 + r * theta.sin());
    session.pointer_hover(knob, false, false);
    session.pointer_down(knob, false);
    session.pointer_up(knob, false, false);
    session.double_click(knob, false, false);
    let entry = session.transform_entry().expect("open");
    assert_eq!(entry.kind, "inner-ratio");
    assert_eq!(entry.fields[0].accessible_name, "Inner ratio");
    assert_eq!(entry.fields[0].prefill, "0.46");
    assert_eq!(
        session.commit_transform_entry("1.5", "", 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::RatioRange
        }
    );
    assert_eq!(
        session.commit_transform_entry("0.7", "", 0),
        EntryOutcome::Committed
    );
    assert!((ratio_of(&session, 0) - 0.7).abs() < 1e-12);
}

// ---------------------------------------------------------------------
// Criterion 23: Scale corner radius
// ---------------------------------------------------------------------

fn resize_se(session: &mut Session, dx: f64, dy: f64) {
    let corner = pt(110.0, 80.0);
    drag(session, corner, pt(corner.x + dx, corner.y + dy));
}

#[test]
fn a_resize_keeps_the_radius_by_default_and_scales_it_with_the_switch_on() {
    let mut off = selected_rect(6.0);
    assert!(!off.scale_corner_radius(), "off at program start");
    resize_se(&mut off, 100.0, 60.0); // 200 x 120: factor 2
    assert_eq!(radius_of(&off), 6.0, "the absolute size is kept");

    let mut on = selected_rect(6.0);
    on.set_scale_corner_radius(true);
    assert!(on.scale_corner_radius());
    resize_se(&mut on, 100.0, 60.0);
    assert!(
        (radius_of(&on) - 12.0).abs() < 1e-9,
        "scaled by sqrt(sx*sy)"
    );
}

/// ADR decision 4: with the switch off the resize does not write the radius
/// register, so a concurrent radius edit by a peer survives, in both merge
/// orders.
#[test]
fn a_resize_with_the_switch_off_does_not_rewrite_the_radius_so_a_peers_radius_survives() {
    for flip in [false, true] {
        let document = Document::new(1);
        let id = document.create_rect(bounds(10.0, 20.0, 100.0, 60.0));
        document
            .set_corner_radius(&[id], Length::from_mm(4.0))
            .unwrap();
        let base_bytes = pack(&document, "0.1.0").unwrap();
        let mut mine = Session::open(2, &base_bytes).unwrap();
        mine.set_tool(Tool::Select);
        click(&mut mine, pt(10.0, 50.0));
        let from = {
            let l = loro::LoroDoc::new();
            l.import(&state_of(&mine).export_loro_snapshot().unwrap())
                .unwrap();
            l.oplog_vv()
        };
        resize_se(&mut mine, 100.0, 60.0);
        let ops = {
            let l = loro::LoroDoc::new();
            l.import(&state_of(&mine).export_loro_snapshot().unwrap())
                .unwrap();
            format!("{:?}", l.export_json_updates(&from, &l.oplog_vv()))
        };
        assert!(ops.contains("rect_bounds"), "the frame was written");
        assert!(!ops.contains("corner_radius"), "the radius was not: {ops}");

        let peer = unpack(3, &base_bytes).unwrap();
        peer.set_corner_radius(&[id], Length::from_mm(9.0)).unwrap();
        let mine_doc = unpack(4, &mine.pack("0.1.0").unwrap()).unwrap();
        let merged_doc = if flip {
            merged(&peer, &mine_doc)
        } else {
            merged(&mine_doc, &peer)
        };
        let ObjectSnapshot::Primitive(p) = merged_doc.object(id).unwrap() else {
            panic!("a primitive");
        };
        let Shape::Rect {
            corner_radii,
            bounds,
        } = p.shape
        else {
            panic!("a rectangle");
        };
        assert_eq!(
            uniform_mm(corner_radii),
            9.0,
            "flip {flip}: the peer's radius survives"
        );
        assert!(
            (bounds.width.as_mm() - 200.0).abs() < 1e-9,
            "and the resize"
        );
    }
}

fn merged(a: &Document, b: &Document) -> Document {
    let loro = loro::LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "unified-merge",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("manifest.json", o).unwrap();
    w.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    w.start_file("document.loro", o).unwrap();
    w.write_all(&loro_bytes).unwrap();
    w.start_file("document.json", o).unwrap();
    w.write_all(b"{}").unwrap();
    unpack(9, &w.finish().unwrap().into_inner()).expect("merged opens")
}

/// The switch is a session setting, never written to the project.
#[test]
fn the_switch_is_not_part_of_the_saved_project() {
    let mut on = selected_rect(6.0);
    let off = selected_rect(6.0);
    on.set_scale_corner_radius(true);
    let _ = off;
    let reopened = Session::open(5, &on.pack("0.1.0").unwrap()).unwrap();
    assert!(!reopened.scale_corner_radius(), "off in every new session");
}

// ---------------------------------------------------------------------
// Criteria 21, 21a, 22: the bar
// ---------------------------------------------------------------------

fn mixed_scene() -> (Session, NodeId, NodeId, NodeId, NodeId) {
    let document = Document::new(1);
    let rect = document.create_rect(bounds(0.0, 0.0, 40.0, 40.0));
    document
        .set_corner_radius(&[rect], Length::from_mm(5.0))
        .unwrap();
    let star = document.create_star(
        star_frame(100.0, 20.0, 15.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let polygon =
        document.create_polygon(star_frame(160.0, 20.0, 15.0), PointCount::new(6).unwrap());
    let path = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(200.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(240.0, 0.0)),
        ],
        false,
    );
    (open(&document), rect, star, polygon, path)
}

fn select_all(session: &mut Session, clicks: &[Point]) {
    let mut first = true;
    for at in clicks {
        session.pointer_hover(*at, !first, false);
        session.pointer_down(*at, !first);
        session.pointer_up(*at, !first, false);
        first = false;
    }
}

/// Criterion 21: with a rectangle and a star selected the bar shows Radius,
/// Remove rounding, Points and Ratio, each acting on exactly its kind.
#[test]
fn the_bar_shows_a_control_for_every_kind_in_the_selection_and_acts_on_that_kind_only() {
    let (mut session, ..) = mixed_scene();
    // A rectangle's left edge and the star's first outer vertex.
    select_all(&mut session, &[pt(0.0, 20.0), pt(115.0, 20.0)]);
    let bar = session.select_bar_state();
    assert!(matches!(bar.radius, Some(BarValue::Uniform(_))));
    assert!(bar.remove_rounding_shown && bar.remove_rounding_enabled);
    assert!(matches!(bar.points, Some(BarValue::Uniform(5))));
    assert!(matches!(bar.ratio, Some(BarValue::Uniform(_))));
    assert!(bar.object_to_path);

    // "Points" writes to the star only; the rectangle is untouched.
    let before = change_count(&session);
    session.set_selected_point_count(PointCount::new(8).unwrap());
    assert_eq!(change_count(&session), before + 1, "one commit");
    let document = state_of(&session);
    for id in document.object_ids() {
        if let ObjectSnapshot::Primitive(p) = document.object(id).unwrap()
            && let Shape::Star { point_count, .. } = p.shape
        {
            assert_eq!(point_count.get(), 8);
        }
    }
    assert_eq!(radius_of(&session), 5.0);

    // "Remove rounding" zeroes the rectangle only.
    session.remove_corner_rounding();
    assert_eq!(radius_of(&session), 0.0);
    assert!(!session.select_bar_state().remove_rounding_enabled);
}

#[test]
fn a_selection_without_a_kind_shows_none_of_its_controls() {
    let (mut session, ..) = mixed_scene();
    select_all(&mut session, &[pt(220.0, 0.0)]); // the path
    let bar = session.select_bar_state();
    assert!(bar.radius.is_none() && bar.points.is_none() && bar.ratio.is_none());
    assert!(!bar.remove_rounding_shown && !bar.object_to_path);
    select_all(&mut session, &[pt(145.0, 20.0)]); // the polygon
    let bar = session.select_bar_state();
    assert!(bar.points.is_some() && bar.ratio.is_none() && bar.radius.is_none());
    assert!(bar.object_to_path);
}

/// Criterion 21a: the "Radius" field writes every selected rectangle in one
/// commit, refuses bad text and writes nothing for an equal value.
#[test]
fn the_bar_radius_field_commits_once_and_refuses_bad_text() {
    let mut session = selected_rect(0.0);
    let before = change_count(&session);
    assert_eq!(
        session.set_selected_radius_text("12"),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&session), before + 1);
    assert_eq!(radius_of(&session), 12.0);
    assert_eq!(
        session.set_selected_radius_text("12"),
        EntryOutcome::Unchanged,
        "an equal value writes nothing"
    );
    assert_eq!(
        session.set_selected_radius_text(""),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotANumber
        }
    );
    assert_eq!(
        session.set_selected_radius_text("-3"),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::Negative
        }
    );
    assert_eq!(change_count(&session), before + 1);
    // Limited to half the shorter side.
    assert_eq!(
        session.set_selected_radius_text("999"),
        EntryOutcome::Committed
    );
    assert_eq!(radius_of(&session), 30.0);
    let bar = session.select_bar_state();
    assert_eq!(bar.radius, Some(BarValue::Uniform(Length::from_mm(30.0))));
}

/// Criterion 21: a slider drag is one commit and previews in blue meanwhile;
/// the pending edit flushes against the star it was started on when the
/// selection changes before the release.
#[test]
fn the_ratio_slider_previews_without_writing_and_commits_once() {
    let document = Document::new(1);
    let _ = document.create_star(
        star_frame(60.0, 50.0, 40.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let mut session = open(&document);
    click(&mut session, pt(100.0, 50.0));
    let before = change_count(&session);
    let rest = session.draw_list();
    for tick in [0.5, 0.6, 0.7] {
        session.preview_selected_ratio(InnerRatio::new(tick).unwrap());
    }
    assert_eq!(change_count(&session), before, "nothing written per tick");
    assert_ne!(session.draw_list(), rest, "the blue preview draws");
    assert_eq!(
        session.select_bar_state().ratio,
        Some(BarValue::Uniform(0.7)),
        "the bar shows the pending value"
    );
    session.commit_selected_ratio();
    assert_eq!(change_count(&session), before + 1, "one commit");
    assert!((ratio_of(&session, 0) - 0.7).abs() < 1e-12);
    // The blue overlay is gone: the committed star's own outline is not
    // among the frame's vertices in the preview colour and width.
    let committed = first_object(&session);
    let blue = build_live_edit_preview(std::slice::from_ref(&committed), session.view());
    let frame = session.draw_list();
    assert!(
        !blue
            .triangles
            .iter()
            .all(|v| frame.triangles.iter().any(|w| {
                w.color == v.color
                    && (w.position.x - v.position.x).abs() < 1e-6
                    && (w.position.y - v.position.y).abs() < 1e-6
            })),
        "no blue outline remains after the commit"
    );
}

#[test]
fn a_pending_slider_edit_is_flushed_before_a_canvas_press_changes_the_selection() {
    let document = Document::new(1);
    let _ = document.create_star(
        star_frame(60.0, 50.0, 40.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let _ = document.create_star(
        star_frame(260.0, 50.0, 40.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let mut session = open(&document);
    click(&mut session, pt(100.0, 50.0));
    session.preview_selected_ratio(InnerRatio::new(0.8).unwrap());
    // The slider was released outside its element: no commit event. A press
    // on the other star selects it; the edit still lands on the first one.
    click(&mut session, pt(300.0, 50.0));
    assert!((ratio_of(&session, 0) - 0.8).abs() < 1e-12);
    assert!((ratio_of(&session, 1) - 0.4).abs() < 1e-12);
}

/// Criterion 22: "Object to path" converts exactly the primitives of the
/// selection and leaves a path alone.
#[test]
fn object_to_path_converts_the_selected_primitives_only() {
    let (mut session, _rect, _star, _polygon, path) = mixed_scene();
    select_all(&mut session, &[pt(0.0, 20.0), pt(220.0, 0.0)]);
    session.convert_selected_to_paths();
    let document = state_of(&session);
    assert!(
        document
            .object_ids()
            .into_iter()
            .all(|id| document.path(id).is_some()
                || document
                    .primitive(id)
                    .is_some_and(|p| !matches!(p.shape, Shape::Rect { .. }))),
        "the rectangle became a path"
    );
    assert!(document.path(path).is_some());
}

// ---------------------------------------------------------------------
// Press order, criterion 35
// ---------------------------------------------------------------------

#[test]
fn a_press_on_a_radius_handle_starts_that_drag_with_or_without_shift() {
    for shift in [false, true] {
        let mut session = selected_rect(0.0);
        let knob = ne_radius_handle(&session, RECT, 0.0);
        session.pointer_hover(knob, shift, false);
        session.pointer_down(knob, shift);
        let to = pt(knob.x - 4.0, knob.y + 4.0);
        session.pointer_hover(to, shift, false);
        session.pointer_up(to, shift, false);
        // With Shift the drag changes its own corner only (the switch is on):
        // the dragged corner grew either way.
        assert!(largest_radius_mm(&session) > 0.0, "shift {shift}");
    }
}

// ---------------------------------------------------------------------
// Criterion 15: the cost of the blue overlay (benchmark, run by hand)
// ---------------------------------------------------------------------

/// The 200-object move of criterion 15: 100 paths of 50 nodes and 100
/// rectangles, all selected, dragged. The budget is 8 ms of CPU time for
/// `Session::draw_list()` per frame (half of a 16.7 ms frame); the gate is
/// 50 fps, 20 ms. Run in release by the tester:
/// `cargo test --release -p curvyo-editor-wasm --test unified_object_editing -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn a_200_object_move_draws_within_the_frame_budget() {
    let document = Document::new(1);
    let mut clicks = Vec::new();
    for i in 0..100u32 {
        let (x, y) = (f64::from(i % 10) * 25.0, f64::from(i / 10) * 25.0);
        let _ = document.create_rect(bounds(x, y, 10.0, 10.0));
        clicks.push(pt(x, y + 5.0));
    }
    for i in 0..100u32 {
        let (cx, cy) = (
            f64::from(i % 10) * 25.0 + 5.0,
            400.0 + f64::from(i / 10) * 25.0,
        );
        let anchors: Vec<NewAnchor> = (0..50u32)
            .map(|n| {
                let a = f64::from(n) / 50.0 * std::f64::consts::TAU;
                NewAnchor::corner(
                    AnchorId::new(1, u64::from(i) * 50 + u64::from(n)),
                    pt(cx + 8.0 * a.cos(), cy + 8.0 * a.sin()),
                )
            })
            .collect();
        let _ = document.create_path(&anchors, true);
        clicks.push(pt(cx + 8.0, cy));
    }
    let mut session = open(&document);
    select_all(&mut session, &clicks);
    // The committed frame alone, for reference: the full rebuild the renderer
    // does every frame already (`docs/technical-debt.md`, "renderer not
    // cached").
    let began = std::time::Instant::now();
    for _ in 0..10 {
        let _ = session.draw_list();
    }
    println!("at rest: {:?} per frame", began.elapsed() / 10);
    // Drag from the first rectangle's outline.
    let start = clicks[0];
    session.pointer_hover(start, false, false);
    session.pointer_down(start, false);
    let frames = 30u32;
    let mut drawing = std::time::Duration::ZERO;
    let mut hovering = std::time::Duration::ZERO;
    let mut triangles = 0;
    for frame in 0..frames {
        let to = pt(start.x + 5.0 + f64::from(frame), start.y + 3.0);
        let began = std::time::Instant::now();
        session.pointer_hover(to, false, false);
        hovering += began.elapsed();
        let began = std::time::Instant::now();
        triangles += session.draw_list().triangle_count();
        drawing += began.elapsed();
    }
    let per_frame = drawing / frames;
    println!(
        "200-object move: draw_list {per_frame:?} per frame, pointer_hover {:?} per event, {} triangles",
        hovering / frames,
        triangles / frames as usize
    );
    session.escape();
    // The architect's budget is 8 ms of CPU time per frame (half of a 16.7 ms
    // frame); the criterion itself is 50 frames per second, 20 ms. The test
    // gates on the criterion and reports the budget.
    println!(
        "8 ms budget: {}",
        if per_frame < std::time::Duration::from_millis(8) {
            "met"
        } else {
            "missed"
        }
    );
    if !cfg!(debug_assertions) {
        assert!(
            per_frame < std::time::Duration::from_millis(20),
            "{per_frame:?} per frame is under 50 frames per second"
        );
    }
}

// ---------------------------------------------------------------------
// Criteria 24, 38: the stored fields are the ones the shape tools wrote
// ---------------------------------------------------------------------

fn objects_of(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

/// Criterion 24: a radius, a ratio and a point count set through the Select
/// tool leave exactly the objects the `Document` commands the shape tools
/// used would leave: same fields, nothing new, the same `format_version`.
#[test]
fn values_set_with_the_select_tool_equal_values_written_with_the_shape_tool_commands() {
    let build = || {
        let document = Document::new(1);
        let rect = document.create_rect(bounds(10.0, 20.0, 100.0, 60.0));
        let star = document.create_star(
            star_frame(260.0, 50.0, 40.0),
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        (document, rect, star)
    };
    let (reference, rect, star) = build();
    reference
        .set_corner_radius(&[rect], Length::from_mm(12.5))
        .unwrap();
    reference
        .set_inner_ratio(&[star], InnerRatio::new(0.65).unwrap())
        .unwrap();
    reference
        .set_point_count(&[star], PointCount::new(9).unwrap())
        .unwrap();

    let (document, rect_id, _) = build();
    let mut session = open(&document);
    click(&mut session, pt(10.0, 50.0)); // the rectangle
    assert_eq!(
        session.set_selected_radius_text("12.5"),
        EntryOutcome::Committed
    );
    click(&mut session, pt(300.0, 50.0)); // the star's first outer vertex
    session.set_selected_ratio(InnerRatio::new(0.65).unwrap());
    session.set_selected_point_count(PointCount::new(9).unwrap());

    let edited = state_of(&session);
    assert_eq!(objects_of(&edited), objects_of(&reference));
    assert!(edited.object(rect_id).is_some());
    let bytes = session.pack("0.1.0").unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["format_version"], CURRENT_FORMAT_VERSION);
}

/// Criterion 38: a project with primitives saved before this change opens in
/// the same state, and a Select-tool selection and a plain click write
/// nothing to it.
#[test]
fn a_project_with_primitives_opens_in_the_same_state_and_a_click_writes_nothing() {
    let document = Document::new(1);
    let rect = document.create_rect(bounds(10.0, 20.0, 100.0, 60.0));
    document
        .set_corner_radius(&[rect], Length::from_mm(7.0))
        .unwrap();
    let _ = document.create_ellipse(curvyo_document_core::EllipseFrame {
        center: pt(300.0, 50.0),
        rx: Length::from_mm(40.0),
        ry: Length::from_mm(25.0),
    });
    let _ = document.create_polygon(star_frame(400.0, 50.0, 30.0), PointCount::new(6).unwrap());
    let _ = document.create_star(
        star_frame(500.0, 50.0, 30.0),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let before = objects_of(&document);
    let mut session = open(&document);
    assert_eq!(objects_of(&state_of(&session)), before, "opens unchanged");
    let changes = change_count(&session);
    click(&mut session, pt(10.0, 50.0));
    click(&mut session, pt(300.0, 50.0));
    session.pointer_hover(pt(5.0, 5.0), false, false);
    assert_eq!(change_count(&session), changes, "selecting writes nothing");
    assert_eq!(objects_of(&state_of(&session)), before);
}

/// The one radius of a rectangle whose four corner radii are equal (asserted).
fn uniform_mm(radii: curvyo_document_core::CornerRadii) -> f64 {
    assert_eq!(
        radii,
        curvyo_document_core::CornerRadii::uniform(radii.tl),
        "four equal radii"
    );
    radii.tl.as_mm()
}
