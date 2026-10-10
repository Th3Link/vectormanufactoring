//! Reads and writes the maker's formats file, `document-formats.toml`
//! (`specs/0045-document-formats-library/` criteria 2, 5, 26, 32, 33): the
//! text of the file to data and back, in a canonical order, so reading and
//! writing again gives the same text. It opens no file; the platform layer
//! moves the text.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::document_presets::{
    DocumentPreset, Orientation, PresetError, PresetGroup, PresetList, PresetReason, PresetSubject,
    RawPreset, checked_name, id_is_valid, same_sides, validate_preset,
};
use crate::units::Length;

/// The schema of the user file.
pub const USER_FILE_FORMAT: i64 = 1;

/// The largest user file, in bytes.
pub const MAX_USER_FILE_BYTES: usize = 256 * 1024;

/// The most formats the maker can own.
pub const MAX_USER_FORMATS: usize = 200;

/// The most groups the maker can add.
pub const MAX_USER_GROUPS: usize = 20;

/// A group of the user file: a new group of the maker's, or formats appended
/// to a built-in group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserGroup {
    pub(crate) id: String,
    /// For an appended group the built-in group's name.
    pub(crate) name: String,
    /// For an appended group the built-in group's orientation.
    pub(crate) default_orientation: Orientation,
    /// Whether `id` is a built-in group's: the group is not the maker's, only
    /// its formats are.
    pub(crate) appended: bool,
    pub(crate) presets: Vec<DocumentPreset>,
}

/// The user file as data.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UserFile {
    /// The ids in the quick selection; `None` while the file has no
    /// `favourites` key (the built-in flags apply).
    pub(crate) favourites: Option<BTreeSet<String>>,
    /// Group id to on or off, only where it differs from the default.
    pub(crate) enabled: BTreeMap<String, bool>,
    pub(crate) groups: Vec<UserGroup>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUserFile {
    format: Option<i64>,
    favourites: Option<Vec<String>>,
    enabled: Option<BTreeMap<String, bool>>,
    #[serde(default)]
    group: Vec<RawUserGroup>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUserGroup {
    id: Option<String>,
    name: Option<String>,
    default_orientation: Option<String>,
    #[serde(default)]
    preset: Vec<RawUserPreset>,
}

/// A user preset has the built-in preset's fields without `favourite`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUserPreset {
    id: Option<String>,
    name: Option<String>,
    note: Option<String>,
    short_side: Option<toml::Value>,
    long_side: Option<toml::Value>,
    unit: Option<String>,
}

impl From<RawUserPreset> for RawPreset {
    fn from(raw: RawUserPreset) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            note: raw.note,
            short_side: raw.short_side,
            long_side: raw.long_side,
            unit: raw.unit,
            favourite: None,
        }
    }
}

fn fail(subject: PresetSubject, reason: PresetReason) -> PresetError {
    PresetError { subject, reason }
}

impl UserFile {
    /// Reads and validates the text of a user file against the built-in list
    /// (criteria 4 and 5). Entries of `favourites` and `enabled` that name
    /// nothing in the library are dropped without an error. With `collisions`
    /// off (an import) a format may have the id or the size of one the library
    /// already has: the merge decides what happens to it (criterion 27).
    ///
    /// # Errors
    /// A [`PresetError`] naming the group or format and the rule it breaks.
    pub(crate) fn parse(
        text: &str,
        builtin: &PresetList,
        collisions: bool,
    ) -> Result<Self, PresetError> {
        if text.len() > MAX_USER_FILE_BYTES {
            return Err(fail(PresetSubject::File, PresetReason::FileTooLarge));
        }
        let raw: RawUserFile = toml::from_str(text).map_err(|error| {
            fail(
                PresetSubject::File,
                PresetReason::Syntax(error.message().to_string()),
            )
        })?;
        match raw.format {
            Some(USER_FILE_FORMAT) => {}
            Some(_) => return Err(fail(PresetSubject::File, PresetReason::FormatNewer)),
            None => {
                return Err(fail(
                    PresetSubject::File,
                    PresetReason::MissingField("format"),
                ));
            }
        }
        let mut file = Self::default();
        let mut known: Vec<DocumentPreset> = builtin.presets().map(|(_, p)| p.clone()).collect();
        for (position, group) in raw.group.into_iter().enumerate() {
            let group = read_group(position, group, builtin, &file.groups)?;
            if collisions {
                check_group_formats(&group, &known)?;
            }
            known.extend(group.presets.iter().cloned());
            file.groups.push(group);
        }
        file.check_limits()?;
        file.keep_known_state(raw.favourites, raw.enabled, builtin);
        Ok(file)
    }

    /// The formats of the maker's, in file order.
    pub(crate) fn presets(&self) -> impl Iterator<Item = &DocumentPreset> {
        self.groups.iter().flat_map(|group| group.presets.iter())
    }

    /// The limits of criterion 5.
    pub(crate) fn check_limits(&self) -> Result<(), PresetError> {
        if self.presets().count() > MAX_USER_FORMATS {
            return Err(fail(PresetSubject::File, PresetReason::TooManyFormats));
        }
        if self.groups.iter().filter(|g| !g.appended).count() > MAX_USER_GROUPS {
            return Err(fail(PresetSubject::File, PresetReason::TooManyGroups));
        }
        Ok(())
    }

