//! `Session`-level checks of the document size presets
//! (`specs/0030-document-size-presets/` criteria 10 to 14, 18 to 21): a press is
//! the typed resize with another size, the orientation rule, the unchanged size
//! writes nothing, and nothing about a preset reaches the file.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    DisplayUnit, Document, DocumentSize, Length, ObjectSnapshot, Orientation, Point, RectBounds,
    Shape, pack, unpack,
};
use curvyo_editor_wasm::{Session, SizeOutcome, Tool};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A session over an A4 document holding the rectangle of `0015` (x = 10, y =
/// 10, 50 x 50).
fn session_with_rect() -> Session {
    session_with_size(210.0, 297.0)
}

fn session_with_size(width: f64, height: f64) -> Session {
    let document = Document::new(1);
    document
        .resize(DocumentSize::from_mm(width, height))
        .unwrap();
    let _ = document.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: Length::from_mm(50.0),
        height: Length::from_mm(50.0),
    });
    Session::open(2, &pack(&document, "0.1.0").unwrap()).unwrap()
}

fn reopened(session: &Session) -> Document {
    unpack(99, &session.pack("0.1.0").unwrap()).unwrap()
}

fn rect(session: &Session) -> (f64, f64, f64, f64) {
    let document = reopened(session);
    let id = document.object_ids()[0];
    let Some(ObjectSnapshot::Primitive(primitive)) = document.object(id) else {
        panic!("a primitive");
    };
    let Shape::Rect { bounds, .. } = primitive.shape else {
        panic!("a rectangle");
    };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.width.as_mm(),
        bounds.height.as_mm(),
    )
}

fn size(session: &Session) -> (f64, f64) {
    let size = reopened(session).size();
    (size.width.as_mm(), size.height.as_mm())
}

fn changes(session: &Session) -> usize {
    let loro = loro::LoroDoc::new();
    loro.import(&reopened(session).export_loro_snapshot().unwrap())
        .unwrap();
    loro.len_changes()
}

fn pressed(session: &Session) -> Vec<String> {
    let view = session.document_presets_view();
    view.groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| e.pressed)
        .map(|e| e.name.clone())
        .collect()
}

/// Criterion 11: the objects move by half the change, from the spec's example.
#[test]
fn a_pick_resizes_around_the_centre_and_the_objects_keep_their_size() {
    let mut s = session_with_rect();
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Committed);
    assert_eq!(size(&s), (297.0, 420.0));
    assert_eq!(rect(&s), (53.5, 71.5, 50.0, 50.0));
    assert_eq!(s.apply_document_preset("a5"), SizeOutcome::Committed);
    assert_eq!(size(&s), (148.0, 210.0));
    assert_eq!(rect(&s), (-21.0, -33.5, 50.0, 50.0));
    let mut s = session_with_rect();
    s.apply_document_preset("slide-16-9");
    assert_eq!(size(&s), (508.0, 285.75));
    assert_eq!(rect(&s), (159.0, 4.375, 50.0, 50.0));
}

/// Criterion 12.
#[test]
fn pressing_the_size_the_document_already_has_writes_nothing() {
    let mut s = session_with_rect();
    let before = changes(&s);
    assert_eq!(s.apply_document_preset("a4"), SizeOutcome::Unchanged);
    assert_eq!(changes(&s), before);
    // 210.004 is shown selected; the press sets the exact preset size.
    let mut near = session_with_size(210.004, 297.0);
    assert_eq!(pressed(&near), ["A4"]);
    assert_eq!(near.apply_document_preset("a4"), SizeOutcome::Committed);
    assert_eq!(size(&near), (210.0, 297.0));
}

/// Criterion 13's table, through the session.
#[test]
fn the_orientation_of_a_pick_follows_the_rule() {
    for (start, id, want) in [
        ((297.0, 210.0), "a3", (420.0, 297.0)),
        ((210.0, 297.0), "slide-16-9", (508.0, 285.75)),
        ((300.0, 400.0), "a4", (210.0, 297.0)),
        ((400.0, 300.0), "a4", (210.0, 297.0)),
        ((285.75, 508.0), "slide-16-10", (317.5, 508.0)),
    ] {
        let mut s = session_with_size(start.0, start.1);
        s.apply_document_preset(id);
        assert_eq!(size(&s), want, "{start:?} then {id}");
    }
}

