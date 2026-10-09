//! Independent tester cases for `0017-style-panel-rework`, block 1 (gradient
//! removal, the read-past rule for old gradient files, format version 9, odd
//! dash lists, the RGBA style edits). Written from `specification.md` and
//! `adrs.md` before the implementation was read.
//!
//! Criteria covered here: 7, 8, 9 (stored side), 13, 15, 32, 33, 50, 53, 54.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color, DashPattern, DisplayUnit,
    Document, Length, ObjectSnapshot, Opacity, OpenError, Point, RectBounds, Style, StyleEdit,
    pack, unpack,
};
use loro::{LoroDoc, LoroValue};
use zip::ZipArchive;

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn op(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

fn rect(doc: &Document, x: f64) -> curvyo_document_core::NodeId {
    doc.create_rect(RectBounds {
        origin: Point::new(x, 0.0),
        width: mm(10.0),
        height: mm(6.0),
    })
}

fn style_of(doc: &Document, id: curvyo_document_core::NodeId) -> Style {
    match doc.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => p.style,
        ObjectSnapshot::Primitive(p) => p.style,
    }
}

fn styles(doc: &Document) -> Vec<Style> {
    doc.object_ids()
        .into_iter()
        .map(|id| style_of(doc, id))
        .collect()
}

fn loro_of(doc: &Document) -> LoroDoc {
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    loro
}

/// Sum of the version vector: grows for any write.
fn op_count(doc: &Document) -> i64 {
    loro_of(doc)
        .oplog_vv()
        .values()
        .map(|c| i64::from(*c))
        .sum()
}

fn change_count(doc: &Document) -> usize {
    loro_of(doc).len_changes()
}

/// Loro merges adjacent same-label commits of one peer; a differently labelled
/// commit in between keeps the counts meaningful.
fn separate_label(doc: &Document) {
    let unit = if doc.display_unit() == DisplayUnit::In {
        DisplayUnit::Mm
    } else {
        DisplayUnit::In
    };
    assert!(doc.set_display_unit(unit));
}

/// The deep value of every object meta map of the document.
fn metas(doc: &Document) -> Vec<LoroValue> {
    let loro = loro_of(doc);
    let tree = loro.get_tree("paths");
    tree.nodes()
        .into_iter()
        .map(|n| tree.get_meta(n).unwrap().get_deep_value())
        .collect()
}

fn has_key(meta: &LoroValue, key: &str) -> bool {
    match meta {
        LoroValue::Map(m) => m.contains_key(key),
        _ => false,
    }
}

fn reopen(doc: &Document) -> Document {
    unpack(77, &pack(doc, "tester").unwrap()).expect("reopens")
}

fn manifest_version(container: &[u8]) -> u64 {
    let mut archive = ZipArchive::new(Cursor::new(container)).expect("zip");
    let mut bytes = Vec::new();
    archive
        .by_name("manifest.json")
        .expect("manifest")
        .read_to_end(&mut bytes)
        .expect("read");
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    manifest["format_version"].as_u64().unwrap()
}

fn container_from_loro(format_version: u32, loro_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": format_version,
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

fn merged(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    unpack(99, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes)).expect("merged opens")
}

/// The ids of the legacy fixture by shape, in the order of `object_ids`.
struct Legacy {
    doc: Document,
    linear_path: curvyo_document_core::NodeId,
    radial_rect: curvyo_document_core::NodeId,
    solid_ellipse: curvyo_document_core::NodeId,
    star: curvyo_document_core::NodeId,
}

fn legacy() -> Legacy {
    let doc = unpack(5, &fixture("legacy_gradient_v7.curvyo")).expect("legacy file opens");
    let mut linear_path = None;
    let mut radial_rect = None;
    let mut solid_ellipse = None;
    let mut star = None;
    for id in doc.object_ids() {
        match doc.object(id).unwrap() {
            ObjectSnapshot::Path(_) => linear_path = Some(id),
            ObjectSnapshot::Primitive(p) => match format!("{:?}", p.shape) {
                s if s.starts_with("Rect") => radial_rect = Some(id),
                s if s.starts_with("Ellipse") => solid_ellipse = Some(id),
                _ => star = Some(id),
            },
        }
    }
    Legacy {
        doc,
        linear_path: linear_path.unwrap(),
        radial_rect: radial_rect.unwrap(),
        solid_ellipse: solid_ellipse.unwrap(),
        star: star.unwrap(),
    }
}

