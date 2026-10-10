//! `Document::transform_objects` and the document-axes snapshot maps
//! (`specs/0019-multi-object-transform/` criteria 20, 31, 41, 50): one commit
//! for N objects, atomic, writing only what differs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, Length, NewAnchor, NodeId, ObjectEditError,
    ObjectSnapshot, Point, PrimitiveSnapshot, RectBounds, Vec2, pack, unpack,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(document: &Document, x: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn path(document: &Document, first: u64) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, first), pt(0.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, first + 1), pt(10.0, 30.0)),
        ],
        false,
    )
}

fn changes(document: &Document) -> usize {
    let doc = loro::LoroDoc::new();
    doc.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    doc.len_changes()
}

fn moved(object: &ObjectSnapshot, by: f64) -> ObjectSnapshot {
    object.translated(Vec2::new(by, 0.0))
}

/// Criterion 31: one commit writes the results of all objects, whatever their
/// kinds.
#[test]
fn one_commit_writes_every_result() {
    let document = Document::new(1);
    let (a, b, c) = (
        rect(&document, 0.0),
        path(&document, 1),
        rect(&document, 50.0),
    );
    let results: Vec<_> = [a, b, c]
        .iter()
        .map(|&id| moved(&document.object(id).unwrap(), 5.0))
        .collect();
    let before = changes(&document);
    document.transform_objects(&results, false).unwrap();
    assert_eq!(changes(&document), before + 1);
    for (id, result) in [a, b, c].iter().zip(&results) {
        assert_eq!(&document.object(*id).unwrap(), result);
    }
}

/// Criterion 31: a stale id refuses the whole call and writes nothing.
#[test]
fn a_stale_object_refuses_everything() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    let results: Vec<_> = [a, b]
        .iter()
        .map(|&id| moved(&document.object(id).unwrap(), 5.0))
        .collect();
    let original_a = document.object(a).unwrap();
    document.delete_objects(&[b]).unwrap();
    let before = changes(&document);
    assert_eq!(
        document.transform_objects(&results, false),
        Err(ObjectEditError::NoSuchObject)
    );
    assert_eq!(changes(&document), before, "nothing committed");
    assert_eq!(document.object(a).unwrap(), original_a, "no prefix written");
}

/// A result whose kind differs from the stored object's refuses the call.
#[test]
fn a_changed_kind_refuses_everything() {
    let document = Document::new(1);
    let a = rect(&document, 0.0);
    let ellipse = document.create_ellipse(EllipseFrame {
        center: pt(0.0, 0.0),
        rx: Length::from_mm(3.0),
        ry: Length::from_mm(3.0),
    });
    let mut wrong = document.object(ellipse).unwrap();
    // Re-tag the ellipse's id onto a rectangle result.
    if let ObjectSnapshot::Primitive(p) = &mut wrong {
        *p = PrimitiveSnapshot { id: a, ..p.clone() };
    }
    let before = document.object(a).unwrap();
    assert_eq!(
        document.transform_objects(&[wrong], false),
        Err(ObjectEditError::NoSuchObject)
    );
    assert_eq!(document.object(a).unwrap(), before);
}

/// Criterion 31: the stroke width is written only on request, and a width that
/// is not above zero refuses the call before any write.
#[test]
fn the_stroke_width_is_written_only_on_request() {
    let document = Document::new(1);
    let a = rect(&document, 0.0);
    let mut result = document.object(a).unwrap();
    result.style_mut().stroke.width = Length::from_mm(3.0);
    document
        .transform_objects(&[result.clone()], false)
        .unwrap();
    assert_ne!(
        document.object(a).unwrap().style().stroke.width.as_mm(),
        3.0
    );
    document.transform_objects(&[result.clone()], true).unwrap();
    assert_eq!(
        document.object(a).unwrap().style().stroke.width.as_mm(),
        3.0
    );
    result.style_mut().stroke.width = Length::from_mm(0.0);
    let before = changes(&document);
    assert_eq!(
        document.transform_objects(&[result], true),
        Err(ObjectEditError::InvalidStrokeWidth)
    );
    assert_eq!(changes(&document), before);
}

/// Criterion 20: a result equal to the stored objects commits nothing new and
/// changes nothing.
#[test]
fn an_unchanged_result_leaves_the_document_alone() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), path(&document, 1));
    let results: Vec<_> = [a, b]
        .iter()
        .map(|&id| document.object(id).unwrap())
        .collect();
    let packed = pack(&document, "0.1.0").unwrap();
    document.transform_objects(&results, false).unwrap();
    assert_eq!(
        unpack(5, &pack(&document, "0.1.0").unwrap())
            .unwrap()
            .object(a),
        unpack(5, &packed).unwrap().object(a)
    );
}

/// Criterion 50: a transform writes only fields the single-object gestures
/// write: the saved and reopened objects equal the results.
#[test]
fn the_reopened_file_holds_the_results() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), path(&document, 1));
    let turned: Vec<_> = [a, b]
        .iter()
        .map(|&id| {
            document
                .object(id)
                .unwrap()
                .rotated(pt(5.0, 5.0), Angle::from_radians(0.6))
        })
        .collect();
    document.transform_objects(&turned, false).unwrap();
    let reopened = unpack(9, &pack(&document, "0.1.0").unwrap()).unwrap();
    for (id, result) in [a, b].iter().zip(&turned) {
        assert_eq!(&reopened.object(*id).unwrap(), result);
    }
}

/// `scaled_along` with the zero frame is a plain scale in the document axes; the
/// path's own `rotation` has no say, and is kept.
#[test]
fn scaled_along_ignores_the_paths_own_rotation() {
    let document = Document::new(1);
    let id = path(&document, 1);
    let turned = document
        .object(id)
        .unwrap()
        .rotated(pt(0.0, 0.0), Angle::from_radians(0.5));
    let ObjectSnapshot::Path(turned) = turned else {
        panic!("a path");
    };
    let scaled = turned.scaled_along(pt(0.0, 0.0), 2.0, 3.0, Angle::from_radians(0.0));
    for (a, b) in turned.anchors.iter().zip(&scaled.anchors) {
        assert!((b.point.x - 2.0 * a.point.x).abs() < 1e-9);
        assert!((b.point.y - 3.0 * a.point.y).abs() < 1e-9);
    }
    assert_eq!(scaled.rotation, turned.rotation);
    assert_eq!(turned.scaled(pt(0.0, 0.0), 1.0, 1.0), turned);
}

/// `sheared_along` with the zero frame: an x shear moves a point by `ku` times
/// its y offset from the pivot; `sheared` is the same with the path's rotation.
#[test]
fn sheared_along_shears_in_the_given_axes() {
    let document = Document::new(1);
    let id = path(&document, 1);
    let ObjectSnapshot::Path(path) = document.object(id).unwrap() else {
        panic!("a path");
    };
    let sheared = path.sheared_along(pt(0.0, 20.0), 1.0, 0.0, Angle::from_radians(0.0));
    assert!((sheared.anchors[1].point.x - 20.0).abs() < 1e-9);
    assert!((sheared.anchors[1].point.y - 30.0).abs() < 1e-9);
    assert_eq!(
        path.sheared(pt(0.0, 20.0), 1.0, 0.0),
        sheared,
        "rotation is 0"
    );
}
