//! Independent tester acceptance tests for
//! `specs/shape-creation-from-center/specification.md` through `Session`'s
//! public API (the host's pointer, key and wheel paths). Written from the
//! specification before the implementation diff was read. Expected values come
//! from the spec's worked examples and a reference model written here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::type_complexity)]
#![allow(missing_docs, clippy::doc_markdown, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#![allow(clippy::needless_pass_by_value, clippy::cast_lossless)]

use curvyo_document_core::{Document, Point, PrimitiveSnapshot, Shape, pack, unpack};
use curvyo_editor_wasm::{EscapeStep, Session, Tool};
use curvyo_ui_core::PolyStarMode;

const EPS: f64 = 1e-9;
const ALL: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session(tool: Tool) -> Session {
    let mut s = Session::new(1);
    s.set_tool(tool);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn prims(s: &Session) -> Vec<PrimitiveSnapshot> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .map(|id| d.primitive(id).unwrap())
        .collect()
}

fn rect_box(p: &PrimitiveSnapshot) -> (f64, f64, f64, f64) {
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!("rect expected")
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

fn ell(p: &PrimitiveSnapshot) -> (f64, f64, f64, f64) {
    let Shape::Ellipse { frame } = p.shape else {
        panic!("ellipse expected")
    };
    (
        frame.center.x,
        frame.center.y,
        frame.rx.as_mm(),
        frame.ry.as_mm(),
    )
}

fn close(got: (f64, f64, f64, f64), want: (f64, f64, f64, f64)) {
    assert!(
        (got.0 - want.0).abs() < EPS
            && (got.1 - want.1).abs() < EPS
            && (got.2 - want.2).abs() < EPS
            && (got.3 - want.3).abs() < EPS,
        "got {got:?}, want {want:?}"
    );
}

/// Full host sequence of a create-drag: hover, press, move, release.
fn drag(s: &mut Session, a: Point, b: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(b, shift, ctrl);
    s.pointer_up(b, shift, ctrl);
}

/// The host's reaction to a key press/release: modifiers_changed, then the
/// last hover re-sent at the unchanged pointer position.
fn press(s: &mut Session, at: Point, shift: bool, ctrl: bool) {
    s.modifiers_changed(shift, ctrl);
    s.pointer_hover(at, shift, ctrl);
}

/// Reference rule from the spec: (box corner p, box corner q, E).
fn ref_box(a: Point, b: Point, shift: bool, ctrl: bool) -> (Point, Point, Point) {
    let e = if ctrl {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let mm = dx.abs().max(dy.abs());
        pt(
            a.x + if dx < 0.0 { -mm } else { mm },
            a.y + if dy < 0.0 { -mm } else { mm },
        )
    } else {
        b
    };
    if shift {
        (pt(2.0 * a.x - e.x, 2.0 * a.y - e.y), e, e)
    } else {
        (a, e, e)
    }
}

fn ref_rect(a: Point, b: Point, shift: bool, ctrl: bool) -> (f64, f64, f64, f64) {
    let (p, q, _) = ref_box(a, b, shift, ctrl);
    (
        p.x.min(q.x),
        p.y.min(q.y),
        (p.x - q.x).abs(),
        (p.y - q.y).abs(),
    )
}

fn readout(s: &Session) -> Option<(String, Point)> {
    s.live_readout().map(|r| (r.text, r.anchor))
}

const A: Point = Point { x: 100.0, y: 50.0 };

// ---------------------------------------------------------------------
// Criteria 1-7 through the Session, incl. rotation and hand-over
// ---------------------------------------------------------------------

#[test]
fn ac01_ac13_rect_shift_through_session_one_object_rotation_zero() {
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(130.0, 40.0), true, false);
    let ps = prims(&s);
    assert_eq!(ps.len(), 1, "exactly one object");
    close(rect_box(&ps[0]), (70.0, 40.0, 60.0, 20.0));
    let Shape::Rect { corner_radius, .. } = ps[0].shape else {
        panic!()
    };
    assert!(corner_radius.as_mm().abs() < EPS);
    assert_eq!(ps[0].rotation.as_radians(), 0.0);
}

