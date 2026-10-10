//! The document background (`specs/0040-document-background` criteria 1 to 8,
//! 22 and 42 to 45, `adrs.md` decisions 1 to 5 and 11): two root registers,
//! strict validation on open, one labelled commit that writes only the
//! register that changed, and the golden `background_v10.curvyo`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Read, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;

use curvyo_document_core::{
    BackgroundPaint, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color, DisplayUnit,
    Document, DocumentBackground, DocumentSize, Length, NodeId, Opacity, OpenError, Point,
    RectBounds, pack, unpack,
};
use loro::{LoroDoc, LoroValue};

/// The version this slice took at its branch point. A rebase onto a merge that
/// took 10 first renumbers this constant, the fixtures and their names.
const THIS_SLICE_VERSION: u32 = 10;
const GOLDEN: &str = "background_v10.curvyo";
const DAMAGED: [&str; 4] = [
    "background_paint_unknown_v10.curvyo",
    "background_color_three_v10.curvyo",
    "background_color_range_v10.curvyo",
    "background_color_type_v10.curvyo",
];

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn bg(paint: BackgroundPaint, r: u8, g: u8, b: u8, alpha: f64) -> DocumentBackground {
    DocumentBackground {
        paint,
        color: Color { r, g, b },
        opacity: Opacity::new(alpha).unwrap(),
    }
}

fn loro_of(document: &Document) -> LoroDoc {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro
}

fn root_value(document: &Document, key: &str) -> Option<LoroValue> {
    loro_of(document)
        .get_map("root")
        .get(key)
        .map(|v| v.get_deep_value())
}

fn labels(document: &Document) -> Vec<String> {
    let loro = loro_of(document);
    let ids: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|s| s.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, s)| s).collect()
}

fn op_count(document: &Document) -> i32 {
    loro_of(document).oplog_vv().values().copied().sum()
}

fn container_from_loro(loro_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes.to_vec()),
        ("document.json", b"{}".to_vec()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A file whose root map carries the given extra values, as a hand-made or
/// damaged file would.
fn file_with_root(values: &[(&str, LoroValue)]) -> Vec<u8> {
    let loro = loro_of(&Document::new(1));
    loro.set_peer_id(1).unwrap();
    let root = loro.get_map("root");
    for (key, value) in values {
        root.insert(key, value.clone()).unwrap();
    }
    loro.commit();
    container_from_loro(&loro.export(loro::ExportMode::Snapshot).unwrap())
}

fn list(values: Vec<LoroValue>) -> LoroValue {
    LoroValue::List(values.into())
}

fn damaged_files() -> [(&'static str, Vec<u8>); 4] {
    let colour = |r: LoroValue, g, b, a| list(vec![r, g, b, a]);
    let i = LoroValue::I64;
    let d = LoroValue::Double;
    [
        (
            DAMAGED[0],
            file_with_root(&[("background_paint", LoroValue::from("checker"))]),
        ),
        (
            DAMAGED[1],
            file_with_root(&[("background_color", list(vec![i(1), i(2), i(3)]))]),
        ),
        (
            DAMAGED[2],
            file_with_root(&[("background_color", colour(i(1), i(2), i(3), d(1.5)))]),
        ),
        (
            DAMAGED[3],
            file_with_root(&[("background_color", colour(d(1.0), i(2), i(3), d(1.0)))]),
        ),
    ]
}

fn golden_document() -> Document {
    let document = Document::new(1);
    assert!(document.set_background(bg(BackgroundPaint::None, 47, 111, 238, 128.0 / 255.0)));
    document
}

fn rect(document: &Document) -> NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(10.0, 10.0),
        width: Length::from_mm(20.0),
        height: Length::from_mm(30.0),
    })
}

// ---- criteria 1 to 3: the value and its two registers ----------------------

#[test]
fn a_new_document_has_the_default_background_and_stores_neither_register() {
    let document = Document::new(1);
    let background = document.background();
    assert_eq!(background, DocumentBackground::DEFAULT);
    assert_eq!(background.paint, BackgroundPaint::Solid);
    assert_eq!(
        background.color,
        Color {
            r: 232,
            g: 232,
            b: 235
        }
    );
    assert_eq!(background.opacity, Opacity::OPAQUE);
    assert_eq!(root_value(&document, "background_paint"), None);
    assert_eq!(root_value(&document, "background_color"), None);
}

