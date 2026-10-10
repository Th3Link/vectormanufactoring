//! What the Document section shows and does for the size presets
//! (`specs/0030-document-size-presets/` criteria 10 to 14, 16, 17): the pick
//! rule, the orientation swap, and the view of groups, buttons, tooltips and the
//! subject line. Pure functions of the preset list and the document size, so
//! the DOM holds no preset number, no name and no rule.

use curvyo_document_core::{
    DisplayUnit, DocumentPreset, DocumentSize, Orientation, PresetList, PresetUnit,
};

use crate::display_unit_text::format_field_length;

/// The size a press on the preset `id` sets: its sides in the orientation of
/// criterion 13. Staying inside the group of the current size keeps the current
/// orientation (a square counts as portrait); entering a group starts with its
/// default. `None` for an id the list does not hold.
#[must_use]
pub fn preset_pick_size(
    list: &PresetList,
    current: DocumentSize,
    id: &str,
) -> Option<DocumentSize> {
    let (group, preset) = list.presets().find(|(_, preset)| preset.id == id)?;
    let in_this_group = list
        .matching(current)
        .is_some_and(|(matched, _)| matched.id == group.id);
    let orientation = if in_this_group {
        Orientation::of(current).unwrap_or(Orientation::Portrait)
    } else {
        group.default_orientation
    };
    Some(oriented(preset, orientation))
}

/// The preset's sides as a document size in `orientation`.
fn oriented(preset: &DocumentPreset, orientation: Orientation) -> DocumentSize {
    match orientation {
        Orientation::Portrait => DocumentSize::new(preset.short_side, preset.long_side),
        Orientation::Landscape => DocumentSize::new(preset.long_side, preset.short_side),
    }
}

/// The size an Orientation press sets: width and height swapped. `None` when
/// the document already is in `wanted` or is square (criterion 14).
#[must_use]
pub fn orientation_swap(current: DocumentSize, wanted: Orientation) -> Option<DocumentSize> {
    match Orientation::of(current) {
        Some(now) if now != wanted => Some(DocumentSize::new(current.height, current.width)),
        _ => None,
    }
}

/// One preset button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetEntry {
    /// The preset's id, which the host sends back on a press.
    pub id: String,
    /// The button's label.
    pub name: String,
    /// The document has this preset's size (either orientation).
    pub pressed: bool,
    /// "Paper A4": the group name and the preset name.
    pub accessible_name: String,
    /// The text-only tooltip: the size a press sets, in the display unit, and
    /// the note on a second line.
    pub tooltip: String,
}

/// A heading and its buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetGroupView {
    /// The group's id.
    pub id: String,
    /// The heading.
    pub name: String,
    /// The buttons, in file order.
    pub entries: Vec<PresetEntry>,
}

/// Everything the presets part of the Document section shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetsView {
    /// The groups, in file order.
    pub groups: Vec<PresetGroupView>,
    /// The pressed orientation item; `None` for a square.
    pub orientation: Option<Orientation>,
    /// "A4, portrait", "16:9, landscape", the name alone for a square that
    /// matches a preset, or "Custom".
    pub subject: String,
}

/// The multiplication sign the status bar uses.
const TIMES: char = '\u{d7}';

/// The view for a document of `size` shown in `unit`.
#[must_use]
pub fn presets_view(list: &PresetList, size: DocumentSize, unit: DisplayUnit) -> PresetsView {
    let matched = list.matching(size).map(|(_, preset)| preset.id.clone());
    let groups = list
        .groups
        .iter()
        .map(|group| PresetGroupView {
            id: group.id.clone(),
            name: group.name.clone(),
            entries: group
                .presets
                .iter()
                .map(|preset| {
                    let press = preset_pick_size(list, size, &preset.id).unwrap_or(size);
                    PresetEntry {
                        id: preset.id.clone(),
                        name: preset.name.clone(),
                        pressed: matched.as_deref() == Some(preset.id.as_str()),
                        accessible_name: format!("{} {}", group.name, preset.name),
                        tooltip: tooltip(preset, press, unit),
                    }
                })
                .collect(),
        })
        .collect();
    let orientation = Orientation::of(size);
    let subject = match list.matching(size) {
        None => "Custom".to_string(),
        Some((_, preset)) => match orientation {
            Some(Orientation::Portrait) => format!("{}, portrait", preset.name),
            Some(Orientation::Landscape) => format!("{}, landscape", preset.name),
            None => preset.name.clone(),
        },
    };
    PresetsView {
        groups,
        orientation,
        subject,
    }
}

