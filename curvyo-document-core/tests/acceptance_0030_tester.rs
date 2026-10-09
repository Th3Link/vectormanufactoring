//! Independent tester cases for `0030-document-size-presets`, document side:
//! the shipped data file, the loader and its validation rules (criteria 1 to
//! 8), `matching` and `Orientation::of` (criteria 10 and 14 support). Written
//! from `specification.md` before the implementation was read; every failing
//! fixture here is an inline string of my own.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::assert_is_empty,
    missing_docs
)]

use std::fs;
use std::path::{Path, PathBuf};

use curvyo_document_core::{
    DocumentSize, Orientation, PresetList, PresetReason, PresetSubject, PresetUnit,
};

const SHIPPED: &str = include_str!("../data/document-presets.toml");

fn shipped() -> PresetList {
    PresetList::shipped().expect("the shipped file loads")
}

fn all(list: &PresetList) -> Vec<(String, String, f64, f64)> {
    list.groups
        .iter()
        .flat_map(|g| {
            g.presets.iter().map(|p| {
                (
                    g.id.clone(),
                    p.name.clone(),
                    p.short_side.as_mm(),
                    p.long_side.as_mm(),
                )
            })
        })
        .collect()
}

// ------------------------------------------------------------ criterion 1, 5

#[test]
fn the_shipped_list_is_exactly_the_table_of_criterion_1() {
    let list = shipped();
    let want: [(&str, &str, f64, f64); 10] = [
        ("paper", "A0", 841.0, 1189.0),
        ("paper", "A1", 594.0, 841.0),
        ("paper", "A2", 420.0, 594.0),
        ("paper", "A3", 297.0, 420.0),
        ("paper", "A4", 210.0, 297.0),
        ("paper", "A5", 148.0, 210.0),
        ("paper", "A6", 105.0, 148.0),
        ("slides", "16:9", 285.75, 508.0),
        ("slides", "16:10", 317.5, 508.0),
        ("slides", "4:3", 203.2, 25.4 * 1024.0 / 96.0),
    ];
    let got = all(&list);
    assert_eq!(got.len(), want.len());
    for ((g, name, s, l), (wg, wname, ws, wl)) in got.iter().zip(want) {
        assert_eq!(g, wg);
        assert_eq!(name, wname);
        assert!((s - ws).abs() < 1e-9, "{name} short {s} vs {ws}");
        assert!((l - wl).abs() < 1e-9, "{name} long {l} vs {wl}");
    }
    // 4:3 long side is 270.9333...
    let four_three = got.last().unwrap();
    assert!((four_three.3 - 270.933_333_333_333_3).abs() < 1e-9);
    assert_eq!(list.groups.len(), 2);
    assert_eq!(list.groups[0].name, "Paper");
    assert_eq!(list.groups[1].name, "Slides");
    assert_eq!(list.groups[0].default_orientation, Orientation::Portrait);
    assert_eq!(list.groups[1].default_orientation, Orientation::Landscape);
}

#[test]
fn px_and_inch_conversions_are_exact() {
    let text = r#"
format = 1
[[group]]
id = "g"
name = "G"
default_orientation = "portrait"
  [[group.preset]]
  id = "px"
  name = "px"
  short_side = 1080
  long_side = 1920
  unit = "px"
  [[group.preset]]
  id = "px2"
  name = "px2"
  short_side = 1200
  long_side = 1920
  unit = "px"
  [[group.preset]]
  id = "px3"
  name = "px3"
  short_side = 768
  long_side = 1024
  unit = "px"
  [[group.preset]]
  id = "letter"
  name = "Letter"
  short_side = 8.5
  long_side = 11
  unit = "in"
  [[group.preset]]
  id = "mm"
  name = "mm"
  short_side = 50.5
  long_side = 70.25
  unit = "mm"
"#;
    let list = PresetList::parse(text).unwrap();
    let p = &list.groups[0].presets;
    assert_eq!(p[0].short_side.as_mm(), 285.75);
    assert_eq!(p[0].long_side.as_mm(), 508.0);
    assert_eq!(p[1].short_side.as_mm(), 317.5);
    assert!((p[2].short_side.as_mm() - 203.2).abs() < 1e-9);
    assert!((p[2].long_side.as_mm() - 270.933_333_333_333_3).abs() < 1e-9);
    // 8.5 in is the double nearest 215.9, as `value * 254 / 10`.
    assert_eq!(p[3].short_side.as_mm(), 8.5 * 254.0 / 10.0);
    assert!((p[3].short_side.as_mm() - 215.9).abs() < 1e-12);
    assert_eq!(p[3].long_side.as_mm(), 11.0 * 254.0 / 10.0);
    assert_eq!(p[4].short_side.as_mm(), 50.5);
    assert_eq!(p[4].long_side.as_mm(), 70.25);
    // Authored numbers are kept.
    assert_eq!(p[0].authored.short, 1080.0);
    assert_eq!(p[0].authored.long, 1920.0);
    assert_eq!(p[0].authored.unit, PresetUnit::Px);
    assert_eq!(p[3].authored.unit, PresetUnit::In);
    assert_eq!(p[4].authored.unit, PresetUnit::Mm);
}

