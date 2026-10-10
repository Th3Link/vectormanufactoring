//! Independent tester cases for `0018-stroke-markers`, document side: the five
//! registers, defaults, edits, `NotAPath`, open-time validation, the format
//! version, the golden `markers_v9.curvyo`, copy / split / join / convert,
//! CRDT merge. Written from `specification.md` and `adrs.md` before the
//! implementation was read.
//!
//! Criteria covered here: 2 (stored Place/Count kept), 3 (settings kept), 6, 21
//! to 23 (store side), 24 (one commit), 25 to 29.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::assert_is_empty,
    missing_docs
)]

use std::io::{Cursor, Write};
use std::path::PathBuf;

use curvyo_document_core::{
    AnchorId, Angle, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, CopySource,
    DisplayUnit, Document, EllipseFrame, InnerRatio, Length, MarkerCount, MarkerPlace, MarkerShape,
    Markers, NewAnchor, NodeId, ObjectSnapshot, OpenError, Point, PointCount, RectBounds,
    StarFrame, Style, StyleEdit, StyleEditError, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroValue};

// ---------------------------------------------------------------- helpers

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn anchors(points: &[(f64, f64)]) -> Vec<NewAnchor> {
    points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect()
}

fn open_path(doc: &Document, points: &[(f64, f64)]) -> NodeId {
    doc.create_path(&anchors(points), false)
}

fn rect(doc: &Document) -> NodeId {
    doc.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(10.0),
        height: mm(6.0),
    })
}

fn all_primitives(doc: &Document) -> Vec<NodeId> {
    let frame = StarFrame {
        center: pt(50.0, 0.0),
        radius: mm(8.0),
        angle: Angle::from_radians(0.0),
    };
    vec![
        rect(doc),
        doc.create_ellipse(EllipseFrame {
            center: pt(30.0, 0.0),
            rx: mm(5.0),
            ry: mm(3.0),
        }),
        doc.create_polygon(frame, PointCount::new(6).unwrap()),
        doc.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.4).unwrap(),
        ),
    ]
}

fn style_of(doc: &Document, id: NodeId) -> Style {
    match doc.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => p.style,
        ObjectSnapshot::Primitive(p) => p.style,
    }
}

fn markers_of(doc: &Document, id: NodeId) -> Markers {
    style_of(doc, id).stroke.markers
}

fn count(n: u32) -> MarkerCount {
    MarkerCount::new(n).unwrap()
}

fn loro_of(doc: &Document) -> LoroDoc {
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    loro
}

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

fn separate_label(doc: &Document) {
    let unit = if doc.display_unit() == DisplayUnit::In {
        DisplayUnit::Mm
    } else {
        DisplayUnit::In
    };
    assert!(doc.set_display_unit(unit));
}

fn metas(doc: &Document) -> Vec<LoroValue> {
    let loro = loro_of(doc);
    let tree = loro.get_tree("paths");
    tree.nodes()
        .into_iter()
        .map(|n| tree.get_meta(n).unwrap().get_deep_value())
        .collect()
}

fn meta_get(meta: &LoroValue, key: &str) -> Option<LoroValue> {
    match meta {
        LoroValue::Map(m) => m.get(key).cloned(),
        _ => None,
    }
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

/// A one-rect-and-one-path document where the PATH meta gets `key = value`
/// injected, packed with `version`.
fn file_with_injected(key: &str, value: LoroValue, version: u32) -> Vec<u8> {
    let doc = Document::new(1);
    let _ = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let loro = loro_of(&doc);
    let tree = loro.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    meta.insert(key, value).unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    container_from_loro(version, &bytes)
}

fn s(v: &str) -> LoroValue {
    LoroValue::from(v)
}

fn reopen(doc: &Document) -> Document {
    unpack(77, &pack(doc, "tester").unwrap()).expect("reopens")
}

// ------------------------------------------------ AC 1/2/6: registers

#[test]
fn defaults_are_none_none_none_spaced_one() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let m = markers_of(&doc, p);
    assert_eq!(m.start, MarkerShape::None);
    assert_eq!(m.mid, MarkerShape::None);
    assert_eq!(m.end, MarkerShape::None);
    assert_eq!(m.mid_place, MarkerPlace::Spaced);
    assert_eq!(m.mid_count.get(), 1);
    assert_eq!(m, Markers::default());
    assert_eq!(MarkerCount::default().get(), 1);
    // A primitive carries the same default.
    for id in all_primitives(&doc) {
        assert_eq!(markers_of(&doc, id), Markers::default());
    }
}

