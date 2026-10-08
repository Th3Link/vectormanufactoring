//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 4, document
//! side: the stop list as a mergeable child container (criterion 16, concurrent
//! creation), files written before that change, round trips of coincident and
//! out-of-range stop lists (16, 35), and the colour a new stop gets (18) at a
//! hard edge. Written from `specification.md` and the PR 4 decision note in
//! `adrs.md`; the implementation was not read.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names, missing_docs)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color, CopySource, Document, FillMode,
    FillModeTarget, GradientStop, Length, NodeId, ObjectSnapshot, Opacity, Point, RectBounds,
    StopChange, StopEdit, StopId, StopPosition, Vec2, pack, ramp_at, sorted_stops, unpack,
};
use loro::{LoroDoc, LoroMap, LoroMovableList, LoroValue};

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn stop(peer: u64, n: u64, p: f64, c: Color, o: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(peer, n),
        position: StopPosition::new(p).unwrap(),
        color: c,
        opacity: Opacity::new(o).unwrap(),
    }
}

fn rect(d: &Document) -> NodeId {
    d.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(6.0),
    })
}

fn target(id: NodeId, seeds: Vec<GradientStop>) -> FillModeTarget {
    FillModeTarget {
        id,
        seed_stops: seeds,
    }
}

fn stops_of(d: &Document, id: NodeId) -> Vec<GradientStop> {
    match d.object(id).unwrap() {
        ObjectSnapshot::Path(p) => p.style.fill.stops,
        ObjectSnapshot::Primitive(p) => p.style.fill.stops,
    }
}

fn container(loro_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": CURRENT_FORMAT_VERSION,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes.to_vec()),
        ("document.json", b"{}".to_vec()),
    ] {
        w.start_file(name, o).unwrap();
        w.write_all(&bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn merged(a: &Document, b: &Document) -> Document {
    let l = LoroDoc::new();
    l.import(&a.export_loro_snapshot().unwrap()).unwrap();
    l.import(&b.export_loro_snapshot().unwrap()).unwrap();
    l.commit();
    unpack(
        99,
        &container(&l.export(loro::ExportMode::Snapshot).unwrap()),
    )
    .expect("merged document opens")
}

fn reopen(d: &Document) -> Document {
    unpack(77, &pack(d, "tester").unwrap()).unwrap()
}

fn seed(peer: u64, base: Color) -> Vec<GradientStop> {
    GradientStop::default_pair(base, StopId::new(peer, 1), StopId::new(peer, 2)).to_vec()
}

fn color_edit(id: NodeId, s: StopId, c: Color) -> StopEdit {
    StopEdit {
        id,
        stop: s,
        change: StopChange::Color(c),
    }
}

// ------------------------------------------ concurrent creation (16, 35)

#[test]
fn concurrent_creation_keeps_both_peers_stops_and_the_edits_made_to_them() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.set_fill_mode(FillMode::Linear, &[target(id, seed(2, rgb(255, 0, 0)))])
        .unwrap();
    b.set_fill_mode(FillMode::Linear, &[target(id, seed(3, rgb(0, 0, 255)))])
        .unwrap();
    // After creating, each peer edits its own first stop and appends a third.
    a.edit_stops(&[color_edit(id, StopId::new(2, 1), rgb(1, 1, 1))])
        .unwrap();
    b.edit_stops(&[color_edit(id, StopId::new(3, 2), rgb(2, 2, 2))])
        .unwrap();
    a.add_stop(id, stop(2, 3, 0.5, rgb(9, 9, 9), 1.0)).unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let st = stops_of(&m, id);
        let by_id = |s: StopId| st.iter().find(|x| x.id == s).copied();
        assert_eq!(by_id(StopId::new(2, 1)).unwrap().color, rgb(1, 1, 1));
        assert_eq!(by_id(StopId::new(3, 2)).unwrap().color, rgb(2, 2, 2));
        assert!(by_id(StopId::new(2, 3)).is_some(), "peer 2's added stop");
        assert!(by_id(StopId::new(2, 2)).is_some() && by_id(StopId::new(3, 1)).is_some());
        assert_eq!(st.len(), 5, "two seed pairs plus one added stop");
        // The merged file saves and reopens identically and stays editable.
        let r = reopen(&m);
        assert_eq!(stops_of(&r, id), st);
        r.edit_stops(&[color_edit(id, StopId::new(3, 1), rgb(7, 7, 7))])
            .unwrap();
        r.remove_stop(id, StopId::new(2, 3)).unwrap();
        assert_eq!(stops_of(&r, id).len(), 4);
    }
    assert_eq!(
        stops_of(&merged(&a, &b), id),
        stops_of(&merged(&b, &a), id),
        "merge order does not change the list"
    );
}