#[test]
fn the_shipped_notes_and_authored_units() {
    let list = shipped();
    let a4 = &list.groups[0].presets[4];
    assert_eq!(a4.id, "a4");
    assert_eq!(a4.authored.unit, PresetUnit::Mm);
    let s169 = &list.groups[1].presets[0];
    assert_eq!(s169.authored.unit, PresetUnit::Px);
    assert_eq!(s169.authored.short, 1080.0);
    assert_eq!(s169.authored.long, 1920.0);
    assert_eq!(s169.note.as_deref(), Some("Full HD"));
}

// ------------------------------------------------------------- criterion 2

fn rs_and_ts_sources(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if matches!(
                name.as_str(),
                "target" | "node_modules" | "tests" | "dist" | "fixtures" | ".git" | "pkg"
            ) {
                continue;
            }
            rs_and_ts_sources(&p, out);
        } else if matches!(
            p.extension().and_then(|x| x.to_str()),
            Some("rs" | "ts" | "tsx")
        ) {
            out.push(p);
        }
    }
}

#[test]
fn no_preset_size_is_written_in_non_test_sources() {
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for dir in fs::read_dir(ws).unwrap().flatten() {
        let name = dir.file_name().to_string_lossy().to_string();
        if name.starts_with("curvyo-") {
            rs_and_ts_sources(&dir.path().join("src"), &mut files);
        }
    }
    rs_and_ts_sources(&ws.join("frontend/src"), &mut files);
    assert!(files.len() > 20, "the scan found the sources");
    for f in files {
        let text = fs::read_to_string(&f).unwrap();
        // Drop in-file test modules: everything after `#[cfg(test)]`.
        let body: String = text
            .split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if name.contains(".test.") {
            continue;
        }
        for needle in ["1189", "\"A4\"", "'A4'", "270.93", "285.75"] {
            assert!(
                !body.contains(needle),
                "{} holds the preset literal {needle}",
                f.display()
            );
        }
    }
}

#[test]
fn the_data_file_is_embedded_not_read_at_run_time() {
    // The loader takes the text: parsing the file's own text gives the shipped list.
    assert_eq!(PresetList::parse(SHIPPED).unwrap().groups, shipped().groups);
}

// ------------------------------------------------------------- criterion 3

#[test]
fn one_more_block_appears_at_the_expected_index() {
    let extra = r#"
  [[group.preset]]
  id = "b5"
  name = "B5"
  short_side = 176
  long_side = 250
  unit = "mm"
"#;
    // Insert after A6 (end of the paper group): before `[[group]]\nid = "slides"`.
    let marker = "[[group]]\nid = \"slides\"";
    let at = SHIPPED.find(marker).unwrap();
    let mut text = String::from(&SHIPPED[..at]);
    text.push_str(extra);
    text.push('\n');
    text.push_str(&SHIPPED[at..]);
    let list = PresetList::parse(&text).unwrap();
    assert_eq!(list.groups[0].presets.len(), 8);
    assert_eq!(list.groups[0].presets[7].name, "B5");
    assert_eq!(list.groups[1].presets.len(), 3);
    assert_eq!(all(&list).len(), 11);
    // Appending at the very end goes to the last group.
    let appended = format!("{SHIPPED}\n{extra}");
    let list = PresetList::parse(&appended).unwrap();
    assert_eq!(list.groups[1].presets.last().unwrap().name, "B5");
    assert_eq!(list.groups[1].presets.len(), 4);
}

// ------------------------------------------------------------- criterion 4/8

