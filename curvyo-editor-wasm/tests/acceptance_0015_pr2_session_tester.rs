//! Independent tester tests for PR 2 (rulers and pasteboard) of
//! `specs/0015-document-size-and-rulers` through `Session`: the status bar
//! texts (criteria 12, 21), the default view (11a), the ruler layout of the
//! live view and the click-on-a-tick rule (2, 9), and drawing and editing on
//! the pasteboard with every tool (29, 30, 31). Expected values come from the
//! specification's arithmetic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::many_single_char_names)]

use curvyo_document_core::{
    DisplayUnit, Document, DocumentSize, ObjectSnapshot, Point, Shape, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::{CANVAS_BG, PASTEBOARD_BG};
use curvyo_ui_core::RulerAxis;

const MINUS: char = '\u{2212}';

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn session_with(unit: DisplayUnit, w: f64, h: f64) -> Session {
    let doc = Document::new(1);
    doc.resize(DocumentSize::from_mm(w, h)).unwrap();
    let _ = doc.set_display_unit(unit);
    let bytes = pack(&doc, "0.1.0").unwrap();
    Session::open(2, &bytes).unwrap()
}

fn reopened(session: &Session) -> Document {
    unpack(99, &session.pack("0.1.0").unwrap()).unwrap()
}

fn corner(session: &Session) -> (f64, f64) {
    session.view().document_to_screen(pt(0.0, 0.0))
}

// ---- AC 12, 21: status texts ----

#[test]
fn ac12_new_project_shows_a4_with_unit() {
    let s = Session::new(1);
    assert_eq!(s.size_text(), "210.0 \u{d7} 297.0 mm");
    assert_eq!(s.display_unit(), DisplayUnit::Mm);
}

#[test]
fn ac21_cursor_text_mm_examples_and_rounding() {
    let s = Session::new(1);
    assert_eq!(s.cursor_text(12.3, 45.6), "x: 12.3  y: 45.6 mm");
    assert_eq!(s.cursor_text(0.0, 0.0), "x: 0.0  y: 0.0 mm");
    assert_eq!(s.cursor_text(210.0, 297.0), "x: 210.0  y: 297.0 mm");
    assert_eq!(
        s.cursor_text(-12.34, -0.06),
        format!("x: {MINUS}12.3  y: {MINUS}0.1 mm").replace(MINUS, "-")
    );
}

#[test]
fn ac21_cursor_text_unit_is_written_once_and_decimals_are_fixed() {
    let s = Session::new(1);
    for (x, y) in [
        (1.0, 2.0),
        (-1234.56, 99999.9),
        (1e6, -1e6),
        (0.04, 0.06),
        (1e-9, -1e-9),
    ] {
        let t = s.cursor_text(x, y);
        assert_eq!(t.matches("mm").count(), 1, "{t}");
        assert!(t.starts_with("x: ") && t.contains("  y: "), "{t}");
        for part in t.trim_end_matches(" mm").split("  ") {
            let v = part.split(": ").nth(1).unwrap();
            let decimals = v.split('.').nth(1).map_or(0, str::len);
            assert_eq!(decimals, 1, "{t}: {part}");
        }
        assert!(!t.contains("e-") && !t.contains("e+"), "{t}");
    }
}

#[test]
fn ac21_cursor_text_has_no_negative_zero() {
    let s = Session::new(1);
    for v in [-0.0, -0.04, -1e-9, -0.049] {
        let t = s.cursor_text(v, v);
        assert!(!t.contains("-0.0"), "negative zero readout for {v}: {t}");
        assert!(
            !t.contains(&format!("{MINUS}0.0")),
            "negative zero readout for {v}: {t}"
        );
    }
}

#[test]
fn ac21_cursor_text_survives_nonfinite_values() {
    let s = Session::new(1);
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e12, -1e12] {
        let t = s.cursor_text(v, v);
        eprintln!("cursor_text({v}) = {t:?}");
        assert!(t.len() < 100, "{} chars", t.len());
    }
}

