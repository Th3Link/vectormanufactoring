//! File-format tests for `stroke-and-fill-styling`
//! (`specs/0007-stroke-and-fill-styling/adrs.md`, "`format_version`") and the
//! legacy-fill rule of `specs/0017-style-panel-rework/adrs.md`, decision 1:
//! older containers opening with every style at its frozen default and
//! unchanged by opening, a version-7 file that holds a gradient fill reading as
//! a fill that is off, a fill edit dropping the legacy keys, `document.json`'s
//! `style` object, and every open-file refusal case.
//!
//! `legacy_gradient_v7.curvyo` is the golden of what the first format-7 builds
//! wrote (a linear and a radial gradient with their stop lists). It is never
//! regenerated: its bytes are the point.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use curvyo_document_core::{
    AnchorId, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color, Document, Length,
    LineCap, LineJoin, NewAnchor, ObjectSnapshot, OpenError, Point, RectBounds, Style, StyleEdit,
    pack, unpack,
};
use loro::{LoroDoc, LoroMap, LoroMovableList, LoroValue};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const LEGACY_FIXTURE: &str = "legacy_gradient_v7.curvyo";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("reading {}: {err}", path.display()))
}

fn member(container: &[u8], name: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(container)).expect("zip");
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("member")
        .read_to_end(&mut bytes)
        .expect("read");
    bytes
}

fn container(format_version: u32, loro_bytes: &[u8], json: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": format_version,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "0.1.0",
    });
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes.to_vec()),
        ("document.json", json.to_vec()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .map(|id| document.object(id).unwrap())
        .collect()
}

fn style_of(snapshot: &ObjectSnapshot) -> &Style {
    match snapshot {
        ObjectSnapshot::Path(p) => &p.style,
        ObjectSnapshot::Primitive(p) => &p.style,
    }
}

// ---------------------------------------------------------------------
// Raw construction, for the refusal cases and the fixture
// ---------------------------------------------------------------------

/// A document with one rectangle and one path, exported to a raw Loro doc
/// whose nodes `edit` may change by hand, then wrapped at the current version
/// and opened.
fn open_with(edit: impl FnOnce(&LoroMap, &LoroMap)) -> Result<Document, OpenError> {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(10.0),
        height: mm(5.0),
    });
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(5.0, 5.0)),
        ],
        false,
    );
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let tree = loro.get_tree("paths");
    let roots = tree.roots();
    let rect_meta = tree.get_meta(roots[0]).unwrap();
    let path_meta = tree.get_meta(roots[1]).unwrap();
    edit(&rect_meta, &path_meta);
    loro.commit();
    let bytes = container(
        CURRENT_FORMAT_VERSION,
        &loro.export(loro::ExportMode::Snapshot).unwrap(),
        b"{}",
    );
    unpack(2, &bytes)
}

fn push_stop(list: &LoroMovableList, fields: &[(&str, LoroValue)]) {
    let map = list.push_container(LoroMap::new()).unwrap();
    for (key, value) in fields {
        map.insert(key, value.clone()).unwrap();
    }
}

fn good_stop(counter: u64) -> Vec<(&'static str, LoroValue)> {
    vec![
        ("id", LoroValue::from(format!("stop-{counter}"))),
        ("position", LoroValue::from(0.5)),
        ("color", LoroValue::from(vec![1_i64, 2, 3])),
        ("opacity", LoroValue::from(1.0)),
    ]
}

// ---------------------------------------------------------------------
// Open-file validation (adrs.md, "format_version goes to 4" list, as 7)
// ---------------------------------------------------------------------

