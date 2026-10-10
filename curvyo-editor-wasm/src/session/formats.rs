//! `Session`'s glue for the document formats library
//! (`specs/0045-document-formats-library/`, `adrs.md` decision 6): the library
//! the Document tab reads, and the edits its list and forms make. Every edit
//! returns the new text of the user file, which the host writes; core and this
//! glue never touch a file. While the user file cannot be read the library runs
//! on the built-in formats and refuses every edit (criterion 6).

use curvyo_document_core::{
    FormatLibrary, FormatSpec, Orientation, PresetList, PresetUnit, describe_error,
};
use curvyo_ui_core::{
    FieldError, FormDraft, FormPrefill, FormatListView, add_prefill, added_notice,
    check_format_form, edit_prefill, format_list_view, import_notice, import_refusal,
    refusal_message, saved_notice,
};

use super::Session;

/// What an edit of the library did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatEdit {
    /// The edit was applied.
    pub ok: bool,
    /// After a successful edit: the text of the user file to write.
    pub file_text: String,
    /// The notice to show under the button that was pressed.
    pub notice: String,
    /// Why the edit was refused, field by field, in form order.
    pub errors: Vec<FieldError>,
}

/// The typed text of the add and edit form, as the host sends it.
#[derive(Debug, Clone)]
pub struct FormTexts {
    /// The name field.
    pub name: String,
    /// The width field.
    pub width: String,
    /// The height field.
    pub height: String,
    /// The unit symbol (`mm`, `cm`, `in`, `px`).
    pub unit: String,
    /// The chosen group's id, or empty for a new group.
    pub group_id: String,
    /// The new group's name.
    pub new_group_name: String,
    /// `portrait` or `landscape`.
    pub orientation: String,
    /// "Add to quick selection".
    pub favourite: bool,
}

fn orientation_of(word: &str) -> Orientation {
    if word == "landscape" {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    }
}

impl Session {
    /// Loads the user file `text` over the built-in formats. On a file that
    /// cannot be read the library keeps the built-in formats alone, every edit
    /// is refused, and the reason is returned (criterion 6).
    ///
    /// # Errors
    /// The reason, "made by a newer version" and the like.
    pub fn load_formats(&mut self, text: &str) -> Result<(), String> {
        let builtin = Self::formats_builtin();
        match FormatLibrary::load(builtin.clone(), text) {
            Ok(library) => {
                self.formats = library;
                self.formats_broken = None;
                Ok(())
            }
            Err(error) => {
                let reason = describe_error(&error);
                self.formats = FormatLibrary::from_builtin(builtin);
                self.formats_broken = Some(reason.clone());
                Err(reason)
            }
        }
    }

    /// The host could not read the user file at all (not valid text, say): the
    /// library runs on the built-in formats and refuses every edit, as for a
    /// file that fails to load (criterion 6).
    pub fn mark_formats_broken(&mut self, reason: &str) {
        self.formats = FormatLibrary::from_builtin(Self::formats_builtin());
        self.formats_broken = Some(reason.to_string());
    }

    fn formats_builtin() -> PresetList {
        PresetList::shipped().unwrap_or_default()
    }

    /// Why the user file could not be read, while it cannot.
    #[must_use]
    pub fn formats_broken(&self) -> Option<&str> {
        self.formats_broken.as_deref()
    }

    /// The file was moved aside: the library runs on the built-in formats and
    /// edits work again (criterion 6).
    pub fn formats_file_set_aside(&mut self) {
        self.formats = FormatLibrary::from_builtin(Self::formats_builtin());
        self.formats_broken = None;
    }

    /// The "All formats" list for the current size and display unit.
    #[must_use]
    pub fn format_list(&self) -> FormatListView {
        format_list_view(
            self.formats.list(),
            self.document.size(),
            self.display_unit(),
            self.formats_broken.is_none(),
        )
    }

    /// The form a press on "Add format" (`editing` empty) or on a format's
    /// edit button opens (criteria 18 and 21). `last_group` is the group last
    /// used.
    #[must_use]
    pub fn format_prefill(&self, editing: &str, last_group: &str) -> FormPrefill {
        if let Some((group, preset)) = self.formats.list().find(editing) {
            return edit_prefill(group, preset);
        }
        add_prefill(
            &self.formats,
            self.document.size(),
            self.display_unit(),
            last_group,
        )
    }

    fn refuse_while_broken(&self) -> Option<FormatEdit> {
        self.formats_broken.as_ref().map(|_| FormatEdit::default())
    }

