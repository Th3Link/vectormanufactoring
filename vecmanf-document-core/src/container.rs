//! The `.vmf` project container: a zip archive holding `manifest.json`,
//! `document.loro` and `document.json` (ADR 0004 §1), as pure, byte-level
//! code — zip in/out over `&[u8]`/`Vec<u8>`, no `std::fs`
//! (ADR 0011 §2, `vecmanf-document-core`'s responsibility).
//!
//! `manifest.json` is read first and alone decides whether the rest of the
//! container is trusted (`specs/project-file-foundation/adrs.md`,
//! "container version lives in a `manifest.json` member"): it carries the
//! container `format_version`, the `loro_snapshot_version` this crate wrote
//! the snapshot with, and an informational `app_version`.

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::document::{CURRENT_FORMAT_VERSION, Document};
use crate::error::{OpenError, SaveError};

/// The Loro snapshot encoding version this build writes and the newest it
/// accepts on read (ADR 0004 §9: "a Loro format change is treated as a
/// format migration of ours"). Independent of the `loro` crate's own
/// version — this number only changes when *this crate* changes how it
/// writes or reads the snapshot bytes.
pub const CURRENT_LORO_SNAPSHOT_VERSION: u32 = 1;

const MANIFEST_MEMBER: &str = "manifest.json";
const LORO_SNAPSHOT_MEMBER: &str = "document.loro";
const DOCUMENT_JSON_MEMBER: &str = "document.json";

/// Zip local-file-header and empty-archive signatures. A buffer not
/// starting with one of these is not zip-shaped at all — the "not a .vmf
/// (.vmf) file" case — as distinct from a zip-shaped buffer that is
/// truncated or otherwise corrupt.
const ZIP_LOCAL_FILE_HEADER: &[u8] = b"PK\x03\x04";
const ZIP_EMPTY_ARCHIVE: &[u8] = b"PK\x05\x06";

/// The container's `manifest.json` member.
// All three fields ending in `version` is the on-disk shape this slice's
// feature-local decision fixes (`specs/project-file-foundation/adrs.md`,
// "container version lives in a `manifest.json` member") — not a naming
// accident to refactor away.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format_version: u32,
    loro_snapshot_version: u32,
    app_version: String,
}

