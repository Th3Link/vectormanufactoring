//! The vocabulary of the formats library's edits (`specs/0045-document-formats-
//! library/` criteria 18 to 21): what a format is as the form describes it, the
//! group it goes into, and why an edit is refused.

use crate::document_presets::{Orientation, PresetUnit};

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
