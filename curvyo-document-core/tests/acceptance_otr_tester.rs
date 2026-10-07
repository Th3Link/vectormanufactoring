//! Independent tester tests for `PathSnapshot::sheared` and the concurrency
//! story of a skew commit (`specs/object-transform-refinements`, criteria
//! 38, 42, 43, 44, 46). Expected values are computed here from the spec's
//! arithmetic, not read back from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names)]

use std::io::{Cursor, Write};

use loro::LoroDoc;
use vecmanf_document_core::{
    AnchorId, AnchorKind, Angle, CURRENT_FORMAT_VERSION, Document, NewAnchor, ObjectSnapshot,
    PathSnapshot, Point, Vec2, pack, unpack,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64, e: f64) -> bool {
    (a - b).abs() <= e
}

fn rich_path(scale: f64) -> (Document, PathSnapshot) {
    let d = Document::new(1);
    let id = d.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(6.0 * scale, -8.0 * scale),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 10.0 * scale))
            },
            NewAnchor {
                handle_in: Vec2::new(-5.0 * scale, -6.0 * scale),
                handle_out: Vec2::new(5.0 * scale, 6.0 * scale),
                kind: AnchorKind::Symmetric,
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(25.0 * scale, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(-3.0 * scale, -7.0 * scale),
                handle_out: Vec2::new(6.0 * scale, 14.0 * scale),
                kind: AnchorKind::Asymmetric,
                ..NewAnchor::corner(AnchorId::new(1, 3), pt(45.0 * scale, 22.0 * scale))
            },
            NewAnchor::corner(AnchorId::new(1, 4), pt(10.0 * scale, 30.0 * scale)),
        ],
        true,
    );
    let p = d.path(id).unwrap();
    (d, p)
}

#[test]
fn sheared_applies_the_linear_map_about_the_pivot_to_points_and_only_the_linear_part_to_handles() {
    let (_d, p) = rich_path(1.0);
    let pivot = pt(3.0, 30.0);
    // x skew with k = 0.4 (ku), theta = 0: u' = u + k (v - pivot.y)
    let s = p.sheared(pivot, 0.4, 0.0);
    for (a, o) in s.anchors.iter().zip(&p.anchors) {
        assert!(near(
            a.point.x,
            o.point.x + 0.4 * (o.point.y - pivot.y),
            1e-12
        ));
        assert!(near(a.point.y, o.point.y, 1e-12));
        assert!(near(
            a.handle_out.x,
            o.handle_out.x + 0.4 * o.handle_out.y,
            1e-12
        ));
        assert!(near(a.handle_out.y, o.handle_out.y, 1e-12));
        assert!(near(
            a.handle_in.x,
            o.handle_in.x + 0.4 * o.handle_in.y,
            1e-12
        ));
        assert_eq!(a.kind, o.kind);
        assert_eq!(a.id, o.id);
    }
    assert_eq!(s.rotation.as_radians(), p.rotation.as_radians());
    assert_eq!(s.closed, p.closed);
    assert_eq!(s.stroke_width, p.stroke_width);
    // y skew
    let s = p.sheared(pivot, 0.0, -0.7);
    for (a, o) in s.anchors.iter().zip(&p.anchors) {
        assert!(near(
            a.point.y,
            o.point.y - 0.7 * (o.point.x - pivot.x),
            1e-12
        ));
        assert!(near(a.point.x, o.point.x, 1e-12));
        assert!(near(
            a.handle_in.y,
            o.handle_in.y - 0.7 * o.handle_in.x,
            1e-12
        ));
    }
}

#[test]
fn sheared_by_zero_is_the_identity_even_for_a_rotated_path() {
    let (d, _) = rich_path(1.0);
    let id = d.object_ids()[0];
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(pt(20.0, 15.0), Angle::from_radians(0.77)))
        .unwrap();
    let p = d.path(id).unwrap();
    let s = p.sheared(pt(5.0, 5.0), 0.0, 0.0);
    for (a, b) in s.anchors.iter().zip(&p.anchors) {
        assert!(near(a.point.x, b.point.x, 1e-9) && near(a.point.y, b.point.y, 1e-9));
    }
}

