//! `Document::duplicate_objects` (`specs/edit-interaction-polish/adrs.md`,
//! decision 2; criteria 34 and 35 of the specification): a copy is every
//! stored value of its original under fresh ids, directly above it, in one
//! commit, and a merge with a concurrent edit of the original converges.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::io::{Cursor, Write};

use loro::LoroDoc;
use vecmanf_document_core::{
    AnchorId, AnchorKind, Angle, CURRENT_FORMAT_VERSION, CopySource, Document, EllipseFrame,
    InnerRatio, Length, NewAnchor, NodeId, ObjectEditError, ObjectSnapshot, Point, PointCount,
    RectBounds, StarFrame, Vec2, pack, unpack,
};

const OFFSET: Vec2 = Vec2::new(12.5, -3.25);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Fresh anchor ids for `count` anchors, from a counter range no fixture uses.
fn fresh(start: u64, count: usize) -> Vec<AnchorId> {
    (0..count as u64)
        .map(|i| AnchorId::new(77, start + i))
        .collect()
}

fn curved_path(document: &Document, closed: bool) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor {
                id: AnchorId::new(1, 2),
                point: pt(10.0, 5.0),
                handle_in: Vec2::new(-3.0, 0.0),
                handle_out: Vec2::new(3.0, 1.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor {
                id: AnchorId::new(1, 3),
                point: pt(20.0, 0.0),
                handle_in: Vec2::new(-2.0, 2.0),
                handle_out: Vec2::new(1.0, -1.0),
                kind: AnchorKind::Asymmetric,
            },
        ],
        closed,
    )
}

/// One of every kind, each with a value in every register the model has: a
/// rectangle with a corner radius and a rotation, a star, a polygon, an
/// ellipse and a curved path with kinds and a rotation.
fn every_kind(document: &Document) -> Vec<NodeId> {
    let rect = document.create_rect(RectBounds {
        origin: pt(5.0, 6.0),
        width: Length::from_mm(30.0),
        height: Length::from_mm(12.0),
    });
    document
        .resize_rect(
            rect,
            RectBounds {
                origin: pt(5.0, 6.0),
                width: Length::from_mm(30.0),
                height: Length::from_mm(12.0),
            },
            Length::from_mm(2.5),
            Some(Length::from_mm(0.75)),
        )
        .unwrap();
    let rotated = document
        .object(rect)
        .unwrap()
        .rotated(pt(20.0, 12.0), Angle::from_radians(0.6));
    document.rotate_object(&rotated).unwrap();

    let ellipse = document.create_ellipse(EllipseFrame {
        center: pt(50.0, 40.0),
        rx: Length::from_mm(8.0),
        ry: Length::from_mm(5.0),
    });
    let polygon = document.create_polygon(
        StarFrame {
            center: pt(70.0, 40.0),
            radius: Length::from_mm(9.0),
            angle: Angle::from_radians(0.3),
        },
        PointCount::new(7).unwrap(),
    );
    let star = document.create_star(
        StarFrame {
            center: pt(90.0, 40.0),
            radius: Length::from_mm(9.0),
            angle: Angle::from_radians(-0.2),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let path = curved_path(document, false);
    let rotated = document
        .object(path)
        .unwrap()
        .rotated(pt(10.0, 0.0), Angle::from_radians(0.25));
    document.rotate_object(&rotated).unwrap();
    vec![rect, ellipse, polygon, star, path]
}

fn sources(document: &Document, ids: &[NodeId], start: u64) -> Vec<CopySource> {
    let mut next = start;
    ids.iter()
        .map(|&id| {
            let count = match document.object(id).unwrap() {
                ObjectSnapshot::Path(path) => path.anchors.len(),
                ObjectSnapshot::Primitive(_) => 0,
            };
            let anchor_ids = fresh(next, count);
            next += count as u64;
            CopySource { id, anchor_ids }
        })
        .collect()
}

/// `object` with another id (and, for a path, the given anchor ids): what a
/// copy of it must read as when no offset was applied.
fn reidentified(object: &ObjectSnapshot, id: NodeId, anchor_ids: &[AnchorId]) -> ObjectSnapshot {
    match object {
        ObjectSnapshot::Path(path) => {
            let mut path = path.clone();
            path.id = id;
            for (anchor, fresh) in path.anchors.iter_mut().zip(anchor_ids) {
                anchor.id = *fresh;
            }
            ObjectSnapshot::Path(path)
        }
        ObjectSnapshot::Primitive(primitive) => {
            let mut primitive = *primitive;
            primitive.id = id;
            ObjectSnapshot::Primitive(primitive)
        }
    }
}

#[test]
fn a_copy_holds_every_value_of_its_original_displaced_by_the_offset() {
    let document = Document::new(1);
    let ids = every_kind(&document);
    let originals: Vec<ObjectSnapshot> =
        ids.iter().map(|&id| document.object(id).unwrap()).collect();
    let copy_sources = sources(&document, &ids, 1);

    let copies = document.duplicate_objects(&copy_sources, OFFSET).unwrap();

    assert_eq!(copies.len(), ids.len());
    for ((original, copy_id), source) in originals.iter().zip(&copies).zip(&copy_sources) {
        assert!(!ids.contains(copy_id), "a new NodeId");
        let copy = document.object(*copy_id).unwrap();
        // Preview equals commit: the blue outline is `translated(offset)`.
        let expected = reidentified(&original.translated(OFFSET), *copy_id, &source.anchor_ids);
        assert_eq!(copy, expected);
        // The originals are untouched, every stored value.
        assert_eq!(&document.object(original.id()).unwrap(), original);
    }
}

#[test]
fn a_closed_path_keeps_its_closed_state_in_the_copy() {
    let document = Document::new(1);
    let path = curved_path(&document, true);
    let copy = document
        .duplicate_objects(&sources(&document, &[path], 1), OFFSET)
        .unwrap()[0];
    let ObjectSnapshot::Path(copy) = document.object(copy).unwrap() else {
        panic!("a path")
    };
    assert!(copy.closed);
}

#[test]
fn the_copy_sits_directly_above_its_original_and_a_selection_keeps_its_order() {
    let document = Document::new(1);
    let a = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(4.0),
        height: Length::from_mm(4.0),
    });
    let b = document.create_rect(RectBounds {
        origin: pt(10.0, 0.0),
        width: Length::from_mm(4.0),
        height: Length::from_mm(4.0),
    });
    let c = document.create_rect(RectBounds {
        origin: pt(20.0, 0.0),
        width: Length::from_mm(4.0),
        height: Length::from_mm(4.0),
    });
    // Given in the order B, A (not the z-order).
    let copies = document
        .duplicate_objects(&sources(&document, &[b, a], 1), OFFSET)
        .unwrap();
    assert_eq!(copies.len(), 2);
    let (b_copy, a_copy) = (copies[0], copies[1]);
    assert_eq!(document.object_ids(), vec![a, a_copy, b, b_copy, c]);
}

