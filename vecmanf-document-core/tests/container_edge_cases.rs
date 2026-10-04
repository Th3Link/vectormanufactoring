//! White-box edge-case tests for the `.vmf` container (`src/container.rs`),
//! written after reading the implementation diff. These target the
//! degenerate inputs the golden fixtures and inline unit tests do not
//! cover: a genuinely empty file versus a structurally-valid-but-empty
//! zip, a container missing only the non-manifest required member, the
//! exact version boundary (current vs. current+1), the snapshot-version
//! field refusing independently of the container-version field, a
//! differently-shaped-but-valid zip (not a vecmanf container at all), and
//! a corrupt `document.loro` hiding behind an otherwise valid manifest.
//!
//! Scope note: this file only constructs container bytes and calls the
//! crate's public `pack`/`unpack`. It does not touch production code.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Cursor, Write};

use serde::Serialize;
use vecmanf_document_core::{
    CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Document, OpenError, pack, unpack,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A hand-built mirror of the private `Manifest` shape in `container.rs`.
/// Field names/types must match exactly for these tests to exercise the
/// real parser; this is deliberate duplication (a black-box test cannot
/// `use` a private type) rather than an attempt to share code with it.
#[allow(clippy::struct_field_names)]
#[derive(Serialize)]
struct TestManifest {
    format_version: u32,
    loro_snapshot_version: u32,
    app_version: String,
}

fn write_member(writer: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]) {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(name, options).expect("start_file");
    writer.write_all(bytes).expect("write_all");
}

fn zip_with_members(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        write_member(&mut writer, name, bytes);
    }
    writer.finish().expect("finish").into_inner()
}

fn manifest_bytes(format_version: u32, loro_snapshot_version: u32) -> Vec<u8> {
    serde_json::to_vec(&TestManifest {
        format_version,
        loro_snapshot_version,
        app_version: "0.1.0-test".to_string(),
    })
    .expect("serialize manifest")
}

/// A genuinely empty file (0 bytes) is not zip-shaped at all and must be
/// refused as `NotAVmf`, the same bucket as a renamed text file.
#[test]
fn truly_empty_file_is_not_a_vmf() {
    assert!(matches!(unpack(2, &[]), Err(OpenError::NotAVmf)));
}

/// A *structurally valid* empty zip archive (just an end-of-central-
/// directory record, no entries at all) is zip-shaped and must therefore
/// fall into the `Damaged` bucket (missing required members), not
/// `NotAVmf` — these two cases must not be conflated by the zip-sniff
/// check.
#[test]
fn structurally_valid_but_empty_zip_is_damaged_not_not_a_vmf() {
    let empty_zip = ZipWriter::new(Cursor::new(Vec::new()))
        .finish()
        .expect("finish")
        .into_inner();
    // Sanity: this really is a minimal-but-valid zip, not accidentally
    // empty bytes.
    assert_ne!(empty_zip.len(), 0);

    let result = unpack(2, &empty_zip);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}

/// A zip containing only `manifest.json` (valid, current versions) and
/// neither `document.loro` nor `document.json` must be refused as
/// `Damaged` — the required-member check for `document.loro` must fire
/// even when the manifest itself is perfectly valid.
#[test]
fn zip_with_only_manifest_json_is_damaged() {
    let manifest = manifest_bytes(CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION);
    let bytes = zip_with_members(&[("manifest.json", &manifest)]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}

/// Boundary check: a `format_version` and `loro_snapshot_version` exactly
/// equal to what this build writes must open successfully — the refusal
/// must be a strict "newer than", not "newer than or equal to". Uses a
/// real exported Loro snapshot so a passing manifest check actually
/// reaches a successful `Document` reconstruction.
#[test]
fn format_version_exactly_current_opens_successfully() {
    let document = Document::new(1);
    let whole = pack(&document, "0.1.0").expect("pack");
    // The real pack() already writes exactly the current versions; this
    // test pins that "current == accepted" boundary explicitly so a
    // future off-by-one in the `>` comparison (e.g. accidentally becoming
    // `>=`) is caught here rather than only in the "one above" case.
    let result = unpack(2, &whole);
    assert!(
        result.is_ok(),
        "pack()'s own current-version output must open"
    );
}

/// The `loro_snapshot_version` field must independently gate acceptance:
/// a container whose container-level `format_version` is current but
/// whose `loro_snapshot_version` is one above current must still be
/// refused as `FormatTooNew`, with `found`/`supported` reflecting the
/// snapshot-version field, not the (current, and therefore unremarkable)
/// container version.
#[test]
fn loro_snapshot_version_above_current_is_refused_even_when_format_version_is_current() {
    let manifest = manifest_bytes(CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION + 1);
    let bytes = zip_with_members(&[
        ("manifest.json", &manifest),
        ("document.loro", b"irrelevant, must not be read"),
        ("document.json", b"{}"),
    ]);

    let result = unpack(2, &bytes);
    match result {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!(found, CURRENT_LORO_SNAPSHOT_VERSION + 1);
            // `supported` must echo back the *snapshot* version's own
            // bound, not some combination with the (here, unrelated and
            // already-current) container `format_version` — the
            // production code's own documented contract is "each
            // comparison reports found/supported from the one field it
            // actually checked" (`src/container.rs`). A `.max()` of the
            // two version numbers happened to equal this when both
            // defaulted to 1; `path-node-editing` bumping only
            // `format_version` (now 2) exposed that the `.max()` here was
            // never actually what the implementation promises.
            assert_eq!(supported, CURRENT_LORO_SNAPSHOT_VERSION);
        }
        Ok(_) => panic!("expected FormatTooNew, got Ok"),
        Err(other) => panic!("expected FormatTooNew, got {other:?}"),
    }
}

