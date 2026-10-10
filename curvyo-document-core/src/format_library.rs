//! The document formats library (`specs/0045-document-formats-library/`): the
//! built-in list with the maker's file laid over it, and the edits the panel
//! makes (add, edit and delete a format or group, stars, the Show switches).
//! It holds the state and checks every edit against the whole library; the
//! file text is `format_library_file.rs`'s and the import merge is
//! `format_library_import.rs`'s. It opens no file and reads no clock.

use std::collections::BTreeSet;

use crate::document_presets::{DocumentPreset, PresetError, PresetList, checked_name};
use crate::format_edit::{FormatError, FormatField, FormatReason, FormatSpec, GroupTarget};
use crate::format_library_file::{UserFile, UserGroup, new_list_group, user_preset};
use crate::preset_units::Orientation;

/// The built-in list and the maker's file over it.
#[derive(Debug, Clone)]
pub struct FormatLibrary {
    builtin: PresetList,
    pub(crate) user: UserFile,
    pub(crate) effective: PresetList,
}

impl FormatLibrary {
    /// The library of the shipped built-in file alone.
    ///
    /// # Errors
    /// The error of [`PresetList::shipped`].
    pub fn shipped() -> Result<Self, PresetError> {
        Ok(Self::from_builtin(PresetList::shipped()?))
    }

    /// The library of `builtin` alone.
    #[must_use]
    pub fn from_builtin(builtin: PresetList) -> Self {
        let mut library = Self {
            effective: builtin.clone(),
            builtin,
            user: UserFile::default(),
        };
        library.rebuild();
        library
    }

    /// The library of `builtin` with the user file `user_text` over it
    /// (criterion 3).
    ///
    /// # Errors
    /// A [`PresetError`] naming the id and the rule; no library.
    pub fn load(builtin: PresetList, user_text: &str) -> Result<Self, PresetError> {
        let user = UserFile::parse(user_text, &builtin, true)?;
        let mut library = Self::from_builtin(builtin);
        library.user = user;
        library.rebuild();
        Ok(library)
    }

    /// The merged list: built-in groups with the maker's formats appended,
    /// then the maker's groups, with `enabled` and `favourite` applied.
    #[must_use]
    pub fn list(&self) -> &PresetList {
        &self.effective
    }

    /// The built-in list alone.
    #[must_use]
    pub(crate) fn builtin(&self) -> &PresetList {
        &self.builtin
    }

    /// The text of the user file for the library as it is now (criterion 33).
    ///
    /// # Errors
    /// A [`PresetError`] if the writer fails, which no valid library causes.
    pub fn user_text(&self) -> Result<String, PresetError> {
        self.user.to_text(&self.builtin)
    }

    /// How many formats the maker owns.
    #[must_use]
    pub fn user_format_count(&self) -> usize {
        self.user.presets().count()
    }

    pub(crate) fn rebuild(&mut self) {
        let mut list = self.builtin.clone();
        for group in &mut list.groups {
            if let Some(on) = self.user.enabled.get(&group.id) {
                group.enabled = *on;
            }
        }
        for user in &self.user.groups {
            if let Some(group) = list.groups.iter_mut().find(|g| g.id == user.id && !g.user) {
                group.presets.extend(user.presets.iter().cloned());
            } else {
                let enabled = self.user.enabled.get(&user.id).copied().unwrap_or(true);
                list.groups.push(new_list_group(user, enabled));
            }
        }
        for group in &mut list.groups {
            for preset in &mut group.presets {
                if let Some(favourites) = &self.user.favourites {
                    preset.favourite = favourites.contains(&preset.id);
                } else if preset.user {
                    // A user format is in the quick selection only once the
                    // file has a `favourites` list (criterion 34).
                    preset.favourite = false;
                }
            }
        }
        self.effective = list;
    }

    /// The favourites as the library shows them now, which a first change
    /// turns into the file's own list.
    fn favourites_mut(&mut self) -> &mut BTreeSet<String> {
        if self.user.favourites.is_none() {
            let now = self
                .effective
                .presets()
                .filter(|(_, preset)| preset.favourite)
                .map(|(_, preset)| preset.id.clone())
                .collect();
            self.user.favourites = Some(now);
        }
        self.user.favourites.get_or_insert_with(BTreeSet::new)
    }