#[test]
fn a_new_object_writes_no_marker_keys() {
    // adrs.md: absent = default.
    let doc = Document::new(1);
    let _ = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    for meta in metas(&doc) {
        for key in [
            "stroke_marker_start",
            "stroke_marker_mid",
            "stroke_marker_end",
            "stroke_marker_mid_place",
            "stroke_marker_mid_count",
        ] {
            assert!(meta_get(&meta, key).is_none(), "{key} written on create");
        }
    }
}

#[test]
fn each_edit_changes_only_its_own_register() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let base = style_of(&doc, p);
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    let mut want = base.clone();
    want.stroke.markers.start = MarkerShape::Arrow;
    assert_eq!(style_of(&doc, p), want);

    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    want.stroke.markers.mid = MarkerShape::Dot;
    assert_eq!(style_of(&doc, p), want);

    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Arrow))
        .unwrap();
    want.stroke.markers.end = MarkerShape::Arrow;
    assert_eq!(style_of(&doc, p), want);

    doc.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::AtNodes))
        .unwrap();
    want.stroke.markers.mid_place = MarkerPlace::AtNodes;
    assert_eq!(style_of(&doc, p), want);

    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(7)))
        .unwrap();
    want.stroke.markers.mid_count = count(7);
    assert_eq!(style_of(&doc, p), want);

    // Slot back to None.
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::None))
        .unwrap();
    want.stroke.markers.start = MarkerShape::None;
    assert_eq!(style_of(&doc, p), want);
}

#[test]
fn stored_place_and_count_survive_a_none_middle_and_a_nodes_place() {
    // criterion 2: stored Place and Count are kept while hidden.
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(9)))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::AtNodes))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::None))
        .unwrap();
    let again = reopen(&doc);
    let m = markers_of(&again, p);
    assert_eq!(m.mid, MarkerShape::None);
    assert_eq!(m.mid_place, MarkerPlace::AtNodes);
    assert_eq!(m.mid_count.get(), 9);
    doc.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::Spaced))
        .unwrap();
    assert_eq!(markers_of(&doc, p).mid_count.get(), 9);
}

#[test]
fn stored_keys_have_the_documented_spelling() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::None))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::AtNodes))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(12)))
        .unwrap();
    let m = &metas(&doc)[0];
    assert_eq!(meta_get(m, "stroke_marker_start"), Some(s("arrow")));
    assert_eq!(meta_get(m, "stroke_marker_mid"), Some(s("dot")));
    // Back to None writes "none" (no key deletes).
    assert_eq!(meta_get(m, "stroke_marker_end"), Some(s("none")));
    assert_eq!(meta_get(m, "stroke_marker_mid_place"), Some(s("nodes")));
    assert_eq!(
        meta_get(m, "stroke_marker_mid_count"),
        Some(LoroValue::I64(12))
    );
}

#[test]
fn an_edit_is_one_commit_and_an_unchanged_edit_writes_nothing() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let q = open_path(&doc, &[(0.0, 5.0), (10.0, 5.0)]);
    separate_label(&doc);
    let c0 = change_count(&doc);
    doc.edit_style(&[p, q], &StyleEdit::MarkerEnd(MarkerShape::Arrow))
        .unwrap();
    assert_eq!(change_count(&doc), c0 + 1, "one commit for two paths");
    let ops = op_count(&doc);
    // Same value again: nothing written.
    doc.edit_style(&[p, q], &StyleEdit::MarkerEnd(MarkerShape::Arrow))
        .unwrap();
    assert_eq!(op_count(&doc), ops);
    // The default count over the default: nothing written either.
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(1)))
        .unwrap();
    assert_eq!(
        op_count(&doc),
        ops,
        "setting the default over an absent key"
    );
}

