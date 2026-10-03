//! The typed refusal reasons for opening a `.vmf` project
//! (`specs/project-file-foundation/adrs.md`, "open-refusal reasons are a
//! typed error in `vecmanf-document-core`") and the (currently narrow)
//! failure modes for building one.

/// Why a byte buffer could not be opened as a `.vmf` project.
///
/// This is a pure function of the bytes: given the same input it always
/// returns the same variant, so it is testable with fixture files and no
/// UI (acceptance criterion 7). The host maps each variant to one of the
/// three named sentences in `specs/project-file-foundation/specification.md`
/// ("Error handling — invalid/corrupt file"); this crate names *why*, the
/// host and frontend decide *how to say it*.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OpenError {
    /// The bytes are not a zip archive at all — for example a renamed
    /// empty text file. Detected before any zip parsing is attempted, by
    /// checking for a zip local-file-header or end-of-central-directory
    /// signature.
    #[error("not a zip archive")]
    NotAVmf,
    /// The bytes look like a zip archive but are truncated or corrupt, or
    /// a required container member (`manifest.json`, `document.loro`) is
    /// missing or unparsable.
    #[error("zip archive is truncated, corrupt, or missing a required member")]
    Damaged,
    /// The container's `manifest.json` declares a `format_version` or
    /// `loro_snapshot_version` newer than this build supports
    /// (ADR 0004 §9).
    #[error("project format version {found} is newer than the {supported} this build supports")]
    FormatTooNew {
        /// The version found in the file's `manifest.json`.
        found: u32,
        /// The newest version this build knows how to read.
        supported: u32,
    },
}

/// Why building a `.vmf` container from an in-memory [`crate::Document`]
/// failed.
///
/// Every variant here indicates a defect in this crate or its dependencies
/// rather than anything the caller did wrong: a document built through
/// [`crate::Document::new`] or [`crate::unpack`] always has the fields
/// this crate itself reads back when saving.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SaveError {
    /// The Loro CRDT snapshot could not be encoded.
    #[error("failed to encode the document snapshot")]
    Encode,
    /// The zip container could not be assembled.
    #[error("failed to build the project container")]
    Container,
}