/// "210 × 297 mm", or for a pixel preset "1920 × 1080 px = 508 × 285.75 mm",
/// then the note on a second line.
fn tooltip(preset: &DocumentPreset, press: DocumentSize, unit: DisplayUnit) -> String {
    let landscape = press.width.as_mm() > press.height.as_mm();
    let size = format!(
        "{} {TIMES} {} {}",
        format_field_length(press.width, unit),
        format_field_length(press.height, unit),
        unit.symbol()
    );
    let mut text = if preset.authored.unit == PresetUnit::Px {
        let (short, long) = (preset.authored.short, preset.authored.long);
        let (width, height) = if landscape {
            (long, short)
        } else {
            (short, long)
        };
        format!("{width} {TIMES} {height} px = {size}")
    } else {
        size
    };
    if let Some(note) = &preset.note {
        text.push('\n');
        text.push_str(note);
    }
    text
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::Length;

    use super::*;

    fn list() -> PresetList {
        PresetList::shipped().unwrap()
    }

    fn mm(width: f64, height: f64) -> DocumentSize {
        DocumentSize::from_mm(width, height)
    }

    fn picked(current: DocumentSize, id: &str) -> (f64, f64) {
        let size = preset_pick_size(&list(), current, id).unwrap();
        (size.width.as_mm(), size.height.as_mm())
    }

    /// Criterion 13's table.
    #[test]
    fn the_orientation_after_a_pick() {
        assert_eq!(
            picked(mm(297.0, 210.0), "a3"),
            (420.0, 297.0),
            "A4 landscape, pick A3"
        );
        assert_eq!(
            picked(mm(210.0, 297.0), "slide-16-9"),
            (508.0, 285.75),
            "enter Slides"
        );
        assert_eq!(
            picked(mm(300.0, 400.0), "a4"),
            (210.0, 297.0),
            "custom portrait"
        );
        assert_eq!(
            picked(mm(400.0, 300.0), "a4"),
            (210.0, 297.0),
            "custom landscape"
        );
        assert_eq!(
            picked(mm(285.75, 508.0), "slide-16-10"),
            (317.5, 508.0),
            "16:9 turned to portrait, pick 16:10"
        );
        assert_eq!(picked(mm(210.0, 297.0), "a5"), (148.0, 210.0));
        assert_eq!(preset_pick_size(&list(), mm(210.0, 297.0), "nope"), None);
    }

    #[test]
    fn a_square_document_counts_as_portrait_inside_its_group() {
        let mut square = list();
        square.groups[0].presets[0].short_side = Length::from_mm(500.0);
        square.groups[0].presets[0].long_side = Length::from_mm(500.0);
        let size = preset_pick_size(&square, mm(500.0, 500.0), "a1").unwrap();
        assert_eq!(size.width.as_mm(), 594.0);
        assert_eq!(size.height.as_mm(), 841.0);
    }

    /// Criterion 14: swap, and nothing for the pressed item or a square.
    #[test]
    fn an_orientation_press_swaps_the_sides_or_does_nothing() {
        let swapped = orientation_swap(mm(210.0, 297.0), Orientation::Landscape).unwrap();
        assert_eq!(
            (swapped.width.as_mm(), swapped.height.as_mm()),
            (297.0, 210.0)
        );
        assert_eq!(
            orientation_swap(mm(210.0, 297.0), Orientation::Portrait),
            None
        );
        assert_eq!(
            orientation_swap(mm(100.0, 100.0), Orientation::Landscape),
            None
        );
        assert_eq!(
            orientation_swap(mm(100.0, 100.0), Orientation::Portrait),
            None
        );
        let custom = orientation_swap(mm(300.0, 400.0), Orientation::Landscape).unwrap();
        assert_eq!(
            (custom.width.as_mm(), custom.height.as_mm()),
            (400.0, 300.0)
        );
    }

    /// Criteria 10 and 10a.
    #[test]
    fn the_pressed_preset_and_the_subject_follow_the_size() {
        let unit = DisplayUnit::Mm;
        let pressed = |size| {
            presets_view(&list(), size, unit)
                .groups
                .iter()
                .flat_map(|g| g.entries.iter())
                .filter(|e| e.pressed)
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(pressed(mm(210.0, 297.0)), ["A4"]);
        assert_eq!(pressed(mm(297.0, 210.0)), ["A4"]);
        assert_eq!(pressed(mm(210.004, 297.0)), ["A4"]);
        assert_eq!(pressed(mm(211.0, 297.0)).len(), 0);
        assert_eq!(pressed(mm(508.0, 285.75)), ["16:9"]);
        assert_eq!(pressed(mm(285.75, 508.0)), ["16:9"]);
        let subject = |size| presets_view(&list(), size, unit).subject;
        assert_eq!(subject(mm(210.0, 297.0)), "A4, portrait");
        assert_eq!(subject(mm(297.0, 210.0)), "A4, landscape");
        assert_eq!(subject(mm(211.0, 297.0)), "Custom");
        assert_eq!(subject(mm(508.0, 285.75)), "16:9, landscape");
    }

    #[test]
    fn a_square_that_matches_a_preset_shows_the_name_alone() {
        let mut square = list();
        square.groups[0].presets[4].short_side = Length::from_mm(250.0);
        square.groups[0].presets[4].long_side = Length::from_mm(250.0);
        assert_eq!(
            presets_view(&square, mm(250.0, 250.0), DisplayUnit::Mm).subject,
            "A4"
        );
        assert_eq!(
            presets_view(&square, mm(250.0, 250.0), DisplayUnit::Mm).orientation,
            None
        );
    }

    /// Criteria 8, 16 and 17.
    #[test]
    fn buttons_are_in_file_order_with_names_and_tooltips() {
        let view = presets_view(&list(), mm(210.0, 297.0), DisplayUnit::Mm);
        assert_eq!(view.groups.len(), 2);
        assert_eq!(view.groups[0].name, "Paper");
        let a4 = &view.groups[0].entries[4];
        assert_eq!(a4.accessible_name, "Paper A4");
        assert_eq!(a4.tooltip, "210 \u{d7} 297 mm\nISO 216");
        let sixteen_nine = &view.groups[1].entries[0];
        assert_eq!(sixteen_nine.accessible_name, "Slides 16:9");
        assert_eq!(
            sixteen_nine.tooltip,
            "1920 \u{d7} 1080 px = 508 \u{d7} 285.75 mm\nFull HD"
        );
        assert_eq!(
            view.groups[1].entries[1].tooltip,
            "1920 \u{d7} 1200 px = 508 \u{d7} 317.5 mm"
        );
        assert_eq!(view.orientation, Some(Orientation::Portrait));
    }

    #[test]
    fn a_tooltip_names_the_size_a_press_would_set() {
        let landscape = presets_view(&list(), mm(297.0, 210.0), DisplayUnit::Mm);
        assert!(
            landscape.groups[0].entries[4]
                .tooltip
                .starts_with("297 \u{d7} 210 mm")
        );
        // From A4 portrait, 16:9 starts landscape; its pixel numbers follow.
        let portrait = presets_view(&list(), mm(210.0, 297.0), DisplayUnit::Mm);
        assert!(
            portrait.groups[1].entries[0]
                .tooltip
                .starts_with("1920 \u{d7} 1080 px")
        );
        // A 16:9 turned portrait: the press keeps portrait.
        let turned = presets_view(&list(), mm(285.75, 508.0), DisplayUnit::Mm);
        assert!(
            turned.groups[1].entries[0]
                .tooltip
                .starts_with("1080 \u{d7} 1920 px = 285.75 \u{d7} 508 mm")
        );
    }

    #[test]
    fn tooltips_use_the_display_unit() {
        let inches = presets_view(&list(), mm(210.0, 297.0), DisplayUnit::In);
        assert!(
            inches.groups[0].entries[4]
                .tooltip
                .starts_with("8.2677 \u{d7} 11.6929 in")
        );
        assert!(
            inches.groups[1].entries[0]
                .tooltip
                .starts_with("1920 \u{d7} 1080 px = 20 \u{d7} 11.25 in")
        );
    }

    /// Criterion 3, last clause: nine paper presets give nine entries.
    #[test]
    fn a_list_of_nine_paper_presets_has_nine_entries() {
        let mut nine = list();
        let template = nine.groups[0].presets[0].clone();
        for n in 0..2 {
            let mut extra = template.clone();
            extra.id = format!("x{n}");
            extra.name = format!("X{n}");
            extra.short_side = Length::from_mm(50.0 + f64::from(n));
            extra.long_side = Length::from_mm(60.0);
            nine.groups[0].presets.push(extra);
        }
        let view = presets_view(&nine, mm(210.0, 297.0), DisplayUnit::Mm);
        assert_eq!(view.groups[0].entries.len(), 9);
        assert_eq!(view.groups[1].entries.len(), 3);
    }

    #[test]
    fn an_empty_list_has_no_groups_and_a_custom_subject() {
        let view = presets_view(&PresetList::default(), mm(210.0, 297.0), DisplayUnit::Mm);
        assert_eq!(view.groups.len(), 0);
        assert_eq!(view.subject, "Custom");
        assert_eq!(view.orientation, Some(Orientation::Portrait));
    }
}