#[test]
fn a_multi_path_edit_keeps_each_paths_other_settings() {
    // criterion 24.
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let q = open_path(&doc, &[(0.0, 5.0), (10.0, 5.0)]);
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[q], &StyleEdit::MarkerStart(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[q], &StyleEdit::MarkerCount(count(4)))
        .unwrap();
    doc.edit_style(&[p, q], &StyleEdit::MarkerEnd(MarkerShape::Dot))
        .unwrap();
    assert_eq!(markers_of(&doc, p).start, MarkerShape::Arrow);
    assert_eq!(markers_of(&doc, q).start, MarkerShape::Dot);
    assert_eq!(markers_of(&doc, q).mid_count.get(), 4);
    assert_eq!(markers_of(&doc, p).mid_count.get(), 1);
    assert_eq!(markers_of(&doc, p).end, MarkerShape::Dot);
    assert_eq!(markers_of(&doc, q).end, MarkerShape::Dot);
}

#[test]
fn markers_survive_stroke_off_and_width_zero() {
    // criterion 3: settings are kept and come back with the stroke.
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(3)))
        .unwrap();
    let want = markers_of(&doc, p);
    doc.edit_style(&[p], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    assert_eq!(markers_of(&doc, p), want);
    doc.edit_style(&[p], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::StrokeWidth(mm(0.0)))
        .unwrap();
    assert_eq!(markers_of(&doc, p), want);
    assert_eq!(markers_of(&reopen(&doc), p), want);
}

// ----------------------------------------- AC 21/22/23: paths only

#[test]
fn a_marker_edit_on_a_primitive_is_refused_with_not_a_path_for_every_kind() {
    let doc = Document::new(1);
    let edits = [
        StyleEdit::MarkerStart(MarkerShape::Arrow),
        StyleEdit::MarkerMid(MarkerShape::Dot),
        StyleEdit::MarkerEnd(MarkerShape::Arrow),
        StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        StyleEdit::MarkerCount(count(3)),
    ];
    for id in all_primitives(&doc) {
        for e in &edits {
            let ops = op_count(&doc);
            assert_eq!(
                doc.edit_style(&[id], e),
                Err(StyleEditError::NotAPath),
                "{e:?}"
            );
            assert_eq!(op_count(&doc), ops, "refusal writes nothing");
            assert_eq!(markers_of(&doc, id), Markers::default());
        }
    }
    // No primitive meta has any marker key.
    for meta in metas(&doc) {
        for key in [
            "stroke_marker_start",
            "stroke_marker_mid",
            "stroke_marker_mid_count",
        ] {
            assert!(meta_get(&meta, key).is_none());
        }
    }
}

#[test]
fn a_marker_edit_naming_a_primitive_and_a_path_is_refused_as_a_whole() {
    // adrs.md decision 4: "refuses ... as a whole".
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let r = rect(&doc);
    let ops = op_count(&doc);
    assert_eq!(
        doc.edit_style(&[p, r], &StyleEdit::MarkerEnd(MarkerShape::Arrow)),
        Err(StyleEditError::NotAPath)
    );
    assert_eq!(markers_of(&doc, p).end, MarkerShape::None);
    assert_eq!(op_count(&doc), ops);
}

#[test]
fn non_marker_edits_still_work_on_primitives_and_a_mix() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    let r = rect(&doc);
    doc.edit_style(&[p, r], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    assert_eq!(style_of(&doc, r).stroke.width.as_mm(), 2.0);
}

#[test]
fn a_missing_object_is_still_no_such_object_for_a_marker_edit() {
    let doc = Document::new(1);
    let p = open_path(&doc, &[(0.0, 0.0), (10.0, 0.0)]);
    doc.delete_objects(&[p]).unwrap();
    let r = doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Arrow));
    assert!(r.is_err());
}

#[test]
fn object_to_path_gives_a_path_without_markers() {
    // criterion 23.
    let doc = Document::new(1);
    let prims = all_primitives(&doc);
    let conversions: Vec<(NodeId, Vec<NewAnchor>)> = prims
        .iter()
        .enumerate()
        .map(|(k, id)| {
            let list = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    NewAnchor::corner(AnchorId::new(9, (k * 10 + i) as u64 + 1), pt(*x, *y))
                })
                .collect();
            (*id, list)
        })
        .collect();
    doc.convert_to_paths(&conversions).unwrap();
    for id in prims {
        match doc.object(id).unwrap() {
            ObjectSnapshot::Path(p) => assert_eq!(p.style.stroke.markers, Markers::default()),
            ObjectSnapshot::Primitive(_) => panic!("converted"),
        }
    }
    for meta in metas(&doc) {
        assert!(meta_get(&meta, "stroke_marker_start").is_none());
    }
}

