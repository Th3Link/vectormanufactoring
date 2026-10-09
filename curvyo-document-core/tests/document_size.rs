//! Golden-file and round-trip tests for the document size and the display unit
//! (`specs/0015-document-size-and-rulers/` criteria 13, 36, 38 and 39), through
//! the crate's public API only.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{
    DisplayUnit, Document, DocumentSize, Length, NodeId, ObjectSnapshot, Point, RectBounds, Shape,
    pack, unpack,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("reading fixture {}: {err}", path.display()))
}

fn rect_origin(document: &Document, id: NodeId) -> Point {
    let Some(ObjectSnapshot::Primitive(primitive)) = document.object(id) else {
        panic!("a primitive");
    };
    let Shape::Rect { bounds, .. } = primitive.shape else {
        panic!("a rect");
    };
    bounds.origin
}

fn first_rect(document: &Document) -> NodeId {
    document
        .object_ids()
        .into_iter()
        .find(|id| document.primitive(*id).is_some())
        .expect("a primitive")
}

/// Criterion 13: every earlier build's file opens at the size stored in it,
/// 210 x 297 mm so far, in the default display unit.
#[test]
fn every_earlier_golden_opens_at_a4_in_mm() {
    for name in [
        "valid.curvyo",
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "corner_radii_per_corner.curvyo",
        "styles_v7.curvyo",
        "styles_v7_mergeable_stops.curvyo",
    ] {
        let document = unpack(2, &fixture(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            document.size(),
            DocumentSize::from_mm(210.0, 297.0),
            "{name}"
        );
        assert_eq!(document.display_unit(), DisplayUnit::Mm, "{name}");
    }
}

/// Criterion 13: a file whose size values are not finite or out of range
/// opens as A4 on both axes, with its objects where they were, and is not
/// refused.
#[test]
fn a_file_with_damaged_size_values_opens_at_a4() {
    let document = unpack(2, &fixture("size_damaged_v7.curvyo")).expect("not refused");

    assert_eq!(document.size(), DocumentSize::from_mm(210.0, 297.0));
    assert_eq!(
        rect_origin(&document, first_rect(&document)),
        Point::new(10.0, 10.0)
    );
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    assert!((json["size"]["width"].as_f64().unwrap() - 210.0).abs() < 1e-9);
}

/// Criteria 35, 36 and 38: a stored unit survives a save and a reopen, and the
/// size typed in inches is the exact millimetre value.
#[test]
fn a_file_with_a_stored_display_unit_opens_and_round_trips() {
    let document = unpack(2, &fixture("display_unit_in_v7.curvyo")).expect("opens");

    assert_eq!(document.display_unit(), DisplayUnit::In);
    let size = document.size();
    assert!((size.width.in_unit(DisplayUnit::In) - 8.5).abs() < 1e-9);
    assert!((size.height.in_unit(DisplayUnit::In) - 11.0).abs() < 1e-9);
    assert!((size.width.as_mm() - 215.9).abs() < 1e-9);
    // 50 x 50 mm at (10, 10) in A4, moved by ((215.9 - 210) / 2, (279.4 - 297) / 2).
    let origin = rect_origin(&document, first_rect(&document));
    assert!(
        (origin.x - 12.95).abs() < 1e-9 && (origin.y - 1.2).abs() < 1e-9,
        "{origin:?}"
    );

    let again = unpack(3, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(again.display_unit(), DisplayUnit::In);
    assert_eq!(again.size(), size);
}

/// The unit is stored next to the size in the non-authoritative JSON view,
/// and `format_version` stays 7 (criterion 38: no bump).
#[test]
fn document_json_carries_the_display_unit_and_the_format_stays_7() {
    let document = Document::new(1);
    assert!(document.set_display_unit(DisplayUnit::Cm));
    let json: serde_json::Value = serde_json::from_slice(&document.export_json().unwrap()).unwrap();
    assert_eq!(json["display_unit"], "cm");
    assert_eq!(json["format_version"], 7);
    assert_eq!(curvyo_document_core::CURRENT_FORMAT_VERSION, 7);
}

/// Criterion 39: resize and fit followed by save, close and open keep the
/// size, every position and the unit.
#[test]
fn resize_and_fit_survive_save_and_reopen() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: Point::new(10.0, 10.0),
        width: Length::from_mm(50.0),
        height: Length::from_mm(50.0),
    });
    document
        .resize(DocumentSize::from_mm(300.0, 400.0))
        .unwrap();
    let reopened = unpack(2, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(reopened.size(), DocumentSize::from_mm(300.0, 400.0));
    assert_eq!(rect_origin(&reopened, id), Point::new(55.0, 61.5));

    reopened
        .fit_to_content((Point::new(55.0, 61.5), Point::new(105.0, 111.5)))
        .unwrap();
    assert!(reopened.set_display_unit(DisplayUnit::Cm));
    let again = unpack(3, &pack(&reopened, "0.1.0").unwrap()).unwrap();
    assert_eq!(again.size(), DocumentSize::from_mm(50.0, 50.0));
    assert_eq!(rect_origin(&again, id), Point::new(0.0, 0.0));
    assert_eq!(again.display_unit(), DisplayUnit::Cm);
}
