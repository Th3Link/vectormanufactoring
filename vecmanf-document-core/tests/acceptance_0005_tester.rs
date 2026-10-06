//! Tester's independent document-model tests for
//! `specs/0005-object-transform/specification.md` (criteria 19, 20, 21, 24)
//! and `adrs.md`'s format decisions: the golden fixture, older files (an
//! absent `rotation` reads as 0), damaged `rotation` values, Split copying
//! `rotation`, Join keeping the survivor's, and object to path.
//!
//! Kept apart from the implementer's `acceptance_0005.rs` so the two sets
//! stay independent.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use std::f64::consts::PI;
use std::io::{Cursor, Read, Write};

use loro::LoroDoc;
use vecmanf_document_core::{
    AnchorId, Angle, CURRENT_FORMAT_VERSION, Document, Length, NewAnchor, NodeId, ObjectSnapshot,
    OpenError, Point, RectBounds, Shape, Vec2, pack, unpack,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("reading fixture {}: {err}", path.display()))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(document: &Document) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    })
}

/// A three-anchor open path (0,0) (10,0) (10,10), distinct anchor ids.
fn open_path(document: &Document, peer: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(peer, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(peer, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(peer, 3), pt(10.0, 10.0)),
        ],
        false,
    )
}

fn zip_entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut out = Vec::new();
    file.read_to_end(&mut out).unwrap();
    out
}

/// Re-writes `bytes`'s Loro snapshot with `mutate` applied to the meta map
/// of the first tree node, and wraps it into a fresh container whose
/// manifest says `format_version` = `CURRENT_FORMAT_VERSION`.
fn craft(document: &Document, mutate: impl Fn(&loro::LoroMap)) -> Vec<u8> {
    let good = document.export_loro_snapshot().unwrap();
    let loro = LoroDoc::new();
    loro.import(&good).unwrap();
    let tree = loro.get_tree("paths");
    let nodes = tree.nodes();
    assert_ne!(nodes, [] as [loro::TreeID; 0]);
    let meta = tree.get_meta(nodes[0]).unwrap();
    mutate(&meta);
    loro.commit();
    let loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();

    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": 1,
        "app_version": "tester-crafted-fixture",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(&loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    writer.finish().unwrap().into_inner()
}

// ---------------------------------------------------------------------
// format_version and the golden fixture
// ---------------------------------------------------------------------

#[test]
fn format_version_is_bumped_past_main_to_5() {
    // main's CURRENT_FORMAT_VERSION is 4 (path-merge-split-and-node-types).
    assert_eq!(CURRENT_FORMAT_VERSION, 5);
    let document = Document::new(1);
    let _ = rect(&document);
    let bytes = pack(&document, "0.1.0").unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&zip_entry(&bytes, "manifest.json")).unwrap();
    assert_eq!(manifest["format_version"], 5);
}

#[test]
fn golden_rotation_v5_fixture_declares_version_5_and_opens_with_both_rotations() {
    let bytes = fixture("rotation_v5.vmf");
    let manifest: serde_json::Value =
        serde_json::from_slice(&zip_entry(&bytes, "manifest.json")).unwrap();
    assert_eq!(manifest["format_version"], 5, "fixture is a version-5 file");
    let document = unpack(3, &bytes).expect("golden fixture opens");
    let ids = document.object_ids();
    assert_eq!(ids.len(), 2);
    let mut seen_rect = false;
    let mut seen_path = false;
    for id in ids {
        match document.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => {
                assert!(matches!(p.shape, Shape::Rect { .. }));
                assert!((p.rotation.as_radians() - 0.5).abs() < 1e-12);
                seen_rect = true;
            }
            ObjectSnapshot::Path(p) => {
                assert!((p.rotation.as_radians() + 0.75).abs() < 1e-12);
                seen_path = true;
            }
        }
    }
    assert!(seen_rect && seen_path);
}