/// The legacy keys of the meta of `id`, as present now.
fn legacy_keys_of(doc: &Document, id: curvyo_document_core::NodeId) -> (bool, bool) {
    let loro = loro_of(doc);
    let tree = loro.get_tree("paths");
    let wanted = format!("{id:?}").replace("NodeId", "");
    for node in tree.nodes() {
        if format!("{node:?}").replace("TreeID", "") == wanted {
            let meta = tree.get_meta(node).unwrap().get_deep_value();
            return (has_key(&meta, "fill_kind"), has_key(&meta, "fill_stops"));
        }
    }
    panic!("node not found");
}

// ------------------------------------------- AC 53: legacy gradient files

#[test]
fn legacy_gradient_file_opens_without_error_and_keeps_geometry_and_stroke() {
    let l = legacy();
    assert_eq!(l.doc.object_ids().len(), 4);

    // The linear-gradient path: fill off, stroke and dash exactly as stored.
    let s = style_of(&l.doc, l.linear_path);
    assert!(!s.fill.enabled, "a gradient fill reads as Paint None");
    assert!(s.stroke.enabled);
    assert_eq!(s.stroke.color, rgb(255, 0, 0));
    assert_eq!(s.stroke.width.as_mm(), 1.5);
    assert_eq!(s.stroke.opacity.get(), 0.75);
    assert_eq!(s.stroke.dash.as_slice(), &[6.0, 3.0, 1.0, 3.0]);
    match l.doc.object(l.linear_path).unwrap() {
        ObjectSnapshot::Path(p) => {
            assert_eq!(p.anchors.len(), 3);
            assert!(!p.closed);
        }
        ObjectSnapshot::Primitive(_) => panic!("expected path"),
    }

    // The radial rectangle: fill off, stroke off with its stored width.
    let s = style_of(&l.doc, l.radial_rect);
    assert!(!s.fill.enabled);
    assert!(!s.stroke.enabled);
    assert_eq!(s.stroke.width.as_mm(), 2.0);
    assert_eq!(
        s.stroke.dash.as_slice(),
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    );

    // The plain solid object keeps its solid fill.
    let s = style_of(&l.doc, l.solid_ellipse);
    assert!(s.fill.enabled);
    assert_eq!(s.fill.color, rgb(0, 0, 255));
    assert_eq!(s.fill.opacity.get(), 0.2);
    assert_eq!(s.stroke.cap, curvyo_document_core::LineCap::Round);

    // An object without any fill key stays as it was.
    let s = style_of(&l.doc, l.star);
    assert!(!s.fill.enabled);
}

#[test]
fn opening_and_reading_the_legacy_file_writes_nothing() {
    let l = legacy();
    let before = op_count(&l.doc);
    for id in l.doc.object_ids() {
        let _ = l.doc.object(id);
    }
    let _ = l.doc.export_json().unwrap();
    assert_eq!(op_count(&l.doc), before);
    assert_eq!(legacy_keys_of(&l.doc, l.linear_path), (true, true));
    assert_eq!(legacy_keys_of(&l.doc, l.radial_rect), (true, true));
    assert_eq!(legacy_keys_of(&l.doc, l.solid_ellipse), (false, false));
}

#[test]
fn saving_without_a_fill_edit_keeps_the_gradient_data_and_reopens_the_same() {
    let l = legacy();
    let again = reopen(&l.doc);
    assert_eq!(styles(&again), styles(&l.doc));
    let kept = metas(&again)
        .iter()
        .filter(|m| has_key(m, "fill_kind"))
        .count();
    assert_eq!(
        kept, 2,
        "both gradient objects keep their keys through save"
    );
}

#[test]
fn a_stroke_edit_on_a_gradient_object_leaves_the_gradient_data_alone() {
    let l = legacy();
    l.doc
        .edit_style(&[l.linear_path], &StyleEdit::StrokeColor(rgb(1, 2, 3)))
        .unwrap();
    assert_eq!(legacy_keys_of(&l.doc, l.linear_path), (true, true));
    assert!(!style_of(&l.doc, l.linear_path).fill.enabled);
}