#[test]
fn nine_paper_presets_load_in_file_order() {
    let mut s = String::from(
        "format = 1\n[[group]]\nid = \"paper\"\nname = \"Paper\"\ndefault_orientation = \"portrait\"\n",
    );
    for i in 0..9 {
        use std::fmt::Write as _;
        write!(
            s,
            "[[group.preset]]\nid = \"p{i}\"\nname = \"Z{}\"\nshort_side = {}\nlong_side = {}\nunit = \"mm\"\n",
            9 - i,
            100 + i * 10,
            200 + i * 10
        )
        .unwrap();
    }
    let list = PresetList::parse(&s).unwrap();
    let names: Vec<_> = list.groups[0]
        .presets
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(
        names,
        ["Z9", "Z8", "Z7", "Z6", "Z5", "Z4", "Z3", "Z2", "Z1"],
        "nothing sorted"
    );
}

#[test]
fn groups_keep_file_order_even_when_not_alphabetical() {
    let s = r#"format = 1
[[group]]
id = "z"
name = "Zed"
default_orientation = "landscape"
  [[group.preset]]
  id = "a"
  name = "a"
  short_side = 10
  long_side = 20
  unit = "mm"
[[group]]
id = "a"
name = "Alpha"
default_orientation = "portrait"
  [[group.preset]]
  id = "b"
  name = "b"
  short_side = 11
  long_side = 22
  unit = "mm"
"#;
    let list = PresetList::parse(s).unwrap();
    assert_eq!(list.groups[0].id, "z");
    assert_eq!(list.groups[1].id, "a");
    assert_eq!(list.groups[0].default_orientation, Orientation::Landscape);
}

// ------------------------------------------------------------- criterion 6

fn one_preset(fields: &str) -> String {
    format!(
        "format = 1\n[[group]]\nid = \"g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\n[[group.preset]]\n{fields}\n"
    )
}

fn good_fields() -> &'static str {
    "id = \"p\"\nname = \"P\"\nshort_side = 100\nlong_side = 200\nunit = \"mm\"\n"
}

fn err(text: &str) -> curvyo_document_core::PresetError {
    PresetList::parse(text).expect_err("must be rejected")
}

#[test]
fn the_good_baseline_loads() {
    PresetList::parse(&one_preset(good_fields())).unwrap();
}

#[test]
fn rule_a_not_toml_and_unknown_keys_at_every_level() {
    let e = err("this is [not toml");
    assert!(matches!(e.reason, PresetReason::Syntax(_)), "{e:?}");
    assert_eq!(e.subject, PresetSubject::File);
    // unknown top-level key
    let e = err(&format!("{}\nbogus = 1\n", "format = 1"));
    assert!(matches!(e.reason, PresetReason::Syntax(_)), "{e:?}");
    // typo in a group key
    let e = err(
        "format = 1\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\ndefault_orientaton=\"x\"\n",
    );
    assert!(matches!(e.reason, PresetReason::Syntax(_)), "{e:?}");
    // typo in a preset key
    let e = err(&one_preset(&format!("{}shortside = 3\n", good_fields())));
    assert!(matches!(e.reason, PresetReason::Syntax(_)), "{e:?}");
    // empty text
    let e = PresetList::parse("");
    assert!(e.is_err() || e.unwrap().groups.is_empty());
}

#[test]
fn rule_b_format_missing_or_not_one() {
    let body = &one_preset(good_fields())["format = 1\n".len()..];
    let e = err(body);
    assert_eq!(e.reason, PresetReason::FormatNotOne);
    assert_eq!(e.subject, PresetSubject::File);
    for bad in ["format = 2\n", "format = 0\n", "format = -1\n"] {
        assert_eq!(
            err(&format!("{bad}{body}")).reason,
            PresetReason::FormatNotOne
        );
    }
    // wrong type
    assert!(PresetList::parse(&format!("format = \"1\"\n{body}")).is_err());
    assert!(PresetList::parse(&format!("format = true\n{body}")).is_err());
}

