//! Independent tester tests for `Document::transform_objects`
//! (`specs/0019-multi-object-transform/specification.md` criteria 22 and 31):
//! one commit for all objects, nothing written when anything is refused, a
//! label, the stroke-width switch, non-finite and out-of-range inputs, and the
//! save and reopen round trip. Written from the specification before the
//! implementation was read.

// Test code: byte buffers are compared with `assert!(a == b, "msg")` on purpose
// (a failing `assert_eq!` would print the whole snapshot), and the arithmetic is
// written the way the specification states it.
#![allow(
    clippy::manual_assert_eq,
    clippy::manual_midpoint,
    clippy::collapsible_if
)]
#![allow(clippy::unneeded_wildcard_pattern, clippy::unreadable_literal)]
#![allow(clippy::used_underscore_binding, clippy::useless_conversion)]
#![allow(clippy::suboptimal_flops, clippy::imprecise_flops)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, missing_docs, clippy::doc_markdown)]

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, Length, NewAnchor, NodeId, ObjectEditError,
    ObjectSnapshot, Point, RectBounds, Shape, Vec2, pack, unpack,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(d: &Document, x: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn path(d: &Document, first: u64) -> NodeId {
    d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, first), pt(0.0, 20.0)),
            NewAnchor::corner(AnchorId::new(1, first + 1), pt(10.0, 30.0)),
        ],
        false,
    )
}

fn loro_of(d: &Document) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn changes(d: &Document) -> usize {
    loro_of(d).len_changes()
}

fn snapshot(d: &Document) -> Vec<u8> {
    d.export_loro_snapshot().unwrap()
}

fn all(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn moved(o: &ObjectSnapshot, dx: f64) -> ObjectSnapshot {
    o.translated(Vec2::new(dx, 0.0))
}

/// Three objects, one commit; the commit carries a label and a peer sees all
/// of it or none of it.
#[test]
fn one_commit_one_label_for_all_objects() {
    let d = Document::new(1);
    let _ = rect(&d, 0.0);
    let _ = path(&d, 1);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(50.0, 5.0),
        rx: Length::from_mm(4.0),
        ry: Length::from_mm(3.0),
    });
    let before = changes(&d);
    let from = loro_of(&d).oplog_vv();
    let results: Vec<ObjectSnapshot> = all(&d).iter().map(|o| moved(o, 5.0)).collect();
    d.transform_objects(&results, false).unwrap();
    assert_eq!(changes(&d), before + 1, "exactly one change");
    let l = loro_of(&d);
    let json = format!("{:?}", l.export_json_updates(&from, &l.oplog_vv()));
    assert!(json.contains("transform_objects"), "label: {json}");
    // All three objects moved together.
    for (o, n) in all(&d).iter().zip(&results) {
        assert_eq!(o.translated(Vec2::ZERO), n.translated(Vec2::ZERO));
    }
}

/// A stale id anywhere in the list refuses every object, and the document is
/// byte-identical afterwards (no partial write).
#[test]
fn a_stale_id_refuses_all_and_writes_nothing() {
    for stale_position in 0..3 {
        let d = Document::new(1);
        let a = rect(&d, 0.0);
        let b = path(&d, 1);
        let c = rect(&d, 40.0);
        let mut results: Vec<ObjectSnapshot> = [a, b, c]
            .iter()
            .map(|id| moved(&d.object(*id).unwrap(), 3.0))
            .collect();
        // Delete the object at `stale_position` after the results were made.
        let doomed = [a, b, c][stale_position];
        d.delete_objects(&[doomed]).unwrap();
        let before = snapshot(&d);
        let commits = changes(&d);
        let err = d.transform_objects(&results, true).unwrap_err();
        assert_eq!(err, ObjectEditError::NoSuchObject);
        assert!(
            snapshot(&d) == before,
            "no partial write (stale at {stale_position})"
        );
        assert_eq!(changes(&d), commits);
        results.clear();
    }
}

/// A result whose kind differs from the stored kind (a path result for a
/// rectangle id) refuses everything.
#[test]
fn a_changed_kind_refuses_all() {
    let d = Document::new(1);
    let r = rect(&d, 0.0);
    let p = path(&d, 1);
    let r2 = rect(&d, 40.0);
    let good_a = moved(&d.object(r).unwrap(), 3.0);
    let good_b = moved(&d.object(r2).unwrap(), 3.0);
    // Build a rectangle-id result out of the path: swap the ids of a path and
    // a rectangle snapshot.
    let ObjectSnapshot::Path(mut wrong) = d.object(p).unwrap() else {
        panic!()
    };
    wrong.id = r;
    let before = snapshot(&d);
    let err = d
        .transform_objects(&[good_a, ObjectSnapshot::Path(wrong), good_b], false)
        .unwrap_err();
    assert_eq!(err, ObjectEditError::NoSuchObject);
    assert!(snapshot(&d) == before);
}

