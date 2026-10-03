//! The document model's root: a Loro-backed replica carrying exactly the
//! fields this slice needs — `format_version` and `size` — as per-field
//! last-writer-wins registers (ADR 0009 §3; ADR 0002 §1, §9; ADR 0004 §2,
//! §3; `specs/project-file-foundation/adrs.md`, "a minimal document root
//! record, and no more").

use loro::{ExportMode, LoroDoc, LoroMap, LoroValue};
use serde::Serialize;

use crate::error::{OpenError, SaveError};
use crate::units::{DocumentSize, Length};

/// The container `format_version` this build writes and the newest it
/// accepts on read (ADR 0004 §9).
pub const CURRENT_FORMAT_VERSION: u32 = 1;

const ROOT_MAP: &str = "root";
const KEY_FORMAT_VERSION: &str = "format_version";
const KEY_WIDTH_MM: &str = "size_width_mm";
const KEY_HEIGHT_MM: &str = "size_height_mm";

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
    /// portrait, 210 × 297 mm).
    ///
    /// Each call gets its own fresh Loro peer id — Loro assigns one
    /// internally at construction — which is what this slice's "fresh peer
    /// id per open session" decision asks for, without this crate touching
    /// any source of randomness itself (`CLAUDE.md` §6: `*-core` crates do
    /// no I/O).
    ///
    /// # Panics
    /// Does not panic in practice: it only inserts known-valid keys into a
    /// freshly created, attached root map, which Loro's API cannot reject.
    #[must_use]
    pub fn new() -> Self {
        let loro = LoroDoc::new();
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
    /// # Errors
    /// Returns [`OpenError::Damaged`] if the bytes are not a valid Loro
    /// snapshot this build can import.
    pub(crate) fn from_loro_snapshot(bytes: &[u8]) -> Result<Self, OpenError> {
        let loro = LoroDoc::new();
        loro.import(bytes).map_err(|_| OpenError::Damaged)?;
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
        let root = self.loro.get_map(ROOT_MAP);
        let width = Self::read_mm(&root, KEY_WIDTH_MM).unwrap_or(210.0);
        let height = Self::read_mm(&root, KEY_HEIGHT_MM).unwrap_or(297.0);
        DocumentSize::new(Length::from_mm(width), Length::from_mm(height))
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
        let view = DocumentJsonView {
            format_version: CURRENT_FORMAT_VERSION,
            size: self.size(),
        };
        serde_json::to_vec_pretty(&view).map_err(|_| SaveError::Encode)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// The shape of the non-authoritative `document.json` export
/// (ADR 0004 §1).
#[derive(Serialize)]
struct DocumentJsonView {
    format_version: u32,
    size: DocumentSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_defaults_to_a4_portrait() {
        let document = Document::new();
        let size = document.size();
        assert!((size.width.as_mm() - 210.0).abs() < f64::EPSILON);
        assert!((size.height.as_mm() - 297.0).abs() < f64::EPSILON);
    }

    #[test]
    fn two_new_documents_get_different_peer_ids() {
        let a = Document::new();
        let b = Document::new();
        assert_ne!(a.loro.peer_id(), b.loro.peer_id());
    }

    #[test]
    fn loro_snapshot_round_trips_the_size() {
        let original = Document::new();
        let snapshot = original.export_loro_snapshot().expect("export");
        let reopened = Document::from_loro_snapshot(&snapshot).expect("import");
        assert_eq!(original.size(), reopened.size());
    }

    #[test]
    fn json_export_matches_the_live_size() {
        let document = Document::new();
        let json = document.export_json().expect("export");
        let value: serde_json::Value = serde_json::from_slice(&json).expect("parse");
        assert_eq!(value["format_version"], CURRENT_FORMAT_VERSION);
        assert!((value["size"]["width"].as_f64().unwrap() - 210.0).abs() < f64::EPSILON);
    }
}
