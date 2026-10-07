//! Independent tester acceptance tests for PR 1 of
//! `specs/edit-interaction-polish/specification.md`, Part A (criteria 1 to 8:
//! the shown angle of a polygon or star, Ctrl snap of the create-drag and of
//! an absolute Ctrl rotate, typed angle, old files). Written from the
//! specification before the implementation diff was read. Everything goes
//! through `Session`'s public API; expected angles come from the outline
//! geometry (`outline_of_rotated`, the first vertex) and from tables written
//! here, never from `ObjectSnapshot::orientation`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(missing_docs, clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::f64::consts::PI;

use curvyo_document_core::{
    Angle, Document, InnerRatio, Length, ObjectSnapshot, Point, PointCount, PrimitiveSnapshot,
    RectBounds, Shape, StarFrame, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, PolyStarMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Wraps degrees into (-180, 180].
fn wrap(d: f64) -> f64 {
    let mut x = d % 360.0;
    if x > 180.0 {
        x -= 360.0;
    }
    if x <= -180.0 {
        x += 360.0;
    }
    x
}

/// Difference of two angles in degrees, modulo 360, in (-180, 180].
fn adiff(a: f64, b: f64) -> f64 {
    wrap(a - b)
}

fn make_session(mode: PolyStarMode, n: u32, ratio: f64) -> Session {
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(mode);
    s.set_poly_star_point_count(PointCount::new(n).unwrap());
    s.set_poly_star_ratio(InnerRatio::new(ratio).unwrap());
    s
}

fn create(s: &mut Session, a: Point, b: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(b, shift, ctrl);
    s.pointer_up(b, shift, ctrl);
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

fn prim_of(s: &Session, index: usize) -> PrimitiveSnapshot {
    let d = doc_of(s);
    d.primitive(d.object_ids()[index]).unwrap()
}

fn frame_of(p: &PrimitiveSnapshot) -> StarFrame {
    match p.shape {
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame,
        _ => panic!("polygon or star expected"),
    }
}

fn first_vertex(p: &PrimitiveSnapshot) -> Point {
    outline_of_rotated(&p.shape, p.rotation)[0].point
}

/// The real orientation from the geometry: the clockwise angle (document Y
/// down) of the first outer vertex from straight right, in degrees.
fn geom_angle(p: &PrimitiveSnapshot) -> f64 {
    let c = frame_of(p).center;
    let v = first_vertex(p);
    (v.y - c.y).atan2(v.x - c.x).to_degrees()
}

fn key(k: &str) -> KeyInput<'_> {
    KeyInput {
        key: k,
        ..KeyInput::default()
    }
}

fn parse_deg(text: &str) -> f64 {
    text.trim()
        .trim_end_matches('°')
        .replace('\u{2212}', "-")
        .trim()
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("not an angle: {text:?}"))
}

/// The angle shown in the typed-angle chip (opened by R), then closed again.
fn chip_angle(s: &mut Session) -> f64 {
    assert_eq!(
        s.key_down(key("r")),
        KeyOutcome::EntryOpened,
        "R opens the chip"
    );
    let v = s.transform_entry().expect("entry view");
    assert_eq!(v.kind, "angle");
    let a = parse_deg(&v.fields[0].prefill);
    s.cancel_transform_entry();
    a
}

/// Types `text` into the angle chip (opened by R) and commits.
fn type_angle(s: &mut Session, text: &str) -> EntryOutcome {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    s.commit_transform_entry(text, "", 0)
}

fn swept_to(handle: Point, pivot: Point, deg: f64) -> Point {
    let v = (handle.x - pivot.x, handle.y - pivot.y);
    let end = v.1.atan2(v.0) + deg.to_radians();
    let dist = v.0.hypot(v.1);
    pt(pivot.x + dist * end.cos(), pivot.y + dist * end.sin())
}

/// Rotates the selected object by dragging its top-right corner rotate
/// handle (where the R chip sits) `deg` clockwise about the box centre,
/// leaving the dead zone first. Returns the live readout at the target.
fn rotate_drag(s: &mut Session, deg: f64, ctrl: bool) -> Option<String> {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    let (h, c) = (v.handle, v.center);
    s.cancel_transform_entry();
    s.pointer_hover(h, false, ctrl);
    s.pointer_down(h, false);
    // a detour well outside the 3 px dead zone, then the target
    let detour = swept_to(h, c, if deg >= 0.0 { 40.0 } else { -40.0 });
    s.pointer_hover(detour, false, ctrl);
    let to = swept_to(h, c, deg);
    s.pointer_hover(to, false, ctrl);
    let text = s.live_readout().map(|r| r.text);
    s.pointer_up(to, false, ctrl);
    text
}

/// The nearest stop of the union of multiples of 15 and 22.5 degrees, or
/// `None` on a tie (the table does not say which way a tie goes).
fn nearest_stop(deg: f64) -> Option<f64> {
    let mut stops: Vec<f64> = Vec::new();
    for i in -48..=48 {
        stops.push(f64::from(i) * 15.0);
    }
    for i in -32..=32 {
        stops.push(f64::from(i) * 22.5);
    }
    let mut best = (f64::MAX, 0.0);
    let mut second = f64::MAX;
    for st in stops {
        let d = adiff(deg, st).abs();
        if d < best.0 - 1e-9 {
            second = best.0;
            best = (d, st);
        } else if d < second && (d - best.0).abs() > 1e-9 {
            second = d;
        } else if (d - best.0).abs() <= 1e-9 && (st - best.1).abs() > 1e-9 {
            // a different stop equally far: a tie, unless same direction mod 360
            if adiff(st, best.1).abs() > 1e-9 {
                return None;
            }
        }
    }
    let _ = second;
    Some(wrap(best.1))
}

const A: (f64, f64) = (100.0, 50.0);

fn drag_point(deg: f64, r: f64) -> Point {
    let t = deg.to_radians();
    pt(A.0 + r * t.cos(), A.1 + r * t.sin())
}

// ---------------------------------------------------------------------
// Criteria 1 and 3: the shown angle is the real orientation; create-drag
// ---------------------------------------------------------------------