#[test]
fn a_present_style_key_with_the_wrong_type_or_range_is_refused_as_damaged() {
    let nan = LoroValue::from(f64::NAN);
    let cases: Vec<(&str, LoroValue)> = vec![
        ("stroke_enabled", LoroValue::from("yes")),
        ("stroke_enabled", LoroValue::from(1_i64)),
        ("stroke_width", LoroValue::from(0.0)),
        ("stroke_width", LoroValue::from(-1.0)),
        ("stroke_width", nan.clone()),
        ("stroke_width", LoroValue::from(f64::INFINITY)),
        ("stroke_width", LoroValue::from("thick")),
        ("stroke", LoroValue::from(vec![1_i64, 2])),
        ("stroke", LoroValue::from(vec![1_i64, 2, 300])),
        ("stroke", LoroValue::from("red")),
        ("stroke_opacity", LoroValue::from(1.5)),
        ("stroke_opacity", LoroValue::from(-0.1)),
        ("stroke_opacity", nan.clone()),
        ("stroke_dash", LoroValue::from(vec![0.0_f64])),
        ("stroke_dash", LoroValue::from(vec![-1.0_f64, 2.0])),
        ("stroke_dash", LoroValue::from(vec![0.0_f64, 0.0])),
        ("stroke_dash", LoroValue::from(vec![f64::NAN, 1.0])),
        ("stroke_dash", LoroValue::from("dashed")),
        ("stroke_join", LoroValue::from("arrow")),
        ("stroke_join", LoroValue::from(3_i64)),
        ("stroke_cap", LoroValue::from("flat")),
        ("fill_enabled", LoroValue::from("no")),
        ("fill_kind", LoroValue::from("conic")),
        ("fill", LoroValue::from(vec![0_i64, 0, 256])),
        ("fill", LoroValue::from(vec![0_i64, 0, -1])),
        ("fill_opacity", LoroValue::from(2.0)),
    ];
    for (key, value) in cases {
        for on_rect in [true, false] {
            let result = open_with(|rect, path| {
                let meta = if on_rect { rect } else { path };
                meta.insert(key, value.clone()).unwrap();
            });
            assert!(
                matches!(result, Err(OpenError::Damaged)),
                "{key} = {value:?} on {}",
                if on_rect { "a rectangle" } else { "a path" }
            );
        }
    }
}

#[test]
fn a_legacy_stop_list_of_any_size_or_shape_opens_and_is_never_read() {
    for count in [0_u64, 1, 2, 17, 40] {
        let document = open_with(|rect, _| {
            let list = rect
                .insert_container("fill_stops", LoroMovableList::new())
                .unwrap();
            for n in 1..=count {
                push_stop(&list, &good_stop(n));
            }
            rect.insert("fill_enabled", true).unwrap();
            rect.insert("fill_kind", "linear").unwrap();
        })
        .unwrap_or_else(|err| panic!("{count} stops: {err:?}"));
        let first = objects(&document).remove(0);
        assert!(!style_of(&first).fill.enabled, "{count} stops");
    }
}

#[test]
fn integer_numbers_and_every_valid_edge_value_open() {
    let document = open_with(|rect, path| {
        rect.insert("stroke_width", 2_i64).unwrap();
        rect.insert("stroke_opacity", 0.0).unwrap();
        rect.insert("fill_opacity", 1_i64).unwrap();
        rect.insert("stroke_dash", vec![0.0_f64, 3.0]).unwrap();
        path.insert("stroke_dash", Vec::<f64>::new()).unwrap();
    })
    .unwrap();
    let all = objects(&document);
    let rect = style_of(&all[0]);
    assert_eq!(rect.stroke.width, mm(2.0));
    assert_eq!(rect.stroke.opacity.get(), 0.0);
    assert_eq!(rect.fill.opacity.get(), 1.0);
    assert_eq!(rect.stroke.dash.as_slice(), [0.0, 3.0]);
    assert!(style_of(&all[1]).stroke.dash.is_solid());
}

#[test]
fn an_unknown_extra_key_is_still_tolerated() {
    assert!(open_with(|rect, _| rect.insert("future_style_key", 1_i64).unwrap()).is_ok());
}

// ---------------------------------------------------------------------
// Older files open at the defaults and opening writes nothing (AC 3)
// ---------------------------------------------------------------------

#[test]
fn every_older_fixture_opens_with_every_style_at_its_default() {
    for name in [
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "corner_radii_per_corner.curvyo",
    ] {
        let bytes = fixture(name);
        let document = unpack(2, &bytes).unwrap_or_else(|err| panic!("{name}: {err:?}"));
        let all = objects(&document);
        assert!(!all.is_empty(), "{name} holds objects");
        for object in &all {
            assert_eq!(style_of(object), &Style::default(), "{name}");
        }
        // Opening rewrites nothing: the replica's history is the file's.
        let stored = LoroDoc::new();
        stored.import(&member(&bytes, "document.loro")).unwrap();
        let reopened = LoroDoc::new();
        reopened
            .import(&document.export_loro_snapshot().unwrap())
            .unwrap();
        assert_eq!(reopened.oplog_vv(), stored.oplog_vv(), "{name}");
    }
}