#[test]
fn paint_solid_on_a_gradient_object_uses_the_stored_colour_and_drops_both_keys_in_one_commit() {
    let l = legacy();
    separate_label(&l.doc);
    let before = change_count(&l.doc);
    l.doc
        .edit_style(&[l.linear_path], &StyleEdit::FillEnabled(true))
        .unwrap();
    assert_eq!(change_count(&l.doc), before + 1, "one commit");
    let s = style_of(&l.doc, l.linear_path);
    assert!(s.fill.enabled);
    assert_eq!(
        s.fill.color,
        rgb(0, 128, 0),
        "the object's stored solid colour"
    );
    assert_eq!(s.fill.opacity.get(), 0.5);
    assert_eq!(legacy_keys_of(&l.doc, l.linear_path), (false, false));
    // The other gradient object is untouched.
    assert_eq!(legacy_keys_of(&l.doc, l.radial_rect), (true, true));
    assert!(!style_of(&l.doc, l.radial_rect).fill.enabled);
}

#[test]
fn paint_solid_on_a_gradient_object_without_a_stored_colour_is_black() {
    let l = legacy();
    l.doc
        .edit_style(&[l.radial_rect], &StyleEdit::FillEnabled(true))
        .unwrap();
    let s = style_of(&l.doc, l.radial_rect);
    assert!(s.fill.enabled);
    assert_eq!(s.fill.color, Color::BLACK);
    assert_eq!(s.fill.opacity.get(), 1.0);
    assert_eq!(legacy_keys_of(&l.doc, l.radial_rect), (false, false));
}

#[test]
fn every_fill_edit_replaces_the_gradient_and_none_of_them_turns_the_fill_on() {
    let edits = [
        StyleEdit::FillColor(rgb(9, 9, 9)),
        StyleEdit::FillOpacity(op(0.3)),
        StyleEdit::FillRgba(rgb(9, 9, 9), op(0.3)),
    ];
    for edit in edits {
        let l = legacy();
        l.doc.edit_style(&[l.linear_path], &edit).unwrap();
        assert_eq!(
            legacy_keys_of(&l.doc, l.linear_path),
            (false, false),
            "{edit:?} drops the legacy keys in the same commit"
        );
        let s = style_of(&l.doc, l.linear_path);
        assert!(
            !s.fill.enabled,
            "{edit:?}: a fill value edit does not turn a fill on, and a gradient was off"
        );
        // And it survives a save and a reopen.
        let again = reopen(&l.doc);
        assert!(!style_of(&again, l.linear_path).fill.enabled);
        assert_eq!(legacy_keys_of(&again, l.linear_path), (false, false));
    }
}

#[test]
fn a_fill_edit_after_the_gradient_is_dropped_and_the_fill_turned_on_stays_solid_forever() {
    let l = legacy();
    l.doc
        .edit_style(&[l.linear_path], &StyleEdit::FillColor(rgb(5, 6, 7)))
        .unwrap();
    l.doc
        .edit_style(&[l.linear_path], &StyleEdit::FillEnabled(true))
        .unwrap();
    let again = reopen(&l.doc);
    let s = style_of(&again, l.linear_path);
    assert!(s.fill.enabled);
    assert_eq!(s.fill.color, rgb(5, 6, 7));
}

#[test]
fn a_multi_object_fill_edit_over_a_gradient_and_a_solid_object_is_one_commit() {
    let l = legacy();
    separate_label(&l.doc);
    let before = change_count(&l.doc);
    l.doc
        .edit_style(
            &[l.linear_path, l.solid_ellipse, l.radial_rect],
            &StyleEdit::FillColor(rgb(70, 80, 90)),
        )
        .unwrap();
    assert_eq!(change_count(&l.doc), before + 1);
    assert!(
        style_of(&l.doc, l.solid_ellipse).fill.enabled,
        "solid stays on"
    );
    assert!(!style_of(&l.doc, l.linear_path).fill.enabled);
    assert!(!style_of(&l.doc, l.radial_rect).fill.enabled);
    for id in [l.linear_path, l.radial_rect, l.solid_ellipse] {
        assert_eq!(style_of(&l.doc, id).fill.color, rgb(70, 80, 90));
    }
}

#[test]
fn paint_none_on_a_gradient_object_makes_no_visible_change_and_does_not_panic() {
    let l = legacy();
    let before = styles(&l.doc);
    l.doc
        .edit_style(&[l.linear_path], &StyleEdit::FillEnabled(false))
        .unwrap();
    assert_eq!(styles(&l.doc), before);
}

// ----------------------------------------------- AC 50: nothing is written