#[test]
fn a_paint_edit_writes_only_the_paint_register() {
    let document = Document::new(1);
    assert!(document.set_background(DocumentBackground {
        paint: BackgroundPaint::None,
        ..DocumentBackground::DEFAULT
    }));
    assert_eq!(
        root_value(&document, "background_paint"),
        Some(LoroValue::from("none"))
    );
    assert_eq!(root_value(&document, "background_color"), None);
}

#[test]
fn a_colour_edit_writes_only_the_colour_register_as_one_list() {
    let document = Document::new(1);
    assert!(document.set_background(bg(BackgroundPaint::Solid, 255, 0, 17, 0.5)));
    assert_eq!(root_value(&document, "background_paint"), None);
    assert_eq!(
        root_value(&document, "background_color"),
        Some(list(vec![
            LoroValue::I64(255),
            LoroValue::I64(0),
            LoroValue::I64(17),
            LoroValue::Double(0.5),
        ]))
    );
}

#[test]
fn a_value_equal_to_the_stored_one_writes_nothing_and_makes_no_commit() {
    let document = Document::new(1);
    let ops = op_count(&document);
    let commits = labels(&document);
    assert!(!document.set_background(DocumentBackground::DEFAULT));
    assert_eq!(op_count(&document), ops, "the default is not written");
    let _ = document.set_background(bg(BackgroundPaint::Solid, 1, 2, 3, 0.25));
    let ops = op_count(&document);
    let commits_after = labels(&document);
    assert!(!document.set_background(bg(BackgroundPaint::Solid, 1, 2, 3, 0.25)));
    assert_eq!(op_count(&document), ops);
    assert_eq!(labels(&document), commits_after);
    assert_eq!(commits_after.len(), commits.len() + 1);
}

#[test]
fn every_edit_is_one_commit_labelled_set_document_background() {
    let document = Document::new(1);
    let before = labels(&document).len();
    assert!(document.set_background(bg(BackgroundPaint::None, 9, 9, 9, 0.5)));
    let after = labels(&document);
    assert_eq!(after.len(), before + 1, "paint and colour in one commit");
    assert_eq!(after.last().unwrap(), "set_document_background");
}

// ---- criterion 6: save, close, open ------------------------------------------

#[test]
fn every_component_survives_save_and_open_exactly() {
    let document = Document::new(1);
    let alpha = 128.0 / 255.0;
    let _ = document.set_background(bg(BackgroundPaint::None, 47, 111, 238, alpha));
    let reopened = unpack(2, &pack(&document, "t").unwrap()).unwrap();
    let background = reopened.background();
    assert_eq!(background.paint, BackgroundPaint::None);
    assert_eq!(
        background.color,
        Color {
            r: 47,
            g: 111,
            b: 238
        }
    );
    assert_eq!(background.opacity.get(), alpha, "not rounded to 50 %");
    // Pressing Solid shows the colour as it was before the save.
    assert!(reopened.set_background(DocumentBackground {
        paint: BackgroundPaint::Solid,
        ..background
    }));
    assert_eq!(
        reopened.background().color,
        Color {
            r: 47,
            g: 111,
            b: 238
        }
    );
}

#[test]
fn document_json_shows_the_background() {
    let document = golden_document();
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    assert_eq!(json["background"]["paint"], "none");
    assert_eq!(json["background"]["color"][0], 47);
    assert_eq!(json["background"]["color"][2], 238);
    assert_eq!(json["background"]["color"][3], 128.0 / 255.0);
    assert_eq!(json["format_version"], CURRENT_FORMAT_VERSION);
}

// ---- criterion 5: the format version -----------------------------------------

#[test]
fn this_slice_took_its_format_version_and_the_writer_declares_it() {
    assert_eq!(CURRENT_FORMAT_VERSION, THIS_SLICE_VERSION);
    let bytes = pack(&Document::new(1), "t").unwrap();
    assert_eq!(manifest_version(&bytes), CURRENT_FORMAT_VERSION);
}

