//! The compound path in the document model (`specs/0016-boolean-operations`
//! criteria 19 to 22, 27, 28, 30, 35, 35a, 36, 36a, 37, 37a, 38a): one path
//! object with several outlines, its file format and every place of the
//! architect's audit that read or wrote a path's anchors.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]

use std::io::{Cursor, Read, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;

use curvyo_document_core::{
    AnchorId, Angle, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, CopySource, Document,
    DocumentSize, Length, NewAnchor, NodeId, ObjectEditError, ObjectSnapshot, OpenError,
    PathEditError, PathSnapshot, Point, RectBounds, StyleEdit, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroMap, LoroMovableList};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn id(n: u64) -> AnchorId {
    AnchorId::new(7, n)
}

/// A closed square outline of `side` at `(x, y)` with anchor ids from `first`.
fn square(first: u64, x: f64, y: f64, side: f64, reversed: bool) -> (Vec<NewAnchor>, bool) {
    let mut corners = vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    if reversed {
        corners.reverse();
    }
    let anchors = corners
        .into_iter()
        .enumerate()
        .map(|(k, (cx, cy))| NewAnchor::corner(id(first + k as u64), pt(cx, cy)))
        .collect();
    (anchors, true)
}

/// A ring: an outer square and a reversed inner square.
fn ring_outlines() -> Vec<(Vec<NewAnchor>, bool)> {
    vec![
        square(100, 0.0, 0.0, 40.0, false),
        square(200, 10.0, 10.0, 20.0, true),
    ]
}

fn rect_object(document: &Document, x: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn path_with_three(document: &Document, first: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(id(first), pt(0.0, 0.0)),
            NewAnchor::corner(id(first + 1), pt(5.0, 0.0)),
            NewAnchor::corner(id(first + 2), pt(5.0, 5.0)),
        ],
        true,
    )
}

fn path_of(document: &Document, node: NodeId) -> PathSnapshot {
    document.path(node).expect("a path")
}

