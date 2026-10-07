//! Re-verification: two peers edit the same object concurrently, one resizes
//! and one rotates about the centre; after a CRDT merge both edits survive
//! (`specs/0005-object-transform/adrs.md`: separate registers per concern).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    Angle, CURRENT_FORMAT_VERSION, Document, Length, Point, RectBounds, Shape, pack, unpack,
};
use loro::LoroDoc;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn merged(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": 1,
        "app_version": "tester-merge",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(&loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    unpack(9, &bytes).expect("merged document opens")
}

fn base() -> (Vec<u8>, curvyo_document_core::NodeId) {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    });
    (pack(&document, "0.1.0").unwrap(), id)
}

#[test]
fn a_peer_resize_and_a_peer_rotate_about_the_centre_both_survive_the_merge() {
    for flip in [false, true] {
        let (bytes, id) = base();
        let a = unpack(2, &bytes).unwrap();
        let b = unpack(3, &bytes).unwrap();
        a.resize_rect(
            id,
            RectBounds {
                origin: pt(0.0, 0.0),
                width: Length::from_mm(25.0),
                height: Length::from_mm(9.0),
            },
            Length::from_mm(0.0),
            Some(Length::from_mm(2.0)),
        )
        .unwrap();
        b.rotate_object(
            &b.object(id)
                .unwrap()
                .rotated(pt(5.0, 2.0), Angle::from_radians(0.6)),
        )
        .unwrap();
        let m = if flip { merged(&b, &a) } else { merged(&a, &b) };
        let p = m.primitive(id).unwrap();
        let Shape::Rect { bounds, .. } = p.shape else {
            panic!()
        };
        assert_eq!(bounds.width.as_mm(), 25.0, "resize survived (flip {flip})");
        assert_eq!(bounds.height.as_mm(), 9.0);
        assert_eq!(p.stroke_width.as_mm(), 2.0);
        assert!(
            (p.rotation.as_radians() - 0.6).abs() < 1e-12,
            "rotation survived (flip {flip}): {}",
            p.rotation.as_radians()
        );
    }
}

#[test]
fn concurrent_rotations_of_the_same_object_converge_to_one_finite_value() {
    let (bytes, id) = base();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.rotate_object(
        &a.object(id)
            .unwrap()
            .rotated(pt(5.0, 2.0), Angle::from_radians(0.3)),
    )
    .unwrap();
    b.rotate_object(
        &b.object(id)
            .unwrap()
            .rotated(pt(5.0, 2.0), Angle::from_radians(-1.1)),
    )
    .unwrap();
    let ab = merged(&a, &b).primitive(id).unwrap().rotation.as_radians();
    let ba = merged(&b, &a).primitive(id).unwrap().rotation.as_radians();
    assert_eq!(ab, ba, "merge is order independent");
    assert!(
        (ab - 0.3).abs() < 1e-12 || (ab + 1.1).abs() < 1e-12,
        "got {ab}"
    );
}
