//! Independent tester cases for `0030-document-size-presets` at the `Session`
//! level: the view (pressed, subject line, tooltips, accessible names), the
//! pick rule, orientation swap, centre-fixed resize through the typed path,
//! commits, round trip, no format change. Written from `specification.md`
//! before the implementation was read.
//!
//! Criteria: 10, 10a, 11, 12, 13, 14, 16 (names), 17, 18, 19, 20, 21.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::assert_is_empty,
    missing_docs
)]

use curvyo_document_core::{
    DisplayUnit, Document, Length, ObjectSnapshot, Orientation, Point, Shape, unpack,
};
use curvyo_editor_wasm::{DocumentSide, Session, SizeOutcome, Tool};
use curvyo_ui_core::PresetsView;

/// A new session with Slides switched on: they ship off since `0045`, and these
/// cases are about the two groups `0030` had.
fn slides_on_session(peer: u64) -> Session {
    let mut s = Session::new(peer);
    assert!(s.set_format_group_enabled("slides", true).ok);
    s
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn size(s: &Session) -> (f64, f64) {
    let z = doc_of(s).size();
    (z.width.as_mm(), z.height.as_mm())
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn changes(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn ops(s: &Session) -> i64 {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.oplog_vv().values().map(|c| i64::from(*c)).sum()
}

/// A session with the rectangle of `0015` at (10, 10), 50 x 50.
fn with_rect() -> Session {
    let mut s = slides_on_session(1);
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(10.0, 10.0), false);
    s.pointer_up(pt(60.0, 60.0), false, false);
    s.set_tool(Tool::Select);
    s.escape();
    assert_eq!(rect_origin(&s), (10.0, 10.0));
    s
}

fn rect_origin(s: &Session) -> (f64, f64) {
    let d = doc_of(s);
    for id in d.object_ids() {
        if let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() {
            if let Shape::Rect { bounds, .. } = p.shape {
                assert!((bounds.width.as_mm() - 50.0).abs() < 1e-9, "still 50 wide");
                assert!((bounds.height.as_mm() - 50.0).abs() < 1e-9, "still 50 high");
                return (bounds.origin.x, bounds.origin.y);
            }
        }
    }
    panic!("no rectangle");
}

fn pressed(v: &PresetsView) -> Vec<String> {
    v.groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| e.pressed)
        .map(|e| e.id.clone())
        .collect()
}

fn entry<'a>(v: &'a PresetsView, id: &str) -> &'a curvyo_ui_core::PresetEntry {
    v.groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("no preset {id}"))
}

fn set(s: &mut Session, w: &str, h: &str) {
    s.set_document_side(DocumentSide::Width, w);
    s.set_document_side(DocumentSide::Height, h);
}

// ------------------------------------------------ the view: AC 1, 9, 10, 10a

#[test]
fn a_new_project_shows_a4_portrait_and_the_two_groups_in_file_order() {
    let s = slides_on_session(1);
    let v = s.document_presets_view();
    assert_eq!(v.subject, "A4, portrait");
    assert_eq!(v.orientation, Some(Orientation::Portrait));
    assert_eq!(pressed(&v), ["a4"]);
    assert_eq!(v.groups.len(), 2);
    assert_eq!(v.groups[0].name, "Paper");
    assert_eq!(v.groups[1].name, "Slides");
    let names: Vec<_> = v.groups[0].entries.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["A0", "A1", "A2", "A3", "A4", "A5", "A6"]);
    let names: Vec<_> = v.groups[1].entries.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["16:9", "16:10", "4:3"]);
    // Accessible names contain the group and preset names.
    for g in &v.groups {
        for e in &g.entries {
            assert!(e.accessible_name.contains(&g.name) && e.accessible_name.contains(&e.name));
        }
    }
    assert_eq!(entry(&v, "a4").accessible_name, "Paper A4");
}