    fn applied(&self, notice: String) -> FormatEdit {
        match self.formats.user_text() {
            Ok(file_text) => FormatEdit {
                ok: true,
                file_text,
                notice,
                errors: Vec::new(),
            },
            Err(error) => FormatEdit {
                notice: describe_error(&error),
                ..FormatEdit::default()
            },
        }
    }

    fn refused(error: &curvyo_document_core::FormatError) -> FormatEdit {
        FormatEdit {
            errors: vec![refusal_message(error)],
            ..FormatEdit::default()
        }
    }

    fn check_form(
        &self,
        texts: &FormTexts,
        editing: Option<&str>,
    ) -> Result<FormatSpec, FormatEdit> {
        let unit = PresetUnit::from_symbol(&texts.unit).unwrap_or(PresetUnit::Mm);
        let draft = FormDraft {
            name: &texts.name,
            width: &texts.width,
            height: &texts.height,
            unit,
            group_id: &texts.group_id,
            new_group_name: &texts.new_group_name,
            new_group_orientation: orientation_of(&texts.orientation),
            favourite: texts.favourite,
        };
        check_format_form(&self.formats, &draft, editing).map_err(|errors| FormatEdit {
            errors,
            ..FormatEdit::default()
        })
    }

    /// A press on Add in the form (criterion 20). The document is not changed.
    pub fn add_format(&mut self, texts: &FormTexts) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        let spec = match self.check_form(texts, None) {
            Ok(spec) => spec,
            Err(edit) => return edit,
        };
        match self.formats.add_format(&spec) {
            Ok(_) => self.applied(added_notice(&spec.name)),
            Err(error) => Self::refused(&error),
        }
    }

    /// A press on Save in the form of the format `id` (criterion 21).
    pub fn save_format(&mut self, id: &str, texts: &FormTexts) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        let spec = match self.check_form(texts, Some(id)) {
            Ok(spec) => spec,
            Err(edit) => return edit,
        };
        match self.formats.edit_format(id, &spec) {
            Ok(()) => self.applied(saved_notice(&spec.name)),
            Err(error) => Self::refused(&error),
        }
    }

    /// A press on Delete of the format `id` (criterion 22).
    pub fn delete_format(&mut self, id: &str) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self.formats.delete_format(id) {
            Ok(()) => self.applied(String::new()),
            Err(error) => Self::refused(&error),
        }
    }

    /// A press on Save in the form of the user group `id` (criterion 24).
    pub fn save_format_group(&mut self, id: &str, name: &str, orientation: &str) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self
            .formats
            .edit_group(id, name, orientation_of(orientation))
        {
            Ok(()) => self.applied(saved_notice(name)),
            Err(mut error) => {
                // The group form has one field.
                error.field = curvyo_document_core::FormatField::GroupName;
                Self::refused(&error)
            }
        }
    }

    /// A press on Delete of the user group `id` (criterion 22).
    pub fn delete_format_group(&mut self, id: &str) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self.formats.delete_group(id) {
            Ok(()) => self.applied(String::new()),
            Err(error) => Self::refused(&error),
        }
    }

    /// A press on a star (criterion 15).
    pub fn set_format_favourite(&mut self, id: &str, on: bool) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self.formats.set_favourite(id, on) {
            Ok(()) => self.applied(String::new()),
            Err(error) => Self::refused(&error),
        }
    }

    /// A press on a Show switch (criterion 16).
    pub fn set_format_group_enabled(&mut self, id: &str, on: bool) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self.formats.set_group_enabled(id, on) {
            Ok(()) => self.applied(String::new()),
            Err(error) => Self::refused(&error),
        }
    }

    /// The file the host picked for Import (criterion 27). A file that fails is
    /// refused whole with the reason in the notice.
    pub fn import_formats(&mut self, text: &str, file_name: &str) -> FormatEdit {
        if let Some(refusal) = self.refuse_while_broken() {
            return refusal;
        }
        match self.formats.import(text) {
            Ok(report) => self.applied(import_notice(&report)),
            Err(error) => FormatEdit {
                notice: import_refusal(file_name, &describe_error(&error)),
                ..FormatEdit::default()
            },
        }
    }

    /// The text for Export (criterion 26); `None` without a format of the
    /// maker's, and while the file is broken.
    #[must_use]
    pub fn export_formats(&self) -> Option<String> {
        if self.formats_broken.is_some() {
            return None;
        }
        self.formats.export_text().ok().flatten()
    }
}
