//! Golden-file and file-format tests for `rectangle-corner-radii`
//! (`specs/rectangle-corner-radii/adrs.md`, decisions 2, 3, 10 and 11;
//! `CLAUDE.md` §5): the version-6 container with per-corner radii, a
//! genuine-shape version-5 container with the legacy single `corner_radius`,
//! the outline anchors for mixed radii, the refusal cases, and the rule that
//! opening writes nothing.
//!
//! Fixtures are written by the `#[ignore]`d generator below
//! (`cargo test -p curvyo-document-core --test corner_radii_fixtures
//! generate_corner_radii_fixtures -- --ignored`); the committed bytes are what
//! the tests read.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, CornerRadii, Document, Length, NodeId,
    ObjectSnapshot, OpenError, Point, RectBounds, Shape, effective_corner_radii,
    outline_of_rotated, pack, rect_outline, unpack,
};
use loro::{LoroDoc, LoroMap};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const V6_FIXTURE: &str = "corner_radii_v6.curvyo";
const LEGACY_FIXTURE: &str = "legacy_corner_radius_v5.curvyo";
const OUTLINE_FIXTURE: &str = "rect_outline_mixed_radii.json";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("reading fixture {}: {err}", path.display()))
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

fn manifest_format_version(container: &[u8]) -> u64 {
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(container, "manifest.json")).expect("manifest json");
    manifest["format_version"].as_u64().expect("format_version")
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

fn mm(value: f64) -> Length {
    Length::from_mm(value)
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: mm(tl),
        tr: mm(tr),
        br: mm(br),
        bl: mm(bl),
    }
}

fn bounds(x: f64, y: f64, width: f64, height: f64) -> RectBounds {
    RectBounds {
        origin: Point::new(x, y),
        width: mm(width),
        height: mm(height),
    }
}

fn rect_of(document: &Document, id: NodeId) -> (RectBounds, CornerRadii) {
    let ObjectSnapshot::Primitive(primitive) = document.object(id).expect("object") else {
        panic!("a primitive");
    };
    let Shape::Rect {
        bounds,
        corner_radii,
    } = primitive.shape
    else {
        panic!("a rectangle");
    };
    (bounds, corner_radii)
}

/// The root nodes of a raw Loro snapshot, in z-order, with their meta maps.
fn raw_nodes(loro: &LoroDoc) -> Vec<LoroMap> {
    let tree = loro.get_tree("paths");
    tree.roots()
        .into_iter()
        .map(|id| tree.get_meta(id).unwrap())
        .collect()
}

fn raw_rect(meta: &LoroMap, bounds: [f64; 4]) {
    meta.insert("shape", "rect").unwrap();
    meta.insert("rect_bounds", bounds.to_vec()).unwrap();
}

/// A one-node container whose rectangle meta map `build` fills in by hand.
fn raw_rect_container(build: impl FnOnce(&LoroMap)) -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    let tree = loro.get_tree("paths");
    let node = tree.create(loro::TreeParentId::Root).unwrap();
    let meta = tree.get_meta(node).unwrap();
    raw_rect(&meta, [0.0, 0.0, 40.0, 20.0]);
    build(&meta);
    loro.commit();
    container(
        CURRENT_FORMAT_VERSION,
        &loro.export(loro::ExportMode::Snapshot).unwrap(),
        b"{}",
    )
}

// ---------------------------------------------------------------------
// The fixture contents, shared by the generator and the tests
// ---------------------------------------------------------------------