#[test]
fn three_peers_creating_at_once_converge() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let peers: Vec<Document> = (2..5).map(|p| unpack(p, &bytes).unwrap()).collect();
    for (i, d) in peers.iter().enumerate() {
        let mode = if i % 2 == 0 {
            FillMode::Linear
        } else {
            FillMode::Radial
        };
        d.set_fill_mode(mode, &[target(id, seed(2 + i as u64, rgb(i as u8, 0, 0)))])
            .unwrap();
    }
    let ab = merged(&merged(&peers[0], &peers[1]), &peers[2]);
    let ca = merged(&merged(&peers[2], &peers[0]), &peers[1]);
    assert_eq!(stops_of(&ab, id), stops_of(&ca, id));
    assert_eq!(stops_of(&ab, id).len(), 6);
    assert_eq!(
        ab.object(id).map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        }),
        ca.object(id).map(|o| match o {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
    );
}

#[test]
fn one_peer_creates_while_the_other_only_edits_the_solid_colour() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.set_fill_mode(FillMode::Radial, &[target(id, seed(2, rgb(255, 0, 0)))])
        .unwrap();
    b.edit_style(
        &[id],
        &curvyo_document_core::StyleEdit::FillColor(rgb(0, 255, 0)),
    )
    .unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let st = stops_of(&m, id);
        assert_eq!(st.len(), 2);
        let s = match m.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => p.style,
            ObjectSnapshot::Path(p) => p.style,
        };
        assert_eq!(s.fill.color, rgb(0, 255, 0));
        assert!(s.fill.enabled);
    }
}

// ------------------------------------ files that predate the mergeable list

/// Builds a container whose only object holds its stops in a regular (not
/// mergeable) movable list, as a PR-3 build or an earlier PR-4 build wrote it.
fn legacy_regular_list(stops: &[(u64, f64)]) -> (Vec<u8>, NodeId) {
    let d = Document::new(1);
    let id = rect(&d);
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let tree = l.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    meta.insert("fill_enabled", true).unwrap();
    meta.insert("fill_kind", "linear").unwrap();
    let list = meta
        .insert_container("fill_stops", LoroMovableList::new())
        .unwrap();
    for (n, p) in stops {
        let m = list.push_container(LoroMap::new()).unwrap();
        m.insert("id", StopId::new(5, *n).to_hex()).unwrap();
        m.insert("position", *p).unwrap();
        m.insert(
            "color",
            LoroValue::from(vec![
                LoroValue::from(10_i64),
                LoroValue::from(20_i64),
                LoroValue::from(30_i64),
            ]),
        )
        .unwrap();
        m.insert("opacity", 1.0_f64).unwrap();
    }
    l.commit();
    (
        container(&l.export(loro::ExportMode::Snapshot).unwrap()),
        id,
    )
}

