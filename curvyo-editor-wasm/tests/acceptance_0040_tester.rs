//! Independent tester cases for `0040-document-background` at the `Session` level: the new
//! project and Open (2, 4, 6, 7, 8), the draw list (13, 14, 47), the Background block's
//! commands (17 to 24), the eyedropper picking the background (28 to 39), the background not
//! being an object (40 to 43) and the Pen knockout (46). Written from `specification.md` before
//! the implementation was read. Not testable here (no browser): the checkerboard shader (12),
//! focus and Tab order (21, 25, 26), accessible names (27), the 800 x 600 layout (15a), the
//! DOM of the block (15, 16), the chip drawing (35) and the status region (39, wording only).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::type_complexity,
    clippy::similar_names,
    clippy::many_single_char_names,
    missing_docs
)]

use curvyo_document_core::{
    BackgroundPaint, Color, DisplayUnit, Document, DocumentBackground, ObjectSnapshot, Opacity,
    OpenError, Point, Style, unpack,
};
use curvyo_editor_wasm::{FitOutcome, Session, SizeOutcome, Tool};
use curvyo_render_core::{CANVAS_BG, CHECKER_A, DrawList, PASTEBOARD_BG, RgbaColor};
use curvyo_ui_core::{Grid, PaintTarget, PanelContent, StyleEntryError, StyleField};
use loro::{LoroDoc, LoroValue};

// ---------------------------------------------------------------- helpers

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn loro_of(s: &Session) -> LoroDoc {
    let l = LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l
}

fn ops(s: &Session) -> i64 {
    loro_of(s).oplog_vv().values().map(|c| i64::from(*c)).sum()
}

fn changes(s: &Session) -> usize {
    loro_of(s).len_changes()
}

/// Toggles the display unit (a commit with another label) so that the next commit cannot merge
/// into the previous one, and returns the change count.
fn baseline(s: &mut Session) -> usize {
    let unit = if s.display_unit() == DisplayUnit::In {
        DisplayUnit::Mm
    } else {
        DisplayUnit::In
    };
    assert!(s.set_display_unit(unit));
    changes(s)
}

fn registers(s: &Session) -> (Option<LoroValue>, Option<LoroValue>) {
    let l = loro_of(s);
    let root = l.get_map("root");
    (
        root.get("background_paint").map(|v| v.get_deep_value()),
        root.get("background_color").map(|v| v.get_deep_value()),
    )
}

fn stored(s: &Session) -> DocumentBackground {
    doc_of(s).background()
}

fn last_label(s: &Session) -> String {
    let l = loro_of(s);
    let mut out = String::new();
    l.travel_change_ancestors(&[l.oplog_frontiers().iter().next().unwrap()], &mut |m| {
        out = m.message.map(|x| x.to_string()).unwrap_or_default();
        std::ops::ControlFlow::Break(())
    })
    .unwrap();
    out
}

fn solid(r: u8, g: u8, b: u8, a: f64) -> DocumentBackground {
    DocumentBackground {
        paint: BackgroundPaint::Solid,
        color: Color { r, g, b },
        opacity: Opacity::new(a).unwrap(),
    }
}

/// Sets a stored background through the Background block's own commands (hex).
fn set_bg_hex(s: &mut Session, hex: &str) {
    assert!(
        s.set_background_hex(hex).unwrap(),
        "{hex} must change the background"
    );
}

fn click(s: &mut Session, at: Point) {
    s.pointer_hover(at, false, false);
    s.pointer_down(at, false);
    s.pointer_up(at, false, false);
}

fn draw_rect(s: &mut Session, a: Point, b: Point) {
    s.set_tool(Tool::Rectangle);
    s.pointer_down(a, false);
    s.pointer_up(b, false, false);
    s.set_tool(Tool::Select);
}

/// Selects whatever the marquee from `a` to `b` touches.
fn marquee(s: &mut Session, a: Point, b: Point) {
    s.set_tool(Tool::Select);
    s.escape();
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(b, false, false);
    s.pointer_up(b, false, false);
}

fn styles(s: &Session) -> Vec<Style> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

/// Rectangle 0 at (0,0)-(20,20) with a green fill and a blue 80 % stroke; rectangle 1 at
/// (40,0)-(60,20) with defaults and selected as the pick target.
fn painted() -> Session {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "00FF00").unwrap();
    s.set_style_text(StyleField::StrokeColor, "0000FFCC")
        .unwrap();
    draw_rect(&mut s, pt(40.0, 0.0), pt(60.0, 20.0));
    marquee(&mut s, pt(35.0, -5.0), pt(65.0, 25.0));
    assert_eq!(s.selected_object_count(), 1);
    s
}

fn colours(list: &DrawList) -> Vec<RgbaColor> {
    list.triangles.iter().map(|v| v.color).collect()
}

// ------------------------------------------------ AC 2, 8: new, Open, no leaks

#[test]
fn ac2_a_new_session_shows_the_default_in_the_view_and_the_draw_list() {
    let s = Session::new(1);
    let v = s.background_view();
    assert_eq!(v.paint, "solid");
    assert_eq!(v.hex, "#E8E8EBFF");
    assert_eq!(v.opacity, 100.0);
    assert_eq!(v.color, 0x00E8_E8EB);
    assert!(!v.picking);
    let list = s.frame_draw_list();
    assert!(list.triangles[..6].iter().all(|x| x.color == CANVAS_BG));
    assert_eq!(list.checker_end(), 0);
    assert_eq!(
        registers(&s),
        (None, None),
        "a new project writes neither register"
    );
}

#[test]
fn ac8_open_shows_its_own_background_and_nothing_of_the_previous_project() {
    let mut a = Session::new(1);
    set_bg_hex(&mut a, "FF0000FF");
    assert_eq!(a.background_view().hex, "#FF0000FF");
    let plain = Session::new(2).pack("0.1.0").unwrap();
    // Open a file without the registers after editing another project
    let b = Session::open(3, &plain).unwrap();
    assert_eq!(b.background_view().hex, "#E8E8EBFF");
    assert!(
        b.frame_draw_list().triangles[..6]
            .iter()
            .all(|x| x.color == CANVAS_BG)
    );
    // and a file with a background shows that one
    let red = a.pack("0.1.0").unwrap();
    let c = Session::open(4, &red).unwrap();
    assert_eq!(c.background_view().hex, "#FF0000FF");
    assert!(
        c.frame_draw_list().triangles[..6]
            .iter()
            .all(|x| x.color == RgbaColor::opaque(255, 0, 0))
    );
}

#[test]
fn ac8_a_preview_in_flight_does_not_survive_into_a_newly_opened_project() {
    let mut a = Session::new(1);
    a.preview_background_hsv(120.0, 1.0, 1.0);
    let b = Session::open(2, &Session::new(3).pack("0.1.0").unwrap()).unwrap();
    assert_eq!(b.background_view().hex, "#E8E8EBFF");
    assert_ne!(
        a.background_view().hex,
        "#E8E8EBFF",
        "the first session is still previewing"
    );
}

