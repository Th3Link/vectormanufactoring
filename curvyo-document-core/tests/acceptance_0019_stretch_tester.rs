//! Independent tester tests for the conversion path of `Document::transform_objects`
//! (`specs/0019-multi-object-transform/specification.md` criteria 22, 31, 50, 53):
//! a path result for a primitive converts it under the same id and tree position
//! without writing a style field, in one commit, all or nothing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, missing_docs, clippy::doc_markdown)]
#![allow(clippy::many_single_char_names, clippy::cast_precision_loss, clippy::type_complexity)]
#![allow(clippy::manual_assert_eq)]

use curvyo_document_core::{
    AnchorId, Angle, Document, InnerRatio, Length, NewAnchor, NodeId, ObjectEditError,
    ObjectSnapshot, PathSnapshot, Point, PointCount, RectBounds, StarFrame, StyleEdit, pack,
    unpack,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn loro_of(d: &Document) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn changes(d: &Document) -> usize {
    loro_of(d).len_changes()
}

fn bytes(d: &Document) -> Vec<u8> {
    d.export_loro_snapshot().unwrap()
}

fn star(d: &Document, cx: f64) -> NodeId {
    d.create_star(
        StarFrame {
            center: pt(cx, 20.0),
            radius: Length::from_mm(8.0),
            angle: Angle::from_radians(-1.0),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    )
}

fn rect(d: &Document, x: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

/// A closed `n`-gon path result for `id`, anchors with ids (peer 77, counter base..).
fn conversion(d: &Document, id: NodeId, n: u64, base: u64) -> PathSnapshot {
    let donor_anchors: Vec<NewAnchor> = (0..n)
        .map(|i| {
            NewAnchor::corner(
                AnchorId::new(78, base + i),
                pt(i as f64 * 3.0, (i * i) as f64),
            )
        })
        .collect();
    let donor = d.create_path(&donor_anchors, true);
    let ObjectSnapshot::Path(mut p) = d.object(donor).unwrap() else {
        panic!()
    };
    d.delete_objects(&[donor]).unwrap();
    p.id = id;
    for (i, a) in p.anchors.iter_mut().enumerate() {
        a.id = AnchorId::new(77, base + i as u64);
    }
    p
}

fn kind(d: &Document, id: NodeId) -> &'static str {
    match d.object(id).unwrap() {
        ObjectSnapshot::Path(_) => "path",
        ObjectSnapshot::Primitive(_) => "primitive",
    }
}

/// A conversion keeps id and tree position and writes no style field of the donor;
/// it is one commit labelled as a transform; the file round-trips.
#[test]
fn a_conversion_keeps_id_position_and_style_in_one_commit() {
    let d = Document::new(1);
    let a = rect(&d, 0.0);
    let s = star(&d, 30.0);
    let c = rect(&d, 60.0);
    d.edit_style(&[s], &StyleEdit::StrokeWidth(Length::from_mm(2.5)))
        .unwrap();
    d.edit_style(&[s], &StyleEdit::FillEnabled(true)).unwrap();
    let style_before = match d.object(s).unwrap() {
        ObjectSnapshot::Primitive(p) => format!("{:?}", p.style),
        ObjectSnapshot::Path(_) => panic!(),
    };
    let order = d.object_ids();
    // The donor path is a default-styled path: its style must NOT be written.
    let p = conversion(&d, s, 4, 1);
    let before = changes(&d);
    d.transform_objects(&[ObjectSnapshot::Path(p.clone())], false)
        .unwrap();
    assert_eq!(changes(&d), before + 1);
    assert_eq!(d.object_ids(), order, "same tree order");
    assert_eq!(kind(&d, s), "path");
    assert_eq!(kind(&d, a), "primitive");
    assert_eq!(kind(&d, c), "primitive");
    let ObjectSnapshot::Path(now) = d.object(s).unwrap() else {
        panic!()
    };
    assert_eq!(
        format!("{:?}", now.style),
        style_before,
        "no style field written"
    );
    assert_eq!(now.anchors.len(), 4);
    assert!(now.closed);
    // Reopens.
    let again = unpack(3, &pack(&d, "0.1.0").unwrap()).unwrap();
    assert_eq!(again.object_ids(), order);
    assert_eq!(kind(&again, s), "path");
}

/// All or nothing: an invalid conversion anywhere in the list leaves the whole
/// document byte-identical (also the valid conversion before it and the move after it).
#[test]
fn an_invalid_conversion_refuses_everything() {
    let mut cases: Vec<(&str, Box<dyn Fn(&mut PathSnapshot)>)> = Vec::new();
    cases.push(("one anchor", Box::new(|p| p.anchors.truncate(1))));
    cases.push(("open", Box::new(|p| p.closed = false)));
    cases.push((
        "NaN anchor",
        Box::new(|p| p.anchors[0].point = pt(f64::NAN, 0.0)),
    ));
    cases.push((
        "infinite handle",
        Box::new(|p| p.anchors[1].handle_out = curvyo_document_core::Vec2::new(f64::INFINITY, 0.0)),
    ));
    cases.push((
        "NaN rotation",
        Box::new(|p| p.rotation = Angle::from_radians(f64::NAN)),
    ));
    cases.push((
        "repeated anchor id",
        Box::new(|p| p.anchors[1].id = p.anchors[0].id),
    ));
    for (name, spoil) in cases {
        let d = Document::new(1);
        let r1 = rect(&d, 0.0);
        let s1 = star(&d, 30.0);
        let s2 = star(&d, 60.0);
        let good = conversion(&d, s1, 4, 1);
        let mut bad = conversion(&d, s2, 4, 10);
        spoil(&mut bad);
        let moved = d
            .object(r1)
            .unwrap()
            .translated(curvyo_document_core::Vec2::new(5.0, 0.0));
        let snapshot = bytes(&d);
        let n = changes(&d);
        let err = d
            .transform_objects(
                &[ObjectSnapshot::Path(good), ObjectSnapshot::Path(bad), moved],
                false,
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                ObjectEditError::InvalidConversion | ObjectEditError::NonFiniteGeometry
            ),
            "{name}: {err:?}"
        );
        assert!(bytes(&d) == snapshot, "{name}: not a byte written");
        assert_eq!(changes(&d), n, "{name}: no commit");
        assert_eq!(kind(&d, s1), "primitive");
    }
}

/// A stale id with a valid conversion in the list: refused as a whole.
#[test]
fn a_stale_id_next_to_a_conversion_refuses_all() {
    let d = Document::new(1);
    let s1 = star(&d, 30.0);
    let gone = rect(&d, 0.0);
    let good = conversion(&d, s1, 4, 1);
    let stale = d.object(gone).unwrap();
    d.delete_objects(&[gone]).unwrap();
    let snapshot = bytes(&d);
    let err = d
        .transform_objects(&[ObjectSnapshot::Path(good), stale], false)
        .unwrap_err();
    assert_eq!(err, ObjectEditError::NoSuchObject);
    assert!(bytes(&d) == snapshot);
    assert_eq!(kind(&d, s1), "primitive");
}

/// Criterion 53.1 / ADR: the anchor ids of a path are unique in the document. A
/// conversion that reuses the anchor id of another path would break that; the
/// writer should refuse it (or the caller never produces it). Reports the
/// behaviour; fails when the duplicate is written.
#[test]
fn a_conversion_with_an_anchor_id_of_another_path_is_refused() {
    let d = Document::new(1);
    let s1 = star(&d, 30.0);
    let other = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(5, 1), pt(0.0, 50.0)),
            NewAnchor::corner(AnchorId::new(5, 2), pt(10.0, 50.0)),
        ],
        false,
    );
    let mut p = conversion(&d, s1, 3, 1);
    p.anchors[0].id = AnchorId::new(5, 1);
    let snapshot = bytes(&d);
    let result = d.transform_objects(&[ObjectSnapshot::Path(p)], false);
    println!("conversion with a colliding anchor id: {result:?}");
    let _ = other;
    assert!(
        result.is_err() && bytes(&d) == snapshot,
        "a duplicate document-wide anchor id was written: {result:?}"
    );
}