/// Version 6: one object per notable state, in z-order.
fn build_v6_document() -> Vec<u8> {
    let document = Document::new(1);
    // 0: four different radii, TR sharp.
    let first = document.create_rect(bounds(0.0, 0.0, 100.0, 40.0));
    document
        .set_corner_radii(&[(first, radii(12.0, 0.0, 8.0, 3.5))])
        .unwrap();
    // 1: a stored sum above the side (shrunk on evaluation, never rewritten).
    let over = document.create_rect(bounds(150.0, 0.0, 60.0, 40.0));
    document
        .set_corner_radii(&[(over, radii(50.0, 50.0, 10.0, 0.0))])
        .unwrap();
    // 2: rotated, TL sharp.
    let rotated = document.create_rect(bounds(0.0, 80.0, 50.0, 30.0));
    document
        .set_corner_radii(&[(rotated, radii(0.0, 6.0, 9.0, 2.0))])
        .unwrap();
    let ObjectSnapshot::Primitive(_) = document.object(rotated).unwrap() else {
        panic!("a primitive");
    };
    document
        .rotate_object(&document.object(rotated).unwrap().rotated(
            Point::new(25.0, 95.0),
            curvyo_document_core::Angle::from_radians(0.5),
        ))
        .unwrap();
    // 3: some own keys plus the legacy key (a version-5 file after one corner
    // edit): written by hand below.
    let mixed = document.create_rect(bounds(100.0, 80.0, 40.0, 40.0));
    document
        .set_corner_radii(&[(mixed, radii(4.0, 4.0, 7.5, 4.0))])
        .unwrap();

    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let nodes = raw_nodes(&loro);
    let meta = &nodes[3];
    for key in ["corner_radius_tl", "corner_radius_tr", "corner_radius_bl"] {
        meta.delete(key).unwrap();
    }
    meta.insert("corner_radius", 4.0_f64).unwrap();
    loro.commit();
    let raw = container(
        CURRENT_FORMAT_VERSION,
        &loro.export(loro::ExportMode::Snapshot).unwrap(),
        b"{}",
    );
    // Reopen and save once more so `document.json` shows what the file holds.
    pack(&unpack(1, &raw).unwrap(), "0.1.0").unwrap()
}

/// Version 5: the single legacy `corner_radius` key, built with raw Loro
/// inserts the way the version-3 fixture was.
fn build_legacy_v5_document() -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    let tree = loro.get_tree("paths");
    for (bounds, radius) in [
        ([0.0, 0.0, 40.0, 20.0], 5.0),
        ([60.0, 0.0, 30.0, 30.0], 0.0),
    ] {
        let node = tree.create(loro::TreeParentId::Root).unwrap();
        let meta = tree.get_meta(node).unwrap();
        raw_rect(&meta, bounds);
        meta.insert("corner_radius", radius).unwrap();
        meta.insert("stroke_width", 0.25_f64).unwrap();
    }
    loro.commit();
    container(
        5,
        &loro.export(loro::ExportMode::Snapshot).unwrap(),
        b"{\"format_version\":5,\"objects\":[]}",
    )
}

struct OutlineCase {
    name: &'static str,
    bounds: RectBounds,
    radii: CornerRadii,
}

fn outline_cases() -> Vec<OutlineCase> {
    vec![
        OutlineCase {
            name: "spec_example_one_f_is_1",
            bounds: bounds(0.0, 0.0, 100.0, 40.0),
            radii: radii(30.0, 30.0, 0.0, 0.0),
        },
        OutlineCase {
            name: "spec_example_two_neighbours_shrink",
            bounds: bounds(0.0, 0.0, 100.0, 40.0),
            radii: radii(30.0, 30.0, 0.0, 30.0),
        },
        OutlineCase {
            name: "diagonal_corners_large",
            bounds: bounds(0.0, 0.0, 80.0, 80.0),
            radii: radii(40.0, 0.0, 40.0, 0.0),
        },
        OutlineCase {
            name: "single_rounded_corner",
            bounds: bounds(0.0, 0.0, 50.0, 30.0),
            radii: radii(0.0, 0.0, 0.0, 10.0),
        },
        OutlineCase {
            name: "all_sharp",
            bounds: bounds(5.0, 6.0, 20.0, 10.0),
            radii: radii(0.0, 0.0, 0.0, 0.0),
        },
        OutlineCase {
            name: "four_equal_within_limit",
            bounds: bounds(0.0, 0.0, 100.0, 40.0),
            radii: radii(5.0, 5.0, 5.0, 5.0),
        },
        OutlineCase {
            name: "four_equal_above_limit_clamped",
            bounds: bounds(0.0, 0.0, 100.0, 40.0),
            radii: radii(60.0, 60.0, 60.0, 60.0),
        },
        OutlineCase {
            name: "offset_origin_all_different",
            bounds: bounds(10.0, 20.0, 100.0, 60.0),
            radii: radii(8.0, 12.0, 4.0, 0.0),
        },
    ]
}