// ------------------------------------------------ AC 4, 7 at the session level

#[test]
fn ac4_every_older_fixture_opens_in_a_session_with_the_default_background() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../curvyo-document-core/tests/fixtures");
    for name in [
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_gradient_v7.curvyo",
        "compound_v8.curvyo",
        "markers_v9.curvyo",
        "dash_v9.curvyo",
    ] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let s = Session::open(1, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(s.background_view().hex, "#E8E8EBFF", "{name}");
        assert_eq!(s.background_view().paint, "solid", "{name}");
        assert_eq!(
            registers(&s),
            (None, None),
            "{name}: opening wrote a register"
        );
    }
}

#[test]
fn ac7_a_damaged_background_is_refused_as_damaged_like_any_damaged_file() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../curvyo-document-core/tests/fixtures");
    for name in [
        "background_paint_unknown_v10.curvyo",
        "background_color_three_v10.curvyo",
        "background_color_range_v10.curvyo",
        "background_color_type_v10.curvyo",
    ] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        assert!(
            matches!(Session::open(1, &bytes), Err(OpenError::Damaged)),
            "{name}"
        );
    }
}

// ------------------------------------------------ AC 6: save, close, open

#[test]
fn ac6_a_none_background_keeps_its_colour_through_save_and_open() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "2F6FEE80");
    assert!(s.set_background_paint(BackgroundPaint::None));
    let mut back = Session::open(2, &s.pack("0.1.0").unwrap()).unwrap();
    let v = back.background_view();
    assert_eq!(v.paint, "none");
    assert!(back.frame_draw_list().checker_end() > 0);
    assert!(back.set_background_paint(BackgroundPaint::Solid));
    assert_eq!(
        back.background_view().hex,
        "#2F6FEE80",
        "same colour, alpha 128/255 not 50 %"
    );
    assert_eq!(
        stored(&back).opacity.get().to_bits(),
        (128.0_f64 / 255.0).to_bits()
    );
}

// ------------------------------------------------ AC 17: the Paint group

#[test]
fn ac17_none_and_solid_are_one_commit_each_and_the_colour_returns() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "11223380");
    let c = baseline(&mut s);
    assert!(s.set_background_paint(BackgroundPaint::None));
    assert_eq!(changes(&s), c + 1);
    assert_eq!(last_label(&s), "set_document_background");
    assert_eq!(s.background_view().paint, "none");
    assert!(s.set_background_paint(BackgroundPaint::Solid));
    assert_eq!(s.background_view().hex, "#11223380");
    // a press on the already pressed item writes nothing
    let o = ops(&s);
    let c = changes(&s);
    assert!(!s.set_background_paint(BackgroundPaint::Solid));
    assert_eq!((ops(&s), changes(&s)), (o, c));
}

#[test]
fn ac17_the_paint_edit_writes_only_the_paint_register() {
    let mut s = Session::new(1);
    let o = ops(&s);
    assert!(s.set_background_paint(BackgroundPaint::None));
    assert_eq!(ops(&s), o + 1);
    let (paint, colour) = registers(&s);
    assert_eq!(paint, Some(LoroValue::String("none".to_string().into())));
    assert_eq!(colour, None);
}

// ------------------------------------------------ AC 18: the hex field

#[test]
fn ac18_the_hex_field_shows_8_upper_case_digits_and_accepts_3_4_6_and_8() {
    let mut s = Session::new(1);
    assert_eq!(s.background_view().hex, "#E8E8EBFF");
    assert!(s.set_background_hex("fff").unwrap());
    assert_eq!(s.background_view().hex, "#FFFFFFFF");
    assert!(s.set_background_hex("f80c").unwrap());
    assert_eq!(s.background_view().hex, "#FF8800CC");
    // 3 and 6 digits keep the alpha
    assert!(s.set_background_hex("abc").unwrap());
    assert_eq!(s.background_view().hex, "#AABBCCCC");
    assert!(s.set_background_hex("#123456").unwrap());
    assert_eq!(s.background_view().hex, "#123456CC");
    // 8 digits, with #, any case, surrounding spaces ignored (0017 criterion 12)
    assert!(s.set_background_hex("  #2f6fee80  ").unwrap());
    assert_eq!(s.background_view().hex, "#2F6FEE80");
    assert!(s.set_background_hex("2F6FEEFF").unwrap());
    assert_eq!(s.background_view().hex, "#2F6FEEFF");
    assert_eq!(stored(&s).opacity.get().to_bits(), 1.0_f64.to_bits());
}

#[test]
fn ac18_refused_input_writes_nothing_and_reports_the_hex_error() {
    let mut s = Session::new(1);
    for bad in [
        "12345",
        "1234567",
        "123456789",
        "",
        "#",
        "  ",
        "GGG",
        "12 3 4 5",
        "\u{ff10}\u{ff10}\u{ff10}",
        "FF00zz",
        "\u{0}",
        "FFFFFFF",
        "ffffffffff",
        "\u{1f600}\u{1f600}\u{1f600}",
        "-1-1-1",
    ] {
        let o = ops(&s);
        assert_eq!(
            s.set_background_hex(bad),
            Err(StyleEntryError::Hex),
            "{bad:?}"
        );
        assert_eq!(ops(&s), o, "{bad:?} wrote");
        assert_eq!(s.background_view().hex, "#E8E8EBFF");
    }
    // very long input must not panic
    let long = "F".repeat(100_000);
    assert!(s.set_background_hex(&long).is_err());
}

#[test]
fn ac18_a_value_equal_to_the_stored_one_writes_nothing() {
    let mut s = Session::new(1);
    let (o, c) = (ops(&s), changes(&s));
    assert_eq!(s.set_background_hex("E8E8EBFF"), Ok(false));
    assert_eq!(s.set_background_hex("#e8e8eb"), Ok(false));
    assert_eq!((ops(&s), changes(&s)), (o, c));
}

// ------------------------------------------------ AC 19: Opacity

#[test]
fn ac19_hex_and_opacity_edit_the_same_alpha_each_in_its_own_grid() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "2F6FEE80");
    let v = s.background_view();
    assert_eq!(v.opacity_text, "50", "128/255 shows as 50");
    assert_eq!(
        stored(&s).opacity.get().to_bits(),
        (128.0_f64 / 255.0).to_bits()
    );
    assert_eq!(s.set_background_opacity_text("50"), Ok(true));
    assert_eq!(stored(&s).opacity.get().to_bits(), 0.5_f64.to_bits());
    assert_eq!(s.background_view().hex, "#2F6FEE80");
    // opacity 100 returns to opaque, 0 to alpha 0
    assert_eq!(s.set_background_opacity_text("100"), Ok(true));
    assert_eq!(s.background_view().hex, "#2F6FEEFF");
    assert_eq!(s.set_background_opacity_text("0"), Ok(true));
    assert_eq!(s.background_view().hex, "#2F6FEE00");
    assert_eq!(stored(&s).opacity.get(), 0.0);
}