fn labels(document: &Document) -> Vec<String> {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
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

fn ring(document: &Document) -> NodeId {
    let base = path_with_three(document, 1);
    document
        .replace_with_path(&[base], base, &ring_outlines(), "boolean_difference")
        .unwrap()
}

// ============================================================ replace_with_path

/// Criteria 19 to 22, 27, 28: one new object takes the operands' place with
/// the base's style, rotation 0, in one commit with the operation's label.
#[test]
fn replace_with_path_replaces_the_operands_in_one_labelled_commit() {
    let document = Document::new(1);
    let low = path_with_three(&document, 1);
    let kept = rect_object(&document, 50.0);
    let high = rect_object(&document, 20.0);
    let above = rect_object(&document, 80.0);
    document
        .edit_style(&[low], &StyleEdit::StrokeWidth(Length::from_mm(2.5)))
        .unwrap();
    let before = labels(&document).len();

    let result = document
        .replace_with_path(&[high, low], low, &ring_outlines(), "boolean_union")
        .unwrap();

    assert_eq!(labels(&document).len(), before + 1, "one commit");
    assert_eq!(labels(&document).last().unwrap(), "boolean_union");
    assert_eq!(
        document.object_ids(),
        vec![result, kept, above],
        "the result sits where the base was; the others keep their order"
    );
    let path = path_of(&document, result);
    assert_eq!(path.style.stroke.width.as_mm(), 2.5, "the base's style");
    assert_eq!(path.rotation, Angle::from_radians(0.0));
    assert!(path.is_compound());
    assert!(document.object(low).is_none() && document.object(high).is_none());
}

/// The result takes the style of a primitive base too (the style keys are the
/// same for every kind).
#[test]
fn replace_with_path_takes_the_style_of_a_rectangle_base() {
    let document = Document::new(1);
    let rect = rect_object(&document, 0.0);
    let other = rect_object(&document, 5.0);
    document
        .edit_style(&[rect], &StyleEdit::StrokeWidth(Length::from_mm(4.0)))
        .unwrap();
    let result = document
        .replace_with_path(&[rect, other], rect, &ring_outlines(), "boolean_union")
        .unwrap();
    assert_eq!(path_of(&document, result).style.stroke.width.as_mm(), 4.0);
}

/// Criterion 20: one outline gives an ordinary path; criterion 20/30: several
/// give one object whose outlines, order, winding and ids are kept.
#[test]
fn one_outline_is_an_ordinary_path_and_several_are_one_compound_path() {
    let document = Document::new(1);
    let a = path_with_three(&document, 1);
    let b = path_with_three(&document, 10);
    let single = document
        .replace_with_path(
            &[a],
            a,
            &[square(300, 0.0, 0.0, 10.0, false)],
            "boolean_union",
        )
        .unwrap();
    let plain = path_of(&document, single);
    assert!(!plain.is_compound());
    assert_eq!(plain.extra_subpaths, Vec::new());

    let compound = document
        .replace_with_path(&[b], b, &ring_outlines(), "boolean_difference")
        .unwrap();
    let path = path_of(&document, compound);
    let outlines: Vec<_> = path.subpaths().collect();
    assert_eq!(outlines.len(), 2);
    assert_eq!(outlines[0].anchors.len(), 4);
    assert_eq!(outlines[1].anchors.len(), 4);
    assert!(outlines[0].closed && outlines[1].closed);
    let expected: Vec<_> = ring_outlines()
        .into_iter()
        .flat_map(|(anchors, _)| anchors)
        .collect();
    assert_eq!(path.all_anchors().copied().collect::<Vec<_>>(), expected);
    assert_eq!(document.object_ids().len(), 2);
}

/// "Nothing changes" on every refusal: same objects, no commit.
#[test]
fn replace_with_path_refuses_without_changing_anything() {
    let document = Document::new(1);
    let a = path_with_three(&document, 1);
    let b = rect_object(&document, 20.0);
    let ghost = {
        let doomed = rect_object(&document, 99.0);
        document.delete_objects(&[doomed]).unwrap();
        doomed
    };
    let ids = document.object_ids();
    let changes = labels(&document).len();
    let snapshot = (document.object(a), document.object(b));

    let cases: Vec<(Result<NodeId, ObjectEditError>, ObjectEditError)> = vec![
        (
            document.replace_with_path(&[a, ghost], a, &ring_outlines(), "x"),
            ObjectEditError::NoSuchObject,
        ),
        (
            document.replace_with_path(&[a], b, &ring_outlines(), "x"),
            ObjectEditError::BaseNotAnOperand,
        ),
        (
            document.replace_with_path(&[a, b], a, &[], "x"),
            ObjectEditError::NoOutlines,
        ),
        (
            document.replace_with_path(&[a, b], a, &[(Vec::new(), true)], "x"),
            ObjectEditError::NoOutlines,
        ),
        (
            document.replace_with_path(
                &[a, b],
                a,
                &[
                    square(1, 0.0, 0.0, 5.0, false),
                    square(3, 9.0, 9.0, 5.0, false),
                ],
                "x",
            ),
            ObjectEditError::AnchorIds,
        ),
    ];
    for (result, expected) in cases {
        assert_eq!(result, Err(expected));
    }
    assert_eq!(document.object_ids(), ids);
    assert_eq!(labels(&document).len(), changes, "no commit");
    assert_eq!((document.object(a), document.object(b)), snapshot);
}

// ============================================================ file format

/// Criteria 30, 37: save and reopen keeps outlines, order, winding, ids and
/// style; the file declares the new format version.
#[test]
fn a_compound_path_survives_save_and_reopen() {
    let document = Document::new(1);
    let node = ring(&document);
    document
        .edit_style(&[node], &StyleEdit::StrokeWidth(Length::from_mm(3.0)))
        .unwrap();
    let before = document.object(node).unwrap();

    let bytes = pack(&document, "0.1.0").unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&bytes, "manifest.json")).unwrap();
    assert_eq!(manifest["format_version"], CURRENT_FORMAT_VERSION);
    assert!(
        manifest["format_version"].as_u64().unwrap() > u64::from(PREVIOUS_FORMAT_VERSION),
        "an earlier build must refuse the file as too new"
    );

    let reopened = unpack(2, &bytes).unwrap();
    assert_eq!(reopened.object(node), Some(before));
    let json: serde_json::Value = serde_json::from_slice(&member(&bytes, "document.json")).unwrap();
    let objects = json["objects"].as_array().unwrap();
    assert_eq!(objects[0]["extra_subpaths"].as_array().unwrap().len(), 1);
}

/// The format version of the build that introduced compound paths. The golden declares it and
/// stays as that build wrote it: a later build has to open it, so a later bump leaves this number
/// and the golden alone (the literal pin on `CURRENT_FORMAT_VERSION` lives in `dash_format.rs`).
const COMPOUND_FORMAT_VERSION: u32 = 8;