#[test]
fn ac01_ac03_create_drag_points_the_first_vertex_at_the_pointer_for_every_n_and_angle() {
    for (mode, ratio) in [
        (PolyStarMode::Polygon, 0.5),
        (PolyStarMode::Star, 0.5),
        (PolyStarMode::Star, 0.01),
        (PolyStarMode::Star, 0.99),
    ] {
        for n in [3, 4, 5, 6, 7, 8, 9, 10, 12, 16, 33, 100, 255, 1024] {
            for deg in [
                -179.0, -135.0, -90.0, -45.0, -11.3, 0.0, 22.5, 45.0, 90.0, 120.0, 179.0,
            ] {
                let mut s = make_session(mode, n, ratio);
                let b = drag_point(deg, 12.3);
                create(&mut s, pt(A.0, A.1), b, false, false);
                assert_eq!(n_objects(&s), 1, "{mode:?} n={n} deg={deg}");
                let p = prim_of(&s, 0);
                let f = frame_of(&p);
                assert!((f.center.x - A.0).abs() < 1e-9 && (f.center.y - A.1).abs() < 1e-9);
                assert!((f.radius.as_mm() - 12.3).abs() < 1e-9, "radius");
                let v = first_vertex(&p);
                assert!(
                    (v.x - b.x).abs() < 1e-7 && (v.y - b.y).abs() < 1e-7,
                    "{mode:?} n={n} deg={deg}: first vertex {v:?} vs pointer {b:?}"
                );
                assert!(adiff(geom_angle(&p), deg).abs() < 1e-6);
            }
        }
    }
}

#[test]
fn ac01_ac03_after_release_select_tool_is_active_and_the_chip_shows_the_real_angle() {
    for (mode, n) in [
        (PolyStarMode::Polygon, 3),
        (PolyStarMode::Polygon, 4),
        (PolyStarMode::Polygon, 7),
        (PolyStarMode::Star, 5),
        (PolyStarMode::Star, 12),
    ] {
        for deg in [
            -180.0 + 0.5,
            -150.0,
            -90.0,
            -33.7,
            -0.1,
            0.0,
            0.1,
            45.0,
            78.7,
            90.0,
            133.3,
            179.5,
        ] {
            let mut s = make_session(mode, n, 0.4);
            create(&mut s, pt(A.0, A.1), drag_point(deg, 20.0), false, false);
            assert_eq!(s.tool(), Tool::Select, "criterion 3: Select is active");
            assert_eq!(s.selected_object_count(), 1);
            let got = chip_angle(&mut s);
            assert!(
                adiff(got, deg).abs() <= 0.051,
                "{mode:?} n={n} deg={deg}: chip shows {got}"
            );
            // the geometry agrees with what is shown
            assert!(adiff(geom_angle(&prim_of(&s, 0)), got).abs() <= 0.051);
        }
    }
}

#[test]
fn ac01_the_four_cardinal_directions_show_0_90_minus_90_and_180() {
    for (deg, want) in [(0.0, 0.0), (90.0, 90.0), (-90.0, -90.0)] {
        let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
        create(&mut s, pt(A.0, A.1), drag_point(deg, 10.0), false, false);
        let got = chip_angle(&mut s);
        assert!((got - want).abs() < 0.051, "deg {deg}: got {got}");
    }
    // straight left: 180 (the range is -180..180; either end is the same
    // direction, the spec's table says "left, 180")
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    create(&mut s, pt(A.0, A.1), pt(A.0 - 10.0, A.1), false, false);
    let got = chip_angle(&mut s);
    assert!(
        (got - 180.0).abs() < 0.051 || (got + 180.0).abs() < 0.051,
        "left shows {got}"
    );
    assert!(
        (got - 180.0).abs() < 0.051,
        "spec: left shows 180, got {got}"
    );
}

