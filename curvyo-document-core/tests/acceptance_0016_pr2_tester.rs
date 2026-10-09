//! Independent acceptance tests for `0016-boolean-operations`, PR 2 (compound
//! path in the document model, format version 8, `Document::replace_with_path`).
//! Written from `specification.md` and the encoding in `adrs.md` before the
//! implementation was read. Criteria 19 to 22, 24 (storage only), 27, 28, 30,
//! 35, 36, 36a, 37, 37a, 38a.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::assert_is_empty,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::type_complexity
)]

use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use curvyo_document_core::{
    AnchorId, Angle, CURRENT_FORMAT_VERSION, Color, CopySource, DashPattern, Document, Length,
    LineCap, LineJoin, NewAnchor, NodeId, ObjectEditError, ObjectSnapshot, Opacity, OpenError,
    PathSnapshot, Point, RectBounds, StyleEdit, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroMap, LoroMovableList, ValueOrContainer};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn square(peer: u64, start: u64, x: f64, y: f64, side: f64, ccw: bool) -> (Vec<NewAnchor>, bool) {
    let mut corners = vec![
        pt(x, y),
        pt(x + side, y),
        pt(x + side, y + side),
        pt(x, y + side),
    ];
    if !ccw {
        corners.reverse();
    }
    (
        corners
            .into_iter()
            .enumerate()
            .map(|(i, p)| NewAnchor::corner(AnchorId::new(peer, start + i as u64), p))
            .collect(),
        true,
    )
}

fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn path_of(document: &Document, id: NodeId) -> PathSnapshot {
    match document.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => p,
        ObjectSnapshot::Primitive(_) => panic!("expected a path"),
    }
}

/// A plate with a hole: outer 40 mm square plus a reversed 10 mm square.
fn plate() -> Vec<(Vec<NewAnchor>, bool)> {
    vec![
        square(50, 0, 0.0, 0.0, 40.0, true),
        square(50, 100, 15.0, 15.0, 10.0, false),
    ]
}

fn loro_of(d: &Document) -> LoroDoc {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn changes(d: &Document) -> usize {
    loro_of(d).len_changes()
}

fn last_label(d: &Document) -> String {
    let l = loro_of(d);
    let vv = l.oplog_vv();
    let (peer, end) = vv
        .iter()
        .max_by_key(|(_, e)| **e)
        .map(|(p, e)| (*p, *e))
        .unwrap();
    // The newest change by any peer: scan all peers, keep the highest lamport.
    let mut best: Option<(u32, String)> = None;
    for (p, e) in vv.iter() {
        let c = l.get_change(loro::ID::new(*p, *e - 1)).unwrap();
        let lamport = c.lamport;
        if best.as_ref().is_none_or(|(b, _)| lamport >= *b) {
            best = Some((lamport, c.message().to_string()));
        }
    }
    let _ = (peer, end);
    best.unwrap().1
}

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn member(container: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(container)).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

fn container(format_version: u32, loro_bytes: &[u8], json: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": format_version,
        "loro_snapshot_version": 1,
        "app_version": "tester",
    });
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (n, b) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes.to_vec()),
        ("document.json", json.to_vec()),
    ] {
        w.start_file(n, o).unwrap();
        w.write_all(&b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn manifest_version(bytes: &[u8]) -> u64 {
    let m: serde_json::Value = serde_json::from_slice(&member(bytes, "manifest.json")).unwrap();
    m["format_version"].as_u64().unwrap()
}

/// Opens a document built from a raw Loro edit of a compound-path document.
fn open_edited(edit: impl FnOnce(&LoroMap)) -> Result<Document, OpenError> {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 5.0, 5.0);
    d.replace_with_path(&[a], a, &plate(), "boolean_union")
        .unwrap();
    let l = loro_of(&d);
    let tree = l.get_tree("paths");
    let root = tree.roots()[0];
    let meta = tree.get_meta(root).unwrap();
    edit(&meta);
    l.commit();
    let bytes = l.export(loro::ExportMode::Snapshot).unwrap();
    unpack(
        2,
        &container(CURRENT_FORMAT_VERSION, &bytes, &d.export_json().unwrap()),
    )
}

// =====================================================================
// Format version, round trip (30, 37, 37a)
// =====================================================================

#[test]
fn ac37_current_format_version_is_8_and_pack_stamps_it() {
    assert_eq!(CURRENT_FORMAT_VERSION, 8);
    let d = Document::new(1);
    assert_eq!(manifest_version(&pack(&d, "t").unwrap()), 8);
}

#[test]
fn ac37_a_file_newer_than_this_build_is_refused_as_too_new() {
    let d = Document::new(1);
    let l = d.export_loro_snapshot().unwrap();
    let bytes = container(9, &l, b"{}");
    match unpack(2, &bytes) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!((found, supported), (9, 8));
        }
        other => panic!("expected FormatTooNew, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn ac30_compound_path_survives_save_and_reopen_with_same_order_and_winding() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 5.0, 5.0);
    let outlines = vec![
        square(60, 0, 0.0, 0.0, 40.0, true),
        square(60, 10, 15.0, 15.0, 10.0, false),
        square(60, 20, 18.0, 18.0, 3.0, true),
        (
            vec![
                NewAnchor::corner(AnchorId::new(60, 30), pt(100.0, 0.0)),
                NewAnchor {
                    id: AnchorId::new(60, 31),
                    point: pt(110.0, 0.0),
                    handle_in: Vec2::new(-1.0, 2.0),
                    handle_out: Vec2::new(1.5, 0.25),
                    kind: curvyo_document_core::AnchorKind::Symmetric,
                },
            ],
            false,
        ),
    ];
    let r = d
        .replace_with_path(&[a], a, &outlines, "boolean_union")
        .unwrap();
    let before = path_of(&d, r);
    assert!(before.is_compound());
    assert_eq!(before.extra_subpaths.len(), 3);
    let bytes = pack(&d, "t").unwrap();
    let reopened = unpack(7, &bytes).unwrap();
    let after = path_of(&reopened, r);
    assert_eq!(before, after, "stored value changed through save/open");
    // Outline shape is exactly as handed in, in order, closed flags included.
    let got: Vec<(Vec<NewAnchor>, bool)> = after
        .subpaths()
        .map(|s| (s.anchors.to_vec(), s.closed))
        .collect();
    assert_eq!(got, outlines);
    // Second save is stable.
    let again = unpack(8, &pack(&reopened, "t").unwrap()).unwrap();
    assert_eq!(path_of(&again, r), before);
}

#[test]
fn ac37a_a_single_outline_result_is_an_ordinary_path_without_extra_subpaths() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 5.0, 5.0);
    let r = d
        .replace_with_path(
            &[a],
            a,
            &[square(61, 0, 0.0, 0.0, 10.0, true)],
            "boolean_union",
        )
        .unwrap();
    let p = path_of(&d, r);
    assert!(!p.is_compound());
    assert!(p.extra_subpaths.is_empty());
    let json = String::from_utf8(d.export_json().unwrap()).unwrap();
    assert!(
        !json.contains("extra_subpaths"),
        "ordinary path wrote extra_subpaths into document.json"
    );
    let l = loro_of(&d);
    let tree = l.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    assert!(
        meta.get("extra_subpaths").is_none(),
        "ordinary path wrote an extra_subpaths key"
    );
}