#[test]
fn ac19_hostile_opacity_text_is_refused_and_writes_nothing() {
    let mut s = Session::new(1);
    for bad in [
        "NaN", "nan", "inf", "-inf", "Infinity", "-1", "100.5", "101", "1e999", "abc", "", " ",
        "5 0", "--5", "0x10", "٥٠",
    ] {
        let o = ops(&s);
        let r = s.set_background_opacity_text(bad);
        assert!(r.is_err(), "{bad:?} gave {r:?}");
        assert_eq!(ops(&s), o, "{bad:?} wrote");
    }
    assert!(stored(&s).opacity.get().is_finite());
}

#[test]
fn ac19_the_reset_slot_returns_alpha_to_one_and_writes_only_when_needed() {
    let mut s = Session::new(1);
    assert!(!s.reset_background_opacity(), "already 100 %");
    set_bg_hex(&mut s, "11223340");
    let c = baseline(&mut s);
    assert!(s.reset_background_opacity());
    assert_eq!(changes(&s), c + 1);
    assert_eq!(s.background_view().hex, "#112233FF");
    assert_eq!(last_label(&s), "set_document_background");
}

#[test]
fn ac19_dragging_the_opacity_previews_and_commits_once_on_release() {
    let mut s = Session::new(1);
    let before = registers(&s);
    let c = baseline(&mut s);
    for p in [0.9, 0.7, 0.5, 0.25] {
        s.preview_background_opacity(p, Grid::Normal);
        assert_eq!(registers(&s), before, "preview is ephemeral");
    }
    let shown = s.background_view().opacity;
    assert!(shown < 100.0);
    assert!(
        s.frame_draw_list().checker_end() > 0,
        "the canvas shows the preview"
    );
    s.commit_background_preview();
    assert_eq!(changes(&s), c + 1, "one commit");
    assert_eq!(last_label(&s), "set_document_background");
    assert!((s.background_view().opacity - shown).abs() < 1e-9);
    // a second commit call without a preview writes nothing
    let o = ops(&s);
    s.commit_background_preview();
    assert_eq!(ops(&s), o);
}

#[test]
fn ac19_an_opacity_drag_back_to_the_stored_value_writes_nothing() {
    let mut s = Session::new(1);
    s.preview_background_opacity(0.5, Grid::Normal);
    s.preview_background_opacity(1.0, Grid::Normal);
    let o = ops(&s);
    s.commit_background_preview();
    assert_eq!(ops(&s), o);
}

// ------------------------------------------------ AC 20, 22, 23: the picker, previews

#[test]
fn ac20_ac22_a_picker_drag_shows_a_preview_and_commits_rgb_once_keeping_alpha() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "11223380");
    let before = registers(&s);
    let c = baseline(&mut s);
    for (h, sa, v) in [(10.0, 0.2, 0.9), (120.0, 0.9, 0.8), (240.0, 1.0, 1.0)] {
        s.preview_background_hsv(h, sa, v);
        assert_eq!(
            registers(&s),
            before,
            "stored registers equal the values before the press"
        );
        let doc = doc_of(&s);
        assert_eq!(
            doc.background(),
            solid(0x11, 0x22, 0x33, 128.0 / 255.0),
            "pack carries no preview"
        );
    }
    let hex = s.background_view().hex;
    assert!(
        hex.starts_with("#0000FF") && hex.ends_with("80"),
        "{hex}: blue at the old alpha"
    );
    s.commit_background_preview();
    assert_eq!(changes(&s), c + 1);
    assert_eq!(last_label(&s), "set_document_background");
    assert_eq!(s.background_view().hex, "#0000FF80");
    assert_eq!(
        stored(&s).opacity.get().to_bits(),
        (128.0_f64 / 255.0).to_bits(),
        "alpha untouched"
    );
}

#[test]
fn ac22_escape_or_cancel_drops_the_preview_and_writes_nothing() {
    let mut s = Session::new(1);
    let o = ops(&s);
    s.preview_background_hsv(200.0, 0.5, 0.5);
    assert_ne!(s.background_view().hex, "#E8E8EBFF");
    s.cancel_background_preview();
    assert_eq!(s.background_view().hex, "#E8E8EBFF");
    assert_eq!(ops(&s), o);
    s.preview_background_opacity(0.1, Grid::Normal);
    s.cancel_background_preview();
    assert_eq!(s.background_view().opacity, 100.0);
    assert_eq!(ops(&s), o);
    // the draw list falls back as well
    assert!(
        s.frame_draw_list().triangles[..6]
            .iter()
            .all(|v| v.color == CANVAS_BG)
    );
}

#[test]
fn ac22_the_system_cancel_of_a_pointer_drops_a_running_background_preview_or_leaves_it_to_the_host()
{
    // `pointer_cancelled` is the system cancel for tool drags. The host cancels the
    // picker drag with `cancel_background_preview`; this pins that a tool cancel never writes.
    let mut s = Session::new(1);
    s.preview_background_hsv(10.0, 1.0, 1.0);
    let o = ops(&s);
    s.pointer_cancelled();
    assert_eq!(ops(&s), o);
}

#[test]
fn ac22_a_preview_equal_to_the_stored_colour_commits_nothing() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "FF0000FF");
    s.preview_background_hsv(0.0, 1.0, 1.0);
    let o = ops(&s);
    s.commit_background_preview();
    assert_eq!(ops(&s), o);
}

#[test]
fn ac22_each_edit_writes_one_register_and_is_labelled() {
    let mut s = Session::new(1);
    let o = ops(&s);
    set_bg_hex(&mut s, "00FF00FF");
    assert_eq!(ops(&s), o + 1);
    assert_eq!(last_label(&s), "set_document_background");
    let (paint, _) = registers(&s);
    assert_eq!(paint, None, "the colour edit left the paint register alone");
}

#[test]
fn ac22_hostile_preview_values_do_not_panic_or_write() {
    let mut s = Session::new(1);
    let o = ops(&s);
    for (h, sa, v) in [
        (f64::NAN, 0.5, 0.5),
        (10.0, f64::NAN, 0.5),
        (10.0, 0.5, f64::NAN),
        (f64::INFINITY, 1.0, 1.0),
        (-720.0, 2.0, -1.0),
        (1e300, 1e300, 1e300),
    ] {
        s.preview_background_hsv(h, sa, v);
        let hex = s.background_view().hex;
        assert_eq!(hex.len(), 9, "{hex}");
        assert!(hex.starts_with('#'));
        s.cancel_background_preview();
    }
    for p in [f64::NAN, f64::INFINITY, -1.0, 2.0, 1e300] {
        for grid in [Grid::Normal, Grid::Coarse, Grid::Fine] {
            s.preview_background_opacity(p, grid);
            let o = s.background_view().opacity;
            assert!(o.is_finite() && (0.0..=100.0).contains(&o), "{p} -> {o}");
            s.cancel_background_preview();
        }
    }
    assert_eq!(ops(&s), o);
    // committing a hostile preview stores a valid colour
    s.preview_background_hsv(f64::NAN, f64::NAN, f64::NAN);
    s.commit_background_preview();
    let b = stored(&s);
    assert!(b.opacity.get().is_finite());
}