#[test]
#[ignore = "FINDING (low): adrs.md says document.json omits `rotation` when 0; a fresh rect writes \"rotation\":0.0"]
fn golden_fixture_document_json_lists_rotation_only_where_non_zero() {
    // adrs.md: `document.json` adds `rotation` (radians) to each entry in
    // `objects`, omitted when 0.
    let bytes = fixture("rotation_v5.vmf");
    let json: serde_json::Value =
        serde_json::from_slice(&zip_entry(&bytes, "document.json")).unwrap();
    let objects = json["objects"].as_array().expect("objects array");
    assert_eq!(objects.len(), 2);
    for object in objects {
        assert!(
            object.get("rotation").is_some(),
            "both fixture objects are rotated: {object}"
        );
    }
    // A fresh unrotated document omits the key.
    let document = Document::new(1);
    let _ = rect(&document);
    let packed = pack(&document, "0.1.0").unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&zip_entry(&packed, "document.json")).unwrap();
    for object in json["objects"].as_array().unwrap() {
        assert!(
            object.get("rotation").is_none(),
            "rotation omitted when 0: {object}"
        );
    }
}

#[test]
fn golden_fixture_round_trips_exactly_through_save_and_open() {
    let document = unpack(3, &fixture("rotation_v5.vmf")).unwrap();
    let again = unpack(4, &pack(&document, "0.1.0").unwrap()).unwrap();
    for id in document.object_ids() {
        assert_eq!(again.object(id), document.object(id));
    }
}

#[test]
fn every_older_fixture_still_opens_and_reads_rotation_as_zero() {
    for name in [
        "format_version_1.vmf",
        "paths_v2.vmf",
        "primitives_v3.vmf",
        "valid.vmf",
    ] {
        let document =
            unpack(2, &fixture(name)).unwrap_or_else(|e| panic!("{name} must still open: {e:?}"));
        for id in document.object_ids() {
            assert_eq!(
                document.object(id).unwrap().rotation().as_radians(),
                0.0,
                "{name}: absent rotation reads as 0"
            );
        }
    }
}

#[test]
fn a_file_one_version_newer_than_current_is_refused_as_too_new() {
    let result = unpack(2, &fixture("future_format_version.vmf"));
    assert!(matches!(result, Err(OpenError::FormatTooNew { .. })));
}

// ---------------------------------------------------------------------
// Damaged rotation values (open-file validation)
// ---------------------------------------------------------------------

#[test]
fn non_finite_or_mistyped_rotation_is_refused_as_damaged_for_primitives() {
    type Mutation = fn(&loro::LoroMap);
    let mutations: [(&str, Mutation); 6] = [
        ("NaN", |m| m.insert("rotation", f64::NAN).unwrap()),
        ("+inf", |m| m.insert("rotation", f64::INFINITY).unwrap()),
        ("-inf", |m| m.insert("rotation", f64::NEG_INFINITY).unwrap()),
        ("string", |m| m.insert("rotation", "1.5").unwrap()),
        ("bool", |m| m.insert("rotation", true).unwrap()),
        ("null", |m| {
            m.insert("rotation", loro::LoroValue::Null).unwrap();
        }),
    ];
    for (label, mutate) in mutations {
        let document = Document::new(1);
        let _ = rect(&document);
        let bytes = craft(&document, mutate);
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "primitive with rotation = {label} must be Damaged"
        );
    }
}

#[test]
fn non_finite_or_mistyped_rotation_is_refused_as_damaged_for_paths() {
    type Mutation = fn(&loro::LoroMap);
    let mutations: [(&str, Mutation); 4] = [
        ("NaN", |m| m.insert("rotation", f64::NAN).unwrap()),
        ("+inf", |m| m.insert("rotation", f64::INFINITY).unwrap()),
        ("string", |m| m.insert("rotation", "x").unwrap()),
        ("bool", |m| m.insert("rotation", false).unwrap()),
    ];
    for (label, mutate) in mutations {
        let document = Document::new(1);
        let _ = open_path(&document, 1);
        let bytes = craft(&document, mutate);
        assert!(
            matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
            "path with rotation = {label} must be Damaged"
        );
    }
}

#[test]
#[ignore = "FINDING (low): adrs.md says any finite rotation is normalized on read; 1e6 reads back as 1e6"]
fn a_huge_but_finite_rotation_is_accepted_and_normalized_on_read() {
    for raw in [1.0e6_f64, -1.0e6, 7.0 * PI, 1.0e300] {
        let document = Document::new(1);
        let id = rect(&document);
        let bytes = craft(&document, |m| m.insert("rotation", raw).unwrap());
        let reopened = unpack(2, &bytes).unwrap_or_else(|e| panic!("{raw}: {e:?}"));
        let r = reopened.object(id).unwrap().rotation().as_radians();
        assert!(r.is_finite(), "{raw}: {r}");
        assert!(
            r > -PI - 1e-9 && r <= PI + 1e-9,
            "{raw} normalizes into (-pi, pi]: {r}"
        );
    }
}