#[test]
fn a_written_document_never_holds_a_gradient_key_or_word() {
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    for edit in [
        StyleEdit::FillEnabled(true),
        StyleEdit::FillColor(rgb(10, 20, 30)),
        StyleEdit::FillOpacity(op(0.4)),
        StyleEdit::FillRgba(rgb(1, 2, 3), op(128.0 / 255.0)),
        StyleEdit::StrokeRgba(rgb(4, 5, 6), op(0.5)),
        StyleEdit::FillEnabled(false),
    ] {
        doc.edit_style(&[id], &edit).unwrap();
    }
    let json = String::from_utf8(doc.export_json().unwrap())
        .unwrap()
        .to_lowercase();
    for word in [
        "gradient",
        "fill_kind",
        "fill_stops",
        "linear",
        "radial",
        "stop",
    ] {
        assert!(!json.contains(word), "export contains {word}");
    }
    for meta in metas(&doc) {
        assert!(!has_key(&meta, "fill_kind"));
        assert!(!has_key(&meta, "fill_stops"));
    }
    let packed = pack(&doc, "tester").unwrap();
    let again = unpack(2, &packed).unwrap();
    for meta in metas(&again) {
        assert!(!has_key(&meta, "fill_kind"));
        assert!(!has_key(&meta, "fill_stops"));
    }
}

#[test]
fn an_unknown_fill_kind_string_is_still_a_damaged_file() {
    // adrs.md: validation keeps `fill_kind` one of solid/linear/radial.
    let doc = Document::new(1);
    let _ = rect(&doc, 0.0);
    let loro = loro_of(&doc);
    let tree = loro.get_tree("paths");
    let node = tree.roots()[0];
    tree.get_meta(node)
        .unwrap()
        .insert("fill_kind", "conic")
        .unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let result = unpack(3, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes));
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "{:?}",
        result.err()
    );
}

#[test]
fn a_garbage_fill_stops_value_is_ignored_not_fatal() {
    // adrs.md: `fill_stops` is accepted in any form and never read.
    let doc = Document::new(1);
    let _ = rect(&doc, 0.0);
    let loro = loro_of(&doc);
    let tree = loro.get_tree("paths");
    let node = tree.roots()[0];
    let meta = tree.get_meta(node).unwrap();
    meta.insert("fill_kind", "linear").unwrap();
    meta.insert("fill_stops", "not a list").unwrap();
    meta.insert("fill_enabled", true).unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let opened = unpack(3, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes)).unwrap();
    assert!(!style_of(&opened, opened.object_ids()[0]).fill.enabled);
}

// ------------------------------------------------ AC 54: format version 9

#[test]
fn the_format_version_is_nine_and_new_files_say_so() {
    assert_eq!(CURRENT_FORMAT_VERSION, 9);
    let doc = Document::new(1);
    let _ = rect(&doc, 0.0);
    assert_eq!(manifest_version(&pack(&doc, "t").unwrap()), 9);
}

#[test]
fn removing_gradients_does_not_bump_the_version_of_old_files_nor_rewrite_them() {
    // Opening a v7 file and saving it again keeps working and writes version 9
    // only because the container is always written at the current version.
    let l = legacy();
    let saved = pack(&l.doc, "t").unwrap();
    assert_eq!(manifest_version(&saved), u64::from(CURRENT_FORMAT_VERSION));
}

#[test]
fn every_older_fixture_still_opens_with_the_styles_it_stored() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let bad = [
        "future_format_version.curvyo",
        "malformed_paths.curvyo",
        "not_a_project.txt",
        "truncated.curvyo",
        "size_damaged_v7.curvyo",
        "size_damaged_v7.resaved_by_v7_reader.curvyo",
        "baseline_pre_0015_export.json",
        "rect_outline_mixed_radii.json",
    ];
    let mut opened = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if bad.contains(&name.as_str()) || !name.ends_with(".curvyo") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let doc = unpack(5, &bytes).unwrap_or_else(|e| panic!("{name} must open: {e:?}"));
        let before = op_count(&doc);
        let first = styles(&doc);
        // No gradient survives into the in-memory style: reading is pure.
        assert_eq!(first, styles(&doc), "{name}");
        assert_eq!(op_count(&doc), before, "{name}: opening wrote something");
        opened += 1;
    }
    assert!(opened >= 8, "only {opened} fixtures opened");
}

// ------------------------------------ AC 32, 33, 54: odd and long dash lists

