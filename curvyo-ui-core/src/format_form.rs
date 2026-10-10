//! The inline form that adds or edits a format and the group form
//! (`specs/0045-document-formats-library/` criteria 18 to 21 and 24): what the
//! form starts with, how typed text is checked, and the message of each
//! refusal. The host holds the typed text; every rule and every message is
//! here.

use curvyo_document_core::{
    DisplayUnit, DocumentPreset, DocumentSize, FormatError, FormatLibrary, FormatReason,
    FormatSpec, GroupTarget, ImportReport, MAX_DOCUMENT_MM, MAX_USER_FORMATS, MIN_DOCUMENT_MM,
    Orientation, PresetGroup, PresetUnit, checked_name,
};

use crate::display_unit_text::{decimal_text, format_field_length, range_message};
use crate::transform_entry::parse_entry_number;

/// The field of the form a message belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    /// The format's name.
    Name,
    /// The width.
    Width,
    /// The height.
    Height,
    /// The new group's name.
    GroupName,
}

impl FormField {
    /// The field's name for the host.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Width => "width",
            Self::Height => "height",
            Self::GroupName => "group-name",
        }
    }
}

/// A refused field with the text of its chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// The field to mark invalid.
    pub field: FormField,
    /// The chip's text.
    pub message: String,
}

/// What the form starts with: typed text, as the host shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormPrefill {
    /// The name.
    pub name: String,
    /// The width text.
    pub width: String,
    /// The height text.
    pub height: String,
    /// The unit symbol of the two fields.
    pub unit: &'static str,
    /// The id of the chosen group, or empty for "New group…".
    pub group_id: String,
    /// "Add to quick selection".
    pub favourite: bool,
}

/// The form a press on "Add format" opens (criterion 18): the document's size
/// in the display unit (`px` is never prefilled), an empty name, the group last
/// used, else the first group of the maker's, else "New group…".
#[must_use]
pub fn add_prefill(
    library: &FormatLibrary,
    size: DocumentSize,
    unit: DisplayUnit,
    last_group: &str,
) -> FormPrefill {
    let list = library.list();
    let group_id = if list.groups.iter().any(|g| g.id == last_group) {
        last_group.to_string()
    } else {
        list.groups
            .iter()
            .find(|g| g.user)
            .map(|g| g.id.clone())
            .unwrap_or_default()
    };
    FormPrefill {
        name: String::new(),
        width: format_field_length(size.width, unit),
        height: format_field_length(size.height, unit),
        unit: unit.symbol(),
        group_id,
        favourite: true,
    }
}

/// The form a press on a format's edit button opens (criterion 21): its values
/// in the unit they were typed in, in the order the list shows them.
#[must_use]
pub fn edit_prefill(group: &PresetGroup, preset: &DocumentPreset) -> FormPrefill {
    let authored = preset.authored;
    let (first, second) = match group.default_orientation {
        Orientation::Portrait => (authored.short, authored.long),
        Orientation::Landscape => (authored.long, authored.short),
    };
    FormPrefill {
        name: preset.name.clone(),
        width: decimal_text(first, 4),
        height: decimal_text(second, 4),
        unit: authored.unit.symbol(),
        group_id: group.id.clone(),
        favourite: preset.favourite,
    }
}

/// The typed text of the form.
#[derive(Debug, Clone, PartialEq)]
pub struct FormDraft<'a> {
    /// The name field.
    pub name: &'a str,
    /// The width field.
    pub width: &'a str,
    /// The height field.
    pub height: &'a str,
    /// The unit symbol of the two fields.
    pub unit: PresetUnit,
    /// The chosen group's id, or empty for "New group…".
    pub group_id: &'a str,
    /// The new group's name (used with an empty `group_id`).
    pub new_group_name: &'a str,
    /// The new group's "Opens as".
    pub new_group_orientation: Orientation,
    /// "Add to quick selection".
    pub favourite: bool,
}

const NAME_MESSAGE: &str = "Enter a name of 1 to 24 characters";