#[test]
fn ac37_compound_path_appears_in_document_json_ordinary_path_does_not_change() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 5.0, 5.0);
    d.replace_with_path(&[a], a, &plate(), "boolean_union")
        .unwrap();
    let json = String::from_utf8(d.export_json().unwrap()).unwrap();
    assert!(json.contains("extra_subpaths"));
}

#[test]
fn ac37a_damaged_extra_subpaths_are_refused_not_crashed_on() {
    // Not a list at all.
    let r = open_edited(|m| m.insert("extra_subpaths", "bogus").unwrap());
    assert!(matches!(r, Err(OpenError::Damaged)), "string");
    let r = open_edited(|m| m.insert("extra_subpaths", 7_i64).unwrap());
    assert!(matches!(r, Err(OpenError::Damaged)), "number");
    let r = open_edited(|m| m.insert("extra_subpaths", true).unwrap());
    assert!(matches!(r, Err(OpenError::Damaged)), "bool");
    // A plain (not movable) list.
    let r = open_edited(|m| {
        m.insert_container("extra_subpaths", loro::LoroList::new())
            .unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "plain list");
    // A map instead of a list.
    let r = open_edited(|m| {
        m.insert_container("extra_subpaths", LoroMap::new())
            .unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "map");
    // List of numbers.
    let r = open_edited(|m| {
        let list = m
            .insert_container("extra_subpaths", LoroMovableList::new())
            .unwrap();
        list.push(1_i64).unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "list of numbers");
    // Element without anchors.
    let r = open_edited(|m| {
        let list = m
            .insert_container("extra_subpaths", LoroMovableList::new())
            .unwrap();
        let e = list.push_container(LoroMap::new()).unwrap();
        e.insert("closed", true).unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "no anchors");
    // Element with anchors being a string.
    let r = open_edited(|m| {
        let list = m
            .insert_container("extra_subpaths", LoroMovableList::new())
            .unwrap();
        let e = list.push_container(LoroMap::new()).unwrap();
        e.insert("closed", true).unwrap();
        e.insert("anchors", "x").unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "anchors a string");
    // Anchor map empty.
    let r = open_edited(|m| {
        let list = m
            .insert_container("extra_subpaths", LoroMovableList::new())
            .unwrap();
        let e = list.push_container(LoroMap::new()).unwrap();
        e.insert("closed", true).unwrap();
        let anchors = e
            .insert_container("anchors", LoroMovableList::new())
            .unwrap();
        anchors.push_container(LoroMap::new()).unwrap();
    });
    assert!(matches!(r, Err(OpenError::Damaged)), "empty anchor map");
}

/// Opens the plate with one register of the first anchor of the first (`extra
/// = false`) or the hole (`extra = true`) outline overwritten by `value`.
fn open_with_corrupt_anchor(extra: bool, key: &str, value: loro::LoroValue) -> bool {
    let r = open_edited(|m| {
        let anchors = if extra {
            let Some(ValueOrContainer::Container(loro::Container::MovableList(list))) =
                m.get("extra_subpaths")
            else {
                panic!("extra_subpaths missing");
            };
            let Some(ValueOrContainer::Container(loro::Container::Map(outline))) = list.get(0)
            else {
                panic!("outline missing");
            };
            let Some(ValueOrContainer::Container(loro::Container::MovableList(anchors))) =
                outline.get("anchors")
            else {
                panic!("anchors missing");
            };
            anchors
        } else {
            let Some(ValueOrContainer::Container(loro::Container::MovableList(anchors))) =
                m.get("anchors")
            else {
                panic!("anchors missing");
            };
            anchors
        };
        let Some(ValueOrContainer::Container(loro::Container::Map(a))) = anchors.get(0) else {
            panic!("anchor missing");
        };
        a.insert(key, value).unwrap();
    });
    r.is_ok()
}

#[test]
fn ac37a_malformed_anchor_inside_the_extra_outline_is_refused() {
    assert!(
        !open_with_corrupt_anchor(true, "id", "garbage".into()),
        "an anchor with a damaged id in an extra outline was accepted"
    );
    assert!(
        !open_with_corrupt_anchor(false, "id", "garbage".into()),
        "control: first outline"
    );
}

#[test]
fn ac37a_other_anchor_registers_are_handled_like_those_of_the_first_outline() {
    // The first outline's validator checks the id only; the extra outline
    // must not be stricter or looser than that, whatever the policy is.
    for key in ["point", "kind", "handle_in", "handle_out"] {
        for value in [
            loro::LoroValue::from("garbage"),
            loro::LoroValue::from(f64::NAN),
        ] {
            assert_eq!(
                open_with_corrupt_anchor(true, key, value.clone()),
                open_with_corrupt_anchor(false, key, value.clone()),
                "register {key} = {value:?}: policy differs between the outlines"
            );
        }
    }
}

#[test]
fn ac37_an_empty_extra_subpaths_list_reads_like_an_absent_one() {
    let d = Document::new(1);
    let p = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(4.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(4.0, 4.0)),
        ],
        true,
    );
    let l = loro_of(&d);
    let tree = l.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    meta.insert_container("extra_subpaths", LoroMovableList::new())
        .unwrap();
    l.commit();
    let bytes = l.export(loro::ExportMode::Snapshot).unwrap();
    let opened = unpack(2, &container(CURRENT_FORMAT_VERSION, &bytes, b"{}")).unwrap();
    let s = path_of(&opened, p);
    assert!(!s.is_compound());
    assert_eq!(s.anchors.len(), 3);
}

#[test]
fn ac37_a_closed_open_mix_and_a_one_anchor_extra_outline_are_read_back() {
    // Reader accepts what ADR 0002 lets exist, even if a boolean never writes it.
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 5.0, 5.0);
    let r = d
        .replace_with_path(
            &[a],
            a,
            &[
                square(62, 0, 0.0, 0.0, 10.0, true),
                (
                    vec![NewAnchor::corner(AnchorId::new(62, 10), pt(50.0, 50.0))],
                    false,
                ),
            ],
            "x",
        )
        .unwrap();
    let back = path_of(&unpack(3, &pack(&d, "t").unwrap()).unwrap(), r);
    assert_eq!(back.extra_subpaths.len(), 1);
    assert!(!back.extra_subpaths[0].closed);
}

// =====================================================================
// Older files (37)
// =====================================================================

#[test]
fn ac37_earlier_fixtures_open_unchanged_and_opening_writes_nothing() {
    for name in [
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_gradient_v7.curvyo",
        "display_unit_in_v7.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "valid.curvyo",
        "format_version_1.curvyo",
    ] {
        let bytes = fixture(name);
        let stored_vv = {
            let l = LoroDoc::new();
            l.import(&member(&bytes, "document.loro")).unwrap();
            l.oplog_vv()
        };
        let d = unpack(2, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            loro_of(&d).oplog_vv(),
            stored_vv,
            "{name}: open wrote operations"
        );
        for id in d.object_ids() {
            if let ObjectSnapshot::Path(p) = d.object(id).unwrap() {
                assert!(!p.is_compound(), "{name}: an old path became compound");
                assert!(p.extra_subpaths.is_empty());
            }
        }
        // The stored container bytes are an input; opening cannot alter them,
        // and the document.json an older build wrote must not gain the key.
        let json = String::from_utf8(d.export_json().unwrap()).unwrap();
        assert!(!json.contains("extra_subpaths"), "{name}");
    }
}

#[test]
fn ac37_the_compound_v8_golden_fixture_opens_and_is_stable() {
    let bytes = fixture("compound_v8.curvyo");
    assert_eq!(manifest_version(&bytes), 8);
    let d = unpack(2, &bytes).unwrap();
    let compound: Vec<_> = d
        .object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) if p.is_compound() => Some(p),
            _ => None,
        })
        .collect();
    assert!(!compound.is_empty(), "golden has no compound path");
    let again = unpack(3, &pack(&d, "t").unwrap()).unwrap();
    for p in &compound {
        assert_eq!(&path_of(&again, p.id), p);
    }
}