#[test]
fn the_dash_model_accepts_odd_long_and_zero_on_lists_and_refuses_bad_numbers() {
    for ok in [
        vec![1.0],
        vec![1.0, 2.0, 4.0],
        vec![0.0, 3.0],
        vec![5.0, 0.0],
        vec![0.5; 17],
        vec![1e6, 1.0],
        vec![f64::MIN_POSITIVE],
    ] {
        assert!(DashPattern::new(ok.clone()).is_ok(), "{ok:?}");
    }
    assert!(DashPattern::new(vec![]).unwrap().is_solid());
    for bad in [
        vec![0.0],
        vec![0.0, 0.0],
        vec![-1.0, 2.0],
        vec![1.0, f64::NAN],
        vec![f64::INFINITY, 1.0],
        vec![f64::NEG_INFINITY],
        vec![1.0, -0.000_001],
    ] {
        assert!(DashPattern::new(bad.clone()).is_err(), "{bad:?}");
    }
}

#[test]
fn odd_long_and_zero_on_lists_survive_pack_and_unpack_as_typed() {
    let doc = Document::new(1);
    let a = rect(&doc, 0.0);
    let b = rect(&doc, 20.0);
    let c = rect(&doc, 40.0);
    doc.edit_style(
        &[a],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 2.0, 4.0]).unwrap()),
    )
    .unwrap();
    let seventeen: Vec<f64> = (1..=17).map(f64::from).collect();
    doc.edit_style(
        &[b],
        &StyleEdit::StrokeDash(DashPattern::new(seventeen.clone()).unwrap()),
    )
    .unwrap();
    doc.edit_style(
        &[c],
        &StyleEdit::StrokeDash(DashPattern::new(vec![0.0, 3.0]).unwrap()),
    )
    .unwrap();
    let again = reopen(&doc);
    assert_eq!(style_of(&again, a).stroke.dash.as_slice(), &[1.0, 2.0, 4.0]);
    assert_eq!(
        style_of(&again, b).stroke.dash.as_slice(),
        seventeen.as_slice()
    );
    assert_eq!(style_of(&again, c).stroke.dash.as_slice(), &[0.0, 3.0]);
}

#[test]
fn the_dash_golden_opens_with_its_three_lists_and_is_a_version_nine_container() {
    let bytes = fixture("dash_v9.curvyo");
    assert_eq!(manifest_version(&bytes), 9);
    let doc = unpack(5, &bytes).unwrap();
    let mut lists: Vec<Vec<f64>> = styles(&doc)
        .iter()
        .map(|s| s.stroke.dash.as_slice().to_vec())
        .collect();
    lists.sort_by_key(Vec::len);
    assert_eq!(lists[0], vec![0.0, 3.0]);
    assert_eq!(lists[1], vec![1.0, 2.0, 4.0]);
    assert_eq!(lists[2].len(), 17);
    // Looking at it writes nothing, and a save keeps all three.
    let before = op_count(&doc);
    let _ = styles(&doc);
    assert_eq!(op_count(&doc), before);
    let mut again: Vec<Vec<f64>> = styles(&reopen(&doc))
        .iter()
        .map(|s| s.stroke.dash.as_slice().to_vec())
        .collect();
    again.sort_by_key(Vec::len);
    assert_eq!(again, lists);
}

#[test]
fn a_version_eight_container_holding_an_odd_dash_list_is_never_silently_altered() {
    // Spec criterion 54: an odd list needs version 9. A file that claims 8 yet
    // holds one was not written by any build; the open must either refuse it
    // (the "damaged" message) or read the list as stored. It must not double
    // it, drop it or panic.
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 2.0, 4.0]).unwrap()),
    )
    .unwrap();
    let bytes = container_from_loro(8, &doc.export_loro_snapshot().unwrap());
    match unpack(3, &bytes) {
        Err(OpenError::Damaged) => {}
        Ok(opened) => {
            let s = style_of(&opened, opened.object_ids()[0]);
            assert_eq!(s.stroke.dash.as_slice(), &[1.0, 2.0, 4.0]);
        }
        Err(other) => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_version_eight_container_with_an_even_dash_list_opens_unchanged() {
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 3.0, 1.0, 3.0]).unwrap()),
    )
    .unwrap();
    let bytes = container_from_loro(8, &doc.export_loro_snapshot().unwrap());
    let opened = unpack(3, &bytes).unwrap();
    assert_eq!(
        style_of(&opened, opened.object_ids()[0])
            .stroke
            .dash
            .as_slice(),
        &[6.0, 3.0, 1.0, 3.0]
    );
}