// ---------------------------------------------------------------------
// The legacy gradient fixture (0017 adrs.md, decision 1)
// ---------------------------------------------------------------------

const RED: Color = Color { r: 255, g: 0, b: 0 };
const GREEN: Color = Color { r: 0, g: 128, b: 0 };
const BLUE: Color = Color { r: 0, g: 0, b: 255 };

/// Object 0: a path with every stroke key and a linear gradient. Object 1: a
/// rectangle with the stroke off, a radial gradient and a long dash list.
/// Object 2: an ellipse with a solid translucent fill. Object 3: a star left
/// at the defaults.
#[test]
fn the_legacy_fixture_declares_the_version_that_introduced_styles() {
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&fixture(LEGACY_FIXTURE), "manifest.json")).unwrap();
    assert_eq!(manifest["format_version"], 7);
}

#[test]
fn the_legacy_fixture_reads_back_with_every_gradient_fill_off() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let all = objects(&document);
    assert_eq!(all.len(), 4);

    let path = style_of(&all[0]);
    assert!(path.stroke.enabled);
    assert_eq!(path.stroke.width, mm(1.5));
    assert_eq!(path.stroke.color, RED);
    assert_eq!(path.stroke.opacity.get(), 0.75);
    assert_eq!(path.stroke.dash.as_slice(), [6.0, 3.0, 1.0, 3.0]);
    assert_eq!(path.stroke.join, LineJoin::Round);
    assert_eq!(path.stroke.cap, LineCap::Square);
    assert!(!path.fill.paints(), "a linear gradient reads as no fill");
    assert_eq!(
        path.fill.color, GREEN,
        "the stored colour is read as stored"
    );
    assert_eq!(path.fill.opacity.get(), 0.5);

    let rect = style_of(&all[1]);
    assert!(!rect.stroke.enabled);
    assert_eq!(rect.stroke.width, mm(2.0));
    assert_eq!(rect.stroke.dash.as_slice().len(), 8);
    assert!(!rect.fill.paints(), "a radial gradient reads as no fill");

    let ellipse = style_of(&all[2]);
    assert!(ellipse.fill.paints());
    assert_eq!(ellipse.fill.color, BLUE);
    assert_eq!(ellipse.fill.opacity.get(), 0.2);
    assert_eq!(ellipse.stroke.cap, LineCap::Round);

    assert_eq!(style_of(&all[3]), &Style::default());
}

/// The raw meta keys of the object at `index` in a document's Loro snapshot.
fn stored_keys(document: &Document, index: usize) -> Vec<String> {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let tree = loro.get_tree("paths");
    let node = tree.roots()[index];
    let meta = tree.get_meta(node).unwrap();
    let mut keys = Vec::new();
    meta.for_each(|k, _| keys.push(k.to_string()));
    keys
}

fn changes(document: &Document) -> usize {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro.len_changes()
}

#[test]
fn opening_and_saving_the_legacy_fixture_writes_nothing_and_keeps_the_keys() {
    let bytes = fixture(LEGACY_FIXTURE);
    let document = unpack(2, &bytes).unwrap();
    let stored = LoroDoc::new();
    stored.import(&member(&bytes, "document.loro")).unwrap();
    let reopened = LoroDoc::new();
    reopened
        .import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    assert_eq!(reopened.oplog_vv(), stored.oplog_vv());

    let saved = unpack(3, &pack(&document, "0.1.0").unwrap()).unwrap();
    for index in [0, 1] {
        let keys = stored_keys(&saved, index);
        assert!(keys.contains(&"fill_kind".to_string()), "{keys:?}");
        assert!(keys.contains(&"fill_stops".to_string()), "{keys:?}");
    }
    for (a, b) in objects(&document).iter().zip(objects(&saved).iter()) {
        assert_eq!(style_of(a), style_of(b));
    }
}

#[test]
fn an_edit_that_leaves_the_fill_alone_keeps_the_legacy_keys() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let path = document.object_ids()[0];
    document
        .edit_style(&[path], &StyleEdit::StrokeWidth(mm(3.0)))
        .unwrap();
    let keys = stored_keys(&document, 0);
    assert!(keys.contains(&"fill_kind".to_string()));
    assert!(keys.contains(&"fill_stops".to_string()));
}

