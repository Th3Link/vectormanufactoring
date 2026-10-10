//! Independent tester cases for `0045-document-formats-library`, document-core
//! side: schema 2 of the built-in file, the user file (`format = 1`), the
//! overlay rules, validation and limits, the edits, ids, import and export,
//! round trips. Written from `specification.md` before the implementation was
//! read (only the public item names were listed to compile against).
//!
//! Criteria: 1, 2, 3, 4, 5, 7, 15, 16, 18 to 24 (model side), 26, 27, 32, 33, 34.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::needless_pass_by_value,
    missing_docs,
    clippy::assert_is_empty,
    clippy::type_complexity,
    clippy::format_push_string,
    clippy::manual_range_patterns
)]

use curvyo_document_core::{
    DocumentSize, FormatError, FormatField, FormatLibrary, FormatReason, FormatSpec, GroupTarget,
    Length, MAX_USER_FILE_BYTES, MAX_USER_FORMATS, MAX_USER_GROUPS, Orientation, PresetError,
    PresetList, PresetReason, PresetUnit, describe_error,
};

const SHIPPED_TEXT: &str = include_str!("../data/document-presets.toml");
const FIXTURE_FULL: &str = include_str!("fixtures/formats/tester_valid_full.toml");
const FIXTURE_HAND: &str = include_str!("fixtures/formats/tester_hand_edited.toml");
const FIXTURE_FUTURE: &str = include_str!("fixtures/formats/tester_future_version.toml");
const FIXTURE_IMPORT: &str = include_str!("fixtures/formats/tester_import_in.toml");

fn builtin() -> PresetList {
    PresetList::shipped().unwrap()
}

fn fresh() -> FormatLibrary {
    FormatLibrary::shipped().unwrap()
}

fn load(user: &str) -> Result<FormatLibrary, PresetError> {
    FormatLibrary::load(builtin(), user)
}

fn reason_of(user: &str) -> PresetReason {
    load(user)
        .err()
        .unwrap_or_else(|| panic!("expected a refusal for:\n{user}"))
        .reason
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
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

fn spec_new_group(name: &str, group_name: &str, short: f64, long: f64) -> FormatSpec {
    FormatSpec {
        name: name.to_string(),
        group: GroupTarget::New {
            name: group_name.to_string(),
            orientation: Orientation::Landscape,
        },
        short,
        long,
        unit: PresetUnit::Mm,
        favourite: true,
    }
}

/// One line per format: (group id, group on, preset id, name, short mm, long mm, favourite).
type Row = (String, bool, String, String, f64, f64, bool);

fn rows(lib: &FormatLibrary) -> Vec<Row> {
    lib.list()
        .presets()
        .map(|(g, p)| {
            (
                g.id.clone(),
                g.enabled,
                p.id.clone(),
                p.name.clone(),
                p.short_side.as_mm(),
                p.long_side.as_mm(),
                p.favourite,
            )
        })
        .collect()
}

fn group_ids(lib: &FormatLibrary) -> Vec<String> {
    lib.list().groups.iter().map(|g| g.id.clone()).collect()
}

fn fav_ids(lib: &FormatLibrary) -> Vec<String> {
    lib.list()
        .presets()
        .filter(|(_, p)| p.favourite)
        .map(|(_, p)| p.id.clone())
        .collect()
}

fn text(lib: &FormatLibrary) -> String {
    lib.user_text().unwrap()
}

const LASER: &str = r#"
format = 1

[[group]]
id = "u-laser"
name = "Laser"
default_orientation = "landscape"

  [[group.preset]]
  id = "u-keyring"
  name = "Key ring"
  short_side = 30
  long_side = 50
  unit = "mm"
"#;

const LASER_PLUS: &str = r#"
format = 1
[[group]]
id = "u-laser"
name = "Laser"
default_orientation = "landscape"
  [[group.preset]]
  id = "u-keyring"
  name = "Key ring"
  short_side = 30
  long_side = 50
  unit = "mm"
  [[group.preset]]
  id = "u-sign"
  name = "Sign"
  short_side = 60
  long_side = 80
  unit = "mm"
"#;

fn user(extra: &str) -> String {
    format!("format = 1\n{extra}")
}

// ---------------------------------------------------------------- Part A

#[test]
fn c1_shipped_file_is_schema_2_paper_on_slides_off_all_favourites() {
    assert!(SHIPPED_TEXT.contains("format = 2"));
    let list = builtin();
    let paper = &list.groups[0];
    let slides = &list.groups[1];
    assert_eq!((paper.id.as_str(), paper.enabled), ("paper", true));
    assert_eq!((slides.id.as_str(), slides.enabled), ("slides", false));
    assert_eq!(list.groups.len(), 2);
    let names: Vec<&str> = paper.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["A0", "A1", "A2", "A3", "A4", "A5", "A6"]);
    assert_eq!(paper.presets.iter().filter(|p| p.favourite).count(), 7);
    assert_eq!(slides.presets.len(), 3);
    assert_eq!(slides.presets.iter().filter(|p| p.favourite).count(), 3);
    assert!(list.groups.iter().all(|g| !g.user));
    assert!(list.presets().all(|(_, p)| !p.user));
}

#[test]
fn c32_shipped_ids_never_start_with_u_dash() {
    let list = builtin();
    for g in &list.groups {
        assert!(!g.id.starts_with("u-"), "{}", g.id);
    }
    for (_, p) in list.presets() {
        assert!(!p.id.starts_with("u-"), "{}", p.id);
    }
}

#[test]
fn c1_a4_is_exactly_210_by_297() {
    let list = builtin();
    let (_, a4) = list.find("a4").unwrap();
    assert!((a4.short_side.as_mm() - 210.0).abs() < 1e-9);
    assert!((a4.long_side.as_mm() - 297.0).abs() < 1e-9);
    let matched = list
        .matching(DocumentSize {
            width: mm(297.0),
            height: mm(210.0),
        })
        .unwrap();
    assert_eq!(matched.1.id, "a4");
}

#[test]
fn c1_reader_refuses_any_other_schema_value() {
    for head in [
        "format = 1",
        "format = 3",
        "format = 0",
        "format = -2",
        "format = \"2\"",
        "format = 2.0",
        "",
    ] {
        let t = SHIPPED_TEXT.replacen("format = 2", head, 1);
        assert!(PresetList::parse(&t).is_err(), "accepted: {head:?}");
    }
    let t = SHIPPED_TEXT.replacen("format = 2", head_none(), 1);
    assert!(PresetList::parse(&t).is_err());
}

fn head_none() -> &'static str {
    "# no format key"
}

#[test]
fn c1_schema_2_units_enabled_and_favourite_flags() {
    let t = r#"
format = 2
[[group]]
id = "g"
name = "G"
default_orientation = "portrait"
enabled = false
  [[group.preset]]
  id = "cm"
  name = "Cm"
  short_side = 10
  long_side = 20
  unit = "cm"
  favourite = false
  [[group.preset]]
  id = "inch"
  name = "Inch"
  short_side = 8.5
  long_side = 11
  unit = "in"
  [[group.preset]]
  id = "px"
  name = "Px"
  short_side = 1080
  long_side = 1920
  unit = "px"
"#;
    let list = PresetList::parse(t).unwrap();
    let g = &list.groups[0];
    assert!(!g.enabled);
    let by = |id: &str| g.presets.iter().find(|p| p.id == id).unwrap();
    assert!((by("cm").short_side.as_mm() - 100.0).abs() < 1e-9);
    assert!((by("cm").long_side.as_mm() - 200.0).abs() < 1e-9);
    assert!(!by("cm").favourite);
    assert!((by("inch").short_side.as_mm() - 215.9).abs() < 1e-9);
    assert!((by("inch").long_side.as_mm() - 279.4).abs() < 1e-9);
    assert!(by("inch").favourite, "favourite defaults to true");
    assert!((by("px").short_side.as_mm() - 1080.0 * 25.4 / 96.0).abs() < 1e-9);
}

#[test]
fn c1_unknown_unit_and_unknown_key_in_the_builtin_file_are_refused() {
    let t = SHIPPED_TEXT.replacen("unit = \"mm\"", "unit = \"pt\"", 1);
    assert!(matches!(
        PresetList::parse(&t).unwrap_err().reason,
        PresetReason::UnknownUnit(_)
    ));
    let t = SHIPPED_TEXT.replacen("format = 2", "format = 2\nsurprise = 1", 1);
    assert!(PresetList::parse(&t).is_err());
}

// ---------------------------------------------------------------- Part A: user file

