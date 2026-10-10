//! The document size presets and their data file
//! (`specs/0030-document-size-presets/` criteria 1 to 8, Part A): the shipped
//! list is the table of the spec, the loader converts and validates, and every
//! rule of criterion 6 has a failing fixture that names the preset and the rule.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::path::PathBuf;

use curvyo_document_core::{
    DocumentSize, Orientation, PresetError, PresetList, PresetReason, PresetSubject, PresetUnit,
};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/document_presets")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn shipped() -> PresetList {
    PresetList::shipped().expect("the shipped file loads")
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9, "{a} vs {b}");
}

// ---- criteria 1 and 7: the shipped file ----------------------------------------

#[test]
fn the_shipped_presets_load() {
    assert!(PresetList::shipped().is_ok());
}

#[test]
fn the_shipped_presets_are_the_table_of_the_spec_in_order() {
    let list = shipped();
    assert_eq!(list.groups.len(), 2);
    let paper = &list.groups[0];
    assert_eq!(
        (paper.name.as_str(), paper.default_orientation),
        ("Paper", Orientation::Portrait)
    );
    let sides: Vec<(&str, f64, f64)> = paper
        .presets
        .iter()
        .map(|p| (p.name.as_str(), p.short_side.as_mm(), p.long_side.as_mm()))
        .collect();
    let want = [
        ("A0", 841.0, 1189.0),
        ("A1", 594.0, 841.0),
        ("A2", 420.0, 594.0),
        ("A3", 297.0, 420.0),
        ("A4", 210.0, 297.0),
        ("A5", 148.0, 210.0),
        ("A6", 105.0, 148.0),
    ];
    assert_eq!(sides.len(), want.len());
    for ((name, short, long), (want_name, want_short, want_long)) in sides.iter().zip(want) {
        assert_eq!(*name, want_name);
        near(*short, want_short);
        near(*long, want_long);
    }
    let slide_group = &list.groups[1];
    assert_eq!(
        (slide_group.name.as_str(), slide_group.default_orientation),
        ("Slides", Orientation::Landscape)
    );
    let slide_sides: Vec<(&str, f64, f64)> = slide_group
        .presets
        .iter()
        .map(|p| (p.name.as_str(), p.short_side.as_mm(), p.long_side.as_mm()))
        .collect();
    assert_eq!(slide_sides.len(), 3);
    near(slide_sides[0].1, 285.75);
    near(slide_sides[0].2, 508.0);
    near(slide_sides[1].1, 317.5);
    near(slide_sides[1].2, 508.0);
    near(slide_sides[2].1, 203.2);
    near(slide_sides[2].2, 1024.0 * 254.0 / 960.0);
}

#[test]
fn the_pixel_rule_is_a_ninety_sixth_of_an_inch() {
    let list = shipped();
    let sixteen_nine = &list.groups[1].presets[0];
    assert_eq!(sixteen_nine.long_side.as_mm(), 508.0);
    assert_eq!(sixteen_nine.short_side.as_mm(), 285.75);
    assert_eq!(list.groups[1].presets[1].short_side.as_mm(), 317.5);
    near(list.groups[1].presets[2].short_side.as_mm(), 203.2);
    assert_eq!(sixteen_nine.authored.unit, PresetUnit::Px);
    assert_eq!(sixteen_nine.authored.long, 1920.0);
    assert_eq!(sixteen_nine.note.as_deref(), Some("Full HD"));
}

#[test]
fn inches_convert_as_254_over_10() {
    let text = "format = 1\n[[group]]\nid = \"us\"\nname = \"US\"\ndefault_orientation = \"portrait\"\n\
        [[group.preset]]\nid = \"letter\"\nname = \"Letter\"\nshort_side = 8.5\nlong_side = 11\nunit = \"in\"\n";
    let list = PresetList::parse(text).unwrap();
    let letter = &list.groups[0].presets[0];
    assert_eq!(letter.short_side.as_mm(), 215.9);
    assert_eq!(letter.long_side.as_mm(), 279.4);
}

// ---- criterion 2: no number in a source file ---------------------------------------

