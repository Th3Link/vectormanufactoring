//! File-format tests for the odd-length dash list of `0017-style-panel-rework`
//! (`specs/0017-style-panel-rework/adrs.md`, decision 2): the format version
//! that introduced it, the golden container `dash_v9.curvyo`, and a list of any
//! length reading back as stored.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Read};
use std::path::PathBuf;

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, DashPattern, Document, EllipseFrame, Length, ObjectSnapshot, Point,
    RectBounds, StyleEdit, pack, unpack,
};
use zip::ZipArchive;

/// The format version of the build that introduced odd dash lists. A later
/// bump moves `CURRENT_FORMAT_VERSION` on and leaves this number and the
/// golden alone: the golden is the file that build wrote, and every later build
/// has to open it. The next format bump moves the one literal pin on
/// `CURRENT_FORMAT_VERSION` (the test below) to that feature's own test file;
/// this constant and the golden stay as they are.
const DASH_FORMAT_VERSION: u32 = 9;

const GOLDEN: &str = "dash_v9.curvyo";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn manifest_version(container: &[u8]) -> u64 {
    let mut archive = ZipArchive::new(Cursor::new(container)).expect("zip");
    let mut bytes = Vec::new();
    archive
        .by_name("manifest.json")
        .expect("manifest")
        .read_to_end(&mut bytes)
        .expect("read");
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    manifest["format_version"].as_u64().unwrap()
}

fn lists(document: &Document) -> Vec<Vec<f64>> {
    document
        .object_ids()
        .into_iter()
        .map(|id| match document.object(id).unwrap() {
            ObjectSnapshot::Path(p) => p.style.stroke.dash.as_slice().to_vec(),
            ObjectSnapshot::Primitive(p) => p.style.stroke.dash.as_slice().to_vec(),
        })
        .collect()
}

/// A rectangle with an odd list, an ellipse with 17 numbers (past the editing
/// limit of 16, which a file may exceed) and a rectangle with a zero "on"
/// entry.
fn golden_document() -> Document {
    let document = Document::new(1);
    let rect = |x: f64| {
        document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    };
    let odd = rect(0.0);
    let long = document.create_ellipse(EllipseFrame {
        center: Point::new(30.0, 5.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(5.0),
    });
    let zero = rect(50.0);
    let set = |id, lengths: Vec<f64>| {
        document
            .edit_style(
                &[id],
                &StyleEdit::StrokeDash(DashPattern::new(lengths).unwrap()),
            )
            .unwrap();
    };
    set(odd, vec![1.0, 2.0, 4.0]);
    set(long, (1..=17).map(f64::from).collect());
    set(zero, vec![0.0, 3.0]);
    document
}

/// The one literal pin on the format version: it fails if the number moves by
/// accident.
#[test]
fn the_current_format_version_is_the_one_odd_dash_lists_introduced() {
    assert_eq!(CURRENT_FORMAT_VERSION, DASH_FORMAT_VERSION);
}

#[test]
fn the_golden_declares_the_version_that_introduced_odd_lists_and_reads_back() {
    if std::env::var_os("CURVYO_WRITE_FIXTURES").is_some() {
        std::fs::write(
            fixture_path(GOLDEN),
            pack(&golden_document(), "0.1.0").unwrap(),
        )
        .unwrap();
    }
    let bytes = std::fs::read(fixture_path(GOLDEN)).unwrap();
    assert_eq!(manifest_version(&bytes), u64::from(DASH_FORMAT_VERSION));
    let document = unpack(2, &bytes).unwrap();
    let read = lists(&document);
    assert_eq!(read[0], [1.0, 2.0, 4.0], "an odd list is kept as stored");
    assert_eq!(read[1].len(), 17, "17 numbers are shown in full and kept");
    assert_eq!(read[2], [0.0, 3.0], "a zero on entry is kept");
}

#[test]
fn opening_the_golden_writes_nothing_and_saving_keeps_every_list() {
    let bytes = std::fs::read(fixture_path(GOLDEN)).unwrap();
    let document = unpack(2, &bytes).unwrap();
    let saved = unpack(3, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(lists(&saved), lists(&document));
}

#[test]
fn a_list_of_any_length_survives_save_and_reopen() {
    for count in [1_u32, 2, 3, 5, 16, 17, 40] {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let lengths: Vec<f64> = (0..count).map(|n| 0.5 + f64::from(n)).collect();
        document
            .edit_style(
                &[id],
                &StyleEdit::StrokeDash(DashPattern::new(lengths.clone()).unwrap()),
            )
            .unwrap();
        let reopened = unpack(2, &pack(&document, "0.1.0").unwrap()).unwrap();
        assert_eq!(lists(&reopened), [lengths], "{count} numbers");
    }
}

#[test]
fn document_json_exports_an_odd_list_as_stored() {
    let document = golden_document();
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    assert_eq!(
        json["objects"][0]["style"]["stroke"]["dash"],
        serde_json::json!([1.0, 2.0, 4.0])
    );
}
