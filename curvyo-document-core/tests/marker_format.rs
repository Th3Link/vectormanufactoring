//! Storage and commands of the stroke markers
//! (`specs/0018-stroke-markers` criteria 3, 6, 10, 21 to 23, 25 to 29,
//! `adrs.md` decisions 1 and 4): five registers, strict open validation, the
//! `NotAPath` refusal, structure-preserving commands, and the golden
//! `markers_v9.curvyo`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Read};
use std::path::PathBuf;

use curvyo_document_core::{
    AnchorId, AnchorKind, CopySource, Document, Length, MarkerCount, MarkerPlace, MarkerShape,
    Markers, NewAnchor, NodeId, ObjectSnapshot, OpenError, Point, RectBounds, StyleEdit,
    StyleEditError, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroMap};
use zip::ZipArchive;

const GOLDEN: &str = "markers_v9.curvyo";

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn open_path(document: &Document, first: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, first), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, first + 1), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, first + 2), pt(10.0, 10.0)),
        ],
        false,
    )
}

fn closed_path(document: &Document, first: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, first), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, first + 1), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, first + 2), pt(10.0, 10.0)),
        ],
        true,
    )
}

fn rect(document: &Document) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(30.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn markers_of(document: &Document, id: NodeId) -> Markers {
    match document.object(id).unwrap() {
        ObjectSnapshot::Path(p) => p.style.stroke.markers,
        ObjectSnapshot::Primitive(p) => p.style.stroke.markers,
    }
}

fn set_all(document: &Document, id: NodeId) {
    for edit in [
        StyleEdit::MarkerStart(MarkerShape::Arrow),
        StyleEdit::MarkerMid(MarkerShape::Dot),
        StyleEdit::MarkerEnd(MarkerShape::Arrow),
        StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        StyleEdit::MarkerCount(MarkerCount::new(7).unwrap()),
    ] {
        document.edit_style(&[id], &edit).unwrap();
    }
}

fn reopen(document: &Document) -> Document {
    unpack(2, &pack(document, "0.1.0").unwrap()).unwrap()
}

fn op_count(document: &Document) -> i32 {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro.oplog_vv().values().copied().sum()
}

// ---- defaults, edits, persistence (3, 6, 28) --------------------------------

#[test]
fn a_new_path_has_no_markers_with_spaced_and_one_stored() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    assert_eq!(markers_of(&document, id), Markers::default());
    let markers = Markers::default();
    assert_eq!(markers.mid_place, MarkerPlace::Spaced);
    assert_eq!(markers.mid_count.get(), 1);
}

#[test]
fn each_slot_is_its_own_edit_and_survives_save_and_reopen() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    document
        .edit_style(&[id], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    assert_eq!(
        markers_of(&document, id).end,
        MarkerShape::None,
        "criterion 6"
    );
    set_all(&document, id);
    let want = markers_of(&document, id);
    assert_eq!(want.mid_count.get(), 7);
    assert_eq!(markers_of(&reopen(&document), id), want);
}

#[test]
fn place_and_count_stay_stored_when_the_middle_slot_goes_back_to_none() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    set_all(&document, id);
    document
        .edit_style(&[id], &StyleEdit::MarkerMid(MarkerShape::None))
        .unwrap();
    let markers = markers_of(&reopen(&document), id);
    assert_eq!(markers.mid, MarkerShape::None);
    assert_eq!(markers.mid_place, MarkerPlace::AtNodes);
    assert_eq!(markers.mid_count.get(), 7);
}

#[test]
fn an_unchanged_marker_edit_writes_nothing() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    set_all(&document, id);
    let ops = op_count(&document);
    document
        .edit_style(&[id], &StyleEdit::MarkerStart(MarkerShape::Arrow))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::MarkerCount(MarkerCount::new(7).unwrap()))
        .unwrap();
    assert_eq!(op_count(&document), ops);
}

#[test]
fn a_marker_count_is_a_whole_number_of_at_least_one() {
    assert!(MarkerCount::new(0).is_err());
    assert_eq!(MarkerCount::new(1).unwrap().get(), 1);
    assert_eq!(MarkerCount::new(501).unwrap().get(), 501);
}

#[test]
fn a_closed_path_keeps_its_start_and_end_settings() {
    let document = Document::new(1);
    let id = closed_path(&document, 1);
    set_all(&document, id);
    let markers = markers_of(&reopen(&document), id);
    assert_eq!(markers.start, MarkerShape::Arrow);
    assert_eq!(markers.end, MarkerShape::Arrow);
}

#[test]
fn document_json_has_the_markers_inside_the_stroke_style() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    set_all(&document, id);
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    let markers = &json["objects"][0]["style"]["stroke"]["markers"];
    assert_eq!(markers["start"], "arrow");
    assert_eq!(markers["mid"], "dot");
    assert_eq!(markers["mid_place"], "nodes");
    assert_eq!(markers["mid_count"], 7);
}

