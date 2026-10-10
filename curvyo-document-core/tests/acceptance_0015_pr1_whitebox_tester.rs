//! Tester white-box tests for PR 1 of `specs/0015-document-size-and-rulers`,
//! written after reading the diff: damaged stored sizes and units (criterion
//! 13), a baseline of what earlier builds exported (criterion 13: "every
//! object where it was"), and the view of a version-7 reader (criterion 38).
//!
//! `fixtures/baseline_pre_0015_export.json` holds `export_json` as produced
//! by the build at `2a83559` (before this story) for every older fixture.
//! `fixtures/*resaved_by_v7_reader.curvyo` were written by that same build
//! after opening this story's two new fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, DisplayUnit, Document, DocumentSize, Length, Point, RectBounds, unpack,
};
use loro::{LoroDoc, LoroValue};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn a4() -> DocumentSize {
    DocumentSize::from_mm(210.0, 297.0)
}

fn zip_from_loro(loro_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": 1,
        "app_version": "tester-damage",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    writer.finish().unwrap().into_inner()
}

/// A project file whose root registers `size_width_mm`, `size_height_mm` and
/// `display_unit` are set by `edit` (a hand-damaged file). It also holds one
/// rectangle at (10, 10), 50 x 50 mm.
fn damaged(edit: impl FnOnce(&loro::LoroMap)) -> Vec<u8> {
    let base = Document::new(1);
    let _ = base.create_rect(RectBounds {
        origin: Point::new(10.0, 10.0),
        width: Length::from_mm(50.0),
        height: Length::from_mm(50.0),
    });
    let loro = LoroDoc::new();
    loro.import(&base.export_loro_snapshot().unwrap()).unwrap();
    edit(&loro.get_map("root"));
    loro.commit();
    zip_from_loro(&loro.export(loro::ExportMode::Snapshot).unwrap())
}

fn change_count(d: &Document) -> usize {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l.len_changes()
}

fn rect_origin(d: &Document) -> Point {
    let id = d.object_ids()[0];
    let Some(curvyo_document_core::ObjectSnapshot::Primitive(p)) = d.object(id) else {
        panic!()
    };
    let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    bounds.origin
}

/// AC 13: every kind of damaged size opens at A4 on both axes, is not
/// refused, writes nothing and keeps the objects where they are.
#[test]
fn damaged_sizes_open_at_a4_without_writing() {
    type Edit = Box<dyn Fn(&loro::LoroMap)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "width missing",
            Box::new(|m| m.delete("size_width_mm").unwrap()),
        ),
        (
            "height missing",
            Box::new(|m| m.delete("size_height_mm").unwrap()),
        ),
        (
            "both missing",
            Box::new(|m| {
                m.delete("size_width_mm").unwrap();
                m.delete("size_height_mm").unwrap();
            }),
        ),
        (
            "width NaN",
            Box::new(|m| m.insert("size_width_mm", f64::NAN).unwrap()),
        ),
        (
            "height +inf",
            Box::new(|m| m.insert("size_height_mm", f64::INFINITY).unwrap()),
        ),
        (
            "height -inf",
            Box::new(|m| m.insert("size_height_mm", f64::NEG_INFINITY).unwrap()),
        ),
        (
            "width zero, height fine",
            Box::new(|m| {
                m.insert("size_width_mm", 0.0).unwrap();
                m.insert("size_height_mm", 400.0).unwrap();
            }),
        ),
        (
            "width negative",
            Box::new(|m| m.insert("size_width_mm", -210.0).unwrap()),
        ),
        (
            "width just below 1 mm",
            Box::new(|m| m.insert("size_width_mm", 0.999_999).unwrap()),
        ),
        (
            "width a hair below 1 mm",
            Box::new(|m| m.insert("size_width_mm", 1.0 - 1e-12).unwrap()),
        ),
        (
            "height just above the maximum",
            Box::new(|m| m.insert("size_height_mm", 100_000.000_1).unwrap()),
        ),
        (
            "width 1e300",
            Box::new(|m| m.insert("size_width_mm", 1e300).unwrap()),
        ),
        (
            "width f64::MAX",
            Box::new(|m| m.insert("size_width_mm", f64::MAX).unwrap()),
        ),
        (
            "width stored as a string",
            Box::new(|m| m.insert("size_width_mm", "300").unwrap()),
        ),
        (
            "width stored as an integer",
            Box::new(|m| m.insert("size_width_mm", 300_i64).unwrap()),
        ),
        (
            "height stored as a bool",
            Box::new(|m| m.insert("size_height_mm", true).unwrap()),
        ),
    ];
    for (what, edit) in cases {
        let bytes = damaged(|m| edit(m));
        let d = unpack(3, &bytes).unwrap_or_else(|e| panic!("{what}: refused {e:?}"));
        let n = change_count(&d);
        assert_eq!(d.size(), a4(), "{what}");
        assert_eq!(d.size(), a4(), "{what} (second read)");
        assert_eq!(change_count(&d), n, "{what}: reading must not write");
        assert_eq!(rect_origin(&d), Point::new(10.0, 10.0), "{what}");
        assert_eq!(d.display_unit(), DisplayUnit::Mm, "{what}");
    }
}