/// A `.vmf` that is actually a different, perfectly valid zip-based
/// document format (simulated here as a minimal "docx-shaped" archive:
/// `[Content_Types].xml` and `word/document.xml`, no vecmanf members at
/// all) must be refused as `Damaged` ("missing a required member"), not
/// `NotAVmf` — per the spec's own grouping, "missing required members" is
/// explicitly part of the *damaged* sentence, not the *not-a-.vmf-at-all*
/// one, even though intuitively one might call a foreign-but-valid zip
/// "not a vecmanf project".
#[test]
fn a_different_but_valid_zip_format_is_damaged_not_not_a_vmf() {
    let bytes = zip_with_members(&[
        (
            "[Content_Types].xml",
            b"<?xml version=\"1.0\"?><Types/>" as &[u8],
        ),
        ("word/document.xml", b"<document/>"),
    ]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged (valid zip, wrong contents), got {:?}",
        result.err()
    );
}

/// A corrupt `document.loro` member behind an otherwise perfectly valid
/// manifest and `document.json` must fail cleanly as `Damaged`, not
/// panic. This exercises the path from `container::unpack` through
/// `Document::from_loro_snapshot`'s own error mapping, which no existing
/// test (unit or golden-fixture) drives end-to-end through the public
/// `unpack` entry point.
#[test]
fn corrupt_loro_snapshot_behind_a_valid_manifest_is_damaged() {
    let manifest = manifest_bytes(CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION);
    let bytes = zip_with_members(&[
        ("manifest.json", &manifest),
        ("document.loro", b"this is not a loro snapshot"),
        ("document.json", b"{}"),
    ]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}

/// A zip-shaped file whose `manifest.json` member is present but is not
/// valid JSON must fail as `Damaged`, not panic — the manifest parse
/// failure path, distinct from the manifest-missing-entirely path already
/// covered elsewhere.
#[test]
fn unparsable_manifest_json_is_damaged() {
    let bytes = zip_with_members(&[
        ("manifest.json", b"{ this is not valid json"),
        ("document.loro", b"irrelevant"),
        ("document.json", b"{}"),
    ]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}

/// A `document.loro` snapshot that is itself a perfectly valid Loro
/// document, behind a perfectly valid manifest, but whose `paths` tree
/// has a node not shaped like `vecmanf_document_core::paths` ever writes
/// one (`specs/0002-path-node-editing/adrs.md`'s architect review: "a
/// container whose path data does not match the schema is refused with
/// `OpenError::Damaged`"). Without that validation, this exact byte
/// sequence would import without error and only panic the first time
/// something called `Document::path_ids()`/`path()` on it — this test
/// pins the earlier, named refusal instead (project-file-foundation's
/// acceptance criterion 7: "not a crash").
///
/// Built with `loro` directly rather than through
/// `vecmanf_document_core::Document`: the public API has no way to
/// produce a malformed path node, by design.
#[test]
fn a_path_node_missing_its_anchors_list_is_damaged_not_a_panic() {
    let loro = loro::LoroDoc::new();
    loro.set_peer_id(1).expect("set peer id");
    let tree = loro.get_tree("paths");
    let node = tree.create(loro::TreeParentId::Root).expect("create node");
    let meta = tree.get_meta(node).expect("meta");
    // A real path node always has an `anchors` movable list; this one
    // has no `anchors` key at all, which `anchors_container` would
    // otherwise `.unwrap()` on.
    meta.insert("closed", false).expect("insert closed");
    loro.commit();
    let snapshot = loro.export(loro::ExportMode::Snapshot).expect("export");

    let manifest = manifest_bytes(CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION);
    let bytes = zip_with_members(&[
        ("manifest.json", &manifest),
        ("document.loro", &snapshot),
        ("document.json", b"{}"),
    ]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}

/// The sibling case: an `anchors` list whose element is a plain value
/// (not a map), which `anchor_map_at` would otherwise `panic!` on the
/// first read.
#[test]
fn a_path_node_with_a_non_map_anchor_element_is_damaged_not_a_panic() {
    let loro = loro::LoroDoc::new();
    loro.set_peer_id(1).expect("set peer id");
    let tree = loro.get_tree("paths");
    let node = tree.create(loro::TreeParentId::Root).expect("create node");
    let meta = tree.get_meta(node).expect("meta");
    meta.insert("closed", false).expect("insert closed");
    let anchors = meta
        .insert_container("anchors", loro::LoroMovableList::new())
        .expect("insert anchors container");
    anchors.push("not a map".to_string()).expect("push");
    loro.commit();
    let snapshot = loro.export(loro::ExportMode::Snapshot).expect("export");

    let manifest = manifest_bytes(CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION);
    let bytes = zip_with_members(&[
        ("manifest.json", &manifest),
        ("document.loro", &snapshot),
        ("document.json", b"{}"),
    ]);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected Damaged, got {:?}",
        result.err()
    );
}