#[test]
fn damaged_dash_values_in_a_file_are_still_refused() {
    for bad in [
        vec![-1.0, 2.0],
        vec![0.0, 0.0],
        vec![f64::NAN, 1.0],
        vec![f64::INFINITY, 1.0],
        vec![0.0],
    ] {
        let doc = Document::new(1);
        let _ = rect(&doc, 0.0);
        let loro = loro_of(&doc);
        let tree = loro.get_tree("paths");
        let meta = tree.get_meta(tree.roots()[0]).unwrap();
        let list: Vec<LoroValue> = bad.iter().map(|v| LoroValue::from(*v)).collect();
        meta.insert("stroke_dash", LoroValue::from(list)).unwrap();
        loro.commit();
        let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
        let result = unpack(3, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes));
        assert!(
            matches!(result, Err(OpenError::Damaged)),
            "{bad:?} must be damaged, got {:?}",
            result.err()
        );
    }
}

// ----------------------------- AC 9, 13, 15: the RGBA edits and the registers

#[test]
fn stroke_rgba_stores_alpha_exactly_as_typed_and_turns_the_stroke_on() {
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let alpha = 128.0 / 255.0;
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeRgba(rgb(0x2F, 0x6F, 0xEE), op(alpha)),
    )
    .unwrap();
    let s = style_of(&reopen(&doc), id).stroke;
    assert!(s.enabled);
    assert_eq!(s.color, rgb(0x2F, 0x6F, 0xEE));
    assert_eq!(s.opacity.get(), alpha, "not rounded to a whole percent");
}

#[test]
fn fill_rgba_never_turns_a_fill_on_and_a_second_identical_edit_writes_nothing() {
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::FillRgba(rgb(1, 2, 3), op(0.5)))
        .unwrap();
    let s = style_of(&doc, id).fill;
    assert!(!s.enabled);
    assert_eq!((s.color, s.opacity.get()), (rgb(1, 2, 3), 0.5));
    let before = op_count(&doc);
    doc.edit_style(&[id], &StyleEdit::FillRgba(rgb(1, 2, 3), op(0.5)))
        .unwrap();
    assert_eq!(op_count(&doc), before);
    doc.edit_style(&[id], &StyleEdit::StrokeRgba(rgb(0, 0, 0), op(1.0)))
        .unwrap();
    assert_eq!(
        op_count(&doc),
        before,
        "default stroke colour/opacity writes nothing"
    );
}

#[test]
fn rgba_edit_over_several_objects_is_one_commit_and_each_object_ends_identical() {
    let doc = Document::new(1);
    let ids = [rect(&doc, 0.0), rect(&doc, 20.0), rect(&doc, 40.0)];
    doc.edit_style(&[ids[0]], &StyleEdit::StrokeOpacity(op(0.2)))
        .unwrap();
    doc.edit_style(&[ids[1]], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    separate_label(&doc);
    let before = change_count(&doc);
    doc.edit_style(&ids, &StyleEdit::StrokeRgba(rgb(9, 8, 7), op(0.5)))
        .unwrap();
    assert_eq!(change_count(&doc), before + 1);
    for id in ids {
        let s = style_of(&doc, id).stroke;
        assert!(s.enabled, "the off stroke is turned on by a colour edit");
        assert_eq!((s.color, s.opacity.get()), (rgb(9, 8, 7), 0.5));
    }
}

#[test]
fn width_zero_keeps_the_last_width_and_a_later_solid_restores_it_and_defaults_to_025() {
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
        .unwrap();
    let s = style_of(&doc, id).stroke;
    assert!(!s.enabled);
    assert_eq!(s.width.as_mm(), 2.0);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    assert_eq!(style_of(&doc, id).stroke.width.as_mm(), 2.0);

    // Never above zero (width 0 straight away): default 0.25 on Solid.
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    assert_eq!(style_of(&doc, id).stroke.width.as_mm(), 0.25);
    // Negative zero is zero.
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(-0.0)))
        .unwrap();
    assert!(!style_of(&doc, id).stroke.enabled);
    assert!(
        doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(-0.001)))
            .is_err()
    );
    assert!(
        doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(f64::NAN)))
            .is_err()
    );
}