#[test]
fn a_file_with_a_regular_stop_list_opens_edits_and_merges() {
    let (bytes, _) = legacy_regular_list(&[(1, 0.0), (2, 1.0)]);
    let d = unpack(2, &bytes).expect("older stop list opens");
    let id = d.object_ids()[0];
    let st = stops_of(&d, id);
    assert_eq!(st.len(), 2);
    assert_eq!(st[0].color, rgb(10, 20, 30));
    d.edit_stops(&[color_edit(id, StopId::new(5, 1), rgb(1, 2, 3))])
        .unwrap();
    d.add_stop(id, stop(2, 1, 0.5, rgb(4, 5, 6), 1.0)).unwrap();
    assert_eq!(stops_of(&d, id).len(), 3);
    d.remove_stop(id, StopId::new(5, 2)).unwrap();
    assert_eq!(stops_of(&reopen(&d), id).len(), 2);

    // Two peers on the regular list: both adds survive, as before.
    let a = unpack(3, &bytes).unwrap();
    let b = unpack(4, &bytes).unwrap();
    a.add_stop(id, stop(3, 1, 0.25, Color::BLACK, 1.0)).unwrap();
    b.add_stop(id, stop(4, 1, 0.75, Color::BLACK, 1.0)).unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        assert_eq!(stops_of(&m, id).len(), 4);
    }
}

#[test]
fn switching_mode_on_a_file_with_a_regular_list_keeps_its_stops() {
    let (bytes, _) = legacy_regular_list(&[(1, 0.1), (2, 0.9)]);
    let d = unpack(2, &bytes).unwrap();
    let id = d.object_ids()[0];
    let before = stops_of(&d, id);
    for mode in [
        FillMode::Radial,
        FillMode::Solid,
        FillMode::None,
        FillMode::Linear,
    ] {
        d.set_fill_mode(mode, &[target(id, seed(8, Color::BLACK))])
            .unwrap();
        assert_eq!(stops_of(&d, id), before, "{mode:?} kept the stops");
    }
}

// ------------------------------------------------------- round trips

#[test]
fn coincident_and_unordered_lists_round_trip_in_list_order() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let list = vec![
        stop(1, 1, 0.5, rgb(1, 0, 0), 0.25),
        stop(1, 2, 0.0, rgb(2, 0, 0), 1.0),
        stop(1, 3, 0.5, rgb(3, 0, 0), 0.125),
        stop(1, 4, 0.123_456_789, rgb(4, 0, 0), 0.01),
        stop(1, 5, 1.0, rgb(5, 0, 0), 0.0),
    ];
    doc.set_fill_mode(FillMode::Radial, &[target(id, list.clone())])
        .unwrap();
    let r = reopen(&reopen(&doc));
    assert_eq!(stops_of(&r, id), list);
    // The sorted view is stable over the coincident pair.
    let ids: Vec<StopId> = sorted_stops(&stops_of(&r, id))
        .iter()
        .map(|s| s.id)
        .collect();
    let want: Vec<StopId> = [2, 4, 1, 3, 5].iter().map(|n| StopId::new(1, *n)).collect();
    assert_eq!(ids, want);
}

#[test]
fn zero_one_and_many_stop_lists_open_save_and_stay_unchanged() {
    for n in [0_usize, 1, 16, 17, 40] {
        let doc = Document::new(1);
        let id = rect(&doc);
        let list: Vec<_> = (0..n)
            .map(|k| stop(1, k as u64 + 1, k as f64 / 40.0, rgb(k as u8, 0, 0), 1.0))
            .collect();
        doc.set_fill_mode(FillMode::Linear, &[target(id, list.clone())])
            .unwrap();
        let r = reopen(&doc);
        assert_eq!(stops_of(&r, id), list, "{n} stops");
        // Opening and saving changes nothing: a second round trip is the same.
        assert_eq!(stops_of(&reopen(&r), id), list);
    }
}

// ------------------------------------- AC 18 at a hard edge (new stop)

