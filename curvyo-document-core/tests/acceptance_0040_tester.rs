//! Independent tester cases for `0040-document-background`, document-core side:
//! the model (criteria 1 to 3), older files and the format number (4, 5), the
//! round trip (6), strict validation on open (7), the write rules and the commit
//! label (22), a resize that keeps the background (42, 43) and the merge of two
//! peers (45). Written from `specification.md` before the implementation was read.
//! Every damaged file here is built in memory by hand-writing the root registers,
//! not taken from the implementer's fixtures.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::type_complexity,
    missing_docs
)]

use std::io::{Cursor, Read, Write};

use curvyo_document_core::{
    BackgroundPaint, CURRENT_FORMAT_VERSION, Color, Document, DocumentBackground, DocumentSize,
    Length, Opacity, OpenError, Point, RectBounds, pack, unpack,
};
use loro::{LoroDoc, LoroValue};

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn bg(paint: BackgroundPaint, r: u8, g: u8, b: u8, a: f64) -> DocumentBackground {
    DocumentBackground {
        paint,
        color: Color { r, g, b },
        opacity: Opacity::new(a).unwrap(),
    }
}

fn solid(r: u8, g: u8, b: u8, a: f64) -> DocumentBackground {
    bg(BackgroundPaint::Solid, r, g, b, a)
}

fn loro_of(d: &Document) -> LoroDoc {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn changes(d: &Document) -> usize {
    loro_of(d).len_changes()
}

fn ops(d: &Document) -> i64 {
    loro_of(d).oplog_vv().values().map(|c| i64::from(*c)).sum()
}

fn zip_from_loro(loro_bytes: &[u8], format_version: u32) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": format_version,
        "loro_snapshot_version": 1,
        "app_version": "tester-0040",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    writer.finish().unwrap().into_inner()
}

/// A project file with one rectangle whose root map was edited by hand.
fn hand_written(edit: impl FnOnce(&loro::LoroMap)) -> Vec<u8> {
    let base = Document::new(1);
    let _ = base.create_rect(RectBounds {
        origin: Point::new(10.0, 10.0),
        width: Length::from_mm(50.0),
        height: Length::from_mm(50.0),
    });
    let l = LoroDoc::new();
    l.import(&base.export_loro_snapshot().unwrap()).unwrap();
    edit(&l.get_map("root"));
    l.commit();
    zip_from_loro(
        &l.export(loro::ExportMode::Snapshot).unwrap(),
        CURRENT_FORMAT_VERSION,
    )
}

fn list(items: Vec<LoroValue>) -> LoroValue {
    LoroValue::List(items.into())
}

fn merged(a: &Document, b: &Document) -> Document {
    let l = LoroDoc::new();
    l.import(&a.export_loro_snapshot().unwrap()).unwrap();
    l.import(&b.export_loro_snapshot().unwrap()).unwrap();
    l.commit();
    unpack(
        9,
        &zip_from_loro(
            &l.export(loro::ExportMode::Snapshot).unwrap(),
            CURRENT_FORMAT_VERSION,
        ),
    )
    .expect("merged document opens")
}

fn root_value(d: &Document, key: &str) -> Option<LoroValue> {
    loro_of(d)
        .get_map("root")
        .get(key)
        .map(|v| v.get_deep_value())
}

fn last_change_message(d: &Document) -> String {
    let l = loro_of(d);
    let mut out = String::new();
    l.travel_change_ancestors(&[l.oplog_frontiers().iter().next().unwrap()], &mut |m| {
        out = m.message.map(|s| s.to_string()).unwrap_or_default();
        std::ops::ControlFlow::Break(())
    })
    .unwrap();
    out
}