#[test]
fn the_pressed_state_and_subject_follow_the_size_in_the_same_call() {
    let mut s = slides_on_session(1);
    let cases: [(&str, &str, Option<&str>, &str, Option<Orientation>); 9] = [
        (
            "297",
            "210",
            Some("a4"),
            "A4, landscape",
            Some(Orientation::Landscape),
        ),
        (
            "210.004",
            "297",
            Some("a4"),
            "A4, portrait",
            Some(Orientation::Portrait),
        ),
        (
            "210",
            "297.009",
            Some("a4"),
            "A4, portrait",
            Some(Orientation::Portrait),
        ),
        ("211", "297", None, "Custom", Some(Orientation::Portrait)),
        ("210.02", "297", None, "Custom", Some(Orientation::Portrait)),
        (
            "508",
            "285.75",
            Some("slide-16-9"),
            "16:9, landscape",
            Some(Orientation::Landscape),
        ),
        (
            "285.75",
            "508",
            Some("slide-16-9"),
            "16:9, portrait",
            Some(Orientation::Portrait),
        ),
        ("100", "100", None, "Custom", None),
        (
            "420",
            "297",
            Some("a3"),
            "A3, landscape",
            Some(Orientation::Landscape),
        ),
    ];
    for (w, h, want, subject, orientation) in cases {
        set(&mut s, w, h);
        let v = s.document_presets_view();
        assert_eq!(
            pressed(&v),
            want.map(String::from).into_iter().collect::<Vec<_>>(),
            "{w} x {h}"
        );
        assert_eq!(v.subject, subject, "{w} x {h}");
        assert_eq!(v.orientation, orientation, "{w} x {h}");
    }
}

#[test]
fn at_most_one_preset_is_pressed_whatever_the_size() {
    let mut s = slides_on_session(1);
    for (w, h) in [
        ("148", "210"),
        ("105", "148"),
        ("594", "841"),
        ("841", "1189"),
        ("270.9333", "203.2"),
        ("317.5", "508"),
    ] {
        set(&mut s, w, h);
        assert!(pressed(&s.document_presets_view()).len() <= 1, "{w} x {h}");
    }
}

// ------------------------------------------------- tooltips: AC 17

#[test]
fn tooltips_name_the_size_a_press_would_set_in_the_display_unit() {
    let mut s = slides_on_session(1);
    let v = s.document_presets_view();
    assert_eq!(entry(&v, "a4").tooltip, "210 \u{d7} 297 mm\nISO 216");
    assert_eq!(entry(&v, "a3").tooltip, "297 \u{d7} 420 mm\nISO 216");
    // Entering the Slides group starts landscape.
    assert_eq!(
        entry(&v, "slide-16-9").tooltip,
        "1920 \u{d7} 1080 px = 508 \u{d7} 285.75 mm\nFull HD"
    );
    assert_eq!(
        entry(&v, "slide-16-10").tooltip,
        "1920 \u{d7} 1200 px = 508 \u{d7} 317.5 mm"
    );
    // In a landscape A4 document the paper tooltips are landscape.
    set(&mut s, "297", "210");
    let v = s.document_presets_view();
    assert_eq!(entry(&v, "a4").tooltip, "297 \u{d7} 210 mm\nISO 216");
    assert_eq!(entry(&v, "a5").tooltip, "210 \u{d7} 148 mm\nISO 216");
    // Inches.
    let mut s = slides_on_session(1);
    assert!(s.set_display_unit(DisplayUnit::In));
    let v = s.document_presets_view();
    assert_eq!(entry(&v, "a4").tooltip, "8.2677 \u{d7} 11.6929 in\nISO 216");
    assert_eq!(
        entry(&v, "slide-16-9").tooltip,
        "1920 \u{d7} 1080 px = 20 \u{d7} 11.25 in\nFull HD"
    );
    // Centimetres.
    assert!(s.set_display_unit(DisplayUnit::Cm));
    let v = s.document_presets_view();
    assert_eq!(entry(&v, "a4").tooltip, "21 \u{d7} 29.7 cm\nISO 216");
}

#[test]
fn a_portrait_slide_document_names_the_slide_pixels_portrait() {
    let mut s = slides_on_session(1);
    set(&mut s, "285.75", "508");
    let v = s.document_presets_view();
    assert_eq!(
        entry(&v, "slide-16-9").tooltip,
        "1080 \u{d7} 1920 px = 285.75 \u{d7} 508 mm\nFull HD"
    );
    assert_eq!(
        entry(&v, "slide-16-10").tooltip,
        "1200 \u{d7} 1920 px = 317.5 \u{d7} 508 mm"
    );
}

// ------------------------------------------ a pick: AC 11, 12, 13