fn manifest_version(bytes: &[u8]) -> u32 {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut manifest = Vec::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut manifest)
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    u32::try_from(manifest["format_version"].as_u64().unwrap()).unwrap()
}

#[test]
fn a_file_of_the_new_version_without_the_registers_means_the_default() {
    let bytes = pack(&Document::new(1), "t").unwrap();
    assert_eq!(
        unpack(2, &bytes).unwrap().background(),
        DocumentBackground::DEFAULT
    );
}

// ---- criterion 7: strict validation ---------------------------------------------

#[test]
fn each_malformed_register_is_refused_as_damaged() {
    for (name, bytes) in damaged_files() {
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "{name}"
        );
    }
}

#[test]
fn the_damaged_fixtures_are_refused_as_damaged() {
    write_fixtures_on_request();
    for name in DAMAGED {
        let bytes = std::fs::read(fixture_path(name)).unwrap();
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "{name}"
        );
    }
}

#[test]
fn more_malformed_shapes_are_damaged_and_every_valid_edge_opens() {
    let i = LoroValue::I64;
    let d = LoroValue::Double;
    let bad: Vec<(&str, LoroValue)> = vec![
        ("background_paint", LoroValue::I64(1)),
        ("background_paint", LoroValue::from("Solid")),
        ("background_color", LoroValue::from("#E8E8EB")),
        (
            "background_color",
            list(vec![i(1), i(2), i(3), d(1.0), d(1.0)]),
        ),
        ("background_color", list(vec![i(256), i(2), i(3), d(1.0)])),
        ("background_color", list(vec![i(-1), i(2), i(3), d(1.0)])),
        ("background_color", list(vec![i(1), d(0.5), i(3), d(1.0)])),
        ("background_color", list(vec![i(1), i(2), i(3), d(-0.1)])),
        (
            "background_color",
            list(vec![i(1), i(2), i(3), d(f64::NAN)]),
        ),
        (
            "background_color",
            list(vec![i(1), i(2), i(3), d(f64::INFINITY)]),
        ),
        (
            "background_color",
            list(vec![i(1), i(2), i(3), LoroValue::from("1")]),
        ),
    ];
    for (key, value) in bad {
        let result = unpack(2, &file_with_root(&[(key, value.clone())]));
        assert!(
            matches!(result, Err(OpenError::Damaged)),
            "{key} = {value:?}"
        );
    }
    for ok in [
        list(vec![i(0), i(0), i(0), d(0.0)]),
        list(vec![i(255), i(255), i(255), d(1.0)]),
        list(vec![i(1), i(2), i(3), i(1)]),
    ] {
        assert!(
            unpack(2, &file_with_root(&[("background_color", ok.clone())])).is_ok(),
            "{ok:?}"
        );
    }
    for paint in ["none", "solid"] {
        assert!(
            unpack(
                2,
                &file_with_root(&[("background_paint", LoroValue::from(paint))])
            )
            .is_ok()
        );
    }
}

// ---- golden --------------------------------------------------------------------

fn write_fixtures_on_request() {
    if std::env::var_os("CURVYO_WRITE_FIXTURES").is_some() {
        std::fs::write(
            fixture_path(GOLDEN),
            pack(&golden_document(), "0.1.0").unwrap(),
        )
        .unwrap();
        for (name, bytes) in damaged_files() {
            std::fs::write(fixture_path(name), bytes).unwrap();
        }
    }
}

#[test]
fn the_golden_reads_back_and_opening_it_writes_nothing() {
    write_fixtures_on_request();
    let bytes = std::fs::read(fixture_path(GOLDEN)).unwrap();
    assert_eq!(manifest_version(&bytes), THIS_SLICE_VERSION);
    let document = unpack(2, &bytes).unwrap();
    let background = document.background();
    assert_eq!(background.paint, BackgroundPaint::None);
    assert_eq!(
        background.color,
        Color {
            r: 47,
            g: 111,
            b: 238
        }
    );
    assert_eq!(background.opacity.get(), 128.0 / 255.0);

    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut stored_bytes = Vec::new();
    archive
        .by_name("document.loro")
        .unwrap()
        .read_to_end(&mut stored_bytes)
        .unwrap();
    let stored = LoroDoc::new();
    stored.import(&stored_bytes).unwrap();
    assert_eq!(loro_of(&document).oplog_vv(), stored.oplog_vv());
}