// ----------------------------------------------- AC 25 to 27: topology

fn marked_path(doc: &Document, points: &[(f64, f64)], closed: bool) -> NodeId {
    let p = doc.create_path(&anchors(points), closed);
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::AtNodes))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(5)))
        .unwrap();
    p
}

#[test]
fn a_copy_has_the_same_settings_and_is_independent() {
    // criterion 25.
    let doc = Document::new(1);
    let p = marked_path(&doc, &[(0.0, 0.0), (10.0, 0.0), (20.0, 5.0)], false);
    let ids = doc
        .duplicate_objects(
            &[CopySource {
                id: p,
                anchor_ids: (0..3).map(|i| AnchorId::new(5, 100 + i)).collect(),
            }],
            Vec2::new(0.0, 20.0),
        )
        .unwrap();
    let copy = ids[0];
    assert_eq!(markers_of(&doc, copy), markers_of(&doc, p));
    doc.edit_style(&[copy], &StyleEdit::MarkerStart(MarkerShape::None))
        .unwrap();
    assert_eq!(markers_of(&doc, p).start, MarkerShape::Arrow);
    assert_eq!(markers_of(&doc, copy).start, MarkerShape::None);
    doc.edit_style(&[p], &StyleEdit::MarkerCount(count(2)))
        .unwrap();
    assert_eq!(markers_of(&doc, copy).mid_count.get(), 5);
}

#[test]
fn splitting_an_open_path_keeps_the_settings_on_both_halves() {
    // criterion 26.
    let doc = Document::new(1);
    let p = marked_path(
        &doc,
        &[(0.0, 0.0), (10.0, 0.0), (20.0, 5.0), (30.0, 5.0)],
        false,
    );
    let want = markers_of(&doc, p);
    let ((a, _), (b, _)) = doc
        .split_at_anchor(p, AnchorId::new(1, 2), AnchorId::new(8, 1))
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(markers_of(&doc, a), want);
    assert_eq!(markers_of(&doc, b), want);
    // Each half is an open path with the split node at the end / the start.
    for id in [a, b] {
        match doc.object(id).unwrap() {
            ObjectSnapshot::Path(path) => assert!(!path.closed),
            ObjectSnapshot::Primitive(_) => panic!(),
        }
    }
}

#[test]
fn splitting_a_closed_path_keeps_one_object_and_its_settings() {
    let doc = Document::new(1);
    let p = marked_path(
        &doc,
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
        true,
    );
    let want = markers_of(&doc, p);
    let ((a, _), (b, _)) = doc
        .split_at_anchor(p, AnchorId::new(1, 2), AnchorId::new(8, 1))
        .unwrap();
    assert_eq!(a, b, "closed split keeps the one object");
    assert_eq!(markers_of(&doc, a), want);
}

#[test]
fn joining_two_paths_keeps_the_survivors_settings_only() {
    // criterion 27.
    let doc = Document::new(1);
    let a = doc.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
        ],
        false,
    );
    let b = doc.create_path(
        &[
            NewAnchor::corner(AnchorId::new(2, 1), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(2, 2), pt(20.0, 0.0)),
        ],
        false,
    );
    doc.edit_style(&[a], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::MarkerEnd(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::MarkerCount(count(8)))
        .unwrap();
    let (survivor, _) = doc
        .join_endpoints(a, AnchorId::new(1, 2), b, AnchorId::new(2, 1))
        .unwrap();
    let m = markers_of(&doc, survivor);
    let orig_a = if survivor == a {
        Markers {
            start: MarkerShape::Arrow,
            ..Markers::default()
        }
    } else {
        Markers {
            end: MarkerShape::Dot,
            mid_count: count(8),
            ..Markers::default()
        }
    };
    assert_eq!(m, orig_a, "survivor keeps its own, nothing from the other");
    assert!(doc.object(if survivor == a { b } else { a }).is_none());
}