// ---- which objects (21 to 23) ------------------------------------------------

#[test]
fn a_marker_edit_that_names_a_primitive_is_refused_as_a_whole() {
    let document = Document::new(1);
    let path = open_path(&document, 1);
    let shape = rect(&document);
    let ops = op_count(&document);
    for edit in [
        StyleEdit::MarkerStart(MarkerShape::Arrow),
        StyleEdit::MarkerMid(MarkerShape::Dot),
        StyleEdit::MarkerEnd(MarkerShape::Arrow),
        StyleEdit::MarkerPlace(MarkerPlace::AtNodes),
        StyleEdit::MarkerCount(MarkerCount::new(3).unwrap()),
    ] {
        assert_eq!(
            document.edit_style(&[path, shape], &edit),
            Err(StyleEditError::NotAPath)
        );
        assert_eq!(
            document.edit_style(&[shape], &edit),
            Err(StyleEditError::NotAPath)
        );
    }
    assert_eq!(op_count(&document), ops, "nothing written");
    assert_eq!(markers_of(&document, path), Markers::default());
    // Other edits of the same batch are unaffected.
    document
        .edit_style(
            &[path, shape],
            &StyleEdit::StrokeCap(curvyo_document_core::LineCap::Round),
        )
        .unwrap();
}

#[test]
fn object_to_path_gives_the_new_path_no_markers() {
    let document = Document::new(1);
    let shape = rect(&document);
    let anchors: Vec<NewAnchor> = (0..4)
        .map(|i| NewAnchor {
            kind: AnchorKind::Corner,
            ..NewAnchor::corner(
                AnchorId::new(4, i + 1),
                pt(f64::from(u32::try_from(i).unwrap()), 0.0),
            )
        })
        .collect();
    document.convert_to_paths(&[(shape, anchors)]).unwrap();
    assert!(document.path(shape).is_some());
    assert_eq!(markers_of(&document, shape), Markers::default());
}

// ---- copy, split, join (25 to 27) -------------------------------------------

#[test]
fn a_copy_has_the_same_markers_and_is_independent() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    set_all(&document, id);
    let copies = document
        .duplicate_objects(
            &[CopySource {
                id,
                anchor_ids: vec![
                    AnchorId::new(5, 1),
                    AnchorId::new(5, 2),
                    AnchorId::new(5, 3),
                ],
            }],
            Vec2::new(5.0, 5.0),
        )
        .unwrap();
    let copy = copies[0];
    assert_eq!(markers_of(&document, copy), markers_of(&document, id));
    document
        .edit_style(&[copy], &StyleEdit::MarkerStart(MarkerShape::None))
        .unwrap();
    assert_eq!(markers_of(&document, id).start, MarkerShape::Arrow);
}

#[test]
fn splitting_an_open_path_keeps_the_markers_on_both_halves() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    set_all(&document, id);
    let want = markers_of(&document, id);
    let ((first, _), (second, _)) = document
        .split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(9, 1))
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(markers_of(&document, first), want);
    assert_eq!(markers_of(&document, second), want);
    document
        .edit_style(&[second], &StyleEdit::MarkerEnd(MarkerShape::None))
        .unwrap();
    assert_eq!(markers_of(&document, first), want);
    assert_eq!(
        markers_of(&reopen(&document), second).end,
        MarkerShape::None
    );
}

#[test]
fn splitting_a_closed_path_keeps_the_one_object_and_its_markers() {
    let document = Document::new(1);
    let id = closed_path(&document, 1);
    set_all(&document, id);
    let want = markers_of(&document, id);
    if let Ok(((same, _), _)) =
        document.split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(9, 1))
    {
        assert_eq!(same, id);
    }
    assert_eq!(markers_of(&document, id), want);
}

#[test]
fn a_join_keeps_the_survivors_markers_and_closing_keeps_them_too() {
    let document = Document::new(1);
    let a = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(5.0, 0.0)),
        ],
        false,
    );
    let b = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 3), pt(5.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(9.0, 0.0)),
        ],
        false,
    );
    set_all(&document, a);
    document
        .edit_style(&[b], &StyleEdit::MarkerStart(MarkerShape::Dot))
        .unwrap();
    let want = markers_of(&document, a);
    let (survivor, _) = document
        .join_endpoints(a, AnchorId::new(1, 2), b, AnchorId::new(1, 3))
        .unwrap();
    assert_eq!(survivor, a);
    assert_eq!(markers_of(&document, a), want);
    assert!(document.object(b).is_none());

    let ring = open_path(&document, 20);
    set_all(&document, ring);
    let want = markers_of(&document, ring);
    document
        .join_endpoints(ring, AnchorId::new(1, 20), ring, AnchorId::new(1, 22))
        .unwrap();
    assert!(document.path(ring).unwrap().closed);
    assert_eq!(markers_of(&document, ring), want);
}