#[test]
fn ac01_examples_triangle_vertex_up_is_minus_90_square_at_45_is_upright() {
    // a triangle with its vertex up shows -90 and stands on its base
    let mut s = make_session(PolyStarMode::Polygon, 3, 0.5);
    create(&mut s, pt(A.0, A.1), pt(A.0, A.1 - 10.0), false, false);
    let p = prim_of(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    assert!(
        (pts[1].point.y - pts[2].point.y).abs() < 1e-9,
        "base horizontal"
    );
    assert!(pts[1].point.y > pts[0].point.y, "base below the apex");
    assert!((chip_angle(&mut s) + 90.0).abs() < 0.051);
    // a 4-point polygon at 45 is an upright square
    let mut s = make_session(PolyStarMode::Polygon, 4, 0.5);
    create(&mut s, pt(A.0, A.1), drag_point(45.0, 10.0), false, false);
    let p = prim_of(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    for i in 0..4 {
        let (a, b) = (pts[i].point, pts[(i + 1) % 4].point);
        assert!(
            (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9,
            "edge {i} axis-parallel"
        );
    }
    // a star with its first tip up shows -90
    let mut s = make_session(PolyStarMode::Star, 5, 0.5);
    create(&mut s, pt(A.0, A.1), pt(A.0, A.1 - 10.0), false, false);
    assert!((chip_angle(&mut s) + 90.0).abs() < 0.051);
}

#[test]
fn ac01_the_shown_angle_survives_deselect_reselect_save_and_reopen() {
    let mut s = make_session(PolyStarMode::Star, 5, 0.5);
    create(&mut s, pt(A.0, A.1), drag_point(-37.0, 20.0), false, false);
    let want = chip_angle(&mut s);
    assert!((want + 37.0).abs() < 0.051);
    // deselect with a click on empty canvas, reselect by the outline
    s.pointer_hover(pt(400.0, 400.0), false, false);
    s.pointer_down(pt(400.0, 400.0), false);
    s.pointer_up(pt(400.0, 400.0), false, false);
    assert_eq!(s.selected_object_count(), 0);
    let p = prim_of(&s, 0);
    let tip = first_vertex(&p);
    s.pointer_hover(tip, false, false);
    s.pointer_down(tip, false);
    s.pointer_up(tip, false, false);
    assert_eq!(s.selected_object_count(), 1);
    assert!((chip_angle(&mut s) - want).abs() < 1e-9);
    // save and reopen
    let bytes = s.pack("0.1.0").unwrap();
    let mut s2 = Session::open(5, &bytes).unwrap();
    s2.set_tool(Tool::Select);
    s2.pointer_hover(tip, false, false);
    s2.pointer_down(tip, false);
    s2.pointer_up(tip, false, false);
    assert_eq!(s2.selected_object_count(), 1);
    assert!((chip_angle(&mut s2) - want).abs() < 1e-9);
    assert_eq!(first_vertex(&prim_of(&s2, 0)), tip);
}

// ---------------------------------------------------------------------
// Criterion 4, 5: Ctrl snap of the create-drag
// ---------------------------------------------------------------------

#[test]
fn ac04_worked_example_a_100_50_b_110_48_ctrl() {
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    create(&mut s, pt(100.0, 50.0), pt(110.0, 48.0), false, true);
    let p = prim_of(&s, 0);
    let v = first_vertex(&p);
    assert!(
        (v.x - 109.85).abs() < 0.01 && (v.y - 47.36).abs() < 0.01,
        "{v:?}"
    );
    let r = frame_of(&p).radius.as_mm();
    assert!((r - 10.20).abs() < 0.01, "radius {r}");
    assert!((chip_angle(&mut s) + 15.0).abs() < 0.051);
    // without Ctrl the angle is the raw one
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    create(&mut s, pt(100.0, 50.0), pt(110.0, 48.0), false, false);
    let p = prim_of(&s, 0);
    let raw = (-2.0_f64).atan2(10.0).to_degrees();
    assert!(adiff(geom_angle(&p), raw).abs() < 1e-6);
    assert!((first_vertex(&p).x - 110.0).abs() < 1e-7);
}

#[test]
fn ac04_ctrl_create_drag_snaps_to_the_table_in_every_quadrant_and_keeps_the_radius() {
    let mut checked = 0;
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        let mut raw = -180.0;
        while raw < 180.0 {
            if let Some(stop) = nearest_stop(raw) {
                let mut s = make_session(mode, 6, 0.5);
                let r = 17.0;
                create(&mut s, pt(A.0, A.1), drag_point(raw, r), false, true);
                let p = prim_of(&s, 0);
                let f = frame_of(&p);
                assert!((f.radius.as_mm() - r).abs() < 1e-9, "radius stays |AB|");
                let want = drag_point(stop, r);
                let v = first_vertex(&p);
                assert!(
                    (v.x - want.x).abs() < 1e-6 && (v.y - want.y).abs() < 1e-6,
                    "{mode:?} raw {raw} -> stop {stop}: vertex {v:?} want {want:?}"
                );
                checked += 1;
            }
            raw += 1.3;
        }
    }
    assert!(checked > 400);
}

#[test]
fn ac04_stops_include_22_5_multiples_negative_and_positive() {
    for (raw, want) in [
        (10.0, 15.0),
        (19.0, 22.5),
        (26.0, 22.5),
        (26.5, 30.0),
        (40.0, 45.0),
        (64.0, 67.5),
        (71.0, 67.5),
        (-10.0, -15.0),
        (-19.0, -22.5),
        (-64.0, -67.5),
        (160.0, 157.5),
        (-160.0, -157.5),
        (170.0, 165.0),
        (178.0, 180.0),
        (-178.0, 180.0),
        (3.0, 0.0),
        (-3.0, 0.0),
        (88.0, 90.0),
        (-88.0, -90.0),
        (112.0, 112.5),
        (-112.0, -112.5),
    ] {
        let mut s = make_session(PolyStarMode::Polygon, 3, 0.5);
        create(&mut s, pt(A.0, A.1), drag_point(raw, 9.0), false, true);
        let got = geom_angle(&prim_of(&s, 0));
        assert!(
            adiff(got, want).abs() < 1e-6,
            "raw {raw}: {got}, want {want}"
        );
    }
}

#[test]
fn ac05_ctrl_is_read_from_the_release_event_and_preview_follows_modifier_changes() {
    // pressed with Ctrl held but released without: the raw angle
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    s.pointer_hover(pt(100.0, 50.0), false, true);
    s.pointer_down(pt(100.0, 50.0), false);
    s.pointer_hover(pt(110.0, 48.0), false, true);
    s.pointer_up(pt(110.0, 48.0), false, false);
    let raw = (-2.0_f64).atan2(10.0).to_degrees();
    assert!(
        adiff(geom_angle(&prim_of(&s, 0)), raw).abs() < 1e-6,
        "release without Ctrl is raw"
    );
    // moved without Ctrl, released with it: snapped
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    s.pointer_hover(pt(100.0, 50.0), false, false);
    s.pointer_down(pt(100.0, 50.0), false);
    s.pointer_hover(pt(110.0, 48.0), false, false);
    s.pointer_up(pt(110.0, 48.0), false, true);
    assert!(
        adiff(geom_angle(&prim_of(&s, 0)), -15.0).abs() < 1e-6,
        "release with Ctrl snaps"
    );

    // pointer held still: Ctrl pressed and released changes preview + readout
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    s.pointer_hover(pt(100.0, 50.0), false, false);
    s.pointer_down(pt(100.0, 50.0), false);
    s.pointer_hover(pt(110.0, 48.0), false, false);
    let raw_text = s.live_readout().unwrap().text;
    let raw_draw = s.draw_list();
    // the host (frontend `applyModifiers`) calls modifiers_changed and then
    // re-runs the hover at the last pointer position
    s.modifiers_changed(false, true);
    s.pointer_hover(pt(110.0, 48.0), false, true);
    let snapped_text = s.live_readout().unwrap().text;
    let snapped_draw = s.draw_list();
    assert_eq!(snapped_text, "r 10.2 mm, -15°");
    assert_eq!(raw_text, "r 10.2 mm, -11.3°");
    assert_ne!(raw_draw, snapped_draw, "preview changes on the next frame");
    s.modifiers_changed(false, false);
    s.pointer_hover(pt(110.0, 48.0), false, false);
    assert_eq!(s.live_readout().unwrap().text, raw_text);
    assert_eq!(s.draw_list(), raw_draw);
    s.escape();
    assert_eq!(n_objects(&s), 0, "Escape cancels and writes nothing");
}

#[test]
fn ac05_shift_has_no_effect_on_a_polygon_or_star_create_drag() {
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        for ctrl in [false, true] {
            let mut a = make_session(mode, 6, 0.5);
            let mut b = make_session(mode, 6, 0.5);
            create(&mut a, pt(A.0, A.1), drag_point(-33.0, 14.0), false, ctrl);
            create(&mut b, pt(A.0, A.1), drag_point(-33.0, 14.0), true, ctrl);
            assert_eq!(first_vertex(&prim_of(&a, 0)), first_vertex(&prim_of(&b, 0)));
            assert_eq!(frame_of(&prim_of(&a, 0)), frame_of(&prim_of(&b, 0)));
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 6: readout
// ---------------------------------------------------------------------

#[test]
fn ac06_readout_strings() {
    let cases = [
        (
            PolyStarMode::Polygon,
            false,
            (110.0, 48.0),
            "r 10.2 mm, -11.3°",
        ),
        (
            PolyStarMode::Polygon,
            true,
            (110.0, 48.0),
            "r 10.2 mm, -15°",
        ),
        (
            PolyStarMode::Star,
            false,
            (110.0, 48.0),
            "r 10.2 mm, ratio 0.50, -11.3°",
        ),
        (
            PolyStarMode::Star,
            true,
            (110.0, 48.0),
            "r 10.2 mm, ratio 0.50, -15°",
        ),
        (
            PolyStarMode::Polygon,
            false,
            (100.0, 62.0),
            "r 12.0 mm, 90°",
        ),
        (
            PolyStarMode::Polygon,
            false,
            (100.0, 38.0),
            "r 12.0 mm, -90°",
        ),
        (PolyStarMode::Polygon, false, (112.0, 50.0), "r 12.0 mm, 0°"),
        (
            PolyStarMode::Polygon,
            true,
            (
                A.0 + 9.5 * 112.0_f64.to_radians().cos(),
                A.1 + 9.5 * 112.0_f64.to_radians().sin(),
            ),
            "r 9.5 mm, 112.5°",
        ),
    ];
    for (mode, ctrl, b, want) in cases {
        let mut s = make_session(mode, 5, 0.5);
        s.pointer_hover(pt(A.0, A.1), false, ctrl);
        s.pointer_down(pt(A.0, A.1), false);
        s.pointer_hover(pt(b.0, b.1), false, ctrl);
        assert_eq!(s.live_readout().map(|r| r.text).as_deref(), Some(want));
        s.escape();
    }
}

// ---------------------------------------------------------------------
// Criterion 7: Ctrl rotate (absolute) and the typed angle
// ---------------------------------------------------------------------

fn selected_polystar_at(mode: PolyStarMode, n: u32, deg: f64) -> Session {
    let mut s = make_session(mode, n, 0.5);
    create(&mut s, pt(A.0, A.1), drag_point(deg, 25.0), false, false);
    assert_eq!(s.selected_object_count(), 1);
    s
}

#[test]
fn ac07_spec_examples_ctrl_rotate_snaps_the_shown_angle() {
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        // created at 45, raw 10 -> 60
        let mut s = selected_polystar_at(mode, 5, 45.0);
        let text = rotate_drag(&mut s, 10.0, true);
        let got = geom_angle(&prim_of(&s, 0));
        assert!(adiff(got, 60.0).abs() < 1e-6, "{mode:?} 45 + 10 -> {got}");
        assert_eq!(text.as_deref(), Some("60°"), "live readout agrees");
        // created at 78.7, raw 1 -> 75
        let mut s = selected_polystar_at(mode, 5, 78.7);
        rotate_drag(&mut s, 1.0, true);
        let got = geom_angle(&prim_of(&s, 0));
        assert!(adiff(got, 75.0).abs() < 1e-6, "{mode:?} 78.7 + 1 -> {got}");
    }
}

#[test]
fn ac07_ctrl_rotate_snaps_the_absolute_angle_for_any_start_and_sweep() {
    let mut n_checked = 0;
    for start in [
        -179.0, -123.4, -90.0, -37.0, -11.3, 0.0, 7.7, 45.0, 78.7, 130.0, 176.2,
    ] {
        for sweep in [-95.0, -37.0, -9.0, 5.0, 12.0, 33.0, 71.0, 125.0] {
            let want = nearest_stop(wrap(start + sweep));
            let Some(want) = want else { continue };
            let mut s = selected_polystar_at(PolyStarMode::Polygon, 6, start);
            rotate_drag(&mut s, sweep, true);
            let got = geom_angle(&prim_of(&s, 0));
            assert!(
                adiff(got, want).abs() < 1e-6,
                "start {start} sweep {sweep}: got {got} want {want}"
            );
            n_checked += 1;
        }
    }
    assert!(n_checked > 60);
}

#[test]
fn ac07_without_ctrl_the_rotation_is_free_and_turns_by_the_sweep() {
    for start in [-170.0, -45.0, 0.0, 10.0, 170.0] {
        for sweep in [-30.0, 17.3, 30.0, 45.0] {
            let mut s = selected_polystar_at(PolyStarMode::Star, 5, start);
            let before = geom_angle(&prim_of(&s, 0));
            rotate_drag(&mut s, sweep, false);
            let after = geom_angle(&prim_of(&s, 0));
            assert!(
                adiff(after, before + sweep).abs() < 1e-6,
                "start {start} sweep {sweep}: {before} -> {after}"
            );
            // the chip agrees with the geometry (criterion 1: same number everywhere)
            assert!(adiff(chip_angle(&mut s), after).abs() <= 0.051);
        }
    }
}

#[test]
fn ac07_live_readout_while_rotating_shows_the_real_angle() {
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 30.0);
    let text = rotate_drag(&mut s, 12.0, false).unwrap();
    assert!(adiff(parse_deg(&text), 42.0).abs() <= 0.051, "{text}");
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 170.0);
    let text = rotate_drag(&mut s, 20.0, false).unwrap();
    assert!(
        adiff(parse_deg(&text), -170.0).abs() <= 0.051,
        "wraps: {text}"
    );
}

#[test]
fn ac07_rotating_turns_the_selection_box_with_the_shape() {
    // the rotate handle (chip anchor) turns about the box centre with the shape
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 4, 0.0);
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let v0 = s.transform_entry().unwrap();
    s.cancel_transform_entry();
    rotate_drag(&mut s, 30.0, false);
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let v1 = s.transform_entry().unwrap();
    s.cancel_transform_entry();
    let a0 = (v0.handle.y - v0.center.y).atan2(v0.handle.x - v0.center.x);
    let a1 = (v1.handle.y - v1.center.y).atan2(v1.handle.x - v1.center.x);
    assert!(
        adiff(a1.to_degrees(), a0.to_degrees() + 30.0).abs() < 1e-6,
        "box handle turned by 30: {} -> {}",
        a0.to_degrees(),
        a1.to_degrees()
    );
}