/// The version before this build, which no longer opens a compound path.
const PREVIOUS_FORMAT_VERSION: u32 = COMPOUND_FORMAT_VERSION - 1;

/// Criterion 37a: an ordinary path is written without the key, and exports
/// without it.
#[test]
fn an_ordinary_path_is_written_without_extra_subpaths() {
    let document = Document::new(1);
    let node = path_with_three(&document, 1);
    let bytes = pack(&document, "0.1.0").unwrap();
    let json: serde_json::Value = serde_json::from_slice(&member(&bytes, "document.json")).unwrap();
    assert!(json["objects"][0].get("extra_subpaths").is_none());
    let loro = LoroDoc::new();
    loro.import(&member(&bytes, "document.loro")).unwrap();
    let tree = loro.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    assert!(meta.get("extra_subpaths").is_none());
    assert!(!path_of(&unpack(2, &bytes).unwrap(), node).is_compound());
}

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

fn container(format_version: u32, loro_bytes: &[u8]) -> Vec<u8> {
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
        ("document.json", b"{}".to_vec()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// The committed golden of a compound path (a ring and a plain path), written
/// by this build's format. Regenerate with
/// `CURVYO_WRITE_FIXTURES=1 cargo test -p curvyo-document-core --test compound_path`.
const GOLDEN: &str = "compound_v8.curvyo";

fn golden_document() -> Document {
    let document = Document::new(1);
    let _ = path_with_three(&document, 1);
    let base = path_with_three(&document, 10);
    let _ = document
        .replace_with_path(&[base], base, &ring_outlines(), "boolean_difference")
        .unwrap();
    document
}

/// Criterion 30/37: the committed golden reopens with the same outlines.
#[test]
fn the_committed_golden_reopens_with_the_same_outlines() {
    if std::env::var_os("CURVYO_WRITE_FIXTURES").is_some() {
        let bytes = pack(&golden_document(), "0.1.0").unwrap();
        std::fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(GOLDEN),
            bytes,
        )
        .unwrap();
    }
    let golden = fixture(GOLDEN);
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&golden, "manifest.json")).unwrap();
    assert_eq!(
        manifest["format_version"], COMPOUND_FORMAT_VERSION,
        "the golden declares the version that introduced compound paths"
    );
    let reopened = unpack(2, &golden).unwrap();
    let ids = reopened.object_ids();
    assert_eq!(ids.len(), 2);
    assert!(!path_of(&reopened, ids[0]).is_compound());
    let compound = path_of(&reopened, ids[1]);
    let expected: Vec<_> = ring_outlines()
        .into_iter()
        .flat_map(|(anchors, _)| anchors)
        .collect();
    assert_eq!(
        compound.all_anchors().copied().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(compound.extra_subpaths.len(), 1);
    assert_eq!(compound.extra_subpaths[0].anchors.len(), 4);
}

/// Criterion 37: a file from an earlier build opens unchanged and is not
/// rewritten: nothing is added to its Loro state, and its paths are ordinary.
#[test]
fn a_file_from_the_previous_version_opens_unchanged() {
    for name in [
        "legacy_gradient_v7.curvyo",
        "paths_v2.curvyo",
        "rotation_v5.curvyo",
    ] {
        let bytes = fixture(name);
        let document = unpack(2, &bytes).unwrap();
        for node in document.object_ids() {
            if let Some(path) = document.path(node) {
                assert!(!path.is_compound(), "{name}");
            }
        }
        let stored = LoroDoc::new();
        stored.import(&member(&bytes, "document.loro")).unwrap();
        let reread = LoroDoc::new();
        reread
            .import(&document.export_loro_snapshot().unwrap())
            .unwrap();
        assert_eq!(
            reread.get_deep_value(),
            stored.get_deep_value(),
            "{name}: opening must not rewrite the document"
        );
    }
}

/// Opens the ring with `edit` applied to its meta map in a raw Loro doc.
fn open_ring_with(edit: impl FnOnce(&LoroMap)) -> Result<Document, OpenError> {
    let document = Document::new(1);
    let _ = ring(&document);
    let loro = LoroDoc::new();
    loro.set_peer_id(1).unwrap();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let tree = loro.get_tree("paths");
    edit(&tree.get_meta(tree.roots()[0]).unwrap());
    loro.commit();
    unpack(
        2,
        &container(
            CURRENT_FORMAT_VERSION,
            &loro.export(loro::ExportMode::Snapshot).unwrap(),
        ),
    )
}