#[test]
fn c2_the_example_file_of_the_spec_loads() {
    let lib = load(
        r#"
format = 1
favourites = ["a4", "a3", "u-coaster-90"]
enabled = { slides = true }

[[group]]
id = "u-laser"
name = "Laser"
default_orientation = "landscape"

  [[group.preset]]
  id = "u-keyring"
  name = "Key ring"
  short_side = 30
  long_side = 50
  unit = "mm"
"#,
    )
    .unwrap();
    // "u-coaster-90" names nothing: ignored. Only a4 and a3 are favourites.
    assert_eq!(fav_ids(&lib), ["a3", "a4"]);
    assert!(
        lib.list()
            .groups
            .iter()
            .find(|g| g.id == "slides")
            .unwrap()
            .enabled
    );
    let laser = lib
        .list()
        .groups
        .iter()
        .find(|g| g.id == "u-laser")
        .unwrap();
    assert!(laser.user);
    assert_eq!(laser.default_orientation, Orientation::Landscape);
    assert!(laser.presets[0].user);
    assert!(!laser.presets[0].favourite, "not named in favourites");
}

#[test]
fn c34_no_favourites_key_uses_the_builtin_flags() {
    let lib = load(LASER).unwrap();
    let builtin_favs = fav_ids(&lib)
        .into_iter()
        .filter(|i| !i.starts_with("u-"))
        .count();
    assert_eq!(builtin_favs, 7 + 3);
}

#[test]
fn c34_favourites_key_is_the_whole_list() {
    let lib = load(&user("favourites = [\"a4\"]\n")).unwrap();
    assert_eq!(fav_ids(&lib), ["a4"]);
    let lib = load(&user("favourites = []\n")).unwrap();
    assert!(fav_ids(&lib).is_empty());
}

#[test]
fn c4e_unknown_ids_in_favourites_and_enabled_are_ignored() {
    let lib = load(&user(
        "favourites = [\"a4\", \"nope\", \"u-gone\"]\nenabled = { slides = true, \"u-deleted-group\" = false }\n",
    ))
    .unwrap();
    assert_eq!(fav_ids(&lib), ["a4"]);
}

#[test]
fn c4a_user_group_with_builtin_id_appends_after_builtin_presets() {
    let lib = load(&user(
        r#"
[[group]]
id = "paper"
name = "Paper"
default_orientation = "portrait"
  [[group.preset]]
  id = "u-letter"
  name = "Letter"
  short_side = 8.5
  long_side = 11
  unit = "in"
"#,
    ))
    .unwrap();
    let paper = &lib.list().groups[0];
    let names: Vec<&str> = paper.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["A0", "A1", "A2", "A3", "A4", "A5", "A6", "Letter"]);
    assert_eq!(
        group_ids(&lib),
        ["paper", "slides"],
        "no second group appears"
    );
}

#[test]
fn c4a_mismatch_of_name_or_orientation_of_a_builtin_group_is_an_error() {
    for (name, orient) in [("Papier", "portrait"), ("Paper", "landscape")] {
        let t = format!(
            "format = 1\n[[group]]\nid = \"paper\"\nname = \"{name}\"\ndefault_orientation = \"{orient}\"\n  [[group.preset]]\n  id = \"u-x\"\n  name = \"X\"\n  short_side = 12\n  long_side = 34\n  unit = \"mm\"\n"
        );
        assert_eq!(
            reason_of(&t),
            PresetReason::GroupMismatch,
            "{name} {orient}"
        );
    }
}

#[test]
fn c4b_new_user_groups_come_after_all_builtin_groups_in_file_order() {
    let lib = load(&user(
        r#"
[[group]]
id = "u-zeta"
name = "Zeta"
default_orientation = "portrait"
[[group]]
id = "u-alpha"
name = "Alpha"
default_orientation = "portrait"
"#,
    ))
    .unwrap();
    assert_eq!(group_ids(&lib), ["paper", "slides", "u-zeta", "u-alpha"]);
}

#[test]
fn c4c_a_user_preset_id_must_differ_from_every_other_id() {
    let dup_builtin = user(
        "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n  id = \"a4\"\n  name = \"Mine\"\n  short_side = 12\n  long_side = 34\n  unit = \"mm\"\n",
    );
    assert_eq!(reason_of(&dup_builtin), PresetReason::DuplicatePresetId);
    let dup_user = user(
        "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n  id = \"u-x\"\n  name = \"X\"\n  short_side = 12\n  long_side = 34\n  unit = \"mm\"\n  [[group.preset]]\n  id = \"u-x\"\n  name = \"Y\"\n  short_side = 13\n  long_side = 35\n  unit = \"mm\"\n",
    );
    assert_eq!(reason_of(&dup_user), PresetReason::DuplicatePresetId);
}

#[test]
fn c4_duplicate_user_group_ids_are_refused() {
    let t = user(
        "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n[[group]]\nid = \"u-g\"\nname = \"H\"\ndefault_orientation = \"portrait\"\n",
    );
    assert!(load(&t).is_err());
}

#[test]
fn c4d_the_user_file_cannot_change_builtins() {
    // Any attempt to restate a built-in preset id is refused (see c4c); the
    // library's built-in rows are untouched by a valid overlay.
    let before: Vec<Row> = rows(&fresh());
    let lib = load(LASER).unwrap();
    let after: Vec<Row> = rows(&lib)
        .into_iter()
        .filter(|r| !r.2.starts_with("u-"))
        .collect();
    assert_eq!(before, after);
}

// ---------------------------------------------------------------- Part A: validation

fn group_with(preset_body: &str) -> String {
    user(&format!(
        "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n{preset_body}"
    ))
}

fn p(id: &str, name: &str, short: &str, long: &str, unit: &str) -> String {
    format!(
        "  id = \"{id}\"\n  name = \"{name}\"\n  short_side = {short}\n  long_side = {long}\n  unit = \"{unit}\"\n"
    )
}

#[test]
fn c5_validation_rules_each_refuse_with_the_named_reason() {
    use PresetReason as R;
    let cases: Vec<(String, fn(&R) -> bool, &str)> = vec![
        (
            p("U-Upper", "X", "12", "34", "mm"),
            |r| matches!(r, R::BadId),
            "upper-case id",
        ),
        (
            p("u x", "X", "12", "34", "mm"),
            |r| matches!(r, R::BadId),
            "space in id",
        ),
        (
            p("", "X", "12", "34", "mm"),
            |r| matches!(r, R::BadId | R::MissingField(_)),
            "empty id",
        ),
        (
            p("u-x", "", "12", "34", "mm"),
            |r| matches!(r, R::BadName | R::MissingField(_)),
            "empty name",
        ),
        (
            p("u-x", "   ", "12", "34", "mm"),
            |r| matches!(r, R::BadName),
            "blank name",
        ),
        (
            p("u-x", &"n".repeat(25), "12", "34", "mm"),
            |r| matches!(r, R::BadName),
            "25 chars",
        ),
        (
            p("u-x", "X", "0", "34", "mm"),
            |r| matches!(r, R::SideNotPositive),
            "zero side",
        ),
        (
            p("u-x", "X", "-5", "34", "mm"),
            |r| matches!(r, R::SideNotPositive),
            "negative side",
        ),
        (
            p("u-x", "X", "nan", "34", "mm"),
            |r| matches!(r, R::SideNotPositive | R::SideOutOfRange),
            "nan",
        ),
        (
            p("u-x", "X", "12", "inf", "mm"),
            |r| matches!(r, R::SideNotPositive | R::SideOutOfRange),
            "inf",
        ),
        (
            p("u-x", "X", "50", "30", "mm"),
            |r| matches!(r, R::ShortSideLonger),
            "short > long",
        ),
        (
            p("u-x", "X", "0.5", "34", "mm"),
            |r| matches!(r, R::SideOutOfRange),
            "below 1 mm",
        ),
        (
            p("u-x", "X", "12", "100001", "mm"),
            |r| matches!(r, R::SideOutOfRange),
            "above 100000 mm",
        ),
        (
            p("u-x", "X", "0.05", "34", "cm"),
            |r| matches!(r, R::SideOutOfRange),
            "0.5 mm in cm",
        ),
        (
            p("u-x", "X", "12", "4000", "in"),
            |r| matches!(r, R::SideOutOfRange),
            "4000 in > 100 m",
        ),
        (
            p("u-x", "X", "12", "34", "pt"),
            |r| matches!(r, R::UnknownUnit(_)),
            "unit pt",
        ),
        (
            p("u-x", "X", "12", "34", "MM"),
            |r| matches!(r, R::UnknownUnit(_)),
            "unit MM",
        ),
        (
            format!(
                "{}  note = \"{}\"\n",
                p("u-x", "X", "12", "34", "mm"),
                "n".repeat(61)
            ),
            |r| matches!(r, R::NoteTooLong),
            "note 61",
        ),
        (
            format!("{}  surprise = 1\n", p("u-x", "X", "12", "34", "mm")),
            |r| matches!(r, R::Syntax(_)),
            "unknown key in preset",
        ),
    ];
    for (body, check, what) in cases {
        let err = load(&group_with(&body))
            .err()
            .unwrap_or_else(|| panic!("accepted: {what}"));
        assert!(check(&err.reason), "{what}: got {:?}", err.reason);
    }
}