// ---- open-file validation and older files (29) --------------------------------

fn container_from_loro(loro_bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let manifest = serde_json::json!({
        "format_version": curvyo_document_core::CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": curvyo_document_core::CURRENT_LORO_SNAPSHOT_VERSION,
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

fn open_with(edit: impl FnOnce(&LoroMap)) -> Result<Document, OpenError> {
    let document = Document::new(1);
    let _ = open_path(&document, 1);
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let tree = loro.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    edit(&meta);
    loro.commit();
    unpack(
        2,
        &container_from_loro(&loro.export(loro::ExportMode::Snapshot).unwrap()),
    )
}

#[test]
fn an_unknown_shape_or_place_or_a_bad_count_is_refused_as_damaged() {
    use loro::LoroValue;
    let cases: Vec<(&str, LoroValue)> = vec![
        ("stroke_marker_start", LoroValue::from("diamond")),
        ("stroke_marker_mid", LoroValue::from(3_i64)),
        ("stroke_marker_end", LoroValue::from("Arrow")),
        ("stroke_marker_mid_place", LoroValue::from("everywhere")),
        ("stroke_marker_mid_count", LoroValue::from(0_i64)),
        ("stroke_marker_mid_count", LoroValue::from(-3_i64)),
        ("stroke_marker_mid_count", LoroValue::from(2.5_f64)),
        ("stroke_marker_mid_count", LoroValue::from("many")),
    ];
    for (key, value) in cases {
        let result = open_with(|meta| meta.insert(key, value.clone()).unwrap());
        assert!(
            matches!(result, Err(OpenError::Damaged)),
            "{key} = {value:?}"
        );
    }
}

#[test]
fn a_count_above_five_hundred_opens_and_is_kept_as_stored() {
    let document = open_with(|meta| {
        meta.insert("stroke_marker_mid", "dot").unwrap();
        meta.insert("stroke_marker_mid_count", 501_i64).unwrap();
    })
    .unwrap();
    let id = document.object_ids()[0];
    assert_eq!(markers_of(&document, id).mid_count.get(), 501);
    assert_eq!(markers_of(&reopen(&document), id).mid_count.get(), 501);
}

#[test]
fn every_older_fixture_opens_with_every_slot_none() {
    for name in [
        "legacy_gradient_v7.curvyo",
        "dash_v9.curvyo",
        "compound_v8.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "corner_radii_per_corner.curvyo",
    ] {
        let bytes = std::fs::read(fixture_path(name)).unwrap();
        let document = unpack(2, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        for id in document.object_ids() {
            assert_eq!(markers_of(&document, id), Markers::default(), "{name}");
        }
    }
}

/// A path with every key set, a count of 501 and a closed path.
fn golden_document() -> Document {
    let document = Document::new(1);
    let open = open_path(&document, 1);
    set_all(&document, open);
    let ring = closed_path(&document, 10);
    set_all(&document, ring);
    document
        .edit_style(
            &[ring],
            &StyleEdit::MarkerCount(MarkerCount::new(501).unwrap()),
        )
        .unwrap();
    let _ = rect(&document);
    document
}

#[test]
fn the_golden_reads_back_and_opening_it_writes_nothing() {
    if std::env::var_os("CURVYO_WRITE_FIXTURES").is_some() {
        std::fs::write(
            fixture_path(GOLDEN),
            pack(&golden_document(), "0.1.0").unwrap(),
        )
        .unwrap();
    }
    let bytes = std::fs::read(fixture_path(GOLDEN)).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut manifest = Vec::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut manifest)
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(manifest["format_version"], 9);

    let document = unpack(2, &bytes).unwrap();
    let ids = document.object_ids();
    let open = markers_of(&document, ids[0]);
    assert_eq!(open.start, MarkerShape::Arrow);
    assert_eq!(open.mid, MarkerShape::Dot);
    assert_eq!(open.mid_place, MarkerPlace::AtNodes);
    assert_eq!(open.mid_count.get(), 7);
    assert_eq!(markers_of(&document, ids[1]).mid_count.get(), 501);
    assert_eq!(
        markers_of(&document, ids[2]),
        Markers::default(),
        "the rectangle"
    );

    let stored = LoroDoc::new();
    let mut loro_bytes = Vec::new();
    archive
        .by_name("document.loro")
        .unwrap()
        .read_to_end(&mut loro_bytes)
        .unwrap();
    stored.import(&loro_bytes).unwrap();
    let reopened = LoroDoc::new();
    reopened
        .import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    assert_eq!(reopened.oplog_vv(), stored.oplog_vv());
}