/// Checks the typed text (criterion 20) and returns the format, or every field
/// that is refused in form order (the host shows the chip of the first and
/// marks the others invalid). `editing` is the id of the format being edited.
///
/// # Errors
/// The refused fields with their messages; nothing is written either way.
pub fn check_format_form(
    library: &FormatLibrary,
    draft: &FormDraft<'_>,
    editing: Option<&str>,
) -> Result<FormatSpec, Vec<FieldError>> {
    let mut errors = Vec::new();
    let mut refuse = |field, message: String| errors.push(FieldError { field, message });
    if checked_name(draft.name).is_none() {
        refuse(FormField::Name, NAME_MESSAGE.to_string());
    }
    let width = side_value(draft.width, draft.unit);
    let height = side_value(draft.height, draft.unit);
    if width.is_none() {
        refuse(FormField::Width, size_message(draft.unit));
    }
    if height.is_none() {
        refuse(FormField::Height, size_message(draft.unit));
    }
    let group = if draft.group_id.is_empty() {
        if checked_name(draft.new_group_name).is_none() {
            refuse(FormField::GroupName, NAME_MESSAGE.to_string());
        }
        GroupTarget::New {
            name: draft.new_group_name.to_string(),
            orientation: draft.new_group_orientation,
        }
    } else {
        GroupTarget::Existing(draft.group_id.to_string())
    };
    let (Some(width), Some(height)) = (width, height) else {
        return Err(errors);
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    let spec = FormatSpec {
        name: draft.name.trim().to_string(),
        group,
        short: width.min(height),
        long: width.max(height),
        unit: draft.unit,
        favourite: draft.favourite,
    };
    library
        .check_format(&spec, editing)
        .map_err(|error| vec![field_error(&error)])?;
    Ok(spec)
}

/// The value of a size field in `unit`, or `None` when the text is not a number
/// or the size is outside 1 mm to 100 000 mm.
fn side_value(text: &str, unit: PresetUnit) -> Option<f64> {
    let value = parse_entry_number(text, false)?;
    let mm = unit.to_length(value).as_mm();
    (value > 0.0 && (MIN_DOCUMENT_MM - 1e-9..=MAX_DOCUMENT_MM + 1e-9).contains(&mm))
        .then_some(value)
}

/// "Enter a number from 1 to 100000 mm", the limits in the chosen unit and
/// rounded inward.
fn size_message(unit: PresetUnit) -> String {
    let (lowest, highest) = (
        MIN_DOCUMENT_MM / unit.to_length(1.0).as_mm(),
        MAX_DOCUMENT_MM / unit.to_length(1.0).as_mm(),
    );
    format!("{} {}", range_message(lowest, highest), unit.symbol())
}

/// The chip of a refusal that only the library can see (criterion 20), and
/// of every edit the library refuses.
#[must_use]
pub fn field_error(error: &FormatError) -> FieldError {
    let message = match &error.reason {
        FormatReason::SameSizeAs(name) => format!("Same size as {name}"),
        FormatReason::NameInGroup(name) => format!("This group already has {name}"),
        FormatReason::LimitFormats => format!("The list is full ({MAX_USER_FORMATS} formats)"),
        FormatReason::LimitGroups => "There are too many groups".to_string(),
        FormatReason::GroupNameTaken => "A group of this name exists".to_string(),
        FormatReason::BadName => NAME_MESSAGE.to_string(),
        FormatReason::BadSize => "Enter a size from 1 to 100000 mm".to_string(),
        FormatReason::NotFound => "This group no longer exists".to_string(),
        FormatReason::BuiltIn => "Built-in entries cannot be changed".to_string(),
    };
    let field = match error.field {
        curvyo_document_core::FormatField::Name => FormField::Name,
        curvyo_document_core::FormatField::Width => FormField::Width,
        curvyo_document_core::FormatField::Height => FormField::Height,
        curvyo_document_core::FormatField::GroupName => FormField::GroupName,
    };
    FieldError { field, message }
}

/// The notice after a format was added (criterion 20).
#[must_use]
pub fn added_notice(name: &str) -> String {
    format!("Added {}.", name.trim())
}

/// The notice after a format or group was saved (criteria 21 and 24).
#[must_use]
pub fn saved_notice(name: &str) -> String {
    format!("Saved {}.", name.trim())
}

/// The notice after an import (criterion 27): "Imported 5 formats in 2 groups.
/// Skipped 2 that were already there."
#[must_use]
pub fn import_notice(report: &ImportReport) -> String {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut text = if report.added == 0 {
        "Nothing new to import.".to_string()
    } else {
        format!(
            "Imported {} in {}.",
            plural(report.added, "format", "formats"),
            plural(report.groups, "group", "groups")
        )
    };
    if report.skipped > 0 {
        let were = if report.skipped == 1 {
            "that was"
        } else {
            "that were"
        };
        text.push_str(" Skipped ");
        text.push_str(&report.skipped.to_string());
        text.push(' ');
        text.push_str(were);
        text.push_str(" already there.");
    }
    text
}

/// The refusal of a file that could not be imported (criterion 25).
#[must_use]
pub fn import_refusal(file_name: &str, reason: &str) -> String {
    format!("Could not import {file_name}: {reason}. Nothing was changed.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> FormatLibrary {
        FormatLibrary::shipped().expect("the shipped file loads")
    }

    fn draft<'a>() -> FormDraft<'a> {
        FormDraft {
            name: "Key ring",
            width: "30",
            height: "50",
            unit: PresetUnit::Mm,
            group_id: "paper",
            new_group_name: "",
            new_group_orientation: Orientation::Portrait,
            favourite: true,
        }
    }

    fn messages(draft: &FormDraft<'_>) -> Vec<(FormField, String)> {
        match check_format_form(&library(), draft, None) {
            Ok(_) => Vec::new(),
            Err(errors) => errors.into_iter().map(|e| (e.field, e.message)).collect(),
        }
    }

    /// Criterion 18: the document's size in the display unit, px never, the
    /// group last used, else the first of the maker's, else "New group…".
    #[test]
    fn the_add_form_opens_with_the_document_size_and_a_group() {
        let mut lib = library();
        let a4 = DocumentSize::from_mm(210.0, 297.0);
        let first = add_prefill(&lib, a4, DisplayUnit::Mm, "");
        assert_eq!(
            (
                first.width.as_str(),
                first.height.as_str(),
                first.unit,
                first.group_id.as_str()
            ),
            ("210", "297", "mm", "")
        );
        assert!(first.name.is_empty() && first.favourite);
        let inches = add_prefill(&lib, a4, DisplayUnit::In, "");
        assert_eq!((inches.width.as_str(), inches.unit), ("8.2677", "in"));
        lib.add_format(&FormatSpec {
            name: "Key ring".into(),
            group: GroupTarget::New {
                name: "Laser".into(),
                orientation: Orientation::Landscape,
            },
            short: 30.0,
            long: 50.0,
            unit: PresetUnit::Mm,
            favourite: true,
        })
        .unwrap();
        assert_eq!(
            add_prefill(&lib, a4, DisplayUnit::Mm, "").group_id,
            "u-laser"
        );
        assert_eq!(
            add_prefill(&lib, a4, DisplayUnit::Mm, "paper").group_id,
            "paper"
        );
        assert_eq!(
            add_prefill(&lib, a4, DisplayUnit::Mm, "gone").group_id,
            "u-laser"
        );
    }

    /// Criterion 21: the edit form holds the values as they were typed.
    #[test]
    fn the_edit_form_holds_the_authored_values() {
        let lib = library();
        let (group, a4) = lib.list().find("a4").unwrap();
        let form = edit_prefill(group, a4);
        assert_eq!(
            (
                form.name.as_str(),
                form.width.as_str(),
                form.height.as_str(),
                form.unit
            ),
            ("A4", "210", "297", "mm")
        );
        let (group, slide) = lib.list().find("slide-16-9").unwrap();
        let form = edit_prefill(group, slide);
        assert_eq!(
            (form.width.as_str(), form.height.as_str(), form.unit),
            ("1920", "1080", "px")
        );
    }

    /// Criterion 20: the message of each refusal; all refused fields are
    /// reported in form order.
    #[test]
    fn each_refusal_has_its_message_and_every_field_is_reported() {
        assert_eq!(messages(&draft()).len(), 0);
        let empty = FormDraft {
            name: " ",
            ..draft()
        };
        assert_eq!(
            messages(&empty),
            [(
                FormField::Name,
                "Enter a name of 1 to 24 characters".to_string()
            )]
        );
        let long = "x".repeat(25);
        let toolong = FormDraft {
            name: &long,
            ..draft()
        };
        assert_eq!(messages(&toolong)[0].0, FormField::Name);
        let both = FormDraft {
            name: "",
            width: "abc",
            height: "0",
            ..draft()
        };
        let fields: Vec<_> = messages(&both).iter().map(|(f, _)| *f).collect();
        assert_eq!(
            fields,
            [FormField::Name, FormField::Width, FormField::Height]
        );
        let text = FormDraft {
            width: "12 mm",
            ..draft()
        };
        assert_eq!(
            messages(&text),
            [(
                FormField::Width,
                "Enter a number from 1 to 100000 mm".to_string()
            )]
        );
        let cm = FormDraft {
            width: "0.05",
            unit: PresetUnit::Cm,
            ..draft()
        };
        assert_eq!(
            messages(&cm),
            [(
                FormField::Width,
                "Enter a number from 0.1 to 10000 cm".to_string()
            )]
        );
        let inches = FormDraft {
            width: "5000",
            unit: PresetUnit::In,
            ..draft()
        };
        assert_eq!(
            messages(&inches)[0].1,
            "Enter a number from 0.04 to 3937 in"
        );
        let px = FormDraft {
            width: "2",
            unit: PresetUnit::Px,
            ..draft()
        };
        assert_eq!(
            messages(&px)[0].1,
            "Enter a number from 3.78 to 377952.75 px"
        );
        let same = FormDraft {
            width: "21",
            height: "29,7",
            unit: PresetUnit::Cm,
            ..draft()
        };
        assert_eq!(
            messages(&same),
            [(FormField::Width, "Same size as A4".to_string())]
        );
        let name_taken = FormDraft {
            name: "a5",
            ..draft()
        };
        assert_eq!(
            messages(&name_taken),
            [(FormField::Name, "This group already has A5".to_string())]
        );
    }

    /// Criterion 19: a new group needs a name, unique ignoring case.
    #[test]
    fn a_new_group_needs_a_unique_name() {
        let new = FormDraft {
            group_id: "",
            new_group_name: "",
            ..draft()
        };
        assert_eq!(
            messages(&new),
            [(FormField::GroupName, NAME_MESSAGE.to_string())]
        );
        let taken = FormDraft {
            group_id: "",
            new_group_name: "paper",
            ..draft()
        };
        assert_eq!(
            messages(&taken),
            [(
                FormField::GroupName,
                "A group of this name exists".to_string()
            )]
        );
        let ok = FormDraft {
            group_id: "",
            new_group_name: "Laser",
            new_group_orientation: Orientation::Landscape,
            ..draft()
        };
        let spec = check_format_form(&library(), &ok, None).unwrap();
        assert_eq!(
            spec.group,
            GroupTarget::New {
                name: "Laser".into(),
                orientation: Orientation::Landscape
            }
        );
    }

    /// The two sides are stored short and long, whatever the order typed.
    #[test]
    fn the_sides_are_stored_short_first() {
        let swapped = FormDraft {
            width: "50",
            height: "30",
            ..draft()
        };
        let spec = check_format_form(&library(), &swapped, None).unwrap();
        assert_eq!(
            (spec.short, spec.long, spec.unit),
            (30.0, 50.0, PresetUnit::Mm)
        );
        let comma = FormDraft {
            width: "30,5",
            ..draft()
        };
        assert_eq!(
            check_format_form(&library(), &comma, None).unwrap().short,
            30.5
        );
    }

    #[test]
    fn the_import_notice_counts_what_happened() {
        let report = |added, groups, skipped| {
            import_notice(&ImportReport {
                added,
                groups,
                skipped,
            })
        };
        assert_eq!(
            report(5, 2, 2),
            "Imported 5 formats in 2 groups. Skipped 2 that were already there."
        );
        assert_eq!(report(1, 1, 0), "Imported 1 format in 1 group.");
        assert_eq!(
            report(0, 0, 1),
            "Nothing new to import. Skipped 1 that was already there."
        );
        assert_eq!(
            import_refusal("a.toml", "made by a newer version"),
            "Could not import a.toml: made by a newer version. Nothing was changed."
        );
    }

    #[test]
    fn notices_name_the_format() {
        assert_eq!(added_notice(" Key ring "), "Added Key ring.");
        assert_eq!(saved_notice("Laser"), "Saved Laser.");
    }
}