fn anchors_json(anchors: &[curvyo_document_core::OutlineAnchor]) -> serde_json::Value {
    anchors
        .iter()
        .map(|a| {
            serde_json::json!({
                "point": [a.point.x, a.point.y],
                "handle_in": [a.handle_in.x, a.handle_in.y],
                "handle_out": [a.handle_out.x, a.handle_out.y],
            })
        })
        .collect()
}

fn outline_fixture_json() -> Vec<u8> {
    let cases: Vec<serde_json::Value> = outline_cases()
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "bounds": [c.bounds.origin.x, c.bounds.origin.y, c.bounds.width.as_mm(), c.bounds.height.as_mm()],
                "radii": [c.radii.tl.as_mm(), c.radii.tr.as_mm(), c.radii.br.as_mm(), c.radii.bl.as_mm()],
                "anchors": anchors_json(&rect_outline(c.bounds, c.radii)),
            })
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&serde_json::json!({ "cases": cases })).unwrap();
    text.push('\n');
    text.into_bytes()
}

/// Writes the fixtures this file pins. Run deliberately, then review the
/// diff of the committed bytes.
#[test]
#[ignore = "run deliberately to regenerate the fixtures, not on every `cargo test`"]
fn generate_corner_radii_fixtures() {
    let dir = fixtures_dir();
    std::fs::write(dir.join(V6_FIXTURE), build_v6_document()).unwrap();
    std::fs::write(dir.join(LEGACY_FIXTURE), build_legacy_v5_document()).unwrap();
    std::fs::write(dir.join(OUTLINE_FIXTURE), outline_fixture_json()).unwrap();
    // A container one version newer than this build, for the "saved by a
    // newer version" refusal (`future_format_version.curvyo`).
    std::fs::write(
        dir.join("future_format_version.curvyo"),
        container(CURRENT_FORMAT_VERSION + 1, b"irrelevant", b"{}"),
    )
    .unwrap();
}

// ---------------------------------------------------------------------
// Version 6 with per-corner radii (AC 17, 19)
// ---------------------------------------------------------------------

#[test]
fn the_version_6_fixture_declares_the_current_version() {
    assert_eq!(CURRENT_FORMAT_VERSION, 6);
    assert_eq!(
        manifest_format_version(&fixture(V6_FIXTURE)),
        u64::from(CURRENT_FORMAT_VERSION)
    );
}

#[test]
fn the_version_6_fixture_opens_with_each_rectangles_exact_radii() {
    let document = unpack(2, &fixture(V6_FIXTURE)).expect("opens");
    let ids = document.object_ids();
    assert_eq!(ids.len(), 4);
    let expected = [
        (bounds(0.0, 0.0, 100.0, 40.0), radii(12.0, 0.0, 8.0, 3.5)),
        (bounds(150.0, 0.0, 60.0, 40.0), radii(50.0, 50.0, 10.0, 0.0)),
        (bounds(0.0, 80.0, 50.0, 30.0), radii(0.0, 6.0, 9.0, 2.0)),
        (bounds(100.0, 80.0, 40.0, 40.0), radii(4.0, 4.0, 7.5, 4.0)),
    ];
    for (id, (expected_bounds, expected_radii)) in ids.into_iter().zip(expected) {
        let (actual_bounds, actual_radii) = rect_of(&document, id);
        assert_eq!(actual_bounds, expected_bounds);
        assert_eq!(actual_radii, expected_radii, "stored raw, as saved");
    }
}

#[test]
fn the_rotated_rectangle_keeps_its_rotation_and_size() {
    let document = unpack(2, &fixture(V6_FIXTURE)).unwrap();
    let id = document.object_ids()[2];
    let object = document.object(id).unwrap();
    assert!((object.rotation().as_radians() - 0.5).abs() < 1e-12);
    let (_, stored) = rect_of(&document, id);
    assert_eq!(stored, radii(0.0, 6.0, 9.0, 2.0));
}

#[test]
fn a_stored_sum_above_a_side_opens_and_is_shrunk_on_evaluation_only() {
    let document = unpack(2, &fixture(V6_FIXTURE)).unwrap();
    let (rect_bounds, stored) = rect_of(&document, document.object_ids()[1]);
    // TL 50 + TR 50 on a 60 mm side: f = 0.6; TL 50 + BL 0 on 40: 0.8.
    let effective = effective_corner_radii(rect_bounds, stored);
    for (actual, expected) in [
        (effective.tl, 30.0),
        (effective.tr, 30.0),
        (effective.br, 6.0),
        (effective.bl, 0.0),
    ] {
        assert!((actual.as_mm() - expected).abs() < 1e-9);
    }
    assert_eq!(stored, radii(50.0, 50.0, 10.0, 0.0), "never written back");
}