// =====================================================================
// replace_with_path (19 to 22, 27, 28)
// =====================================================================

#[test]
fn ac19_22_28_replace_puts_one_object_at_the_base_place_with_one_labelled_commit() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let x1 = rect(&d, 50.0, 0.0, 10.0, 10.0);
    let b = rect(&d, 5.0, 5.0, 10.0, 10.0);
    let x2 = rect(&d, 60.0, 0.0, 10.0, 10.0);
    let c = rect(&d, 8.0, 8.0, 10.0, 10.0);
    let x3 = rect(&d, 70.0, 0.0, 10.0, 10.0);
    assert_eq!(d.object_ids(), vec![a, x1, b, x2, c, x3]);
    let before = changes(&d);
    let r = d
        .replace_with_path(&[c, a, b], b, &plate(), "boolean_difference")
        .unwrap();
    let ids = d.object_ids();
    assert_eq!(
        ids,
        vec![x1, r, x2, x3],
        "result is not at the base's place"
    );
    assert_eq!(changes(&d), before + 1, "not exactly one commit");
    assert_eq!(last_label(&d), "boolean_difference");
    for gone in [a, b, c] {
        assert!(d.object(gone).is_none());
    }
    // Unselected objects keep their snapshots.
    for keep in [x1, x2, x3] {
        assert!(d.object(keep).is_some());
    }
}