#[test]
fn ac07_typed_zero_restores_first_vertex_right_for_every_n_and_star_ratio() {
    for (mode, ratio) in [
        (PolyStarMode::Polygon, 0.5),
        (PolyStarMode::Star, 0.2),
        (PolyStarMode::Star, 0.95),
    ] {
        for n in (3..=1024).step_by(7).chain([4, 5, 1023, 1024]) {
            let mut s = make_session(mode, n, ratio);
            create(&mut s, pt(A.0, A.1), drag_point(-123.4, 20.0), false, false);
            let before = change_count(&s);
            let out = type_angle(&mut s, "0");
            assert_eq!(out, EntryOutcome::Committed, "n={n}");
            assert_eq!(change_count(&s), before + 1, "one commit");
            let p = prim_of(&s, 0);
            let c = frame_of(&p).center;
            let v = first_vertex(&p);
            assert!(
                (v.x - (c.x + frame_of(&p).radius.as_mm())).abs() < 1e-7
                    && (v.y - c.y).abs() < 1e-7,
                "{mode:?} n={n}: first vertex {v:?} c {c:?}"
            );
            assert!(chip_angle(&mut s).abs() < 1e-9);
        }
    }
}

#[test]
fn ac07_typed_angle_becomes_the_shown_angle_for_many_values() {
    for typed in [
        "0", "45", "-45", "90", "-90", "179", "-179", "180", "22.5", "-0.1", "0.1", "33.3", "-135",
        "100", "-170.5",
    ] {
        let mut s = selected_polystar_at(PolyStarMode::Star, 7, 61.0);
        let out = type_angle(&mut s, typed);
        assert!(
            matches!(out, EntryOutcome::Committed | EntryOutcome::Unchanged),
            "{typed}: {out:?}"
        );
        let want: f64 = typed.parse().unwrap();
        let got = geom_angle(&prim_of(&s, 0));
        assert!(
            adiff(got, want).abs() < 1e-6,
            "typed {typed}: geometry {got}"
        );
        assert!(
            adiff(chip_angle(&mut s), want).abs() <= 0.051,
            "typed {typed}"
        );
    }
}