fn extras(meta: &LoroMap) -> LoroMovableList {
    match meta.get("extra_subpaths") {
        Some(loro::ValueOrContainer::Container(loro::Container::MovableList(list))) => list,
        _ => panic!("the ring has extra_subpaths"),
    }
}

type Edit = Box<dyn FnOnce(&LoroMap)>;

/// Criterion 37a: damaged extra outlines are refused as damaged, never a
/// crash.
#[test]
fn damaged_extra_outlines_are_refused_as_damaged() {
    assert!(open_ring_with(|_| {}).is_ok(), "the unedited ring opens");
    let damaged: Vec<(&str, Edit)> = vec![
        (
            "not a list",
            Box::new(|meta| meta.insert("extra_subpaths", "oops").unwrap()),
        ),
        (
            "a number instead of an outline",
            Box::new(|meta| extras(meta).push(3.0).unwrap()),
        ),
        (
            "an outline without anchors",
            Box::new(|meta| {
                let list = extras(meta);
                let _ = list.push_container(LoroMap::new()).unwrap();
            }),
        ),
        (
            "an outline with an empty anchor list",
            Box::new(|meta| {
                let map = extras(meta).push_container(LoroMap::new()).unwrap();
                let _ = map
                    .insert_container("anchors", LoroMovableList::new())
                    .unwrap();
            }),
        ),
        (
            "anchors that are not a list",
            Box::new(|meta| {
                let map = extras(meta).push_container(LoroMap::new()).unwrap();
                map.insert("anchors", 1.0).unwrap();
            }),
        ),
        (
            "an anchor that is not a map",
            Box::new(|meta| {
                let map = extras(meta).push_container(LoroMap::new()).unwrap();
                let anchors = map
                    .insert_container("anchors", LoroMovableList::new())
                    .unwrap();
                anchors.push("x").unwrap();
            }),
        ),
        (
            "an anchor without an id",
            Box::new(|meta| {
                let map = extras(meta).push_container(LoroMap::new()).unwrap();
                let anchors = map
                    .insert_container("anchors", LoroMovableList::new())
                    .unwrap();
                let _ = anchors.push_container(LoroMap::new()).unwrap();
            }),
        ),
    ];
    for (what, edit) in damaged {
        assert!(
            matches!(open_ring_with(edit), Err(OpenError::Damaged)),
            "{what}"
        );
    }
}

/// An empty `extra_subpaths` list reads like an absent one.
#[test]
fn an_empty_extra_list_reads_as_an_ordinary_path() {
    let document = open_ring_with(|meta| {
        let list = extras(meta);
        while !list.is_empty() {
            list.delete(0, 1).unwrap();
        }
    })
    .unwrap();
    let node = document.object_ids()[0];
    assert!(!path_of(&document, node).is_compound());
}

// ============================================================ the audit

fn flat(path: &PathSnapshot) -> Vec<(f64, f64, f64, f64, f64, f64)> {
    path.all_anchors()
        .map(|a| {
            (
                a.point.x,
                a.point.y,
                a.handle_in.x,
                a.handle_in.y,
                a.handle_out.x,
                a.handle_out.y,
            )
        })
        .collect()
}

/// Criterion 35: `rotated`, `scaled` and `sheared` map every outline; the
/// hole stays a hole (the centre of the ring is still outside every fill).
#[test]
fn snapshot_transforms_map_every_outline() {
    let document = Document::new(1);
    let node = ring(&document);
    let path = path_of(&document, node);
    let pivot = pt(20.0, 20.0);

    let rotated = path.rotated(pivot, Angle::from_radians(std::f64::consts::FRAC_PI_2));
    assert_eq!(rotated.extra_subpaths.len(), 1);
    // (0,0) -> about (20,20) by 90 degrees -> (40, 0); the hole's (10,10) -> (30, 10).
    assert!((rotated.anchors[0].point.x - 40.0).abs() < 1e-9);
    assert!((rotated.extra_subpaths[0].anchors[3].point.x - 30.0).abs() < 1e-9);
    assert!((rotated.extra_subpaths[0].anchors[3].point.y - 10.0).abs() < 1e-9);

    let scaled = path.scaled(pivot, 1.5, 0.5);
    let hole_first = scaled.extra_subpaths[0].anchors[0].point;
    // The reversed hole starts at (10, 30): x 20 + (10-20)*1.5, y 20 + (30-20)*0.5.
    assert!((hole_first.x - 5.0).abs() < 1e-9 && (hole_first.y - 25.0).abs() < 1e-9);

    let sheared = path.sheared(pivot, 0.5, 0.0);
    let hole_first = sheared.extra_subpaths[0].anchors[0].point;
    // x shear: x' = x + 0.5 * (y - 20) -> 10 + 0.5 * 10 = 15.
    assert!((hole_first.x - 15.0).abs() < 1e-9 && (hole_first.y - 30.0).abs() < 1e-9);
    for transformed in [&rotated, &scaled, &sheared] {
        assert_eq!(
            transformed.all_anchors().count(),
            path.all_anchors().count()
        );
        assert_eq!(
            transformed.all_anchors().map(|a| a.id).collect::<Vec<_>>(),
            path.all_anchors().map(|a| a.id).collect::<Vec<_>>()
        );
    }
}