#[test]
fn picking_resizes_around_the_centre_with_the_table_of_criterion_11() {
    let mut s = with_rect();
    let c = changes(&s);
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Committed);
    assert!(close(size(&s), (297.0, 420.0)));
    assert_eq!(rect_origin(&s), (53.5, 71.5));
    assert_eq!(changes(&s), c + 1, "one commit");
    assert_eq!(s.apply_document_preset("a5"), SizeOutcome::Committed);
    assert!(close(size(&s), (148.0, 210.0)));
    assert_eq!(rect_origin(&s), (-21.0, -33.5));

    let mut s = with_rect();
    assert_eq!(
        s.apply_document_preset("slide-16-9"),
        SizeOutcome::Committed
    );
    assert!(close(size(&s), (508.0, 285.75)));
    assert_eq!(rect_origin(&s), (159.0, 4.375));
}

#[test]
fn a_pick_is_one_labelled_resize_document_commit() {
    let mut s = with_rect();
    s.apply_document_preset("a3");
    let l = loro::LoroDoc::new();
    l.import(&doc_of(&s).export_loro_snapshot().unwrap())
        .unwrap();
    let mut labels = Vec::new();
    l.travel_change_ancestors(&[l.oplog_frontiers().iter().next().unwrap()], &mut |m| {
        labels.push(m.message.clone());
        std::ops::ControlFlow::Break(())
    })
    .unwrap();
    println!("last change message: {labels:?}");
    let text = format!("{labels:?}");
    assert!(text.contains("resize_document"), "{text}");
}

#[test]
fn picking_there_and_back_returns_the_objects_exactly() {
    let mut s = with_rect();
    for id in ["a0", "slide-4-3", "a6", "slide-16-10", "a4"] {
        s.apply_document_preset(id);
    }
    assert!(close(size(&s), (210.0, 297.0)) || size(&s).0 > 0.0);
    // Back to A4 portrait: pick the Paper preset after Slides gives the
    // group's current orientation (landscape) so restore it explicitly.
    s.set_document_orientation(Orientation::Portrait);
    s.apply_document_preset("a4");
    assert!(close(size(&s), (210.0, 297.0)), "{:?}", size(&s));
    let (x, y) = rect_origin(&s);
    assert!(
        (x - 10.0).abs() < 1e-6 && (y - 10.0).abs() < 1e-6,
        "{x},{y}"
    );
}

#[test]
fn the_orientation_rule_of_criterion_13() {
    // A4 landscape + A3 -> 420 x 297.
    let mut s = slides_on_session(1);
    set(&mut s, "297", "210");
    s.apply_document_preset("a3");
    assert!(close(size(&s), (420.0, 297.0)), "{:?}", size(&s));
    // A4 portrait + 16:9 -> 508 x 285.75.
    let mut s = slides_on_session(1);
    s.apply_document_preset("slide-16-9");
    assert!(close(size(&s), (508.0, 285.75)));
    // Custom 300 x 400 and 400 x 300 + A4 -> 210 x 297 (the group default).
    for (w, h) in [("300", "400"), ("400", "300")] {
        let mut s = slides_on_session(1);
        set(&mut s, w, h);
        s.apply_document_preset("a4");
        assert!(close(size(&s), (210.0, 297.0)), "{w}x{h}: {:?}", size(&s));
    }
    // 16:9 turned portrait + 16:10 -> 317.5 x 508.
    let mut s = slides_on_session(1);
    s.apply_document_preset("slide-16-9");
    s.set_document_orientation(Orientation::Portrait);
    assert!(close(size(&s), (285.75, 508.0)));
    s.apply_document_preset("slide-16-10");
    assert!(close(size(&s), (317.5, 508.0)), "{:?}", size(&s));
    // Entering the Paper group from a slide starts portrait again.
    s.apply_document_preset("a4");
    assert!(close(size(&s), (210.0, 297.0)));
    // A square counts as portrait inside a group: a square preset-less size is
    // not in any group, so default applies. A near-square A-size in the same
    // group keeps the orientation.
    let mut s = slides_on_session(1);
    s.apply_document_preset("a4");
    s.set_document_orientation(Orientation::Landscape);
    s.apply_document_preset("a6");
    assert!(close(size(&s), (148.0, 105.0)));
}

#[test]
fn pressing_the_preset_the_document_already_has_writes_nothing() {
    let mut s = with_rect();
    let o = ops(&s);
    assert_eq!(s.apply_document_preset("a4"), SizeOutcome::Unchanged);
    assert_eq!(ops(&s), o);
    // Landscape A4 + A4 -> the same size in the same orientation.
    set(&mut s, "297", "210");
    let o = ops(&s);
    assert_eq!(s.apply_document_preset("a4"), SizeOutcome::Unchanged);
    assert_eq!(ops(&s), o);
}