#[test]
fn no_preset_size_is_written_in_a_source_file() {
    fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|n| n != "node_modules" && n != "wasm-bindings")
                {
                    walk(&path, hits);
                }
            } else if path
                .extension()
                .is_some_and(|e| e == "rs" || e == "ts" || e == "tsx")
            {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                if text.contains("1189") && text.contains("841") && !text.contains("#[cfg(test)]") {
                    hits.push(path.display().to_string());
                }
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut hits = Vec::new();
    for dir in [
        "curvyo-document-core/src",
        "curvyo-geometry-core/src",
        "curvyo-render-core/src",
        "curvyo-ui-core/src",
        "curvyo-editor-wasm/src",
        "frontend/src",
    ] {
        walk(&root.join(dir), &mut hits);
    }
    assert!(hits.is_empty(), "{hits:?}");
}

// ---- criterion 3: adding a block adds a button ------------------------------------------

#[test]
fn one_more_block_in_a_group_is_one_more_preset_at_that_place() {
    let text = include_str!("../data/document-presets.toml");
    let extra = "  [[group.preset]]\n  id = \"b5\"\n  name = \"B5\"\n  short_side = 176\n  long_side = 250\n  unit = \"mm\"\n";
    // Insert the block after A6, at the end of the Paper group.
    let marker = "\n[[group]]\nid = \"slides\"";
    let at = text.find(marker).unwrap();
    let edited = format!("{}\n{}{}", &text[..at], extra, &text[at..]);
    let list = PresetList::parse(&edited).unwrap();
    let before = shipped();
    assert_eq!(
        list.groups[0].presets.len(),
        before.groups[0].presets.len() + 1
    );
    assert_eq!(list.groups[0].presets[7].name, "B5");
    assert_eq!(list.groups[1], before.groups[1]);
}

// ---- criterion 8: file order -------------------------------------------------------------

#[test]
fn groups_and_presets_keep_file_order() {
    let list = shipped();
    let names: Vec<&str> = list.presets().map(|(_, p)| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "A0", "A1", "A2", "A3", "A4", "A5", "A6", "16:9", "16:10", "4:3"
        ]
    );
}

// ---- criterion 6: one failing fixture per rule -----------------------------------------------

fn error_of(name: &str) -> PresetError {
    PresetList::parse(&fixture(name)).expect_err(name)
}

fn preset_error(name: &str, id: &str, reason: &PresetReason) {
    let error = error_of(name);
    assert_eq!(
        error.subject,
        PresetSubject::Preset(id.to_string()),
        "{name}"
    );
    assert_eq!(&error.reason, reason, "{name}");
}

fn group_error(name: &str, subject: &str, reason: &PresetReason) {
    let error = error_of(name);
    assert_eq!(
        error.subject,
        PresetSubject::Group(subject.to_string()),
        "{name}"
    );
    assert_eq!(&error.reason, reason, "{name}");
}

#[test]
fn rule_a_text_that_is_not_toml_or_has_an_unknown_key_is_a_syntax_error() {
    for name in [
        "6a_not_toml.toml",
        "6a_unknown_key.toml",
        "6a_unknown_preset_key.toml",
    ] {
        let error = error_of(name);
        assert_eq!(error.subject, PresetSubject::File, "{name}");
        assert!(matches!(error.reason, PresetReason::Syntax(_)), "{name}");
    }
}

#[test]
fn rule_b_format_must_be_one() {
    for name in ["6b_format_missing.toml", "6b_format_two.toml"] {
        let error = error_of(name);
        assert_eq!(
            (error.subject, error.reason),
            (PresetSubject::File, PresetReason::FormatNotOne)
        );
    }
}

#[test]
fn rule_c_groups() {
    group_error(
        "6c_group_no_id.toml",
        "#0",
        &PresetReason::MissingField("id"),
    );
    group_error(
        "6c_group_no_name.toml",
        "paper",
        &PresetReason::MissingField("name"),
    );
    group_error(
        "6c_group_no_orientation.toml",
        "paper",
        &PresetReason::MissingField("default_orientation"),
    );
    group_error(
        "6c_group_bad_orientation.toml",
        "paper",
        &PresetReason::BadOrientation("square".to_string()),
    );
    group_error("6c_group_empty.toml", "paper", &PresetReason::EmptyGroup);
    group_error(
        "6c_group_duplicate_id.toml",
        "paper",
        &PresetReason::DuplicateGroupId,
    );
}