/// Criterion 35, 36: Move shifts every outline in one commit; a preview
/// (`ObjectSnapshot::translated`) and the commit agree.
#[test]
fn move_translates_every_outline_and_agrees_with_the_preview() {
    let document = Document::new(1);
    let node = ring(&document);
    let before = document.object(node).unwrap();
    let preview = before.translated(Vec2::new(3.0, -2.0));
    let changes = labels(&document).len();
    document
        .translate_objects(&[node], Vec2::new(3.0, -2.0))
        .unwrap();
    assert_eq!(labels(&document).len(), changes + 1);
    assert_eq!(document.object(node), Some(preview));
    let ObjectSnapshot::Path(path) = document.object(node).unwrap() else {
        panic!("a path")
    };
    assert_eq!(path.extra_subpaths[0].anchors[0].point, pt(13.0, 28.0));
}

/// Criterion 35: a rotate commit writes every outline's anchors.
#[test]
fn rotate_commit_writes_every_outline() {
    let document = Document::new(1);
    let node = ring(&document);
    let rotated = document
        .object(node)
        .unwrap()
        .rotated(pt(20.0, 20.0), Angle::from_radians(0.6));
    document.rotate_object(&rotated).unwrap();
    assert_eq!(document.object(node), Some(rotated.clone()));
    let ObjectSnapshot::Path(path) = rotated else {
        panic!("a path")
    };
    assert_eq!(path.rotation, Angle::from_radians(0.6));
}

/// Criterion 35, 35a: a resize commit writes every outline and the stroke
/// width once for the object.
#[test]
fn resize_commit_writes_every_outline_and_scales_the_width_once() {
    let document = Document::new(1);
    let node = ring(&document);
    let path = path_of(&document, node);
    let scaled = path.scaled(pt(0.0, 0.0), 2.0, 3.0);
    let anchors: Vec<_> = scaled
        .all_anchors()
        .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
        .collect();
    let changes = labels(&document).len();
    document
        .resize_path(node, &anchors, Some(Length::from_mm(7.0)))
        .unwrap();
    assert_eq!(labels(&document).len(), changes + 1);
    let after = path_of(&document, node);
    assert_eq!(flat(&after), flat(&scaled));
    assert_eq!(after.style.stroke.width.as_mm(), 7.0, "written once");
    assert_eq!(
        document.resize_path(
            node,
            &[(id(999), pt(0.0, 0.0), Vec2::ZERO, Vec2::ZERO)],
            None
        ),
        Err(PathEditError::NoSuchAnchor)
    );
}

/// Criteria 36, 36a: a copy has the same outlines, every anchor of every
/// outline a new unique id, and the original keeps its ids.
#[test]
fn duplicate_gives_every_outline_new_anchor_ids() {
    let document = Document::new(1);
    let node = ring(&document);
    let original = path_of(&document, node);
    let fresh: Vec<AnchorId> = (0..8).map(|k| id(5000 + k)).collect();

    let copies = document
        .duplicate_objects(
            &[CopySource {
                id: node,
                anchor_ids: fresh.clone(),
            }],
            Vec2::new(50.0, 0.0),
        )
        .unwrap();
    let copy = path_of(&document, copies[0]);
    assert_eq!(copy.extra_subpaths.len(), 1);
    assert_eq!(
        copy.all_anchors().map(|a| a.id).collect::<Vec<_>>(),
        fresh,
        "new ids in outline order"
    );
    for (new, old) in copy.all_anchors().zip(original.all_anchors()) {
        assert_eq!(new.point, old.point.translated(Vec2::new(50.0, 0.0)));
        assert_eq!(new.kind, old.kind);
    }
    assert_eq!(
        path_of(&document, node),
        original,
        "the original is untouched"
    );

    // The count must cover all outlines, not only the first.
    let first_outline_only: Vec<AnchorId> = (0..4).map(|k| id(6000 + k)).collect();
    assert_eq!(
        document.duplicate_objects(
            &[CopySource {
                id: node,
                anchor_ids: first_outline_only
            }],
            Vec2::ZERO
        ),
        Err(ObjectEditError::AnchorIds)
    );
}