#[test]
fn ac22_held_arrow_key_steps_preview_and_commit_once() {
    let mut s = Session::new(1);
    let c = baseline(&mut s);
    for _ in 0..5 {
        s.step_background_opacity(-1, Grid::Normal);
    }
    assert_eq!(
        stored(&s).opacity.get(),
        1.0,
        "key held: nothing stored yet"
    );
    assert!(s.background_view().opacity < 100.0);
    s.commit_background_preview();
    assert_eq!(changes(&s), c + 1);
    assert!(
        (s.background_view().opacity - 95.0).abs() < 1e-6,
        "{}",
        s.background_view().opacity
    );
}

#[test]
fn ac23_a_running_edit_commits_to_the_document_when_the_section_leaves_the_tree() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(10.0, 10.0), pt(30.0, 30.0));
    s.escape();
    assert_eq!(s.panel_content(), PanelContent::Document);
    let before = registers(&s);
    s.preview_background_hsv(0.0, 1.0, 1.0);
    // selection changes through the keyboard-like path: a click on the rectangle's outline
    click(&mut s, pt(10.0, 20.0));
    assert_eq!(s.selected_object_count(), 1);
    assert_ne!(
        s.panel_content(),
        PanelContent::Document,
        "the Document section left the tree"
    );
    // A press on the canvas may flush the drag (as `begin_colour_pick` does) or leave it for the
    // release; either way the colour reaches the document, in at most one commit.
    let c = changes(&s);
    s.commit_background_preview();
    assert!(changes(&s) <= c + 1);
    assert_ne!(registers(&s), before);
    assert_eq!(
        stored(&s),
        solid(255, 0, 0, 1.0),
        "the drag still commits to the document"
    );
}

// ------------------------------------------------ AC 24: the subject line

#[test]
fn ac24_the_subject_line_does_not_depend_on_the_background() {
    let mut s = Session::new(1);
    assert_eq!(s.document_presets_view().subject, "A4, portrait");
    set_bg_hex(&mut s, "FF0000FF");
    assert_eq!(s.document_presets_view().subject, "A4, portrait");
    s.set_background_paint(BackgroundPaint::None);
    assert_eq!(s.document_presets_view().subject, "A4, portrait");
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Width, "123");
    assert_eq!(s.document_presets_view().subject, "Custom");
    set_bg_hex(&mut s, "00FF00FF");
    assert_eq!(s.document_presets_view().subject, "Custom");
}

// ------------------------------------------------ AC 13, 14, 47: the draw list

#[test]
fn ac13_the_background_is_drawn_first_an_object_fill_over_it_and_overlays_last() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "FF0000FF");
    draw_rect(&mut s, pt(50.0, 50.0), pt(90.0, 90.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "00FF00FF").unwrap();
    let list = s.frame_draw_list();
    let c = colours(&list);
    let red = RgbaColor::opaque(255, 0, 0);
    let green = RgbaColor::opaque(0, 255, 0);
    assert!(
        c[..6].iter().all(|x| *x == red),
        "the first quad is the red background"
    );
    let first_green = c
        .iter()
        .position(|x| *x == green)
        .expect("the green fill is drawn");
    assert!(first_green >= 6);
    assert!(list.layers().len() >= 2);
    // the selected rectangle's overlay (accent) comes after every artwork vertex
    let overlay = list.overlay_start();
    assert!(overlay >= first_green + 6, "overlay {overlay}");
    assert!(overlay <= list.triangles.len());
}

#[test]
fn ac13_an_object_with_fill_none_shows_the_background_through() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "FF0000FF");
    draw_rect(&mut s, pt(50.0, 50.0), pt(90.0, 90.0));
    s.escape();
    let list = s.frame_draw_list();
    let green_or_fill = colours(&list)
        .into_iter()
        .filter(|x| *x != RgbaColor::opaque(255, 0, 0) && *x != RgbaColor::BLACK)
        .count();
    assert_eq!(
        green_or_fill, 0,
        "only the red area and the black stroke are drawn"
    );
}

#[test]
fn ac14_every_edit_shows_in_the_draw_list_of_the_same_call() {
    let mut s = Session::new(1);
    assert!(s.set_background_hex("123456").unwrap());
    assert_eq!(
        s.frame_draw_list().triangles[0].color,
        RgbaColor::opaque(0x12, 0x34, 0x56)
    );
    assert!(s.set_background_paint(BackgroundPaint::None));
    assert!(s.frame_draw_list().checker_end() > 0);
    assert!(s.set_background_paint(BackgroundPaint::Solid));
    assert_eq!(s.frame_draw_list().checker_end(), 0);
    s.preview_background_opacity(0.5, Grid::Normal);
    assert!(
        s.frame_draw_list().checker_end() > 0,
        "the drag preview draws"
    );
    s.cancel_background_preview();
    assert_eq!(s.frame_draw_list().checker_end(), 0);
}

#[test]
fn ac47_the_empty_document_layer_counts_through_the_session() {
    let layers = |bg: &dyn Fn(&mut Session)| {
        let mut s = Session::new(1);
        bg(&mut s);
        let l = s.frame_draw_list();
        (l.layers().len(), l.checker_end(), l.triangles.len())
    };
    let default = layers(&|_| {});
    let opaque = layers(&|s| set_bg_hex(s, "123456FF"));
    let none = layers(&|s| {
        s.set_background_paint(BackgroundPaint::None);
    });
    let translucent = layers(&|s| set_bg_hex(s, "12345680"));
    assert_eq!(default, (1, 0, 6));
    assert_eq!(opaque, (1, 0, 6));
    assert_eq!(none.0, opaque.0);
    assert!(none.1 > 0);
    assert_eq!(translucent.0, opaque.0 + 1);
    assert!(translucent.1 > 0);
}

#[test]
fn ac47_the_session_draw_list_without_the_area_is_unchanged_by_the_background() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(10.0, 10.0), pt(20.0, 20.0));
    let a = s.draw_list();
    set_bg_hex(&mut s, "FF0000FF");
    s.set_background_paint(BackgroundPaint::None);
    let b = s.draw_list();
    assert_eq!(a.triangles.len(), b.triangles.len());
    assert_eq!(
        b.checker_end(),
        0,
        "the artwork list carries no checkerboard prefix"
    );
}

// ------------------------------------------------ AC 28 to 39: the eyedropper

fn hover_pick(s: &mut Session, p: Point) -> Option<(String, PaintTarget)> {
    s.pointer_hover(p, false, false);
    s.colour_pick_hover()
}

#[test]
fn ac28_objects_keep_priority_over_the_background_including_on_the_pasteboard() {
    let mut s = painted();
    set_bg_hex_unselected(&mut s, "FF0000FF");
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(10.0, 10.0)); // inside the green rectangle
    let st = styles(&s);
    assert_eq!(st[1].fill.color, Color { r: 0, g: 255, b: 0 });
    assert_eq!(st[1].fill.opacity.get(), 1.0);
    assert_eq!(s.colour_pick_target(), None, "picking ended");
}

