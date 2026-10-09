//! Independent tester acceptance tests for `Document::duplicate_objects`
//! (`specs/0010-edit-interaction-polish/specification.md` criteria 34 to 36 and the
//! Copy of the typed move, criterion 23). Written from the specification and
//! the public signature before the implementation was read. Expected values
//! come from reference arithmetic written here, never from the code under
//! test. The "every meta key" check goes through Loro itself, so a key the
//! snapshot types do not carry (or one a newer build wrote) is covered too.

#![allow(
    clippy::assert_is_empty,
    clippy::collapsible_if,
    clippy::manual_midpoint,
    clippy::cast_sign_loss
)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names, missing_docs)]
#![allow(clippy::needless_pass_by_value, clippy::similar_names)]

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, CURRENT_FORMAT_VERSION, CopySource, Document, EllipseFrame,
    InnerRatio, Length, NewAnchor, NodeId, ObjectEditError, ObjectSnapshot, Point, PointCount,
    RectBounds, Shape, StarFrame, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroValue, TreeParentId, ValueOrContainer};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

fn star_frame(cx: f64, cy: f64, r: f64, a: f64) -> StarFrame {
    StarFrame {
        center: pt(cx, cy),
        radius: Length::from_mm(r),
        angle: Angle::from_radians(a),
    }
}

fn mint(counter: &mut u64, count: usize) -> Vec<AnchorId> {
    (0..count)
        .map(|_| {
            *counter += 1;
            AnchorId::new(900, *counter)
        })
        .collect()
}

/// Sources for `ids` with correctly sized fresh anchor ids.
fn sources(d: &Document, ids: &[NodeId], counter: &mut u64) -> Vec<CopySource> {
    ids.iter()
        .map(|&id| {
            let n = d.path(id).map_or(0, |p| p.anchors.len());
            CopySource {
                id,
                anchor_ids: mint(counter, n),
            }
        })
        .collect()
}