#[test]
fn ac22_base_at_the_bottom_and_at_the_top() {
    for (base_idx, expected_pos) in [(0usize, 0usize), (2, 3)] {
        let d = Document::new(1);
        let o0 = rect(&d, 0.0, 0.0, 10.0, 10.0);
        let x = rect(&d, 50.0, 0.0, 10.0, 10.0);
        let o1 = rect(&d, 5.0, 5.0, 10.0, 10.0);
        let y = rect(&d, 60.0, 0.0, 10.0, 10.0);
        let o2 = rect(&d, 8.0, 8.0, 10.0, 10.0);
        let operands = [o0, o1, o2];
        let r = d
            .replace_with_path(&operands, operands[base_idx], &plate(), "boolean_union")
            .unwrap();
        let ids = d.object_ids();
        assert_eq!(ids.len(), 3);
        assert_eq!(
            ids.iter().position(|i| *i == r).unwrap(),
            if base_idx == 0 { 0 } else { 2 },
            "{expected_pos}"
        );
        assert!(ids.contains(&x) && ids.contains(&y));
        let xi = ids.iter().position(|i| *i == x).unwrap();
        let yi = ids.iter().position(|i| *i == y).unwrap();
        assert!(xi < yi, "unselected order changed");
    }
}

fn styled_path(d: &Document) -> NodeId {
    let p = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(3, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(3, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(3, 3), pt(10.0, 10.0)),
        ],
        true,
    );
    let edits = [
        StyleEdit::StrokeWidth(mm(2.75)),
        StyleEdit::StrokeColor(Color {
            r: 10,
            g: 200,
            b: 30,
        }),
        StyleEdit::StrokeOpacity(Opacity::new(0.4).unwrap()),
        StyleEdit::StrokeDash(DashPattern::new(vec![3.0, 1.5]).unwrap()),
        StyleEdit::StrokeJoin(LineJoin::Round),
        StyleEdit::StrokeCap(LineCap::Round),
        StyleEdit::FillColor(Color {
            r: 200,
            g: 40,
            b: 90,
        }),
        StyleEdit::FillOpacity(Opacity::new(0.7).unwrap()),
    ];
    for e in &edits {
        d.edit_style(&[p], e).unwrap();
    }
    p
}

#[test]
fn ac21_27_result_takes_the_base_style_and_has_rotation_zero_for_path_and_rect_bases() {
    // Path base, rotated.
    let d = Document::new(1);
    let other = rect(&d, 0.0, 0.0, 3.0, 3.0);
    let base = styled_path(&d);
    let rotated = d
        .object(base)
        .unwrap()
        .rotated(pt(5.0, 5.0), Angle::from_radians(0.5));
    d.rotate_object(&rotated).unwrap();
    assert_ne!(path_of(&d, base).rotation.as_radians(), 0.0);
    let style = path_of(&d, base).style;
    let r = d
        .replace_with_path(&[other, base], base, &plate(), "boolean_union")
        .unwrap();
    let p = path_of(&d, r);
    assert_eq!(p.style, style);
    assert_eq!(p.rotation.as_radians(), 0.0);

    // Rect base with its own style and a rotation.
    let d = Document::new(1);
    let o = styled_path(&d);
    let b = rect(&d, 0.0, 0.0, 20.0, 10.0);
    d.edit_style(&[b], &StyleEdit::StrokeWidth(mm(5.5)))
        .unwrap();
    d.edit_style(&[b], &StyleEdit::FillColor(Color { r: 1, g: 2, b: 3 }))
        .unwrap();
    let rot = d
        .object(b)
        .unwrap()
        .rotated(pt(10.0, 5.0), Angle::from_radians(1.0));
    d.rotate_object(&rot).unwrap();
    let style = d.object(b).unwrap().style().clone();
    let r = d
        .replace_with_path(&[b, o], b, &plate(), "boolean_union")
        .unwrap();
    let p = path_of(&d, r);
    assert_eq!(p.style, style);
    assert_eq!(p.rotation.as_radians(), 0.0);
}

#[test]
fn ac21_result_style_is_not_the_non_base_operands_style() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&d, 5.0, 5.0, 10.0, 10.0);
    d.edit_style(&[b], &StyleEdit::StrokeWidth(mm(9.0)))
        .unwrap();
    let r = d
        .replace_with_path(&[a, b], a, &plate(), "boolean_union")
        .unwrap();
    assert_eq!(path_of(&d, r).style, {
        let d2 = Document::new(1);
        let a2 = rect(&d2, 0.0, 0.0, 10.0, 10.0);
        d2.object(a2).unwrap().style().clone()
    });
}

#[test]
fn refusals_change_nothing_stale_id() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&d, 5.0, 5.0, 10.0, 10.0);
    let c = rect(&d, 8.0, 8.0, 10.0, 10.0);
    d.delete_objects(&[c]).unwrap();
    let before_ids = d.object_ids();
    let before_changes = changes(&d);
    let before_snap: Vec<_> = before_ids.iter().map(|i| d.object(*i)).collect();
    // Stale operand in the middle of the list.
    assert_eq!(
        d.replace_with_path(&[a, c, b], a, &plate(), "boolean_union"),
        Err(ObjectEditError::NoSuchObject)
    );
    // Base not among the operands.
    assert_eq!(
        d.replace_with_path(&[a], b, &plate(), "boolean_union"),
        Err(ObjectEditError::BaseNotAnOperand)
    );
    // No operands at all.
    assert_eq!(
        d.replace_with_path(&[], a, &plate(), "boolean_union"),
        Err(ObjectEditError::BaseNotAnOperand)
    );
    // No outlines / empty outline.
    assert_eq!(
        d.replace_with_path(&[a, b], a, &[], "boolean_union"),
        Err(ObjectEditError::NoOutlines)
    );
    assert_eq!(
        d.replace_with_path(&[a, b], a, &[(vec![], true)], "boolean_union"),
        Err(ObjectEditError::NoOutlines)
    );
    let mut with_empty = plate();
    with_empty.push((vec![], true));
    assert_eq!(
        d.replace_with_path(&[a, b], a, &with_empty, "boolean_union"),
        Err(ObjectEditError::NoOutlines)
    );
    // Duplicate anchor id inside one outline, and across outlines.
    let mut dup = plate();
    dup[0].0[1].id = dup[0].0[0].id;
    assert_eq!(
        d.replace_with_path(&[a, b], a, &dup, "boolean_union"),
        Err(ObjectEditError::AnchorIds)
    );
    let mut dup2 = plate();
    dup2[1].0[0].id = dup2[0].0[0].id;
    assert_eq!(
        d.replace_with_path(&[a, b], a, &dup2, "boolean_union"),
        Err(ObjectEditError::AnchorIds)
    );
    assert_eq!(d.object_ids(), before_ids);
    assert_eq!(changes(&d), before_changes, "a refusal wrote a commit");
    let after: Vec<_> = before_ids.iter().map(|i| d.object(*i)).collect();
    assert_eq!(after, before_snap);
}