    /// Puts a format in or out of the quick selection (criterion 15).
    ///
    /// # Errors
    /// `NotFound` for an unknown id.
    pub fn set_favourite(&mut self, id: &str, on: bool) -> Result<(), FormatError> {
        if self.effective.find(id).is_none() {
            return Err(FormatError::new(FormatField::Name, FormatReason::NotFound));
        }
        let favourites = self.favourites_mut();
        if on {
            favourites.insert(id.to_string());
        } else {
            favourites.remove(id);
        }
        self.rebuild();
        Ok(())
    }

    /// Turns a group on or off (criterion 16).
    ///
    /// # Errors
    /// `NotFound` for an unknown id.
    pub fn set_group_enabled(&mut self, id: &str, on: bool) -> Result<(), FormatError> {
        if !self.effective.groups.iter().any(|g| g.id == id) {
            return Err(FormatError::new(FormatField::Name, FormatReason::NotFound));
        }
        let default = self
            .builtin
            .groups
            .iter()
            .find(|g| g.id == id)
            .is_none_or(|g| g.enabled);
        if on == default {
            self.user.enabled.remove(id);
        } else {
            self.user.enabled.insert(id.to_string(), on);
        }
        self.rebuild();
        Ok(())
    }

    /// Adds a format and returns its id (criteria 20 and 32).
    ///
    /// # Errors
    /// The first rule the format breaks.
    pub fn add_format(&mut self, spec: &FormatSpec) -> Result<String, FormatError> {
        self.check_format(spec, None)?;
        let id = self.unique_preset_id(&spec.name);
        let preset = self.make_preset(&id, spec);
        let group_id = self.group_for(&spec.group);
        self.user_group_mut(&group_id).presets.push(preset);
        if spec.favourite {
            self.favourites_mut().insert(id.clone());
        }
        self.rebuild();
        Ok(id)
    }

    /// Replaces the format `id` (criterion 21): the id and the favourite
    /// state stay, the group may change.
    ///
    /// # Errors
    /// `BuiltIn` for a built-in format, `NotFound`, or the first rule the
    /// format breaks.
    pub fn edit_format(&mut self, id: &str, spec: &FormatSpec) -> Result<(), FormatError> {
        self.check_user_format(id)?;
        self.check_format(spec, Some(id))?;
        let preset = self.make_preset(id, spec);
        let target = self.group_for(&spec.group);
        let in_place = self
            .user
            .groups
            .iter_mut()
            .find(|g| g.id == target)
            .and_then(|group| group.presets.iter_mut().find(|p| p.id == id));
        if let Some(slot) = in_place {
            *slot = preset;
        } else {
            self.remove_user_preset(id);
            self.user_group_mut(&target).presets.push(preset);
        }
        self.rebuild();
        Ok(())
    }

    /// Deletes the maker's format `id` and its star (criterion 22).
    ///
    /// # Errors
    /// `BuiltIn` for a built-in format, `NotFound`.
    pub fn delete_format(&mut self, id: &str) -> Result<(), FormatError> {
        self.check_user_format(id)?;
        self.remove_user_preset(id);
        if let Some(favourites) = &mut self.user.favourites {
            favourites.remove(id);
        }
        self.rebuild();
        Ok(())
    }

    /// Renames the maker's group `id` and sets its orientation (criterion 24).
    ///
    /// # Errors
    /// `BuiltIn` for a built-in group, `NotFound`, or a bad or used name.
    pub fn edit_group(
        &mut self,
        id: &str,
        name: &str,
        orientation: Orientation,
    ) -> Result<(), FormatError> {
        self.check_user_group(id)?;
        self.check_group_name(name, Some(id))?;
        if let (Some(name), Some(group)) = (
            checked_name(name),
            self.user.groups.iter_mut().find(|g| g.id == id),
        ) {
            group.name = name;
            group.default_orientation = orientation;
        }
        self.rebuild();
        Ok(())
    }