/// Packs a [`Document`] into `.vmf` container bytes.
///
/// Writes the three required members in one pass from a single frozen
/// read of `document` (ADR 0009 §4), so `document.loro` and
/// `document.json` can never describe different states
/// (`specs/project-file-foundation/adrs.md`).
///
/// # Errors
/// Returns [`SaveError`] if the document cannot be exported or the zip
/// container cannot be assembled.
pub fn pack(document: &Document, app_version: &str) -> Result<Vec<u8>, SaveError> {
    let loro_bytes = document.export_loro_snapshot()?;
    let json_bytes = document.export_json()?;
    let manifest = Manifest {
        format_version: CURRENT_FORMAT_VERSION,
        loro_snapshot_version: CURRENT_LORO_SNAPSHOT_VERSION,
        app_version: app_version.to_string(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| SaveError::Container)?;

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_member(&mut writer, MANIFEST_MEMBER, &manifest_bytes, options)?;
    write_member(&mut writer, LORO_SNAPSHOT_MEMBER, &loro_bytes, options)?;
    write_member(&mut writer, DOCUMENT_JSON_MEMBER, &json_bytes, options)?;
    let cursor = writer.finish().map_err(|_| SaveError::Container)?;
    Ok(cursor.into_inner())
}

fn write_member(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), SaveError> {
    writer
        .start_file(name, options)
        .map_err(|_| SaveError::Container)?;
    writer.write_all(bytes).map_err(|_| SaveError::Container)
}

/// Unpacks `.vmf` container bytes into a [`Document`], bound to the given
/// Loro peer id.
///
/// `peer_id` exists for the same reason [`Document::new`]'s does: this
/// `*-core` crate must not draw one from `getrandom` itself
/// (`CLAUDE.md` §6). The caller (`vecmanf-app`) mints a fresh id per open
/// session, the same way it does for a brand-new document.
///
/// `manifest.json` is read and validated before `document.loro` is even
/// looked at, so a refusal never partially reads the rest of the container
/// (ADR 0004 §9) and never touches an already-open project
/// (`specs/project-file-foundation/specification.md`, AC7).
///
/// # Errors
/// Returns [`OpenError::NotAVmf`] if `bytes` is not zip-shaped at all,
/// [`OpenError::Damaged`] if it is zip-shaped but truncated, corrupt, or
/// missing a required member, and [`OpenError::FormatTooNew`] if
/// `manifest.json` declares a newer version than this build supports.
pub fn unpack(peer_id: u64, bytes: &[u8]) -> Result<Document, OpenError> {
    if !looks_like_zip(bytes) {
        return Err(OpenError::NotAVmf);
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| OpenError::Damaged)?;
    let manifest = read_manifest(&mut archive)?;

    // Two independent fields, two independent bounds (ADR 0004 §9 covers
    // the Loro snapshot version as well as the container's own). Each
    // comparison reports `found`/`supported` from the one field it
    // actually checked — never a `max()` of two unrelated numbers, which
    // would misattribute the refusal to whichever field happened to have
    // the larger value rather than whichever one actually exceeded its
    // own bound.
    if manifest.format_version > CURRENT_FORMAT_VERSION {
        return Err(OpenError::FormatTooNew {
            found: manifest.format_version,
            supported: CURRENT_FORMAT_VERSION,
        });
    }
    if manifest.loro_snapshot_version > CURRENT_LORO_SNAPSHOT_VERSION {
        return Err(OpenError::FormatTooNew {
            found: manifest.loro_snapshot_version,
            supported: CURRENT_LORO_SNAPSHOT_VERSION,
        });
    }

    let loro_bytes = read_member(&mut archive, LORO_SNAPSHOT_MEMBER)?;
    Document::from_loro_snapshot(peer_id, &loro_bytes)
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(ZIP_LOCAL_FILE_HEADER) || bytes.starts_with(ZIP_EMPTY_ARCHIVE)
}

fn read_manifest(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<Manifest, OpenError> {
    let bytes = read_member(archive, MANIFEST_MEMBER)?;
    serde_json::from_slice(&bytes).map_err(|_| OpenError::Damaged)
}

fn read_member(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, OpenError> {
    let mut file = archive.by_name(name).map_err(|_| OpenError::Damaged)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|_| OpenError::Damaged)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_then_unpack_round_trips_size() {
        let original = Document::new(1);
        let bytes = pack(&original, "0.1.0").expect("pack");
        let reopened = unpack(2, &bytes).expect("unpack");
        assert_eq!(original.size(), reopened.size());
    }

    #[test]
    fn packed_container_contains_the_required_members() {
        let document = Document::new(1);
        let bytes = pack(&document, "0.1.0").expect("pack");
        let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).expect("zip");
        assert!(archive.by_name(MANIFEST_MEMBER).is_ok());
        assert!(archive.by_name(LORO_SNAPSHOT_MEMBER).is_ok());
        assert!(archive.by_name(DOCUMENT_JSON_MEMBER).is_ok());
    }

    // `Document` deliberately has no `Debug`/`PartialEq` (it wraps a Loro
    // replica with no useful structural equality), so these assertions
    // match on the `Err` variant alone via `matches!` rather than
    // `assert_eq!` on the whole `Result`.

    #[test]
    fn empty_bytes_are_not_a_vmf() {
        assert!(matches!(unpack(2, &[]), Err(OpenError::NotAVmf)));
    }

    #[test]
    fn plain_text_is_not_a_vmf() {
        let bytes = b"this is just a renamed text file, not a zip".to_vec();
        assert!(matches!(unpack(2, &bytes), Err(OpenError::NotAVmf)));
    }

    #[test]
    fn truncated_zip_is_damaged() {
        let document = Document::new(1);
        let bytes = pack(&document, "0.1.0").expect("pack");
        let truncated = &bytes[..bytes.len() / 2];
        assert!(matches!(unpack(2, truncated), Err(OpenError::Damaged)));
    }

    #[test]
    fn zip_missing_required_members_is_damaged() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer
            .start_file("not-a-real-member.txt", options)
            .expect("start");
        writer.write_all(b"hello").expect("write");
        let bytes = writer.finish().expect("finish").into_inner();
        assert!(matches!(unpack(2, &bytes), Err(OpenError::Damaged)));
    }

    #[test]
    fn future_format_version_is_refused() {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        let manifest = Manifest {
            format_version: CURRENT_FORMAT_VERSION + 1,
            loro_snapshot_version: CURRENT_LORO_SNAPSHOT_VERSION,
            app_version: "99.0.0".to_string(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize");
        write_member(&mut writer, MANIFEST_MEMBER, &manifest_bytes, options).expect("manifest");
        write_member(&mut writer, LORO_SNAPSHOT_MEMBER, b"irrelevant", options)
            .expect("loro member");
        write_member(&mut writer, DOCUMENT_JSON_MEMBER, b"{}", options).expect("json member");
        let bytes = writer.finish().expect("finish").into_inner();

        let expected_found = CURRENT_FORMAT_VERSION + 1;
        let expected_supported = CURRENT_FORMAT_VERSION.max(CURRENT_LORO_SNAPSHOT_VERSION);
        assert!(matches!(
            unpack(2, &bytes),
            Err(OpenError::FormatTooNew { found, supported })
                if found == expected_found && supported == expected_supported
        ));
    }

    /// One-off generator for `tests/fixtures/*.vmf`
    /// (`CLAUDE.md` §5: golden-file tests for every file-format
    /// importer/exporter). Run once with
    /// `cargo test -p vecmanf-document-core generate_golden_fixtures -- --ignored`
    /// after changing the container format, then delete this test again —
    /// the committed fixture bytes are what `container_fixtures.rs` pins,
    /// not this generator.
    #[test]
    #[ignore = "run deliberately to regenerate tests/fixtures/*.vmf, not on every `cargo test`"]
    // One linear sequence of fixture-writing steps, each with its own
    // explanatory comment; splitting it into sub-functions would just
    // move the same line count behind extra indirection for a
    // `#[ignore]`d, one-off generator nobody calls from production code.
    #[allow(clippy::too_many_lines)]
    fn generate_golden_fixtures() {
        use crate::path_model::{AnchorId, NewAnchor};
        use crate::units::{Length, Point};

        let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

        let document = Document::new(1);
        let valid = pack(&document, "0.1.0").expect("pack");
        std::fs::write(fixtures_dir.join("valid.vmf"), &valid).expect("write valid.vmf");

        let truncated = &valid[..valid.len() / 2];
        std::fs::write(fixtures_dir.join("truncated.vmf"), truncated).expect("write truncated.vmf");

        std::fs::write(
            fixtures_dir.join("not_a_vmf.txt"),
            b"this is just a renamed text file, not a zip",
        )
        .expect("write not_a_vmf.txt");

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        let manifest = Manifest {
            format_version: CURRENT_FORMAT_VERSION + 1,
            loro_snapshot_version: CURRENT_LORO_SNAPSHOT_VERSION,
            app_version: "99.0.0".to_string(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize");
        write_member(&mut writer, MANIFEST_MEMBER, &manifest_bytes, options).expect("manifest");
        write_member(&mut writer, LORO_SNAPSHOT_MEMBER, b"irrelevant", options)
            .expect("loro member");
        write_member(&mut writer, DOCUMENT_JSON_MEMBER, b"{}", options).expect("json member");
        let future_version = writer.finish().expect("finish").into_inner();
        std::fs::write(
            fixtures_dir.join("future_format_version.vmf"),
            &future_version,
        )
        .expect("write future_format_version.vmf");

        // `paths_v2.vmf` is deliberately NOT regenerated here, for the
        // same reason as `format_version_1.vmf` below: `primitive-shapes`
        // bumped `CURRENT_FORMAT_VERSION` to 3, so running this build's
        // own `Document`/`pack` would bake a `format_version: 3` manifest
        // onto it, destroying its value as proof that a *genuine*
        // version-2 container (one `path-node-editing` actually wrote,
        // before this slice's primitive schema existed) still opens
        // unchanged under the new build (`specs/primitive-shapes/plan.md`,
        // task 19). The committed fixture is `path-node-editing`'s own,
        // untouched.

        // `primitives_v3.vmf` (`specs/primitive-shapes/plan.md`, task 19):
        // a genuine `format_version = 3` fixture carrying one of each
        // primitive kind — a rounded rectangle, a circle (rx == ry), a
        // plain polygon and a star — plus one ordinary path, proving the
        // two kinds share one tree/z-order.
        let with_primitives = Document::new(1);
        let rect_id = with_primitives.create_rect(crate::primitive_model::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(20.0),
            height: Length::from_mm(10.0),
        });
        with_primitives
            .set_corner_radius(rect_id, Length::from_mm(2.0))
            .expect("set corner radius");
        let _ = with_primitives.create_ellipse(crate::primitive_model::EllipseFrame {
            center: Point::new(50.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let _ = with_primitives.create_polygon(
            crate::primitive_model::StarFrame {
                center: Point::new(0.0, 50.0),
                radius: Length::from_mm(10.0),
                angle: crate::units::Angle::from_radians(0.0),
            },
            crate::primitive_model::PointCount::new(6).expect("valid point count"),
        );
        let _ = with_primitives.create_star(
            crate::primitive_model::StarFrame {
                center: Point::new(50.0, 50.0),
                radius: Length::from_mm(10.0),
                angle: crate::units::Angle::from_radians(0.0),
            },
            crate::primitive_model::PointCount::new(5).expect("valid point count"),
            crate::primitive_model::InnerRatio::new(0.5).expect("valid inner ratio"),
        );
        let _ = with_primitives.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 101), Point::new(-10.0, -10.0)),
                NewAnchor::corner(AnchorId::new(1, 102), Point::new(-10.0, -20.0)),
            ],
            false,
        );
        let primitives_v3 = pack(&with_primitives, "0.1.0").expect("pack");
        std::fs::write(fixtures_dir.join("primitives_v3.vmf"), &primitives_v3)
            .expect("write primitives_v3.vmf");

        // `format_version_1.vmf` is deliberately NOT regenerated here.
        // Synthesizing it from this build's own `Document::new` would bake
        // in a Loro snapshot whose root map already says
        // `format_version: 2` (today's `CURRENT_FORMAT_VERSION`) underneath
        // a `manifest.json` that claims 1 — a fixture that doesn't actually
        // prove a *real* version-1 container still opens, only that a
        // mislabeled version-2 one does (architect review,
        // `specs/path-node-editing/adrs.md`'s PR review). The committed
        // fixture is instead `project-file-foundation`'s own
        // `valid.vmf` (`main`, commit 968b543), copied byte-for-byte: a
        // genuine container written before this slice's path schema
        // existed at all.

        // `malformed_paths.vmf` (architect review, same PR review note):
        // a perfectly valid Loro snapshot, behind a perfectly valid
        // manifest, whose `paths` tree has one node with no `anchors`
        // list at all — the shape `Document::from_loro_snapshot`'s path-
        // tree validation must catch and refuse as `OpenError::Damaged`,
        // not something any public `vecmanf_document_core::Document`
        // method could ever produce by itself.
        let malformed = loro::LoroDoc::new();
        malformed.set_peer_id(1).expect("set peer id");
        // Literal "paths", matching `document::PATHS_TREE` (private to
        // that module; this generator only needs the tree's well-known
        // name, not the constant itself).
        let malformed_tree = malformed.get_tree("paths");
        let malformed_node = malformed_tree
            .create(loro::TreeParentId::Root)
            .expect("create node");
        let malformed_meta = malformed_tree.get_meta(malformed_node).expect("meta");
        malformed_meta
            .insert("closed", false)
            .expect("insert closed");
        malformed.commit();
        let malformed_loro = malformed
            .export(loro::ExportMode::Snapshot)
            .expect("export");
        let malformed_manifest = Manifest {
            format_version: CURRENT_FORMAT_VERSION,
            loro_snapshot_version: CURRENT_LORO_SNAPSHOT_VERSION,
            app_version: "0.1.0".to_string(),
        };
        let malformed_manifest_bytes = serde_json::to_vec(&malformed_manifest).expect("serialize");
        let mut malformed_writer = ZipWriter::new(Cursor::new(Vec::new()));
        write_member(
            &mut malformed_writer,
            MANIFEST_MEMBER,
            &malformed_manifest_bytes,
            options,
        )
        .expect("manifest");
        write_member(
            &mut malformed_writer,
            LORO_SNAPSHOT_MEMBER,
            &malformed_loro,
            options,
        )
        .expect("loro member");
        write_member(&mut malformed_writer, DOCUMENT_JSON_MEMBER, b"{}", options)
            .expect("json member");
        let malformed_bytes = malformed_writer.finish().expect("finish").into_inner();
        std::fs::write(fixtures_dir.join("malformed_paths.vmf"), &malformed_bytes)
            .expect("write malformed_paths.vmf");
    }
}