/// The block exists only when nothing is selected; the model call is the same, so for set-up
/// the background is written straight into the document through the session's own command.
fn set_bg_hex_unselected(s: &mut Session, hex: &str) {
    assert!(s.set_background_hex(hex).unwrap());
}

#[test]
fn ac28_an_object_lying_partly_on_the_pasteboard_is_picked_there_too() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(-10.0, -10.0), pt(10.0, 10.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "0000FF").unwrap();
    s.set_stroke_paint(true);
    draw_rect(&mut s, pt(100.0, 100.0), pt(120.0, 120.0));
    s.set_fill_paint(true);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(-5.0, -5.0)); // on the pasteboard, inside the blue rectangle
    assert_eq!(styles(&s)[1].fill.color, Color { r: 0, g: 0, b: 255 });
}

#[test]
fn ac29_an_empty_document_point_takes_the_stored_colour_alpha_included() {
    let mut s = painted();
    s.set_background_hex("FF000080").unwrap();
    // 'painted' selected rectangle 1; its stroke is off
    s.set_stroke_paint(false);
    assert!(!styles(&s)[1].stroke.enabled);
    let c = baseline(&mut s);
    s.begin_colour_pick(PaintTarget::Stroke);
    click(&mut s, pt(150.0, 150.0));
    let st = &styles(&s)[1];
    assert_eq!(st.stroke.color, Color { r: 255, g: 0, b: 0 });
    assert_eq!(
        st.stroke.opacity.get().to_bits(),
        (128.0_f64 / 255.0).to_bits(),
        "not the composite"
    );
    assert!(st.stroke.enabled, "a stroke pick turns an off stroke on");
    assert_eq!(changes(&s), c + 1, "one commit");
    assert_eq!(s.colour_pick_target(), None);
    assert_eq!(
        stored(&s),
        solid(255, 0, 0, 128.0 / 255.0),
        "the background itself is unchanged"
    );
}

#[test]
fn ac29_a_fill_pick_from_the_background_never_turns_the_fill_on() {
    let mut s = painted();
    s.set_background_hex("FF000080").unwrap();
    assert!(!styles(&s)[1].fill.enabled);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(150.0, 150.0));
    let st = &styles(&s)[1];
    assert_eq!(st.fill.color, Color { r: 255, g: 0, b: 0 });
    assert!(!st.fill.enabled);
}

#[test]
fn ac29_a_pick_writes_to_every_selected_object_in_one_commit() {
    let mut s = painted();
    marquee(&mut s, pt(-5.0, -5.0), pt(65.0, 25.0));
    assert_eq!(s.selected_object_count(), 2);
    set_bg_hex_unselected_with_selection(&mut s);
    let c = baseline(&mut s);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(150.0, 150.0));
    assert_eq!(changes(&s), c + 1);
    for st in styles(&s) {
        assert_eq!(st.fill.color, Color { r: 1, g: 2, b: 3 });
    }
}

fn set_bg_hex_unselected_with_selection(s: &mut Session) {
    // The model command is available with a selection (the block is simply not shown).
    assert!(s.set_background_hex("010203").unwrap());
}

#[test]
fn ac30_the_pick_is_the_stored_value_at_every_zoom() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(100.0, 100.0), pt(120.0, 120.0));
    s.set_background_hex("336699A1").unwrap();
    let mut zooms = Vec::new();
    let mut picks = Vec::new();
    s.resize_viewport(1000.0, 800.0);
    for direction in [-1.0_f64, 1.0] {
        for _ in 0..80 {
            s.wheel(0.0, direction * 120.0, 500.0, 400.0, false, true);
        }
        zooms.push(s.zoom_percent());
        // select the rectangle so there is a target, then pick at a point on the empty page
        marquee(&mut s, pt(90.0, 90.0), pt(130.0, 130.0));
        s.begin_colour_pick(PaintTarget::Fill);
        s.pointer_down(pt(5.0, 5.0), false);
        s.pointer_up(pt(5.0, 5.0), false, false);
        let st = styles(&s);
        picks.push((st[0].fill.color, st[0].fill.opacity.get().to_bits()));
    }
    s.show_default_view();
    zooms.push(s.zoom_percent());
    marquee(&mut s, pt(90.0, 90.0), pt(130.0, 130.0));
    s.begin_colour_pick(PaintTarget::Fill);
    s.pointer_down(pt(5.0, 5.0), false);
    s.pointer_up(pt(5.0, 5.0), false, false);
    let st = styles(&s);
    picks.push((st[0].fill.color, st[0].fill.opacity.get().to_bits()));
    println!("zooms {zooms:?}");
    assert!(zooms.iter().any(|z| *z <= 5), "{zooms:?}");
    assert!(zooms.iter().any(|z| *z >= 4000), "{zooms:?}");
    let expected = (
        Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        },
        (161.0_f64 / 255.0).to_bits(),
    );
    assert!(picks.iter().all(|p| *p == expected), "{picks:?}");
}

#[test]
fn ac31_an_object_with_fill_none_lets_the_press_through_to_the_background_but_its_stroke_is_picked()
{
    let mut s = painted();
    // rectangle 1 (target) has fill off and a black stroke; make a third picker target elsewhere
    set_bg_hex_unselected(&mut s, "FF0000FF");
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(50.0, 10.0)); // interior of rectangle 1, away from its stroke
    assert_eq!(
        styles(&s)[1].fill.color,
        Color { r: 255, g: 0, b: 0 },
        "the background shows through"
    );
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(40.0, 10.0)); // on its stroke (black)
    assert_eq!(styles(&s)[1].fill.color, Color { r: 0, g: 0, b: 0 });
}

#[test]
fn ac31_a_fill_of_opacity_zero_counts_as_painted() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "11223300").unwrap();
    s.set_stroke_paint(false);
    draw_rect(&mut s, pt(40.0, 0.0), pt(60.0, 20.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "FFFFFF").unwrap();
    s.set_background_hex("FF0000FF").unwrap();
    marquee(&mut s, pt(35.0, -5.0), pt(65.0, 25.0));
    assert_eq!(s.selected_object_count(), 1);
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(10.0, 10.0));
    let st = &styles(&s)[1];
    assert_eq!(
        st.fill.color,
        Color {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
    assert_eq!(st.fill.opacity.get(), 0.0);
}

#[test]
fn ac32_a_none_background_picks_nothing_and_picking_stays_active() {
    let mut s = painted();
    s.set_background_paint(BackgroundPaint::None);
    let o = ops(&s);
    s.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(hover_pick(&mut s, pt(150.0, 150.0)), None, "No paint here");
    click(&mut s, pt(150.0, 150.0));
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Fill));
    assert_eq!(ops(&s), o);
    assert_eq!(s.take_colour_pick_announcement(), "");
}