#[test]
fn ac07_typing_the_current_angle_writes_nothing() {
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 45.0);
    let before = change_count(&s);
    let shown = {
        assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
        let t = s.transform_entry().unwrap().fields[0].prefill.clone();
        s.cancel_transform_entry();
        t
    };
    assert_eq!(type_angle(&mut s, &shown), EntryOutcome::Unchanged);
    assert_eq!(change_count(&s), before);
}

#[test]
fn ac07_typed_angle_hostile_inputs_are_refused_without_writing() {
    for bad in [
        "", "abc", "NaN", "inf", "-inf", "1e999", "--5", "5 5", "°", "1,2.3,4",
    ] {
        let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 45.0);
        let before = change_count(&s);
        let o = type_angle(&mut s, bad);
        assert!(
            matches!(o, EntryOutcome::Invalid { .. } | EntryOutcome::Unchanged),
            "{bad:?}: {o:?}"
        );
        assert_eq!(change_count(&s), before, "{bad:?}");
        let g = geom_angle(&prim_of(&s, 0));
        assert!(adiff(g, 45.0).abs() < 1e-6, "{bad:?}: angle moved to {g}");
        s.cancel_transform_entry();
    }
}

#[test]
fn ac07_large_typed_angles_wrap() {
    for (typed, want) in [("360", 0.0), ("450", 90.0), ("-270", 90.0), ("540", 180.0)] {
        let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 45.0);
        let o = type_angle(&mut s, typed);
        // either accepted (and wrapped) or refused cleanly; never garbage
        match o {
            EntryOutcome::Committed | EntryOutcome::Unchanged => {
                let g = geom_angle(&prim_of(&s, 0));
                assert!(
                    adiff(g, want).abs() < 1e-6,
                    "typed {typed}: geometry {g}, want {want}"
                );
                assert!(chip_angle(&mut s).abs() <= 180.0 + 1e-9);
            }
            EntryOutcome::Invalid { .. } => s.cancel_transform_entry(),
        }
    }
}

#[test]
fn ac07_edge_on_axis_rule_for_every_n() {
    let ns: Vec<u32> = (3..=64).chain([100, 128, 255, 256, 1000, 1024]).collect();
    for n in ns {
        let typed = if n % 4 == 0 {
            180.0 / f64::from(n)
        } else {
            0.0
        };
        let mut s = selected_polystar_at(PolyStarMode::Polygon, n, -77.0);
        type_angle(&mut s, &format!("{typed}"));
        let p = prim_of(&s, 0);
        let pts = outline_of_rotated(&p.shape, p.rotation);
        let r = frame_of(&p).radius.as_mm();
        let tol = 1e-6 * r;
        let edge_on_axis = |pts: &[curvyo_document_core::OutlineAnchor]| {
            (0..pts.len()).any(|i| {
                let (a, b) = (pts[i].point, pts[(i + 1) % pts.len()].point);
                (a.x - b.x).abs() < tol || (a.y - b.y).abs() < tol
            })
        };
        assert!(
            edge_on_axis(&pts),
            "n={n} at {typed}: an edge lies on an axis"
        );
        if n % 4 == 0 {
            // at 0 degrees a multiple of 4 has a vertex on each axis, no edge
            let mut s = selected_polystar_at(PolyStarMode::Polygon, n, -77.0);
            type_angle(&mut s, "0");
            let p = prim_of(&s, 0);
            let pts = outline_of_rotated(&p.shape, p.rotation);
            assert!(
                !edge_on_axis(&pts) || n >= 256,
                "n={n} at 0 must not have an axis edge (n is a multiple of 4)"
            );
        }
    }
}