    fn keep_known_state(
        &mut self,
        favourites: Option<Vec<String>>,
        enabled: Option<BTreeMap<String, bool>>,
        builtin: &PresetList,
    ) {
        let preset_known =
            |id: &str| builtin.find(id).is_some() || self.presets().any(|preset| preset.id == id);
        let favourites = favourites.map(|ids| {
            ids.into_iter()
                .filter(|id| preset_known(id))
                .collect::<BTreeSet<_>>()
        });
        let group_default = |id: &str| {
            builtin
                .groups
                .iter()
                .find(|group| group.id == id)
                .map(|group| group.enabled)
                .or_else(|| self.groups.iter().any(|g| g.id == id).then_some(true))
        };
        // Only a choice that differs from the default is kept, so the file
        // stays canonical.
        let enabled: BTreeMap<String, bool> = enabled
            .unwrap_or_default()
            .into_iter()
            .filter(|(id, on)| group_default(id).is_some_and(|default| default != *on))
            .collect();
        self.favourites = favourites;
        self.enabled = enabled;
    }

    /// The canonical text of the file (criterion 33): `format`, `favourites`,
    /// `enabled`, then the groups in creation order.
    ///
    /// # Errors
    /// A syntax error if the writer fails, which no validated file causes.
    pub(crate) fn to_text(&self, builtin: &PresetList) -> Result<String, PresetError> {
        let favourites = self.favourites.as_ref().map(|set| {
            // List order, so the text does not depend on the set.
            let in_list = |id: &String| set.contains(id);
            let mut ids: Vec<String> = builtin
                .presets()
                .map(|(_, preset)| preset.id.clone())
                .chain(self.presets().map(|preset| preset.id.clone()))
                .filter(in_list)
                .collect();
            ids.dedup();
            ids
        });
        let out = OutFile {
            format: USER_FILE_FORMAT,
            favourites,
            enabled: self.enabled.clone(),
            group: self.groups.iter().map(OutGroup::new).collect(),
        };
        toml::to_string(&out)
            .map_err(|error| fail(PresetSubject::File, PresetReason::Syntax(error.to_string())))
    }
}

fn read_group(
    position: usize,
    raw: RawUserGroup,
    builtin: &PresetList,
    earlier: &[UserGroup],
) -> Result<UserGroup, PresetError> {
    let subject = PresetSubject::Group(raw.id.clone().unwrap_or_else(|| format!("#{position}")));
    let id = raw
        .id
        .ok_or_else(|| fail(subject.clone(), PresetReason::MissingField("id")))?;
    if !id_is_valid(&id) {
        return Err(fail(subject, PresetReason::BadId));
    }
    if earlier.iter().any(|group| group.id == id) {
        return Err(fail(subject, PresetReason::DuplicateGroupId));
    }
    let name = raw
        .name
        .map(|name| checked_name(&name).ok_or_else(|| fail(subject.clone(), PresetReason::BadName)))
        .transpose()?;
    let orientation = raw
        .default_orientation
        .map(|value| orientation_of(&value, &subject))
        .transpose()?;
    let (name, default_orientation, appended) =
        match builtin.groups.iter().find(|group| group.id == id) {
            Some(group) => {
                // Criterion 4a: what the file says about a built-in group must
                // equal the built-in value.
                let name_differs = name.as_ref().is_some_and(|name| *name != group.name);
                let orientation_differs =
                    orientation.is_some_and(|value| value != group.default_orientation);
                if name_differs || orientation_differs {
                    return Err(fail(subject, PresetReason::GroupMismatch));
                }
                (group.name.clone(), group.default_orientation, true)
            }
            None => (
                name.ok_or_else(|| fail(subject.clone(), PresetReason::MissingField("name")))?,
                orientation.ok_or_else(|| {
                    fail(
                        subject.clone(),
                        PresetReason::MissingField("default_orientation"),
                    )
                })?,
                false,
            ),
        };
    let presets = raw
        .preset
        .into_iter()
        .enumerate()
        .map(|(n, preset)| validate_preset(n, preset.into(), true))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UserGroup {
        id,
        name,
        default_orientation,
        appended,
        presets,
    })
}

fn orientation_of(value: &str, subject: &PresetSubject) -> Result<Orientation, PresetError> {
    match value {
        "portrait" => Ok(Orientation::Portrait),
        "landscape" => Ok(Orientation::Landscape),
        _ => Err(fail(
            subject.clone(),
            PresetReason::BadOrientation(value.to_string()),
        )),
    }
}