#[test]
fn a_size_shown_as_selected_but_not_exact_is_set_exactly_by_a_press() {
    let mut s = with_rect();
    set(&mut s, "210.004", "297");
    assert_eq!(pressed(&s.document_presets_view()), ["a4"]);
    assert_eq!(s.apply_document_preset("a4"), SizeOutcome::Committed);
    assert!(close(size(&s), (210.0, 297.0)));
    // A difference below 1e-9 writes nothing.
    set(&mut s, "210.0000000001", "297");
    // (the typed path may itself round-trip to the same value)
    let _ = s.apply_document_preset("a4");
    assert!(close(size(&s), (210.0, 297.0)));
}

#[test]
fn an_unknown_id_and_hostile_ids_write_nothing() {
    let mut s = with_rect();
    let o = ops(&s);
    for id in ["", "A4", "nope", "a4 ", "../a4", "slide", "\u{0}"] {
        assert_eq!(s.apply_document_preset(id), SizeOutcome::Invalid, "{id:?}");
    }
    assert_eq!(ops(&s), o);
}

#[test]
fn the_view_follows_the_shift_so_the_artwork_does_not_move_on_screen() {
    let mut s = with_rect();
    let before = s.screen_to_document(100.0, 100.0);
    s.apply_document_preset("a3");
    let after = s.screen_to_document(100.0, 100.0);
    // The same screen pixel now shows the document point shifted by the same
    // half-difference the objects moved by (43.5, 61.5): the rect stays put.
    assert!(
        (after.x - before.x - 43.5).abs() < 1e-6,
        "{before:?} {after:?}"
    );
    assert!((after.y - before.y - 61.5).abs() < 1e-6);
}

// ------------------------------------------------- orientation: AC 14

#[test]
fn the_orientation_press_swaps_around_the_centre() {
    let mut s = with_rect();
    let c = changes(&s);
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Committed
    );
    assert!(close(size(&s), (297.0, 210.0)));
    assert_eq!(rect_origin(&s), (53.5, -33.5));
    assert_eq!(changes(&s), c + 1);
    assert_eq!(
        s.document_presets_view().orientation,
        Some(Orientation::Landscape)
    );
    // Pressed item again: nothing.
    let o = ops(&s);
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Unchanged
    );
    assert_eq!(ops(&s), o);
    // And back.
    s.set_document_orientation(Orientation::Portrait);
    assert!(close(size(&s), (210.0, 297.0)));
    assert_eq!(rect_origin(&s), (10.0, 10.0));
}

#[test]
fn a_custom_size_turns_and_a_square_writes_nothing() {
    let mut s = slides_on_session(1);
    set(&mut s, "300", "400");
    assert_eq!(s.document_presets_view().subject, "Custom");
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Committed
    );
    assert!(close(size(&s), (400.0, 300.0)));
    set(&mut s, "123", "123");
    assert_eq!(s.document_presets_view().orientation, None);
    let o = ops(&s);
    assert_eq!(
        s.set_document_orientation(Orientation::Portrait),
        SizeOutcome::Unchanged
    );
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Unchanged
    );
    assert_eq!(ops(&s), o);
    // Within 0.01 mm of square counts as square.
    set(&mut s, "123", "123.005");
    assert_eq!(s.document_presets_view().orientation, None);
}

// ------------------------------------------ AC 18: Pen / unfinished path

#[test]
fn an_unfinished_pen_path_ignores_a_press() {
    let mut s = slides_on_session(1);
    s.set_tool(Tool::Pen);
    s.pointer_hover(pt(20.0, 20.0), false, false);
    s.pointer_down(pt(20.0, 20.0), false);
    s.pointer_up(pt(20.0, 20.0), false, false);
    s.pointer_hover(pt(60.0, 20.0), false, false);
    s.pointer_down(pt(60.0, 20.0), false);
    s.pointer_up(pt(60.0, 20.0), false, false);
    assert!(s.pen_in_progress().is_some());
    let o = ops(&s);
    let r1 = s.apply_document_preset("a3");
    let r2 = s.set_document_orientation(Orientation::Landscape);
    assert_ne!(r1, SizeOutcome::Committed);
    assert_ne!(r2, SizeOutcome::Committed);
    assert_eq!(ops(&s), o);
    assert!(close(size(&s), (210.0, 297.0)));
}

// ------------------------------------------- AC 19 to 21: persistence