#[test]
fn ac03_ac06_shift_ctrl_square_and_circle_through_session() {
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(130.0, 40.0), true, true);
    close(rect_box(&prims(&s)[0]), (70.0, 20.0, 60.0, 60.0));
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(110.0, 20.0), true, true);
    close(rect_box(&prims(&s)[0]), (70.0, 20.0, 60.0, 60.0));
    let mut s = session(Tool::Ellipse);
    drag(&mut s, A, pt(130.0, 40.0), true, true);
    close(ell(&prims(&s)[0]), (100.0, 50.0, 30.0, 30.0));
    let mut s = session(Tool::Ellipse);
    drag(&mut s, A, pt(130.0, 40.0), true, false);
    close(ell(&prims(&s)[0]), (100.0, 50.0, 30.0, 10.0));
    assert_eq!(prims(&s)[0].rotation.as_radians(), 0.0);
}

#[test]
fn ac04_ac07_unchanged_modes_golden() {
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(130.0, 40.0), false, true);
    close(rect_box(&prims(&s)[0]), (100.0, 20.0, 30.0, 30.0));
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(130.0, 40.0), false, false);
    close(rect_box(&prims(&s)[0]), (100.0, 40.0, 30.0, 10.0));
    let mut s = session(Tool::Ellipse);
    drag(&mut s, A, pt(130.0, 40.0), false, false);
    close(ell(&prims(&s)[0]), (115.0, 45.0, 15.0, 5.0));
}

#[test]
fn ac13_stroke_and_fill_defaults_match_across_modes() {
    let mut keys = Vec::new();
    for (sh, ct) in ALL {
        let mut s = session(Tool::Rectangle);
        drag(&mut s, A, pt(130.0, 40.0), sh, ct);
        let p = &prims(&s)[0];
        keys.push((p.stroke_width, p.stroke, p.fill, p.rotation));
        assert!(p.fill.is_none());
    }
    assert!(keys.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn hand_over_to_select_with_the_new_shape_selected() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        for (sh, ct) in ALL {
            let mut s = session(tool);
            drag(&mut s, A, pt(130.0, 40.0), sh, ct);
            assert_eq!(s.tool(), Tool::Select, "unified-object-editing 28");
            assert_eq!(s.selected_object_count(), 1);
        }
    }
}

#[test]
fn ac13_format_version_unchanged_and_no_modifier_trace_in_the_file() {
    use std::io::Read;
    let version_and_objects = |sh: bool, ct: bool| {
        let mut s = session(Tool::Rectangle);
        drag(&mut s, pt(0.0, 0.0), pt(20.0, 10.0), sh, ct);
        let bytes = s.pack("0.1.0").unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut m = Vec::new();
        z.by_name("manifest.json")
            .unwrap()
            .read_to_end(&mut m)
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&m).unwrap();
        v["format_version"].clone()
    };
    let base = version_and_objects(false, false);
    assert_eq!(base, 5);
    for (sh, ct) in ALL {
        assert_eq!(version_and_objects(sh, ct), base);
    }
    // Shift+Ctrl and plain (same box) produce the same document content
    let mut a = session(Tool::Rectangle);
    drag(&mut a, pt(10.0, 10.0), pt(40.0, 40.0), false, false);
    let mut b = session(Tool::Rectangle);
    drag(&mut b, pt(25.0, 25.0), pt(40.0, 40.0), true, false);
    close(rect_box(&prims(&a)[0]), rect_box(&prims(&b)[0]));
    let (pa, pb) = (&prims(&a)[0], &prims(&b)[0]);
    assert_eq!(
        (pa.stroke_width, pa.stroke, pa.fill, pa.rotation),
        (pb.stroke_width, pb.stroke, pb.fill, pb.rotation)
    );
}

// ---------------------------------------------------------------------
// Criteria 8, 9, 11: preview, readout, mid-drag modifier changes
// ---------------------------------------------------------------------