#[test]
fn some_own_keys_plus_the_legacy_key_read_per_corner() {
    let document = unpack(2, &fixture(V6_FIXTURE)).unwrap();
    let (_, stored) = rect_of(&document, document.object_ids()[3]);
    assert_eq!(stored, radii(4.0, 4.0, 7.5, 4.0));
}

#[test]
fn save_and_reopen_gives_the_same_radii() {
    let opened = unpack(2, &fixture(V6_FIXTURE)).unwrap();
    let saved = pack(&opened, "0.1.0").unwrap();
    let reopened = unpack(3, &saved).unwrap();
    for (a, b) in opened.object_ids().into_iter().zip(reopened.object_ids()) {
        assert_eq!(rect_of(&opened, a), rect_of(&reopened, b));
        assert_eq!(
            opened.object(a).unwrap().rotation(),
            reopened.object(b).unwrap().rotation()
        );
    }
}

#[test]
fn every_file_this_build_saves_declares_the_new_version() {
    let empty = pack(&Document::new(1), "0.1.0").unwrap();
    assert_eq!(
        manifest_format_version(&empty),
        u64::from(CURRENT_FORMAT_VERSION)
    );
    let with_rect = Document::new(1);
    let _ = with_rect.create_rect(bounds(0.0, 0.0, 10.0, 10.0));
    let saved = pack(&with_rect, "0.1.0").unwrap();
    assert_eq!(
        manifest_format_version(&saved),
        u64::from(CURRENT_FORMAT_VERSION)
    );
}

#[test]
fn a_new_rectangle_stores_four_radius_keys_and_never_the_legacy_one() {
    let document = Document::new(1);
    let id = document.create_rect(bounds(0.0, 0.0, 10.0, 10.0));
    document.set_corner_radius(&[id], mm(2.0)).unwrap();
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let meta = &raw_nodes(&loro)[0];
    for key in [
        "corner_radius_tl",
        "corner_radius_tr",
        "corner_radius_br",
        "corner_radius_bl",
    ] {
        let value = meta.get(key).expect(key).get_deep_value();
        assert_eq!(value, loro::LoroValue::Double(2.0), "{key}");
    }
    assert!(meta.get("corner_radius").is_none());
}

#[test]
fn the_future_fixture_is_newer_than_the_current_version() {
    let bytes = fixture("future_format_version.curvyo");
    assert!(manifest_format_version(&bytes) > u64::from(CURRENT_FORMAT_VERSION));
    assert!(matches!(
        unpack(2, &bytes),
        Err(OpenError::FormatTooNew { .. })
    ));
}

/// AC 19: a build that knows only version 5 refuses a version-6 container; the
/// manifest alone decides, so this is the same refusal for every new file.
#[test]
fn a_version_6_container_is_newer_than_what_a_version_5_build_supports() {
    const OLD_BUILD_SUPPORTED_VERSION: u64 = 5;
    for bytes in [
        fixture(V6_FIXTURE),
        pack(&Document::new(1), "0.1.0").unwrap(),
    ] {
        assert!(manifest_format_version(&bytes) > OLD_BUILD_SUPPORTED_VERSION);
    }
}

// ---------------------------------------------------------------------
// Version 5 with the legacy single radius (AC 18)
// ---------------------------------------------------------------------

#[test]
fn the_legacy_fixture_is_a_genuine_version_5_container() {
    assert_eq!(manifest_format_version(&fixture(LEGACY_FIXTURE)), 5);
}

#[test]
fn a_legacy_single_radius_opens_as_four_equal_corners() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).expect("opens");
    let ids = document.object_ids();
    assert_eq!(ids.len(), 2);
    assert_eq!(rect_of(&document, ids[0]).1, radii(5.0, 5.0, 5.0, 5.0));
    assert_eq!(rect_of(&document, ids[1]).1, radii(0.0, 0.0, 0.0, 0.0));
}