    /// Deletes the maker's group `id` with its formats (criterion 22).
    ///
    /// # Errors
    /// `BuiltIn` for a built-in group, `NotFound`.
    pub fn delete_group(&mut self, id: &str) -> Result<(), FormatError> {
        self.check_user_group(id)?;
        let removed: Vec<String> = self
            .user
            .groups
            .iter()
            .filter(|g| g.id == id)
            .flat_map(|g| g.presets.iter().map(|p| p.id.clone()))
            .collect();
        self.user.groups.retain(|g| g.id != id);
        self.user.enabled.remove(id);
        if let Some(favourites) = &mut self.user.favourites {
            for preset in removed {
                favourites.remove(&preset);
            }
        }
        self.rebuild();
        Ok(())
    }

    fn check_user_format(&self, id: &str) -> Result<(), FormatError> {
        match self.effective.find(id) {
            None => Err(FormatError::new(FormatField::Name, FormatReason::NotFound)),
            Some((_, preset)) if !preset.user => {
                Err(FormatError::new(FormatField::Name, FormatReason::BuiltIn))
            }
            Some(_) => Ok(()),
        }
    }

    fn check_user_group(&self, id: &str) -> Result<(), FormatError> {
        match self.effective.groups.iter().find(|g| g.id == id) {
            None => Err(FormatError::new(
                FormatField::GroupName,
                FormatReason::NotFound,
            )),
            Some(group) if !group.user => Err(FormatError::new(
                FormatField::GroupName,
                FormatReason::BuiltIn,
            )),
            Some(_) => Ok(()),
        }
    }

    fn make_preset(&self, id: &str, spec: &FormatSpec) -> DocumentPreset {
        let mut preset = user_preset(
            id.to_string(),
            spec.name.trim().to_string(),
            spec.short,
            spec.long,
            spec.unit,
        );
        // An edit keeps the note of the format it replaces.
        preset.note = self
            .effective
            .find(id)
            .and_then(|(_, existing)| existing.note.clone());
        preset
    }

    /// The id of the group `target` names, creating a new one in the user
    /// file when it is new.
    fn group_for(&mut self, target: &GroupTarget) -> String {
        match target {
            GroupTarget::Existing(id) => id.clone(),
            GroupTarget::New { name, orientation } => {
                let name = name.trim().to_string();
                let id = self.unique_group_id(&name);
                self.user.groups.push(UserGroup {
                    id: id.clone(),
                    name,
                    default_orientation: *orientation,
                    appended: false,
                    presets: Vec::new(),
                });
                id
            }
        }
    }

    /// The user-file entry of the group `id`, added for a built-in group.
    fn user_group_mut(&mut self, id: &str) -> &mut UserGroup {
        if !self.user.groups.iter().any(|g| g.id == id) {
            let (name, orientation, appended) = self
                .effective
                .groups
                .iter()
                .find(|g| g.id == id)
                .map_or((String::new(), Orientation::Portrait, false), |g| {
                    (g.name.clone(), g.default_orientation, !g.user)
                });
            self.user.groups.push(UserGroup {
                id: id.to_string(),
                name,
                default_orientation: orientation,
                appended,
                presets: Vec::new(),
            });
        }
        let position = self.user.groups.iter().position(|g| g.id == id);
        // invariant: the entry was found or pushed above.
        &mut self.user.groups[position.unwrap_or(0)]
    }

    /// Appends `preset` to the group `group_id` of the user file.
    pub(crate) fn push_user_preset(&mut self, group_id: &str, preset: DocumentPreset) {
        self.user_group_mut(group_id).presets.push(preset);
        self.rebuild();
    }

    /// Adds the maker's group `group` to the user file.
    pub(crate) fn push_user_group(&mut self, group: UserGroup) {
        self.user.groups.push(group);
        self.rebuild();
    }

    pub(crate) fn remove_user_preset(&mut self, id: &str) {
        for group in &mut self.user.groups {
            group.presets.retain(|p| p.id != id);
        }
        self.user
            .groups
            .retain(|g| !(g.appended && g.presets.is_empty()));
    }
}