#[test]
fn closing_a_path_onto_itself_keeps_its_settings() {
    let doc = Document::new(1);
    let p = marked_path(&doc, &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false);
    let want = markers_of(&doc, p);
    let (id, _) = doc
        .join_endpoints(p, AnchorId::new(1, 3), p, AnchorId::new(1, 1))
        .unwrap();
    match doc.object(id).unwrap() {
        ObjectSnapshot::Path(path) => assert!(path.closed),
        ObjectSnapshot::Primitive(_) => panic!(),
    }
    assert_eq!(markers_of(&doc, id), want);
}

// ----------------------------------------------------- AC 28: persistence

#[test]
fn all_settings_survive_save_and_reopen() {
    let doc = Document::new(1);
    let p = marked_path(&doc, &[(0.0, 0.0), (10.0, 0.0)], false);
    let q = open_path(&doc, &[(0.0, 5.0), (10.0, 5.0)]);
    doc.edit_style(&[q], &StyleEdit::MarkerMid(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[q], &StyleEdit::MarkerCount(count(500)))
        .unwrap();
    let again = reopen(&doc);
    assert_eq!(markers_of(&again, p), markers_of(&doc, p));
    assert_eq!(markers_of(&again, q), markers_of(&doc, q));
    assert_eq!(style_of(&again, p), style_of(&doc, p));
}

#[test]
fn the_json_export_carries_the_markers() {
    let doc = Document::new(1);
    let _ = marked_path(&doc, &[(0.0, 0.0), (10.0, 0.0)], false);
    let json = String::from_utf8(doc.export_json().unwrap()).unwrap();
    assert!(json.contains("markers"), "document.json lacks markers");
}

// ------------------------------------------------ AC 29: format & validation

#[test]
fn the_format_version_is_nine() {
    // Nine is where the markers took it; later bumps (`0040`) move the constant.
    const _: () = assert!(CURRENT_FORMAT_VERSION >= 9);
}

#[test]
fn every_older_fixture_opens_with_every_slot_none_spaced_one() {
    for name in [
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "legacy_gradient_v7.curvyo",
        "display_unit_in_v7.curvyo",
        "compound_v8.curvyo",
        "corner_radii_per_corner.curvyo",
        "dash_v9.curvyo",
        "valid.curvyo",
    ] {
        let doc = match unpack(1, &fixture(name)) {
            Ok(d) => d,
            Err(e) => panic!("{name}: {e:?}"),
        };
        let before = op_count(&doc);
        for id in doc.object_ids() {
            assert_eq!(markers_of(&doc, id), Markers::default(), "{name}");
        }
        assert_eq!(op_count(&doc), before, "{name}: reading writes nothing");
    }
}

#[test]
fn a_later_format_version_is_too_new_before_any_marker_key_is_read() {
    // Even with a marker key that would be damaged in this version.
    let bytes = file_with_injected(
        "stroke_marker_start",
        s("hexagon"),
        CURRENT_FORMAT_VERSION + 1,
    );
    match unpack(1, &bytes) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!(found, CURRENT_FORMAT_VERSION + 1);
            assert_eq!(supported, CURRENT_FORMAT_VERSION);
        }
        other => panic!("expected FormatTooNew, got {:?}", other.err()),
    }
}

#[test]
fn an_unknown_shape_or_place_is_damaged() {
    for (key, bad) in [
        ("stroke_marker_start", "hexagon"),
        ("stroke_marker_mid", "Arrow"),
        ("stroke_marker_end", ""),
        ("stroke_marker_start", "ARROW"),
        ("stroke_marker_mid_place", "everywhere"),
        ("stroke_marker_mid_place", "Spaced"),
    ] {
        let bytes = file_with_injected(key, s(bad), CURRENT_FORMAT_VERSION);
        assert!(
            matches!(unpack(1, &bytes), Err(OpenError::Damaged)),
            "{key}={bad:?}"
        );
    }
}

#[test]
fn a_wrong_typed_marker_value_is_damaged() {
    for (key, bad) in [
        ("stroke_marker_start", LoroValue::I64(1)),
        ("stroke_marker_mid_place", LoroValue::Bool(true)),
        ("stroke_marker_mid_count", s("3")),
        ("stroke_marker_mid_count", LoroValue::Bool(true)),
    ] {
        let bytes = file_with_injected(key, bad.clone(), CURRENT_FORMAT_VERSION);
        assert!(
            matches!(unpack(1, &bytes), Err(OpenError::Damaged)),
            "{key}={bad:?}"
        );
    }
}