#[test]
fn rule_c_group_rules() {
    let preset = "[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n";
    let mk = |head: &str| format!("format = 1\n[[group]]\n{head}\n{preset}");
    // missing id / name / default_orientation
    assert!(matches!(
        err(&mk("name=\"G\"\ndefault_orientation=\"portrait\"")).reason,
        PresetReason::MissingField(f) if f == "id"
    ));
    assert!(matches!(
        err(&mk("id=\"g\"\ndefault_orientation=\"portrait\"")).reason,
        PresetReason::MissingField(f) if f == "name"
    ));
    assert!(matches!(
        err(&mk("id=\"g\"\nname=\"G\"")).reason,
        PresetReason::MissingField(f) if f == "default_orientation"
    ));
    // bad orientation values
    for bad in ["Portrait", "sideways", "", "LANDSCAPE"] {
        assert!(
            matches!(
                err(&mk(&format!(
                    "id=\"g\"\nname=\"G\"\ndefault_orientation=\"{bad}\""
                )))
                .reason,
                PresetReason::BadOrientation(_)
            ),
            "{bad}"
        );
    }
    // group without a preset
    let e = err("format = 1\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n");
    assert_eq!(e.reason, PresetReason::EmptyGroup);
    assert_eq!(e.subject, PresetSubject::Group("g".into()));
    // duplicate group id
    let dup = format!(
        "format = 1\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n{preset}[[group]]\nid=\"g\"\nname=\"H\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"q\"\nname=\"Q\"\nshort_side=5\nlong_side=6\nunit=\"mm\"\n"
    );
    let e = err(&dup);
    assert_eq!(e.reason, PresetReason::DuplicateGroupId);
    assert_eq!(e.subject, PresetSubject::Group("g".into()));
}

#[test]
fn rule_d_preset_fields_and_duplicate_ids_across_groups() {
    for (missing, text) in [
        ("id", "name=\"P\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n"),
        ("name", "id=\"p\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n"),
        (
            "short_side",
            "id=\"p\"\nname=\"P\"\nlong_side=2\nunit=\"mm\"\n",
        ),
        (
            "long_side",
            "id=\"p\"\nname=\"P\"\nshort_side=1\nunit=\"mm\"\n",
        ),
        ("unit", "id=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\n"),
    ] {
        let e = err(&one_preset(text));
        assert!(
            matches!(&e.reason, PresetReason::MissingField(f) if *f == missing),
            "{missing}: {e:?}"
        );
    }
    // Same id inside one group and across two groups.
    let across = "format = 1\n[[group]]\nid=\"g1\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n[[group]]\nid=\"g2\"\nname=\"H\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"p\"\nname=\"Q\"\nshort_side=5\nlong_side=6\nunit=\"mm\"\n";
    let e = err(across);
    assert_eq!(e.reason, PresetReason::DuplicatePresetId);
    assert_eq!(e.subject, PresetSubject::Preset("p".into()));
    let within = one_preset(good_fields())
        + "[[group.preset]]\nid=\"p\"\nname=\"Q\"\nshort_side=5\nlong_side=6\nunit=\"mm\"\n";
    assert_eq!(err(&within).reason, PresetReason::DuplicatePresetId);
}

#[test]
fn rule_e_unit() {
    for bad in ["cm", "MM", "pt", "", "px "] {
        let e = err(&one_preset(&format!(
            "id=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"{bad}\"\n"
        )));
        assert!(
            matches!(e.reason, PresetReason::UnknownUnit(_)),
            "{bad:?}: {e:?}"
        );
        assert_eq!(e.subject, PresetSubject::Preset("p".into()));
    }
    assert!(
        PresetList::parse(&one_preset(
            "id=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=5\n"
        ))
        .is_err()
    );
}

fn sides(s: &str, l: &str, unit: &str) -> String {
    one_preset(&format!(
        "id=\"p\"\nname=\"P\"\nshort_side={s}\nlong_side={l}\nunit=\"{unit}\"\n"
    ))
}

#[test]
fn rule_f_sides() {
    for (s, l) in [
        ("nan", "10"),
        ("10", "nan"),
        ("inf", "inf"),
        ("-inf", "10"),
        ("0", "10"),
        ("10", "0"),
        ("-5", "10"),
        ("10", "-5"),
        ("\"ten\"", "20"),
        ("10", "true"),
        ("[1]", "20"),
        ("-0.0", "10"),
    ] {
        let e = PresetList::parse(&sides(s, l, "mm")).expect_err(&format!("{s} {l}"));
        assert_eq!(
            e.subject,
            PresetSubject::Preset("p".into()),
            "{s} {l}: {e:?}"
        );
    }
    // short larger than long
    assert_eq!(
        err(&sides("30", "20", "mm")).reason,
        PresetReason::ShortSideLonger
    );
    // out-of-range after conversion
    for (s, l, u) in [
        ("0.5", "5", "mm"),
        ("0.999", "5", "mm"),
        ("5", "100000.001", "mm"),
        ("5", "1e9", "mm"),
        ("0.03", "5", "in"),   // 0.762 mm
        ("5", "4000", "in"),   // 101600 mm
        ("3", "5", "px"),      // 0.79 mm
        ("5", "400000", "px"), // 105 833 mm
    ] {
        let e = err(&sides(s, l, u));
        assert_eq!(e.reason, PresetReason::SideOutOfRange, "{s} {l} {u}");
    }
    // At the limits it is fine, and short == long (a square) is fine.
    PresetList::parse(&sides("1", "100000", "mm")).unwrap();
    PresetList::parse(&sides("50", "50", "mm")).unwrap();
    // Integers and floats both read.
    PresetList::parse(&sides("50.0", "60", "mm")).unwrap();
}

