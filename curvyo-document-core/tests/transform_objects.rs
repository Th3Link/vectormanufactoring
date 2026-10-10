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

// ---------------------------------------------------------------------
// A path result for a primitive is a conversion (`0019` criteria 53, 50)
// ---------------------------------------------------------------------

use curvyo_document_core::{
    AnchorKind, AnchorSnapshot, InnerRatio, PathSnapshot, PointCount, StarFrame,
};

fn star(document: &Document, x: f64) -> NodeId {
    document.create_star(
        StarFrame {
            center: pt(x, 5.0),
            radius: Length::from_mm(5.0),
            angle: Angle::from_radians(0.5),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    )
}

/// The path a star would become, with ten anchors of the ids `first..first + 10`.
fn path_for(object: &ObjectSnapshot, first: u64, count: u64, rotation: Angle) -> ObjectSnapshot {
    let anchors = (0..count)
        .map(|n| AnchorSnapshot {
            id: AnchorId::new(7, first + n),
            point: pt(f64::from(u32::try_from(n).unwrap()), 1.0),
            handle_in: Vec2::new(0.0, 0.0),
            handle_out: Vec2::new(0.0, 0.0),
            kind: AnchorKind::Corner,
        })
        .collect();
    ObjectSnapshot::Path(PathSnapshot {
        id: object.id(),
        closed: true,
        style: object.style().clone(),
        anchors,
        extra_subpaths: Vec::new(),
        rotation,
    })
}

/// Criterion 53.2: the converted object keeps its id, its place in the list and its
/// style; it is a path afterwards, with the given anchors and rotation, in one commit
/// together with a plain move of another object.
#[test]
fn a_path_result_for_a_primitive_converts_it_in_place() {
    let document = Document::new(1);
    let (a, s, b) = (
        rect(&document, 0.0),
        star(&document, 20.0),
        rect(&document, 50.0),
    );
    let star_before = document.object(s).unwrap();
    let results = vec![
        moved(&document.object(a).unwrap(), 5.0),
        path_for(&star_before, 100, 10, Angle::from_radians(0.5)),
        moved(&document.object(b).unwrap(), 5.0),
    ];
    let commits = changes(&document);
    document.transform_objects(&results, false).unwrap();
    assert_eq!(changes(&document), commits + 1, "one commit");
    assert_eq!(document.object_ids(), vec![a, s, b], "same order");
    let ObjectSnapshot::Path(converted) = document.object(s).unwrap() else {
        panic!("a path now");
    };
    assert_eq!(converted.anchors.len(), 10);
    assert!(converted.closed);
    assert_eq!(&converted.style, star_before.style());
    assert!((converted.rotation.as_radians() - 0.5).abs() < 1e-12);
    assert!(document.primitive(s).is_none());
    // The neighbours moved as before.
    assert_eq!(document.object(a).unwrap(), results[0]);
    // And it survives a save and reopen.
    let reopened = unpack(9, &pack(&document, "0.1.0").unwrap()).unwrap();
    assert_eq!(reopened.object(s).unwrap(), document.object(s).unwrap());
}

/// Criterion 53.6: one stale object, or one result that is not an outline, refuses the
/// whole call before the first write.
#[test]
fn a_bad_conversion_refuses_everything() {
    let document = Document::new(1);
    let (a, s) = (rect(&document, 0.0), star(&document, 20.0));
    let before_a = document.object(a).unwrap();
    let before_s = document.object(s).unwrap();
    let ok = moved(&before_a, 5.0);
    let commits = changes(&document);
    for (anchors, why) in [(1u64, "one anchor"), (0, "no anchor")] {
        let bad = path_for(&before_s, 100, anchors, Angle::from_radians(0.0));
        assert_eq!(
            document.transform_objects(&[ok.clone(), bad], false),
            Err(ObjectEditError::InvalidConversion),
            "{why}"
        );
    }
    // Two anchors with the same id.
    let ObjectSnapshot::Path(mut twice) = path_for(&before_s, 100, 4, Angle::from_radians(0.0))
    else {
        panic!()
    };
    twice.anchors[1].id = twice.anchors[0].id;
    assert_eq!(
        document.transform_objects(&[ok.clone(), ObjectSnapshot::Path(twice)], false),
        Err(ObjectEditError::InvalidConversion)
    );
    // A stale id.
    let good = path_for(&before_s, 100, 10, Angle::from_radians(0.0));
    document.delete_objects(&[a]).unwrap();
    let commits = commits + 1;
    assert_eq!(
        document.transform_objects(&[ok, good], false),
        Err(ObjectEditError::NoSuchObject)
    );
    assert_eq!(changes(&document), commits, "nothing written");
    assert_eq!(document.object(s).unwrap(), before_s, "still the star");
}

/// A path result for a path is no conversion, and a primitive result for a path
/// still refuses (the kind must not go the other way).
#[test]
fn only_a_primitive_becomes_a_path_never_the_reverse() {
    let document = Document::new(1);
    let (p, r) = (path(&document, 1), rect(&document, 0.0));
    let primitive_for_path = ObjectSnapshot::Primitive(PrimitiveSnapshot {
        id: p,
        ..match document.object(r).unwrap() {
            ObjectSnapshot::Primitive(x) => x,
            ObjectSnapshot::Path(_) => panic!(),
        }
    });
    assert_eq!(
        document.transform_objects(&[primitive_for_path], false),
        Err(ObjectEditError::NoSuchObject)
    );
}

/// The debt item closed with this feature: "Object to path" keeps the shown angle of a
/// polygon or star (the frame angle plus the register) as the path's `rotation`.
#[test]
fn object_to_path_keeps_the_shown_angle_of_a_star() {
    let document = Document::new(1);
    let s = star(&document, 20.0);
    let shown = document.object(s).unwrap().orientation();
    let anchors: Vec<(NodeId, Vec<NewAnchor>)> = vec![(
        s,
        (0..4)
            .map(|n| {
                NewAnchor::corner(
                    AnchorId::new(7, n),
                    pt(f64::from(u32::try_from(n).unwrap()), 0.0),
                )
            })
            .collect(),
    )];
    document.convert_to_paths(&anchors).unwrap();
    let ObjectSnapshot::Path(path) = document.object(s).unwrap() else {
        panic!("a path");
    };
    assert!((path.rotation.as_radians() - shown.as_radians()).abs() < 1e-12);
    assert!((shown.as_radians() - 0.5).abs() < 1e-12);
}
