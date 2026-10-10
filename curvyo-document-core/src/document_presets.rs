//! The document size presets: paper sizes and slide formats read from a small
//! data file (`specs/0030-document-size-presets/`). The loader takes the file's
//! text and returns a validated list or an error that names the preset and the
//! rule it breaks; it opens no file and starts nothing. The shipped file is
//! `data/document-presets.toml`, compiled in; a test loads it, so a bad edit
//! fails the quality gate.

use serde::Deserialize;

use crate::display_unit::DisplayUnit;
use crate::document_size::{MAX_DOCUMENT_MM, MIN_DOCUMENT_MM};
use crate::units::{DocumentSize, Length, Tolerance};

/// Two sizes closer than this on each side are the same size: the tolerance of
/// the selected state and of the "no two presets alike" rule.
pub const PRESET_MATCH_TOLERANCE: Tolerance = Tolerance::from_mm(0.01);

/// The longest preset name, in characters.
const MAX_NAME_CHARS: usize = 24;

/// The longest preset note, in characters.
const MAX_NOTE_CHARS: usize = 60;

/// Portrait or landscape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Taller than wide.
    Portrait,
    /// Wider than tall.
    Landscape,
}

impl Orientation {
    /// The orientation of `size`: `None` for a square within
    /// [`PRESET_MATCH_TOLERANCE`].
    #[must_use]
    pub fn of(size: DocumentSize) -> Option<Self> {
        let (width, height) = (size.width.as_mm(), size.height.as_mm());
        if (width - height).abs() <= PRESET_MATCH_TOLERANCE.as_mm() {
            None
        } else if width < height {
            Some(Self::Portrait)
        } else {
            Some(Self::Landscape)
        }
    }
}

/// The unit a preset's size is written in. A pixel exists only here: it is
/// 1/96 inch, and the document stores millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetUnit {
    /// Millimetres.
    Mm,
    /// Inches.
    In,
    /// Pixels at 96 per inch.
    Px,
}

impl PresetUnit {
    /// The unit's symbol as written in the file.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::In => "in",
            Self::Px => "px",
        }
    }

    fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "mm" => Some(Self::Mm),
            "in" => Some(Self::In),
            "px" => Some(Self::Px),
            _ => None,
        }
    }

    /// `value` in this unit, in millimetres: `mm` as is, `in` as
    /// `value * 254 / 10`, `px` as `value * 254 / 960`.
    fn to_length(self, value: f64) -> Length {
        match self {
            Self::Mm => Length::from_mm(value),
            Self::In => Length::from_unit(value, DisplayUnit::In),
            Self::Px => Length::from_mm(value * 254.0 / 960.0),
        }
    }
}

/// A preset's size as written in the file, kept for the tooltip ("1920 × 1080
/// px"). Every comparison and every write uses the `Length` sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredSize {
    /// The shorter side, in `unit`.
    pub short: f64,
    /// The longer side, in `unit`.
    pub long: f64,
    /// The unit the numbers are in.
    pub unit: PresetUnit,
}

/// One format.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentPreset {
    /// A stable id, lower case; never shown.
    pub id: String,
    /// The label of its button.
    pub name: String,
    /// An optional line for the tooltip ("Full HD").
    pub note: Option<String>,
    /// The shorter side.
    pub short_side: Length,
    /// The longer side.
    pub long_side: Length,
    /// The size as written in the file.
    pub authored: AuthoredSize,
}

/// A heading and its formats.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetGroup {
    /// A stable id.
    pub id: String,
    /// The heading in the panel.
    pub name: String,
    /// The orientation a pick starts with when the document is not already in
    /// this group.
    pub default_orientation: Orientation,
    /// The formats, in file order.
    pub presets: Vec<DocumentPreset>,
}

/// The validated list of presets, in file order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresetList {
    /// The groups, in file order.
    pub groups: Vec<PresetGroup>,
}

/// What a loading error is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetSubject {
    /// The file as a whole.
    File,
    /// The group with this id (or the group's position when it has no id).
    Group(String),
    /// The preset with this id (or its position when it has no id).
    Preset(String),
}