#[test]
fn rule_g_ids_names_notes() {
    let with = |id: &str, name: &str, note: Option<&str>| {
        let note = note.map(|n| format!("note=\"{n}\"\n")).unwrap_or_default();
        one_preset(&format!(
            "id=\"{id}\"\nname=\"{name}\"\n{note}short_side=1\nlong_side=2\nunit=\"mm\"\n"
        ))
    };
    for bad in ["", "A4", "a_4", "a 4", "a.4", "ä4", "a4!", "a/4", " a4"] {
        let e = err(&with(bad, "P", None));
        assert_eq!(e.reason, PresetReason::BadId, "id {bad:?}");
    }
    for good in ["a4", "slide-16-9", "0", "-", "a--b"] {
        PresetList::parse(&with(good, "P", None)).unwrap_or_else(|e| panic!("{good}: {e:?}"));
    }
    // names
    assert_eq!(err(&with("p", "", None)).reason, PresetReason::BadName);
    assert_eq!(err(&with("p", "   ", None)).reason, PresetReason::BadName);
    assert_eq!(
        err(&with("p", &"x".repeat(25), None)).reason,
        PresetReason::BadName
    );
    PresetList::parse(&with("p", &"x".repeat(24), None)).unwrap();
    // 24 after trimming is fine even when padded.
    PresetList::parse(&with("p", &format!("  {}  ", "x".repeat(24)), None)).unwrap();
    // 24 characters, not bytes: 24 multibyte chars are fine.
    PresetList::parse(&with("p", &"é".repeat(24), None)).unwrap();
    assert_eq!(
        err(&with("p", &"é".repeat(25), None)).reason,
        PresetReason::BadName
    );
    // notes
    PresetList::parse(&with("p", "P", Some(&"n".repeat(60)))).unwrap();
    assert_eq!(
        err(&with("p", "P", Some(&"n".repeat(61)))).reason,
        PresetReason::NoteTooLong
    );
    // group id
    let g = "format = 1\n[[group]]\nid=\"G_1\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n";
    let e = err(g);
    assert_eq!(e.reason, PresetReason::BadId);
    assert!(matches!(e.subject, PresetSubject::Group(_)));
    // group name
    let g = "format = 1\n[[group]]\nid=\"g\"\nname=\"\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"mm\"\n";
    assert_eq!(err(g).reason, PresetReason::BadName);
}

#[test]
fn rule_h_same_size_within_tolerance() {
    let two = |a: &str, ua: &str, b: &str, ub: &str, gb: bool| {
        let (sa, la) = a.split_once('x').unwrap();
        let (sb, lb) = b.split_once('x').unwrap();
        let second = format!(
            "[[group.preset]]\nid=\"q\"\nname=\"Q\"\nshort_side={sb}\nlong_side={lb}\nunit=\"{ub}\"\n"
        );
        let head =
            "format = 1\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n";
        let first = format!(
            "[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side={sa}\nlong_side={la}\nunit=\"{ua}\"\n"
        );
        if gb {
            format!(
                "{head}{first}[[group]]\nid=\"h\"\nname=\"H\"\ndefault_orientation=\"portrait\"\n{second}"
            )
        } else {
            format!("{head}{first}{second}")
        }
    };
    // Equal; within 0.01 on both sides; across groups; in other units.
    for t in [
        two("100x200", "mm", "100x200", "mm", false),
        two("100x200", "mm", "100.005x200.005", "mm", false),
        two("100x200", "mm", "100x200.01", "mm", false),
        two("100x200", "mm", "100x200", "mm", true),
        two("25.4x50.8", "mm", "1x2", "in", false),
        two("25.4x50.8", "mm", "96x192", "px", false),
    ] {
        let e = err(&t);
        assert!(
            matches!(e.reason, PresetReason::SameSizeAs(_)),
            "{t}\n{e:?}"
        );
    }
    // Just beyond the tolerance: allowed.
    PresetList::parse(&two("100x200", "mm", "100x200.02", "mm", false)).unwrap();
    // Same short side, different long side: allowed (A3/A4 share 297).
    PresetList::parse(&two("100x200", "mm", "200x300", "mm", false)).unwrap();
}

