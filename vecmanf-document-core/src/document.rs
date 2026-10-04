//! The document model's root: a Loro-backed replica carrying exactly the
//! fields this slice needs — `format_version` and `size` — as per-field
//! last-writer-wins registers (ADR 0009 §3; ADR 0002 §1, §9; ADR 0004 §2,
//! §3; `specs/project-file-foundation/adrs.md`, "a minimal document root
//! record, and no more").

use loro::{CommitOptions, ExportMode, LoroDoc, LoroMap, LoroValue};
use serde::Serialize;

use crate::error::{OpenError, SaveError};
use crate::units::{DocumentSize, Length};

/// The container `format_version` this build writes and the newest it
/// accepts on read (ADR 0004 §9).
///
/// Bumped to 2 in `path-node-editing`: a version-1 reader would silently
/// ignore every path node in a version-2 file, which is exactly the silent
/// geometry loss ADR 0004 §9 exists to prevent
/// (`specs/path-node-editing/adrs.md`, "`format_version` goes to 2").
/// Migration from version 1 is empty by construction — a version-1
/// document has no path nodes to migrate.
pub const CURRENT_FORMAT_VERSION: u32 = 2;

const ROOT_MAP: &str = "root";
const KEY_FORMAT_VERSION: &str = "format_version";
const KEY_WIDTH_MM: &str = "size_width_mm";
const KEY_HEIGHT_MM: &str = "size_height_mm";

/// The top-level Loro tree container holding every path object (ADR 0002
/// §5: sibling order among tree nodes is z-order). `vecmanf-document-core`
/// is the only module that names this key; everything else goes through
/// [`Document`]'s methods — see [`crate::paths`].
pub(crate) const PATHS_TREE: &str = "paths";

/// An open vecmanf document.
///
/// Its Loro CRDT backing is an implementation detail and never appears in
/// this type's public API (ADR 0004 §3): callers only ever see
/// [`DocumentSize`] and this crate's own error types.
pub struct Document {
    loro: LoroDoc,
}

impl Document {
    /// Creates a new, empty document at the default page size (A4
    /// portrait, 210 × 297 mm), bound to the given Loro peer id.
    ///
    /// The peer id is a parameter rather than something this crate mints
    /// itself: `LoroDoc::new()` would otherwise draw one from `getrandom`,
    /// and a `*-core` crate reaching an entropy source is exactly what
    /// `CLAUDE.md` §6 forbids ("no filesystem, network, clock, threads or
    /// UI. Callers pass everything in.") — the same reason
    /// `vecmanf-library-core` takes a record's UUID as a parameter instead
    /// of generating it (ADR 0011 §6). `vecmanf-app` mints a fresh id per
    /// open session and passes it in here
    /// (`specs/project-file-foundation/adrs.md`, amended 2026-10-03).
    ///
    /// # Panics
    /// Does not panic in practice: it only inserts known-valid keys into a
    /// freshly created, attached root map, and sets the peer id before any
    /// operation has been recorded — both of which Loro's API cannot
    /// reject.
    #[must_use]
    pub fn new(peer_id: u64) -> Self {
        let loro = LoroDoc::new();
        // invariant: setting the peer id on a freshly created doc with no
        // recorded operations yet cannot fail.
        #[allow(clippy::unwrap_used)]
        loro.set_peer_id(peer_id).unwrap();
        let root = loro.get_map(ROOT_MAP);
        let size = DocumentSize::default();
        // invariant: inserting known-valid keys into a freshly created,
        // attached root map cannot fail.
        #[allow(clippy::unwrap_used)]
        {
            root.insert(KEY_FORMAT_VERSION, CURRENT_FORMAT_VERSION)
                .unwrap();
            root.insert(KEY_WIDTH_MM, size.width.as_mm()).unwrap();
            root.insert(KEY_HEIGHT_MM, size.height.as_mm()).unwrap();
        }
        Self { loro }
    }

    /// Rebuilds a document from a previously exported Loro snapshot — the
    /// container's `document.loro` member, the sole source of truth on a
    /// normal open (ADR 0004 §1).
    ///
    /// `peer_id` is a parameter for the same reason [`Document::new`]'s is:
    /// `LoroDoc::new()` would otherwise draw one from `getrandom` itself,
    /// which this `*-core` crate must not do (`CLAUDE.md` §6). Reopening a
    /// file is a fresh session like any other, so it gets a fresh id from
    /// the caller the same way a brand-new document does.
    ///
    /// # Errors
    /// Returns [`OpenError::Damaged`] if the bytes are not a valid Loro
    /// snapshot this build can import, or if they import cleanly but the
    /// `paths` tree inside them does not match the shape
    /// [`crate::path_codec`]'s writers always produce — a damaged-but-
    /// still-unzippable file (project-file-foundation's acceptance
    /// criterion 7: "not a crash") rather than a panic the first time some
    /// other method reads that path.
    ///
    /// # Panics
    /// Does not panic in practice: setting the peer id on a freshly
    /// created doc, before `import` records anything, cannot fail.
    pub(crate) fn from_loro_snapshot(peer_id: u64, bytes: &[u8]) -> Result<Self, OpenError> {
        let loro = LoroDoc::new();
        // invariant: setting the peer id on a freshly created doc with no
        // recorded operations yet cannot fail.
        #[allow(clippy::unwrap_used)]
        loro.set_peer_id(peer_id).unwrap();
        loro.import(bytes).map_err(|_| OpenError::Damaged)?;
        if !crate::path_codec::validate_path_tree(&loro, PATHS_TREE) {
            return Err(OpenError::Damaged);
        }
        Ok(Self { loro })
    }