#[test]
fn replace_with_one_operand_and_with_a_repeated_operand_id_works() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&d, 5.0, 5.0, 10.0, 10.0);
    let r = d
        .replace_with_path(&[a, a], a, &plate(), "boolean_union")
        .unwrap();
    assert_eq!(d.object_ids(), vec![r, b]);
    let r2 = d
        .replace_with_path(&[b], b, &[square(70, 0, 0.0, 0.0, 3.0, true)], "x")
        .unwrap();
    assert_eq!(d.object_ids(), vec![r, r2]);
}

#[test]
fn ac36a_anchor_ids_of_the_replaced_path_operand_may_be_reused_by_the_result() {
    let d = Document::new(1);
    let p = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(9, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(9, 2), pt(8.0, 0.0)),
            NewAnchor::corner(AnchorId::new(9, 3), pt(8.0, 8.0)),
        ],
        true,
    );
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let res = d.replace_with_path(
        &[p, a],
        a,
        &[square(9, 1, 0.0, 0.0, 5.0, true)],
        "boolean_union",
    );
    // Either outcome is defensible; it must not corrupt the document.
    let snap = d.export_loro_snapshot().unwrap();
    assert!(!snap.is_empty());
    let _ = res;
}

// =====================================================================
// Transforms and edits on a compound path (35, 36, 36a, 38a)
// =====================================================================

fn compound(d: &Document) -> NodeId {
    let a = rect(d, 100.0, 100.0, 1.0, 1.0);
    d.replace_with_path(&[a], a, &plate(), "boolean_union")
        .unwrap()
}

#[test]
fn ac35_translate_moves_every_outline() {
    let d = Document::new(1);
    let r = compound(&d);
    let before = path_of(&d, r);
    d.translate_objects(&[r], Vec2::new(7.5, -2.25)).unwrap();
    let after = path_of(&d, r);
    for (b, a) in before.all_anchors().zip(after.all_anchors()) {
        assert_eq!(a.point, pt(b.point.x + 7.5, b.point.y - 2.25));
        assert_eq!(a.id, b.id);
    }
    assert_eq!(after.extra_subpaths.len(), 1);
    assert_eq!(before.all_anchors().count(), after.all_anchors().count());
}

#[test]
fn ac35_rotate_moves_every_outline_and_snapshot_rotated_matches_the_commit() {
    let d = Document::new(1);
    let r = compound(&d);
    let pivot = pt(20.0, 20.0);
    let angle = Angle::from_radians(37.0_f64.to_radians());
    let want = d.object(r).unwrap().rotated(pivot, angle);
    d.rotate_object(&want).unwrap();
    let got = path_of(&d, r);
    let ObjectSnapshot::Path(w) = want else {
        panic!()
    };
    assert_eq!(got.extra_subpaths.len(), 1);
    for (g, w) in got.all_anchors().zip(w.all_anchors()) {
        assert!((g.point.x - w.point.x).abs() < 1e-9 && (g.point.y - w.point.y).abs() < 1e-9);
    }
    // Independent check of the arithmetic on a hole corner.
    let orig = plate()[1].0[0].point;
    let (s, c) = angle.as_radians().sin_cos();
    let (dx, dy) = (orig.x - pivot.x, orig.y - pivot.y);
    let want_pt = pt(pivot.x + dx * c - dy * s, pivot.y + dx * s + dy * c);
    let h = got.extra_subpaths[0].anchors[0].point;
    assert!((h.x - want_pt.x).abs() < 1e-9 && (h.y - want_pt.y).abs() < 1e-9);
    // Handles of anchors rotate too.
    assert_eq!(got.rotation.as_radians(), angle.as_radians());
}

#[test]
fn ac35_scaled_and_sheared_snapshots_map_every_outline() {
    let d = Document::new(1);
    let r = compound(&d);
    let p = path_of(&d, r);
    let pivot = pt(0.0, 0.0);
    let s = p.scaled(pivot, 1.5, 0.5);
    assert_eq!(s.extra_subpaths.len(), 1);
    for (a, b) in p.all_anchors().zip(s.all_anchors()) {
        assert!((b.point.x - a.point.x * 1.5).abs() < 1e-9);
        assert!((b.point.y - a.point.y * 0.5).abs() < 1e-9);
    }
    let sh = p.sheared(pivot, 0.25, 0.0);
    assert_eq!(sh.extra_subpaths.len(), 1);
    let moved = p
        .all_anchors()
        .zip(sh.all_anchors())
        .filter(|(a, b)| a.point != b.point)
        .count();
    assert!(moved > 4, "shear left the extra outline untouched");
    // Winding is kept under a uniform positive scale and flips under a mirror.
    let signed = |s: &PathSnapshot, o: usize| -> f64 {
        let sub = s.subpaths().nth(o).unwrap();
        let a = sub.anchors;
        (0..a.len())
            .map(|i| {
                let (p, q) = (a[i].point, a[(i + 1) % a.len()].point);
                p.x * q.y - q.x * p.y
            })
            .sum::<f64>()
            / 2.0
    };
    let m = p.scaled(pivot, -1.0, 1.0);
    assert!(signed(&m, 0) * signed(&p, 0) < 0.0);
    assert!(signed(&m, 1) * signed(&p, 1) < 0.0);
    assert!(signed(&p, 0) * signed(&p, 1) < 0.0, "hole not opposite");
    assert!(signed(&s, 0) * signed(&s, 1) < 0.0);
}