#[test]
fn a_legacy_rectangle_renders_and_converts_exactly_as_before() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let ids = document.object_ids();
    let ObjectSnapshot::Primitive(rounded) = document.object(ids[0]).unwrap() else {
        panic!("primitive");
    };
    let outline = outline_of_rotated(&rounded.shape, rounded.rotation);
    assert_eq!(outline.len(), 8, "rounded: 8 nodes as in version 5");
    // 40 x 20 with r 5: the slice-3 anchors.
    let first = outline[0];
    assert_eq!((first.point.x, first.point.y), (5.0, 0.0));
    let ObjectSnapshot::Primitive(sharp) = document.object(ids[1]).unwrap() else {
        panic!("primitive");
    };
    assert_eq!(outline_of_rotated(&sharp.shape, sharp.rotation).len(), 4);
}

#[test]
fn opening_a_legacy_file_writes_nothing_and_leaves_its_keys_alone() {
    let bytes = fixture(LEGACY_FIXTURE);
    let original = LoroDoc::new();
    original.import(&member(&bytes, "document.loro")).unwrap();

    let document = unpack(2, &bytes).unwrap();
    let after_open = LoroDoc::new();
    after_open
        .import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    assert_eq!(
        after_open.oplog_vv(),
        original.oplog_vv(),
        "no operation recorded"
    );

    for meta in raw_nodes(&after_open) {
        assert!(meta.get("corner_radius").is_some(), "the legacy key stays");
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            assert!(meta.get(key).is_none(), "{key} is not written on open");
        }
    }
    // The first save writes the new manifest version and still leaves the keys.
    let saved = pack(&document, "0.1.0").unwrap();
    assert_eq!(
        manifest_format_version(&saved),
        u64::from(CURRENT_FORMAT_VERSION)
    );
}

#[test]
fn editing_one_corner_of_a_legacy_rectangle_writes_only_that_register() {
    let document = unpack(2, &fixture(LEGACY_FIXTURE)).unwrap();
    let id = document.object_ids()[0];
    let (_, before) = rect_of(&document, id);
    document
        .set_corner_radii(&[(id, before_with_tl(before, 9.0))])
        .unwrap();
    assert_eq!(rect_of(&document, id).1, radii(9.0, 5.0, 5.0, 5.0));
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let meta = &raw_nodes(&loro)[0];
    assert!(meta.get("corner_radius_tl").is_some());
    for key in ["corner_radius_tr", "corner_radius_br", "corner_radius_bl"] {
        assert!(meta.get(key).is_none(), "{key} untouched");
    }
}

fn before_with_tl(radii: CornerRadii, tl: f64) -> CornerRadii {
    CornerRadii {
        tl: mm(tl),
        ..radii
    }
}

#[test]
fn object_to_path_strips_every_radius_key_old_and_new() {
    use curvyo_document_core::{AnchorId, NewAnchor};
    for bytes in [fixture(LEGACY_FIXTURE), fixture(V6_FIXTURE)] {
        let document = unpack(2, &bytes).unwrap();
        let conversions: Vec<_> = document
            .object_ids()
            .into_iter()
            .map(|id| {
                (
                    id,
                    vec![
                        NewAnchor::corner(AnchorId::new(2, 1), Point::new(0.0, 0.0)),
                        NewAnchor::corner(AnchorId::new(2, 2), Point::new(1.0, 0.0)),
                    ],
                )
            })
            .collect();
        document.convert_to_paths(&conversions).unwrap();
        let loro = LoroDoc::new();
        loro.import(&document.export_loro_snapshot().unwrap())
            .unwrap();
        for meta in raw_nodes(&loro) {
            for key in [
                "corner_radius",
                "corner_radius_tl",
                "corner_radius_tr",
                "corner_radius_br",
                "corner_radius_bl",
                "rect_bounds",
                "shape",
            ] {
                assert!(meta.get(key).is_none(), "{key} was stripped");
            }
        }
    }
}

// ---------------------------------------------------------------------
// Refusals (AC 20)
// ---------------------------------------------------------------------