#[test]
fn a_missing_rotation_key_is_zero_not_an_error() {
    let document = Document::new(1);
    let id = rect(&document);
    let reopened = unpack(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(reopened.object(id).unwrap().rotation().as_radians(), 0.0);
}

// ---------------------------------------------------------------------
// rotate_object command
// ---------------------------------------------------------------------

#[test]
fn rotate_object_refuses_an_unknown_id_and_writes_nothing() {
    let document = Document::new(1);
    let id = rect(&document);
    let before = document.export_loro_snapshot().unwrap();
    document.delete_objects(&[id]).unwrap();
    let after_delete = document.export_loro_snapshot().unwrap();
    let result = document.rotate_object(id, pt(0.0, 0.0), Angle::from_radians(1.0));
    assert!(result.is_err());
    assert_eq!(
        document.export_loro_snapshot().unwrap(),
        after_delete,
        "failed command must not commit"
    );
    let _ = before;
}

#[test]
fn rotating_a_primitive_about_the_center_writes_only_rotation_not_the_frame() {
    let document = Document::new(1);
    let id = rect(&document);
    let before = document.primitive(id).unwrap();
    document
        .rotate_object(id, pt(5.0, 2.0), Angle::from_radians(0.7))
        .unwrap();
    let after = document.primitive(id).unwrap();
    assert_eq!(
        after.shape, before.shape,
        "frame untouched by a centre rotate"
    );
    assert!((after.rotation.as_radians() - 0.7).abs() < 1e-12);
}

#[test]
fn rotating_a_primitive_about_an_outside_pivot_moves_its_frame_centre_and_adds_rotation() {
    let document = Document::new(1);
    let id = rect(&document);
    // centre (5,2) about (5,6) by +90 deg clockwise (y down): vec (0,-4) -> (4,0): (9,6).
    document
        .rotate_object(id, pt(5.0, 6.0), Angle::from_radians(PI / 2.0))
        .unwrap();
    let p = document.primitive(id).unwrap();
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    let centre = pt(
        bounds.origin.x + bounds.width.as_mm() / 2.0,
        bounds.origin.y + bounds.height.as_mm() / 2.0,
    );
    assert!(
        (centre.x - 9.0).abs() < 1e-9 && (centre.y - 6.0).abs() < 1e-9,
        "{centre:?}"
    );
    assert!((p.rotation.as_radians() - PI / 2.0).abs() < 1e-12);
    assert!(
        (bounds.width.as_mm() - 10.0).abs() < 1e-12 && (bounds.height.as_mm() - 4.0).abs() < 1e-12
    );
}

#[test]
fn rotating_a_path_bakes_anchor_points_and_relative_handles() {
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(3.0, 0.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(0.0, -2.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0))
            },
        ],
        false,
    );
    document
        .rotate_object(id, pt(0.0, 0.0), Angle::from_radians(PI / 2.0))
        .unwrap();
    let p = document.path(id).unwrap();
    // (10,0) -> (0,10); handle_out (3,0) -> (0,3); handle_in (0,-2) -> (2,0).
    assert!(p.anchors[1].point.x.abs() < 1e-9 && (p.anchors[1].point.y - 10.0).abs() < 1e-9);
    assert!(
        p.anchors[0].handle_out.x.abs() < 1e-9 && (p.anchors[0].handle_out.y - 3.0).abs() < 1e-9
    );
    assert!((p.anchors[1].handle_in.x - 2.0).abs() < 1e-9 && p.anchors[1].handle_in.y.abs() < 1e-9);
    assert!((p.rotation.as_radians() - PI / 2.0).abs() < 1e-12);
}