/// AC 13 boundaries: 1 mm and 100 000 mm are valid as stored.
#[test]
fn stored_sizes_on_the_limits_are_kept() {
    for (w, h) in [
        (1.0, 1.0),
        (100_000.0, 100_000.0),
        (1.0, 100_000.0),
        (215.9, 279.4),
    ] {
        let bytes = damaged(|m| {
            m.insert("size_width_mm", w).unwrap();
            m.insert("size_height_mm", h).unwrap();
        });
        let d = unpack(3, &bytes).unwrap();
        assert_eq!(d.size(), DocumentSize::from_mm(w, h));
    }
}

/// AC 13: a damaged file stays damaged on disk until the maker resizes: the
/// shift of the first resize is measured from the A4 the maker saw, and the
/// resize repairs both registers.
#[test]
fn resizing_a_damaged_file_measures_from_the_a4_the_maker_saw() {
    let bytes = damaged(|m| {
        m.insert("size_width_mm", f64::NAN).unwrap();
        m.insert("size_height_mm", 400.0).unwrap();
    });
    let d = unpack(3, &bytes).unwrap();
    assert_eq!(d.size(), a4());
    // Typing the size the maker sees is a no-op.
    assert_eq!(d.resize(a4()), Ok(false));
    assert_eq!(d.resize(DocumentSize::from_mm(300.0, 297.0)), Ok(true));
    assert_eq!(d.size(), DocumentSize::from_mm(300.0, 297.0));
    assert_eq!(rect_origin(&d), Point::new(55.0, 10.0));
    let e = unpack(4, &curvyo_document_core::pack(&d, "t").unwrap()).unwrap();
    assert_eq!(e.size(), DocumentSize::from_mm(300.0, 297.0));
}

/// AC 24 on a damaged file: fit repairs it as well.
#[test]
fn fitting_a_damaged_file_writes_a_valid_size() {
    let bytes = damaged(|m| m.insert("size_height_mm", -1.0).unwrap());
    let d = unpack(3, &bytes).unwrap();
    assert_eq!(
        d.fit_to_content((Point::new(10.0, 10.0), Point::new(60.0, 60.0))),
        Ok(true)
    );
    assert_eq!(d.size(), DocumentSize::from_mm(50.0, 50.0));
    assert_eq!(rect_origin(&d), Point::new(0.0, 0.0));
    let e = unpack(4, &curvyo_document_core::pack(&d, "t").unwrap()).unwrap();
    assert_eq!(e.size(), DocumentSize::from_mm(50.0, 50.0));
}

/// AC 38: absent or unknown or mistyped display units read as mm.
#[test]
fn unknown_display_units_read_as_mm() {
    type Edit = Box<dyn Fn(&loro::LoroMap)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("absent", Box::new(|_| {})),
        (
            "furlong",
            Box::new(|m| m.insert("display_unit", "furlong").unwrap()),
        ),
        (
            "upper case",
            Box::new(|m| m.insert("display_unit", "MM").unwrap()),
        ),
        ("empty", Box::new(|m| m.insert("display_unit", "").unwrap())),
        (
            "number",
            Box::new(|m| m.insert("display_unit", 2.0_f64).unwrap()),
        ),
        (
            "bool",
            Box::new(|m| m.insert("display_unit", false).unwrap()),
        ),
    ];
    for (what, edit) in cases {
        let bytes = damaged(|m| edit(m));
        let d = unpack(3, &bytes).unwrap();
        let n = change_count(&d);
        assert_eq!(d.display_unit(), DisplayUnit::Mm, "{what}");
        assert_eq!(d.size(), a4(), "{what}");
        assert_eq!(change_count(&d), n, "{what}");
        // setting a real unit works from any of these
        assert!(d.set_display_unit(DisplayUnit::In), "{what}");
        assert_eq!(d.display_unit(), DisplayUnit::In, "{what}");
    }
    // the stored symbol is a plain string register
    let bytes = damaged(|_| {});
    let d = unpack(3, &bytes).unwrap();
    assert!(d.set_display_unit(DisplayUnit::Cm));
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let v = l
        .get_map("root")
        .get("display_unit")
        .unwrap()
        .get_deep_value();
    assert_eq!(v, LoroValue::String("cm".to_string().into()));
}

