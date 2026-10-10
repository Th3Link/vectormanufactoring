//! The "All formats" list of the Document tab (`specs/0045-document-formats-
//! library/` criteria 13 and 17): every group of the library with its formats,
//! the counts, which buttons exist and the accessible names. A pure function of
//! the library and the document, so the DOM holds no format and no rule.

use curvyo_document_core::{
    DisplayUnit, DocumentPreset, DocumentSize, Orientation, PresetGroup, PresetList,
};

use crate::display_unit_text::format_field_length;

/// The multiplication sign the status bar uses.
const TIMES: char = '\u{d7}';

/// One format row. The flags are the host's switches (which control exists,
/// which look applies), each decided here.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FormatRowView {
    /// The format's id, which the host sends back on a press.
    pub id: String,
    /// The name.
    pub name: String,
    /// "210 × 297 mm" in the display unit, in the order of the group's default
    /// orientation (so the text does not change with the document).
    pub size_text: String,
    /// "Apply A4, 210 × 297 mm".
    pub apply_name: String,
    /// "Quick selection: A4": constant, the state is in `favourite`.
    pub star_name: String,
    /// The format is in the quick selection.
    pub favourite: bool,
    /// The star exists (the library can be written).
    pub can_star: bool,
    /// The document has this format's size: the selected row look.
    pub selected: bool,
    /// Edit and Delete exist: a format of the maker's, with a writable library.
    pub can_edit: bool,
    /// "Edit Key ring".
    pub edit_name: String,
    /// "Delete Key ring".
    pub delete_name: String,
    /// "Delete Key ring?".
    pub delete_prompt: String,
}

/// One group with its header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FormatGroupView {
    /// The group's id.
    pub id: String,
    /// The name.
    pub name: String,
    /// The number of formats, or "Off" for a group that is off.
    pub count_text: String,
    /// The group is on.
    pub enabled: bool,
    /// The Show switch exists (the library can be written).
    pub can_toggle: bool,
    /// "Show group Slides".
    pub show_name: String,
    /// The fold button's name: "Fold group Paper".
    pub fold_name: String,
    /// Edit and Delete exist: a group of the maker's, with a writable library.
    pub can_edit: bool,
    /// "Edit group Laser".
    pub edit_name: String,
    /// "Delete group Laser".
    pub delete_name: String,
    /// "Delete group Laser and its 2 formats?".
    pub delete_prompt: String,
    /// "portrait" or "landscape": the group's `Opens as`, for its edit form.
    pub orientation: &'static str,
    /// The formats; none for a group that is off.
    pub rows: Vec<FormatRowView>,
}

/// Everything the list shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatListView {
    /// "14 formats": the formats of the groups that are on.
    pub total_text: String,
    /// The groups, in library order.
    pub groups: Vec<FormatGroupView>,
    /// Add format and Import exist (the library can be written).
    pub can_add: bool,
    /// Export exists: the library can be written and holds a format of the
    /// maker's.
    pub can_export: bool,
}

/// The list for a document of `size` shown in `unit`. `writable` is false while
/// the user file is broken: then no button that writes exists.
#[must_use]
pub fn format_list_view(
    list: &PresetList,
    size: DocumentSize,
    unit: DisplayUnit,
    writable: bool,
) -> FormatListView {
    let current = list.matching(size).map(|(_, preset)| preset.id.clone());
    let on_count: usize = list
        .groups
        .iter()
        .filter(|group| group.enabled)
        .map(|group| group.presets.len())
        .sum();
    FormatListView {
        total_text: if on_count == 1 {
            "1 format".to_string()
        } else {
            format!("{on_count} formats")
        },
        groups: list
            .groups
            .iter()
            .map(|group| group_view(group, current.as_deref(), unit, writable))
            .collect(),
        can_add: writable,
        can_export: writable && list.presets().any(|(_, preset)| preset.user),
    }
}

fn group_view(
    group: &PresetGroup,
    current: Option<&str>,
    unit: DisplayUnit,
    writable: bool,
) -> FormatGroupView {
    let count = group.presets.len();
    FormatGroupView {
        id: group.id.clone(),
        name: group.name.clone(),
        count_text: if group.enabled {
            count.to_string()
        } else {
            "Off".to_string()
        },
        enabled: group.enabled,
        can_toggle: writable,
        show_name: format!("Show group {}", group.name),
        fold_name: format!("Fold group {}", group.name),
        can_edit: writable && group.user,
        edit_name: format!("Edit group {}", group.name),
        delete_name: format!("Delete group {}", group.name),
        delete_prompt: format!(
            "Delete group {} and its {count} {}?",
            group.name,
            if count == 1 { "format" } else { "formats" }
        ),
        orientation: match group.default_orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        },
        rows: if group.enabled {
            group
                .presets
                .iter()
                .map(|preset| row_view(group, preset, current, unit, writable))
                .collect()
        } else {
            Vec::new()
        },
    }
}

