//! The document formats library as `wasm-bindgen` calls
//! (`specs/0045-document-formats-library/`): the "All formats" list, the forms'
//! prefill, and the edits. A further `impl WasmSession` block; strings,
//! booleans and numbers only (ADR 0001 §5), every method a pass-through to
//! `Session`. The host holds the text of the user file and writes it.

use curvyo_ui_core::{FormPrefill, FormatListView};
use wasm_bindgen::prelude::*;

use crate::session::{FormTexts, FormatEdit};
use crate::wasm_api::WasmSession;

/// The list as parallel arrays: one entry per group, and one per format row
/// with the group it belongs to. Flags are bit sets: a group has `1` on, `2`
/// can toggle, `4` can edit; a row has `1` favourite, `2` can star, `4`
/// selected, `8` can edit.
#[wasm_bindgen]
pub struct FormatListRecord {
    /// "14 formats".
    #[wasm_bindgen(getter_with_clone)]
    pub total_text: String,
    /// Add format and Import exist.
    pub can_add: bool,
    /// Export exists.
    pub can_export: bool,
    /// Why the user file could not be read; empty while it can.
    #[wasm_bindgen(getter_with_clone)]
    pub broken: String,
    /// The id of each group.
    #[wasm_bindgen(getter_with_clone)]
    pub group_ids: Vec<String>,
    /// The name of each group.
    #[wasm_bindgen(getter_with_clone)]
    pub group_names: Vec<String>,
    /// The count text of each group ("7", "Off").
    #[wasm_bindgen(getter_with_clone)]
    pub group_counts: Vec<String>,
    /// The flags of each group.
    #[wasm_bindgen(getter_with_clone)]
    pub group_flags: Vec<u8>,
    /// "Show group Slides".
    #[wasm_bindgen(getter_with_clone)]
    pub group_show_names: Vec<String>,
    /// "Fold group Paper".
    #[wasm_bindgen(getter_with_clone)]
    pub group_fold_names: Vec<String>,
    /// "Edit group Laser".
    #[wasm_bindgen(getter_with_clone)]
    pub group_edit_names: Vec<String>,
    /// "Delete group Laser".
    #[wasm_bindgen(getter_with_clone)]
    pub group_delete_names: Vec<String>,
    /// "Delete group Laser and its 2 formats?".
    #[wasm_bindgen(getter_with_clone)]
    pub group_delete_prompts: Vec<String>,
    /// "portrait" or "landscape".
    #[wasm_bindgen(getter_with_clone)]
    pub group_orientations: Vec<String>,
    /// The group of each row: an index into the group arrays.
    #[wasm_bindgen(getter_with_clone)]
    pub row_group: Vec<u32>,
    /// The id of each row.
    #[wasm_bindgen(getter_with_clone)]
    pub row_ids: Vec<String>,
    /// The name of each row.
    #[wasm_bindgen(getter_with_clone)]
    pub row_names: Vec<String>,
    /// "210 × 297 mm".
    #[wasm_bindgen(getter_with_clone)]
    pub row_sizes: Vec<String>,
    /// "Apply A4, 210 × 297 mm".
    #[wasm_bindgen(getter_with_clone)]
    pub row_apply_names: Vec<String>,
    /// "Quick selection: A4".
    #[wasm_bindgen(getter_with_clone)]
    pub row_star_names: Vec<String>,
    /// The flags of each row.
    #[wasm_bindgen(getter_with_clone)]
    pub row_flags: Vec<u8>,
    /// "Edit Key ring".
    #[wasm_bindgen(getter_with_clone)]
    pub row_edit_names: Vec<String>,
    /// "Delete Key ring".
    #[wasm_bindgen(getter_with_clone)]
    pub row_delete_names: Vec<String>,
    /// "Delete Key ring?".
    #[wasm_bindgen(getter_with_clone)]
    pub row_delete_prompts: Vec<String>,
}