#[test]
fn rule_d_presets() {
    preset_error(
        "6d_preset_no_id.toml",
        "#0",
        &PresetReason::MissingField("id"),
    );
    preset_error(
        "6d_preset_no_name.toml",
        "a4",
        &PresetReason::MissingField("name"),
    );
    preset_error(
        "6d_preset_no_short.toml",
        "a4",
        &PresetReason::MissingField("short_side"),
    );
    preset_error(
        "6d_preset_no_long.toml",
        "a4",
        &PresetReason::MissingField("long_side"),
    );
    preset_error(
        "6d_preset_no_unit.toml",
        "a4",
        &PresetReason::MissingField("unit"),
    );
    preset_error(
        "6d_preset_duplicate_id.toml",
        "a4",
        &PresetReason::DuplicatePresetId,
    );
}

#[test]
fn rule_e_the_unit_is_mm_in_or_px() {
    preset_error(
        "6e_unit_cm.toml",
        "a4",
        &PresetReason::UnknownUnit("cm".to_string()),
    );
}

#[test]
fn rule_f_sides() {
    for name in [
        "6f_side_zero.toml",
        "6f_side_negative.toml",
        "6f_side_nan.toml",
        "6f_side_inf.toml",
        "6f_side_text.toml",
    ] {
        preset_error(name, "a4", &PresetReason::SideNotPositive);
    }
    preset_error("6f_short_longer.toml", "a4", &PresetReason::ShortSideLonger);
    for name in [
        "6f_side_too_small.toml",
        "6f_side_too_large.toml",
        "6f_px_too_small.toml",
    ] {
        preset_error(name, "a4", &PresetReason::SideOutOfRange);
    }
}

#[test]
fn rule_g_ids_names_and_notes() {
    for name in [
        "6g_id_upper_case.toml",
        "6g_id_underscore.toml",
        "6g_id_empty.toml",
    ] {
        let error = error_of(name);
        assert!(matches!(error.subject, PresetSubject::Preset(_)), "{name}");
        assert_eq!(error.reason, PresetReason::BadId, "{name}");
    }
    preset_error("6g_name_empty.toml", "a4", &PresetReason::BadName);
    preset_error("6g_name_long.toml", "a4", &PresetReason::BadName);
    preset_error("6g_note_long.toml", "a4", &PresetReason::NoteTooLong);
    group_error("6g_group_id_bad.toml", "Paper", &PresetReason::BadId);
}

#[test]
fn rule_h_no_two_presets_have_the_same_size() {
    preset_error(
        "6h_same_size.toml",
        "a4-again",
        &PresetReason::SameSizeAs("a4".to_string()),
    );
    preset_error(
        "6h_same_size_in_inches.toml",
        "letter",
        &PresetReason::SameSizeAs("a4".to_string()),
    );
}

// ---- the selected state's rule (criterion 10) ------------------------------------------------

#[test]
fn a_size_matches_a_preset_in_either_orientation_within_a_hundredth_of_a_millimetre() {
    let list = shipped();
    let name = |size: DocumentSize| list.matching(size).map(|(_, p)| p.name.clone());
    assert_eq!(
        name(DocumentSize::from_mm(210.0, 297.0)).as_deref(),
        Some("A4")
    );
    assert_eq!(
        name(DocumentSize::from_mm(297.0, 210.0)).as_deref(),
        Some("A4")
    );
    assert_eq!(
        name(DocumentSize::from_mm(210.004, 297.0)).as_deref(),
        Some("A4")
    );
    assert_eq!(name(DocumentSize::from_mm(211.0, 297.0)), None);
    assert_eq!(
        name(DocumentSize::from_mm(508.0, 285.75)).as_deref(),
        Some("16:9")
    );
    assert_eq!(
        name(DocumentSize::from_mm(285.75, 508.0)).as_deref(),
        Some("16:9")
    );
    assert_eq!(name(DocumentSize::from_mm(210.0, 210.0)), None);
}

#[test]
fn orientation_of_a_size() {
    assert_eq!(
        Orientation::of(DocumentSize::from_mm(210.0, 297.0)),
        Some(Orientation::Portrait)
    );
    assert_eq!(
        Orientation::of(DocumentSize::from_mm(297.0, 210.0)),
        Some(Orientation::Landscape)
    );
    assert_eq!(Orientation::of(DocumentSize::from_mm(100.0, 100.005)), None);
}