#[test]
fn fresh_anchor_ids_are_unique_and_the_copy_can_be_joined_to_its_original() {
    let document = Document::new(1);
    let path = curved_path(&document, false);
    let copy_sources = sources(&document, &[path], 1);
    let copy = document.duplicate_objects(&copy_sources, OFFSET).unwrap()[0];

    let ObjectSnapshot::Path(original) = document.object(path).unwrap() else {
        panic!("a path")
    };
    let ObjectSnapshot::Path(copied) = document.object(copy).unwrap() else {
        panic!("a path")
    };
    let all: Vec<AnchorId> = original
        .anchors
        .iter()
        .chain(&copied.anchors)
        .map(|a| a.id)
        .collect();
    let mut unique = all.clone();
    unique.sort_by_key(|id| id.as_u128());
    unique.dedup();
    assert_eq!(unique.len(), all.len(), "no id appears twice");

    let joined = document
        .join_endpoints(
            path,
            original.anchors.last().unwrap().id,
            copy,
            copied.anchors.first().unwrap().id,
        )
        .expect("a join of a path's end to its copy's end succeeds");
    let ObjectSnapshot::Path(joined) = document.object(joined.0).unwrap() else {
        panic!("a path")
    };
    // The two joined ends become one node (3 + 3 - 1).
    assert_eq!(joined.anchors.len(), 5);
}