#[test]
fn rotation_accumulates_and_wraps_into_the_half_open_interval() {
    let document = Document::new(1);
    let id = rect(&document);
    // 0.8 * 6 = 4.8 rad -> 4.8 - 2 pi
    for _ in 0..6 {
        document
            .rotate_object(id, pt(5.0, 2.0), Angle::from_radians(0.8))
            .unwrap();
    }
    let r = document.primitive(id).unwrap().rotation.as_radians();
    assert!((r - (4.8 - 2.0 * PI)).abs() < 1e-9, "{r}");
    // Exactly pi stays pi, and -pi never appears.
    let id2 = rect(&document);
    document
        .rotate_object(id2, pt(5.0, 2.0), Angle::from_radians(PI))
        .unwrap();
    let r = document.primitive(id2).unwrap().rotation.as_radians();
    assert!(
        r > 0.0 && (r - PI).abs() < 1e-9,
        "pi must be written as +pi: {r}"
    );
    document
        .rotate_object(id2, pt(5.0, 2.0), Angle::from_radians(-2.0 * PI))
        .unwrap();
    let r = document.primitive(id2).unwrap().rotation.as_radians();
    assert!(
        r > 0.0 && (r - PI).abs() < 1e-9,
        "pi - 2pi = -pi must normalize to +pi: {r}"
    );
}

// ---------------------------------------------------------------------
// Split copies rotation (the architect's requirement); Join keeps the survivor's
// ---------------------------------------------------------------------

#[test]
fn split_of_a_rotated_open_path_copies_rotation_to_the_new_path() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    document
        .rotate_object(id, pt(5.0, 5.0), Angle::from_radians(0.9))
        .unwrap();
    let rotation = document.path(id).unwrap().rotation.as_radians();
    let before_ids = document.object_ids();
    let mid = document.path(id).unwrap().anchors[1].id;
    document
        .split_at_anchor(id, mid, AnchorId::new(1, 99))
        .expect("split");
    let after_ids = document.object_ids();
    assert_eq!(after_ids.len(), before_ids.len() + 1, "one new object");
    for object in after_ids {
        let r = document.object(object).unwrap().rotation().as_radians();
        assert!(
            (r - rotation).abs() < 1e-12,
            "both halves carry the original rotation, got {r} vs {rotation}"
        );
    }
}

#[test]
fn split_of_a_rotated_closed_path_keeps_its_rotation() {
    let document = Document::new(1);
    let id = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(10.0, 10.0)),
        ],
        true,
    );
    document
        .rotate_object(id, pt(5.0, 5.0), Angle::from_radians(-0.4))
        .unwrap();
    let anchor = document.path(id).unwrap().anchors[0].id;
    document
        .split_at_anchor(id, anchor, AnchorId::new(1, 99))
        .expect("split closed path");
    for object in document.object_ids() {
        let r = document.object(object).unwrap().rotation().as_radians();
        assert!((r + 0.4).abs() < 1e-12, "{r}");
    }
}

#[test]
fn split_rotation_survives_save_and_reopen() {
    let document = Document::new(1);
    let id = open_path(&document, 1);
    document
        .rotate_object(id, pt(5.0, 5.0), Angle::from_radians(1.1))
        .unwrap();
    let mid = document.path(id).unwrap().anchors[1].id;
    document
        .split_at_anchor(id, mid, AnchorId::new(1, 99))
        .unwrap();
    let reopened = unpack(7, &pack(&document, "0.1.0").unwrap()).unwrap();
    for object in reopened.object_ids() {
        assert!((reopened.object(object).unwrap().rotation().as_radians() - 1.1).abs() < 1e-12);
    }
}

#[test]
fn join_of_two_rotated_paths_keeps_the_survivors_own_rotation() {
    let document = Document::new(1);
    let a = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
        ],
        false,
    );
    let b = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 3), pt(20.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(30.0, 0.0)),
        ],
        false,
    );
    document
        .rotate_object(a, pt(5.0, 0.0), Angle::from_radians(0.3))
        .unwrap();
    document
        .rotate_object(b, pt(25.0, 0.0), Angle::from_radians(0.7))
        .unwrap();
    let a_rotation = document.path(a).unwrap().rotation.as_radians();
    let b_rotation = document.path(b).unwrap().rotation.as_radians();
    let a_end = document.path(a).unwrap().anchors[1].id;
    let b_start = document.path(b).unwrap().anchors[0].id;
    let (survivor, _) = document
        .join_endpoints(a, a_end, b, b_start)
        .expect("join two open paths");
    let after = document.path(survivor).expect("survivor is a path");
    let expected = if survivor == a {
        a_rotation
    } else {
        b_rotation
    };
    assert!(
        (after.rotation.as_radians() - expected).abs() < 1e-12,
        "survivor {survivor:?} rotation {} expected its own {expected}",
        after.rotation.as_radians()
    );
    assert_eq!(document.object_ids().len(), 1, "the other path is deleted");
}
