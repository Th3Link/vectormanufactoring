//! What an edit of the formats library is and whether it is allowed
//! (`specs/0045-document-formats-library/` criteria 18 to 21): the format as the
//! form describes it, the group it goes into, why an edit is refused, the checks
//! against the whole library, and the ids a new format or group gets.

use crate::document_presets::checked_name;
use crate::format_library::FormatLibrary;
use crate::format_library_file::{MAX_USER_FORMATS, MAX_USER_GROUPS, sides_in_range};
use crate::preset_units::{Orientation, PresetUnit};

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
    pub(crate) fn new(field: FormatField, reason: FormatReason) -> Self {
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

impl FormatLibrary {
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
            // The first typed side is the shorter one.
            let field = if sides_in_range(short, short) {
                FormatField::Height
            } else {
                FormatField::Width
            };
            return Err(FormatError::new(field, FormatReason::BadSize));
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
        if let Some(alike) = self.effective.with_sides(short, long, editing) {
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
