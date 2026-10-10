//! Sharing the maker's formats (`specs/0045-document-formats-library/`
//! criteria 26 and 27): the export text of a library and the merge of an
//! imported file into it, as pure functions of the library and the text
//! (ADR 0004 §8).

use std::collections::{BTreeMap, BTreeSet};

use crate::document_presets::{DocumentPreset, PresetError};
use crate::format_library::FormatLibrary;
use crate::format_library_file::{UserFile, UserGroup};

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportReport {
    /// Formats added to the library.
    pub added: usize,
    /// Groups that received at least one added format.
    pub groups: usize,
    /// Formats left out because the library had them, or one of their size.
    pub skipped: usize,
}

impl FormatLibrary {
    /// The text of an export (criterion 26): the maker's groups and formats and
    /// the favourites among them, no `enabled` choices and nothing built-in.
    /// `None` while the maker owns no format.
    ///
    /// # Errors
    /// A [`PresetError`] if the writer fails, which no valid library causes.
    pub fn export_text(&self) -> Result<Option<String>, PresetError> {
        if self.user_format_count() == 0 {
            return Ok(None);
        }
        let favourites: BTreeSet<String> = self
            .user
            .presets()
            .filter(|preset| {
                self.list()
                    .find(&preset.id)
                    .is_some_and(|(_, shown)| shown.favourite)
            })
            .map(|preset| preset.id.clone())
            .collect();
        let export = UserFile {
            favourites: Some(favourites),
            enabled: BTreeMap::new(),
            groups: self
                .user
                .groups
                .iter()
                .filter(|group| !group.presets.is_empty() || !group.appended)
                .cloned()
                .collect(),
        };
        export.to_text(self.builtin()).map(Some)
    }

    /// Merges the file `text` into the library (criterion 27). A file that
    /// fails as a user file, or that would pass a limit, changes nothing.
    ///
    /// # Errors
    /// A [`PresetError`] naming the format and the rule.
    pub fn import(&mut self, text: &str) -> Result<ImportReport, PresetError> {
        let incoming = UserFile::parse(text, self.builtin(), false)?;
        let mut merged = self.clone();
        let mut report = ImportReport {
            added: 0,
            groups: 0,
            skipped: 0,
        };
        let mut new_ids: BTreeMap<String, String> = BTreeMap::new();
        for group in &incoming.groups {
            let mut received = false;
            for preset in &group.presets {
                match merged.import_format(group, preset) {
                    Merge::Added(id) => {
                        new_ids.insert(preset.id.clone(), id);
                        report.added += 1;
                        received = true;
                    }
                    Merge::Skipped(Some(id)) => {
                        new_ids.insert(preset.id.clone(), id);
                        report.skipped += 1;
                    }
                    Merge::Skipped(None) => report.skipped += 1,
                }
            }
            report.groups += usize::from(received);
        }
        if let Some(favourites) = &incoming.favourites {
            let wanted: Vec<String> = favourites
                .iter()
                .filter_map(|id| new_ids.get(id).cloned())
                .collect();
            for id in wanted {
                // The id exists: the merge just added it or found it.
                merged.set_favourite(&id, true).ok();
            }
        }
        merged.user.check_limits()?;
        *self = merged;
        Ok(report)
    }

    /// Merges one imported format: skipped when the library has it (same id
    /// and content) or has a format of its size, otherwise added to the group
    /// of its id or name (or a new group) under its own id, or a new one when
    /// that is taken.
    fn import_format(&mut self, group: &UserGroup, preset: &DocumentPreset) -> Merge {
        if let Some((_, known)) = self.list().find(&preset.id)
            && same_content(known, preset)
        {
            return Merge::Skipped(Some(known.id.clone()));
        }
        let (short, long) = (preset.short_side.as_mm(), preset.long_side.as_mm());
        if self.list().with_sides(short, long, None).is_some() {
            return Merge::Skipped(None);
        }
        let target = self.group_for_import(group);
        let id = if self.list().find(&preset.id).is_some() {
            self.unique_preset_id(&preset.name)
        } else {
            preset.id.clone()
        };
        let mut added = preset.clone();
        added.id.clone_from(&id);
        self.push_user_preset(&target, added);
        Merge::Added(id)
    }

    /// The group an imported group joins: the one with its id, else the one
    /// with its name ignoring case, else a new group.
    fn group_for_import(&mut self, group: &UserGroup) -> String {
        let name = group.name.to_lowercase();
        let existing = self
            .list()
            .groups
            .iter()
            .find(|g| g.id == group.id)
            .or_else(|| {
                self.list()
                    .groups
                    .iter()
                    .find(|g| g.name.to_lowercase() == name)
            })
            .map(|g| g.id.clone());
        existing.unwrap_or_else(|| {
            let id = self.unique_group_id(&group.name);
            self.push_user_group(UserGroup {
                id: id.clone(),
                name: group.name.clone(),
                default_orientation: group.default_orientation,
                appended: false,
                presets: Vec::new(),
            });
            id
        })
    }
}

enum Merge {
    /// Added under this id.
    Added(String),
    /// Left out; carries the id of the identical format the library has.
    Skipped(Option<String>),
}

fn same_content(known: &DocumentPreset, imported: &DocumentPreset) -> bool {
    known.name == imported.name
        && known.note == imported.note
        && known.authored == imported.authored
}