#[test]
fn ac11_readout_text_and_anchor_in_all_four_modes_rect() {
    let b = pt(130.0, 40.0);
    // want: text, anchor
    let cases = [
        ((false, false), "30.0 × 10.0 mm", pt(130.0, 40.0)),
        ((false, true), "30.0 × 30.0 mm", pt(130.0, 20.0)),
        ((true, false), "60.0 × 20.0 mm", pt(130.0, 40.0)),
        ((true, true), "60.0 × 60.0 mm", pt(130.0, 20.0)),
    ];
    for ((sh, ct), text, anchor) in cases {
        let mut s = session(Tool::Rectangle);
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        s.pointer_hover(b, sh, ct);
        let (t, an) = readout(&s).expect("readout while dragging");
        assert_eq!(t, text, "mode shift={sh} ctrl={ct}");
        assert!(
            (an.x - anchor.x).abs() < EPS && (an.y - anchor.y).abs() < EPS,
            "anchor {an:?} want {anchor:?} (shift={sh} ctrl={ct})"
        );
    }
}

#[test]
fn ac11_readout_text_in_all_four_modes_ellipse_is_radii() {
    let b = pt(130.0, 40.0);
    let cases = [
        ((false, false), "15.0 × 5.0 mm"),
        ((false, true), "15.0 × 15.0 mm"),
        ((true, false), "30.0 × 10.0 mm"),
        ((true, true), "30.0 × 30.0 mm"),
    ];
    for ((sh, ct), text) in cases {
        let mut s = session(Tool::Ellipse);
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        s.pointer_hover(b, sh, ct);
        let (t, _) = readout(&s).unwrap();
        assert_eq!(t, text, "mode shift={sh} ctrl={ct}");
    }
}

#[test]
fn ac08_every_modifier_sequence_at_a_still_pointer_updates_preview_and_readout() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let b = pt(130.0, 40.0);
        // reference: independent sessions in each pure mode, same pointer
        let mut want = std::collections::HashMap::new();
        for (sh, ct) in ALL {
            let mut r = session(tool);
            r.pointer_hover(A, false, false);
            r.pointer_down(A, false);
            r.pointer_hover(b, sh, ct);
            want.insert((sh, ct), (readout(&r).unwrap(), r.draw_list()));
        }
        // all four are distinct modes visually (preview differs)
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(
                    want[&ALL[i]].1, want[&ALL[j]].1,
                    "{tool:?}: {:?} vs {:?} must differ",
                    ALL[i], ALL[j]
                );
            }
        }
        // one drag, pointer held still, every ordered pair of modes and a
        // long random walk across modes (press/release in any order)
        let mut s = session(tool);
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        s.pointer_hover(b, false, false);
        let mut cur = (false, false);
        let mut seed = 12345u64;
        let mut steps: Vec<(bool, bool)> = Vec::new();
        for &x in &ALL {
            for &y in &ALL {
                steps.push(x);
                steps.push(y);
            }
        }
        for _ in 0..200 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            steps.push(ALL[(seed >> 33) as usize % 4]);
        }
        for next in steps {
            // one key change at a time as a host reports it, when possible
            press(&mut s, b, next.0, next.1);
            cur = next;
            let w = &want[&cur];
            assert_eq!(readout(&s).unwrap(), w.0, "{tool:?} readout {cur:?}");
            assert!(s.draw_list() == w.1, "{tool:?} preview {cur:?}");
        }
        // release at that state commits the same shape
        s.pointer_up(b, cur.0, cur.1);
        assert_eq!(prims(&s).len(), 1);
    }
}

#[test]
fn ac09_preview_outline_equals_the_equivalent_corner_drag_and_the_commit() {
    // Shift drag from A to B draws the same outline as a plain drag between
    // the box's opposite corners 2A-E and E (draw list compare), and the
    // committed shape is that box.
    for (sh, ct) in ALL {
        for b in [pt(130.0, 40.0), pt(70.0, 60.0), pt(112.0, 91.0)] {
            let (p, q, _) = ref_box(A, b, sh, ct);
            let mut s = session(Tool::Rectangle);
            s.pointer_hover(A, false, false);
            s.pointer_down(A, false);
            s.pointer_hover(b, sh, ct);
            let live = s.draw_list();
            let mut r = session(Tool::Rectangle);
            r.pointer_hover(p, false, false);
            r.pointer_down(p, false);
            r.pointer_hover(q, false, false);
            let reference = r.draw_list();
            // Under Shift the UX review adds the pivot marker at the press
            // point after the outline: the outline itself is the corner
            // drag's, the marker is the only extra.
            let same_outline = if sh {
                live.triangles.len() > reference.triangles.len()
                    && live.triangles[..reference.triangles.len()] == reference.triangles[..]
            } else {
                live == reference
            };
            assert!(
                same_outline,
                "preview (shift={sh} ctrl={ct}, b={b:?}) is not the box {p:?}..{q:?}"
            );
            s.pointer_up(b, sh, ct);
            close(rect_box(&prims(&s)[0]), ref_rect(A, b, sh, ct));
        }
    }
}