#[test]
fn turning_the_fill_on_makes_it_solid_in_the_stored_colour_and_drops_both_keys_in_one_commit() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let path = document.object_ids()[0];
    let before = changes(&document);
    document
        .edit_style(&[path], &StyleEdit::FillEnabled(true))
        .unwrap();
    assert_eq!(changes(&document), before + 1, "one commit");
    let all = objects(&document);
    let fill = &style_of(&all[0]).fill;
    assert!(fill.paints());
    assert_eq!(fill.color, GREEN);
    assert_eq!(fill.opacity.get(), 0.5);
    let keys = stored_keys(&document, 0);
    assert!(!keys.contains(&"fill_kind".to_string()), "{keys:?}");
    assert!(!keys.contains(&"fill_stops".to_string()), "{keys:?}");
    // The other gradient object is untouched.
    assert!(stored_keys(&document, 1).contains(&"fill_stops".to_string()));
    // And it survives a save and reopen.
    let again = unpack(4, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(style_of(&objects(&again)[0]).fill, *fill);
}

#[test]
fn a_colour_edit_on_a_gradient_object_drops_the_keys_but_keeps_the_fill_off() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let rect = document.object_ids()[1];
    document
        .edit_style(&[rect], &StyleEdit::FillColor(RED))
        .unwrap();
    let all = objects(&document);
    let fill = &style_of(&all[1]).fill;
    assert!(!fill.paints(), "a fill edit never turns the fill on");
    assert_eq!(fill.color, RED);
    let keys = stored_keys(&document, 1);
    assert!(!keys.contains(&"fill_kind".to_string()));
    assert!(!keys.contains(&"fill_stops".to_string()));
    let again = unpack(4, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert!(!style_of(&objects(&again)[1]).fill.paints());
}

#[test]
fn turning_the_fill_off_on_a_gradient_object_is_no_change_and_writes_nothing() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let path = document.object_ids()[0];
    let before = changes(&document);
    document
        .edit_style(&[path], &StyleEdit::FillEnabled(false))
        .unwrap();
    assert_eq!(changes(&document), before, "the fill already reads as off");
    assert!(stored_keys(&document, 0).contains(&"fill_kind".to_string()));
}

#[test]
fn a_solid_object_of_the_legacy_fixture_is_not_changed_by_an_unrelated_edit() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let ellipse = document.object_ids()[2];
    document
        .edit_style(&[ellipse], &StyleEdit::FillColor(RED))
        .unwrap();
    let all = objects(&document);
    assert!(style_of(&all[2]).fill.paints());
    assert_eq!(style_of(&all[2]).fill.color, RED);
}

#[test]
fn document_json_carries_one_style_object_per_object() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    assert_eq!(json["format_version"], CURRENT_FORMAT_VERSION);
    let objects = json["objects"].as_array().unwrap();
    assert_eq!(objects.len(), 4);
    for object in objects {
        assert!(object["style"].is_object(), "{object}");
        for flat in ["stroke_width", "stroke", "fill"] {
            assert!(object.get(flat).is_none(), "{flat} moved into style");
        }
    }
    let path = &objects[0]["style"];
    assert_eq!(path["stroke"]["width"], 1.5);
    assert_eq!(path["stroke"]["join"], "round");
    assert_eq!(path["stroke"]["cap"], "square");
    assert_eq!(
        path["stroke"]["dash"],
        serde_json::json!([6.0, 3.0, 1.0, 3.0])
    );
    assert_eq!(path["fill"]["enabled"], false);
    assert!(path["fill"].get("kind").is_none());
    assert!(path["fill"].get("stops").is_none());
    assert_eq!(objects[2]["style"]["fill"]["enabled"], true);
}

#[test]
fn an_older_reader_would_refuse_this_build_so_the_bump_is_real() {
    // The container this build writes says at least the version that
    // introduced styles, so a build from before it answers "saved by a newer
    // version".
    let bytes = pack(&Document::new(1), "0.1.0").unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&bytes, "manifest.json")).unwrap();
    assert!(manifest["format_version"].as_u64().unwrap() >= 7);
}