#[test]
fn c5_boundaries_that_must_be_accepted() {
    for body in [
        p("u-x", &"n".repeat(24), "12", "34", "mm"),
        p("u-x", "  X  ", "12", "34", "mm"),
        p("u-x", "X", "1", "100000", "mm"),
        p("u-x", "X", "5", "5", "mm"),
        p("u-x", "X", "0.1", "10000", "cm"),
        format!(
            "{}  note = \"{}\"\n",
            p("u-x", "X", "12", "34", "mm"),
            "n".repeat(60)
        ),
    ] {
        load(&group_with(&body)).unwrap_or_else(|e| panic!("{body}\n{e:?}"));
    }
}

#[test]
fn c5_name_is_trimmed_before_the_length_rule() {
    let body = p("u-x", &format!("  {}  ", "n".repeat(24)), "12", "34", "mm");
    let lib = load(&group_with(&body)).unwrap();
    let (_, preset) = lib.list().find("u-x").unwrap();
    assert_eq!(preset.name, "n".repeat(24));
}

#[test]
fn c5_same_size_is_refused_anywhere_in_the_library_within_tolerance() {
    // A4 is 210 x 297; the same size in another unit and a hair off.
    for (s, l, unit) in [
        ("210", "297", "mm"),
        ("21", "29.7", "cm"),
        ("210.005", "297.005", "mm"),
        ("209.995", "296.995", "mm"),
        ("8.267716535433071", "11.69291338582677", "in"),
    ] {
        let body = p("u-x", "X", s, l, unit);
        let r = reason_of(&group_with(&body));
        assert!(
            matches!(r, PresetReason::SameSizeAs(_)),
            "{s} x {l} {unit}: {r:?}"
        );
    }
    // Slides is off: still counts ("anywhere in the library").
    let body = p("u-x", "X", "9", "16", "in");
    let _ = body;
    let slide = builtin().groups[1].presets[0].clone();
    let body = p(
        "u-x",
        "X",
        &slide.short_side.as_mm().to_string(),
        &slide.long_side.as_mm().to_string(),
        "mm",
    );
    assert!(matches!(
        reason_of(&group_with(&body)),
        PresetReason::SameSizeAs(_)
    ));
}

#[test]
fn c5_a_size_clearly_different_is_accepted() {
    // 0.02 mm is more than the 0.01 mm tolerance.
    load(&group_with(&p("u-x", "X", "210.02", "297", "mm"))).unwrap();
    load(&group_with(&p("u-x", "X", "210", "297.02", "mm"))).unwrap();
}

#[test]
fn c5_same_size_between_two_user_formats_is_refused() {
    let t = user(
        "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n  id = \"u-a\"\n  name = \"A\"\n  short_side = 12\n  long_side = 34\n  unit = \"mm\"\n  [[group.preset]]\n  id = \"u-b\"\n  name = \"B\"\n  short_side = 1.2\n  long_side = 3.4\n  unit = \"cm\"\n",
    );
    assert!(matches!(reason_of(&t), PresetReason::SameSizeAs(_)));
}

fn many_formats(n: usize, groups: usize) -> String {
    let mut t = String::from("format = 1\n");
    for g in 0..groups {
        t.push_str(&format!(
            "[[group]]\nid = \"u-g{g}\"\nname = \"G{g}\"\ndefault_orientation = \"portrait\"\n"
        ));
        let mut i = g;
        while i < n {
            t.push_str(&format!(
                "  [[group.preset]]\n  id = \"u-p{i}\"\n  name = \"P{i}\"\n  short_side = 10\n  long_side = {}\n  unit = \"mm\"\n",
                100 + i
            ));
            i += groups;
        }
    }
    t
}

#[test]
fn c5_limits_200_formats_and_20_groups() {
    assert_eq!((MAX_USER_FORMATS, MAX_USER_GROUPS), (200, 20));
    let ok = load(&many_formats(200, 20)).unwrap();
    assert_eq!(ok.user_format_count(), 200);
    assert_eq!(
        reason_of(&many_formats(201, 20)),
        PresetReason::TooManyFormats
    );
    let mut t = many_formats(21, 21);
    assert_eq!(reason_of(&t), PresetReason::TooManyGroups);
    t = many_formats(0, 20);
    load(&t).unwrap();
}

#[test]
fn c5_file_size_limit_256_kib_checked_on_the_text() {
    assert_eq!(MAX_USER_FILE_BYTES, 256 * 1024);
    let base = "format = 1\n";
    let pad = |total: usize| {
        let filler = total - base.len() - 2;
        format!("{base}#{}\n", "x".repeat(filler))
    };
    let at_limit = pad(MAX_USER_FILE_BYTES);
    assert_eq!(at_limit.len(), MAX_USER_FILE_BYTES);
    load(&at_limit).unwrap();
    let over = pad(MAX_USER_FILE_BYTES + 1);
    assert_eq!(reason_of(&over), PresetReason::FileTooLarge);
    // Even garbage that is too large reports size, not syntax (checked before parsing).
    let garbage = "\u{0}".repeat(MAX_USER_FILE_BYTES + 10);
    assert_eq!(reason_of(&garbage), PresetReason::FileTooLarge);
}

#[test]
fn c2_unknown_top_level_keys_and_block_keys_are_refused() {
    assert!(load(&user("colour = \"red\"\n")).is_err());
    assert!(
        load(&user(
            "[[group]]\nid = \"u-g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\nextra = 1\n"
        ))
        .is_err()
    );
    assert!(load("this is = = not toml").is_err());
}

#[test]
fn c2_wrong_types_are_refused_not_panicked_on() {
    for t in [
        "format = 1\nfavourites = \"a4\"\n",
        "format = 1\nfavourites = [1, 2]\n",
        "format = 1\nenabled = [\"slides\"]\n",
        "format = 1\nenabled = { slides = \"yes\" }\n",
        "format = 1\ngroup = 5\n",
        "format = true\n",
        "format = 1.0\n",
    ] {
        assert!(load(t).is_err(), "accepted: {t}");
    }
}

#[test]
fn c33_a_file_whose_format_is_not_1_is_refused_as_newer_and_never_partly_read() {
    for head in ["format = 2", "format = 3", "format = 99"] {
        let r = reason_of(&format!("{head}\n"));
        assert_eq!(r, PresetReason::FormatNewer, "{head}");
    }
    // A newer file will usually carry keys this version does not know: the
    // version check must win over the unknown-key error.
    assert_eq!(reason_of(FIXTURE_FUTURE), PresetReason::FormatNewer);
    let err = load(FIXTURE_FUTURE).err().unwrap();
    assert!(
        describe_error(&err)
            .to_lowercase()
            .contains("newer version"),
        "{}",
        describe_error(&err)
    );
}

#[test]
fn c6_missing_or_ill_typed_format_key_is_an_error_with_a_message() {
    for t in ["", "favourites = []\n", "# only a comment\n"] {
        let err = load(t).err().unwrap_or_else(|| panic!("accepted {t:?}"));
        assert!(!describe_error(&err).is_empty());
    }
}

#[test]
fn c6_every_error_has_a_readable_description_naming_the_id() {
    let err = load(&group_with(&p("u-bad", "X", "50", "30", "mm")))
        .err()
        .unwrap();
    let msg = describe_error(&err);
    assert!(msg.contains("u-bad"), "{msg}");
    assert!(!msg.contains("PresetError"), "debug output leaked: {msg}");
}

// ---------------------------------------------------------------- round trips and goldens

#[test]
fn c33_round_trip_is_the_identity_on_text() {
    for src in [FIXTURE_FULL, FIXTURE_HAND, LASER] {
        let a = load(src).unwrap();
        let t1 = text(&a);
        let b = load(&t1).unwrap_or_else(|e| panic!("writer output unreadable: {e:?}\n{t1}"));
        let t2 = text(&b);
        assert_eq!(t1, t2, "write-read-write is not the identity");
        assert_eq!(rows(&a), rows(&b));
        assert_eq!(group_ids(&a), group_ids(&b));
    }
}