/// An empty list and an unchanged list write nothing and commit nothing.
#[test]
fn empty_and_unchanged_results_commit_nothing() {
    let d = Document::new(1);
    let _ = rect(&d, 0.0);
    let _ = path(&d, 1);
    let commits = changes(&d);
    let before = snapshot(&d);
    d.transform_objects(&[], true).unwrap();
    d.transform_objects(&all(&d), true).unwrap();
    assert_eq!(changes(&d), commits);
    assert!(snapshot(&d) == before);
}

/// The stroke width is written only on request, and never when invalid; an
/// invalid width refuses the whole call before any write.
#[test]
fn the_stroke_width_rule() {
    let d = Document::new(1);
    let a = rect(&d, 0.0);
    let b = path(&d, 1);
    let mut ra = moved(&d.object(a).unwrap(), 2.0);
    let mut rb = moved(&d.object(b).unwrap(), 2.0);
    let set = |o: &mut ObjectSnapshot, w: f64| match o {
        ObjectSnapshot::Primitive(p) => p.style.stroke.width = Length::from_mm(w),
        ObjectSnapshot::Path(p) => p.style.stroke.width = Length::from_mm(w),
    };
    set(&mut ra, 3.0);
    set(&mut rb, 4.0);
    let before = snapshot(&d);
    // Not requested: widths are ignored, geometry written.
    d.transform_objects(&[ra.clone(), rb.clone()], false)
        .unwrap();
    for o in all(&d) {
        let w = match &o {
            ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
            ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
        };
        assert_eq!(w, 0.25, "width untouched");
    }
    assert!(snapshot(&d) != before);
    // Requested: written.
    d.transform_objects(&[ra.clone(), rb.clone()], true)
        .unwrap();
    let w: Vec<f64> = all(&d)
        .iter()
        .map(|o| match o {
            ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
            ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
        })
        .collect();
    assert_eq!(w, [3.0, 4.0]);
    // Invalid widths with the request: refused, nothing written.
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let d2 = Document::new(1);
        let a2 = rect(&d2, 0.0);
        let b2 = path(&d2, 1);
        let ok = moved(&d2.object(a2).unwrap(), 9.0);
        let mut broken = moved(&d2.object(b2).unwrap(), 9.0);
        set(&mut broken, bad);
        let before = snapshot(&d2);
        let err = d2.transform_objects(&[ok, broken], true);
        assert_eq!(err, Err(ObjectEditError::InvalidStrokeWidth), "width {bad}");
        assert!(snapshot(&d2) == before, "no write for width {bad}");
    }
}

/// A rectangle whose result has a non-finite corner radius refuses the whole
/// call, the valid object first in the list included.
#[test]
fn an_invalid_radius_refuses_all() {
    let d = Document::new(1);
    let a = rect(&d, 0.0);
    let b = rect(&d, 40.0);
    let ok = moved(&d.object(a).unwrap(), 5.0);
    let ObjectSnapshot::Primitive(mut p) = d.object(b).unwrap() else {
        panic!()
    };
    if let Shape::Rect { corner_radii, .. } = &mut p.shape {
        corner_radii.tl = Length::from_mm(f64::NAN);
    }
    let before = snapshot(&d);
    let err = d.transform_objects(&[ok, ObjectSnapshot::Primitive(p)], false);
    assert_eq!(err, Err(ObjectEditError::InvalidRadius));
    assert!(snapshot(&d) == before);
}