#[test]
fn ac21_other_units_use_their_decimals() {
    let cm = session_with(DisplayUnit::Cm, 210.0, 297.0);
    assert_eq!(cm.size_text(), "21.00 \u{d7} 29.70 cm");
    assert_eq!(cm.cursor_text(12.3, 45.6), "x: 1.23  y: 4.56 cm");
    let inch = session_with(DisplayUnit::In, 215.9, 279.4);
    assert_eq!(inch.size_text(), "8.500 \u{d7} 11.000 in");
    assert_eq!(inch.cursor_text(25.4, 50.8), "x: 1.000  y: 2.000 in");
    assert_eq!(inch.cursor_text(12.3, 45.6), "x: 0.484  y: 1.795 in");
}

#[test]
fn ac21_size_text_is_fixed_decimals_for_odd_sizes() {
    let s = session_with(DisplayUnit::Mm, 1.0, 100_000.0);
    assert_eq!(s.size_text(), "1.0 \u{d7} 100000.0 mm");
    let s = session_with(DisplayUnit::Mm, 123.456, 0.04 + 1.0);
    assert_eq!(s.size_text(), "123.5 \u{d7} 1.0 mm");
}

#[test]
fn ac12_size_text_follows_the_open_file() {
    let s = session_with(DisplayUnit::Mm, 300.0, 400.0);
    assert_eq!(s.size_text(), "300.0 \u{d7} 400.0 mm");
}

// ---- AC 11a ----

#[test]
fn ac11a_default_view_survives_resizes_until_first_pan_or_zoom() {
    let mut s = Session::new(1);
    s.show_default_view();
    for (w, h) in [(0.0, 0.0), (1000.0, 700.0), (496.0, 546.0), (1500.0, 900.0)] {
        s.resize_viewport(w, h);
        let (x, y) = corner(&s);
        assert!(
            (x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9,
            "{w}x{h}: ({x}, {y})"
        );
    }
    assert_eq!(s.zoom_percent(), 100);
    s.wheel(0.0, 40.0, 100.0, 100.0, false, false);
    let before = s.screen_to_document(750.0, 450.0);
    s.resize_viewport(500.0, 450.0);
    let after = s.screen_to_document(250.0, 225.0);
    assert!((before.x - after.x).abs() < 1e-9 && (before.y - after.y).abs() < 1e-9);
}

#[test]
fn ac11a_reopening_also_gets_the_default_view_only_when_the_host_asks() {
    // Session::open itself leaves the plain view; the host calls
    // show_default_view for New and Open (wasm_api). A repeated call resets.
    let mut s = session_with(DisplayUnit::Mm, 210.0, 297.0);
    s.resize_viewport(900.0, 600.0);
    s.wheel(0.0, 0.0, 10.0, 10.0, true, true);
    s.wheel(30.0, 40.0, 10.0, 10.0, false, false);
    s.show_default_view();
    let (x, y) = corner(&s);
    assert!((x - 128.0).abs() < 1e-9 && (y - 128.0).abs() < 1e-9);
    assert_eq!(s.zoom_percent(), 100);
}

// ---- AC 2, 3, 4, 9: ruler layout of the live view ----

#[test]
fn ac2_session_layout_matches_the_canvas_projection_across_scripted_steps() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    for step in 0..20 {
        match step % 5 {
            0 => s.wheel(0.0, -120.0, 300.0, 200.0, false, true),
            1 => s.wheel(35.0, 12.0, 0.0, 0.0, false, false),
            2 => s.wheel(0.0, 80.0, 800.0, 600.0, false, true),
            3 => s.wheel(-90.0, 0.0, 0.0, 0.0, true, false),
            _ => s.resize_viewport(
                1000.0 - f64::from(step) * 11.0,
                700.0 - f64::from(step) * 5.0,
            ),
        }
        for axis in [RulerAxis::Horizontal, RulerAxis::Vertical] {
            let l = s.ruler_layout(axis, 900.0, 7.0, 7.0);
            for m in &l.majors {
                let mm = m.index as f64 * l.step();
                let (sx, sy) = s.view().document_to_screen(pt(mm, mm));
                let want = if axis == RulerAxis::Horizontal {
                    sx
                } else {
                    sy
                };
                assert!(
                    (m.px - want).abs() <= 0.5,
                    "step {step} {axis:?}: {} vs {want}",
                    m.px
                );
            }
        }
    }
}

