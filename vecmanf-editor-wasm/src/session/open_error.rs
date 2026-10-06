//! The one-sentence user-facing message for each way opening a `.vmf` can
//! fail. Split out of `session/mod.rs` (`docs/technical-debt.md`,
//! "`Session` is one module past the size limit").

#[cfg(any(test, target_arch = "wasm32"))]
use vecmanf_document_core::OpenError;

/// Returns the one-sentence message the frontend's `ErrorDialog` shows
/// for `error` — moved here from `vecmanf-app`'s native `open_error.rs`
/// (`specs/0001-project-file-foundation/specification.md`, "Error handling —
/// invalid/corrupt file") now that [`Session::open`] (and the
/// `Document::open` it wraps) only ever runs inside this wasm session,
/// never natively (`specs/0002-path-node-editing/adrs.md`'s PR review: "the
/// host does byte I/O only"). Plain Rust, not `wasm_api`'s `wasm32`-only
/// shell, so it stays exercised by ordinary `cargo test` — its only
/// caller is `wasm_api::WasmSession::open`, which is itself `wasm32`-
/// only, so this function is `cfg`-gated the same way plus `test`
/// (otherwise a host `cargo build`/`clippy` sees it as genuinely unused
/// dead code, since its one caller does not exist in that build).
#[cfg(any(test, target_arch = "wasm32"))]
#[must_use]
pub const fn map_open_error(error: &OpenError) -> &'static str {
    match error {
        OpenError::NotAVmf => "This file isn't a vecmanf project (.vmf) file.",
        OpenError::Damaged => "This file is damaged and can't be read.",
        OpenError::FormatTooNew { .. } => {
            "This file was saved by a newer version of vecmanf. Update the app to open it."
        }
    }
}

#[cfg(test)]
mod map_open_error_tests {
    use super::map_open_error;
    use vecmanf_document_core::OpenError;

    #[test]
    fn not_a_vmf_names_the_specific_cause() {
        assert_eq!(
            map_open_error(&OpenError::NotAVmf),
            "This file isn't a vecmanf project (.vmf) file."
        );
    }

    #[test]
    fn damaged_names_the_specific_cause() {
        assert_eq!(
            map_open_error(&OpenError::Damaged),
            "This file is damaged and can't be read."
        );
    }

    #[test]
    fn format_too_new_names_the_specific_cause() {
        let error = OpenError::FormatTooNew {
            found: 2,
            supported: 1,
        };
        assert_eq!(
            map_open_error(&error),
            "This file was saved by a newer version of vecmanf. Update the app to open it."
        );
    }
}