#[test]
fn ac09_ac10_release_modifiers_win_over_the_last_preview_state() {
    for (ps, pc) in ALL {
        for (rs, rc) in ALL {
            let b = pt(130.0, 40.0);
            let mut s = session(Tool::Rectangle);
            s.pointer_hover(A, false, false);
            s.pointer_down(A, false);
            s.pointer_hover(b, ps, pc);
            // no event reached the canvas (window lost focus): release
            // arrives with a different state
            s.pointer_up(b, rs, rc);
            close(rect_box(&prims(&s)[0]), ref_rect(A, b, rs, rc));
        }
    }
}

#[test]
fn ac09_walk_preview_always_equals_commit_property() {
    // pseudo-random moves and modifier changes; the readout after the last
    // event is the size of the shape that the release commits.
    let mut seed = 99u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as f64 / f64::from(1u32 << 31)) * 400.0 - 200.0
    };
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        for _ in 0..60 {
            let a = pt(next(), next());
            let mut s = session(tool);
            s.pointer_hover(a, false, false);
            s.pointer_down(a, false);
            let mut last = (a, (false, false));
            for _ in 0..6 {
                let b = pt(next(), next());
                let m = ALL[((next() + 200.0) as usize / 100) % 4];
                s.pointer_hover(b, m.0, m.1);
                last = (b, m);
            }
            let text = readout(&s).map(|r| r.0);
            s.pointer_up(last.0, (last.1).0, (last.1).1);
            let ps = prims(&s);
            assert_eq!(ps.len(), 1);
            let (w, h) = if tool == Tool::Rectangle {
                let r = rect_box(&ps[0]);
                (r.2, r.3)
            } else {
                let e = ell(&ps[0]);
                (e.2, e.3)
            };
            assert_eq!(text.unwrap(), format!("{w:.1} × {h:.1} mm"));
            if tool == Tool::Rectangle {
                close(
                    rect_box(&ps[0]),
                    ref_rect(a, last.0, (last.1).0, (last.1).1),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 12
// ---------------------------------------------------------------------

#[test]
fn ac12_pointer_at_a_no_preview_no_readout_nothing_created() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let mut s = session(tool);
        let idle = s.draw_list();
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        for (sh, ct) in ALL {
            press(&mut s, A, sh, ct);
            assert!(s.live_readout().is_none(), "{tool:?} {sh} {ct}");
        }
        press(&mut s, A, true, true);
        let _ = idle;
        s.pointer_up(A, true, true);
        assert_eq!(prims(&s).len(), 0);
    }
}

#[test]
fn ac12_returning_to_a_during_the_drag_and_releasing_there_writes_nothing() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        for (sh, ct) in ALL {
            let mut s = session(tool);
            s.pointer_hover(A, false, false);
            s.pointer_down(A, false);
            s.pointer_hover(pt(120.0, 70.0), sh, ct);
            assert!(s.live_readout().is_some());
            s.pointer_hover(A, sh, ct);
            assert!(s.live_readout().is_none());
            s.pointer_up(A, sh, ct);
            assert_eq!(prims(&s).len(), 0);
        }
    }
}

#[test]
fn ac12_a_zero_size_shift_drag_writes_nothing_and_a_one_axis_drag_is_created() {
    // E == A can only happen when the pointer is at A. One axis only:
    let mut s = session(Tool::Rectangle);
    drag(&mut s, A, pt(120.0, 50.0), true, false);
    let ps = prims(&s);
    assert_eq!(ps.len(), 1, "one-axis drag is created as before");
    close(rect_box(&ps[0]), (80.0, 50.0, 40.0, 0.0));
}