#[test]
fn the_error_names_the_offender() {
    let e = err(&one_preset(
        "id=\"bad-one\"\nname=\"P\"\nshort_side=1\nlong_side=2\nunit=\"cm\"\n",
    ));
    assert_eq!(e.subject, PresetSubject::Preset("bad-one".into()));
    let text = format!("{e}");
    assert!(text.contains("bad-one"), "{text}");
}

#[test]
fn hostile_inputs_do_not_panic() {
    for t in [
        "\u{0}",
        "format = 1\n[[group]]\n[[group.preset]]\n",
        "format = 1\n[group]\nid = 1\n",
        "format = 1\n[[group]]\nid = \"g\"\nname = \"G\"\ndefault_orientation = \"portrait\"\npreset = 3\n",
        "format = 99999999999999999999\n",
        "format = 1\ngroup = []\n",
        "format = 1\n[[group]]\nid=\"g\"\nname=\"G\"\ndefault_orientation=\"portrait\"\n[[group.preset]]\nid=\"p\"\nname=\"P\"\nshort_side=1e308\nlong_side=1e309\nunit=\"mm\"\n",
    ] {
        let _ = PresetList::parse(t);
    }
    let big = "x".repeat(1_000_000);
    let _ = PresetList::parse(&big);
    let many = "[[group]]\n".repeat(10_000);
    let _ = PresetList::parse(&format!("format = 1\n{many}"));
}

// ------------------------------------------------- matching / Orientation

#[test]
fn matching_follows_the_tolerance_and_both_orientations() {
    let list = shipped();
    let name = |w: f64, h: f64| {
        list.matching(DocumentSize::from_mm(w, h))
            .map(|(_, p)| p.name.clone())
    };
    assert_eq!(name(210.0, 297.0).as_deref(), Some("A4"));
    assert_eq!(name(297.0, 210.0).as_deref(), Some("A4"));
    assert_eq!(name(210.004, 297.0).as_deref(), Some("A4"));
    assert_eq!(name(210.0, 297.009).as_deref(), Some("A4"));
    assert_eq!(name(211.0, 297.0), None);
    assert_eq!(name(210.02, 297.0), None);
    assert_eq!(name(508.0, 285.75).as_deref(), Some("16:9"));
    assert_eq!(name(285.75, 508.0).as_deref(), Some("16:9"));
    assert_eq!(name(317.5, 508.0).as_deref(), Some("16:10"));
    assert_eq!(name(203.2, 270.933_333).as_deref(), Some("4:3"));
    assert_eq!(name(0.0, 0.0), None);
    assert_eq!(name(f64::NAN, 297.0), None);
    assert_eq!(name(-210.0, 297.0), None);
    // Every shipped preset matches itself and only itself, in either orientation.
    for (_, _, s, l) in all(&list) {
        let hit = list.matching(DocumentSize::from_mm(s, l)).unwrap();
        let hit2 = list.matching(DocumentSize::from_mm(l, s)).unwrap();
        assert_eq!(hit.1.id, hit2.1.id);
    }
    // The group is reported.
    assert_eq!(
        list.matching(DocumentSize::from_mm(508.0, 285.75))
            .unwrap()
            .0
            .id,
        "slides"
    );
}

#[test]
fn orientation_of_a_size() {
    let o = |w, h| Orientation::of(DocumentSize::from_mm(w, h));
    assert_eq!(o(210.0, 297.0), Some(Orientation::Portrait));
    assert_eq!(o(297.0, 210.0), Some(Orientation::Landscape));
    assert_eq!(o(100.0, 100.0), None);
    assert_eq!(o(100.0, 100.009), None);
    assert_eq!(o(100.0, 100.02), Some(Orientation::Portrait));
    assert_eq!(o(100.02, 100.0), Some(Orientation::Landscape));
}

#[test]
fn an_empty_list_matches_nothing_and_does_not_panic() {
    let list = PresetList::default();
    assert!(list.groups.is_empty());
    assert!(list.matching(DocumentSize::from_mm(210.0, 297.0)).is_none());
    assert_eq!(list.presets().count(), 0);
}