/// Adding a stop "changes nothing on screen": insert one at `t` carrying the
/// ramp colour of `t` through the document, then compare the ramp everywhere.
fn assert_adding_changes_nothing(list: Vec<GradientStop>, t: f64) {
    let doc = Document::new(1);
    let id = rect(&doc);
    doc.set_fill_mode(FillMode::Linear, &[target(id, list)])
        .unwrap();
    let before = sorted_stops(&stops_of(&doc, id));
    let (c, o) = ramp_at(&before, t).unwrap();
    doc.add_stop(
        id,
        GradientStop {
            id: StopId::new(8, 1),
            position: StopPosition::new(t).unwrap(),
            color: c,
            opacity: o,
        },
    )
    .unwrap();
    let after = sorted_stops(&stops_of(&doc, id));
    assert_eq!(after.len(), before.len() + 1);
    for k in 0..=2000 {
        let u = f64::from(k) / 2000.0;
        let (c0, o0) = ramp_at(&before, u).unwrap();
        let (c1, o1) = ramp_at(&after, u).unwrap();
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
        assert!(
            d(c0.r, c1.r) <= 1 && d(c0.g, c1.g) <= 1 && d(c0.b, c1.b) <= 1,
            "t {u}: {c0:?} became {c1:?} after adding at {t}"
        );
        assert!((o0.get() - o1.get()).abs() < 0.01, "opacity at {u}");
    }
}

#[test]
fn adding_a_stop_on_a_smooth_ramp_changes_nothing() {
    let l = vec![
        stop(1, 1, 0.0, rgb(255, 0, 0), 1.0),
        stop(1, 2, 0.4, rgb(0, 255, 0), 0.3),
        stop(1, 3, 1.0, rgb(0, 0, 255), 1.0),
    ];
    for t in [0.0, 0.1, 0.4, 0.77, 1.0] {
        assert_adding_changes_nothing(l.clone(), t);
    }
}

#[test]
fn adding_a_stop_exactly_on_a_hard_edge_changes_nothing() {
    let l = vec![
        stop(1, 1, 0.0, rgb(0, 0, 0), 1.0),
        stop(1, 2, 0.5, rgb(255, 0, 0), 1.0),
        stop(1, 3, 0.5, rgb(0, 0, 255), 1.0),
        stop(1, 4, 1.0, rgb(255, 255, 255), 1.0),
    ];
    assert_adding_changes_nothing(l, 0.5);
}

#[test]
fn adding_a_stop_on_a_hard_edge_at_an_end_changes_nothing() {
    let l = vec![
        stop(1, 1, 0.0, rgb(255, 0, 0), 1.0),
        stop(1, 2, 0.0, rgb(0, 0, 255), 1.0),
        stop(1, 3, 1.0, rgb(255, 255, 255), 1.0),
    ];
    assert_adding_changes_nothing(l, 0.0);
    let l = vec![
        stop(1, 1, 0.0, rgb(255, 255, 255), 1.0),
        stop(1, 2, 1.0, rgb(255, 0, 0), 1.0),
        stop(1, 3, 1.0, rgb(0, 0, 255), 0.5),
    ];
    assert_adding_changes_nothing(l, 1.0);
}

// -------------------------------- AC 30-33 with a multi-stop gradient

fn three_stop(doc: &Document, id: NodeId) {
    doc.set_fill_mode(
        FillMode::Linear,
        &[target(
            id,
            vec![
                stop(1, 1, 0.0, rgb(255, 0, 0), 1.0),
                stop(1, 2, 0.5, rgb(0, 255, 0), 0.5),
                stop(1, 3, 0.5, rgb(0, 0, 255), 0.25),
            ],
        )],
    )
    .unwrap();
}