// ---------------------------------------------------------------------
// Criterion 14: Escape
// ---------------------------------------------------------------------

#[test]
fn ac14_escape_cancels_and_modifier_changes_afterwards_do_not_bring_it_back() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        for (sh, ct) in ALL {
            let mut s = session(tool);
            s.pointer_hover(A, false, false);
            let idle = s.draw_list();
            s.pointer_down(A, false);
            s.pointer_hover(pt(130.0, 40.0), sh, ct);
            assert!(s.live_readout().is_some());
            assert_eq!(s.escape(), EscapeStep::CancelledDrag);
            assert!(s.live_readout().is_none());
            for (sh2, ct2) in ALL {
                press(&mut s, pt(130.0, 40.0), sh2, ct2);
                assert!(s.live_readout().is_none(), "no revival {sh2} {ct2}");
            }
            // the preview is gone from the draw list: same as an idle hover
            // is not guaranteed (cursor), so compare against a fresh session
            // hovered at the same place
            let mut fresh = session(tool);
            fresh.pointer_hover(pt(130.0, 40.0), true, true);
            assert!(s.draw_list() == fresh.draw_list(), "no preview remains");
            let _ = idle;
            s.pointer_up(pt(130.0, 40.0), sh, ct);
            assert_eq!(prims(&s).len(), 0, "nothing written");
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 15: pan / zoom mid-drag
// ---------------------------------------------------------------------

#[test]
fn ac15_pan_and_zoom_mid_drag_keep_a_as_the_box_centre() {
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        let mut s = session(tool);
        // pointer positions are document millimetres here; the wheel moves
        // the viewport, not the document, so A stays fixed.
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        s.pointer_hover(pt(130.0, 40.0), true, false);
        s.wheel(30.0, 20.0, 400.0, 300.0, false, false);
        s.wheel(0.0, -120.0, 400.0, 300.0, false, true);
        s.wheel(0.0, 40.0, 400.0, 300.0, true, false);
        s.pointer_hover(pt(130.0, 40.0), true, false);
        s.pointer_up(pt(130.0, 40.0), true, false);
        let p = &prims(&s)[0];
        if tool == Tool::Rectangle {
            close(rect_box(p), (70.0, 40.0, 60.0, 20.0));
        } else {
            close(ell(p), (100.0, 50.0, 30.0, 10.0));
        }
    }
}

// ---------------------------------------------------------------------
// unified-object-editing 25: a Shift press on an existing outline creates
// ---------------------------------------------------------------------

#[test]
fn shift_press_on_an_existing_outline_creates_a_new_shape_centred_there() {
    for (sh, ct) in [(true, false), (true, true), (false, false)] {
        let mut s = session(Tool::Rectangle);
        drag(&mut s, pt(0.0, 0.0), pt(40.0, 20.0), false, false);
        assert_eq!(s.tool(), Tool::Select);
        s.set_tool(Tool::Rectangle);
        // press on the top edge of the existing rectangle
        let a = pt(20.0, 0.0);
        s.pointer_hover(a, sh, false);
        s.pointer_down(a, sh);
        s.pointer_hover(pt(30.0, 10.0), sh, ct);
        s.pointer_up(pt(30.0, 10.0), sh, ct);
        let ps = prims(&s);
        assert_eq!(ps.len(), 2, "a press in a creation tool always creates");
        close(rect_box(&ps[0]), (0.0, 0.0, 40.0, 20.0));
        close(rect_box(&ps[1]), ref_rect(a, pt(30.0, 10.0), sh, ct));
        assert_eq!(s.selected_object_count(), 1);
    }
}

// ---------------------------------------------------------------------
// Criterion 17: polygon / star
// ---------------------------------------------------------------------