#[test]
fn ac33_the_pasteboard_picks_nothing_and_picking_stays_active() {
    let mut s = painted();
    let o = ops(&s);
    s.begin_colour_pick(PaintTarget::Fill);
    for p in [
        pt(-1.0, 100.0),
        pt(100.0, -1.0),
        pt(211.0, 100.0),
        pt(100.0, 298.0),
        pt(-1e9, 1e9),
        pt(210.000_001, 100.0),
        pt(-0.000_001, 100.0),
    ] {
        assert_eq!(hover_pick(&mut s, p), None, "{p:?}");
        click(&mut s, p);
        assert_eq!(s.colour_pick_target(), Some(PaintTarget::Fill), "{p:?}");
    }
    assert_eq!(ops(&s), o);
}

#[test]
fn ac33_ac34_the_document_edges_are_inside_with_no_tolerance() {
    let mut s = painted();
    s.begin_colour_pick(PaintTarget::Fill);
    for p in [
        pt(0.0, 100.0),
        pt(210.0, 100.0),
        pt(100.0, 0.0),
        pt(100.0, 297.0),
        pt(210.0, 297.0),
        pt(0.0, 297.0),
        pt(210.0, 0.0),
    ] {
        let hit = hover_pick(&mut s, p);
        assert_eq!(
            hit,
            Some(("#E8E8EBFF".to_string(), PaintTarget::Background)),
            "{p:?}"
        );
    }
    // the corner (0,0) lies on rectangle 0's stroke corner, so use the other three; the
    // outside is exact:
    for p in [
        pt(-1e-9, 100.0),
        pt(210.0 + 1e-9, 100.0),
        pt(100.0, -1e-9),
        pt(100.0, 297.0 + 1e-9),
    ] {
        assert_eq!(hover_pick(&mut s, p), None, "{p:?}");
    }
}

#[test]
fn ac34_hostile_pointer_values_pick_nothing_and_do_not_panic() {
    let mut s = painted();
    s.begin_colour_pick(PaintTarget::Fill);
    for p in [
        pt(f64::NAN, 5.0),
        pt(5.0, f64::NAN),
        pt(f64::INFINITY, f64::INFINITY),
        pt(f64::NEG_INFINITY, 5.0),
        pt(f64::MAX, f64::MAX),
        pt(f64::MIN, f64::MIN),
    ] {
        assert_eq!(hover_pick(&mut s, p), None, "{p:?}");
    }
    let o = ops(&s);
    s.pointer_down(pt(f64::NAN, f64::NAN), false);
    s.pointer_up(pt(f64::NAN, f64::NAN), false, false);
    assert_eq!(ops(&s), o);
}

#[test]
fn ac35_the_hover_names_the_background_and_the_stored_colour() {
    let mut s = painted();
    s.set_background_hex("11223380").unwrap();
    s.begin_colour_pick(PaintTarget::Stroke);
    assert_eq!(
        hover_pick(&mut s, pt(150.0, 150.0)),
        Some(("#11223380".to_string(), PaintTarget::Background))
    );
    assert_eq!(
        hover_pick(&mut s, pt(10.0, 10.0)),
        Some(("#00FF00FF".to_string(), PaintTarget::Fill))
    );
    assert_eq!(
        hover_pick(&mut s, pt(0.0, 10.0)).map(|x| x.1),
        Some(PaintTarget::Stroke)
    );
    assert_eq!(
        hover_pick(&mut s, pt(-50.0, 10.0)),
        None,
        "pasteboard: No paint here"
    );
}

#[test]
fn ac35_a_pan_or_zoom_under_a_resting_pointer_refreshes_the_hover() {
    let mut s = painted();
    s.resize_viewport(1000.0, 800.0);
    s.begin_colour_pick(PaintTarget::Stroke);
    let p = s.screen_to_document(500.0, 400.0);
    s.pointer_hover(p, false, false);
    let first = s.colour_pick_hover();
    s.wheel(300.0, 0.0, 500.0, 400.0, false, false);
    let second = s.colour_pick_hover();
    println!("hover before {first:?} after pan {second:?}");
    let expected = {
        let q = s.screen_to_document(500.0, 400.0);
        s.pointer_hover(q, false, false);
        s.colour_pick_hover()
    };
    assert_eq!(second, expected);
}

#[test]
fn ac36_the_background_button_picks_for_the_background_and_ends_like_any_pick() {
    let mut s = Session::new(1);
    s.begin_colour_pick(PaintTarget::Background);
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Background));
    assert!(s.background_view().picking);
    s.begin_colour_pick(PaintTarget::Background);
    assert_eq!(
        s.colour_pick_target(),
        None,
        "a second press on the button ends picking"
    );
    assert!(!s.background_view().picking);
    // Escape, tool change
    let o = ops(&s);
    s.begin_colour_pick(PaintTarget::Background);
    s.escape();
    assert_eq!(s.colour_pick_target(), None);
    s.begin_colour_pick(PaintTarget::Background);
    s.set_tool(Tool::Rectangle);
    assert_eq!(s.colour_pick_target(), None);
    s.set_tool(Tool::Select);
    s.begin_colour_pick(PaintTarget::Background);
    s.end_colour_pick();
    assert_eq!(s.colour_pick_target(), None);
    assert_eq!(ops(&s), o, "ending writes nothing");
    // pan and zoom keep picking
    s.begin_colour_pick(PaintTarget::Background);
    s.wheel(10.0, 10.0, 100.0, 100.0, false, false);
    s.wheel(0.0, 120.0, 100.0, 100.0, false, true);
    s.begin_pan(10.0, 10.0);
    s.pan_to(40.0, 40.0);
    s.end_pan();
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Background));
}

#[test]
fn ac36_pressing_another_eyedropper_switches_the_target() {
    let mut s = painted();
    s.begin_colour_pick(PaintTarget::Stroke);
    s.begin_colour_pick(PaintTarget::Background);
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Background));
}

#[test]
fn ac37_a_background_pick_takes_an_object_colour_alpha_included_in_one_commit() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.set_stroke_paint(true);
    s.set_style_text(StyleField::StrokeColor, "0000FFCC")
        .unwrap();
    s.escape();
    let c = baseline(&mut s);
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(0.0, 10.0)); // the stroke
    assert_eq!(changes(&s), c + 1);
    assert_eq!(last_label(&s), "set_document_background");
    assert_eq!(s.background_view().hex, "#0000FFCC");
    assert_eq!(
        stored(&s).opacity.get().to_bits(),
        (204.0_f64 / 255.0).to_bits()
    );
    assert_eq!(s.colour_pick_target(), None);
    // pressing on the empty area afterwards, with a stroke eyedropper, gives the new value
    marquee(&mut s, pt(-5.0, -5.0), pt(25.0, 25.0));
    s.begin_colour_pick(PaintTarget::Stroke);
    click(&mut s, pt(150.0, 150.0));
    let st = &styles(&s)[0];
    assert_eq!(st.stroke.color, Color { r: 0, g: 0, b: 255 });
    assert_eq!(
        st.stroke.opacity.get().to_bits(),
        (204.0_f64 / 255.0).to_bits()
    );
}