impl FormatListRecord {
    fn new(view: &FormatListView, broken: &str) -> Self {
        let mut record = Self {
            total_text: view.total_text.clone(),
            can_add: view.can_add,
            can_export: view.can_export,
            broken: broken.to_string(),
            group_ids: Vec::new(),
            group_names: Vec::new(),
            group_counts: Vec::new(),
            group_flags: Vec::new(),
            group_show_names: Vec::new(),
            group_fold_names: Vec::new(),
            group_edit_names: Vec::new(),
            group_delete_names: Vec::new(),
            group_delete_prompts: Vec::new(),
            group_orientations: Vec::new(),
            row_group: Vec::new(),
            row_ids: Vec::new(),
            row_names: Vec::new(),
            row_sizes: Vec::new(),
            row_apply_names: Vec::new(),
            row_star_names: Vec::new(),
            row_flags: Vec::new(),
            row_edit_names: Vec::new(),
            row_delete_names: Vec::new(),
            row_delete_prompts: Vec::new(),
        };
        for (index, group) in view.groups.iter().enumerate() {
            record.group_ids.push(group.id.clone());
            record.group_names.push(group.name.clone());
            record.group_counts.push(group.count_text.clone());
            record.group_flags.push(
                u8::from(group.enabled)
                    | u8::from(group.can_toggle) << 1
                    | u8::from(group.can_edit) << 2,
            );
            record.group_show_names.push(group.show_name.clone());
            record.group_fold_names.push(group.fold_name.clone());
            record.group_edit_names.push(group.edit_name.clone());
            record.group_delete_names.push(group.delete_name.clone());
            record
                .group_delete_prompts
                .push(group.delete_prompt.clone());
            record
                .group_orientations
                .push(group.orientation.to_string());
            for row in &group.rows {
                record
                    .row_group
                    .push(u32::try_from(index).unwrap_or(u32::MAX));
                record.row_ids.push(row.id.clone());
                record.row_names.push(row.name.clone());
                record.row_sizes.push(row.size_text.clone());
                record.row_apply_names.push(row.apply_name.clone());
                record.row_star_names.push(row.star_name.clone());
                record.row_flags.push(
                    u8::from(row.favourite)
                        | u8::from(row.can_star) << 1
                        | u8::from(row.selected) << 2
                        | u8::from(row.can_edit) << 3,
                );
                record.row_edit_names.push(row.edit_name.clone());
                record.row_delete_names.push(row.delete_name.clone());
                record.row_delete_prompts.push(row.delete_prompt.clone());
            }
        }
        record
    }
}

/// What a form starts with.
#[wasm_bindgen]
pub struct FormPrefillRecord {
    /// The name.
    #[wasm_bindgen(getter_with_clone)]
    pub name: String,
    /// The width text.
    #[wasm_bindgen(getter_with_clone)]
    pub width: String,
    /// The height text.
    #[wasm_bindgen(getter_with_clone)]
    pub height: String,
    /// The unit symbol.
    #[wasm_bindgen(getter_with_clone)]
    pub unit: String,
    /// The chosen group's id; empty for a new group.
    #[wasm_bindgen(getter_with_clone)]
    pub group_id: String,
    /// "Add to quick selection".
    pub favourite: bool,
}

impl From<FormPrefill> for FormPrefillRecord {
    fn from(prefill: FormPrefill) -> Self {
        Self {
            name: prefill.name,
            width: prefill.width,
            height: prefill.height,
            unit: prefill.unit.to_string(),
            group_id: prefill.group_id,
            favourite: prefill.favourite,
        }
    }
}

/// What an edit did.
#[wasm_bindgen]
pub struct FormatEditRecord {
    /// The edit was applied.
    pub ok: bool,
    /// The text of the user file to write after a successful edit.
    #[wasm_bindgen(getter_with_clone)]
    pub file_text: String,
    /// The notice to show.
    #[wasm_bindgen(getter_with_clone)]
    pub notice: String,
    /// The refused fields in form order: `name`, `width`, `height`,
    /// `group-name`.
    #[wasm_bindgen(getter_with_clone)]
    pub error_fields: Vec<String>,
    /// The chip text of each refused field.
    #[wasm_bindgen(getter_with_clone)]
    pub error_messages: Vec<String>,
}