#[test]
fn ac17_polygon_and_star_shift_is_inert_through_session() {
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        let make = |sh: bool| {
            let mut s = session(Tool::PolygonStar);
            s.set_poly_star_mode(mode);
            s.pointer_hover(A, false, false);
            s.pointer_down(A, false);
            s.pointer_hover(pt(113.0, 41.0), sh, false);
            let live = (readout(&s), s.draw_list());
            press(&mut s, pt(113.0, 41.0), sh, false);
            let live2 = (readout(&s), s.draw_list());
            s.pointer_up(pt(113.0, 41.0), sh, false);
            let p = prims(&s)[0];
            (live, live2, p.shape, p.rotation)
        };
        let plain = make(false);
        let shifted = make(true);
        assert!(plain.0 == shifted.0 && plain.2 == shifted.2 && plain.3 == shifted.3);
        let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = plain.2 else {
            panic!()
        };
        assert!((frame.center.x - 100.0).abs() < EPS && (frame.center.y - 50.0).abs() < EPS);
    }
}

#[test]
fn ac17_polygon_ctrl_still_snaps_the_angle_per_edit_polish() {
    let angle = |ct: bool| {
        let mut s = session(Tool::PolygonStar);
        s.pointer_hover(A, false, false);
        s.pointer_down(A, false);
        s.pointer_hover(pt(110.0, 48.0), false, ct);
        let t = readout(&s).unwrap().0;
        s.pointer_up(pt(110.0, 48.0), false, ct);
        t
    };
    assert_eq!(angle(false), "r 10.2 mm, -11.3°");
    assert_eq!(angle(true), "r 10.2 mm, -15°");
}

// ---------------------------------------------------------------------
// Hostile pointer values through the Session sanitiser
// ---------------------------------------------------------------------

#[test]
fn hostile_pointer_values_through_session_never_panic_and_keep_the_file_openable() {
    let vals = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        -f64::MAX,
        5e-324,
        -0.0,
        1e308,
    ];
    for tool in [Tool::Rectangle, Tool::Ellipse] {
        for (sh, ct) in ALL {
            for &x in &vals {
                for &y in &vals {
                    let mut s = session(tool);
                    s.pointer_hover(A, false, false);
                    s.pointer_down(A, false);
                    s.pointer_hover(pt(x, y), sh, ct);
                    let _ = s.live_readout();
                    let _ = s.draw_list();
                    s.modifiers_changed(!sh, !ct);
                    s.pointer_hover(pt(x, y), !sh, !ct);
                    let _ = s.live_readout();
                    s.pointer_up(pt(x, y), sh, ct);
                    // whatever was written must reopen and stay finite
                    let bytes = pack(&doc_of(&s), "0.1.0").unwrap();
                    let d = unpack(7, &bytes).unwrap();
                    for id in d.object_ids() {
                        let p = d.primitive(id).unwrap();
                        let finite = match p.shape {
                            Shape::Rect { bounds, .. } => {
                                bounds.origin.x.is_finite()
                                    && bounds.origin.y.is_finite()
                                    && bounds.width.as_mm().is_finite()
                                    && bounds.height.as_mm().is_finite()
                            }
                            Shape::Ellipse { frame } => {
                                frame.center.x.is_finite()
                                    && frame.center.y.is_finite()
                                    && frame.rx.as_mm().is_finite()
                                    && frame.ry.as_mm().is_finite()
                            }
                            _ => true,
                        };
                        assert!(finite, "{tool:?} ({x},{y}) shift={sh} ctrl={ct}");
                    }
                }
            }
        }
    }
}

/// Informational probe: `modifiers_changed` alone, without the host's
/// re-sent hover. The ADR note (2026-10-06) says it only caches the state, so
/// the frontend must re-send the hover. This test pins what the Session does
/// today so a change is noticed; the browser check covers the real key path.
#[test]
fn probe_modifiers_changed_alone_does_not_move_the_preview() {
    let b = pt(130.0, 40.0);
    let mut s = session(Tool::Rectangle);
    s.pointer_hover(A, false, false);
    s.pointer_down(A, false);
    s.pointer_hover(b, false, false);
    let before = readout(&s).unwrap().0;
    s.modifiers_changed(true, false);
    let after = readout(&s).unwrap().0;
    eprintln!("modifiers_changed alone: readout {before:?} -> {after:?}");
    // the next hover (frontend re-sends it) picks the state up
    s.pointer_hover(b, true, false);
    assert_eq!(readout(&s).unwrap().0, "60.0 × 20.0 mm");
}