#[test]
fn c33_canonical_order_favourites_enabled_then_groups_in_creation_order() {
    let lib = load(FIXTURE_FULL).unwrap();
    let t = text(&lib);
    let pos = |needle: &str| {
        t.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{t}"))
    };
    assert!(t.trim_start().starts_with("format = 1"), "{t}");
    assert!(pos("favourites") < pos("enabled"), "{t}");
    assert!(pos("enabled") < pos("[[group"), "{t}");
    let first = pos("name = \"Laser\"");
    let second = pos("name = \"Embroidery\"");
    assert!(first < second, "groups in creation order:\n{t}");
}

#[test]
fn c33_golden_full_fixture_reads_to_the_documented_state() {
    let lib = load(FIXTURE_FULL).unwrap();
    assert_eq!(
        group_ids(&lib),
        ["paper", "slides", "u-laser", "u-embroidery"]
    );
    assert_eq!(fav_ids(&lib), ["a3", "a4", "u-coaster-90", "u-hoop-130"]);
    let slides = lib.list().groups.iter().find(|g| g.id == "slides").unwrap();
    assert!(slides.enabled);
    let emb = lib
        .list()
        .groups
        .iter()
        .find(|g| g.id == "u-embroidery")
        .unwrap();
    assert!(!emb.enabled);
    let (_, hoop) = lib.list().find("u-hoop-130").unwrap();
    assert!((hoop.short_side.as_mm() - 130.0).abs() < 1e-9);
    let (_, coaster) = lib.list().find("u-coaster-90").unwrap();
    assert!((coaster.long_side.as_mm() - 90.0).abs() < 1e-9);
    let (_, small) = lib.list().find("u-card").unwrap();
    assert!((small.short_side.as_mm() - 2.0 * 25.4).abs() < 1e-9);
    assert!((small.long_side.as_mm() - 3.5 * 25.4).abs() < 1e-9);
    assert_eq!(small.authored.unit, PresetUnit::In);
}

#[test]
fn c33_a_hand_edited_file_with_comments_and_odd_whitespace_loads() {
    let lib = load(FIXTURE_HAND).unwrap();
    assert!(lib.list().find("u-ring").is_some());
}

#[test]
fn c2_user_text_of_an_unchanged_library_loads_back_to_the_same_library() {
    let lib = fresh();
    let t = text(&lib);
    let again = load(&t).unwrap();
    assert_eq!(rows(&lib), rows(&again));
}

// ---------------------------------------------------------------- edits

#[test]
fn c32_new_ids_are_u_dash_slug_deterministic_with_numbered_collisions() {
    let mut a = fresh();
    let id1 = a
        .add_format(&spec("Key ring", "paper", 30.0, 50.0, PresetUnit::Mm))
        .unwrap();
    let id2 = a
        .add_format(&spec_new_group("Key  Ring!", "Laser", 31.0, 51.0))
        .unwrap();
    let id3 = a
        .add_format(&spec("key ring", "u-laser", 32.0, 52.0, PresetUnit::Mm))
        .unwrap();
    assert_eq!(id1, "u-key-ring");
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
    for id in [&id1, &id2, &id3] {
        assert!(id.starts_with("u-"), "{id}");
    }
    // A name that slugs to the same stem gets -2, -3, ...
    let mut b = fresh();
    let x1 = b
        .add_format(&spec("Coaster", "paper", 90.0, 91.0, PresetUnit::Mm))
        .unwrap();
    let x2 = b.add_format(&spec("Coaster", "paper", 90.0, 92.0, PresetUnit::Mm));
    // Same name twice in one group is refused (criterion 20); use a second group.
    assert_eq!(x1, "u-coaster");
    assert_eq!(
        x2.unwrap_err().reason,
        FormatReason::NameInGroup("Coaster".into())
    );
    let x3 = b
        .add_format(&spec_new_group("Coaster", "Other", 90.0, 93.0))
        .unwrap();
    assert_eq!(x3, "u-coaster-2");
    let x4 = b
        .add_format(&spec_new_group("Coaster", "Third", 90.0, 94.0))
        .unwrap();
    assert_eq!(x4, "u-coaster-3");
    // Determinism: the same inputs, the same ids and text.
    let mut c = fresh();
    c.add_format(&spec("Coaster", "paper", 90.0, 91.0, PresetUnit::Mm))
        .unwrap();
    c.add_format(&spec_new_group("Coaster", "Other", 90.0, 93.0))
        .unwrap();
    c.add_format(&spec_new_group("Coaster", "Third", 90.0, 94.0))
        .unwrap();
    assert_eq!(text(&b), text(&c));
}

#[test]
fn c32_group_ids_follow_the_same_rule() {
    let mut lib = fresh();
    lib.add_format(&spec_new_group("A", "Laser", 30.0, 50.0))
        .unwrap();
    let gid = group_ids(&lib).last().unwrap().clone();
    assert_eq!(gid, "u-laser");
}

#[test]
fn c32_names_without_ascii_letters_still_yield_ids_the_reader_accepts() {
    let mut lib = fresh();
    for (i, name) in ["★★", "Äpfel", "日本語", "!!!", "-", "- -", "a_b", "ÀÉ"]
        .iter()
        .enumerate()
    {
        let r = lib.add_format(&spec(
            name,
            "paper",
            20.0 + i as f64,
            40.0 + i as f64,
            PresetUnit::Mm,
        ));
        let id = r.unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let t = text(&lib);
        let back =
            load(&t).unwrap_or_else(|e| panic!("id {id:?} for {name:?} unreadable: {e:?}\n{t}"));
        assert!(back.list().find(&id).is_some(), "{name}");
    }
    let ids: Vec<String> = lib.list().presets().map(|(_, p)| p.id.clone()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "ids unique");
}

#[test]
fn c20_add_converts_units_exactly_and_keeps_the_authored_unit() {
    let mut lib = fresh();
    let id = lib
        .add_format(&spec("Postcard", "paper", 4.0, 6.0, PresetUnit::In))
        .unwrap();
    let (_, p) = lib.list().find(&id).unwrap();
    assert!((p.short_side.as_mm() - 101.6).abs() < 1e-9);
    assert!((p.long_side.as_mm() - 152.4).abs() < 1e-9);
    assert_eq!(p.authored.unit, PresetUnit::In);
    assert_eq!((p.authored.short, p.authored.long), (4.0, 6.0));
    assert!(p.user);

    let id = lib
        .add_format(&spec("Tile", "paper", 7.5, 12.0, PresetUnit::Cm))
        .unwrap();
    let (_, p) = lib.list().find(&id).unwrap();
    assert!((p.short_side.as_mm() - 75.0).abs() < 1e-9);

    // 1 in is 25.4 mm: a one inch square collides with a 25.4 mm square.
    let mut l2 = fresh();
    l2.add_format(&spec("Inch", "paper", 1.0, 2.0, PresetUnit::In))
        .unwrap();
    let e = l2
        .add_format(&spec("Mm", "paper", 25.4, 50.8, PresetUnit::Mm))
        .unwrap_err();
    assert!(
        matches!(e.reason, FormatReason::SameSizeAs(ref n) if n == "Inch"),
        "{e:?}"
    );

    // px is 1/96 in.
    let id = lib
        .add_format(&spec("Screen", "paper", 1000.0, 1500.0, PresetUnit::Px))
        .unwrap();
    let (_, p) = lib.list().find(&id).unwrap();
    assert!((p.short_side.as_mm() - 1000.0 * 25.4 / 96.0).abs() < 1e-9);
}