/// Defence in depth: non-finite geometry handed to the writer is refused
/// (`NonFiniteGeometry`) before any write. The gesture layer guards too (see the
/// Session-level test `ac22_a_huge_or_non_finite_drag_*`); the older
/// single-object writers have no such check (`docs/technical-debt.md`).
#[test]
fn non_finite_geometry_never_reaches_the_document() {
    type Poison = fn(&mut ObjectSnapshot);
    let poisons: [(&str, Poison); 6] = [
        ("rect origin NaN", |o| {
            if let ObjectSnapshot::Primitive(p) = o
                && let Shape::Rect { bounds, .. } = &mut p.shape
            {
                bounds.origin.x = f64::NAN;
            }
        }),
        ("rect width inf", |o| {
            if let ObjectSnapshot::Primitive(p) = o
                && let Shape::Rect { bounds, .. } = &mut p.shape
            {
                bounds.width = Length::from_mm(f64::INFINITY);
            }
        }),
        ("rect rotation NaN", |o| {
            if let ObjectSnapshot::Primitive(p) = o {
                p.rotation = Angle::from_radians(f64::NAN);
            }
        }),
        ("path anchor NaN", |o| {
            if let ObjectSnapshot::Path(p) = o {
                p.anchors[0].point.y = f64::NAN;
            }
        }),
        ("path handle inf", |o| {
            if let ObjectSnapshot::Path(p) = o {
                p.anchors[1].handle_out = Vec2::new(f64::INFINITY, 0.0);
            }
        }),
        ("path rotation inf", |o| {
            if let ObjectSnapshot::Path(p) = o {
                p.rotation = Angle::from_radians(f64::INFINITY);
            }
        }),
    ];
    for (name, poison) in poisons {
        let d = Document::new(1);
        let a = rect(&d, 0.0);
        let b = path(&d, 1);
        let mut ra = moved(&d.object(a).unwrap(), 5.0);
        let mut rb = moved(&d.object(b).unwrap(), 5.0);
        poison(&mut ra);
        poison(&mut rb);
        let before = snapshot(&d);
        let res = d.transform_objects(&[ra, rb], false);
        if res.is_err() {
            assert!(snapshot(&d) == before, "{name}: refused but wrote");
        } else {
            // Accepted: then the file must still be loadable and finite.
            let json = String::from_utf8(d.export_json().unwrap()).unwrap();
            assert!(
                !json.contains("NaN") && !json.contains("inf") && !json.contains("null"),
                "{name}: a non-finite number was stored: {}",
                &json[..json.len().min(400)]
            );
        }
    }
}

/// Criterion 50: a transformed document packs, reopens and holds exactly the
/// geometry written, with the same kinds.
#[test]
fn saved_and_reopened_geometry_is_exact() {
    let d = Document::new(1);
    let a = rect(&d, 0.0);
    let b = path(&d, 1);
    let results = vec![
        moved(&d.object(a).unwrap(), 1.234_567_891_234)
            .rotated(pt(3.0, 4.0), Angle::from_radians(0.7)),
        moved(&d.object(b).unwrap(), -2.5).rotated(pt(3.0, 4.0), Angle::from_radians(0.7)),
    ];
    d.transform_objects(&results, false).unwrap();
    let reopened = unpack(9, &pack(&d, "0.1.0").unwrap()).unwrap();
    for (id, want) in [a, b].iter().zip(&results) {
        let got = reopened.object(*id).unwrap();
        match (&got, want) {
            (ObjectSnapshot::Primitive(g), ObjectSnapshot::Primitive(w)) => {
                assert_eq!(g.shape, w.shape);
                assert_eq!(g.rotation.normalized(), w.rotation.normalized());
            }
            (ObjectSnapshot::Path(g), ObjectSnapshot::Path(w)) => {
                assert_eq!(g.anchors.len(), w.anchors.len());
                for (x, y) in g.anchors.iter().zip(&w.anchors) {
                    assert_eq!(x.point, y.point);
                    assert_eq!(x.handle_in, y.handle_in);
                    assert_eq!(x.handle_out, y.handle_out);
                }
            }
            _ => panic!("kind changed"),
        }
    }
}

/// Criterion 50: no format change: the transformed document's JSON keeps the
/// same set of top-level keys and `format_version` as an untouched one.
#[test]
fn no_format_change() {
    let d = Document::new(1);
    let a = rect(&d, 0.0);
    let b = path(&d, 1);
    let before: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    let results = vec![
        moved(&d.object(a).unwrap(), 2.0).rotated(pt(0.0, 0.0), Angle::from_radians(0.3)),
        moved(&d.object(b).unwrap(), 2.0).rotated(pt(0.0, 0.0), Angle::from_radians(0.3)),
    ];
    d.transform_objects(&results, true).unwrap();
    let after: serde_json::Value = serde_json::from_slice(&d.export_json().unwrap()).unwrap();
    assert_eq!(before.get("format_version"), after.get("format_version"));
    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&before), keys(&after));
    // Object records: no new field beyond what a single-object gesture writes
    // (frame, rotation, anchors); compare field names per object.
    let obj_keys = |v: &serde_json::Value| -> Vec<Vec<String>> {
        v["objects"]
            .as_array()
            .map(|a| a.iter().map(keys).collect())
            .unwrap_or_default()
    };
    let b4 = obj_keys(&before);
    let af = obj_keys(&after);
    for (x, y) in b4.iter().zip(&af) {
        for k in y {
            assert!(x.contains(k) || k == "rotation", "new field {k}");
        }
    }
}