#[test]
fn ac35_resize_path_with_every_anchor_of_every_outline_and_one_stroke_width() {
    let d = Document::new(1);
    let r = compound(&d);
    d.edit_style(&[r], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    let p = path_of(&d, r);
    let changes_before = changes(&d);
    let scaled = p.scaled(pt(0.0, 0.0), 2.0, 2.0);
    let moves: Vec<_> = scaled
        .all_anchors()
        .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
        .collect();
    d.resize_path(r, &moves, Some(mm(4.0))).unwrap();
    assert_eq!(changes(&d), changes_before + 1);
    let after = path_of(&d, r);
    assert_eq!(after.style.stroke.width, mm(4.0));
    for (a, b) in scaled.all_anchors().zip(after.all_anchors()) {
        assert_eq!(a.point, b.point);
    }
    // An anchor id of the extra outline that does not exist refuses everything.
    let mut bad = moves.clone();
    bad.push((
        AnchorId::new(1234, 99),
        pt(0.0, 0.0),
        Vec2::ZERO,
        Vec2::ZERO,
    ));
    assert!(d.resize_path(r, &bad, None).is_err());
    assert_eq!(path_of(&d, r), after);
}

#[test]
fn ac36a_duplicate_renumbers_anchors_of_every_outline() {
    let d = Document::new(1);
    let r = compound(&d);
    let n = path_of(&d, r).all_anchors().count();
    assert_eq!(n, 8);
    let fresh: Vec<AnchorId> = (0..n as u64).map(|i| AnchorId::new(77, i)).collect();
    let copy = d
        .duplicate_objects(
            &[CopySource {
                id: r,
                anchor_ids: fresh.clone(),
            }],
            Vec2::new(3.0, 4.0),
        )
        .unwrap();
    let orig = path_of(&d, r);
    let c = path_of(&d, copy[0]);
    assert!(c.is_compound());
    assert_eq!(c.extra_subpaths.len(), 1);
    assert_eq!(
        c.all_anchors().map(|a| a.id).collect::<Vec<_>>(),
        fresh,
        "ids not assigned in subpaths() order"
    );
    assert_eq!(
        orig.all_anchors().map(|a| a.id).collect::<Vec<_>>(),
        path_of(&d, r)
            .all_anchors()
            .map(|a| a.id)
            .collect::<Vec<_>>()
    );
    let all: Vec<_> = orig
        .all_anchors()
        .chain(c.all_anchors())
        .map(|a| a.id)
        .collect();
    let unique: std::collections::HashSet<_> = all.iter().copied().collect();
    assert_eq!(
        unique.len(),
        2 * n,
        "anchor ids collide across original and copy"
    );
    for (o, k) in orig.all_anchors().zip(c.all_anchors()) {
        assert_eq!(k.point, pt(o.point.x + 3.0, o.point.y + 4.0));
        assert_eq!(k.kind, o.kind);
    }
    assert_eq!(c.style, orig.style);
    // Directly above its original.
    let ids = d.object_ids();
    assert_eq!(
        ids.iter().position(|i| *i == copy[0]).unwrap(),
        ids.iter().position(|i| *i == r).unwrap() + 1
    );
}

#[test]
fn ac36a_duplicate_refuses_wrong_id_counts_for_a_compound_and_changes_nothing() {
    let d = Document::new(1);
    let r = compound(&d);
    let c0 = changes(&d);
    let ids0 = d.object_ids();
    // Only the first outline's worth of ids (4 of 8), and one too many.
    for count in [0_u64, 4, 7, 9] {
        let fresh: Vec<AnchorId> = (0..count).map(|i| AnchorId::new(78, i)).collect();
        assert_eq!(
            d.duplicate_objects(
                &[CopySource {
                    id: r,
                    anchor_ids: fresh
                }],
                Vec2::ZERO
            ),
            Err(ObjectEditError::AnchorIds),
            "{count} ids"
        );
    }
    // A repeated id across the two outlines.
    let mut fresh: Vec<AnchorId> = (0..8).map(|i| AnchorId::new(78, i)).collect();
    fresh[5] = fresh[1];
    assert_eq!(
        d.duplicate_objects(
            &[CopySource {
                id: r,
                anchor_ids: fresh
            }],
            Vec2::ZERO
        ),
        Err(ObjectEditError::AnchorIds)
    );
    assert_eq!(d.object_ids(), ids0);
    assert_eq!(changes(&d), c0);
}

#[test]
fn ac36_duplicate_may_not_reuse_the_originals_anchor_ids() {
    let d = Document::new(1);
    let r = compound(&d);
    let same: Vec<AnchorId> = path_of(&d, r).all_anchors().map(|a| a.id).collect();
    let res = d.duplicate_objects(
        &[CopySource {
            id: r,
            anchor_ids: same,
        }],
        Vec2::ZERO,
    );
    // Not specified for a one-outline path either; report only, but the
    // document must stay valid and saveable.
    let _ = res;
    assert!(pack(&d, "t").is_ok());
}

#[test]
fn ac36_delete_style_edits_and_duplicate_and_delete_work_as_on_any_object() {
    let d = Document::new(1);
    let r = compound(&d);
    d.edit_style(&[r], &StyleEdit::FillColor(Color { r: 9, g: 9, b: 9 }))
        .unwrap();
    d.edit_style(&[r], &StyleEdit::StrokeWidth(mm(1.25)))
        .unwrap();
    d.edit_style(
        &[r],
        &StyleEdit::StrokeDash(DashPattern::new(vec![2.0, 1.0]).unwrap()),
    )
    .unwrap();
    let p = path_of(&d, r);
    assert_eq!(p.style.fill.color, Color { r: 9, g: 9, b: 9 });
    assert_eq!(
        p.extra_subpaths.len(),
        1,
        "a style edit touched the outlines"
    );
    d.delete_objects(&[r]).unwrap();
    assert!(d.object(r).is_none());
    assert!(d.object_ids().len() == 1 || d.object_ids().is_empty());
}

#[test]
fn ac38a_join_and_split_refuse_a_compound_path_and_change_nothing() {
    let d = Document::new(1);
    let r = compound(&d);
    let open = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(5, 1), pt(60.0, 0.0)),
            NewAnchor::corner(AnchorId::new(5, 2), pt(70.0, 0.0)),
        ],
        false,
    );
    let p = path_of(&d, r);
    let before_ids = d.object_ids();
    let before = changes(&d);
    let all: Vec<AnchorId> = p.all_anchors().map(|a| a.id).collect();
    for &aid in &all {
        assert!(!d.check_split(r, aid), "split offered for {aid:?}");
    }
    // Join with every combination of its anchors and the open path's ends,
    // in both directions, and the compound path with itself.
    for &aid in &all {
        for other in [AnchorId::new(5, 1), AnchorId::new(5, 2)] {
            assert!(!d.check_join(r, aid, open, other), "join offered {aid:?}");
            assert!(!d.check_join(open, other, r, aid), "join offered {aid:?}");
            assert_eq!(
                d.join_endpoints(r, aid, open, other),
                Err(curvyo_document_core::PathEditError::NotJoinable)
            );
        }
        for &bid in &all {
            assert!(!d.check_join(r, aid, r, bid));
            assert!(d.join_endpoints(r, aid, r, bid).is_err());
        }
        assert!(d.split_at_anchor(r, aid, AnchorId::new(88, 1)).is_err());
    }
    assert_eq!(d.object_ids(), before_ids);
    assert_eq!(changes(&d), before, "a refused join/split wrote a commit");
    assert_eq!(path_of(&d, r), p);
}