#[test]
fn a_wrong_anchor_id_count_refuses_and_writes_nothing() {
    let document = Document::new(1);
    let path = curved_path(&document, false);
    let rect = document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(4.0),
        height: Length::from_mm(4.0),
    });
    let before = document.export_json().unwrap();
    let changes = document.export_loro_snapshot().unwrap();

    let too_few = CopySource {
        id: path,
        anchor_ids: fresh(1, 2),
    };
    let ok_rect = CopySource {
        id: rect,
        anchor_ids: vec![],
    };
    assert_eq!(
        document.duplicate_objects(&[ok_rect.clone(), too_few], OFFSET),
        Err(ObjectEditError::AnchorIds)
    );
    let doubled = CopySource {
        id: path,
        anchor_ids: vec![AnchorId::new(77, 1); 3],
    };
    assert_eq!(
        document.duplicate_objects(&[doubled], OFFSET),
        Err(ObjectEditError::AnchorIds)
    );
    let rect_with_ids = CopySource {
        id: rect,
        anchor_ids: fresh(1, 1),
    };
    assert_eq!(
        document.duplicate_objects(&[rect_with_ids], OFFSET),
        Err(ObjectEditError::AnchorIds)
    );
    assert_eq!(document.export_json().unwrap(), before);
    assert_eq!(document.export_loro_snapshot().unwrap(), changes);
}

#[test]
fn a_stale_source_refuses_the_whole_call_and_writes_nothing() {
    let document = Document::new(1);
    let ids = every_kind(&document);
    // Sources are read while every object still exists, then one goes stale.
    let copy_sources = sources(&document, &ids, 1);
    document.delete_objects(&[ids[0]]).unwrap();
    let before = document.export_loro_snapshot().unwrap();
    let objects_before = document.object_ids();

    assert_eq!(
        document.duplicate_objects(&copy_sources, OFFSET),
        Err(ObjectEditError::NoSuchObject)
    );
    assert_eq!(document.object_ids(), objects_before);
    assert_eq!(document.export_loro_snapshot().unwrap(), before);
}

#[test]
fn one_commit_and_a_saved_project_holds_the_copies() {
    let document = Document::new(1);
    let ids = every_kind(&document);
    let copy_sources = sources(&document, &ids, 1);
    let objects_before = document.object_ids().len();

    let copies = document.duplicate_objects(&copy_sources, OFFSET).unwrap();
    assert_eq!(document.object_ids().len(), objects_before + copies.len());

    let reopened = unpack(5, &pack(&document, "0.1.0").unwrap()).unwrap();
    for copy in &copies {
        assert_eq!(reopened.object(*copy), document.object(*copy));
    }
    assert_eq!(reopened.object_ids(), document.object_ids());
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
fn a_copy_and_a_concurrent_move_of_the_original_both_survive_the_merge() {
    let base = Document::new(1);
    let rect = base.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    });
    let bytes = pack(&base, "0.1.0").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();

    let copy = a
        .duplicate_objects(
            &[CopySource {
                id: rect,
                anchor_ids: vec![],
            }],
            Vec2::new(20.0, 0.0),
        )
        .unwrap()[0];
    b.translate_objects(&[rect], Vec2::new(0.0, 7.0)).unwrap();

    for merged in [merged(&a, &b), merged(&b, &a)] {
        let ObjectSnapshot::Primitive(original) = merged.object(rect).unwrap() else {
            panic!("a primitive")
        };
        let ObjectSnapshot::Primitive(copied) = merged.object(copy).unwrap() else {
            panic!("a primitive")
        };
        let (
            vecmanf_document_core::Shape::Rect { bounds: o, .. },
            vecmanf_document_core::Shape::Rect { bounds: c, .. },
        ) = (original.shape, copied.shape)
        else {
            panic!("rectangles")
        };
        assert_eq!(o.origin, pt(0.0, 7.0), "the original moved");
        assert_eq!(
            c.origin,
            pt(20.0, 0.0),
            "the copy is at A's offset from the old position"
        );
        let ids = merged.object_ids();
        let original_at = ids.iter().position(|id| *id == rect).unwrap();
        assert_eq!(
            ids[original_at + 1],
            copy,
            "the copy stays above its original"
        );
    }
}

#[test]
fn a_copy_of_an_object_a_peer_deleted_concurrently_still_converges() {
    let base = Document::new(1);
    let rect = base.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(4.0),
    });
    let bytes = pack(&base, "0.1.0").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    // A has already resolved its stale read of `rect`; B deletes it meanwhile.
    let copy = a
        .duplicate_objects(
            &[CopySource {
                id: rect,
                anchor_ids: vec![],
            }],
            Vec2::new(1.0, 1.0),
        )
        .unwrap()[0];
    b.delete_objects(&[rect]).unwrap();
    let merged = merged(&a, &b);
    assert!(merged.object(rect).is_none());
    assert!(
        merged.object(copy).is_some(),
        "the copy has no link to the original"
    );
}