#[test]
fn ac07_named_polygon_examples() {
    // hexagon at 0: horizontal edge at the top and the bottom
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 6, 17.0);
    type_angle(&mut s, "0");
    let p = prim_of(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    let ys: Vec<f64> = pts.iter().map(|a| a.point.y).collect();
    let top = ys.iter().copied().fold(f64::MAX, f64::min);
    let bot = ys.iter().copied().fold(f64::MIN, f64::max);
    assert_eq!(ys.iter().filter(|y| (**y - top).abs() < 1e-7).count(), 2);
    assert_eq!(ys.iter().filter(|y| (**y - bot).abs() < 1e-7).count(), 2);
    // pentagon at 0: a vertical edge on its left
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 17.0);
    type_angle(&mut s, "0");
    let p = prim_of(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    let xs: Vec<f64> = pts.iter().map(|a| a.point.x).collect();
    let left = xs.iter().copied().fold(f64::MAX, f64::min);
    assert_eq!(xs.iter().filter(|x| (**x - left).abs() < 1e-7).count(), 2);
    // square at 45: four axis-parallel edges
    let mut s = selected_polystar_at(PolyStarMode::Polygon, 4, 17.0);
    type_angle(&mut s, "45");
    let p = prim_of(&s, 0);
    let pts = outline_of_rotated(&p.shape, p.rotation);
    for i in 0..4 {
        let (a, b) = (pts[i].point, pts[(i + 1) % 4].point);
        assert!((a.x - b.x).abs() < 1e-7 || (a.y - b.y).abs() < 1e-7);
    }
}

#[test]
fn ac07_double_click_on_the_rotate_handle_opens_the_same_entry_as_r() {
    let mut s = selected_polystar_at(PolyStarMode::Star, 5, -15.0);
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let via_key = s.transform_entry().unwrap();
    s.cancel_transform_entry();
    let h = via_key.handle;
    s.pointer_hover(h, false, false);
    s.pointer_down(h, false);
    s.pointer_up(h, false, false);
    s.pointer_hover(h, false, false);
    s.double_click(h, false, false);
    let via_dbl = s
        .transform_entry()
        .expect("double click opens the angle chip");
    assert_eq!(via_dbl.kind, "angle");
    assert_eq!(
        via_dbl.fields, via_key.fields,
        "same prefill (criterion 1: same number)"
    );
    assert!(
        (parse_deg(&via_dbl.fields[0].prefill) + 15.0).abs() <= 0.051,
        "star shown at -15"
    );
}

// ---------------------------------------------------------------------
// Criterion 8: rectangle and ellipse unchanged
// ---------------------------------------------------------------------

#[test]
fn ac08_rectangle_and_ellipse_creation_unchanged() {
    // Rectangle: Ctrl = 1:1 (Shift = from the centre belongs to
    // `shape-creation-from-center`, which is not on main yet: not checked)
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    create(&mut s, pt(10.0, 10.0), pt(40.0, 20.0), false, true);
    let p = prim_of(&s, 0);
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(
        (bounds.width.as_mm() - bounds.height.as_mm()).abs() < 1e-9,
        "Ctrl is 1:1"
    );
    // Ellipse: Ctrl circle, Shift from centre
    let mut s = Session::new(1);
    s.set_tool(Tool::Ellipse);
    create(&mut s, pt(10.0, 10.0), pt(40.0, 20.0), false, true);
    let Shape::Ellipse { frame } = prim_of(&s, 0).shape else {
        panic!()
    };
    assert!((frame.rx.as_mm() - frame.ry.as_mm()).abs() < 1e-9);
}

#[test]
fn ac08_rectangle_ctrl_rotate_keeps_the_relative_rule() {
    // rect at 10 degrees, sweep 19 with Ctrl -> delta snaps to 22.5 -> 32.5
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    let o = d.object(id).unwrap();
    let c = pt(20.0, 10.0);
    d.rotate_object(&o.rotated(c, Angle::from_radians(10.0_f64.to_radians())))
        .unwrap();
    let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    // select by the outline, then rotate through the chip's handle
    let p = prim_of(&s, 0);
    let first = outline_of_rotated(&p.shape, p.rotation)[0].point;
    let second = outline_of_rotated(&p.shape, p.rotation)[1].point;
    let mid = pt(
        f64::midpoint(first.x, second.x),
        f64::midpoint(first.y, second.y),
    );
    s.pointer_hover(mid, false, false);
    s.pointer_down(mid, false);
    s.pointer_up(mid, false, false);
    assert_eq!(s.selected_object_count(), 1);
    rotate_drag(&mut s, 19.0, true);
    let rot = prim_of(&s, 0).rotation.as_radians().to_degrees();
    assert!(
        adiff(rot, 32.5).abs() < 1e-6,
        "rect keeps the relative snap: {rot}"
    );
}

// ---------------------------------------------------------------------
// Criterion 2: old files open unchanged
// ---------------------------------------------------------------------

fn fixture(name: &str) -> Vec<u8> {
    let p = format!(
        "{}/../curvyo-document-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn polystar_prims(d: &Document) -> Vec<PrimitiveSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id).unwrap() {
            ObjectSnapshot::Primitive(p)
                if matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }) =>
            {
                Some(p)
            }
            _ => None,
        })
        .collect()
}

fn select_by_first_vertex(s: &mut Session, index_in_doc: usize) {
    let p = prim_of(s, index_in_doc);
    let v = first_vertex(&p);
    s.pointer_hover(v, false, false);
    s.pointer_down(v, false);
    s.pointer_up(v, false, false);
}