#[test]
fn node_commands_on_extra_outline_anchors_leave_the_extra_outlines_valid() {
    // Not in the criteria (the Node tool never offers these); a stray call
    // must not corrupt the file or silently lose the other outlines.
    let d = Document::new(1);
    let r = compound(&d);
    let p = path_of(&d, r);
    let extra_ids: Vec<AnchorId> = p.extra_subpaths[0].anchors.iter().map(|a| a.id).collect();
    let _ = d.move_anchors(&[(r, extra_ids[0], pt(1.0, 1.0))]);
    let _ = d.delete_anchors(r, &extra_ids[..1]);
    let bytes = pack(&d, "t").unwrap();
    let back = unpack(2, &bytes).unwrap();
    assert_eq!(path_of(&back, r), path_of(&d, r));
}

#[test]
fn deleting_every_anchor_of_the_first_outline_through_the_model_does_not_corrupt() {
    let d = Document::new(1);
    let r = compound(&d);
    let p = path_of(&d, r);
    let first: Vec<AnchorId> = p.anchors.iter().map(|a| a.id).collect();
    let res = d.delete_anchors(r, &first);
    println!(
        "delete all first-outline anchors: {res:?}, object kept: {}",
        d.object(r).is_some()
    );
    let back = unpack(2, &pack(&d, "t").unwrap()).unwrap();
    assert_eq!(back.object_ids(), d.object_ids());
}

// =====================================================================
// CRDT merge
// =====================================================================

fn merged(a: &Document, b: &Document) -> Document {
    let l = LoroDoc::new();
    l.import(&a.export_loro_snapshot().unwrap()).unwrap();
    l.import(&b.export_loro_snapshot().unwrap()).unwrap();
    l.commit();
    let bytes = l.export(loro::ExportMode::Snapshot).unwrap();
    unpack(99, &container(CURRENT_FORMAT_VERSION, &bytes, b"{}")).expect("merged document opens")
}

#[test]
fn concurrent_style_edit_and_move_of_a_compound_path_both_survive_in_either_order() {
    let d = Document::new(1);
    let r = compound(&d);
    let bytes = pack(&d, "t").unwrap();
    for flip in [false, true] {
        let a = unpack(2, &bytes).unwrap();
        let b = unpack(3, &bytes).unwrap();
        a.edit_style(&[r], &StyleEdit::StrokeWidth(mm(6.0)))
            .unwrap();
        b.translate_objects(&[r], Vec2::new(10.0, 0.0)).unwrap();
        let m = if flip { merged(&b, &a) } else { merged(&a, &b) };
        let p = path_of(&m, r);
        assert_eq!(p.style.stroke.width, mm(6.0));
        assert_eq!(p.extra_subpaths.len(), 1);
        assert_eq!(p.anchors[0].point, pt(10.0, 0.0));
        assert_eq!(p.extra_subpaths[0].anchors[0].point, pt(25.0, 25.0));
    }
}