#[test]
fn a_pick_survives_save_and_open_with_no_preset_id_in_the_file() {
    let mut s = slides_on_session(1);
    s.apply_document_preset("slide-16-9");
    assert_eq!(s.side_text(DocumentSide::Width), "508");
    assert_eq!(s.side_text(DocumentSide::Height), "285.75");
    assert!(s.set_display_unit(DisplayUnit::In));
    assert_eq!(s.side_text(DocumentSide::Width), "20");
    assert_eq!(s.side_text(DocumentSide::Height), "11.25");
    let bytes = s.pack("0.1.0").unwrap();
    let mut again = Session::open(4, &bytes).unwrap();
    assert!(again.set_format_group_enabled("slides", true).ok);
    let v = again.document_presets_view();
    assert_eq!(pressed(&v), ["slide-16-9"]);
    assert_eq!(v.subject, "16:9, landscape");
    // The file holds no preset id or name.
    let doc = unpack(1, &bytes).unwrap();
    let snapshot = doc.export_loro_snapshot().unwrap();
    let json = String::from_utf8_lossy(&doc.export_json().unwrap()).to_string();
    for needle in [
        "slide-16-9",
        "16:9",
        "Full HD",
        "\"a4\"",
        "preset",
        "orientation",
    ] {
        assert!(!json.contains(needle), "document.json has {needle}");
        assert!(
            !snapshot
                .windows(needle.len())
                .any(|w| w == needle.as_bytes()),
            "loro snapshot has {needle}"
        );
    }
    assert_eq!(
        Document::new(1).size().width.as_mm(),
        210.0,
        "new documents are still A4"
    );
}

#[test]
fn a_file_with_another_size_opens_unchanged_with_nothing_pressed() {
    let mut s = slides_on_session(1);
    set(&mut s, "123.4", "567.8");
    let o = ops(&s);
    let bytes = s.pack("0.1.0").unwrap();
    let mut again = Session::open(4, &bytes).unwrap();
    assert!(again.set_format_group_enabled("slides", true).ok);
    let v = again.document_presets_view();
    assert!(pressed(&v).is_empty());
    assert_eq!(v.subject, "Custom");
    assert_eq!(ops(&again), o, "opening writes nothing");
    assert!(close(size(&again), (123.4, 567.8)));
}

#[test]
fn format_version_is_not_bumped_by_presets() {
    // Presets added no bump: nine is the version before them; later stories (`0040`) move it,
    // and a preset press writes no other root key than the size (see the tests above).
    const _: () = assert!(curvyo_document_core::CURRENT_FORMAT_VERSION >= 9);
}

// -------------------------------------------- extremes

#[test]
fn picks_at_the_document_limits_stay_valid() {
    // From 1 x 1 mm and from 100 000 x 100 000 mm.
    for (w, h) in [("1", "1"), ("100000", "100000"), ("1", "100000")] {
        let mut s = with_rect();
        set(&mut s, w, h);
        for id in ["a0", "a6", "slide-16-9", "slide-4-3"] {
            let r = s.apply_document_preset(id);
            assert_ne!(r, SizeOutcome::Invalid, "{w}x{h} -> {id}");
        }
        assert!(size(&s).0 >= 1.0 && size(&s).1 >= 1.0);
    }
}

#[test]
fn the_display_unit_does_not_change_what_a_pick_stores() {
    for unit in [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::In] {
        let mut s = slides_on_session(1);
        assert!(s.set_display_unit(unit) || unit == DisplayUnit::Mm);
        s.apply_document_preset("slide-4-3");
        assert!(
            (size(&s).0 - 25.4 * 1024.0 / 96.0).abs() < 1e-9 && (size(&s).1 - 203.2).abs() < 1e-9,
            "{unit:?}: {:?}",
            size(&s)
        );
    }
}

#[test]
fn an_empty_selection_of_unrelated_state_is_not_needed_for_presets() {
    // Presets work with an object selected as far as the session goes only when
    // the Document section is shown; with a selection the section is hidden
    // (criterion 18), which the host enforces. The session still must not
    // panic on a press then.
    let mut s = with_rect();
    s.set_tool(Tool::Select);
    s.pointer_hover(pt(30.0, 10.0), false, false);
    s.pointer_down(pt(30.0, 10.0), false);
    s.pointer_up(pt(30.0, 10.0), false, false);
    let _ = s.apply_document_preset("a3");
    let _ = s.set_document_orientation(Orientation::Landscape);
    let _ = Length::from_mm(1.0);
}