/// Criterion 36: Delete and style edits work as on any object.
#[test]
fn delete_and_style_edits_work_on_a_compound_path() {
    let document = Document::new(1);
    let node = ring(&document);
    document
        .edit_style(&[node], &StyleEdit::StrokeWidth(Length::from_mm(1.25)))
        .unwrap();
    let path = path_of(&document, node);
    assert_eq!(path.style.stroke.width.as_mm(), 1.25);
    assert!(path.is_compound(), "a style edit keeps the outlines");
    document.delete_objects(&[node]).unwrap();
    assert!(document.object(node).is_none());
}

/// Criterion 38a: Join and Split refuse a compound path, and check_* say so.
#[test]
fn join_and_split_refuse_a_compound_path() {
    let document = Document::new(1);
    let node = ring(&document);
    let other = document.create_path(
        &[
            NewAnchor::corner(id(900), pt(0.0, 0.0)),
            NewAnchor::corner(id(901), pt(1.0, 0.0)),
            NewAnchor::corner(id(902), pt(2.0, 0.0)),
        ],
        false,
    );
    let first = id(100);
    let extra = id(200);
    let changes = labels(&document).len();
    for (a, b) in [(first, extra), (first, first), (extra, extra)] {
        assert!(!document.check_join(node, a, node, b));
    }
    assert!(!document.check_join(node, first, other, id(900)));
    assert!(!document.check_join(other, id(900), node, first));
    assert_eq!(
        document.join_endpoints(node, first, other, id(900)),
        Err(PathEditError::NotJoinable)
    );
    assert!(!document.check_split(node, first));
    assert!(!document.check_split(node, extra));
    assert_eq!(
        document.split_at_anchor(node, first, id(950)),
        Err(PathEditError::NotSplittable)
    );
    assert_eq!(labels(&document).len(), changes, "nothing changed");
    // A kind of sanity check that an ordinary closed path can still split.
    let plain = path_with_three(&document, 40);
    assert!(document.check_split(plain, id(41)));
}

/// Rulers criterion 17 with a compound path: resizing the document keeps the
/// content centred, which moves every outline by the same half-change.
#[test]
fn resizing_the_document_moves_every_outline_by_the_half_change() {
    let document = Document::new(1);
    let node = ring(&document);
    let before = path_of(&document, node);
    let old = document.size();
    document
        .resize(DocumentSize::from_mm(
            old.width.as_mm() + 90.0,
            old.height.as_mm() + 103.0,
        ))
        .unwrap();
    let after = path_of(&document, node);
    assert_eq!(after.all_anchors().count(), before.all_anchors().count());
    for (new, old) in after.all_anchors().zip(before.all_anchors()) {
        assert_eq!(new.point, old.point.translated(Vec2::new(45.0, 51.5)));
        assert_eq!(new.id, old.id);
    }
    assert_eq!(
        after.extra_subpaths.len(),
        1,
        "the hole moved with the rest"
    );
}

/// Tester D4: an anchor id names a node across the whole document, so a result may not reuse the id
/// of an object that stays. The ids of the replaced operands are free again.
#[test]
fn replace_with_path_refuses_an_anchor_id_of_an_object_that_stays() {
    let document = Document::new(1);
    let stays = path_with_three(&document, 500);
    let operand = path_with_three(&document, 600);
    let before = (document.object(stays), document.object(operand));
    let changes = labels(&document).len();

    let clash = [square(501, 0.0, 0.0, 5.0, false)];
    assert_eq!(
        document.replace_with_path(&[operand], operand, &clash, "boolean_union"),
        Err(ObjectEditError::AnchorIds)
    );
    assert_eq!(labels(&document).len(), changes, "no commit");
    assert_eq!((document.object(stays), document.object(operand)), before);

    // Reusing an id of the operand that is replaced is fine.
    let reuse = [square(600, 0.0, 0.0, 5.0, false)];
    assert!(
        document
            .replace_with_path(&[operand], operand, &reuse, "boolean_union")
            .is_ok()
    );
}