fn manifest_version(bytes: &[u8]) -> u64 {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut f = zip.by_name("manifest.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    serde_json::from_str::<serde_json::Value>(&s).unwrap()["format_version"]
        .as_u64()
        .unwrap()
}

// ------------------------------------------------ AC 1, 2: the model, default

#[test]
fn ac1_ac2_a_new_document_has_the_default_solid_e8e8eb_opaque() {
    let d = Document::new(1);
    let b = d.background();
    assert_eq!(b.paint, BackgroundPaint::Solid);
    assert_eq!(
        b.color,
        Color {
            r: 0xE8,
            g: 0xE8,
            b: 0xEB
        }
    );
    assert_eq!(b.opacity.get(), 1.0);
    assert_eq!(b, DocumentBackground::DEFAULT);
}

#[test]
fn ac1_the_colour_is_kept_while_the_paint_is_none() {
    let d = Document::new(1);
    assert!(d.set_background(bg(BackgroundPaint::None, 12, 34, 56, 0.25)));
    let b = d.background();
    assert_eq!(b.paint, BackgroundPaint::None);
    assert_eq!(
        b.color,
        Color {
            r: 12,
            g: 34,
            b: 56
        }
    );
    assert_eq!(b.opacity.get(), 0.25);
}

// ------------------------------------------------ AC 3: the two registers

#[test]
fn ac3_a_new_document_writes_neither_register() {
    let d = Document::new(1);
    assert_eq!(root_value(&d, "background_paint"), None);
    assert_eq!(root_value(&d, "background_color"), None);
}

#[test]
fn ac3_the_registers_hold_the_string_and_one_four_entry_list() {
    let d = Document::new(1);
    assert!(d.set_background(bg(BackgroundPaint::None, 47, 111, 238, 128.0 / 255.0)));
    assert_eq!(
        root_value(&d, "background_paint"),
        Some(LoroValue::String("none".to_string().into()))
    );
    let Some(LoroValue::List(items)) = root_value(&d, "background_color") else {
        panic!("background_color is one list register");
    };
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], LoroValue::I64(47));
    assert_eq!(items[1], LoroValue::I64(111));
    assert_eq!(items[2], LoroValue::I64(238));
    assert_eq!(items[3], LoroValue::Double(128.0 / 255.0));
    assert!(d.set_background(solid(47, 111, 238, 128.0 / 255.0)));
    assert_eq!(
        root_value(&d, "background_paint"),
        Some(LoroValue::String("solid".to_string().into()))
    );
}