#[test]
fn ac30_a_copy_has_the_same_stops_and_the_two_are_independent_both_ways() {
    let doc = Document::new(1);
    let id = rect(&doc);
    three_stop(&doc, id);
    let copies = doc
        .duplicate_objects(
            &[CopySource {
                id,
                anchor_ids: vec![],
            }],
            Vec2::new(30.0, 0.0),
        )
        .unwrap();
    let copy = copies[0];
    assert_eq!(stops_of(&doc, copy), stops_of(&doc, id));
    let original = stops_of(&doc, id);
    // Edit the copy: the original does not change.
    doc.edit_stops(&[color_edit(copy, StopId::new(1, 2), rgb(9, 9, 9))])
        .unwrap();
    doc.remove_stop(copy, StopId::new(1, 3)).unwrap();
    assert_eq!(stops_of(&doc, id), original);
    assert_eq!(stops_of(&doc, copy).len(), 2);
    // Edit the original: the copy does not change.
    let copy_stops = stops_of(&doc, copy);
    doc.add_stop(id, stop(1, 4, 0.9, rgb(1, 1, 1), 1.0))
        .unwrap();
    doc.edit_stops(&[color_edit(id, StopId::new(1, 1), rgb(8, 8, 8))])
        .unwrap();
    assert_eq!(stops_of(&doc, copy), copy_stops);
    // And after a save.
    let r = reopen(&doc);
    assert_eq!(stops_of(&r, copy), copy_stops);
}

#[test]
fn ac33_object_to_path_keeps_every_stop() {
    use curvyo_document_core::NewAnchor;
    let doc = Document::new(1);
    let id = rect(&doc);
    three_stop(&doc, id);
    let before = stops_of(&doc, id);
    let anchors: Vec<NewAnchor> = [(0.0, 0.0), (10.0, 0.0), (10.0, 6.0), (0.0, 6.0)]
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            NewAnchor::corner(
                curvyo_document_core::AnchorId::new(1, i as u64 + 1),
                Point::new(*x, *y),
            )
        })
        .collect();
    doc.convert_to_paths(&[(id, anchors)]).unwrap();
    match doc.object(id).unwrap() {
        ObjectSnapshot::Path(p) => {
            assert_eq!(p.style.fill.stops, before);
            assert!(p.style.fill.enabled);
        }
        ObjectSnapshot::Primitive(_) => panic!("still a primitive"),
    }
}

#[test]
fn stop_commands_refuse_stale_ids_without_writing() {
    let doc = Document::new(1);
    let id = rect(&doc);
    three_stop(&doc, id);
    let before = stops_of(&doc, id);
    assert!(
        doc.edit_stops(&[
            color_edit(id, StopId::new(1, 1), rgb(5, 5, 5)),
            color_edit(id, StopId::new(66, 66), rgb(5, 5, 5)),
        ])
        .is_err()
    );
    assert_eq!(stops_of(&doc, id), before, "a bad batch writes nothing");
    assert!(doc.remove_stop(id, StopId::new(66, 66)).is_err());
}

// ------------------------------------ every shipped fixture takes a gradient

#[test]
fn every_fixture_opens_and_every_object_in_it_can_become_a_gradient_and_back() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut opened = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("curvyo") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(doc) = unpack(3, &bytes) else {
            continue; // a fixture that is meant to be refused
        };
        opened += 1;
        for (k, id) in doc.object_ids().into_iter().enumerate() {
            doc.set_fill_mode(
                FillMode::Radial,
                &[target(id, seed(100 + k as u64, Color::BLACK))],
            )
            .unwrap();
            let first = stops_of(&doc, id)[0].id;
            let n = stops_of(&doc, id).len();
            doc.edit_stops(&[color_edit(id, first, rgb(1, 2, 3))])
                .unwrap();
            doc.add_stop(id, stop(200, k as u64 + 1, 0.5, rgb(4, 5, 6), 1.0))
                .unwrap();
            assert_eq!(stops_of(&doc, id).len(), n + 1);
        }
        let r = reopen(&doc);
        for id in doc.object_ids() {
            assert_eq!(stops_of(&r, id), stops_of(&doc, id), "{path:?}");
            assert!(stops_of(&r, id).len() >= 3);
        }
    }
    assert!(opened >= 3, "found {opened} fixtures");
}