    /// The document's page size in millimetres.
    ///
    /// Falls back to the A4 default if the root map is missing a field
    /// this crate always writes itself. This is a defensive read, not a
    /// panic path: a `.vmf` written by a future, compatible version of
    /// this crate could add fields this build does not know about without
    /// that file being "damaged".
    #[must_use]
    pub fn size(&self) -> DocumentSize {
        let default = DocumentSize::default();
        let root = self.loro.get_map(ROOT_MAP);
        let width = Self::read_mm(&root, KEY_WIDTH_MM).unwrap_or(default.width.as_mm());
        let height = Self::read_mm(&root, KEY_HEIGHT_MM).unwrap_or(default.height.as_mm());
        DocumentSize::new(Length::from_mm(width), Length::from_mm(height))
    }

    /// This document's underlying Loro replica, for [`crate::paths`]'s
    /// methods only — never exposed outside this crate (ADR 0004 §3).
    pub(crate) const fn loro(&self) -> &LoroDoc {
        &self.loro
    }

    /// Commits the pending auto-commit transaction with `label` as its
    /// (persisted) commit message, so each editing method in
    /// [`crate::paths`] ends its own transaction instead of letting every
    /// edit since the last explicit commit pile into one (ADR 0002 §9;
    /// `specs/path-node-editing/adrs.md`'s PR review: "a pen session is
    /// one commit" only holds if *every* mutating method commits its own
    /// work, undo-readiness for a future undo/redo feature depends on
    /// one commit per interaction).
    pub(crate) fn commit_with_label(&self, label: &str) {
        self.loro
            .commit_with(CommitOptions::new().commit_msg(label));
    }

    fn read_mm(root: &LoroMap, key: &str) -> Option<f64> {
        match root.get(key)?.get_deep_value() {
            LoroValue::Double(value) => Some(value),
            _ => None,
        }
    }

    /// Exports this document's Loro CRDT snapshot — the authoritative
    /// container member, `document.loro` (ADR 0004 §1).
    ///
    /// # Errors
    /// Returns [`SaveError::Encode`] if the snapshot cannot be encoded.
    pub fn export_loro_snapshot(&self) -> Result<Vec<u8>, SaveError> {
        self.loro
            .export(ExportMode::Snapshot)
            .map_err(|_| SaveError::Encode)
    }

    /// Exports a plain, non-authoritative JSON view of the current state —
    /// the container's `document.json` member (ADR 0004 §1) — read from
    /// this one immutable snapshot of the document rather than live state
    /// (ADR 0009 §4).
    ///
    /// # Errors
    /// Returns [`SaveError::Encode`] if the view cannot be serialized.
    pub fn export_json(&self) -> Result<Vec<u8>, SaveError> {
        let paths = self
            .path_ids()
            .into_iter()
            .filter_map(|id| self.path(id))
            .collect();
        let view = DocumentJsonView {
            format_version: CURRENT_FORMAT_VERSION,
            size: self.size(),
            paths,
        };
        serde_json::to_vec_pretty(&view).map_err(|_| SaveError::Encode)
    }
}

// Deliberately no `impl Default for Document`: there is no safe default
// peer id to mint without a caller-supplied source of randomness, which is
// exactly what `Document::new`'s `peer_id` parameter exists to avoid this
// crate doing itself.

/// The shape of the non-authoritative `document.json` export
/// (ADR 0004 §1).
#[derive(Serialize)]
struct DocumentJsonView {
    format_version: u32,
    size: DocumentSize,
    paths: Vec<crate::path_model::PathSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_defaults_to_a4_portrait() {
        let document = Document::new(1);
        let size = document.size();
        assert!((size.width.as_mm() - 210.0).abs() < f64::EPSILON);
        assert!((size.height.as_mm() - 297.0).abs() < f64::EPSILON);
    }

    #[test]
    fn new_document_uses_the_given_peer_id() {
        let document = Document::new(0x1234_5678_9abc_def0);
        assert_eq!(document.loro.peer_id(), 0x1234_5678_9abc_def0);
    }

    #[test]
    fn loro_snapshot_round_trips_the_size() {
        let original = Document::new(1);
        let snapshot = original.export_loro_snapshot().expect("export");
        let reopened = Document::from_loro_snapshot(2, &snapshot).expect("import");
        assert_eq!(original.size(), reopened.size());
    }

    #[test]
    fn from_loro_snapshot_uses_the_given_peer_id() {
        let original = Document::new(1);
        let snapshot = original.export_loro_snapshot().expect("export");
        let reopened =
            Document::from_loro_snapshot(0x1234_5678_9abc_def0, &snapshot).expect("import");
        assert_eq!(reopened.loro.peer_id(), 0x1234_5678_9abc_def0);
    }

    #[test]
    fn json_export_matches_the_live_size() {
        let document = Document::new(1);
        let json = document.export_json().expect("export");
        let value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
        assert_eq!(value["format_version"], CURRENT_FORMAT_VERSION);
        assert!((value["size"]["width"].as_f64().unwrap() - 210.0).abs() < f64::EPSILON);
    }
}