#[test]
fn ac3_alpha_is_stored_as_typed_and_never_rounded_on_read() {
    // 50 % typed in the Opacity field is 0.5, not 128/255; AA/255 from hex stays.
    for a in [
        0.5,
        128.0 / 255.0,
        1.0 / 3.0,
        0.0,
        1.0,
        0.004,
        254.0 / 255.0,
    ] {
        let d = Document::new(1);
        // force a write even when the colour equals the default
        assert!(d.set_background(solid(1, 2, 3, a)));
        assert_eq!(d.background().opacity.get().to_bits(), a.to_bits(), "{a}");
        let back = unpack(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        assert_eq!(
            back.background().opacity.get().to_bits(),
            a.to_bits(),
            "{a}"
        );
    }
}

#[test]
fn ac3_an_absent_register_means_its_default_and_the_other_is_independent() {
    let only_paint = hand_written(|m| m.insert("background_paint", "none").unwrap());
    let d = unpack(3, &only_paint).unwrap();
    assert_eq!(d.background().paint, BackgroundPaint::None);
    assert_eq!(d.background().color, DocumentBackground::DEFAULT.color);
    assert_eq!(d.background().opacity.get(), 1.0);

    let only_colour = hand_written(|m| {
        m.insert(
            "background_color",
            list(vec![
                LoroValue::I64(9),
                LoroValue::I64(8),
                LoroValue::I64(7),
                LoroValue::Double(0.5),
            ]),
        )
        .unwrap();
    });
    let d = unpack(3, &only_colour).unwrap();
    assert_eq!(d.background().paint, BackgroundPaint::Solid);
    assert_eq!(d.background().color, Color { r: 9, g: 8, b: 7 });
    assert_eq!(d.background().opacity.get(), 0.5);
}

#[test]
fn ac3_the_json_view_carries_the_background() {
    let d = Document::new(1);
    let _ = d.set_background(bg(BackgroundPaint::None, 47, 111, 238, 0.5));
    let j: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    assert_eq!(j["background"]["paint"], "none");
    assert_eq!(
        j["background"]["color"],
        serde_json::json!([47, 111, 238, 0.5])
    );
}

// --------------------------------- AC 4: older files, nothing written on open

const OLDER: &[&str] = &[
    "format_version_1.curvyo",
    "paths_v2.curvyo",
    "primitives_v3.curvyo",
    "legacy_corner_radius_v5.curvyo",
    "rotation_v5.curvyo",
    "legacy_gradient_v7.curvyo",
    "display_unit_in_v7.curvyo",
    "compound_v8.curvyo",
    "markers_v9.curvyo",
    "dash_v9.curvyo",
    "corner_radii_per_corner.curvyo",
];

#[test]
fn ac4_every_older_fixture_opens_with_the_default_and_writes_nothing() {
    for name in OLDER {
        let bytes = fixture(name);
        let d = unpack(5, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(d.background(), DocumentBackground::DEFAULT, "{name}");
        let source = LoroDoc::new();
        // the number of changes after open is what the file itself holds
        let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut raw = Vec::new();
        zip.by_name("document.loro")
            .unwrap()
            .read_to_end(&mut raw)
            .unwrap();
        source.import(&raw).unwrap();
        assert_eq!(changes(&d), source.len_changes(), "{name}: open wrote");
        assert_eq!(root_value(&d, "background_paint"), None, "{name}");
        assert_eq!(root_value(&d, "background_color"), None, "{name}");
    }
}

#[test]
fn ac4_every_object_of_an_older_file_is_where_it_was() {
    let baseline: serde_json::Value =
        serde_json::from_slice(&fixture("baseline_pre_0015_export.json")).unwrap();
    for (name, old) in baseline.as_object().unwrap() {
        let d = unpack(2, &fixture(name)).unwrap();
        let mut now: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
        let m = now.as_object_mut().unwrap();
        m.remove("display_unit");
        m.remove("format_version");
        let background = m.remove("background").expect("a background member");
        assert_eq!(background["paint"], "solid", "{name}");
        assert_eq!(
            background["color"],
            serde_json::json!([232, 232, 235, 1.0]),
            "{name}"
        );
        let mut old = old.clone();
        old.as_object_mut().unwrap().remove("format_version");
        assert_eq!(&now, &old, "{name}: objects moved");
    }
}

#[test]
fn ac4_the_background_of_an_opened_older_file_is_editable_and_then_saved() {
    let d = unpack(5, &fixture("markers_v9.curvyo")).unwrap();
    let n = d.object_ids().len();
    assert!(d.set_background(solid(1, 2, 3, 1.0)));
    let back = unpack(5, &pack(&d, "0.1.0").unwrap()).unwrap();
    assert_eq!(back.background(), solid(1, 2, 3, 1.0));
    assert_eq!(back.object_ids().len(), n);
}

// ------------------------------------------------ AC 5: the format number

#[test]
fn ac5_the_writer_writes_the_named_constant_and_it_is_ten() {
    let bytes = pack(&Document::new(1), "0.1.0").unwrap();
    assert_eq!(manifest_version(&bytes), u64::from(CURRENT_FORMAT_VERSION));
    // `main` was 9 before this slice, nothing else merged first (specs/README.md plan).
    assert_eq!(CURRENT_FORMAT_VERSION, 10);
}

#[test]
fn ac5_a_reader_refuses_a_newer_number_and_a_file_without_registers_is_valid() {
    let l = loro_of(&Document::new(1));
    let newer = zip_from_loro(
        &l.export(loro::ExportMode::Snapshot).unwrap(),
        CURRENT_FORMAT_VERSION + 1,
    );
    match unpack(1, &newer) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!(found, CURRENT_FORMAT_VERSION + 1);
            assert_eq!(supported, CURRENT_FORMAT_VERSION);
        }
        other => panic!("expected FormatTooNew, got {:?}", other.map(|_| ())),
    }
    let current = zip_from_loro(
        &l.export(loro::ExportMode::Snapshot).unwrap(),
        CURRENT_FORMAT_VERSION,
    );
    let d = unpack(1, &current).unwrap();
    assert_eq!(d.background(), DocumentBackground::DEFAULT);
    // the version before this one is still read
    let nine = zip_from_loro(&l.export(loro::ExportMode::Snapshot).unwrap(), 9);
    assert_eq!(
        unpack(1, &nine).unwrap().background(),
        DocumentBackground::DEFAULT
    );
}

