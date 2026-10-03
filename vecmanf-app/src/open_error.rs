//! Maps [`vecmanf_document_core::OpenError`] to the exact user-facing
//! sentences named in `specs/project-file-foundation/specification.md`
//! ("Error handling — invalid/corrupt file"). The core crate names *why*;
//! this host decides *how to say it*
//! (`specs/project-file-foundation/adrs.md`, "open-refusal reasons are a
//! typed error in `vecmanf-document-core`").

use vecmanf_document_core::OpenError;

/// Returns the one-sentence message the `AlertDialog` shows for `error`.
pub fn map_open_error(error: &OpenError) -> String {
    match error {
        OpenError::NotAVmf => "This file isn't a vecmanf project (.vmf) file.".to_string(),
        OpenError::Damaged => "This file is damaged and can't be read.".to_string(),
        OpenError::FormatTooNew { .. } => {
            "This file was saved by a newer version of vecmanf. Update the app to open it."
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
