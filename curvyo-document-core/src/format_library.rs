//! The document formats library (`specs/0045-document-formats-library/`): the
//! built-in list with the maker's file laid over it, and the edits the panel
//! makes (add, edit and delete a format or group, stars, the Show switches).
//! It holds the state and checks every edit against the whole library; the
//! file text is `format_library_file.rs`'s and the import merge is
//! `format_library_import.rs`'s. It opens no file and reads no clock.

use std::collections::BTreeSet;

use crate::document_presets::{
    DocumentPreset, Orientation, PresetError, PresetList, PresetUnit, checked_name,
};
use crate::format_library_file::{
    MAX_USER_FORMATS, MAX_USER_GROUPS, UserFile, UserGroup, new_list_group, sides_in_range,
    user_preset,
};

/// The field of the form an error belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatField {
    /// The format's name.
    Name,
    /// The width (the first typed size).
    Width,
    /// The height (the second typed size).
    Height,
    /// The new group's name.
    GroupName,
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatReason {
    /// The name is empty or longer than 24 characters.
    #[error("the name must be 1 to 24 characters")]
    BadName,
    /// A side is not a number from 1 to 100 000 mm.
    #[error("a side must be from 1 to 100000 mm")]
    BadSize,
    /// The library already has a format of this size; carries its name.
    #[error("the same size as {0}")]
    SameSizeAs(String),
    /// The group already has a format of this name; carries the name.
    #[error("this group already has {0}")]
    NameInGroup(String),
    /// A group of this name exists.
    #[error("a group of this name exists")]
    GroupNameTaken,
    /// The limit of 200 formats is reached.
    #[error("the list is full")]
    LimitFormats,
    /// The limit of 20 groups is reached.
    #[error("too many groups")]
    LimitGroups,
    /// The group or format does not exist.
    #[error("not found")]
    NotFound,
    /// A built-in group or format cannot be changed or deleted.
    #[error("built-in")]
    BuiltIn,
}

/// A refused edit, with the field it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct FormatError {
    /// The field of the form to mark.
    pub field: FormatField,
    /// The rule it breaks.
    pub reason: FormatReason,
}

impl FormatError {
    fn new(field: FormatField, reason: FormatReason) -> Self {
        Self { field, reason }
    }
}

/// The group a format goes into.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupTarget {
    /// A group that exists, built-in or the maker's.
    Existing(String),
    /// A new group of the maker's.
    New {
        /// Its name (1 to 24 characters, unique ignoring case).
        name: String,
        /// The orientation a pick starts with.
        orientation: Orientation,
    },
}

/// A format as the form describes it. The sides are in `unit`, the shorter
/// first.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatSpec {
    /// The name.
    pub name: String,
    /// The group.
    pub group: GroupTarget,
    /// The shorter side in `unit`.
    pub short: f64,
    /// The longer side in `unit`.
    pub long: f64,
    /// The unit the sides were typed in.
    pub unit: PresetUnit,
    /// Whether a new format starts in the quick selection.
    pub favourite: bool,
}

/// The built-in list and the maker's file over it.
#[derive(Debug, Clone)]
pub struct FormatLibrary {
    builtin: PresetList,
    pub(crate) user: UserFile,
    effective: PresetList,
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

    /// Checks a format against the whole library without changing it
    /// (criterion 20): `editing` is the id of the format being edited, whose
    /// own old size and name are not duplicates.
    ///
    /// # Errors
    /// The first rule the format breaks, with its field.
    pub fn check_format(
        &self,
        spec: &FormatSpec,
        editing: Option<&str>,
    ) -> Result<(), FormatError> {
        if checked_name(&spec.name).is_none() {
            return Err(FormatError::new(FormatField::Name, FormatReason::BadName));
        }
        let (short, long) = (
            spec.unit.to_length(spec.short),
            spec.unit.to_length(spec.long),
        );
        let finite = spec.short.is_finite() && spec.long.is_finite();
        if !finite || spec.short > spec.long || !sides_in_range(short, long) {
            return Err(FormatError::new(FormatField::Width, FormatReason::BadSize));
        }
        let group_id = match &spec.group {
            GroupTarget::Existing(id) => {
                if !self.effective.groups.iter().any(|g| g.id == *id) {
                    return Err(FormatError::new(FormatField::Name, FormatReason::NotFound));
                }
                Some(id.as_str())
            }
            GroupTarget::New { name, .. } => {
                self.check_group_name(name, None)?;
                if self.new_group_count() >= MAX_USER_GROUPS {
                    return Err(FormatError::new(
                        FormatField::Name,
                        FormatReason::LimitGroups,
                    ));
                }
                None
            }
        };
        if let Some(alike) = self
            .effective
            .with_sides(short.as_mm(), long.as_mm(), editing)
        {
            return Err(FormatError::new(
                FormatField::Width,
                FormatReason::SameSizeAs(alike.name.clone()),
            ));
        }
        let name = spec.name.trim();
        let taken = self.effective.presets().find(|(group, preset)| {
            Some(group.id.as_str()) == group_id
                && Some(preset.id.as_str()) != editing
                && preset.name.to_lowercase() == name.to_lowercase()
        });
        if let Some((_, preset)) = taken {
            return Err(FormatError::new(
                FormatField::Name,
                FormatReason::NameInGroup(preset.name.clone()),
            ));
        }
        if editing.is_none() && self.user_format_count() >= MAX_USER_FORMATS {
            return Err(FormatError::new(
                FormatField::Name,
                FormatReason::LimitFormats,
            ));
        }
        Ok(())
    }

    /// Checks the name of a new or renamed group: 1 to 24 characters, not used
    /// by another group ignoring case.
    pub(crate) fn check_group_name(
        &self,
        name: &str,
        editing: Option<&str>,
    ) -> Result<(), FormatError> {
        let Some(name) = checked_name(name) else {
            return Err(FormatError::new(
                FormatField::GroupName,
                FormatReason::BadName,
            ));
        };
        let taken = self.effective.groups.iter().any(|g| {
            Some(g.id.as_str()) != editing && g.name.to_lowercase() == name.to_lowercase()
        });
        if taken {
            return Err(FormatError::new(
                FormatField::GroupName,
                FormatReason::GroupNameTaken,
            ));
        }
        Ok(())
    }

    fn new_group_count(&self) -> usize {
        self.user.groups.iter().filter(|g| !g.appended).count()
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

    /// `u-` and a slug of `name`, with `-2`, `-3` on a collision with another
    /// format (criterion 32). Pure: the same name and library give the same id.
    pub(crate) fn unique_preset_id(&self, name: &str) -> String {
        unique_id(&slug(name, "format"), |id| {
            self.effective.find(id).is_some()
        })
    }

    pub(crate) fn unique_group_id(&self, name: &str) -> String {
        unique_id(&slug(name, "group"), |id| {
            self.effective.groups.iter().any(|g| g.id == id)
        })
    }
}

/// The lower-case letters and digits of `name`, any other run of characters
/// as one `-`; `fallback` when nothing is left.
fn slug(name: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug.to_string()
    }
}

fn unique_id(slug: &str, taken: impl Fn(&str) -> bool) -> String {
    let first = format!("u-{slug}");
    if !taken(&first) {
        return first;
    }
    let mut n = 2_u64;
    loop {
        let id = format!("{first}-{n}");
        if !taken(&id) {
            return id;
        }
        n += 1;
    }
}