// ------------------------------------------------ AC 6: the round trip

#[test]
fn ac6_paint_and_colour_survive_save_and_open_component_exact() {
    let cases = [
        bg(BackgroundPaint::None, 47, 111, 238, 128.0 / 255.0),
        bg(BackgroundPaint::Solid, 47, 111, 238, 128.0 / 255.0),
        bg(BackgroundPaint::Solid, 0, 0, 0, 0.0),
        bg(BackgroundPaint::None, 255, 255, 255, 1.0),
        bg(BackgroundPaint::Solid, 0, 255, 1, 0.5),
        bg(BackgroundPaint::Solid, 255, 0, 254, 1.0 / 3.0),
    ];
    for want in cases {
        let d = Document::new(1);
        let _ = d.set_background(want);
        let back = unpack(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        let got = back.background();
        assert_eq!(got.paint, want.paint);
        assert_eq!(got.color, want.color);
        assert_eq!(got.opacity.get().to_bits(), want.opacity.get().to_bits());
    }
}

#[test]
fn ac6_every_hex_alpha_and_every_percent_round_trips_bit_exact() {
    for aa in 0..=255u32 {
        let a = f64::from(aa) / 255.0;
        let d = Document::new(1);
        let _ = d.set_background(solid(10, 20, 30, a));
        let back = unpack(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        assert_eq!(
            back.background().opacity.get().to_bits(),
            a.to_bits(),
            "{aa}/255"
        );
    }
    for n in 0..=100u32 {
        let a = f64::from(n) / 100.0;
        let d = Document::new(1);
        let _ = d.set_background(solid(10, 20, 30, a));
        let back = unpack(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        assert_eq!(
            back.background().opacity.get().to_bits(),
            a.to_bits(),
            "{n}/100"
        );
    }
}

#[test]
fn ac6_none_keeps_its_colour_after_the_round_trip_and_solid_shows_it_again() {
    let d = Document::new(1);
    let _ = d.set_background(solid(47, 111, 238, 128.0 / 255.0));
    let before = d.background();
    let _ = d.set_background(DocumentBackground {
        paint: BackgroundPaint::None,
        ..before
    });
    let back = unpack(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    assert_eq!(back.background().paint, BackgroundPaint::None);
    let _ = back.set_background(DocumentBackground {
        paint: BackgroundPaint::Solid,
        ..back.background()
    });
    assert_eq!(back.background(), before);
}

#[test]
fn ac6_the_golden_none_fixture_reads_as_specified_by_the_adr() {
    let d = unpack(1, &fixture("background_v10.curvyo")).unwrap();
    let b = d.background();
    assert_eq!(b.paint, BackgroundPaint::None);
    assert_eq!(
        b.color,
        Color {
            r: 47,
            g: 111,
            b: 238
        }
    );
    assert_eq!(b.opacity.get().to_bits(), (128.0_f64 / 255.0).to_bits());
}

// ------------------------------------------------ AC 7: strict validation

fn colour_with(items: Vec<LoroValue>) -> impl Fn(&loro::LoroMap) {
    move |m| m.insert("background_color", list(items.clone())).unwrap()
}

fn i(v: i64) -> LoroValue {
    LoroValue::I64(v)
}
fn f(v: f64) -> LoroValue {
    LoroValue::Double(v)
}

#[test]
fn ac7_each_malformed_register_is_damaged_and_each_has_its_own_case() {
    type Edit = Box<dyn Fn(&loro::LoroMap)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "unknown paint",
            Box::new(|m| m.insert("background_paint", "transparent").unwrap()),
        ),
        (
            "paint upper case",
            Box::new(|m| m.insert("background_paint", "NONE").unwrap()),
        ),
        (
            "paint empty",
            Box::new(|m| m.insert("background_paint", "").unwrap()),
        ),
        (
            "paint padded",
            Box::new(|m| m.insert("background_paint", "solid ").unwrap()),
        ),
        (
            "paint number",
            Box::new(|m| m.insert("background_paint", 1_i64).unwrap()),
        ),
        (
            "paint bool",
            Box::new(|m| m.insert("background_paint", true).unwrap()),
        ),
        (
            "colour three entries",
            Box::new(colour_with(vec![i(1), i(2), i(3)])),
        ),
        (
            "colour five entries",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(1.0), f(1.0)])),
        ),
        ("colour empty list", Box::new(colour_with(vec![]))),
        ("colour one entry", Box::new(colour_with(vec![i(1)]))),
        (
            "alpha 1.5",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(1.5)])),
        ),
        (
            "alpha negative",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(-0.1)])),
        ),
        (
            "alpha NaN",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(f64::NAN)])),
        ),
        (
            "alpha +inf",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(f64::INFINITY)])),
        ),
        (
            "alpha -inf",
            Box::new(colour_with(vec![i(1), i(2), i(3), f(f64::NEG_INFINITY)])),
        ),
        (
            "alpha string",
            Box::new(colour_with(vec![
                i(1),
                i(2),
                i(3),
                LoroValue::String("1".to_string().into()),
            ])),
        ),
        (
            "red as double 0.5",
            Box::new(colour_with(vec![f(0.5), i(2), i(3), f(1.0)])),
        ),
        (
            "red as double 1.0",
            Box::new(colour_with(vec![f(1.0), i(2), i(3), f(1.0)])),
        ),
        (
            "green as double",
            Box::new(colour_with(vec![i(1), f(2.0), i(3), f(1.0)])),
        ),
        (
            "blue as double",
            Box::new(colour_with(vec![i(1), i(2), f(3.0), f(1.0)])),
        ),
        (
            "red 256",
            Box::new(colour_with(vec![i(256), i(2), i(3), f(1.0)])),
        ),
        (
            "green 300",
            Box::new(colour_with(vec![i(1), i(300), i(3), f(1.0)])),
        ),
        (
            "blue -1",
            Box::new(colour_with(vec![i(1), i(2), i(-1), f(1.0)])),
        ),
        (
            "red i64 max",
            Box::new(colour_with(vec![i(i64::MAX), i(2), i(3), f(1.0)])),
        ),
        (
            "red string",
            Box::new(colour_with(vec![
                LoroValue::String("1".to_string().into()),
                i(2),
                i(3),
                f(1.0),
            ])),
        ),
        (
            "red null",
            Box::new(colour_with(vec![LoroValue::Null, i(2), i(3), f(1.0)])),
        ),
        (
            "colour is a string",
            Box::new(|m| m.insert("background_color", "#FF0000").unwrap()),
        ),
        (
            "colour is a number",
            Box::new(|m| m.insert("background_color", 7_i64).unwrap()),
        ),
        (
            "colour is null",
            Box::new(|m| m.insert("background_color", LoroValue::Null).unwrap()),
        ),
        (
            "colour is a bool",
            Box::new(|m| m.insert("background_color", true).unwrap()),
        ),
        (
            "colour nested list",
            Box::new(colour_with(vec![list(vec![i(1)]), i(2), i(3), f(1.0)])),
        ),
        (
            "both malformed",
            Box::new(|m| {
                m.insert("background_paint", "x").unwrap();
                m.insert("background_color", list(vec![i(1)])).unwrap();
            }),
        ),
        (
            "paint fine, colour bad",
            Box::new(|m| {
                m.insert("background_paint", "solid").unwrap();
                m.insert("background_color", list(vec![i(1), i(2), i(3), f(2.0)]))
                    .unwrap();
            }),
        ),
        (
            "colour fine, paint bad",
            Box::new(|m| {
                m.insert("background_paint", "dark").unwrap();
                m.insert("background_color", list(vec![i(1), i(2), i(3), f(1.0)]))
                    .unwrap();
            }),
        ),
    ];
    for (what, edit) in cases {
        let bytes = hand_written(|m| edit(m));
        match unpack(4, &bytes) {
            Err(OpenError::Damaged) => {}
            Err(e) => panic!("{what}: expected Damaged, got {e:?}"),
            Ok(d) => panic!("{what}: opened, background {:?}", d.background()),
        }
    }
}