#[test]
fn ac3_default_view_has_the_origin_tick_72px_in_and_labels_0() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    for axis in [RulerAxis::Horizontal, RulerAxis::Vertical] {
        let l = s.ruler_layout(axis, 900.0, 7.0, 7.0);
        assert!((l.origin_px.unwrap() - 128.0).abs() < 1e-9);
        assert!(
            l.labels
                .iter()
                .any(|x| x.text == "0" && (x.tick_px - 128.0).abs() < 1e-9)
        );
    }
    // mm at 100 %: majors every 20 mm
    let h = s.ruler_layout(RulerAxis::Horizontal, 1000.0, 7.0, 7.0);
    assert!((h.step() - 20.0).abs() < 1e-12);
    assert!(h.labels.iter().any(|x| x.text == "200"));
}

#[test]
fn ac4_rulers_run_on_past_the_document_in_every_direction() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.resize_viewport(1600.0, 1200.0);
    // A4 portrait is 210 wide: values past 210 and before 0 exist.
    let h = s.ruler_layout(RulerAxis::Horizontal, 1500.0, 7.0, 7.0);
    let texts: Vec<_> = h.labels.iter().map(|x| x.text.as_str()).collect();
    assert!(
        texts.contains(&"220") || texts.contains(&"240"),
        "{texts:?}"
    );
    let s2 = {
        let mut s2 = Session::new(1);
        s2.show_default_view();
        s2.resize_viewport(1600.0, 1200.0);
        s2.wheel(-100.0, 0.0, 0.0, 0.0, false, false);
        s2
    };
    let h2 = s2.ruler_layout(RulerAxis::Horizontal, 1500.0, 7.0, 7.0);
    assert!(h2.labels.iter().any(|x| x.text.starts_with(MINUS)));
}

#[test]
fn ac9_click_on_a_tick_hits_its_value_at_100_and_800_percent() {
    // At 100 % the mm step is 20, so the tick "60" is the one near 50; at
    // 800 % the step is 2 and "50" itself is a tick.
    for (wheel_delta, percent, target_mm) in [(0.0, 100, 60.0), (-1200.0, 800, 50.0)] {
        let mut s = Session::new(1);
        s.show_default_view();
        s.resize_viewport(1000.0, 700.0);
        if wheel_delta != 0.0 {
            s.wheel(0.0, wheel_delta, 72.0, 72.0, false, true);
            // pan right so that 50 mm is on screen (800 %: 30.2 px per mm)
            s.wheel(1182.0, 0.0, 72.0, 72.0, false, false);
        }
        assert_eq!(s.zoom_percent(), percent);
        let l = s.ruler_layout(RulerAxis::Horizontal, 900.0, 7.0, 7.0);
        let tick = l
            .majors
            .iter()
            .find(|m| (m.index as f64 * l.step() - target_mm).abs() < 1e-9)
            .unwrap_or_else(|| panic!("no tick {target_mm} at {percent} %"));
        assert!(
            l.labels
                .iter()
                .any(|x| x.text == format!("{target_mm}") && (x.tick_px - tick.px).abs() < 1e-9)
                || l.label_every > 1
        );
        let scale = s.view().scale();
        // press at the tick, any vertical position
        for y in [10.0, 300.0, 640.0] {
            let p = s.screen_to_document(tick.px, y);
            assert!(
                (p.x - target_mm).abs() <= 1.0 / scale + 1e-9,
                "{percent} %: {}",
                p.x
            );
        }
        s.set_tool(Tool::Rectangle);
        let a = s.screen_to_document(tick.px, 300.0);
        let b = s.screen_to_document(tick.px + 60.0, 380.0);
        drag(&mut s, a, b);
        let (x, _, _, _) = rect_of(&reopened(&s), 0);
        assert!(
            (x - target_mm).abs() <= 1.0 / scale + 1e-9,
            "{percent} %: rect at {x}"
        );
    }
}