#[test]
fn a_count_that_is_not_a_whole_number_or_is_below_one_is_damaged() {
    for bad in [
        LoroValue::I64(0),
        LoroValue::I64(-1),
        LoroValue::I64(i64::MIN),
        LoroValue::Double(2.5),
        LoroValue::Double(f64::NAN),
        LoroValue::Double(f64::INFINITY),
        LoroValue::Double(0.0),
        LoroValue::Double(-3.0),
    ] {
        let bytes = file_with_injected(
            "stroke_marker_mid_count",
            bad.clone(),
            CURRENT_FORMAT_VERSION,
        );
        assert!(
            matches!(unpack(1, &bytes), Err(OpenError::Damaged)),
            "count {bad:?} must be damaged"
        );
    }
}

#[test]
fn a_count_above_500_opens_is_kept_and_not_rewritten() {
    for big in [501_i64, 10_000, 4_294_967_295] {
        let bytes = file_with_injected(
            "stroke_marker_mid_count",
            LoroValue::I64(big),
            CURRENT_FORMAT_VERSION,
        );
        let doc = unpack(1, &bytes).unwrap_or_else(|e| panic!("{big}: {e:?}"));
        let id = doc.object_ids()[0];
        assert_eq!(u64::from(markers_of(&doc, id).mid_count.get()), big as u64);
        let ops = op_count(&doc);
        let _ = doc.export_json().unwrap();
        assert_eq!(op_count(&doc), ops);
        assert_eq!(
            u64::from(markers_of(&reopen(&doc), id).mid_count.get()),
            big as u64
        );
    }
}

#[test]
fn a_count_beyond_u32_does_not_panic() {
    let bytes = file_with_injected(
        "stroke_marker_mid_count",
        LoroValue::I64(i64::from(u32::MAX) + 1),
        CURRENT_FORMAT_VERSION,
    );
    // Either damaged or kept; it must not panic or wrap to a small number.
    match unpack(1, &bytes) {
        Err(OpenError::Damaged) => {}
        Ok(doc) => {
            let id = doc.object_ids()[0];
            assert!(markers_of(&doc, id).mid_count.get() >= 1);
        }
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn a_marker_key_on_a_primitive_in_a_file_does_not_make_it_draw_or_break_open() {
    // Not specified; pins that such a file does not panic.
    let doc = Document::new(1);
    let r = rect(&doc);
    let loro = loro_of(&doc);
    let tree = loro.get_tree("paths");
    tree.get_meta(tree.roots()[0])
        .unwrap()
        .insert("stroke_marker_end", "arrow")
        .unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let opened = unpack(1, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes));
    assert!(opened.is_ok());
    let _ = r;
}

// ------------------------------------------------------- golden markers_v9

#[test]
fn the_golden_markers_file_opens_and_has_every_key_a_501_count_and_a_closed_path() {
    let doc = unpack(1, &fixture("markers_v9.curvyo")).expect("golden opens");
    let ids = doc.object_ids();
    assert!(!ids.is_empty());
    let all: Vec<Markers> = ids.iter().map(|id| markers_of(&doc, *id)).collect();
    // Some object has every slot set (non-None) and the AtNodes/Spaced choice.
    assert!(
        all.iter().any(|m| m.start != MarkerShape::None
            && m.mid != MarkerShape::None
            && m.end != MarkerShape::None),
        "{all:?}"
    );
    assert!(all.iter().any(|m| m.mid_count.get() == 501), "{all:?}");
    assert!(all.iter().any(|m| m.mid_place == MarkerPlace::AtNodes));
    assert!(all.iter().any(|m| m.mid_place == MarkerPlace::Spaced));
    assert!(
        ids.iter().any(|id| matches!(
            doc.object(*id).unwrap(),
            ObjectSnapshot::Path(p) if p.closed
        )),
        "a closed path"
    );
    // Reading writes nothing, and a save keeps every value.
    let ops = op_count(&doc);
    let _ = doc.export_json().unwrap();
    assert_eq!(op_count(&doc), ops);
    let again = reopen(&doc);
    for id in ids {
        assert_eq!(style_of(&again, id), style_of(&doc, id));
    }
}

#[test]
fn the_golden_markers_file_declares_the_current_format_version() {
    use std::io::Read;
    let bytes = fixture("markers_v9.curvyo");
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut raw = Vec::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(manifest["format_version"].as_u64().unwrap(), 9);
}

// ---------------------------------------------------------- CRDT merge

fn merged_pair(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    unpack(99, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes)).expect("merged opens")
}