#[test]
fn ac7_a_well_formed_edge_value_is_not_damaged() {
    let good: Vec<(&str, Vec<LoroValue>)> = vec![
        ("all zero", vec![i(0), i(0), i(0), f(0.0)]),
        ("all max", vec![i(255), i(255), i(255), f(1.0)]),
        ("tiny alpha", vec![i(1), i(2), i(3), f(f64::MIN_POSITIVE)]),
        (
            "one below one",
            vec![i(1), i(2), i(3), f(1.0 - f64::EPSILON)],
        ),
    ];
    for (what, items) in good {
        let bytes = hand_written(colour_with(items));
        assert!(unpack(4, &bytes).is_ok(), "{what} must open");
    }
    for paint in ["none", "solid"] {
        let bytes = hand_written(|m| m.insert("background_paint", paint).unwrap());
        assert!(unpack(4, &bytes).is_ok(), "{paint}");
    }
}

#[test]
fn ac7_alpha_given_as_a_whole_number_is_reported_not_asserted() {
    // The spec says "one finite number from 0 to 1"; an integer 1 or 0 is a number.
    // Both answers are defensible, so this records the behaviour without judging it.
    for v in [0_i64, 1_i64] {
        let bytes = hand_written(colour_with(vec![i(1), i(2), i(3), i(v)]));
        println!(
            "alpha as integer {v}: {:?}",
            unpack(4, &bytes).map(|d| d.background())
        );
    }
}