#[test]
fn ac37_picking_the_backgrounds_own_colour_writes_nothing_and_still_ends() {
    let mut s = Session::new(1);
    let (o, c) = (ops(&s), changes(&s));
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(100.0, 100.0));
    assert_eq!(s.colour_pick_target(), None);
    assert_eq!((ops(&s), changes(&s)), (o, c));
    assert_eq!(registers(&s), (None, None));
}

#[test]
fn ac37_a_pick_of_alpha_zero_leaves_a_solid_paint_showing_the_checkerboard() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.set_fill_paint(true);
    s.set_style_text(StyleField::FillColor, "44556600").unwrap();
    s.escape();
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(10.0, 10.0));
    let b = stored(&s);
    assert_eq!(b.paint, BackgroundPaint::Solid);
    assert_eq!(
        b.color,
        Color {
            r: 0x44,
            g: 0x55,
            b: 0x66
        }
    );
    assert_eq!(b.opacity.get(), 0.0);
    assert!(
        s.frame_draw_list().checker_end() > 0,
        "the checkerboard shows through"
    );
}

#[test]
fn ac37_a_background_pick_keeps_the_paint_solid_even_from_none() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.escape();
    s.set_background_paint(BackgroundPaint::None);
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(0.0, 10.0)); // black stroke
    assert_eq!(stored(&s).paint, BackgroundPaint::Solid);
    assert_eq!(s.background_view().hex, "#000000FF");
}

#[test]
fn ac37_nothing_is_picked_for_the_background_over_none_or_the_pasteboard() {
    let mut s = Session::new(1);
    s.set_background_paint(BackgroundPaint::None);
    s.begin_colour_pick(PaintTarget::Background);
    let o = ops(&s);
    click(&mut s, pt(100.0, 100.0));
    click(&mut s, pt(-50.0, 100.0));
    assert_eq!(s.colour_pick_target(), Some(PaintTarget::Background));
    assert_eq!(ops(&s), o);
}

#[test]
fn ac38_a_press_while_picking_the_background_selects_nothing_moves_nothing_draws_nothing() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.escape();
    assert_eq!(s.panel_content(), PanelContent::Document);
    let before = doc_of(&s).object_ids().len();
    s.begin_colour_pick(PaintTarget::Background);
    s.pointer_hover(pt(10.0, 0.0), false, false);
    s.pointer_down(pt(10.0, 0.0), false);
    s.pointer_hover(pt(80.0, 80.0), false, false);
    s.pointer_up(pt(80.0, 80.0), false, false);
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(doc_of(&s).object_ids().len(), before);
    assert_eq!(
        s.panel_content(),
        PanelContent::Document,
        "the Document section stays"
    );
    // tools draw nothing while picking
    s.begin_colour_pick(PaintTarget::Background);
    s.set_tool(Tool::Rectangle);
    assert_eq!(s.colour_pick_target(), None);
}

#[test]
fn ac39_the_announcement_is_said_once_with_the_stored_value() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(0.0, 0.0), pt(20.0, 20.0));
    s.set_stroke_paint(true);
    s.set_style_text(StyleField::StrokeColor, "0000FFCC")
        .unwrap();
    s.escape();
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(0.0, 10.0));
    assert_eq!(
        s.take_colour_pick_announcement(),
        "Background color set to #0000FFCC"
    );
    assert_eq!(s.take_colour_pick_announcement(), "", "once");
    // a pick into an object also announces once
    marquee(&mut s, pt(-5.0, -5.0), pt(25.0, 25.0));
    s.begin_colour_pick(PaintTarget::Fill);
    click(&mut s, pt(150.0, 150.0));
    let said = s.take_colour_pick_announcement();
    assert!(!said.is_empty(), "a background source announces too");
    assert_eq!(s.take_colour_pick_announcement(), "");
    // an equal pick still says what the colour is, or says nothing, but never panics
    s.escape();
    s.begin_colour_pick(PaintTarget::Background);
    click(&mut s, pt(150.0, 150.0));
    let _ = s.take_colour_pick_announcement();
}

// ------------------------------------------------ AC 40 to 44: not an object

#[test]
fn ac40_clicks_and_marquees_over_the_document_select_nothing_from_the_background() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "FF0000FF");
    s.set_tool(Tool::Select);
    click(&mut s, pt(100.0, 100.0));
    assert_eq!(s.selected_object_count(), 0);
    marquee(&mut s, pt(-50.0, -50.0), pt(500.0, 500.0));
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(s.panel_content(), PanelContent::Document);
    assert!(!s.has_objects());
    assert_eq!(
        s.fit_document(),
        FitOutcome::Empty,
        "Fit to content sees no content"
    );
    // a click on empty area clears an existing selection as before
    draw_rect(&mut s, pt(10.0, 10.0), pt(30.0, 30.0));
    click(&mut s, pt(10.0, 20.0));
    assert_eq!(s.selected_object_count(), 1);
    click(&mut s, pt(150.0, 150.0));
    assert_eq!(s.selected_object_count(), 0);
}

#[test]
fn ac40_delete_move_and_the_keys_find_nothing_to_act_on() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "FF0000FF");
    let o = ops(&s);
    s.delete_selected();
    s.escape();
    assert_eq!(ops(&s), o);
    assert_eq!(stored(&s), solid(255, 0, 0, 1.0));
}

#[test]
fn ac41_tools_work_over_a_background_exactly_as_over_the_default() {
    let run = |hex: Option<&str>, paint_none: bool| {
        let mut s = Session::new(1);
        if let Some(h) = hex {
            set_bg_hex(&mut s, h);
        }
        if paint_none {
            s.set_background_paint(BackgroundPaint::None);
        }
        draw_rect(&mut s, pt(10.3, 10.7), pt(40.1, 50.9));
        s.set_tool(Tool::Ellipse);
        s.pointer_down(pt(60.0, 60.0), false);
        s.pointer_up(pt(90.0, 80.0), false, false);
        s.set_tool(Tool::Pen);
        for p in [pt(100.0, 100.0), pt(150.0, 100.0), pt(150.0, 150.0)] {
            s.pointer_down(p, false);
            s.pointer_up(p, false, false);
        }
        s.finish_pen();
        let d = doc_of(&s);
        let mut j: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
        j.as_object_mut().unwrap().remove("background");
        // the background edit itself consumes one operation counter, so ids shift; compare the rest
        for o in j["objects"].as_array_mut().unwrap() {
            o.as_object_mut().unwrap().remove("id");
        }
        j
    };
    let base = run(None, false);
    assert_eq!(base["objects"].as_array().unwrap().len(), 3);
    assert_eq!(run(Some("FF0000FF"), false), base);
    assert_eq!(run(Some("00000080"), false), base);
    assert_eq!(run(None, true), base);
}