fn row_view(
    group: &PresetGroup,
    preset: &DocumentPreset,
    current: Option<&str>,
    unit: DisplayUnit,
    writable: bool,
) -> FormatRowView {
    let (first, second) = match group.default_orientation {
        Orientation::Portrait => (preset.short_side, preset.long_side),
        Orientation::Landscape => (preset.long_side, preset.short_side),
    };
    let size_text = format!(
        "{} {TIMES} {} {}",
        format_field_length(first, unit),
        format_field_length(second, unit),
        unit.symbol()
    );
    FormatRowView {
        id: preset.id.clone(),
        name: preset.name.clone(),
        apply_name: format!("Apply {}, {size_text}", preset.name),
        star_name: format!("Quick selection: {}", preset.name),
        favourite: preset.favourite,
        can_star: writable,
        selected: current == Some(preset.id.as_str()),
        can_edit: writable && preset.user,
        edit_name: format!("Edit {}", preset.name),
        delete_name: format!("Delete {}", preset.name),
        delete_prompt: format!("Delete {}?", preset.name),
        size_text,
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{FormatLibrary, FormatSpec, GroupTarget, PresetUnit};

    use super::*;

    fn mm(width: f64, height: f64) -> DocumentSize {
        DocumentSize::from_mm(width, height)
    }

    fn shipped() -> FormatLibrary {
        FormatLibrary::shipped().expect("the shipped file loads")
    }

    /// Criterion 13: a group that is on lists its rows, one that is off its
    /// header only with the word "Off".
    #[test]
    fn groups_that_are_on_list_rows_and_groups_that_are_off_do_not() {
        let library = shipped();
        let view = format_list_view(library.list(), mm(210.0, 297.0), DisplayUnit::Mm, true);
        assert_eq!(view.total_text, "7 formats");
        assert_eq!(view.groups.len(), 2);
        let paper = &view.groups[0];
        assert_eq!(
            (paper.name.as_str(), paper.count_text.as_str()),
            ("Paper", "7")
        );
        assert_eq!(paper.rows.len(), 7);
        assert!(!paper.can_edit && paper.can_toggle);
        let slides = &view.groups[1];
        assert_eq!(slides.count_text, "Off");
        assert!(slides.rows.is_empty() && !slides.enabled);
        assert_eq!(slides.show_name, "Show group Slides");
        assert!(!view.can_export, "no format of the maker's yet");
    }

    /// Criteria 13, 14 and 17: names, sizes, the current row, built-ins have no
    /// edit or delete.
    #[test]
    fn a_row_has_its_names_its_size_and_the_selected_look_of_the_current_size() {
        let library = shipped();
        let view = format_list_view(library.list(), mm(297.0, 210.0), DisplayUnit::Mm, true);
        let a4 = &view.groups[0].rows[4];
        assert_eq!(a4.apply_name, "Apply A4, 210 \u{d7} 297 mm");
        assert_eq!(a4.star_name, "Quick selection: A4");
        assert_eq!(a4.size_text, "210 \u{d7} 297 mm");
        assert!(a4.selected && a4.favourite && a4.can_star && !a4.can_edit);
        assert_eq!(view.groups[0].rows.iter().filter(|r| r.selected).count(), 1);
        let inches = format_list_view(library.list(), mm(210.0, 297.0), DisplayUnit::In, true);
        assert_eq!(
            inches.groups[0].rows[4].size_text,
            "8.2677 \u{d7} 11.6929 in"
        );
    }

    /// The row of a format that is not a favourite is still the current one
    /// (criterion 11).
    #[test]
    fn the_current_row_shows_also_without_a_star() {
        let mut library = shipped();
        library.set_favourite("a3", false).unwrap();
        let view = format_list_view(library.list(), mm(297.0, 420.0), DisplayUnit::Mm, true);
        let a3 = &view.groups[0].rows[3];
        assert!(a3.selected && !a3.favourite);
    }

    /// A landscape group lists long side first, whatever the document does.
    #[test]
    fn a_landscape_group_lists_the_long_side_first() {
        let mut library = shipped();
        library.set_group_enabled("slides", true).unwrap();
        let view = format_list_view(library.list(), mm(210.0, 297.0), DisplayUnit::Mm, true);
        assert_eq!(view.groups[1].rows[0].size_text, "508 \u{d7} 285.75 mm");
        assert_eq!(view.total_text, "10 formats");
    }

    /// Criteria 6 and 22: the maker's formats and groups have edit and delete;
    /// a broken file removes every button that writes.
    #[test]
    fn the_makers_entries_can_be_edited_and_a_broken_file_hides_every_write() {
        let mut library = shipped();
        library
            .add_format(&FormatSpec {
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
        let view = format_list_view(library.list(), mm(210.0, 297.0), DisplayUnit::Mm, true);
        let laser = &view.groups[2];
        assert!(laser.can_edit && view.can_export && view.can_add);
        assert_eq!(laser.delete_prompt, "Delete group Laser and its 1 format?");
        assert_eq!(laser.rows[0].delete_prompt, "Delete Key ring?");
        assert_eq!(laser.rows[0].size_text, "50 \u{d7} 30 mm");
        assert!(laser.rows[0].can_edit);
        let broken = format_list_view(library.list(), mm(210.0, 297.0), DisplayUnit::Mm, false);
        let laser = &broken.groups[2];
        assert!(!laser.can_edit && !laser.can_toggle && !laser.rows[0].can_edit);
        assert!(!laser.rows[0].can_star && !broken.can_add && !broken.can_export);
    }
}