#[test]
fn ac7_a_missing_register_is_never_damaged() {
    for what in ["both", "paint only", "colour only"] {
        let bytes = hand_written(|m| match what {
            "paint only" => m.insert("background_paint", "none").unwrap(),
            "colour only" => m
                .insert("background_color", list(vec![i(1), i(2), i(3), f(1.0)]))
                .unwrap(),
            _ => {}
        });
        assert!(unpack(4, &bytes).is_ok(), "{what}");
    }
}

#[test]
fn ac7_the_implementers_damaged_fixtures_are_also_refused() {
    for name in [
        "background_paint_unknown_v10.curvyo",
        "background_color_three_v10.curvyo",
        "background_color_range_v10.curvyo",
        "background_color_type_v10.curvyo",
    ] {
        assert!(
            matches!(unpack(1, &fixture(name)), Err(OpenError::Damaged)),
            "{name}"
        );
    }
}

// ------------------------------------------------ AC 22: the write rules

#[test]
fn ac22_a_value_equal_to_the_stored_one_writes_nothing() {
    let d = Document::new(1);
    let (c, o) = (changes(&d), ops(&d));
    assert!(!d.set_background(DocumentBackground::DEFAULT));
    assert_eq!((changes(&d), ops(&d)), (c, o));
    assert!(d.set_background(solid(1, 2, 3, 0.5)));
    let (c, o) = (changes(&d), ops(&d));
    assert!(!d.set_background(solid(1, 2, 3, 0.5)));
    assert_eq!((changes(&d), ops(&d)), (c, o));
}