impl From<FormatEdit> for FormatEditRecord {
    fn from(edit: FormatEdit) -> Self {
        Self {
            ok: edit.ok,
            file_text: edit.file_text,
            notice: edit.notice,
            error_fields: edit
                .errors
                .iter()
                .map(|e| e.field.name().to_string())
                .collect(),
            error_messages: edit.errors.into_iter().map(|e| e.message).collect(),
        }
    }
}

/// The form's typed text as `[name, width, height, unit, group id, new group
/// name, orientation]`.
fn form_texts(fields: &[String], favourite: bool) -> FormTexts {
    let at = |index: usize| fields.get(index).cloned().unwrap_or_default();
    FormTexts {
        name: at(0),
        width: at(1),
        height: at(2),
        unit: at(3),
        group_id: at(4),
        new_group_name: at(5),
        orientation: at(6),
        favourite,
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// Loads the text of the user's formats file over the built-in formats.
    /// Empty when it was read; otherwise the reason it was not, and then the
    /// library runs on the built-ins and refuses every edit.
    pub fn load_formats(&mut self, text: &str) -> String {
        self.session.load_formats(text).err().unwrap_or_default()
    }

    /// Marks the user file as unreadable for a reason the host knows (the file
    /// could not be read at all).
    pub fn mark_formats_broken(&mut self, reason: &str) {
        self.session.mark_formats_broken(reason);
    }

    /// The file was moved aside: edits work again on the built-in formats.
    pub fn formats_file_set_aside(&mut self) {
        self.session.formats_file_set_aside();
    }

    /// The "All formats" list.
    #[must_use]
    pub fn format_list(&self) -> FormatListRecord {
        FormatListRecord::new(
            &self.session.format_list(),
            self.session.formats_broken().unwrap_or_default(),
        )
    }

    /// The form of "Add format" (`editing` empty) or of a format's edit button.
    #[must_use]
    pub fn format_prefill(&self, editing: &str, last_group: &str) -> FormPrefillRecord {
        self.session.format_prefill(editing, last_group).into()
    }

    /// Add in the form. `fields` is `[name, width, height, unit, group id, new
    /// group name, orientation]`.
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen takes the array by value
    pub fn add_format(&mut self, fields: Vec<String>, favourite: bool) -> FormatEditRecord {
        self.session
            .add_format(&form_texts(&fields, favourite))
            .into()
    }

    /// Save in the form of the format `id`.
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen takes the array by value
    pub fn save_format(
        &mut self,
        id: &str,
        fields: Vec<String>,
        favourite: bool,
    ) -> FormatEditRecord {
        self.session
            .save_format(id, &form_texts(&fields, favourite))
            .into()
    }

    /// Delete the format `id`.
    pub fn delete_format(&mut self, id: &str) -> FormatEditRecord {
        self.session.delete_format(id).into()
    }

    /// Save in the form of the user group `id`; `orientation` is `portrait` or
    /// `landscape`.
    pub fn save_format_group(
        &mut self,
        id: &str,
        name: &str,
        orientation: &str,
    ) -> FormatEditRecord {
        self.session.save_format_group(id, name, orientation).into()
    }

    /// Delete the user group `id` with its formats.
    pub fn delete_format_group(&mut self, id: &str) -> FormatEditRecord {
        self.session.delete_format_group(id).into()
    }

    /// A press on a star.
    pub fn set_format_favourite(&mut self, id: &str, on: bool) -> FormatEditRecord {
        self.session.set_format_favourite(id, on).into()
    }

    /// A press on a Show switch.
    pub fn set_format_group_enabled(&mut self, id: &str, on: bool) -> FormatEditRecord {
        self.session.set_format_group_enabled(id, on).into()
    }

    /// Merges the text of an imported file; `file_name` is for the refusal.
    pub fn import_formats(&mut self, text: &str, file_name: &str) -> FormatEditRecord {
        self.session.import_formats(text, file_name).into()
    }

    /// The text of an export; empty without a format of the maker's.
    #[must_use]
    pub fn export_formats(&self) -> String {
        self.session.export_formats().unwrap_or_default()
    }
}