// ---- AC 28 / 31 through the frame ----

#[test]
fn ac28_frame_starts_with_the_document_area_in_canvas_bg() {
    let mut s = Session::new(1);
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    let frame = s.frame_draw_list();
    assert!(frame.triangles.len() >= 6);
    assert!(frame.triangles[..6].iter().all(|v| v.color == CANVAS_BG));
    assert_eq!(frame.layers().first(), Some(&6));
}

#[test]
fn ac28_a_degenerate_pixel_ratio_reads_as_1_and_keeps_the_frame_finite() {
    for dpr in [0.0, -2.0, f64::NAN, f64::INFINITY] {
        let mut s = Session::new(1);
        s.set_device_pixel_ratio(dpr);
        assert_eq!(s.device_pixel_ratio(), 1.0);
        let frame = s.frame_draw_list();
        assert!(
            frame
                .triangles
                .iter()
                .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
        );
    }
}

#[test]
fn ac31_pen_knockout_colour_follows_the_position_in_the_live_frame() {
    for (x, y, want, not) in [
        (-40.0, -40.0, PASTEBOARD_BG, CANVAS_BG),
        (400.0, 100.0, PASTEBOARD_BG, CANVAS_BG),
        (100.0, 150.0, CANVAS_BG, PASTEBOARD_BG),
    ] {
        let mut s = Session::new(1);
        s.set_tool(Tool::Pen);
        s.pointer_hover(pt(x, y), false, false);
        s.pointer_down(pt(x, y), false);
        s.pointer_up(pt(x, y), false, false);
        let list = s.draw_list();
        let colours: Vec<_> = list.triangles.iter().map(|v| v.color).collect();
        assert!(
            colours.contains(&want) && !colours.contains(&not),
            "({x},{y})"
        );
    }
}

#[test]
fn ac31_pen_close_target_works_for_a_first_node_on_the_pasteboard() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    for p in [pt(-50.0, -50.0), pt(-10.0, -50.0), pt(-10.0, -10.0)] {
        s.pointer_hover(p, false, false);
        s.pointer_down(p, false);
        s.pointer_up(p, false, false);
    }
    s.pointer_hover(pt(-50.0, -50.0), false, false);
    assert!(matches!(
        s.pen_target(),
        Some(curvyo_ui_core::PenTarget::Close { .. })
    ));
    let colours: Vec<_> = s.draw_list().triangles.iter().map(|v| v.color).collect();
    assert!(colours.contains(&PASTEBOARD_BG) && !colours.contains(&CANVAS_BG));
    s.pointer_down(pt(-50.0, -50.0), false);
    s.pointer_up(pt(-50.0, -50.0), false, false);
    let d = reopened(&s);
    let id = *d.object_ids().last().unwrap();
    assert!(d.path(id).unwrap().closed, "path closed on the pasteboard");
}

// ---- AC 29, 30: pasteboard behaviour, beyond the implementer's tests ----

fn drag(s: &mut Session, from: Point, to: Point) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
}

fn rect_of(d: &Document, index: usize) -> (f64, f64, f64, f64) {
    let id = d.object_ids()[index];
    let Some(ObjectSnapshot::Primitive(p)) = d.object(id) else {
        panic!("primitive")
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!("rect")
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

#[test]
fn ac29_no_snapping_to_the_document_edge() {
    // Corners within a hair of the document edge keep their exact coordinates.
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(209.9, 10.0), pt(250.3, 296.95));
    let (x, y, w, h) = rect_of(&reopened(&s), 0);
    assert!((x - 209.9).abs() < 1e-9 && (y - 10.0).abs() < 1e-9);
    assert!(
        (w - 40.4).abs() < 1e-9 && (h - 286.95).abs() < 1e-9,
        "{w} {h}"
    );
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(-0.02, -0.02), pt(209.98, 297.01));
    let (x, y, w, h) = rect_of(&reopened(&s), 0);
    assert!((x + 0.02).abs() < 1e-9 && (y + 0.02).abs() < 1e-9);
    assert!((w - 210.0).abs() < 1e-9 && (h - 297.03).abs() < 1e-9);
}