/// Criterion 14.
#[test]
fn the_orientation_items_swap_the_sides_around_the_centre() {
    let mut s = session_with_rect();
    assert_eq!(
        s.document_presets_view().orientation,
        Some(Orientation::Portrait)
    );
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Committed
    );
    assert_eq!(size(&s), (297.0, 210.0));
    assert_eq!(rect(&s), (53.5, -33.5, 50.0, 50.0));
    assert_eq!(
        s.document_presets_view().orientation,
        Some(Orientation::Landscape)
    );
    let before = changes(&s);
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Unchanged
    );
    assert_eq!(changes(&s), before, "the pressed item writes nothing");
    let mut square = session_with_size(100.0, 100.0);
    assert_eq!(square.document_presets_view().orientation, None);
    assert_eq!(
        square.set_document_orientation(Orientation::Portrait),
        SizeOutcome::Unchanged
    );
    assert_eq!(
        square.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Unchanged
    );
    let mut custom = session_with_size(300.0, 400.0);
    custom.set_document_orientation(Orientation::Landscape);
    assert_eq!(size(&custom), (400.0, 300.0));
}

/// Criteria 10 and 10a follow every change of the size in the same call chain
/// as the fields.
#[test]
fn the_pressed_preset_and_the_subject_follow_a_pick_a_typed_size_and_a_fit() {
    let mut s = Session::new(1);
    assert_eq!(s.document_presets_view().subject, "A4, portrait");
    assert_eq!(pressed(&s), ["A4"]);
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Width, "297");
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Height, "210");
    assert_eq!(s.document_presets_view().subject, "A4, landscape");
    s.set_document_side(curvyo_editor_wasm::DocumentSide::Width, "211");
    assert_eq!(s.document_presets_view().subject, "Custom");
    assert!(pressed(&s).is_empty());
    s.apply_document_preset("slide-16-9");
    assert_eq!(s.document_presets_view().subject, "16:9, landscape");
    assert_eq!(pressed(&s), ["16:9"]);
}

/// Criterion 17 in the display unit of the document.
#[test]
fn the_tooltips_follow_the_display_unit() {
    let mut s = Session::new(1);
    let tip = |s: &Session, n: usize| {
        s.document_presets_view().groups[0].entries[n]
            .tooltip
            .clone()
    };
    assert_eq!(tip(&s, 4), "210 \u{d7} 297 mm\nISO 216");
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(tip(&s, 4), "8.2677 \u{d7} 11.6929 in\nISO 216");
}

/// Criterion 18: a pick is ignored while the Pen has an unfinished path.
#[test]
fn a_pick_is_ignored_while_the_pen_has_an_unfinished_path() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Pen);
    for point in [pt(10.0, 10.0), pt(50.0, 40.0)] {
        s.pointer_down(point, false);
        s.pointer_up(point, false, false);
    }
    assert_eq!(s.apply_document_preset("a3"), SizeOutcome::Unchanged);
    assert_eq!(
        s.set_document_orientation(Orientation::Landscape),
        SizeOutcome::Unchanged
    );
    assert_eq!(size(&s), (210.0, 297.0));
}

#[test]
fn an_unknown_id_is_invalid_and_writes_nothing() {
    let mut s = session_with_rect();
    let before = changes(&s);
    assert_eq!(s.apply_document_preset("letter"), SizeOutcome::Invalid);
    assert_eq!(changes(&s), before);
}

/// Criteria 19 and 20: a pick survives save and open, no id in the file, and a
/// project of another size opens with nothing selected and nothing written.
#[test]
fn a_pick_survives_save_and_open_and_the_file_holds_no_preset_id() {
    let mut s = Session::new(1);
    s.apply_document_preset("slide-16-9");
    let bytes = s.pack("0.1.0").unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    for name in ["document.json", "manifest.json"] {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut text).unwrap();
        assert!(
            !text.contains("slide-16-9") && !text.contains("preset"),
            "{name}"
        );
    }
    let reopened = Session::open(3, &bytes).unwrap();
    assert_eq!(pressed(&reopened), ["16:9"]);
    assert_eq!(
        reopened.side_text(curvyo_editor_wasm::DocumentSide::Width),
        "508"
    );
    assert_eq!(
        reopened.side_text(curvyo_editor_wasm::DocumentSide::Height),
        "285.75"
    );
}

#[test]
fn a_custom_size_opens_with_no_preset_and_nothing_is_written_by_opening() {
    let custom = session_with_size(123.0, 456.0);
    assert!(pressed(&custom).is_empty());
    assert_eq!(custom.document_presets_view().subject, "Custom");
}