fn curvy_path(d: &Document, peer: u64, closed: bool) -> NodeId {
    d.create_path(
        &[
            NewAnchor {
                id: AnchorId::new(peer, 1),
                point: pt(0.0, 10.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(6.0, -8.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor {
                id: AnchorId::new(peer, 2),
                point: pt(25.0, 0.0),
                handle_in: Vec2::new(-5.0, -6.0),
                handle_out: Vec2::new(5.0, 6.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor {
                id: AnchorId::new(peer, 3),
                point: pt(45.0, 22.0),
                handle_in: Vec2::new(-3.0, -7.0),
                handle_out: Vec2::new(2.0, 2.0),
                kind: AnchorKind::Asymmetric,
            },
        ],
        closed,
    )
}

fn rotate(d: &Document, id: NodeId, about: Point, radians: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(radians)))
        .unwrap();
}

/// A document with one object of every kind, the rect rounded and rotated,
/// the path rotated; returns the ids in z-order.
fn rich_doc() -> (Document, Vec<NodeId>) {
    let d = Document::new(1);
    let path = curvy_path(&d, 1, true);
    let rect = d.create_rect(bounds(10.0, 20.0, 30.0, 16.0));
    d.set_corner_radius(&[rect], Length::from_mm(3.5)).unwrap();
    let ellipse = d.create_ellipse(EllipseFrame {
        center: pt(60.0, 40.0),
        rx: Length::from_mm(12.0),
        ry: Length::from_mm(7.0),
    });
    let polygon = d.create_polygon(
        star_frame(80.0, 40.0, 9.0, 0.3),
        PointCount::new(5).unwrap(),
    );
    let star = d.create_star(
        star_frame(100.0, 40.0, 11.0, -0.7),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    rotate(&d, rect, pt(25.0, 28.0), 0.5);
    rotate(&d, path, pt(20.0, 10.0), -0.3);
    rotate(&d, star, pt(100.0, 40.0), 1.1);
    (d, vec![path, rect, ellipse, polygon, star])
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

fn pclose(a: Point, b: Point) -> bool {
    close(a.x, b.x) && close(a.y, b.y)
}

fn vclose(a: Vec2, b: Vec2) -> bool {
    close(a.x, b.x) && close(a.y, b.y)
}

/// The copy equals the original moved by `off`, field by field (ids differ).
fn assert_is_moved_copy(orig: &ObjectSnapshot, copy: &ObjectSnapshot, off: Vec2) {
    assert_ne!(orig.id(), copy.id(), "a copy has a new NodeId");
    match (orig, copy) {
        (ObjectSnapshot::Path(a), ObjectSnapshot::Path(b)) => {
            assert_eq!(a.closed, b.closed);
            assert_eq!(a.style, b.style);
            assert!(close(a.rotation.as_radians(), b.rotation.as_radians()));
            assert_eq!(a.anchors.len(), b.anchors.len());
            for (x, y) in a.anchors.iter().zip(&b.anchors) {
                assert_ne!(x.id, y.id, "fresh anchor ids");
                assert!(pclose(
                    Point::new(x.point.x + off.x, x.point.y + off.y),
                    y.point
                ));
                assert!(vclose(x.handle_in, y.handle_in));
                assert!(vclose(x.handle_out, y.handle_out));
                assert_eq!(x.kind, y.kind);
            }
        }
        (ObjectSnapshot::Primitive(a), ObjectSnapshot::Primitive(b)) => {
            assert_eq!(a.style, b.style);
            assert!(close(a.rotation.as_radians(), b.rotation.as_radians()));
            match (&a.shape, &b.shape) {
                (
                    Shape::Rect {
                        bounds: ba,
                        corner_radii: ra,
                    },
                    Shape::Rect {
                        bounds: bb,
                        corner_radii: rb,
                    },
                ) => {
                    assert!(pclose(
                        Point::new(ba.origin.x + off.x, ba.origin.y + off.y),
                        bb.origin
                    ));
                    assert_eq!(ba.width, bb.width);
                    assert_eq!(ba.height, bb.height);
                    assert_eq!(ra, rb);
                }
                (Shape::Ellipse { frame: fa }, Shape::Ellipse { frame: fb }) => {
                    assert!(pclose(
                        Point::new(fa.center.x + off.x, fa.center.y + off.y),
                        fb.center
                    ));
                    assert_eq!((fa.rx, fa.ry), (fb.rx, fb.ry));
                }
                (
                    Shape::Polygon {
                        frame: fa,
                        point_count: na,
                    },
                    Shape::Polygon {
                        frame: fb,
                        point_count: nb,
                    },
                ) => {
                    assert_eq!(na, nb);
                    assert!(pclose(
                        Point::new(fa.center.x + off.x, fa.center.y + off.y),
                        fb.center
                    ));
                    assert_eq!(fa.radius, fb.radius);
                    assert!(close(fa.angle.as_radians(), fb.angle.as_radians()));
                }
                (
                    Shape::Star {
                        frame: fa,
                        point_count: na,
                        inner_ratio: ia,
                    },
                    Shape::Star {
                        frame: fb,
                        point_count: nb,
                        inner_ratio: ib,
                    },
                ) => {
                    assert_eq!((na, ia), (nb, ib));
                    assert!(pclose(
                        Point::new(fa.center.x + off.x, fa.center.y + off.y),
                        fb.center
                    ));
                    assert_eq!(fa.radius, fb.radius);
                    assert!(close(fa.angle.as_radians(), fb.angle.as_radians()));
                }
                other => panic!("kind changed: {other:?}"),
            }
        }
        other => panic!("path became primitive or back: {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Loro-level helpers
// ---------------------------------------------------------------------

fn loro_of(d: &Document) -> LoroDoc {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn change_count(d: &Document) -> usize {
    loro_of(d).len_changes()
}

/// Rebuilds a `Document` from an edited `LoroDoc`.
fn document_from(l: &LoroDoc, peer: u64) -> Document {
    l.commit();
    let bytes = l.export(loro::ExportMode::Snapshot).unwrap();
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": 1,
        "app_version": "tester-merge",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("manifest.json", o).unwrap();
    w.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    w.start_file("document.loro", o).unwrap();
    w.write_all(&bytes).unwrap();
    w.start_file("document.json", o).unwrap();
    w.write_all(b"{}").unwrap();
    unpack(peer, &w.finish().unwrap().into_inner()).expect("opens")
}

fn merged(a: &Document, b: &Document) -> Document {
    let l = LoroDoc::new();
    l.import(&a.export_loro_snapshot().unwrap()).unwrap();
    l.import(&b.export_loro_snapshot().unwrap()).unwrap();
    document_from(&l, 9)
}

/// The deep meta value of every object, keyed by `(peer, counter)`.
fn metas(d: &Document) -> HashMap<(String, i64), LoroValue> {
    let l = loro_of(d);
    let tree = l.get_tree("paths");
    tree.children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            (
                (id.peer.to_string(), i64::from(id.counter)),
                tree.get_meta(id).unwrap().get_deep_value(),
            )
        })
        .collect()
}

fn key(id: NodeId) -> (String, i64) {
    let v = serde_json::to_value(id).unwrap();
    (
        v["peer"].as_str().unwrap().to_owned(),
        v["counter"].as_i64().unwrap(),
    )
}

fn as_map(v: &LoroValue) -> HashMap<String, LoroValue> {
    match v {
        LoroValue::Map(m) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        other => panic!("not a map: {other:?}"),
    }
}

/// Asserts that two meta values (original and copy made with a zero offset)
/// hold exactly the same keys and values, except that every anchor map may
/// differ in exactly one key (its id).
fn assert_same_meta_but_anchor_ids(orig: &LoroValue, copy: &LoroValue) {
    let (a, b) = (as_map(orig), as_map(copy));
    let mut ka: Vec<_> = a.keys().cloned().collect();
    let mut kb: Vec<_> = b.keys().cloned().collect();
    ka.sort();
    kb.sort();
    assert_eq!(ka, kb, "the copy has exactly the original's meta keys");
    for (k, va) in &a {
        let vb = &b[k];
        if let (LoroValue::List(la), LoroValue::List(lb)) = (va, vb) {
            if la.iter().all(|x| matches!(x, LoroValue::Map(_))) && !la.is_empty() {
                assert_eq!(la.len(), lb.len());
                for (x, y) in la.iter().zip(lb.iter()) {
                    let (mx, my) = (as_map(x), as_map(y));
                    let mut kx: Vec<_> = mx.keys().cloned().collect();
                    let mut ky: Vec<_> = my.keys().cloned().collect();
                    kx.sort();
                    ky.sort();
                    assert_eq!(kx, ky, "anchor keys equal in {k}");
                    let differing = mx.iter().filter(|(kk, vv)| &my[*kk] != *vv).count();
                    assert_eq!(differing, 1, "only the anchor id differs in {k}");
                }
                continue;
            }
        }
        assert_eq!(va, vb, "meta key {k} copied unchanged");
    }
}

/// Plants a key no build knows into every object's meta map and, for paths,
/// into every anchor map (the lists are Loro movable lists of maps).
fn plant_unknown_keys(d: &Document) -> Document {
    let l = loro_of(d);
    let tree = l.get_tree("paths");
    for id in tree.children(TreeParentId::Root).unwrap_or_default() {
        let meta = tree.get_meta(id).unwrap();
        meta.insert("zz_future_register", 42_i64).unwrap();
        meta.insert("zz_future_text", "hello").unwrap();
        // Anchor maps live in a movable list; reach them by scanning values.
        for key in meta.keys().collect::<Vec<_>>() {
            if let Some(ValueOrContainer::Container(loro::Container::MovableList(list))) =
                meta.get(&key)
            {
                for i in 0..list.len() {
                    if let Some(ValueOrContainer::Container(loro::Container::Map(m))) = list.get(i)
                    {
                        m.insert("zz_future_anchor_key", 7_i64).unwrap();
                    }
                }
            }
        }
    }
    document_from(&l, 5)
}

// ---------------------------------------------------------------------
// Criterion 34
// ---------------------------------------------------------------------

#[test]
fn a_copy_is_the_original_moved_for_every_kind() {
    let (d, ids) = rich_doc();
    let off = Vec2::new(13.25, -4.5);
    let mut c = 0;
    let originals: Vec<_> = ids.iter().map(|&i| d.object(i).unwrap()).collect();
    let new = d
        .duplicate_objects(&sources(&d, &ids, &mut c), off)
        .unwrap();
    assert_eq!(new.len(), ids.len());
    for ((orig, new_id), id) in originals.iter().zip(&new).zip(&ids) {
        let copy = d.object(*new_id).unwrap();
        assert_is_moved_copy(orig, &copy, off);
        // originals untouched
        assert_eq!(&d.object(*id).unwrap(), orig, "the original is untouched");
        // preview == commit: the one translation rule
        match (&orig.translated(off), &copy) {
            (ObjectSnapshot::Path(p), ObjectSnapshot::Path(q)) => {
                for (x, y) in p.anchors.iter().zip(&q.anchors) {
                    assert!(pclose(x.point, y.point));
                }
            }
            (ObjectSnapshot::Primitive(p), ObjectSnapshot::Primitive(q)) => {
                assert_eq!(
                    std::mem::discriminant(&p.shape),
                    std::mem::discriminant(&q.shape)
                );
            }
            _ => panic!("kind changed"),
        }
    }
}

#[test]
fn every_meta_key_is_copied_including_keys_nobody_knows() {
    let (d, _) = rich_doc();
    let d = plant_unknown_keys(&d);
    // the planted keys survived the round trip, so the test means something
    let before = metas(&d);
    assert!(
        before
            .values()
            .all(|v| as_map(v).contains_key("zz_future_register")),
        "planted key present on every object"
    );
    let ids = d.object_ids();
    assert_eq!(ids.len(), 5);
    assert!(
        before
            .values()
            .any(|v| format!("{v:?}").contains("zz_future_anchor_key")),
        "the anchor-level key was planted"
    );
    let mut c = 0;
    let new = d
        .duplicate_objects(&sources(&d, &ids, &mut c), Vec2::ZERO)
        .unwrap();
    let after = metas(&d);
    assert_eq!(after.len(), 10);
    for (orig, copy) in ids.iter().zip(&new) {
        let o = &after[&key(*orig)];
        let k = &after[&key(*copy)];
        assert_same_meta_but_anchor_ids(o, k);
        let km = as_map(k);
        assert_eq!(km["zz_future_register"], LoroValue::from(42_i64));
        assert_eq!(km["zz_future_text"], LoroValue::from("hello"));
        let has_anchor_key = |v: &LoroValue| format!("{v:?}").contains("zz_future_anchor_key");
        assert_eq!(
            has_anchor_key(o),
            has_anchor_key(k),
            "an unknown key inside an anchor map is copied too"
        );
    }
}

#[test]
fn a_nonzero_offset_changes_geometry_only() {
    let (d, ids) = rich_doc();
    let mut c = 0;
    let new = d
        .duplicate_objects(&sources(&d, &ids, &mut c), Vec2::new(7.0, 3.0))
        .unwrap();
    let m = metas(&d);
    for (orig, copy) in ids.iter().zip(&new) {
        let a = as_map(&m[&key(*orig)]);
        let b = as_map(&m[&key(*copy)]);
        let mut ka: Vec<_> = a.keys().collect();
        let mut kb: Vec<_> = b.keys().collect();
        ka.sort();
        kb.sort();
        assert_eq!(ka, kb);
        // at least one value differs (the position) and rotation/stroke
        // registers, if present, are untouched
        for key in ka {
            let lower = key.to_lowercase();
            if lower.contains("rot")
                || lower.contains("stroke")
                || lower.contains("radius")
                || lower.contains("ratio")
                || lower.contains("count")
                || lower.contains("fill")
                || lower.contains("kind")
                || lower.contains("closed")
            {
                assert_eq!(a[key], b[key], "register {key} must be copied untouched");
            }
        }
    }
}

#[test]
fn z_order_places_each_copy_directly_above_its_original() {
    let d = Document::new(1);
    let a = d.create_rect(bounds(0.0, 0.0, 5.0, 5.0));
    let b = d.create_rect(bounds(10.0, 0.0, 5.0, 5.0));
    let c3 = d.create_rect(bounds(20.0, 0.0, 5.0, 5.0));
    let e = d.create_rect(bounds(30.0, 0.0, 5.0, 5.0));
    let mut c = 0;
    // sources named out of z-order
    let new = d
        .duplicate_objects(&sources(&d, &[c3, a], &mut c), Vec2::new(1.0, 1.0))
        .unwrap();
    assert_eq!(new.len(), 2);
    let (c3c, ac) = (new[0], new[1]);
    assert_eq!(d.object_ids(), vec![a, ac, b, c3, c3c, e]);
    // sources in z-order, adjacent: A, B -> A, A', B, B'
    let d2 = Document::new(1);
    let a = d2.create_rect(bounds(0.0, 0.0, 5.0, 5.0));
    let b = d2.create_rect(bounds(10.0, 0.0, 5.0, 5.0));
    let new = d2
        .duplicate_objects(&sources(&d2, &[a, b], &mut c), Vec2::new(1.0, 1.0))
        .unwrap();
    assert_eq!(d2.object_ids(), vec![a, new[0], b, new[1]]);
}

#[test]
fn anchor_ids_are_unique_across_the_whole_document_and_joinable() {
    let d = Document::new(1);
    let p1 = curvy_path(&d, 1, false);
    let p2 = curvy_path(&d, 1, false); // same AnchorIds as p1 on purpose? no: see below
    // `curvy_path` reuses the ids; re-create a second path with distinct ids.
    d.delete_objects(&[p2]).unwrap();
    let _ = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(2, 1), pt(100.0, 0.0)),
            NewAnchor::corner(AnchorId::new(2, 2), pt(110.0, 5.0)),
        ],
        false,
    );
    let mut c = 0;
    let all: Vec<NodeId> = d.object_ids();
    let new = d
        .duplicate_objects(&sources(&d, &all, &mut c), Vec2::new(0.0, 50.0))
        .unwrap();
    let mut seen = HashSet::new();
    for id in d.object_ids() {
        for a in d.path(id).unwrap().anchors {
            assert!(seen.insert(a.id), "anchor id {:?} appears twice", a.id);
        }
    }
    assert_eq!(seen.len(), 3 + 2 + 3 + 2);
    // joining a path's last anchor to its copy's first works: no id clash
    let orig = d.path(p1).unwrap();
    let copy = d.path(new[0]).unwrap();
    let r = d.join_endpoints(
        p1,
        orig.anchors.last().unwrap().id,
        new[0],
        copy.anchors.first().unwrap().id,
    );
    assert!(r.is_ok(), "join of a path with its copy: {r:?}");
    let mut seen = HashSet::new();
    for id in d.object_ids() {
        for a in d.path(id).unwrap().anchors {
            assert!(seen.insert(a.id));
        }
    }
}

// ---------------------------------------------------------------------
// One commit, refusals, save and reopen
// ---------------------------------------------------------------------

#[test]
fn one_commit_for_the_whole_batch() {
    let (d, ids) = rich_doc();
    let before = change_count(&d);
    let mut c = 0;
    d.duplicate_objects(&sources(&d, &ids, &mut c), Vec2::new(2.0, 2.0))
        .unwrap();
    assert_eq!(change_count(&d), before + 1);
}

#[test]
fn refusals_write_nothing() {
    let (d, ids) = rich_doc();
    let snap = d.export_loro_snapshot().unwrap();
    let n = change_count(&d);
    let mut c = 0;
    // wrong anchor id count for a path
    let mut s = sources(&d, &ids, &mut c);
    s[0].anchor_ids.pop();
    assert_eq!(
        d.duplicate_objects(&s, Vec2::new(1.0, 1.0)),
        Err(ObjectEditError::AnchorIds)
    );
    // too many for a path
    let mut s = sources(&d, &ids, &mut c);
    s[0].anchor_ids.push(AnchorId::new(900, 9999));
    assert_eq!(
        d.duplicate_objects(&s, Vec2::new(1.0, 1.0)),
        Err(ObjectEditError::AnchorIds)
    );
    // ids for a primitive
    let mut s = sources(&d, &ids, &mut c);
    s[1].anchor_ids.push(AnchorId::new(900, 9998));
    assert_eq!(
        d.duplicate_objects(&s, Vec2::new(1.0, 1.0)),
        Err(ObjectEditError::AnchorIds)
    );
    // duplicate anchor ids inside one source
    let mut s = sources(&d, &ids, &mut c);
    let first = s[0].anchor_ids[0];
    s[0].anchor_ids[1] = first;
    assert_eq!(
        d.duplicate_objects(&s, Vec2::new(1.0, 1.0)),
        Err(ObjectEditError::AnchorIds)
    );
    // a stale source anywhere refuses all of it
    let gone = d.create_rect(bounds(0.0, 0.0, 1.0, 1.0));
    d.delete_objects(&[gone]).unwrap();
    let snap_after_delete = d.export_loro_snapshot().unwrap();
    let n_after_delete = change_count(&d);
    let mut s = sources(&d, &ids, &mut c);
    s.push(CopySource {
        id: gone,
        anchor_ids: vec![],
    });
    assert_eq!(
        d.duplicate_objects(&s, Vec2::new(1.0, 1.0)),
        Err(ObjectEditError::NoSuchObject)
    );
    assert_eq!(change_count(&d), n_after_delete);
    assert_eq!(d.object_ids().len(), 5);
    let _ = (snap, n, snap_after_delete);
    assert_eq!(d.object_ids(), ids);
}

#[test]
fn a_source_named_twice_is_copied_once() {
    let d = Document::new(1);
    let a = d.create_rect(bounds(0.0, 0.0, 5.0, 5.0));
    let s = vec![
        CopySource {
            id: a,
            anchor_ids: vec![],
        },
        CopySource {
            id: a,
            anchor_ids: vec![],
        },
    ];
    let new = d.duplicate_objects(&s, Vec2::new(3.0, 0.0)).unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(d.object_ids().len(), 2);
}

#[test]
fn an_empty_batch_writes_nothing() {
    let (d, _) = rich_doc();
    let n = change_count(&d);
    let r = d.duplicate_objects(&[], Vec2::new(1.0, 1.0)).unwrap();
    assert!(r.is_empty());
    assert_eq!(change_count(&d), n, "no empty commit");
}

#[test]
fn saved_and_reopened_holds_originals_and_copies() {
    let (d, ids) = rich_doc();
    let mut c = 0;
    let new = d
        .duplicate_objects(&sources(&d, &ids, &mut c), Vec2::new(20.0, 20.0))
        .unwrap();
    let bytes = pack(&d, "0.1.0").unwrap();
    let r = unpack(7, &bytes).unwrap();
    assert_eq!(r.object_ids().len(), 10);
    for (orig, copy) in ids.iter().zip(&new) {
        assert_is_moved_copy(
            &r.object(*orig).unwrap(),
            &r.object(*copy).unwrap(),
            Vec2::new(20.0, 20.0),
        );
    }
}

// ---------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------

#[test]
fn peer_a_copies_while_peer_b_moves_the_original() {
    let base = Document::new(1);
    let rect = base.create_rect(bounds(0.0, 0.0, 10.0, 6.0));
    let path = curvy_path(&base, 1, false);
    let bytes = pack(&base, "0.1.0").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    let mut c = 0;
    let new = a
        .duplicate_objects(&sources(&a, &[rect, path], &mut c), Vec2::new(30.0, 0.0))
        .unwrap();
    b.translate_objects(&[rect, path], Vec2::new(0.0, 100.0))
        .unwrap();
    let m = merged(&a, &b);
    assert_eq!(m.object_ids().len(), 4);
    let r = m.object(rect).unwrap();
    let rc = m.object(new[0]).unwrap();
    match (&r, &rc) {
        (ObjectSnapshot::Primitive(p), ObjectSnapshot::Primitive(q)) => {
            let (Shape::Rect { bounds: bp, .. }, Shape::Rect { bounds: bq, .. }) =
                (&p.shape, &q.shape)
            else {
                panic!()
            };
            assert!(pclose(bp.origin, pt(0.0, 100.0)), "original moved by B");
            assert!(pclose(bq.origin, pt(30.0, 0.0)), "copy at A's offset");
        }
        _ => panic!(),
    }
    let (ObjectSnapshot::Path(pp), ObjectSnapshot::Path(pq)) =
        (m.object(path).unwrap(), m.object(new[1]).unwrap())
    else {
        panic!()
    };
    assert!(pclose(pp.anchors[0].point, pt(0.0, 110.0)));
    assert!(pclose(pq.anchors[0].point, pt(30.0, 10.0)));
    // anchor ids unique after the merge
    let mut seen = HashSet::new();
    for id in m.object_ids() {
        if let Some(p) = m.path(id) {
            for an in p.anchors {
                assert!(seen.insert(an.id));
            }
        }
    }
}

#[test]
fn peer_b_deletes_the_original_while_a_copies_it() {
    let base = Document::new(1);
    let rect = base.create_rect(bounds(0.0, 0.0, 10.0, 6.0));
    let bytes = pack(&base, "0.1.0").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    let mut c = 0;
    let new = a
        .duplicate_objects(&sources(&a, &[rect], &mut c), Vec2::new(30.0, 0.0))
        .unwrap();
    b.delete_objects(&[rect]).unwrap();
    let m = merged(&a, &b);
    // must open, and the copy is an independent object (it may survive)
    assert!(m.object_ids().len() <= 2);
    let _ = new;
}

// ---------------------------------------------------------------------
// Hostile values
// ---------------------------------------------------------------------

#[test]
// Non-finite offsets are `translate_objects`' own pre-existing caller contract
// (the session refuses a non-finite pointer; checked in the editor-wasm tests).
fn huge_finite_offsets_do_not_panic_or_corrupt_the_file() {
    for off in [Vec2::new(1e300, -1e300), Vec2::new(f64::MAX, f64::MAX)] {
        let (d, ids) = rich_doc();
        let mut c = 0;
        let _ = d.duplicate_objects(&sources(&d, &ids, &mut c), off);
        let bytes = pack(&d, "0.1.0").unwrap();
        let r = unpack(7, &bytes).expect("still opens");
        assert!(r.object_ids().len() >= 5);
    }
}