#[test]
fn ac29_all_tools_leave_the_document_size_alone() {
    let mut s = Session::new(1);
    let before = s.size_text();
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(-500.0, -500.0), pt(900.0, 900.0));
    s.set_tool(Tool::Ellipse);
    drag(&mut s, pt(300.0, 300.0), pt(500.0, 340.0));
    s.set_tool(Tool::PolygonStar);
    drag(&mut s, pt(-100.0, 400.0), pt(-60.0, 400.0));
    s.set_tool(Tool::Pen);
    for p in [pt(-9.0, -9.0), pt(999.0, 5.0), pt(-9.0, 999.0)] {
        s.pointer_hover(p, false, false);
        s.pointer_down(p, false);
        s.pointer_up(p, false, false);
    }
    s.finish_pen();
    assert_eq!(s.size_text(), before);
    assert_eq!(reopened(&s).size(), DocumentSize::default());
    assert_eq!(reopened(&s).object_ids().len(), 4);
}

#[test]
fn ac30_an_object_wholly_on_the_pasteboard_is_marquee_selected_moved_and_deleted() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(300.0, 100.0), pt(340.0, 140.0));
    s.set_tool(Tool::Select);
    // marquee from empty pasteboard around it
    drag(&mut s, pt(280.0, 80.0), pt(360.0, 160.0));
    assert_eq!(
        s.selected_object_count(),
        1,
        "marquee on the pasteboard selects it"
    );
    // move by dragging its top edge
    drag(&mut s, pt(310.0, 100.0), pt(290.0, 130.0));
    let (x, y, w, h) = rect_of(&reopened(&s), 0);
    assert!(
        (x - 280.0).abs() < 1e-6 && (y - 130.0).abs() < 1e-6,
        "({x},{y})"
    );
    assert!((w - 40.0).abs() < 1e-9 && (h - 40.0).abs() < 1e-9);
    s.delete_selected();
    assert_eq!(reopened(&s).object_ids().len(), 0);
}

#[test]
fn ac30_dragging_an_object_across_the_document_edge_moves_it_exactly() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    drag(&mut s, pt(150.0, 50.0), pt(190.0, 90.0));
    s.set_tool(Tool::Select);
    // drag the left edge (press away from handles) 100 mm to the right, over the edge
    drag(&mut s, pt(150.0, 58.0), pt(250.0, 58.0));
    let (x, y, w, h) = rect_of(&reopened(&s), 0);
    assert!(
        (x - 250.0).abs() < 1e-6 && (y - 50.0).abs() < 1e-6,
        "({x},{y})"
    );
    assert!((w - 40.0).abs() < 1e-9 && (h - 40.0).abs() < 1e-9);
}

#[test]
fn ac30_node_tool_edits_a_path_on_the_pasteboard() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    for p in [pt(-80.0, -80.0), pt(-20.0, -80.0), pt(-20.0, -20.0)] {
        s.pointer_hover(p, false, false);
        s.pointer_down(p, false);
        s.pointer_up(p, false, false);
    }
    s.finish_pen();
    s.set_tool(Tool::Node);
    // select the path by a click on its first segment, then drag the third node
    s.pointer_hover(pt(-50.0, -80.0), false, false);
    s.pointer_down(pt(-50.0, -80.0), false);
    s.pointer_up(pt(-50.0, -80.0), false, false);
    drag(&mut s, pt(-20.0, -20.0), pt(-35.0, -5.0));
    let d = reopened(&s);
    let id = *d.object_ids().last().unwrap();
    let pts: Vec<_> = d
        .path(id)
        .unwrap()
        .anchors
        .iter()
        .map(|a| (a.point.x, a.point.y))
        .collect();
    assert_eq!(pts.len(), 3);
    assert!(
        (pts[2].0 - -35.0).abs() < 1e-6 && (pts[2].1 - -5.0).abs() < 1e-6,
        "{pts:?}"
    );
}