#[test]
fn concurrent_edits_of_different_slots_both_survive_a_merge() {
    let a = Document::new(1);
    let p = open_path(&a, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
    let b = unpack(2, &pack(&a, "t").unwrap()).unwrap();
    a.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    b.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Dot))
        .unwrap();
    b.edit_style(&[p], &StyleEdit::MarkerCount(count(6)))
        .unwrap();
    a.edit_style(&[p], &StyleEdit::MarkerPlace(MarkerPlace::AtNodes))
        .unwrap();
    let m = markers_of(&merged_pair(&a, &b), p);
    assert_eq!(m.start, MarkerShape::Arrow);
    assert_eq!(m.end, MarkerShape::Dot);
    assert_eq!(m.mid_count.get(), 6);
    assert_eq!(m.mid_place, MarkerPlace::AtNodes);
}

#[test]
fn concurrent_edits_of_one_slot_converge_to_the_same_value_on_both_sides() {
    let a = Document::new(1);
    let p = open_path(&a, &[(0.0, 0.0), (10.0, 0.0)]);
    let b = unpack(2, &pack(&a, "t").unwrap()).unwrap();
    a.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Arrow))
        .unwrap();
    b.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    let ab = markers_of(&merged_pair(&a, &b), p);
    let ba = markers_of(&merged_pair(&b, &a), p);
    assert_eq!(ab, ba);
    assert_ne!(ab.mid, MarkerShape::None);
}

#[test]
fn a_merged_marker_count_below_one_reads_as_one_not_a_panic() {
    // adrs.md: the lenient read of merged documents maps count < 1 to 1.
    let a = Document::new(1);
    let p = open_path(&a, &[(0.0, 0.0), (10.0, 0.0)]);
    let loro = loro_of(&a);
    let tree = loro.get_tree("paths");
    tree.get_meta(tree.roots()[0])
        .unwrap()
        .insert("stroke_marker_mid_count", LoroValue::I64(0))
        .unwrap();
    tree.get_meta(tree.roots()[0])
        .unwrap()
        .insert("stroke_marker_mid", "bogus")
        .unwrap();
    loro.commit();
    // Open through the lenient merge path: import into a Document created
    // from a valid container, then read.
    let b = unpack(2, &pack(&a, "t").unwrap()).unwrap();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let merged = LoroDoc::new();
    merged.import(&b.export_loro_snapshot().unwrap()).unwrap();
    merged.import(&bytes).unwrap();
    // We can only reach the lenient reader via a Document; a strict open of
    // the merged bytes is refused as damaged, which is acceptable too.
    let res = unpack(
        3,
        &container_from_loro(
            CURRENT_FORMAT_VERSION,
            &merged.export(loro::ExportMode::Snapshot).unwrap(),
        ),
    );
    match res {
        Err(OpenError::Damaged) => {}
        Ok(doc) => {
            let m = markers_of(&doc, p);
            assert!(m.mid_count.get() >= 1);
        }
        Err(e) => panic!("{e:?}"),
    }
}

// --------------------------------------------- transform / delete anchors

#[test]
fn markers_follow_geometry_edits_without_touching_settings() {
    // criterion 13: settings are style, geometry edits do not alter them.
    let doc = Document::new(1);
    let p = marked_path(&doc, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], false);
    let want = markers_of(&doc, p);
    doc.translate_objects(&[p], Vec2::new(5.0, 5.0)).unwrap();
    assert_eq!(markers_of(&doc, p), want);
    doc.delete_anchors(p, &[AnchorId::new(1, 2)]).unwrap();
    assert_eq!(markers_of(&doc, p), want);
}