#[test]
fn c20_validation_messages_carry_the_right_field() {
    let mut lib = fresh();
    let e = lib
        .add_format(&spec("", "paper", 20.0, 40.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(
        (e.field, e.reason),
        (FormatField::Name, FormatReason::BadName)
    );
    let e = lib
        .add_format(&spec(&"n".repeat(25), "paper", 20.0, 40.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::BadName);
    let e = lib
        .add_format(&spec("X", "paper", 0.5, 40.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::BadSize);
    assert!(matches!(e.field, FormatField::Width | FormatField::Height));
    let e = lib
        .add_format(&spec("X", "paper", 20.0, 100_001.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::BadSize);
    for bad in [f64::NAN, f64::INFINITY, -3.0, 0.0] {
        let e = lib.add_format(&spec("X", "paper", bad, 40.0, PresetUnit::Mm));
        assert!(e.is_err(), "short = {bad}");
        let e = lib.add_format(&spec("X", "paper", 20.0, bad, PresetUnit::Mm));
        assert!(e.is_err(), "long = {bad}");
    }
    let e = lib
        .add_format(&spec("X", "paper", 21.0, 29.7, PresetUnit::Cm))
        .unwrap_err();
    assert!(
        matches!(e.reason, FormatReason::SameSizeAs(ref n) if n == "A4"),
        "{e:?}"
    );
    let e = lib
        .add_format(&spec("X", "nowhere", 20.0, 40.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::NotFound);
    assert!(
        lib.user_format_count() == 0,
        "nothing written by the failures"
    );
    assert_eq!(text(&lib), text(&fresh()));
}

#[test]
fn c20_a_size_given_with_the_longer_side_first_never_corrupts_the_library() {
    let mut lib = fresh();
    let r = lib.add_format(&spec("Swapped", "paper", 400.0, 300.0, PresetUnit::Mm));
    let t = text(&lib);
    let back = load(&t).unwrap_or_else(|e| panic!("library became unreadable after {r:?}: {e:?}"));
    assert_eq!(rows(&lib), rows(&back));
}

#[test]
fn c20_name_trimmed_and_unique_per_group_only() {
    let mut lib = fresh();
    let id = lib
        .add_format(&spec("  Ring  ", "paper", 30.0, 50.0, PresetUnit::Mm))
        .unwrap();
    assert_eq!(lib.list().find(&id).unwrap().1.name, "Ring");
    let e = lib
        .add_format(&spec("Ring", "paper", 31.0, 51.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(
        (e.field, e.reason),
        (FormatField::Name, FormatReason::NameInGroup("Ring".into()))
    );
    // The same name in another group is fine.
    lib.add_format(&spec_new_group("Ring", "Laser", 31.0, 51.0))
        .unwrap();
    // A built-in's name counts as used in the built-in group.
    let e = lib
        .add_format(&spec("A4", "paper", 31.0, 51.5, PresetUnit::Mm))
        .unwrap_err();
    assert!(matches!(e.reason, FormatReason::NameInGroup(_)), "{e:?}");
}

#[test]
fn c19_new_group_rules_name_1_to_24_unique_ignoring_case_and_limit_20() {
    let mut lib = fresh();
    for (bad, why) in [("", "empty"), ("   ", "blank"), (&"g".repeat(25)[..], "25")] {
        let e = lib
            .add_format(&spec_new_group("X", bad, 20.0, 40.0))
            .unwrap_err();
        assert_eq!(
            (e.field, e.reason),
            (FormatField::GroupName, FormatReason::BadName),
            "{why}"
        );
    }
    for taken in ["Paper", "paper", "PAPER", "slides"] {
        let e = lib
            .add_format(&spec_new_group("X", taken, 20.0, 40.0))
            .unwrap_err();
        assert_eq!(
            (e.field, e.reason),
            (FormatField::GroupName, FormatReason::GroupNameTaken),
            "{taken}"
        );
    }
    lib.add_format(&spec_new_group("X", "Laser", 20.0, 40.0))
        .unwrap();
    let e = lib
        .add_format(&spec_new_group("Y", "LASER", 21.0, 41.0))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::GroupNameTaken);
    for i in 1..20 {
        lib.add_format(&spec_new_group(
            "F",
            &format!("G{i}"),
            20.0 + f64::from(i),
            40.0 + f64::from(i),
        ))
        .unwrap_or_else(|e| panic!("group {i}: {e:?}"));
    }
    let e = lib
        .add_format(&spec_new_group("F", "One too many", 99.0, 199.0))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::LimitGroups);
    assert_eq!(group_ids(&lib).len(), 2 + 20);
}

#[test]
fn c20_the_201st_format_is_refused_with_limit_formats() {
    let mut lib = fresh();
    for i in 0..200 {
        lib.add_format(
            &spec("F", "paper", 10.0, 100.0 + f64::from(i), PresetUnit::Mm).named(&format!("F{i}")),
        )
        .unwrap_or_else(|e| panic!("{i}: {e:?}"));
    }
    let e = lib
        .add_format(&spec("Last", "paper", 10.0, 1000.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::LimitFormats);
    assert_eq!(lib.user_format_count(), 200);
    // Deleting one makes room again.
    lib.delete_format("u-f0").unwrap();
    lib.add_format(&spec("Last", "paper", 10.0, 1000.0, PresetUnit::Mm))
        .unwrap();
}

trait Named {
    fn named(self, name: &str) -> Self;
}
impl Named for FormatSpec {
    fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
}

#[test]
fn c21_edit_keeps_id_and_favourite_state_and_its_own_old_size_is_not_a_duplicate() {
    let mut lib = fresh();
    let mut s = spec("Ring", "paper", 30.0, 50.0, PresetUnit::Mm);
    s.favourite = false;
    let id = lib.add_format(&s).unwrap();
    assert!(!lib.list().find(&id).unwrap().1.favourite);
    // Same size, same name: allowed.
    lib.edit_format(&id, &spec("Ring", "paper", 30.0, 50.0, PresetUnit::Mm))
        .unwrap();
    assert!(
        !lib.list().find(&id).unwrap().1.favourite,
        "favourite state kept (spec.favourite ignored)"
    );
    // Rename and resize.
    lib.edit_format(&id, &spec("Ring 2", "paper", 3.0, 5.5, PresetUnit::Cm))
        .unwrap();
    let (_, p) = lib.list().find(&id).unwrap();
    assert_eq!((p.id.as_str(), p.name.as_str()), (id.as_str(), "Ring 2"));
    assert!((p.long_side.as_mm() - 55.0).abs() < 1e-9);
    // Another format's size is a duplicate.
    let e = lib
        .edit_format(&id, &spec("Ring 2", "paper", 21.0, 29.7, PresetUnit::Cm))
        .unwrap_err();
    assert!(matches!(e.reason, FormatReason::SameSizeAs(ref n) if n == "A4"));
    // Failing edit changes nothing.
    let p = lib.list().find(&id).unwrap().1.clone();
    assert_eq!(p.name, "Ring 2");
}

#[test]
fn c21_edit_can_move_a_format_to_another_group_or_the_form_cannot() {
    // Either the edit moves it to the target group, or the format stays where it
    // is; in both cases the library stays consistent and the id is kept.
    let mut lib = fresh();
    let id = lib
        .add_format(&spec_new_group("Ring", "Laser", 30.0, 50.0))
        .unwrap();
    lib.add_format(&spec_new_group("Hoop", "Embroidery", 130.0, 130.0))
        .unwrap();
    let r = lib.edit_format(
        &id,
        &spec("Ring", "u-embroidery", 30.0, 50.0, PresetUnit::Mm),
    );
    let t = text(&lib);
    let back = load(&t).unwrap();
    assert_eq!(rows(&lib), rows(&back));
    if r.is_ok() {
        assert_eq!(lib.list().find(&id).unwrap().0.id, "u-embroidery");
    }
}

#[test]
fn c22_built_in_formats_and_groups_cannot_be_edited_or_deleted() {
    let mut lib = fresh();
    let before = rows(&lib);
    let e = lib.delete_format("a4").unwrap_err();
    assert_eq!(e.reason, FormatReason::BuiltIn);
    let e = lib
        .edit_format("a4", &spec("A4x", "paper", 211.0, 297.0, PresetUnit::Mm))
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::BuiltIn);
    let e = lib.delete_group("paper").unwrap_err();
    assert_eq!(e.reason, FormatReason::BuiltIn);
    let e = lib.delete_group("slides").unwrap_err();
    assert_eq!(e.reason, FormatReason::BuiltIn);
    let e = lib
        .edit_group("paper", "Papier", Orientation::Portrait)
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::BuiltIn);
    assert_eq!(
        lib.delete_format("nope").unwrap_err().reason,
        FormatReason::NotFound
    );
    assert_eq!(
        lib.delete_group("nope").unwrap_err().reason,
        FormatReason::NotFound
    );
    assert_eq!(before, rows(&fresh()));
    assert_eq!(rows(&lib), before);
}

#[test]
fn c22_delete_removes_the_format_and_its_favourite_entry() {
    let mut lib = fresh();
    let id = lib
        .add_format(&spec("Ring", "paper", 30.0, 50.0, PresetUnit::Mm))
        .unwrap();
    assert!(fav_ids(&lib).contains(&id));
    lib.delete_format(&id).unwrap();
    assert!(lib.list().find(&id).is_none());
    assert!(!text(&lib).contains(&id), "{}", text(&lib));
    // Re-adding the same name (same id) as a non-favourite must not inherit a stale star.
    let mut s = spec("Ring", "paper", 30.0, 50.0, PresetUnit::Mm);
    s.favourite = false;
    let again = lib.add_format(&s).unwrap();
    assert_eq!(again, id);
    assert!(
        !lib.list().find(&again).unwrap().1.favourite,
        "stale favourite entry survived the delete"
    );
}

#[test]
fn c22_delete_group_removes_its_formats_and_favourites() {
    let mut lib = fresh();
    let a = lib
        .add_format(&spec_new_group("Ring", "Laser", 30.0, 50.0))
        .unwrap();
    let b = lib
        .add_format(&spec("Plate", "u-laser", 31.0, 51.0, PresetUnit::Mm))
        .unwrap();
    lib.delete_group("u-laser").unwrap();
    assert!(lib.list().find(&a).is_none() && lib.list().find(&b).is_none());
    assert!(!group_ids(&lib).contains(&"u-laser".to_string()));
    let t = text(&lib);
    assert!(
        !t.contains("u-ring") && !t.contains("u-plate") && !t.contains("u-laser"),
        "{t}"
    );
    assert_eq!(lib.user_format_count(), 0);
}

#[test]
fn c24_edit_group_renames_and_sets_orientation_with_the_name_rules() {
    let mut lib = fresh();
    lib.add_format(&spec_new_group("Ring", "Laser", 30.0, 50.0))
        .unwrap();
    lib.add_format(&spec_new_group("Hoop", "Embroidery", 130.0, 131.0))
        .unwrap();
    lib.edit_group("u-laser", "Laser cut", Orientation::Portrait)
        .unwrap();
    let g = lib
        .list()
        .groups
        .iter()
        .find(|g| g.id == "u-laser")
        .unwrap();
    assert_eq!(
        (g.name.as_str(), g.default_orientation),
        ("Laser cut", Orientation::Portrait)
    );
    // Case change of its own name is not a clash.
    lib.edit_group("u-laser", "LASER CUT", Orientation::Portrait)
        .unwrap();
    let e = lib
        .edit_group("u-laser", "embroidery", Orientation::Portrait)
        .unwrap_err();
    assert_eq!(
        (e.field, e.reason),
        (FormatField::GroupName, FormatReason::GroupNameTaken)
    );
    let e = lib
        .edit_group("u-laser", "paper", Orientation::Portrait)
        .unwrap_err();
    assert_eq!(e.reason, FormatReason::GroupNameTaken);
    for bad in ["", "  ", &"x".repeat(25)[..]] {
        let e = lib
            .edit_group("u-laser", bad, Orientation::Portrait)
            .unwrap_err();
        assert_eq!(e.reason, FormatReason::BadName);
    }
    assert_eq!(
        lib.edit_group("nope", "X", Orientation::Portrait)
            .unwrap_err()
            .reason,
        FormatReason::NotFound
    );
    let t = text(&lib);
    assert_eq!(rows(&load(&t).unwrap()), rows(&lib));
}

#[test]
fn c15_stars_toggle_favourites_and_a_group_turned_on_again_has_the_same_favourites() {
    let mut lib = fresh();
    lib.set_favourite("a4", false).unwrap();
    assert!(!fav_ids(&lib).contains(&"a4".to_string()));
    assert!(
        fav_ids(&lib).contains(&"a3".to_string()),
        "others unchanged"
    );
    // The slides keep their built-in favourite flags in the written file.
    let t = text(&lib);
    let back = load(&t).unwrap();
    assert!(!fav_ids(&back).contains(&"a4".to_string()));
    assert!(
        fav_ids(&back)
            .iter()
            .any(|i| i.starts_with("slide") || lib.list().find(i).unwrap().0.id == "slides")
    );
    lib.set_favourite("a4", true).unwrap();
    assert!(fav_ids(&lib).contains(&"a4".to_string()));
    // Starring twice is harmless.
    lib.set_favourite("a4", true).unwrap();
    assert_eq!(fav_ids(&lib).iter().filter(|i| *i == "a4").count(), 1);
}

#[test]
fn c16_group_switch_roundtrips_and_all_groups_may_be_off() {
    let mut lib = fresh();
    let slide_favs_before: Vec<String> = lib.list().groups[1]
        .presets
        .iter()
        .map(|p| p.id.clone())
        .collect();
    lib.set_favourite(&slide_favs_before[1], false).unwrap();
    lib.set_group_enabled("slides", true).unwrap();
    assert!(lib.list().groups[1].enabled);
    lib.set_group_enabled("slides", false).unwrap();
    lib.set_group_enabled("paper", false).unwrap();
    assert!(
        lib.list().groups.iter().all(|g| !g.enabled),
        "all groups may be off"
    );
    lib.set_group_enabled("slides", true).unwrap();
    let favs: Vec<bool> = lib.list().groups[1]
        .presets
        .iter()
        .map(|p| p.favourite)
        .collect();
    assert_eq!(favs, [true, false, true], "same favourites as before");
    let back = load(&text(&lib)).unwrap();
    assert_eq!(rows(&back), rows(&lib));
    assert_eq!(
        lib.set_group_enabled("nope", true).unwrap_err().reason,
        FormatReason::NotFound
    );
}

#[test]
fn c11_matching_only_looks_at_groups_that_are_on() {
    let mut lib = fresh();
    let s = lib.list().groups[1].presets[0].clone();
    let size = DocumentSize {
        width: s.long_side,
        height: s.short_side,
    };
    assert!(lib.list().matching(size).is_none(), "Slides are off");
    lib.set_group_enabled("slides", true).unwrap();
    assert_eq!(lib.list().matching(size).unwrap().1.id, s.id);
    let a3 = DocumentSize {
        width: mm(297.0),
        height: mm(420.0),
    };
    lib.set_favourite("a3", false).unwrap();
    assert_eq!(
        lib.list().matching(a3).unwrap().1.id,
        "a3",
        "not a favourite, still the subject"
    );
}

#[test]
fn c11_tolerance_of_one_hundredth_of_a_mm() {
    let lib = fresh();
    let near = |w: f64, h: f64| {
        lib.list()
            .matching(DocumentSize {
                width: mm(w),
                height: mm(h),
            })
            .map(|m| m.1.id.clone())
    };
    assert_eq!(near(210.009, 297.0).as_deref(), Some("a4"));
    assert_eq!(near(297.0, 209.992).as_deref(), Some("a4"));
    assert_eq!(near(210.02, 297.0), None);
    assert_eq!(near(210.0, 297.02), None);
}

// ---------------------------------------------------------------- export and import

#[test]
fn c26_export_is_none_without_user_formats() {
    assert_eq!(fresh().export_text().unwrap(), None);
    let mut lib = fresh();
    lib.set_favourite("a4", false).unwrap();
    lib.set_group_enabled("slides", true).unwrap();
    assert_eq!(
        lib.export_text().unwrap(),
        None,
        "choices alone are not exported"
    );
}

#[test]
fn c26_export_holds_user_groups_favourites_of_exported_ids_no_enabled_nothing_builtin() {
    let mut lib = load(FIXTURE_FULL).unwrap();
    lib.set_favourite("u-keyring", false).ok();
    let out = lib.export_text().unwrap().unwrap();
    let v: toml::Value = toml::from_str(&out).unwrap();
    let t = v.as_table().unwrap();
    assert_eq!(t["format"].as_integer(), Some(1));
    assert!(!t.contains_key("enabled"), "{out}");
    let favs: Vec<&str> = t["favourites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    for f in &favs {
        assert!(f.starts_with("u-"), "built-in id exported: {f}");
    }
    assert!(favs.contains(&"u-coaster-90"));
    assert!(
        !out.contains("\"a4\"") && !out.contains("id = \"a0\"") && !out.contains("Slides"),
        "{out}"
    );
    // The export is a valid user file.
    let back = load(&out).unwrap();
    assert!(back.list().find("u-coaster-90").is_some());
}

#[test]
fn c26_export_then_import_into_a_clean_library_reproduces_the_formats() {
    let src = load(FIXTURE_FULL).unwrap();
    let out = src.export_text().unwrap().unwrap();
    let mut dst = fresh();
    let rep = dst.import(&out).unwrap();
    assert_eq!(rep.skipped, 0);
    assert_eq!(rep.added, src.user_format_count());
    let user_rows = |l: &FormatLibrary| -> Vec<(String, String, f64, f64, bool)> {
        rows(l)
            .into_iter()
            .filter(|r| r.2.starts_with("u-"))
            .map(|r| (r.0, r.3, r.4, r.5, r.6))
            .collect()
    };
    let mut a = user_rows(&src);
    let mut b = user_rows(&dst);
    a.sort_by(|x, y| x.1.cmp(&y.1));
    b.sort_by(|x, y| x.1.cmp(&y.1));
    // The enabled choice of a user group is not exported: compare names, sizes, favourites.
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!((&x.1, x.2, x.3, x.4), (&y.1, y.2, y.3, y.4));
    }
}

#[test]
fn c26_a_user_format_added_to_a_builtin_group_survives_export_and_import() {
    let mut src = fresh();
    src.add_format(&spec("Letter", "paper", 8.5, 11.0, PresetUnit::In))
        .unwrap();
    let out = src.export_text().unwrap().unwrap();
    let mut dst = fresh();
    let rep = dst.import(&out).unwrap();
    assert_eq!(rep.added, 1, "{out}");
    let (g, p) = dst.list().find("u-letter").unwrap();
    assert_eq!(g.id, "paper");
    assert_eq!(p.name, "Letter");
}

#[test]
fn c27_a_failing_file_is_refused_whole_and_changes_nothing() {
    let mut lib = load(LASER).unwrap();
    let before = text(&lib);
    for bad in [
        "not toml = =",
        "format = 2\n",
        "format = 1\ncolour = 1\n",
        // One good group, then a bad preset.
        &user(
            "[[group]]\nid = \"u-ok\"\nname = \"Ok\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n  id = \"u-ok1\"\n  name = \"Ok1\"\n  short_side = 70\n  long_side = 80\n  unit = \"mm\"\n[[group]]\nid = \"u-bad\"\nname = \"Bad\"\ndefault_orientation = \"portrait\"\n  [[group.preset]]\n  id = \"u-bad1\"\n  name = \"Bad1\"\n  short_side = 90\n  long_side = 20\n  unit = \"mm\"\n",
        ),
    ] {
        assert!(lib.import(bad).is_err(), "{bad}");
        assert_eq!(
            text(&lib),
            before,
            "a refused import changed the library: {bad}"
        );
    }
}

#[test]
fn c27_import_merge_rules_via_the_golden_file() {
    let mut lib = load(LASER_PLUS).unwrap();
    let rep = lib.import(FIXTURE_IMPORT).unwrap();
    // The fixture (tester_import_in.toml) carries, in group "Laser" (same id) and
    // "Hoops" (new): see the comments in the file for each row's fate.
    assert_eq!(
        (rep.added, rep.skipped),
        (5, 2),
        "report: {rep:?}\nlibrary:\n{}",
        text(&lib)
    );
    let r = rows(&lib);
    let by_name = |n: &str| r.iter().filter(|x| x.3 == n).cloned().collect::<Vec<_>>();
    // Same id, same content: skipped (still exactly one Key ring).
    assert_eq!(by_name("Key ring").len(), 1);
    // Same id, other content: added under a new id.
    let plate = by_name("Placard");
    assert_eq!(plate.len(), 1);
    assert_ne!(plate[0].2, "u-sign");
    assert_eq!(by_name("Sign").len(), 1, "the old Sign stays");
    // Clash with a built-in id (a4) and another size: added with a new id.
    let mine = by_name("Mine A4 id");
    assert_eq!(mine.len(), 1);
    assert_ne!(mine[0].2, "a4");
    assert!(mine[0].2.starts_with("u-"));
    // A size that already exists (A4): skipped.
    assert!(by_name("Copy of A4").is_empty());
    // Joined the existing Laser group by id and by name.
    assert_eq!(
        lib.list()
            .groups
            .iter()
            .filter(|g| g.name == "Laser")
            .count(),
        1
    );
    assert_eq!(
        by_name("Tile")[0].0,
        "u-laser",
        "group joined by name (different id)"
    );
    // New group came after the existing groups; imported favourites became favourites.
    assert!(group_ids(&lib).contains(&"u-hoops".to_string()));
    assert!(by_name("Hoop 100")[0].6, "listed in favourites");
    // The enabled table of the file and built-in groups are not touched.
    assert!(!lib.list().groups[1].enabled, "slides still off");
    assert!(
        lib.list().groups[0].enabled,
        "paper = false in the file is ignored"
    );
    // The merged library is still a valid user file.
    let back = load(&text(&lib)).unwrap();
    assert_eq!(rows(&back), rows(&lib));
}

/// "Imported 5 formats in 2 groups": the five went into two groups the maker
/// sees (Laser, which took the group of the other name too, and Hoops).
#[test]
fn c27_report_counts_the_groups_that_received_formats() {
    let mut lib = load(LASER_PLUS).unwrap();
    let rep = lib.import(FIXTURE_IMPORT).unwrap();
    assert_eq!(rep.groups, 2, "{rep:?}");
}

/// A star the maker set must survive an import and any other star toggle.
#[test]
fn c15_c27_other_favourites_do_not_change_when_one_star_toggles_or_an_import_runs() {
    let mut lib = load(LASER_PLUS).unwrap();
    let before: Vec<(String, bool)> = lib
        .list()
        .presets()
        .map(|(_, p)| (p.id.clone(), p.favourite))
        .collect();
    lib.set_favourite("a4", false).unwrap();
    for (id, fav) in &before {
        if id != "a4" {
            assert_eq!(
                lib.list().find(id).unwrap().1.favourite,
                *fav,
                "toggling a4 changed {id}"
            );
        }
    }
    lib.set_favourite("a4", true).unwrap();
    lib.import(FIXTURE_IMPORT).unwrap();
    for (id, fav) in &before {
        assert_eq!(
            lib.list().find(id).unwrap().1.favourite,
            *fav,
            "the import changed the star of {id}"
        );
    }
}

#[test]
fn c27_importing_the_same_file_twice_skips_everything_the_second_time() {
    let mut lib = fresh();
    let first = lib.import(FIXTURE_FULL).unwrap();
    assert!(first.added > 0);
    let t1 = text(&lib);
    let second = lib.import(FIXTURE_FULL).unwrap();
    assert_eq!(second.added, 0, "{second:?}");
    assert_eq!(second.skipped, first.added, "{second:?}");
    assert_eq!(
        text(&lib),
        t1,
        "an import of known content must not change the file"
    );
}

#[test]
fn c27_import_ids_are_never_duplicated_and_sizes_stay_distinct() {
    let mut lib = load(LASER).unwrap();
    lib.import(FIXTURE_IMPORT).unwrap();
    lib.import(FIXTURE_FULL).unwrap();
    let r = rows(&lib);
    let mut ids: Vec<&String> = r.iter().map(|x| &x.2).collect();
    ids.sort();
    let n = ids.len();
    ids.dedup();
    assert_eq!(n, ids.len());
    for (i, a) in r.iter().enumerate() {
        for b in &r[i + 1..] {
            assert!(
                (a.4 - b.4).abs() > 0.01 || (a.5 - b.5).abs() > 0.01,
                "{} and {} share a size",
                a.3,
                b.3
            );
        }
    }
}

#[test]
fn c27_import_that_would_exceed_the_limit_is_refused_whole() {
    let mut lib = load(&many_formats(150, 5)).unwrap();
    let before = text(&lib);
    let mut other = String::from(
        "format = 1\n[[group]]\nid = \"u-z\"\nname = \"Z\"\ndefault_orientation = \"portrait\"\n",
    );
    for i in 0..60 {
        other.push_str(&format!(
            "  [[group.preset]]\n  id = \"u-z{i}\"\n  name = \"Z{i}\"\n  short_side = 500\n  long_side = {}\n  unit = \"mm\"\n",
            1000 + i
        ));
    }
    let r = lib.import(&other);
    assert!(r.is_err(), "210 formats accepted: {r:?}");
    assert_eq!(text(&lib), before);
    assert!(lib.user_format_count() <= 200);
}

#[test]
fn c27_import_of_a_file_with_only_known_builtin_choices_adds_nothing() {
    let mut lib = fresh();
    let rep = lib
        .import("format = 1\nfavourites = [\"a4\"]\nenabled = { slides = true }\n")
        .unwrap();
    assert_eq!((rep.added, rep.groups), (0, 0));
    assert!(!lib.list().groups[1].enabled);
    assert_eq!(
        fav_ids(&lib).len(),
        10,
        "built-in favourites untouched by the import"
    );
}

// ---------------------------------------------------------------- pseudo-random model run

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Random edit sequences: after every step (accepted or refused) the library
/// is internally consistent, serialises, reloads to the same rows, and the text
/// is stable. A refused edit changes nothing.
#[test]
fn model_random_edit_sequences_keep_the_library_valid_and_round_tripping() {
    for seed in 1..=12_u64 {
        let mut rng = Lcg(seed * 7919);
        let mut lib = fresh();
        for step in 0..120 {
            let before = text(&lib);
            let ids: Vec<String> = lib.list().presets().map(|(_, p)| p.id.clone()).collect();
            let gids = group_ids(&lib);
            let pick_id = |rng: &mut Lcg| ids[rng.below(ids.len() as u64) as usize].clone();
            let pick_group = |rng: &mut Lcg| gids[rng.below(gids.len() as u64) as usize].clone();
            let names = ["A4", "Ring", "ring", "Hoop", "Ä", "★", "x y", "  pad  ", ""];
            let units = [
                PresetUnit::Mm,
                PresetUnit::Cm,
                PresetUnit::In,
                PresetUnit::Px,
            ];
            let size = |rng: &mut Lcg| {
                let s = [1.0, 2.5, 10.0, 21.0, 100.0, 210.0, 0.4, 0.0, -1.0, 5000.0]
                    [rng.below(10) as usize];
                (
                    s,
                    s + [0.0, 0.005, 0.5, 87.0, 99_999.0][rng.below(5) as usize],
                )
            };
            let result_ok = match rng.below(9) {
                0 | 1 | 2 => {
                    let (s, l) = size(&mut rng);
                    let g = if rng.below(3) == 0 {
                        GroupTarget::New {
                            name: names[rng.below(9) as usize].to_string(),
                            orientation: Orientation::Portrait,
                        }
                    } else {
                        GroupTarget::Existing(pick_group(&mut rng))
                    };
                    lib.add_format(&FormatSpec {
                        name: names[rng.below(9) as usize].to_string(),
                        group: g,
                        short: s,
                        long: l,
                        unit: units[rng.below(4) as usize],
                        favourite: rng.below(2) == 0,
                    })
                    .is_ok()
                }
                3 => {
                    let (s, l) = size(&mut rng);
                    let id = pick_id(&mut rng);
                    lib.edit_format(
                        &id,
                        &spec(
                            names[rng.below(9) as usize],
                            &pick_group(&mut rng),
                            s,
                            l,
                            units[rng.below(4) as usize],
                        ),
                    )
                    .is_ok()
                }
                4 => lib.delete_format(&pick_id(&mut rng)).is_ok(),
                5 => lib
                    .set_favourite(&pick_id(&mut rng), rng.below(2) == 0)
                    .is_ok(),
                6 => lib
                    .set_group_enabled(&pick_group(&mut rng), rng.below(2) == 0)
                    .is_ok(),
                7 => lib.delete_group(&pick_group(&mut rng)).is_ok(),
                _ => lib
                    .edit_group(
                        &pick_group(&mut rng),
                        names[rng.below(9) as usize],
                        Orientation::Landscape,
                    )
                    .is_ok(),
            };
            let after = text(&lib);
            if !result_ok {
                assert_eq!(
                    before, after,
                    "seed {seed} step {step}: a refused edit changed the file text"
                );
            }
            let back = load(&after).unwrap_or_else(|e| {
                panic!("seed {seed} step {step}: own output unreadable: {e:?}\n{after}")
            });
            assert_eq!(rows(&back), rows(&lib), "seed {seed} step {step}");
            assert_eq!(text(&back), after, "seed {seed} step {step}: not canonical");
            assert!(lib.user_format_count() <= MAX_USER_FORMATS);
            let r = rows(&lib);
            for (i, a) in r.iter().enumerate() {
                assert!(a.4 <= a.5 + 1e-9);
                for b in &r[i + 1..] {
                    assert!(a.2 != b.2, "duplicate id {}", a.2);
                    assert!(
                        (a.4 - b.4).abs() > 0.0099 || (a.5 - b.5).abs() > 0.0099,
                        "seed {seed} step {step}: {} and {} share a size",
                        a.3,
                        b.3
                    );
                }
            }
        }
    }
}

/// Export of any library must be importable into a clean one without refusal.
#[test]
fn model_export_of_random_libraries_imports_cleanly() {
    for seed in 100..=108_u64 {
        let mut rng = Lcg(seed);
        let mut lib = fresh();
        for i in 0..40 {
            let g = if rng.below(2) == 0 {
                GroupTarget::New {
                    name: format!("Group {}", rng.below(6)),
                    orientation: Orientation::Landscape,
                }
            } else {
                GroupTarget::Existing(if rng.below(2) == 0 {
                    "paper".into()
                } else {
                    "slides".into()
                })
            };
            let _ = lib.add_format(&FormatSpec {
                name: format!("F {i}"),
                group: g,
                short: 10.0 + f64::from(i) * 3.0,
                long: 500.0 + f64::from(i),
                unit: PresetUnit::Mm,
                favourite: rng.below(2) == 0,
            });
        }
        if let Some(out) = lib.export_text().unwrap() {
            let mut clean = fresh();
            let rep = clean
                .import(&out)
                .unwrap_or_else(|e| panic!("seed {seed}: {e:?}\n{out}"));
            assert_eq!(rep.added, lib.user_format_count(), "seed {seed}");
        }
    }
}

#[allow(dead_code)]
fn unused(_: FormatError) {}

// ---------------------------------------------------------------- white box: text handling

#[test]
fn wb_windows_line_endings_load_and_rewrite_to_the_same_library() {
    let crlf = FIXTURE_FULL.replace('\n', "\r\n");
    let a = load(&crlf).unwrap();
    let b = load(FIXTURE_FULL).unwrap();
    assert_eq!(rows(&a), rows(&b));
}

#[test]
fn wb_a_utf8_byte_order_mark_from_a_text_editor_is_accepted() {
    // Windows Notepad and some sync tools write a BOM; a maker who opens the
    // file and saves it must not lose the whole list to a "broken file" notice.
    let with_bom = format!("\u{feff}{FIXTURE_FULL}");
    let a = load(&with_bom).unwrap_or_else(|e| panic!("BOM refused: {e:?}"));
    assert_eq!(rows(&a), rows(&load(FIXTURE_FULL).unwrap()));
}

#[test]
fn wb_names_with_quotes_backslashes_and_unusual_characters_survive_the_file() {
    let mut lib = fresh();
    let names = [
        "Say \"hi\"",
        "back\\slash",
        "tab\there",
        "Ünï 日本 ★",
        "emoji \u{1f600}",
        "a'b",
        "[table]",
        "x = 1",
        "# not a comment",
        "\u{202e}rtl",
        "\u{7f}del",
        "multi\nline",
        "cr\rhere",
        "nul\u{0}byte",
    ];
    for (i, name) in names.iter().enumerate() {
        let r = lib.add_format(&spec(
            name,
            "paper",
            20.0 + i as f64 * 2.0,
            40.0 + i as f64 * 2.0,
            PresetUnit::Mm,
        ));
        if r.is_err() {
            continue; // refusing a name is acceptable; corrupting the file is not
        }
        let t = text(&lib);
        let back = load(&t).unwrap_or_else(|e| panic!("name {name:?} broke the file: {e:?}\n{t}"));
        assert_eq!(
            rows(&back),
            rows(&lib),
            "name {name:?} did not survive the file"
        );
    }
}

#[test]
fn wb_group_names_with_special_characters_survive_the_file() {
    let mut lib = fresh();
    for (i, g) in ["Say \"hi\"", "a\\b", "日本", "x\ty", "[g]"]
        .iter()
        .enumerate()
    {
        let r = lib.add_format(&spec_new_group("F", g, 20.0 + i as f64, 40.0 + i as f64));
        if r.is_ok() {
            let t = text(&lib);
            let back = load(&t).unwrap_or_else(|e| panic!("group {g:?}: {e:?}\n{t}"));
            assert_eq!(group_ids(&back), group_ids(&lib));
        }
    }
}

#[test]
fn wb_floats_that_do_not_print_short_survive_the_file_exactly() {
    let mut lib = fresh();
    lib.add_format(&spec(
        "Third",
        "paper",
        0.1 + 0.2,
        100.0 / 3.0,
        PresetUnit::Cm,
    ))
    .unwrap();
    lib.add_format(&spec(
        "Inch",
        "paper",
        8.123_456_789,
        11.987_654_321,
        PresetUnit::In,
    ))
    .unwrap();
    let t = text(&lib);
    let back = load(&t).unwrap();
    for id in ["u-third", "u-inch"] {
        let a = lib.list().find(id).unwrap().1;
        let b = back.list().find(id).unwrap().1;
        assert_eq!(a.short_side.as_mm(), b.short_side.as_mm(), "{id}");
        assert_eq!(a.long_side.as_mm(), b.long_side.as_mm(), "{id}");
    }
}