/// Criterion 4c and 5: the group's formats have ids and sizes that no earlier
/// format of the library has, and none of them twice.
fn check_group_formats(group: &UserGroup, known: &[DocumentPreset]) -> Result<(), PresetError> {
    let mut seen: Vec<&DocumentPreset> = known.iter().collect();
    for preset in &group.presets {
        let subject = PresetSubject::Preset(preset.id.clone());
        if seen.iter().any(|other| other.id == preset.id) {
            return Err(fail(subject, PresetReason::DuplicatePresetId));
        }
        let (short, long) = (preset.short_side.as_mm(), preset.long_side.as_mm());
        if let Some(alike) = seen.iter().find(|other| same_sides(other, short, long)) {
            return Err(fail(subject, PresetReason::SameSizeAs(alike.id.clone())));
        }
        seen.push(preset);
    }
    Ok(())
}

#[derive(Serialize)]
struct OutFile {
    format: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    favourites: Option<Vec<String>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    enabled: BTreeMap<String, bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    group: Vec<OutGroup>,
}

#[derive(Serialize)]
struct OutGroup {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_orientation: Option<&'static str>,
    preset: Vec<OutPreset>,
}

#[derive(Serialize)]
struct OutPreset {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    short_side: f64,
    long_side: f64,
    unit: &'static str,
}

impl OutGroup {
    fn new(group: &UserGroup) -> Self {
        Self {
            id: group.id.clone(),
            // A group that is only a built-in group's id adds formats to it
            // and says nothing else about it.
            name: (!group.appended).then(|| group.name.clone()),
            default_orientation: (!group.appended).then_some(match group.default_orientation {
                Orientation::Portrait => "portrait",
                Orientation::Landscape => "landscape",
            }),
            preset: group
                .presets
                .iter()
                .map(|preset| OutPreset {
                    id: preset.id.clone(),
                    name: preset.name.clone(),
                    note: preset.note.clone(),
                    short_side: preset.authored.short,
                    long_side: preset.authored.long,
                    unit: preset.authored.unit.symbol(),
                })
                .collect(),
        }
    }
}

/// The group a user format belongs to, as the effective list holds it.
pub(crate) fn new_list_group(group: &UserGroup, enabled: bool) -> PresetGroup {
    PresetGroup {
        id: group.id.clone(),
        name: group.name.clone(),
        default_orientation: group.default_orientation,
        presets: group.presets.clone(),
        enabled,
        user: true,
    }
}

/// A user preset built from typed values; the sides are in the unit typed.
pub(crate) fn user_preset(
    id: String,
    name: String,
    short: f64,
    long: f64,
    unit: crate::document_presets::PresetUnit,
) -> DocumentPreset {
    DocumentPreset {
        id,
        name,
        note: None,
        short_side: unit.to_length(short),
        long_side: unit.to_length(long),
        authored: crate::document_presets::AuthoredSize { short, long, unit },
        favourite: false,
        user: true,
    }
}

/// Whether both sides are within the document limits (criterion 5).
pub(crate) fn sides_in_range(short: Length, long: Length) -> bool {
    let range = crate::document_size::MIN_DOCUMENT_MM..=crate::document_size::MAX_DOCUMENT_MM;
    range.contains(&short.as_mm()) && range.contains(&long.as_mm())
}

/// A human sentence for a failed file: the subject and the rule it breaks,
/// without the quoting of the Debug form (criterion 6: "Your formats file could
/// not be read: <reason>").
#[must_use]
pub fn describe_error(error: &PresetError) -> String {
    let rule = match &error.reason {
        PresetReason::Syntax(message) => {
            format!("not valid TOML or has an unknown key ({message})")
        }
        PresetReason::FormatNotSupported => "unsupported file version".to_string(),
        PresetReason::FormatNewer => "made by a newer version".to_string(),
        PresetReason::MissingField(field) => format!("\"{field}\" is missing"),
        PresetReason::BadOrientation(value) => {
            format!("\"{value}\" is not portrait or landscape")
        }
        PresetReason::EmptyGroup => "has no formats".to_string(),
        PresetReason::DuplicateGroupId | PresetReason::DuplicatePresetId => {
            "id is used twice".to_string()
        }
        PresetReason::UnknownUnit(unit) => format!("unit \"{unit}\" is not mm, cm, in or px"),
        PresetReason::SideNotPositive => "a side is not a number above zero".to_string(),
        PresetReason::ShortSideLonger => "short_side is longer than long_side".to_string(),
        PresetReason::SideOutOfRange => "a side is not between 1 and 100000 mm".to_string(),
        PresetReason::BadId => "id may use lower-case letters, digits and - only".to_string(),
        PresetReason::BadName => "name must be 1 to 24 characters".to_string(),
        PresetReason::NoteTooLong => "note is longer than 60 characters".to_string(),
        PresetReason::SameSizeAs(id) => format!("same size as \"{id}\""),
        PresetReason::FileTooLarge => "file is larger than 256 KiB".to_string(),
        PresetReason::TooManyFormats => format!("more than {MAX_USER_FORMATS} formats"),
        PresetReason::TooManyGroups => format!("more than {MAX_USER_GROUPS} groups"),
        PresetReason::GroupMismatch => {
            "name or default_orientation differs from the built-in group".to_string()
        }
    };
    match &error.subject {
        PresetSubject::File => rule,
        PresetSubject::Group(id) => format!("group \"{id}\": {rule}"),
        PresetSubject::Preset(id) => format!("format \"{id}\": {rule}"),
    }
}