/// The rule a file breaks (criterion 6 of the spec).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetReason {
    /// Not valid TOML, or a key the format does not define; carries the
    /// parser's message.
    Syntax(String),
    /// `format` is missing or is not 1.
    FormatNotOne,
    /// A required field is missing.
    MissingField(&'static str),
    /// `default_orientation` is neither `portrait` nor `landscape`.
    BadOrientation(String),
    /// A group has no preset.
    EmptyGroup,
    /// Two groups share an id.
    DuplicateGroupId,
    /// Two presets share an id.
    DuplicatePresetId,
    /// `unit` is not `mm`, `in` or `px`.
    UnknownUnit(String),
    /// A side is not a finite number above zero.
    SideNotPositive,
    /// `short_side` is larger than `long_side`.
    ShortSideLonger,
    /// A converted side is below 1 mm or above 100 000 mm.
    SideOutOfRange,
    /// An id has characters other than lower-case letters, digits and `-`.
    BadId,
    /// A name is empty or longer than 24 characters after trimming.
    BadName,
    /// A note is longer than 60 characters.
    NoteTooLong,
    /// The preset has the same size as the preset with this id.
    SameSizeAs(String),
}

/// A preset file that failed validation, with what and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("document presets: {subject:?}: {reason:?}")]
pub struct PresetError {
    /// What the error is about.
    pub subject: PresetSubject,
    /// The rule that is broken.
    pub reason: PresetReason,
}

impl PresetError {
    fn new(subject: PresetSubject, reason: PresetReason) -> Self {
        Self { subject, reason }
    }
}