#[test]
fn a_negative_or_non_finite_radius_is_refused_as_damaged() {
    for bad in [-0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            let bytes = raw_rect_container(|meta| {
                meta.insert("corner_radius", 1.0_f64).unwrap();
                meta.insert(key, bad).unwrap();
            });
            assert!(
                matches!(unpack(2, &bytes), Err(OpenError::Damaged)),
                "{key} = {bad}"
            );
        }
        let legacy = raw_rect_container(|meta| meta.insert("corner_radius", bad).unwrap());
        assert!(
            matches!(unpack(2, &legacy), Err(OpenError::Damaged)),
            "legacy = {bad}"
        );
    }
}

#[test]
fn a_mistyped_per_corner_key_is_damaged_even_with_a_valid_legacy_key() {
    let bytes = raw_rect_container(|meta| {
        meta.insert("corner_radius", 1.0_f64).unwrap();
        meta.insert("corner_radius_br", "big").unwrap();
    });
    assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
}

#[test]
fn a_mistyped_legacy_key_is_damaged_when_a_corner_falls_back_to_it() {
    let bytes = raw_rect_container(|meta| {
        meta.insert("corner_radius", "big").unwrap();
        meta.insert("corner_radius_tl", 1.0_f64).unwrap();
    });
    assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
    // Every corner has its own key: the legacy key is never read.
    let fine = raw_rect_container(|meta| {
        meta.insert("corner_radius", "big").unwrap();
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            meta.insert(key, 1.0_f64).unwrap();
        }
    });
    assert!(unpack(2, &fine).is_ok());
}

#[test]
fn a_corner_without_a_key_and_without_a_legacy_key_is_damaged() {
    let none = raw_rect_container(|_| {});
    assert!(matches!(unpack(2, &none), Err(OpenError::Damaged)));
    let three = raw_rect_container(|meta| {
        for key in ["corner_radius_tl", "corner_radius_tr", "corner_radius_br"] {
            meta.insert(key, 1.0_f64).unwrap();
        }
    });
    assert!(matches!(unpack(2, &three), Err(OpenError::Damaged)));
}

#[test]
fn a_stored_sum_above_a_side_is_not_damaged() {
    let bytes = raw_rect_container(|meta| {
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            meta.insert(key, 500.0_f64).unwrap();
        }
    });
    let document = unpack(2, &bytes).expect("a merge can produce this; open must not refuse it");
    assert_eq!(
        rect_of(&document, document.object_ids()[0]).1,
        radii(500.0, 500.0, 500.0, 500.0)
    );
}

// ---------------------------------------------------------------------
// The outline golden file (AC 11, 16, 21)
// ---------------------------------------------------------------------

fn pair(value: &serde_json::Value) -> (f64, f64) {
    (value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

#[test]
fn the_outline_matches_the_golden_anchors() {
    let golden: serde_json::Value = serde_json::from_slice(&fixture(OUTLINE_FIXTURE)).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), outline_cases().len());
    for (case, golden_case) in outline_cases().iter().zip(cases) {
        assert_eq!(golden_case["name"], case.name);
        let actual = rect_outline(case.bounds, case.radii);
        let expected = golden_case["anchors"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{}: node count", case.name);
        for (anchor, golden_anchor) in actual.iter().zip(expected) {
            for (got, want) in [
                (
                    (anchor.point.x, anchor.point.y),
                    pair(&golden_anchor["point"]),
                ),
                (
                    (anchor.handle_in.x, anchor.handle_in.y),
                    pair(&golden_anchor["handle_in"]),
                ),
                (
                    (anchor.handle_out.x, anchor.handle_out.y),
                    pair(&golden_anchor["handle_out"]),
                ),
            ] {
                assert!(
                    (got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9,
                    "{}: {got:?} vs {want:?}",
                    case.name
                );
            }
        }
    }
}

#[test]
fn the_golden_node_counts_follow_the_sharp_corners() {
    let golden: serde_json::Value = serde_json::from_slice(&fixture(OUTLINE_FIXTURE)).unwrap();
    let counts: Vec<(&str, usize)> = golden["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap(),
                c["anchors"].as_array().unwrap().len(),
            )
        })
        .collect();
    assert_eq!(
        counts,
        [
            ("spec_example_one_f_is_1", 6),
            ("spec_example_two_neighbours_shrink", 7),
            ("diagonal_corners_large", 6),
            ("single_rounded_corner", 5),
            ("all_sharp", 4),
            ("four_equal_within_limit", 8),
            ("four_equal_above_limit_clamped", 8),
            ("offset_origin_all_different", 7),
        ]
    );
}