/// An old-build style document: a polygon and a star whose angle sits in the
/// frame and whose `rotation` register is also non-zero.
fn old_style_doc() -> Document {
    let d = Document::new(1);
    let frame = |cx: f64, cy: f64, deg: f64| StarFrame {
        center: pt(cx, cy),
        radius: Length::from_mm(20.0),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let poly = d.create_polygon(frame(50.0, 50.0, 78.7), PointCount::new(5).unwrap());
    let star = d.create_star(
        frame(200.0, 50.0, -200.0),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let odd = d.create_star(
        frame(350.0, 50.0, 3.0 * 180.0 + 20.0),
        PointCount::new(6).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    for (id, deg) in [(poly, 30.0), (star, -50.0), (odd, 190.0)] {
        let o = d.object(id).unwrap();
        let c = match &o {
            ObjectSnapshot::Primitive(p) => frame_of(p).center,
            ObjectSnapshot::Path(_) => unreachable!(),
        };
        d.rotate_object(&o.rotated(c, Angle::from_radians(f64::to_radians(deg))))
            .unwrap();
    }
    d
}

#[test]
fn ac02_old_files_keep_their_look_and_show_frame_angle_plus_rotation() {
    let mut docs: Vec<(String, Vec<u8>)> =
        vec![("generated".into(), pack(&old_style_doc(), "0.1.0").unwrap())];
    for name in [
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "valid.curvyo",
        "paths_v2.curvyo",
        "format_version_1.curvyo",
    ] {
        docs.push((name.into(), fixture(name)));
    }
    let mut polystars_seen = 0;
    for (name, bytes) in docs {
        let reference = unpack(7, &bytes).unwrap();
        let mut s = Session::open(3, &bytes).unwrap();
        s.set_tool(Tool::Select);
        let before_changes = change_count(&s);
        // looks: every object's outline is identical to the file's own
        let opened = doc_of(&s);
        for id in reference.object_ids() {
            assert_eq!(
                opened.object(id),
                reference.object(id),
                "{name}: object unchanged"
            );
        }
        for (i, id) in reference.object_ids().into_iter().enumerate() {
            let Some(ObjectSnapshot::Primitive(p)) = reference.object(id) else {
                continue;
            };
            if !matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }) {
                continue;
            }
            polystars_seen += 1;
            let f = frame_of(&p);
            let want =
                wrap(f.angle.as_radians().to_degrees() + p.rotation.as_radians().to_degrees());
            // geometry says the same thing as frame.angle + rotation
            assert!(
                adiff(geom_angle(&p), want).abs() < 1e-6,
                "{name}[{i}] geometry"
            );
            select_by_first_vertex(&mut s, i);
            if s.selected_object_count() != 1 {
                continue; // overlapping neighbour won the press; checked below by chip only when selected
            }
            let got = chip_angle(&mut s);
            assert!(
                adiff(got, want).abs() <= 0.051,
                "{name}[{i}]: shown {got}, frame {} + rotation {} = {want}",
                f.angle.as_radians().to_degrees(),
                p.rotation.as_radians().to_degrees()
            );
        }
        // selecting, reading the angle and closing the chip wrote nothing
        assert_eq!(
            change_count(&s),
            before_changes,
            "{name}: no write by merely looking"
        );
        // saving reproduces every object
        let saved = unpack(8, &s.pack("0.1.0").unwrap()).unwrap();
        for id in reference.object_ids() {
            assert_eq!(saved.object(id), reference.object(id), "{name}: round trip");
        }
    }
    assert!(
        polystars_seen >= 3,
        "fixtures held polygons or stars: {polystars_seen}"
    );
}

#[test]
fn ac02_the_generated_old_style_doc_has_the_expected_angles() {
    // direct arithmetic, independent of the fixture helper: frame 78.7 + 30,
    // -200 + -50 = -250 -> 110, 560 + 190 = 750 -> 30
    let d = old_style_doc();
    let mut s = Session::open(3, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    for (i, want) in [(0, 108.7), (1, 110.0), (2, 30.0)] {
        let ids = d.object_ids();
        let p = d.primitive(ids[i]).unwrap();
        let v = first_vertex(&p);
        s.pointer_hover(v, false, false);
        s.pointer_down(v, false);
        s.pointer_up(v, false, false);
        assert_eq!(s.selected_object_count(), 1);
        let got = chip_angle(&mut s);
        assert!(
            adiff(got, want).abs() <= 0.051,
            "object {i}: shown {got}, want {want}"
        );
        // deselect
        s.pointer_hover(pt(900.0, 900.0), false, false);
        s.pointer_down(pt(900.0, 900.0), false);
        s.pointer_up(pt(900.0, 900.0), false, false);
    }
    // the shape set is the one that was authored
    assert_eq!(polystar_prims(&doc_of(&s)).len(), 3);
}

#[test]
fn ac02_new_files_use_the_same_stored_representation_as_before() {
    // (Should) a build before this change opens a new file with the same
    // shapes: a created shape keeps the rotation register at 0 and carries
    // its angle in the frame, byte-compatible with what main writes.
    let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
    create(&mut s, pt(A.0, A.1), drag_point(-37.0, 20.0), false, false);
    let p = prim_of(&s, 0);
    assert_eq!(
        p.rotation.as_radians(),
        0.0,
        "rotation register stays 0 for a created shape"
    );
    assert!((frame_of(&p).angle.as_radians().to_degrees() + 37.0).abs() < 1e-6);
    let bytes = s.pack("0.1.0").unwrap();
    let manifest = {
        let mut z = zip_reader(&bytes);
        z.0.remove("manifest.json").unwrap()
    };
    let m: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(m["format_version"], 5, "format_version unchanged");
}

fn zip_reader(bytes: &[u8]) -> (std::collections::HashMap<String, Vec<u8>>,) {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut map = std::collections::HashMap::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        map.insert(f.name().to_string(), buf);
    }
    (map,)
}

#[test]
fn ac01_rotation_by_delta_and_wrap_property_over_a_grid() {
    let _ = PI;
    for n in [3, 4, 5, 8] {
        for start in [-179.0, -90.0, 0.0, 135.0, 179.0] {
            for typed in [-180.0, -135.0, -1.0, 0.0, 1.0, 90.0, 179.0, 180.0] {
                let mut s = selected_polystar_at(PolyStarMode::Polygon, n, start);
                let o = type_angle(&mut s, &format!("{typed}"));
                assert!(matches!(
                    o,
                    EntryOutcome::Committed | EntryOutcome::Unchanged
                ));
                assert!(adiff(geom_angle(&prim_of(&s, 0)), typed).abs() < 1e-6);
            }
        }
    }
}

// ---------------------------------------------------------------------
// White-box follow-ups after reading the diff
// ---------------------------------------------------------------------

/// Rotates with Shift held (pivot = the opposite corner of the oriented box)
/// by dragging the top-right corner rotate handle.
fn rotate_drag_shift(s: &mut Session, deg: f64, ctrl: bool) {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    let (h, c) = (v.handle, v.center);
    s.cancel_transform_entry();
    let k = s.view().scale();
    let dist = (h.x - c.x).hypot(h.y - c.y);
    let corner_dist = dist - 32.0 / k;
    let ux = (h.x - c.x) / dist;
    let uy = (h.y - c.y) / dist;
    let pivot = pt(c.x - ux * corner_dist, c.y - uy * corner_dist);
    s.pointer_hover(h, true, ctrl);
    s.pointer_down(h, true);
    let detour = swept_to(h, pivot, if deg >= 0.0 { 40.0 } else { -40.0 });
    s.pointer_hover(detour, true, ctrl);
    let to = swept_to(h, pivot, deg);
    s.pointer_hover(to, true, ctrl);
    s.pointer_up(to, true, ctrl);
}

#[test]
fn ac07_shift_pivot_rotation_still_turns_the_shown_angle_by_the_sweep_and_snaps_absolute() {
    for start in [-100.0, -11.0, 0.0, 78.7, 150.0] {
        for sweep in [-40.0, 13.0, 37.0] {
            let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, start);
            rotate_drag_shift(&mut s, sweep, false);
            let got = geom_angle(&prim_of(&s, 0));
            assert!(
                adiff(got, start + sweep).abs() < 1e-6,
                "shift pivot: start {start} sweep {sweep}: {got}"
            );
            assert_eq!(n_objects(&s), 1);
            // with Ctrl: the shown angle lands on a stop
            let mut s = selected_polystar_at(PolyStarMode::Star, 5, start);
            rotate_drag_shift(&mut s, sweep, true);
            let got = geom_angle(&prim_of(&s, 0));
            if let Some(want) = nearest_stop(wrap(start + sweep)) {
                assert!(
                    adiff(got, want).abs() < 1e-6,
                    "shift+ctrl: start {start} sweep {sweep}: {got}, want {want}"
                );
            }
        }
    }
}

