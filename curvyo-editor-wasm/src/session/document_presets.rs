//! `Session`'s glue for the document size presets
//! (`specs/0030-document-size-presets/`, `adrs.md`): what the Document section
//! shows and the two presses (a preset, an orientation). A press is the typed
//! resize of `0015` with another size, through the same function, so the
//! centre stays fixed, the view follows and an unchanged size writes nothing.
//! The rules are `curvyo_ui_core::document_presets_view`'s.

use curvyo_document_core::Orientation;
use curvyo_ui_core::{PresetsView, orientation_swap, preset_pick_size, presets_view};

use super::Session;
use super::document::SizeOutcome;

impl Session {
    /// What the presets part of the Document section shows for the current
    /// size and display unit.
    #[must_use]
    pub fn document_presets_view(&self) -> PresetsView {
        presets_view(
            self.formats.list(),
            self.document.size(),
            self.display_unit(),
        )
    }

    /// A press on the preset `id` (criteria 11 to 13): the preset's size in the
    /// orientation of the rule, as the typed resize applies it.
    pub fn apply_document_preset(&mut self, id: &str) -> SizeOutcome {
        match preset_pick_size(self.formats.list(), self.document.size(), id) {
            Some(size) => self.resize_document_to(size),
            None => SizeOutcome::Invalid,
        }
    }

    /// A press on an orientation item (criterion 14): width and height swap; the
    /// pressed item and a square document write nothing.
    pub fn set_document_orientation(&mut self, wanted: Orientation) -> SizeOutcome {
        match orientation_swap(self.document.size(), wanted) {
            Some(size) => self.resize_document_to(size),
            None => SizeOutcome::Unchanged,
        }
    }
}

/// [`Session::document_presets_view`] as the flat record the host reads (ADR
/// 0001 §5): parallel lists, one entry per preset button in file order, with
/// the group each belongs to.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPresetsRecord {
    /// The header's subject line: "A4, portrait", "Custom".
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub subject: String,
    /// `"portrait"`, `"landscape"` or `"none"` (a square): the pressed item.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub orientation: String,
    /// The group headings, in file order.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub group_names: Vec<String>,
    /// The group of each button: an index into `group_names`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_group: Vec<u32>,
    /// The id of each button, which a press sends back.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_ids: Vec<String>,
    /// The label of each button.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_names: Vec<String>,
    /// The accessible name of each button ("Paper A4").
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_accessible: Vec<String>,
    /// The tooltip of each button (lines separated by a newline).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_tooltips: Vec<String>,
    /// `1` for the pressed button, else `0`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
    pub preset_pressed: Vec<u8>,
}

impl DocumentPresetsRecord {
    /// The record of `view`.
    #[must_use]
    pub fn new(view: &PresetsView) -> Self {
        let mut record = Self {
            subject: view.subject.clone(),
            orientation: match view.orientation {
                Some(Orientation::Portrait) => "portrait",
                Some(Orientation::Landscape) => "landscape",
                None => "none",
            }
            .to_string(),
            group_names: Vec::new(),
            preset_group: Vec::new(),
            preset_ids: Vec::new(),
            preset_names: Vec::new(),
            preset_accessible: Vec::new(),
            preset_tooltips: Vec::new(),
            preset_pressed: Vec::new(),
        };
        for (index, group) in view.groups.iter().enumerate() {
            record.group_names.push(group.name.clone());
            for entry in &group.entries {
                record
                    .preset_group
                    .push(u32::try_from(index).unwrap_or(u32::MAX));
                record.preset_ids.push(entry.id.clone());
                record.preset_names.push(entry.name.clone());
                record.preset_accessible.push(entry.accessible_name.clone());
                record.preset_tooltips.push(entry.tooltip.clone());
                record.preset_pressed.push(u8::from(entry.pressed));
            }
        }
        record
    }
}