// The file as parsed: every field optional so that a missing one is named by the
// validation pass, and unknown keys are refused.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    format: Option<i64>,
    #[serde(default)]
    group: Vec<RawGroup>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroup {
    id: Option<String>,
    name: Option<String>,
    default_orientation: Option<String>,
    #[serde(default)]
    preset: Vec<RawPreset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPreset {
    id: Option<String>,
    name: Option<String>,
    note: Option<String>,
    short_side: Option<toml::Value>,
    long_side: Option<toml::Value>,
    unit: Option<String>,
}

fn id_is_valid(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn side_number(value: &toml::Value) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    match value {
        toml::Value::Float(f) => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

impl PresetList {
    /// The presets this build ships (`data/document-presets.toml`). The file is
    /// checked by tests, so the error branch is unreachable in a built app.
    ///
    /// # Errors
    /// The same as [`PresetList::parse`].
    pub fn shipped() -> Result<Self, PresetError> {
        Self::parse(include_str!("../data/document-presets.toml"))
    }

    /// Reads and validates the text of a presets file.
    ///
    /// # Errors
    /// A [`PresetError`] naming the group or preset and the rule it breaks; no
    /// list.
    pub fn parse(text: &str) -> Result<Self, PresetError> {
        let raw: RawFile = toml::from_str(text).map_err(|error| {
            PresetError::new(
                PresetSubject::File,
                PresetReason::Syntax(error.message().to_string()),
            )
        })?;
        if raw.format != Some(1) {
            return Err(PresetError::new(
                PresetSubject::File,
                PresetReason::FormatNotOne,
            ));
        }
        let mut list = Self::default();
        for (position, group) in raw.group.into_iter().enumerate() {
            let group = validate_group(position, group)?;
            if list.groups.iter().any(|known| known.id == group.id) {
                return Err(PresetError::new(
                    PresetSubject::Group(group.id),
                    PresetReason::DuplicateGroupId,
                ));
            }
            list.groups.push(group);
        }
        list.check_presets_are_distinct()?;
        Ok(list)
    }

    /// Every preset of every group, in file order.
    pub fn presets(&self) -> impl Iterator<Item = (&PresetGroup, &DocumentPreset)> {
        self.groups
            .iter()
            .flat_map(|group| group.presets.iter().map(move |preset| (group, preset)))
    }

    /// The preset `size` has, in either orientation: its shorter and longer
    /// side equal the preset's within [`PRESET_MATCH_TOLERANCE`]. At most one
    /// matches, because the file never holds two alike.
    #[must_use]
    pub fn matching(&self, size: DocumentSize) -> Option<(&PresetGroup, &DocumentPreset)> {
        let (short, long) = sides_of(size);
        self.presets()
            .find(|(_, preset)| same_sides(preset, short, long))
    }

    fn check_presets_are_distinct(&self) -> Result<(), PresetError> {
        let mut seen: Vec<&DocumentPreset> = Vec::new();
        for (_, preset) in self.presets() {
            if seen.iter().any(|known| known.id == preset.id) {
                return Err(PresetError::new(
                    PresetSubject::Preset(preset.id.clone()),
                    PresetReason::DuplicatePresetId,
                ));
            }
            let (short, long) = (preset.short_side.as_mm(), preset.long_side.as_mm());
            if let Some(alike) = seen.iter().find(|known| same_sides(known, short, long)) {
                return Err(PresetError::new(
                    PresetSubject::Preset(preset.id.clone()),
                    PresetReason::SameSizeAs(alike.id.clone()),
                ));
            }
            seen.push(preset);
        }
        Ok(())
    }
}

/// The shorter and longer side of a size, in millimetres.
fn sides_of(size: DocumentSize) -> (f64, f64) {
    let (width, height) = (size.width.as_mm(), size.height.as_mm());
    (width.min(height), width.max(height))
}

fn same_sides(preset: &DocumentPreset, short: f64, long: f64) -> bool {
    let tolerance = PRESET_MATCH_TOLERANCE.as_mm();
    (preset.short_side.as_mm() - short).abs() <= tolerance
        && (preset.long_side.as_mm() - long).abs() <= tolerance
}

fn validate_group(position: usize, raw: RawGroup) -> Result<PresetGroup, PresetError> {
    let subject = PresetSubject::Group(raw.id.clone().unwrap_or_else(|| format!("#{position}")));
    let fail = |reason| PresetError::new(subject.clone(), reason);
    let id = raw
        .id
        .ok_or_else(|| fail(PresetReason::MissingField("id")))?;
    let name = raw
        .name
        .ok_or_else(|| fail(PresetReason::MissingField("name")))?;
    let orientation = raw
        .default_orientation
        .ok_or_else(|| fail(PresetReason::MissingField("default_orientation")))?;
    if !id_is_valid(&id) {
        return Err(fail(PresetReason::BadId));
    }
    let name = checked_name(&name).ok_or_else(|| fail(PresetReason::BadName))?;
    let default_orientation = match orientation.as_str() {
        "portrait" => Orientation::Portrait,
        "landscape" => Orientation::Landscape,
        _ => return Err(fail(PresetReason::BadOrientation(orientation))),
    };
    if raw.preset.is_empty() {
        return Err(fail(PresetReason::EmptyGroup));
    }
    let presets = raw
        .preset
        .into_iter()
        .enumerate()
        .map(|(n, preset)| validate_preset(n, preset))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PresetGroup {
        id,
        name,
        default_orientation,
        presets,
    })
}

fn checked_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    (!trimmed.is_empty() && trimmed.chars().count() <= MAX_NAME_CHARS).then(|| trimmed.to_string())
}

fn validate_preset(position: usize, raw: RawPreset) -> Result<DocumentPreset, PresetError> {
    let subject = PresetSubject::Preset(raw.id.clone().unwrap_or_else(|| format!("#{position}")));
    let fail = |reason| PresetError::new(subject.clone(), reason);
    let id = raw
        .id
        .ok_or_else(|| fail(PresetReason::MissingField("id")))?;
    let name = raw
        .name
        .ok_or_else(|| fail(PresetReason::MissingField("name")))?;
    let short = raw
        .short_side
        .ok_or_else(|| fail(PresetReason::MissingField("short_side")))?;
    let long = raw
        .long_side
        .ok_or_else(|| fail(PresetReason::MissingField("long_side")))?;
    let unit = raw
        .unit
        .ok_or_else(|| fail(PresetReason::MissingField("unit")))?;
    let unit =
        PresetUnit::from_symbol(&unit).ok_or_else(|| fail(PresetReason::UnknownUnit(unit)))?;
    if !id_is_valid(&id) {
        return Err(fail(PresetReason::BadId));
    }
    let name = checked_name(&name).ok_or_else(|| fail(PresetReason::BadName))?;
    if raw
        .note
        .as_ref()
        .is_some_and(|note| note.chars().count() > MAX_NOTE_CHARS)
    {
        return Err(fail(PresetReason::NoteTooLong));
    }
    let positive = |value: &toml::Value| {
        side_number(value)
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or_else(|| fail(PresetReason::SideNotPositive))
    };
    let (short, long) = (positive(&short)?, positive(&long)?);
    if short > long {
        return Err(fail(PresetReason::ShortSideLonger));
    }
    let (short_side, long_side) = (unit.to_length(short), unit.to_length(long));
    let in_range = |side: Length| (MIN_DOCUMENT_MM..=MAX_DOCUMENT_MM).contains(&side.as_mm());
    if !in_range(short_side) || !in_range(long_side) {
        return Err(fail(PresetReason::SideOutOfRange));
    }
    Ok(DocumentPreset {
        id,
        name,
        note: raw.note,
        short_side,
        long_side,
        authored: AuthoredSize { short, long, unit },
    })
}