#[test]
fn paint_none_then_solid_restores_every_value() {
    // Criterion 7.
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeRgba(rgb(255, 0, 0), op(128.0 / 255.0)),
    )
    .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 3.0]).unwrap()),
    )
    .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeJoin(curvyo_document_core::LineJoin::Round),
    )
    .unwrap();
    let before = style_of(&doc, id);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    assert_eq!(style_of(&doc, id), before);
    assert_eq!(style_of(&reopen(&doc), id), before);
}

#[test]
fn dash_join_and_cap_edits_never_change_the_on_off_state() {
    // Criterion 9, second half.
    let doc = Document::new(1);
    let id = rect(&doc, 0.0);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    )
    .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeJoin(curvyo_document_core::LineJoin::Bevel),
    )
    .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeCap(curvyo_document_core::LineCap::Round),
    )
    .unwrap();
    assert!(!style_of(&doc, id).stroke.enabled);
}

// -------------------------------------------------------- CRDT merge

fn fork(doc: &Document, peer: u64) -> Document {
    unpack(peer, &pack(doc, "t").unwrap()).unwrap()
}

#[test]
fn concurrent_dash_and_colour_edits_both_survive_a_merge_in_either_order() {
    let base = Document::new(1);
    let id = rect(&base, 0.0);
    let a = fork(&base, 2);
    let b = fork(&base, 3);
    a.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 2.0, 4.0]).unwrap()),
    )
    .unwrap();
    b.edit_style(
        &[id],
        &StyleEdit::StrokeRgba(rgb(10, 20, 30), op(128.0 / 255.0)),
    )
    .unwrap();
    for merged_doc in [merged(&a, &b), merged(&b, &a)] {
        let s = style_of(&merged_doc, id).stroke;
        assert_eq!(s.dash.as_slice(), &[1.0, 2.0, 4.0]);
        assert_eq!(s.color, rgb(10, 20, 30));
        assert_eq!(s.opacity.get(), 128.0 / 255.0);
    }
}

#[test]
fn concurrent_rgba_and_opacity_edits_converge_to_the_same_style_everywhere() {
    let base = Document::new(1);
    let id = rect(&base, 0.0);
    let a = fork(&base, 2);
    let b = fork(&base, 3);
    a.edit_style(&[id], &StyleEdit::StrokeRgba(rgb(1, 1, 1), op(0.25)))
        .unwrap();
    b.edit_style(&[id], &StyleEdit::StrokeOpacity(op(0.75)))
        .unwrap();
    let ab = style_of(&merged(&a, &b), id);
    let ba = style_of(&merged(&b, &a), id);
    assert_eq!(ab, ba, "merge is order independent");
    assert_eq!(
        ab.stroke.color,
        rgb(1, 1, 1),
        "the colour register has one writer"
    );
    assert!(ab.stroke.opacity.get() == 0.25 || ab.stroke.opacity.get() == 0.75);
}

#[test]
fn concurrent_dash_lists_resolve_to_exactly_one_of_them_never_a_blend() {
    let base = Document::new(1);
    let id = rect(&base, 0.0);
    let a = fork(&base, 2);
    let b = fork(&base, 3);
    a.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 2.0, 4.0]).unwrap()),
    )
    .unwrap();
    b.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![9.0, 1.0]).unwrap()),
    )
    .unwrap();
    let ab = style_of(&merged(&a, &b), id).stroke.dash;
    let ba = style_of(&merged(&b, &a), id).stroke.dash;
    assert_eq!(ab, ba);
    assert!(
        ab.as_slice() == [1.0, 2.0, 4.0] || ab.as_slice() == [9.0, 1.0],
        "{ab:?}"
    );
}

#[test]
fn a_fill_edit_on_one_peer_and_a_legacy_gradient_survive_a_merge_without_error() {
    let l = legacy();
    let a = fork(&l.doc, 2);
    let b = fork(&l.doc, 3);
    a.edit_style(&[l.linear_path], &StyleEdit::FillEnabled(true))
        .unwrap();
    b.edit_style(&[l.linear_path], &StyleEdit::StrokeColor(rgb(1, 2, 3)))
        .unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let s = style_of(&m, l.linear_path);
        assert!(s.fill.enabled);
        assert_eq!(s.stroke.color, rgb(1, 2, 3));
        assert_eq!(s.fill.color, rgb(0, 128, 0));
    }
}