/// AC 13: the objects of every earlier file are exactly what the build before
/// this story exported (size and every object, field by field); the only new
/// key is `display_unit`, which reads "mm".
#[test]
fn older_files_export_exactly_what_the_previous_build_exported() {
    let baseline: serde_json::Value =
        serde_json::from_slice(&fixture("baseline_pre_0015_export.json")).unwrap();
    let map = baseline.as_object().unwrap();
    assert_eq!(map.len(), 8);
    for (name, old) in map {
        let d = unpack(2, &fixture(name)).unwrap();
        let mut now: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
        let unit = now.as_object_mut().unwrap().remove("display_unit");
        assert_eq!(unit, Some(serde_json::json!("mm")), "{name}");
        // `0040-document-background` adds the `background` member; an older file has the default.
        let background = now.as_object_mut().unwrap().remove("background");
        assert_eq!(
            background,
            Some(serde_json::json!({ "paint": "solid", "color": [232, 232, 235, 1.0] })),
            "{name}"
        );
        // The export names the build's version, which moves with every
        // format bump; everything else is what the previous build exported.
        let version = now.as_object_mut().unwrap().remove("format_version");
        assert_eq!(
            version,
            Some(serde_json::json!(CURRENT_FORMAT_VERSION)),
            "{name}"
        );
        let mut old = old.clone();
        old.as_object_mut().unwrap().remove("format_version");
        assert_eq!(now, old, "{name}: export differs from the previous build");
    }
}

/// AC 38: the unit is exported lower case in the readable document.json.
#[test]
fn export_json_names_the_display_unit() {
    let d = unpack(2, &fixture("display_unit_in_v7.curvyo")).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    assert_eq!(v["display_unit"], "in");
    assert_eq!(v["size"]["width"], 215.9);
    assert_eq!(v["size"]["height"], 279.4);
}

/// AC 38: a version-7 reader (the build before this story) opened the
/// display-unit fixture, showed its size, and saved it again. This build
/// reads the result: the unit it never understood is still there.
#[test]
fn a_file_saved_by_a_v7_reader_keeps_the_display_unit() {
    let d = unpack(
        2,
        &fixture("display_unit_in_v7.resaved_by_v7_reader.curvyo"),
    )
    .unwrap();
    assert_eq!(d.display_unit(), DisplayUnit::In);
    assert_eq!(d.size(), DocumentSize::from_mm(215.9, 279.4));
}

/// AC 13/38: the v7 reader's re-save of the damaged-size fixture still opens
/// at A4 here.
#[test]
fn a_damaged_size_saved_by_a_v7_reader_still_opens_at_a4() {
    let d = unpack(2, &fixture("size_damaged_v7.resaved_by_v7_reader.curvyo")).unwrap();
    assert_eq!(d.size(), a4());
}

/// 500 objects: one resize is still one commit and moves them all. (Loro
/// itself splits a change of about 1400 ops when a snapshot is encoded, as it
/// does for `translate_objects`, so the count is kept below that.)
#[test]
fn resize_of_five_hundred_objects_is_one_commit() {
    let d = Document::new(1);
    for i in 0..500_i32 {
        let _ = d.create_rect(RectBounds {
            origin: Point::new(f64::from(i), -f64::from(i)),
            width: Length::from_mm(1.0),
            height: Length::from_mm(1.0),
        });
    }
    let n = change_count(&d);
    d.resize(DocumentSize::from_mm(1210.0, 1297.0)).unwrap();
    assert_eq!(change_count(&d), n + 1);
    let ids = d.object_ids();
    assert_eq!(ids.len(), 500);
    for (i, id) in ids.iter().enumerate() {
        let Some(curvyo_document_core::ObjectSnapshot::Primitive(p)) = d.object(*id) else {
            panic!()
        };
        let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
            panic!()
        };
        #[allow(clippy::cast_precision_loss)]
        let i = i as f64;
        assert!((bounds.origin.x - (i + 500.0)).abs() < 1e-9);
        assert!((bounds.origin.y - (-i + 500.0)).abs() < 1e-9);
    }
}

/// A deleted object is not resurrected or moved into view by a resize; the
/// live ones move.
#[test]
fn resize_ignores_deleted_objects() {
    let d = Document::new(1);
    let keep = d.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    let gone = d.create_rect(RectBounds {
        origin: Point::new(100.0, 100.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    d.delete_objects(&[gone]).unwrap();
    assert_eq!(d.resize(DocumentSize::from_mm(310.0, 297.0)), Ok(true));
    assert_eq!(d.object_ids(), vec![keep]);
    assert_eq!(rect_origin(&d), Point::new(50.0, 0.0));
    assert!(d.object(gone).is_none());
}