/// The stroke-width switch applies to a converted object like to any path; a
/// non-positive width refuses everything.
#[test]
fn the_stroke_width_of_a_conversion_is_written_only_when_asked() {
    for write in [false, true] {
        let d = Document::new(1);
        let s1 = star(&d, 30.0);
        d.edit_style(&[s1], &StyleEdit::StrokeWidth(Length::from_mm(1.0)))
            .unwrap();
        let mut p = conversion(&d, s1, 4, 1);
        p.style.stroke.width = Length::from_mm(0.4);
        d.transform_objects(&[ObjectSnapshot::Path(p)], write)
            .unwrap();
        let ObjectSnapshot::Path(now) = d.object(s1).unwrap() else {
            panic!()
        };
        assert_eq!(
            now.style.stroke.width.as_mm(),
            if write { 0.4 } else { 1.0 }
        );
    }
    let d = Document::new(1);
    let s1 = star(&d, 30.0);
    let mut p = conversion(&d, s1, 4, 1);
    p.style.stroke.width = Length::from_mm(0.0);
    let snapshot = bytes(&d);
    assert!(
        d.transform_objects(&[ObjectSnapshot::Path(p)], true)
            .is_err()
    );
    assert!(bytes(&d) == snapshot);
}

/// The same object twice in one list (a caller bug) must not corrupt the document.
#[test]
fn the_same_object_twice_leaves_a_valid_document() {
    let d = Document::new(1);
    let s1 = star(&d, 30.0);
    let a = conversion(&d, s1, 4, 1);
    let b = conversion(&d, s1, 5, 10);
    let r = d.transform_objects(&[ObjectSnapshot::Path(a), ObjectSnapshot::Path(b)], false);
    println!("the same id twice: {r:?}");
    let again = unpack(3, &pack(&d, "0.1.0").unwrap()).expect("still a valid file");
    assert_eq!(again.object_ids().len(), 1);
    let ObjectSnapshot::Path(p) = again.object(s1).unwrap() else {
        panic!("a path")
    };
    let mut ids: Vec<_> = p.anchors.iter().map(|a| format!("{:?}", a.id)).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), p.anchors.len(), "no anchor appears twice");
}