#[test]
fn ac22_a_paint_edit_writes_only_the_paint_register() {
    let d = Document::new(1);
    let o = ops(&d);
    assert!(d.set_background(DocumentBackground {
        paint: BackgroundPaint::None,
        ..DocumentBackground::DEFAULT
    }));
    assert_eq!(ops(&d), o + 1, "one register");
    assert!(root_value(&d, "background_paint").is_some());
    assert_eq!(root_value(&d, "background_color"), None);
    // and back
    let o = ops(&d);
    assert!(d.set_background(DocumentBackground::DEFAULT));
    assert_eq!(ops(&d), o + 1);
    assert_eq!(root_value(&d, "background_color"), None);
}

#[test]
fn ac22_a_colour_or_opacity_edit_writes_only_the_colour_register() {
    let d = Document::new(1);
    let o = ops(&d);
    assert!(d.set_background(solid(9, 9, 9, 1.0)));
    assert_eq!(ops(&d), o + 1);
    assert_eq!(root_value(&d, "background_paint"), None);
    let o = ops(&d);
    assert!(d.set_background(solid(9, 9, 9, 0.5)));
    assert_eq!(ops(&d), o + 1);
    assert_eq!(root_value(&d, "background_paint"), None);
}

#[test]
fn ac22_each_edit_is_one_commit_labelled_set_document_background() {
    let d = Document::new(1);
    let c = changes(&d);
    assert!(d.set_background(solid(9, 8, 7, 0.5)));
    assert_eq!(changes(&d), c + 1);
    assert_eq!(last_change_message(&d), "set_document_background");
    // paint and colour together are still one commit. Loro merges adjacent commits of
    // one peer, so a commit with another label separates them (as the 0017 tests do).
    assert!(d.set_display_unit(curvyo_document_core::DisplayUnit::In));
    let c = changes(&d);
    assert!(d.set_background(bg(BackgroundPaint::None, 1, 1, 1, 0.25)));
    assert_eq!(changes(&d), c + 1);
    assert_eq!(last_change_message(&d), "set_document_background");
}

// ------------------------------------------------ AC 42, 43: resize keeps it

#[test]
fn ac42_resize_and_fit_write_no_background_register_and_keep_the_value() {
    let make = |with_bg: bool| {
        let d = Document::new(1);
        let _ = d.create_rect(RectBounds {
            origin: Point::new(10.0, 10.0),
            width: Length::from_mm(50.0),
            height: Length::from_mm(50.0),
        });
        if with_bg {
            assert!(d.set_background(solid(255, 0, 0, 1.0)));
        }
        d
    };
    let plain = make(false);
    let red = make(true);
    let (po, ro) = (ops(&plain), ops(&red));
    let a3 = DocumentSize::from_mm(297.0, 420.0);
    assert!(plain.resize(a3).unwrap());
    assert!(red.resize(a3).unwrap());
    assert_eq!(ops(&plain) - po, ops(&red) - ro, "same number of writes");
    assert_eq!(red.background(), solid(255, 0, 0, 1.0));
    assert_eq!(red.size(), a3);
    let content = (Point::new(10.0, 10.0), Point::new(60.0, 60.0));
    let (po, ro) = (ops(&plain), ops(&red));
    plain.fit_to_content(content).unwrap();
    red.fit_to_content(content).unwrap();
    assert_eq!(ops(&plain) - po, ops(&red) - ro);
    assert_eq!(red.background(), solid(255, 0, 0, 1.0));
}