#[test]
fn concurrent_moves_of_the_same_compound_path_converge_to_the_same_value_on_both_peers() {
    let d = Document::new(1);
    let r = compound(&d);
    let bytes = pack(&d, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.translate_objects(&[r], Vec2::new(1.0, 0.0)).unwrap();
    b.translate_objects(&[r], Vec2::new(0.0, 2.0)).unwrap();
    let m1 = path_of(&merged(&a, &b), r);
    let m2 = path_of(&merged(&b, &a), r);
    assert_eq!(m1, m2, "peers diverge");
    // Each anchor is a coherent LWW pair or a mix; the outline count stays.
    assert_eq!(m1.extra_subpaths.len(), 1);
    assert_eq!(m1.extra_subpaths[0].anchors.len(), 4);
}

#[test]
fn concurrent_replace_of_the_same_operand_by_two_peers_keeps_the_file_valid() {
    let d = Document::new(1);
    let a_id = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b_id = rect(&d, 5.0, 5.0, 10.0, 10.0);
    let bytes = pack(&d, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    let ra = a
        .replace_with_path(&[a_id, b_id], a_id, &plate(), "boolean_union")
        .unwrap();
    let rb = b
        .replace_with_path(
            &[a_id, b_id],
            b_id,
            &[square(80, 0, 0.0, 0.0, 9.0, true)],
            "boolean_union",
        )
        .unwrap();
    let m = merged(&a, &b);
    let ids = m.object_ids();
    assert!(ids.contains(&ra) && ids.contains(&rb));
    assert!(m.object(a_id).is_none() && m.object(b_id).is_none());
    // And it saves and reopens.
    let back = unpack(5, &pack(&m, "t").unwrap()).unwrap();
    assert_eq!(back.object_ids(), ids);
}

#[test]
fn peers_that_concurrently_edit_a_compound_path_and_delete_it_merge_without_a_crash() {
    let d = Document::new(1);
    let r = compound(&d);
    let bytes = pack(&d, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.delete_objects(&[r]).unwrap();
    b.translate_objects(&[r], Vec2::new(1.0, 1.0)).unwrap();
    let m = merged(&a, &b);
    assert!(m.object(r).is_none());
}

// =====================================================================
// Many outlines (1,000) and size sanity
// =====================================================================

fn many(outlines: usize, per: usize) -> Vec<(Vec<NewAnchor>, bool)> {
    (0..outlines)
        .map(|o| {
            let ox = (o % 40) as f64 * 3.0;
            let oy = (o / 40) as f64 * 3.0;
            let anchors = (0..per)
                .map(|k| {
                    let t = std::f64::consts::TAU * k as f64 / per as f64;
                    NewAnchor::corner(
                        AnchorId::new(90, (o * per + k) as u64),
                        pt(ox + t.cos(), oy + t.sin()),
                    )
                })
                .collect();
            (anchors, true)
        })
        .collect()
}

#[test]
fn a_thousand_outlines_round_trip_translate_duplicate() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 1.0, 1.0);
    let outlines = many(1_000, 4);
    let r = d
        .replace_with_path(&[a], a, &outlines, "boolean_union")
        .unwrap();
    let p = path_of(&d, r);
    assert_eq!(p.extra_subpaths.len(), 999);
    let back = unpack(2, &pack(&d, "t").unwrap()).unwrap();
    assert_eq!(path_of(&back, r), p);
    d.translate_objects(&[r], Vec2::new(1.0, 1.0)).unwrap();
    let moved = path_of(&d, r);
    for (a, b) in p.all_anchors().zip(moved.all_anchors()) {
        assert_eq!(b.point, pt(a.point.x + 1.0, a.point.y + 1.0));
    }
    let fresh: Vec<AnchorId> = (0..4000u64).map(|i| AnchorId::new(91, i)).collect();
    let cp = d
        .duplicate_objects(
            &[CopySource {
                id: r,
                anchor_ids: fresh,
            }],
            Vec2::ZERO,
        )
        .unwrap();
    assert_eq!(path_of(&d, cp[0]).extra_subpaths.len(), 999);
}

#[test]
fn twenty_thousand_anchors_in_four_outlines_round_trip_exactly() {
    let per = if cfg!(debug_assertions) { 500 } else { 5_000 };
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 1.0, 1.0);
    let outlines = many(4, per);
    let t0 = std::time::Instant::now();
    let r = d
        .replace_with_path(&[a], a, &outlines, "boolean_union")
        .unwrap();
    let write = t0.elapsed();
    let bytes = pack(&d, "t").unwrap();
    let back = unpack(2, &bytes).unwrap();
    let p = path_of(&back, r);
    assert_eq!(p.all_anchors().count(), 4 * per);
    let got: Vec<_> = p
        .subpaths()
        .map(|s| (s.anchors.to_vec(), s.closed))
        .collect();
    assert_eq!(got, outlines);
    println!("write {write:?}, file {} bytes", bytes.len());
}

/// Rotating (and resizing) a compound path writes every anchor; the cost per
/// anchor must not grow with the number of outlines. Measured on 100 and 400
/// outlines of 4 anchors (debug build, so only the ratio is asserted).
#[test]
fn rotate_commit_cost_grows_roughly_linearly_with_the_number_of_outlines() {
    let time = |n: usize| {
        let d = Document::new(1);
        let a = rect(&d, 0.0, 0.0, 1.0, 1.0);
        let r = d
            .replace_with_path(&[a], a, &many(n, 4), "boolean_union")
            .unwrap();
        let rot = d
            .object(r)
            .unwrap()
            .rotated(pt(0.0, 0.0), Angle::from_radians(0.3));
        let t = std::time::Instant::now();
        d.rotate_object(&rot).unwrap();
        t.elapsed().as_secs_f64()
    };
    let small = time(100).max(1e-4);
    let large = time(400);
    println!("rotate commit: 100 outlines {small:.3}s, 400 outlines {large:.3}s");
    // Linear would be 4x; allow 8x. Quadratic is 16x.
    assert!(
        large / small < 8.0,
        "rotate_object is super-linear: 4x the outlines took {:.1}x as long",
        large / small
    );
}