#[test]
fn sheared_inverse_round_trips_within_1e_9_up_to_a_metre() {
    for scale in [1.0, 100.0, 1000.0] {
        for theta in [0.0, 0.7, -2.5, 3.1] {
            let (d, _) = rich_path(scale);
            let id = d.object_ids()[0];
            if theta != 0.0 {
                let o = d.object(id).unwrap();
                d.rotate_object(&o.rotated(pt(20.0, 15.0), Angle::from_radians(theta)))
                    .unwrap();
            }
            let p = d.path(id).unwrap();
            let pivot = pt(3.0 * scale, 9.0 * scale);
            for (ku, kv) in [(0.9, 0.0), (-3.0, 0.0), (0.0, 1.7), (0.0, -0.2)] {
                let back = p.sheared(pivot, ku, kv).sheared(pivot, -ku, -kv);
                for (a, b) in back.anchors.iter().zip(&p.anchors) {
                    assert!(
                        near(a.point.x, b.point.x, 1e-9 * scale.max(1.0)),
                        "scale {scale} th {theta}"
                    );
                    assert!(near(a.point.y, b.point.y, 1e-9 * scale.max(1.0)));
                    assert!(near(a.handle_out.x, b.handle_out.x, 1e-9 * scale.max(1.0)));
                    assert!(near(a.handle_in.y, b.handle_in.y, 1e-9 * scale.max(1.0)));
                }
            }
        }
    }
}

#[test]
fn skew_commit_survives_save_load_and_keeps_every_kind() {
    let (d, p) = rich_path(1.0);
    let id = d.object_ids()[0];
    let s = p.sheared(pt(0.0, 30.0), 0.5, 0.0);
    let writes: Vec<_> = s
        .anchors
        .iter()
        .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
        .collect();
    d.resize_path(id, &writes, None).unwrap();
    let bytes = pack(&d, "0.1.0").unwrap();
    let re = unpack(9, &bytes).unwrap();
    let q = re.path(re.object_ids()[0]).unwrap();
    for ((a, b), o) in q.anchors.iter().zip(&s.anchors).zip(&p.anchors) {
        assert_eq!(a.point, b.point);
        assert_eq!(a.kind, o.kind);
        assert_eq!(a.kind, b.kind);
    }
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

#[test]
fn concurrent_skew_and_a_peers_rotation_both_survive_the_merge() {
    let (a, p) = rich_path(1.0);
    let bytes = pack(&a, "0.1.0").unwrap();
    let b = unpack(2, &bytes).unwrap();
    let id = a.object_ids()[0];
    let s = p.sheared(pt(0.0, 30.0), 0.5, 0.0);
    let writes: Vec<_> = s
        .anchors
        .iter()
        .map(|x| (x.id, x.point, x.handle_in, x.handle_out))
        .collect();
    a.resize_path(id, &writes, None).unwrap();
    let ob = b.object(id).unwrap();
    b.rotate_object(&ob.rotated(pt(20.0, 15.0), Angle::from_radians(0.3)))
        .unwrap();
    let m = merged(&a, &b);
    let q = m.path(id).unwrap();
    for x in &q.anchors {
        assert!(x.point.x.is_finite() && x.point.y.is_finite());
    }
    assert!(
        near(q.rotation.as_radians(), 0.3, 1e-12),
        "peer's rotation survives: {}",
        q.rotation.as_radians()
    );
    assert!(unpack(7, &pack(&m, "0.1.0").unwrap()).is_ok());
}

#[test]
fn a_skew_against_a_path_a_peer_deleted_is_refused_not_applied() {
    let (a, p) = rich_path(1.0);
    let bytes = pack(&a, "0.1.0").unwrap();
    let id = a.object_ids()[0];
    let s = p.sheared(pt(0.0, 30.0), 0.5, 0.0);
    let writes: Vec<_> = s
        .anchors
        .iter()
        .map(|x| (x.id, x.point, x.handle_in, x.handle_out))
        .collect();
    let c = unpack(3, &bytes).unwrap();
    c.delete_objects(&[id]).unwrap();
    assert!(c.resize_path(id, &writes, None).is_err());
    assert!(c.object(id).is_none());
    let _: Option<ObjectSnapshot> = c.object(id);
}