#[test]
fn ac07_old_style_objects_with_frame_angle_and_rotation_snap_their_sum() {
    // frame 78.7 + rotation 30 = 108.7 -> sweep 3 -> 111.7 -> stop 112.5
    let d = old_style_doc();
    for (i, sweep, want) in [(0usize, 3.0, 112.5), (1, 4.0, 112.5), (2, -4.0, 22.5)] {
        let mut s = Session::open(3, &pack(&d, "0.1.0").unwrap()).unwrap();
        s.set_tool(Tool::Select);
        let p = prim_of(&s, i);
        let v = first_vertex(&p);
        s.pointer_hover(v, false, false);
        s.pointer_down(v, false);
        s.pointer_up(v, false, false);
        assert_eq!(s.selected_object_count(), 1);
        rotate_drag(&mut s, sweep, true);
        let got = geom_angle(&prim_of(&s, i));
        assert!(
            adiff(got, want).abs() < 1e-6,
            "object {i}: {got}, want {want}"
        );
    }
}

#[test]
fn ac01_pressing_a_typed_angle_with_leading_and_trailing_spaces_and_plus_sign() {
    for typed in [" 30", "30 ", "+30", "30.0", "3e1", "30\u{b0}"] {
        let mut s = selected_polystar_at(PolyStarMode::Polygon, 5, 45.0);
        let o = type_angle(&mut s, typed);
        if o == EntryOutcome::Committed {
            assert!(
                adiff(geom_angle(&prim_of(&s, 0)), 30.0).abs() < 1e-6,
                "{typed:?}"
            );
        } else {
            eprintln!("note: {typed:?} refused: {o:?}");
            s.cancel_transform_entry();
        }
    }
}

#[test]
fn ac03_tiny_and_huge_create_drags_keep_the_angle_rule() {
    for r in [0.001, 0.05, 1.0, 1000.0, 100_000.0] {
        for deg in [-90.0_f64, -15.0, 33.0, 135.0] {
            let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
            create(
                &mut s,
                pt(0.0, 0.0),
                pt(r * deg.to_radians().cos(), r * deg.to_radians().sin()),
                false,
                false,
            );
            if n_objects(&s) == 1 {
                let got = geom_angle(&prim_of(&s, 0));
                assert!(adiff(got, deg).abs() < 1e-4, "r {r} deg {deg}: {got}");
            }
        }
    }
    // a zero-length drag creates nothing, also with Ctrl
    for ctrl in [false, true] {
        let mut s = make_session(PolyStarMode::Polygon, 5, 0.5);
        create(&mut s, pt(5.0, 5.0), pt(5.0, 5.0), false, ctrl);
        assert_eq!(n_objects(&s), 0);
    }
}

#[test]
fn ac03_every_point_count_3_to_1024_orients_the_first_vertex_at_the_pointer_and_shows_it() {
    for n in 3..=1024u32 {
        for (mode, deg) in [(PolyStarMode::Polygon, -123.4), (PolyStarMode::Star, 37.0)] {
            if mode == PolyStarMode::Star && n % 7 != 0 && n >= 30 {
                continue;
            }
            let mut s = make_session(mode, n, 0.37);
            let b = drag_point(deg, 14.0);
            create(&mut s, pt(A.0, A.1), b, false, false);
            let p = prim_of(&s, 0);
            let v = first_vertex(&p);
            assert!(
                (v.x - b.x).abs() < 1e-7 && (v.y - b.y).abs() < 1e-7,
                "{mode:?} n={n}: first vertex {v:?}, pointer {b:?}"
            );
            if n % 61 == 0 || !(12..=1020).contains(&n) {
                assert!(
                    adiff(chip_angle(&mut s), deg).abs() <= 0.051,
                    "{mode:?} n={n}"
                );
            }
        }
    }
}
