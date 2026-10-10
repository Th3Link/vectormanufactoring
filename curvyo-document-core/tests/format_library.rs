//! The document formats library (`specs/0045-document-formats-library/`
//! criteria 1 to 7, 15, 16, 20 to 22, 24, 26, 27, 32 to 34): the built-in file
//! with the user file over it, the edits, the golden fixtures in
//! `tests/fixtures/formats/`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::format_push_string)]

use curvyo_document_core::{
    DocumentSize, FormatError, FormatField, FormatLibrary, FormatReason, FormatSpec, GroupTarget,
    MAX_USER_FILE_BYTES, MAX_USER_FORMATS, MAX_USER_GROUPS, Orientation, PresetList, PresetReason,
    PresetSubject, PresetUnit, describe_error,
};

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/formats/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn builtin() -> PresetList {
    PresetList::shipped().expect("the shipped file loads")
}

fn library(user: &str) -> FormatLibrary {
    FormatLibrary::load(builtin(), user).expect("the user file loads")
}

fn fails(user: &str) -> (PresetSubject, PresetReason) {
    let error = FormatLibrary::load(builtin(), user).expect_err("must be refused");
    (error.subject, error.reason)
}

fn ids(library: &FormatLibrary, group: &str) -> Vec<String> {
    library
        .list()
        .groups
        .iter()
        .find(|g| g.id == group)
        .map(|g| g.presets.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default()
}

fn favourites(library: &FormatLibrary) -> Vec<String> {
    library
        .list()
        .presets()
        .filter(|(_, p)| p.favourite)
        .map(|(_, p)| p.id.clone())
        .collect()
}

fn spec(name: &str, group: &str, short: f64, long: f64, unit: PresetUnit) -> FormatSpec {
    FormatSpec {
        name: name.to_string(),
        group: GroupTarget::Existing(group.to_string()),
        short,
        long,
        unit,
        favourite: true,
    }
}

// ---- Part A: the data

/// Criteria 1 and 7, and 32: the shipped file.
#[test]
fn the_shipped_file_has_paper_on_and_slides_off_and_no_user_ids() {
    let list = builtin();
    let paper = &list.groups[0];
    let slides = &list.groups[1];
    assert!(paper.enabled && !slides.enabled);
    assert_eq!(paper.presets.iter().filter(|p| p.favourite).count(), 7);
    assert_eq!(slides.presets.iter().filter(|p| p.favourite).count(), 3);
    assert!(
        list.presets()
            .all(|(g, p)| !g.id.starts_with("u-") && !p.id.starts_with("u-"))
    );
    assert!(list.presets().all(|(g, p)| !g.user && !p.user));
}

/// Criterion 1: the other built-in schema numbers, the cm unit.
#[test]
fn the_builtin_reader_refuses_other_formats_and_knows_cm() {
    let text = "format = 1\n";
    assert_eq!(
        PresetList::parse(text).unwrap_err().reason,
        PresetReason::FormatNotSupported
    );
    let cm = "format = 2\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n\
              [[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=9\nlong_side=12.5\nunit=\"cm\"\n";
    let list = PresetList::parse(cm).unwrap();
    assert!((list.groups[0].presets[0].long_side.as_mm() - 125.0).abs() < 1e-9);
    assert!(list.groups[0].presets[0].favourite && list.groups[0].enabled);
}

/// Criteria 2 and 4: the valid fixture, laid over the shipped list.
#[test]
fn the_valid_user_file_is_laid_over_the_built_in_list() {
    let lib = library(&fixture("valid.toml"));
    let groups: Vec<_> = lib.list().groups.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(groups, ["paper", "slides", "u-laser"]);
    // 4a: the format is appended after the built-in ones; the group stays built-in.
    assert_eq!(ids(&lib, "paper").last().map(String::as_str), Some("u-b5"));
    assert!(!lib.list().groups[0].user && lib.list().groups[2].user);
    // `enabled = { slides = true }`.
    assert!(lib.list().groups[1].enabled);
    // Once `favourites` exists it is the whole list (criterion 34).
    assert_eq!(favourites(&lib), ["a3", "a4", "u-keyring", "u-coaster-90"]);
    // The unit cm and the note.
    let coaster = lib.list().find("u-coaster-90").unwrap().1;
    assert!((coaster.short_side.as_mm() - 90.0).abs() < 1e-9);
    assert_eq!(coaster.note.as_deref(), Some("Cork"));
    assert!(coaster.user);
}

/// Criterion 34: without the `favourites` key the built-in flags apply, and a
/// user format is not a favourite.
#[test]
fn without_a_favourites_key_the_builtin_flags_apply() {
    let lib = library(
        "format = 1\n[[group]]\nid = \"paper\"\n[[group.preset]]\nid = \"u-b5\"\nname = \"B5\"\n\
         short_side = 176\nlong_side = 250\nunit = \"mm\"\n",
    );
    assert_eq!(favourites(&lib).len(), 10);
    assert!(!favourites(&lib).contains(&"u-b5".to_string()));
}

/// Criterion 4e: unknown ids in the state entries are ignored without error.
#[test]
fn state_entries_that_name_nothing_are_ignored() {
    let lib = library(
        "format = 1\nfavourites = [\"a4\", \"gone\"]\nenabled = { slides = true, gone = false }\n",
    );
    assert_eq!(favourites(&lib), ["a4"]);
    assert!(lib.list().groups[1].enabled);
    let text = lib.user_text().unwrap();
    assert!(!text.contains("gone"), "{text}");
}

/// Criterion 5 and 33: one failing fixture per rule.
#[test]
fn every_rule_has_a_failing_fixture() {
    use PresetReason as R;
    use PresetSubject::{File, Group, Preset};
    let table: Vec<(&str, PresetSubject, PresetReason)> = vec![
        ("fail_syntax.toml", File, R::Syntax(String::new())),
        ("fail_unknown_key.toml", File, R::Syntax(String::new())),
        ("fail_format_newer.toml", File, R::FormatNewer),
        ("fail_format_missing.toml", File, R::MissingField("format")),
        (
            "fail_bad_group_id.toml",
            Group("Laser Group".into()),
            R::BadId,
        ),
        (
            "fail_group_missing_name.toml",
            Group("u-laser".into()),
            R::MissingField("name"),
        ),
        (
            "fail_bad_orientation.toml",
            Group("u-laser".into()),
            R::BadOrientation("sideways".into()),
        ),
        (
            "fail_builtin_group_renamed.toml",
            Group("paper".into()),
            R::GroupMismatch,
        ),
        (
            "fail_builtin_group_orientation.toml",
            Group("paper".into()),
            R::GroupMismatch,
        ),
        (
            "fail_duplicate_group.toml",
            Group("u-laser".into()),
            R::DuplicateGroupId,
        ),
        ("fail_bad_preset_id.toml", Preset("B5!".into()), R::BadId),
        ("fail_bad_name.toml", Preset("u-b5".into()), R::BadName),
        ("fail_name_too_long.toml", Preset("u-b5".into()), R::BadName),
        (
            "fail_note_too_long.toml",
            Preset("u-b5".into()),
            R::NoteTooLong,
        ),
        (
            "fail_side_not_positive.toml",
            Preset("u-b5".into()),
            R::SideNotPositive,
        ),
        (
            "fail_short_longer.toml",
            Preset("u-b5".into()),
            R::ShortSideLonger,
        ),
        (
            "fail_side_out_of_range.toml",
            Preset("u-huge".into()),
            R::SideOutOfRange,
        ),
        (
            "fail_unknown_unit.toml",
            Preset("u-b5".into()),
            R::UnknownUnit("furlong".into()),
        ),
        (
            "fail_id_of_builtin.toml",
            Preset("a4".into()),
            R::DuplicatePresetId,
        ),
        (
            "fail_duplicate_preset_id.toml",
            Preset("u-b5".into()),
            R::DuplicatePresetId,
        ),
        (
            "fail_same_size.toml",
            Preset("u-my-a4".into()),
            R::SameSizeAs("a4".into()),
        ),
        (
            "fail_favourite_in_block.toml",
            File,
            R::Syntax(String::new()),
        ),
    ];
    for (name, subject, reason) in table {
        let (got_subject, got_reason) = fails(&fixture(name));
        assert_eq!(got_subject, subject, "{name}");
        // The parser's own message is not part of the contract.
        if matches!(reason, R::Syntax(_)) {
            assert!(matches!(got_reason, R::Syntax(_)), "{name}: {got_reason:?}");
        } else {
            assert_eq!(got_reason, reason, "{name}");
        }
    }
}

/// Criterion 5: the limits, and that the failing file is described.
#[test]
fn the_limits_of_200_formats_20_groups_and_256_kib() {
    let many = |n: usize| {
        let mut text = String::from("format = 1\n[[group]]\nid = \"paper\"\n");
        for i in 0..n {
            text.push_str(&format!(
                "[[group.preset]]\nid = \"u-f{i}\"\nname = \"F{i}\"\nshort_side = {}\nlong_side = 500\nunit = \"mm\"\n",
                10 + i
            ));
        }
        text
    };
    assert_eq!(
        library(&many(MAX_USER_FORMATS)).user_format_count(),
        MAX_USER_FORMATS
    );
    assert_eq!(
        fails(&many(MAX_USER_FORMATS + 1)).1,
        PresetReason::TooManyFormats
    );
    let groups = |n: usize| {
        let mut text = String::from("format = 1\n");
        for i in 0..n {
            text.push_str(&format!(
                "[[group]]\nid = \"u-g{i}\"\nname = \"G{i}\"\ndefault_orientation = \"portrait\"\n"
            ));
        }
        text
    };
    assert!(FormatLibrary::load(builtin(), &groups(MAX_USER_GROUPS)).is_ok());
    assert_eq!(
        fails(&groups(MAX_USER_GROUPS + 1)).1,
        PresetReason::TooManyGroups
    );
    let big = format!("format = 1\n# {}\n", "x".repeat(MAX_USER_FILE_BYTES));
    assert_eq!(fails(&big).1, PresetReason::FileTooLarge);
}

#[test]
fn errors_are_described_with_the_id_and_the_reason() {
    let error = FormatLibrary::load(builtin(), &fixture("fail_same_size.toml")).unwrap_err();
    assert_eq!(
        describe_error(&error),
        "format \"u-my-a4\": same size as \"a4\""
    );
    let newer = FormatLibrary::load(builtin(), "format = 3\n").unwrap_err();
    assert_eq!(describe_error(&newer), "made by a newer version");
}

// ---- Part F: the file text

/// Criterion 33: the golden file, and read-write-read is the identity.
#[test]
fn the_written_file_is_canonical_and_reads_back_to_itself() {
    let lib = library(&fixture("valid.toml"));
    let text = lib.user_text().unwrap();
    assert_eq!(text, fixture("valid.canonical.toml"));
    let again = library(&text);
    assert_eq!(again.user_text().unwrap(), text);
    assert_eq!(again.list(), lib.list());
}

#[test]
fn a_library_without_a_user_file_writes_a_bare_header() {
    let lib = FormatLibrary::from_builtin(builtin());
    assert_eq!(lib.user_text().unwrap(), "format = 1\n");
    assert_eq!(lib.user_format_count(), 0);
}

// ---- Parts B to D: the edits

/// Criteria 20 and 32: add a format; ids are `u-` and a slug, deterministic.
#[test]
fn adding_formats_gives_deterministic_ids_and_the_star() {
    let mut lib = FormatLibrary::from_builtin(builtin());
    let id = lib
        .add_format(&FormatSpec {
            group: GroupTarget::New {
                name: "Laser".into(),
                orientation: Orientation::Landscape,
            },
            ..spec("Key ring", "", 30.0, 50.0, PresetUnit::Mm)
        })
        .unwrap();
    assert_eq!(id, "u-key-ring");
    assert!(lib.list().find(&id).unwrap().1.favourite);
    // A second one of the same name in another group: `-2`.
    let second = lib
        .add_format(&spec("Key ring", "u-laser", 31.0, 51.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(second.reason, FormatReason::NameInGroup("Key ring".into()));
    let other = lib
        .add_format(&spec("Key ring", "paper", 31.0, 51.0, PresetUnit::Mm))
        .unwrap();
    assert_eq!(other, "u-key-ring-2");
    // The group id is a slug too; a name with no letters falls back.
    assert_eq!(lib.list().groups[2].id, "u-laser");
    let odd = lib
        .add_format(&spec("!!!", "u-laser", 40.0, 60.0, PresetUnit::Mm))
        .unwrap();
    assert_eq!(odd, "u-format");
    // The same input on the same library gives the same ids.
    let mut twin = FormatLibrary::from_builtin(builtin());
    let again = twin
        .add_format(&FormatSpec {
            group: GroupTarget::New {
                name: "Laser".into(),
                orientation: Orientation::Landscape,
            },
            ..spec("Key ring", "", 30.0, 50.0, PresetUnit::Mm)
        })
        .unwrap();
    assert_eq!(again, id);
    // The star is in the file and the document size of a format is exact.
    let key_ring = lib.list().find("u-key-ring").unwrap().1;
    assert!((key_ring.long_side.as_mm() - 50.0).abs() < 1e-9);
    assert!(lib.user_text().unwrap().contains("favourites"));
}

/// Criterion 20: each refusal names its field and reason; nothing changes.
#[test]
fn a_refused_format_changes_nothing() {
    let mut lib = library(&fixture("valid.toml"));
    let before = lib.user_text().unwrap();
    let refused = |lib: &mut FormatLibrary, spec: FormatSpec| -> FormatError {
        lib.add_format(&spec).expect_err("must be refused")
    };
    let e = refused(&mut lib, spec("", "paper", 30.0, 50.0, PresetUnit::Mm));
    assert_eq!(
        (e.field, e.reason),
        (FormatField::Name, FormatReason::BadName)
    );
    let long = "x".repeat(25);
    let e = refused(&mut lib, spec(&long, "paper", 30.0, 50.0, PresetUnit::Mm));
    assert_eq!(e.reason, FormatReason::BadName);
    let e = refused(&mut lib, spec("Tiny", "paper", 0.5, 50.0, PresetUnit::Mm));
    assert_eq!(
        (e.field, e.reason),
        (FormatField::Width, FormatReason::BadSize)
    );
    let e = refused(&mut lib, spec("Huge", "paper", 10.0, 1.0e6, PresetUnit::Cm));
    assert_eq!(e.reason, FormatReason::BadSize);
    let e = refused(
        &mut lib,
        spec("Nan", "paper", f64::NAN, 50.0, PresetUnit::Mm),
    );
    assert_eq!(e.reason, FormatReason::BadSize);
    let e = refused(&mut lib, spec("My A4", "paper", 21.0, 29.7, PresetUnit::Cm));
    assert_eq!(e.reason, FormatReason::SameSizeAs("A4".into()));
    // Slides is off but its sizes are still taken (criterion 5).
    let e = refused(
        &mut lib,
        spec("Wide", "paper", 1080.0, 1920.0, PresetUnit::Px),
    );
    assert_eq!(e.reason, FormatReason::SameSizeAs("16:9".into()));
    let e = refused(&mut lib, spec("b5", "paper", 100.0, 150.0, PresetUnit::Mm));
    assert_eq!(e.reason, FormatReason::NameInGroup("B5".into()));
    let e = refused(&mut lib, spec("X", "nowhere", 100.0, 150.0, PresetUnit::Mm));
    assert_eq!(e.reason, FormatReason::NotFound);
    let new_group = |name: &str| FormatSpec {
        group: GroupTarget::New {
            name: name.into(),
            orientation: Orientation::Portrait,
        },
        ..spec("X", "", 100.0, 150.0, PresetUnit::Mm)
    };
    let e = refused(&mut lib, new_group("laser"));
    assert_eq!(
        (e.field, e.reason),
        (FormatField::GroupName, FormatReason::GroupNameTaken)
    );
    let e = refused(&mut lib, new_group(""));
    assert_eq!(e.reason, FormatReason::BadName);
    assert_eq!(lib.user_text().unwrap(), before);
}

#[test]
fn the_list_is_full_at_200_formats_and_20_groups() {
    let mut lib = FormatLibrary::from_builtin(builtin());
    for i in 0..MAX_USER_FORMATS {
        let short = 10.0 + f64::from(u32::try_from(i).unwrap());
        lib.add_format(&spec(
            &format!("F{i}"),
            "paper",
            short,
            500.0,
            PresetUnit::Mm,
        ))
        .unwrap();
    }
    let e = lib
        .add_format(&spec("One more", "paper", 5.0, 500.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::LimitFormats);
    let mut groups = FormatLibrary::from_builtin(builtin());
    for i in 0..MAX_USER_GROUPS {
        groups
            .add_format(&FormatSpec {
                group: GroupTarget::New {
                    name: format!("G{i}"),
                    orientation: Orientation::Portrait,
                },
                ..spec(
                    "F",
                    "",
                    10.0 + f64::from(u32::try_from(i).unwrap()),
                    90.0,
                    PresetUnit::Mm,
                )
            })
            .unwrap();
    }
    let e = groups
        .add_format(&FormatSpec {
            group: GroupTarget::New {
                name: "Too many".into(),
                orientation: Orientation::Portrait,
            },
            ..spec("F", "", 500.0, 900.0, PresetUnit::Mm)
        })
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::LimitGroups);
}

/// Criterion 21: edit keeps the id and the star; the old size is no duplicate.
#[test]
fn editing_keeps_the_id_and_the_favourite_state() {
    let mut lib = library(&fixture("valid.toml"));
    // Unstar, then edit: still not a favourite afterwards.
    lib.set_favourite("u-keyring", false).unwrap();
    lib.edit_format(
        "u-keyring",
        &spec("Key ring", "u-laser", 30.0, 55.0, PresetUnit::Mm),
    )
    .unwrap();
    let (_, edited) = lib.list().find("u-keyring").unwrap();
    assert!(!edited.favourite);
    assert!((edited.long_side.as_mm() - 55.0).abs() < 1e-9);
    // Same size as itself is fine, as another format is not.
    lib.edit_format(
        "u-keyring",
        &spec("Key ring", "u-laser", 30.0, 55.0, PresetUnit::Mm),
    )
    .unwrap();
    let e = lib
        .edit_format(
            "u-keyring",
            &spec("Key ring", "u-laser", 9.0, 9.0, PresetUnit::Cm),
        )
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::SameSizeAs("Coaster".into()));
    // Moving it to another group keeps the id.
    lib.edit_format(
        "u-keyring",
        &spec("Key ring", "paper", 30.0, 55.0, PresetUnit::Mm),
    )
    .unwrap();
    assert!(ids(&lib, "paper").contains(&"u-keyring".to_string()));
    assert!(!ids(&lib, "u-laser").contains(&"u-keyring".to_string()));
    // Built-in formats are not editable or deletable.
    assert_eq!(
        lib.edit_format("a4", &spec("A4", "paper", 1.0, 2.0, PresetUnit::Cm))
            .unwrap_err()
            .reason,
        FormatReason::BuiltIn
    );
    assert_eq!(
        lib.delete_format("a4").unwrap_err().reason,
        FormatReason::BuiltIn
    );
}

/// Criterion 22: delete removes the format and its star; a group goes with its
/// formats.
#[test]
fn deleting_removes_the_star_and_the_group_takes_its_formats() {
    let mut lib = library(&fixture("valid.toml"));
    lib.delete_format("u-keyring").unwrap();
    assert!(lib.list().find("u-keyring").is_none());
    assert!(!lib.user_text().unwrap().contains("u-keyring"));
    lib.delete_group("u-laser").unwrap();
    assert!(lib.list().groups.iter().all(|g| g.id != "u-laser"));
    assert!(lib.list().find("u-coaster-90").is_none());
    assert_eq!(
        lib.delete_group("paper").unwrap_err().reason,
        FormatReason::BuiltIn
    );
    assert_eq!(lib.user_format_count(), 1);
    // The B5 appended to Paper is the last one; deleting it removes the entry.
    lib.delete_format("u-b5").unwrap();
    assert!(!lib.user_text().unwrap().contains("[[group]]"));
}

/// Criterion 24: rename a group and set its orientation.
#[test]
fn a_user_group_can_be_renamed() {
    let mut lib = library(&fixture("valid.toml"));
    lib.edit_group("u-laser", "Laser cut", Orientation::Portrait)
        .unwrap();
    let group = lib
        .list()
        .groups
        .iter()
        .find(|g| g.id == "u-laser")
        .unwrap();
    assert_eq!(group.name, "Laser cut");
    assert_eq!(group.default_orientation, Orientation::Portrait);
    assert_eq!(
        lib.edit_group("u-laser", "paper", Orientation::Portrait)
            .unwrap_err()
            .reason,
        FormatReason::GroupNameTaken
    );
    assert_eq!(
        lib.edit_group("paper", "Sheets", Orientation::Portrait)
            .unwrap_err()
            .reason,
        FormatReason::BuiltIn
    );
}

/// Criteria 15 and 16: a star materialises the list; a Show switch is kept only
/// where it differs from the default; both survive a round trip.
#[test]
fn stars_and_switches_are_written_and_read_back() {
    let mut lib = FormatLibrary::from_builtin(builtin());
    lib.set_favourite("a0", false).unwrap();
    assert_eq!(favourites(&lib).len(), 9);
    lib.set_group_enabled("slides", true).unwrap();
    lib.set_group_enabled("paper", false).unwrap();
    let text = lib.user_text().unwrap();
    assert!(
        text.contains("slides = true") && text.contains("paper = false"),
        "{text}"
    );
    let back = library(&text);
    assert_eq!(back.list(), lib.list());
    // Back to the default removes the entry.
    lib.set_group_enabled("paper", true).unwrap();
    assert!(!lib.user_text().unwrap().contains("paper"));
    assert_eq!(
        lib.set_favourite("nope", true).unwrap_err().reason,
        FormatReason::NotFound
    );
    assert_eq!(
        lib.set_group_enabled("nope", true).unwrap_err().reason,
        FormatReason::NotFound
    );
}

/// Criteria 11 and 14 at the model: a group that is off does not name a size.
#[test]
fn matching_ignores_groups_that_are_off() {
    let mut lib = FormatLibrary::from_builtin(builtin());
    let slide = DocumentSize::from_mm(508.0, 285.75);
    assert!(lib.list().matching(slide).is_none());
    lib.set_group_enabled("slides", true).unwrap();
    assert_eq!(lib.list().matching(slide).unwrap().1.name, "16:9");
}

// ---- Part E: share the list

/// Criterion 26: the export holds the maker's groups and formats and their
/// favourites only.
#[test]
fn the_export_is_the_user_groups_and_their_favourites() {
    assert_eq!(
        FormatLibrary::from_builtin(builtin())
            .export_text()
            .unwrap(),
        None
    );
    let text = library(&fixture("valid.toml"))
        .export_text()
        .unwrap()
        .unwrap();
    assert_eq!(text, fixture("export.expected.toml"));
    assert!(!text.contains("enabled"));
    // An export is a valid user file.
    assert!(FormatLibrary::load(builtin(), &text).is_ok());
}

/// Criterion 27: the merge, golden.
#[test]
fn an_import_merges_by_id_size_and_group() {
    let mut lib = library(&fixture("valid.toml"));
    let report = lib.import(&fixture("import.toml")).unwrap();
    assert_eq!((report.added, report.groups, report.skipped), (5, 3, 3));
    assert_eq!(lib.user_text().unwrap(), fixture("import.expected.toml"));
    // Built-in groups and the enabled choices are not changed.
    assert!(lib.list().groups[1].enabled);
    // The formats keep their groups: the Letter joined Paper under a new id.
    assert_eq!(
        ids(&lib, "paper").last().map(String::as_str),
        Some("u-letter")
    );
    // The same import again adds nothing.
    let again = lib.import(&fixture("import.toml")).unwrap();
    assert_eq!((again.added, again.groups), (0, 0));
    assert_eq!(lib.user_text().unwrap(), fixture("import.expected.toml"));
}

#[test]
fn a_refused_import_changes_nothing() {
    let mut lib = library(&fixture("valid.toml"));
    let before = lib.user_text().unwrap();
    for name in [
        "fail_syntax.toml",
        "fail_format_newer.toml",
        "fail_bad_preset_id.toml",
        "fail_builtin_group_renamed.toml",
    ] {
        assert!(lib.import(&fixture(name)).is_err(), "{name}");
    }
    assert_eq!(lib.user_text().unwrap(), before);
    // A file that would pass the limit is refused whole.
    let mut full = FormatLibrary::from_builtin(builtin());
    for i in 0..MAX_USER_FORMATS {
        let short = 10.0 + f64::from(u32::try_from(i).unwrap());
        full.add_format(&spec(
            &format!("F{i}"),
            "paper",
            short,
            500.0,
            PresetUnit::Mm,
        ))
        .unwrap();
    }
    let before = full.user_text().unwrap();
    let error = full.import(&fixture("import.toml")).unwrap_err();
    assert_eq!(error.reason, PresetReason::TooManyFormats);
    assert_eq!(full.user_text().unwrap(), before);
}