// ---- criterion 4: older files ----------------------------------------------------

#[test]
fn every_older_fixture_opens_with_the_default_background() {
    for name in [
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_gradient_v7.curvyo",
        "compound_v8.curvyo",
        "dash_v9.curvyo",
        "markers_v9.curvyo",
        "display_unit_in_v7.curvyo",
    ] {
        let bytes = std::fs::read(fixture_path(name)).unwrap();
        let document = unpack(2, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(document.background(), DocumentBackground::DEFAULT, "{name}");
        assert_eq!(root_value(&document, "background_paint"), None, "{name}");
        assert_eq!(root_value(&document, "background_color"), None, "{name}");
    }
}

// ---- criteria 42 to 45: the background is not an object --------------------------

#[test]
fn resize_fit_and_the_display_unit_write_no_background_register() {
    let document = Document::new(1);
    let want = bg(BackgroundPaint::None, 255, 0, 0, 1.0);
    let _ = document.set_background(want);
    let _ = rect(&document);
    let colour = root_value(&document, "background_color");
    let paint = root_value(&document, "background_paint");
    document
        .resize(DocumentSize::from_mm(297.0, 420.0))
        .unwrap();
    document
        .fit_to_content((Point::new(10.0, 10.0), Point::new(30.0, 40.0)))
        .unwrap();
    assert!(document.set_display_unit(DisplayUnit::In));
    assert_eq!(document.background(), want);
    assert_eq!(root_value(&document, "background_color"), colour);
    assert_eq!(root_value(&document, "background_paint"), paint);
}

#[test]
fn the_background_is_not_in_the_object_tree() {
    let document = Document::new(1);
    let _ = document.set_background(bg(BackgroundPaint::Solid, 255, 0, 0, 1.0));
    assert_eq!(document.object_ids(), Vec::<NodeId>::new());
}

fn merged(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    unpack(
        99,
        &container_from_loro(&loro.export(loro::ExportMode::Snapshot).unwrap()),
    )
    .unwrap()
}

#[test]
fn two_peers_setting_the_colour_end_with_one_complete_colour() {
    let base = Document::new(1);
    let a = unpack(10, &pack(&base, "t").unwrap()).unwrap();
    let b = unpack(11, &pack(&base, "t").unwrap()).unwrap();
    let one = bg(BackgroundPaint::Solid, 0x11, 0x22, 0x33, 1.0);
    let two = bg(BackgroundPaint::Solid, 0x44, 0x55, 0x66, 1.0);
    let _ = a.set_background(one);
    let _ = b.set_background(two);
    let ab = merged(&a, &b).background();
    let ba = merged(&b, &a).background();
    assert_eq!(ab, ba, "both peers hold the same colour");
    assert!(
        ab == one || ab == two,
        "one of the two, never a mix: {ab:?}"
    );
}

#[test]
fn a_paint_edit_and_a_colour_edit_of_two_peers_both_survive() {
    let base = Document::new(1);
    let a = unpack(10, &pack(&base, "t").unwrap()).unwrap();
    let b = unpack(11, &pack(&base, "t").unwrap()).unwrap();
    let _ = a.set_background(DocumentBackground {
        paint: BackgroundPaint::None,
        ..DocumentBackground::DEFAULT
    });
    let _ = b.set_background(bg(BackgroundPaint::Solid, 0x44, 0x55, 0x66, 0.5));
    let merged = merged(&a, &b).background();
    assert_eq!(merged.paint, BackgroundPaint::None);
    assert_eq!(
        merged.color,
        Color {
            r: 0x44,
            g: 0x55,
            b: 0x66
        }
    );
    assert_eq!(merged.opacity.get(), 0.5);
}