#[test]
fn ac42_a_resize_keeps_the_background_and_covers_the_new_area() {
    let mut a = Session::new(1);
    draw_rect(&mut a, pt(10.0, 10.0), pt(60.0, 60.0));
    let mut b = Session::new(1);
    draw_rect(&mut b, pt(10.0, 10.0), pt(60.0, 60.0));
    set_bg_hex(&mut a, "FF0000FF");
    let regs = registers(&a);
    let (oa, ob) = (ops(&a), ops(&b));
    assert_eq!(a.apply_document_preset("a3"), SizeOutcome::Committed);
    assert_eq!(b.apply_document_preset("a3"), SizeOutcome::Committed);
    assert_eq!(registers(&a), regs, "the registers are unchanged");
    assert_eq!(
        ops(&a) - oa,
        ops(&b) - ob,
        "the commit changed only size and positions"
    );
    let l = a.frame_draw_list();
    let xs = l.triangles[..6].iter().map(|v| v.position.x);
    let ys = l.triangles[..6].iter().map(|v| v.position.y);
    let (w, h) = (
        xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min),
        ys.clone().fold(f64::MIN, f64::max) - ys.fold(f64::MAX, f64::min),
    );
    assert!((w - 297.0).abs() < 0.6 || (w - 420.0).abs() < 0.6, "{w}");
    assert!((h - 420.0).abs() < 0.6 || (h - 297.0).abs() < 0.6, "{h}");
    assert!(
        l.triangles[..6]
            .iter()
            .all(|v| v.color == RgbaColor::opaque(255, 0, 0))
    );
}

#[test]
fn ac42_typed_sizes_fit_and_orientation_never_write_a_background_register() {
    let mut s = Session::new(1);
    draw_rect(&mut s, pt(10.0, 10.0), pt(60.0, 60.0));
    set_bg_hex(&mut s, "00FF0080");
    s.set_background_paint(BackgroundPaint::None);
    let regs = registers(&s);
    let before = stored(&s);
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Width, "400");
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Height, "123");
    assert_eq!(registers(&s), regs);
    assert_eq!(s.fit_document(), FitOutcome::Fitted);
    assert_eq!(registers(&s), regs);
    s.set_display_unit(DisplayUnit::In);
    s.apply_document_preset("a5");
    s.set_document_orientation(curvyo_document_core::Orientation::Landscape);
    assert_eq!(registers(&s), regs);
    assert_eq!(stored(&s), before);
}

// ------------------------------------------------ AC 46: the Pen close marker

fn pen_close_hover_colours(setup: impl FnOnce(&mut Session), at: [Point; 3]) -> Vec<RgbaColor> {
    let mut s = Session::new(1);
    setup(&mut s);
    s.set_tool(Tool::Pen);
    for p in at {
        s.pointer_down(p, false);
        s.pointer_up(p, false, false);
    }
    s.pointer_hover(at[0], false, false);
    assert!(s.pen_target().is_some());
    colours(&s.draw_list())
}

#[test]
fn ac46_the_close_target_marker_is_filled_with_the_colour_behind_it() {
    let at = [pt(50.0, 50.0), pt(100.0, 50.0), pt(100.0, 100.0)];
    let red = pen_close_hover_colours(|s| set_bg_hex(s, "FF0000FF"), at);
    assert!(
        red.contains(&RgbaColor::opaque(255, 0, 0)),
        "opaque background colour"
    );
    assert!(!red.contains(&CANVAS_BG));
    let default = pen_close_hover_colours(|_| {}, at);
    assert!(default.contains(&CANVAS_BG));
    let none = pen_close_hover_colours(
        |s| {
            s.set_background_paint(BackgroundPaint::None);
        },
        at,
    );
    assert!(none.contains(&CHECKER_A), "None: checker-a");
    let translucent = pen_close_hover_colours(|s| set_bg_hex(s, "FF000080"), at);
    assert!(
        translucent
            .iter()
            .any(|c| c.a == 255 && c.r == 255 && c.g.abs_diff(127) <= 1 && c.b.abs_diff(127) <= 1),
        "translucent red over checker-a"
    );
}

#[test]
fn ac46_over_the_pasteboard_it_is_as_before() {
    let at = [pt(-50.0, 50.0), pt(-20.0, 50.0), pt(-20.0, 100.0)];
    let c = pen_close_hover_colours(|s| set_bg_hex(s, "FF0000FF"), at);
    assert!(c.contains(&PASTEBOARD_BG));
}

// ------------------------------------------------ AC 43, 44: unit change, clipboard

#[test]
fn ac43_a_unit_change_leaves_the_background_alone() {
    let mut s = Session::new(1);
    set_bg_hex(&mut s, "ABCDEF12");
    let before = (registers(&s), stored(&s));
    for u in [DisplayUnit::In, DisplayUnit::Cm, DisplayUnit::Mm] {
        s.set_display_unit(u);
    }
    assert_eq!((registers(&s), stored(&s)), before);
}

// ------------------------------------------------ white-box: the draw list with a prefix

#[test]
fn whitebox_a_checkerboard_prefix_keeps_the_layer_and_depth_bookkeeping_coherent() {
    for hex in [None, Some("12345680")] {
        let mut s = Session::new(1);
        match hex {
            None => {
                s.set_background_paint(BackgroundPaint::None);
            }
            Some(h) => set_bg_hex(&mut s, h),
        }
        draw_rect(&mut s, pt(10.0, 10.0), pt(60.0, 60.0));
        s.set_fill_paint(true);
        draw_rect(&mut s, pt(70.0, 70.0), pt(90.0, 90.0));
        let list = s.frame_draw_list();
        let layers = list.layers().to_vec();
        assert!(
            layers.windows(2).all(|w| w[0] < w[1]),
            "ascending {layers:?}"
        );
        assert_eq!(list.checker_end(), layers[0], "the prefix is layer 0");
        assert!(list.checker_end() > 0);
        assert!(list.overlay_start() >= *layers.last().unwrap());
        assert!(list.overlay_start() <= list.triangles.len());
        assert_eq!(list.triangles.len() % 3, 0);
        let depths = list.vertex_depths();
        assert_eq!(depths.len(), list.triangles.len());
        assert!(depths.iter().all(|d| d.is_finite()));
        // the vertices of one layer share one depth, and the layers differ
        let mut from = 0;
        let mut seen = Vec::new();
        for end in &layers {
            let d = depths[from];
            assert!(
                depths[from..*end].iter().all(|x| *x == d),
                "layer {from}..{end}"
            );
            seen.push(d);
            from = *end;
        }
        let mut sorted = seen.clone();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            seen.len(),
            "distinct depth per layer: {seen:?}"
        );
        // the document area is behind everything
        assert!(
            seen[1..].iter().all(|d| *d != seen[0]),
            "artwork has its own depth"
        );
    }
}

#[test]
fn whitebox_the_artwork_list_of_the_session_is_never_given_a_prefix() {
    let mut s = Session::new(1);
    s.set_background_paint(BackgroundPaint::None);
    draw_rect(&mut s, pt(10.0, 10.0), pt(60.0, 60.0));
    let art = s.draw_list();
    assert_eq!(art.checker_end(), 0);
    // two frames in a row give the same list (no state leaks between frames)
    let a = s.frame_draw_list();
    let b = s.frame_draw_list();
    assert_eq!(a.triangles, b.triangles);
    assert_eq!(a.layers(), b.layers());
}