#[test]
fn ac43_a_unit_change_does_not_touch_the_background() {
    let d = Document::new(1);
    let _ = d.set_background(bg(BackgroundPaint::None, 3, 2, 1, 0.5));
    let before = d.background();
    let _ = d.set_display_unit(curvyo_document_core::DisplayUnit::In);
    assert_eq!(d.background(), before);
}

// ------------------------------------------------ AC 45: merging peers

#[test]
fn ac45_two_peers_setting_the_colour_end_with_one_complete_colour() {
    for (first, second) in [(1_u64, 2_u64), (2, 1)] {
        let base = pack(&Document::new(1), "0.1.0").unwrap();
        let a = unpack(first, &base).unwrap();
        let b = unpack(second, &base).unwrap();
        assert!(a.set_background(solid(0x11, 0x22, 0x33, 1.0)));
        assert!(b.set_background(solid(0x44, 0x55, 0x66, 1.0)));
        let ab = merged(&a, &b);
        let ba = merged(&b, &a);
        assert_eq!(
            ab.background(),
            ba.background(),
            "both peers hold the same colour"
        );
        let got = ab.background().color;
        assert!(
            got == Color {
                r: 0x11,
                g: 0x22,
                b: 0x33
            } || got
                == Color {
                    r: 0x44,
                    g: 0x55,
                    b: 0x66
                },
            "a mix of channels: {got:?}"
        );
    }
}

#[test]
fn ac45_alpha_and_channels_from_two_peers_are_never_mixed() {
    // A types an opacity, B picks a colour: the alpha 0.5 must stay with channels (1,2,3)
    // or the alpha 1 with channels (9,9,9). Never (9,9,9) at 0.5 or (1,2,3) at 1.
    let base = pack(&Document::new(1), "0.1.0").unwrap();
    let a = unpack(1, &base).unwrap();
    let b = unpack(2, &base).unwrap();
    let _ = a.set_background(solid(1, 2, 3, 0.5));
    let _ = b.set_background(solid(9, 9, 9, 1.0));
    let m = merged(&a, &b).background();
    let ok = (m.color == Color { r: 1, g: 2, b: 3 } && m.opacity.get() == 0.5)
        || (m.color == Color { r: 9, g: 9, b: 9 } && m.opacity.get() == 1.0);
    assert!(ok, "{m:?}");
}

#[test]
fn ac45_paint_from_one_peer_and_colour_from_the_other_both_survive() {
    let base = pack(&Document::new(1), "0.1.0").unwrap();
    let a = unpack(1, &base).unwrap();
    let b = unpack(2, &base).unwrap();
    assert!(a.set_background(DocumentBackground {
        paint: BackgroundPaint::None,
        ..DocumentBackground::DEFAULT
    }));
    assert!(b.set_background(solid(0x44, 0x55, 0x66, 0.75)));
    for m in [merged(&a, &b), merged(&b, &a)] {
        let bgd = m.background();
        assert_eq!(bgd.paint, BackgroundPaint::None);
        assert_eq!(
            bgd.color,
            Color {
                r: 0x44,
                g: 0x55,
                b: 0x66
            }
        );
        assert_eq!(bgd.opacity.get(), 0.75);
    }
}

// ------------------------------------------------ hostile typed values

#[test]
fn hostile_opacity_values_cannot_be_constructed() {
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.001,
        1.000_001,
        2.0,
    ] {
        assert!(Opacity::new(bad).is_err(), "{bad}");
    }
}

#[test]
fn opaque_edge_values_are_stored_and_read_back() {
    let d = Document::new(1);
    for (r, g, b, a) in [
        (0, 0, 0, 0.0),
        (255, 255, 255, 1.0),
        (0, 255, 0, f64::MIN_POSITIVE),
    ] {
        let _ = d.set_background(solid(r, g, b, a));
        assert_eq!(d.background(), solid(r, g, b, a));
    }
}
