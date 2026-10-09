//! Independent tester tests for PR 1 (model) of
//! `specs/0015-document-size-and-rulers` (criteria 12, 13, 17-19, 24-27a,
//! 33, 35, 36, 38, 39 at model level). Expected values are computed from the
//! spec's arithmetic here, never read back from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::too_many_lines)]

use std::io::{Cursor, Write};
use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, CURRENT_FORMAT_VERSION, DisplayUnit, Document, DocumentSize,
    DocumentSizeError, EllipseFrame, InnerRatio, Length, MAX_DOCUMENT_MM, MIN_DOCUMENT_MM,
    NewAnchor, NodeId, ObjectSnapshot, Point, PointCount, RectBounds, StarFrame, Vec2,
    outline_of_rotated, pack, unpack, validated_document_side,
};
use loro::LoroDoc;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn near(a: f64, b: f64, e: f64) -> bool {
    (a - b).abs() <= e
}

fn size(w: f64, h: f64) -> DocumentSize {
    DocumentSize::from_mm(w, h)
}

/// Commit labels of a document, oldest first, as seen after a snapshot round
/// trip through a fresh Loro document.
fn labels(d: &Document) -> Vec<String> {
    let l = LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let ids: Vec<loro::ID> = l.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String)> = Vec::new();
    l.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|s| s.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, s)| s).collect()
}

/// Flat description of one object: absolute outline points, handles and the
/// rotation register.
#[derive(Debug, Clone, PartialEq)]
struct Sig {
    id: NodeId,
    points: Vec<(f64, f64)>,
    handles: Vec<(f64, f64, f64, f64)>,
    rotation: f64,
}

fn sig_of(d: &Document, id: NodeId) -> Sig {
    match d.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => Sig {
            id,
            points: p.anchors.iter().map(|a| (a.point.x, a.point.y)).collect(),
            handles: p
                .anchors
                .iter()
                .map(|a| (a.handle_in.x, a.handle_in.y, a.handle_out.x, a.handle_out.y))
                .collect(),
            rotation: p.rotation.as_radians(),
        },
        ObjectSnapshot::Primitive(p) => {
            let o = outline_of_rotated(&p.shape, p.rotation);
            Sig {
                id,
                points: o.iter().map(|a| (a.point.x, a.point.y)).collect(),
                handles: o
                    .iter()
                    .map(|a| (a.handle_in.x, a.handle_in.y, a.handle_out.x, a.handle_out.y))
                    .collect(),
                rotation: p.rotation.as_radians(),
            }
        }
    }
}

fn sigs(d: &Document) -> Vec<Sig> {
    d.object_ids().into_iter().map(|id| sig_of(d, id)).collect()
}

fn assert_shifted(before: &[Sig], after: &[Sig], dx: f64, dy: f64, tol: f64) {
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(after) {
        assert_eq!(b.id, a.id);
        assert_eq!(b.points.len(), a.points.len(), "anchor count of {:?}", b.id);
        for (p, q) in b.points.iter().zip(&a.points) {
            assert!(near(p.0 + dx, q.0, tol), "x {:?} {p:?} -> {q:?}", b.id);
            assert!(near(p.1 + dy, q.1, tol), "y {:?} {p:?} -> {q:?}", b.id);
        }
        for (h, g) in b.handles.iter().zip(&a.handles) {
            assert!(near(h.0, g.0, tol) && near(h.1, g.1, tol), "handle in");
            assert!(near(h.2, g.2, tol) && near(h.3, g.3, tol), "handle out");
        }
        assert!(near(b.rotation, a.rotation, 1e-12), "rotation changed");
    }
}

fn rotate(d: &Document, id: NodeId, about: Point, rad: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(rad)))
        .unwrap();
}

fn curvy_path(d: &Document) -> NodeId {
    d.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(0.0, 40.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(-20.0, -10.0))
            },
            NewAnchor {
                handle_in: Vec2::new(-15.0, 5.0),
                handle_out: Vec2::new(15.0, -5.0),
                kind: AnchorKind::Symmetric,
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(60.0, 70.0))
            },
            NewAnchor::corner(AnchorId::new(1, 3), pt(130.0, 20.0)),
        ],
        true,
    )
}

/// One of every kind, some rotated, some off the page.
fn populated() -> Document {
    let d = Document::new(1);
    let r = d.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: mm(50.0),
        height: mm(50.0),
    });
    let r2 = d.create_rect(RectBounds {
        origin: pt(-80.0, 500.0),
        width: mm(20.0),
        height: mm(5.0),
    });
    let e = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 120.0),
        rx: mm(30.0),
        ry: mm(12.0),
    });
    let pg = d.create_polygon(
        StarFrame {
            center: pt(150.0, 40.0),
            radius: mm(25.0),
            angle: Angle::from_radians(0.3),
        },
        PointCount::new(5).unwrap(),
    );
    let st = d.create_star(
        StarFrame {
            center: pt(40.0, 200.0),
            radius: mm(35.0),
            angle: Angle::from_radians(-0.5),
        },
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let p = curvy_path(&d);
    rotate(&d, r, pt(20.0, 20.0), 0.7);
    rotate(&d, e, pt(100.0, 120.0), 1.1);
    rotate(&d, st, pt(0.0, 0.0), -2.0);
    rotate(&d, p, pt(50.0, 50.0), 0.25);
    let _ = (r2, pg);
    d
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

fn reopen(d: &Document) -> Document {
    unpack(5, &pack(d, "tester").unwrap()).unwrap()
}

// ---------------------------------------------------------------- defaults

/// AC 12, 33, 36: a new project is A4 portrait in mm.
#[test]
fn a_new_document_is_a4_portrait_in_mm() {
    let d = Document::new(1);
    assert_eq!(d.size(), size(210.0, 297.0));
    assert_eq!(d.display_unit(), DisplayUnit::Mm);
    assert!(labels(&d).iter().all(|l| l != "resize_document"));
}

/// AC 35: 1 in is exactly 25.4 mm; 8.5 x 11 in is 215.9 x 279.4 mm.
#[test]
fn inch_conversion_is_exact_and_round_trips() {
    assert_eq!(DisplayUnit::In.mm_per_unit(), 25.4);
    assert_eq!(DisplayUnit::Cm.mm_per_unit(), 10.0);
    assert_eq!(DisplayUnit::Mm.mm_per_unit(), 1.0);
    assert_eq!(Length::from_unit(1.0, DisplayUnit::In).as_mm(), 25.4);
    assert!(near(
        Length::from_unit(8.5, DisplayUnit::In).as_mm(),
        215.9,
        1e-12
    ));
    assert!(near(
        Length::from_unit(11.0, DisplayUnit::In).as_mm(),
        279.4,
        1e-12
    ));
    assert_eq!(Length::from_unit(3.0, DisplayUnit::Mm).as_mm(), 3.0);
    assert_eq!(Length::from_unit(2.5, DisplayUnit::Cm).as_mm(), 25.0);
    for unit in DisplayUnit::ALL {
        for v in [0.0, 1.0, 0.04, 3937.0, 123.456, -7.25, 1e5, 1e-9] {
            let back = Length::from_unit(v, unit).in_unit(unit);
            assert!(near(back, v, 1e-12 * v.abs().max(1.0)), "{unit:?} {v}");
        }
        assert_eq!(DisplayUnit::from_symbol(unit.symbol()), Some(unit));
    }
    assert_eq!(DisplayUnit::ALL.len(), 3);
    assert_eq!(DisplayUnit::Mm.symbol(), "mm");
    assert_eq!(DisplayUnit::Cm.symbol(), "cm");
    assert_eq!(DisplayUnit::In.symbol(), "in");
}

#[test]
fn unknown_unit_symbols_are_not_units() {
    for s in [
        "", "MM", "m", "px", " mm", "mm ", "inch", "\"", "pt", "in2", "mm\0",
    ] {
        assert_eq!(DisplayUnit::from_symbol(s), None, "{s:?}");
    }
}

#[test]
fn unit_conversion_propagates_non_finite_values_without_panicking() {
    assert!(
        Length::from_unit(f64::NAN, DisplayUnit::In)
            .as_mm()
            .is_nan()
    );
    assert!(
        Length::from_unit(f64::INFINITY, DisplayUnit::Cm)
            .as_mm()
            .is_infinite()
    );
}

// ------------------------------------------------------------------ limits

#[test]
fn limits_are_one_mm_to_one_hundred_thousand_mm() {
    assert_eq!(MIN_DOCUMENT_MM, 1.0);
    assert_eq!(MAX_DOCUMENT_MM, 100_000.0);
}

/// AC 16: limits with a 1e-9 mm tolerance; non-finite refused.
#[test]
fn validated_side_accepts_the_range_and_clamps_within_one_nanometre() {
    assert_eq!(validated_document_side(mm(1.0)).unwrap().as_mm(), 1.0);
    assert_eq!(
        validated_document_side(mm(100_000.0)).unwrap().as_mm(),
        100_000.0
    );
    assert_eq!(validated_document_side(mm(210.0)).unwrap().as_mm(), 210.0);
    assert_eq!(
        validated_document_side(mm(1.0 - 5e-10)).unwrap().as_mm(),
        1.0
    );
    assert_eq!(
        validated_document_side(mm(100_000.0 + 5e-10))
            .unwrap()
            .as_mm(),
        100_000.0
    );
    for bad in [
        0.0,
        -1.0,
        0.999_999,
        1.0 - 1e-6,
        100_000.001,
        1e9,
        f64::MAX,
        f64::MIN_POSITIVE,
        -0.0,
    ] {
        assert_eq!(
            validated_document_side(mm(bad)),
            Err(DocumentSizeError::OutOfRange),
            "{bad}"
        );
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            validated_document_side(mm(bad)),
            Err(DocumentSizeError::NotFinite)
        );
    }
}

// ------------------------------------------------------------------ resize

/// AC 17 worked example: rect at (10, 10), 50 x 50, 210x297 -> 300x400.
#[test]
fn resize_example_from_the_spec() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: mm(50.0),
        height: mm(50.0),
    });
    assert_eq!(d.resize(size(300.0, 400.0)), Ok(true));
    assert_eq!(d.size(), size(300.0, 400.0));
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let curvyo_document_core::Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(near(bounds.origin.x, 55.0, 1e-9));
    assert!(near(bounds.origin.y, 61.5, 1e-9));
    assert_eq!(bounds.width.as_mm(), 50.0);
    assert_eq!(bounds.height.as_mm(), 50.0);
}

/// AC 17, 18: every kind, rotated or not, on and off the page, shifts by half
/// the size change; nothing else changes.
#[test]
fn resize_shifts_every_object_kind_by_half_the_size_change() {
    let d = populated();
    let before = sigs(&d);
    let styles: Vec<_> = d
        .object_ids()
        .into_iter()
        .map(|i| d.object(i).unwrap().style().clone())
        .collect();
    assert_eq!(d.resize(size(300.0, 400.0)), Ok(true));
    assert_shifted(&before, &sigs(&d), 45.0, 51.5, 1e-9);
    let after: Vec<_> = d
        .object_ids()
        .into_iter()
        .map(|i| d.object(i).unwrap().style().clone())
        .collect();
    assert_eq!(styles, after);
}

/// AC 17: shrinking moves objects the other way, including onto the
/// pasteboard; nothing is cropped or deleted (AC 18).
#[test]
fn shrinking_below_the_content_moves_objects_but_keeps_them_all() {
    let d = populated();
    let before = sigs(&d);
    assert_eq!(d.resize(size(20.0, 5.0)), Ok(true));
    assert_eq!(d.size(), size(20.0, 5.0));
    assert_shifted(&before, &sigs(&d), -95.0, -146.0, 1e-9);
    assert_eq!(d.object_ids().len(), before.len());
}

/// AC 17: the centre of the document and the objects relative to it are kept
/// at every one of a sweep of sizes, and going there and back is lossless.
#[test]
fn resize_there_and_back_restores_positions() {
    let d = populated();
    let before = sigs(&d);
    for (w, h) in [
        (1.0, 1.0),
        (100_000.0, 100_000.0),
        (1.0, 100_000.0),
        (333.333, 0.7_f64.max(1.0)),
        (210.0, 297.0),
    ] {
        d.resize(size(w, h)).unwrap();
        let s = d.size();
        let (dx, dy) = (
            (s.width.as_mm() - 210.0) / 2.0,
            (s.height.as_mm() - 297.0) / 2.0,
        );
        assert_shifted(&before, &sigs(&d), dx, dy, 1e-7);
    }
    d.resize(size(210.0, 297.0)).unwrap();
    assert_shifted(&before, &sigs(&d), 0.0, 0.0, 1e-7);
}

/// AC 17: one axis changes, the other stays: only that axis shifts.
#[test]
fn resize_of_one_axis_shifts_only_that_axis() {
    let d = populated();
    let before = sigs(&d);
    assert_eq!(d.resize(size(210.0, 400.0)), Ok(true));
    assert_shifted(&before, &sigs(&d), 0.0, 51.5, 1e-9);
}

/// AC 19: one commit, label `resize_document`.
#[test]
fn resize_is_one_commit_labelled_resize_document() {
    let d = populated();
    let before = labels(&d);
    d.resize(size(300.0, 400.0)).unwrap();
    let after = labels(&d);
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    assert_eq!(after.last().unwrap(), "resize_document");
    assert_eq!(&after[..before.len()], &before[..]);
}

/// AC 19: a size equal to the current one writes nothing.
#[test]
fn resize_to_the_same_size_writes_nothing() {
    let d = populated();
    let before_labels = labels(&d);
    let before_sigs = sigs(&d);
    assert_eq!(d.resize(size(210.0, 297.0)), Ok(false));
    assert_eq!(d.resize(size(210.0 + 1e-10, 297.0 - 1e-10)), Ok(false));
    assert_eq!(labels(&d), before_labels);
    assert_eq!(sigs(&d), before_sigs);
    assert_eq!(d.size(), size(210.0, 297.0));
}

/// AC 16/19: any refused size changes nothing: no commit, no object moved,
/// and a half-valid pair (one good, one bad side) is refused as a whole.
#[test]
fn refused_sizes_write_nothing() {
    let d = populated();
    let before_labels = labels(&d);
    let before_sigs = sigs(&d);
    let cases = [
        (size(0.0, 100.0), DocumentSizeError::OutOfRange),
        (size(100.0, 0.5), DocumentSizeError::OutOfRange),
        (size(100_000.1, 100.0), DocumentSizeError::OutOfRange),
        (size(300.0, 1e12), DocumentSizeError::OutOfRange),
        (size(-5.0, 300.0), DocumentSizeError::OutOfRange),
        (size(f64::NAN, 300.0), DocumentSizeError::NotFinite),
        (size(300.0, f64::INFINITY), DocumentSizeError::NotFinite),
        (
            size(f64::NEG_INFINITY, f64::NAN),
            DocumentSizeError::NotFinite,
        ),
    ];
    for (s, expected) in cases {
        assert_eq!(d.resize(s), Err(expected), "{s:?}");
        assert_eq!(d.size(), size(210.0, 297.0), "{s:?}");
        assert_eq!(labels(&d), before_labels, "{s:?}");
        assert_eq!(sigs(&d), before_sigs, "{s:?}");
    }
}

/// AC 16: a side within 1e-9 mm of a limit is accepted and clamped.
#[test]
fn resize_accepts_and_clamps_sides_at_the_limits() {
    let d = Document::new(1);
    assert_eq!(d.resize(size(1.0 - 5e-10, 100_000.0 + 5e-10)), Ok(true));
    assert_eq!(d.size(), size(1.0, 100_000.0));
    assert_eq!(d.resize(size(100_000.0, 1.0)), Ok(true));
    assert_eq!(d.size(), size(100_000.0, 1.0));
}

/// AC 17 with a huge size and a far object: precision stays within 1e-9 mm
/// relative to the shift's own magnitude.
#[test]
fn resize_at_the_largest_size_keeps_a_far_object_exact() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(-99_000.0, 99_000.0),
        width: mm(2.0),
        height: mm(3.0),
    });
    let before = sig_of(&d, id);
    d.resize(size(100_000.0, 100_000.0)).unwrap();
    let after = sig_of(&d, id);
    let dx = (100_000.0 - 210.0) / 2.0;
    let dy = (100_000.0 - 297.0) / 2.0;
    assert_shifted(&[before], &[after], dx, dy, 1e-9);
}

/// AC 19: resize of an empty document still changes the size in one commit.
#[test]
fn resize_of_an_empty_document_changes_only_the_size() {
    let d = Document::new(1);
    assert_eq!(d.resize(size(50.0, 60.0)), Ok(true));
    assert_eq!(d.size(), size(50.0, 60.0));
    assert_eq!(labels(&d).last().unwrap(), "resize_document");
}

/// AC 39 (model): resize, pack, unpack keeps size and positions.
#[test]
fn resize_survives_save_and_open() {
    let d = populated();
    d.resize(size(412.5, 100.25)).unwrap();
    let e = reopen(&d);
    assert_eq!(e.size(), size(412.5, 100.25));
    assert_eq!(sigs(&e), sigs(&d));
    assert_eq!(e.display_unit(), DisplayUnit::Mm);
}

/// AC 19 "however the operation ends": a resize on a document that has other
/// pending state commits only its own label (no stray commit merged in).
#[test]
fn resize_does_not_absorb_neighbouring_commits() {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(1.0),
        height: mm(1.0),
    });
    d.resize(size(100.0, 100.0)).unwrap();
    let l = labels(&d);
    assert_eq!(l.last().unwrap(), "resize_document");
    assert_eq!(l.iter().filter(|s| *s == "resize_document").count(), 1);
    assert!(l.iter().any(|s| s == "create_rect"));
}

// -------------------------------------------------------------------- fit

/// AC 24: top-left of the box moves to (0,0); size is the box size.
#[test]
fn fit_sets_the_size_to_the_box_and_moves_its_corner_to_the_origin() {
    let d = populated();
    let before = sigs(&d);
    assert_eq!(
        d.fit_to_content((pt(-80.0, -10.0), pt(150.0, 525.0))),
        Ok(true)
    );
    assert_eq!(d.size(), size(230.0, 535.0));
    assert_shifted(&before, &sigs(&d), 80.0, 10.0, 1e-9);
}

/// AC 25: a flat box (one horizontal line, 100 x 0) fits to 100 x 1 and the
/// line sits at y = 0.5.
#[test]
fn fit_of_a_flat_box_centres_on_the_one_millimetre_minimum() {
    let d = Document::new(1);
    let id = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 0.0)),
        ],
        false,
    );
    assert_eq!(d.fit_to_content((pt(0.0, 0.0), pt(100.0, 0.0))), Ok(true));
    assert_eq!(d.size(), size(100.0, 1.0));
    let s = sig_of(&d, id);
    assert!(near(s.points[0].1, 0.5, 1e-12));
    assert!(near(s.points[1].1, 0.5, 1e-12));
    assert!(near(s.points[0].0, 0.0, 1e-12));
}

/// AC 25 symmetric: a vertical line off to the side gets the width centred.
#[test]
fn fit_of_a_vertical_line_centres_on_x() {
    let d = Document::new(1);
    let id = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(33.0, 5.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(33.0, 55.0)),
        ],
        false,
    );
    assert_eq!(d.fit_to_content((pt(33.0, 5.0), pt(33.0, 55.0))), Ok(true));
    assert_eq!(d.size(), size(1.0, 50.0));
    let s = sig_of(&d, id);
    assert!(near(s.points[0].0, 0.5, 1e-12));
    assert!(near(s.points[0].1, 0.0, 1e-12));
}

/// AC 25: a box of 0.4 mm on both axes (a dot) becomes 1 x 1, centred.
#[test]
fn fit_of_a_tiny_box_is_one_by_one() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(7.0, 9.0),
        width: mm(0.4),
        height: mm(0.4),
    });
    d.fit_to_content((pt(7.0, 9.0), pt(7.4, 9.4))).unwrap();
    assert_eq!(d.size(), size(1.0, 1.0));
    let s = sig_of(&d, id);
    let (minx, maxx) = s
        .points
        .iter()
        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    assert!(
        near(minx, 0.3, 1e-9) && near(maxx, 0.7, 1e-9),
        "{minx} {maxx}"
    );
}

/// AC 26: applying the same fit twice is a no-op the second time.
#[test]
fn fit_is_idempotent_and_the_second_call_commits_nothing() {
    let d = populated();
    let bx = (pt(-80.0, -10.0), pt(150.0, 525.0));
    assert_eq!(d.fit_to_content(bx), Ok(true));
    let l = labels(&d);
    let s = sigs(&d);
    let bx2 = (pt(0.0, 0.0), pt(230.0, 535.0));
    assert_eq!(d.fit_to_content(bx2), Ok(false));
    assert_eq!(labels(&d), l);
    assert_eq!(sigs(&d), s);
    assert_eq!(d.size(), size(230.0, 535.0));
}

/// AC 26: also idempotent for a flat box (the 1 mm minimum case).
#[test]
fn fit_of_a_flat_box_is_idempotent() {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 0.0)),
        ],
        false,
    );
    d.fit_to_content((pt(0.0, 0.0), pt(100.0, 0.0))).unwrap();
    let l = labels(&d);
    // After the first fit the line is at y = 0.5, the box is (0,0.5)-(100,0.5).
    assert_eq!(d.fit_to_content((pt(0.0, 0.5), pt(100.0, 0.5))), Ok(false));
    assert_eq!(labels(&d), l);
}

/// AC 27: one commit labelled `fit_document_to_content`.
#[test]
fn fit_is_one_commit_labelled_fit_document_to_content() {
    let d = populated();
    let before = labels(&d);
    d.fit_to_content((pt(-80.0, -10.0), pt(150.0, 525.0)))
        .unwrap();
    let after = labels(&d);
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    assert_eq!(after.last().unwrap(), "fit_document_to_content");
}

/// AC 22: no objects -> Ok(false) and nothing written, whatever the bounds.
#[test]
fn fit_on_an_empty_document_writes_nothing() {
    let d = Document::new(1);
    let l = labels(&d);
    let r = d.fit_to_content((pt(5.0, 5.0), pt(50.0, 50.0)));
    assert_eq!(r, Ok(false));
    assert_eq!(d.size(), size(210.0, 297.0));
    assert_eq!(labels(&d), l);
}

/// AC 27a: a box larger than 100 000 mm on an axis is refused untouched.
#[test]
fn fit_refuses_content_larger_than_the_largest_document() {
    let d = populated();
    let l = labels(&d);
    let s = sigs(&d);
    for bx in [
        (pt(0.0, 0.0), pt(100_000.001, 10.0)),
        (pt(0.0, 0.0), pt(10.0, 100_000.001)),
        (pt(-60_000.0, 0.0), pt(60_000.0, 10.0)),
        (pt(0.0, 0.0), pt(1e12, 1e12)),
    ] {
        assert_eq!(d.fit_to_content(bx), Err(DocumentSizeError::OutOfRange));
        assert_eq!(d.size(), size(210.0, 297.0));
        assert_eq!(labels(&d), l);
        assert_eq!(sigs(&d), s);
    }
}

/// AC 27a boundary: exactly 100 000 is allowed.
#[test]
fn fit_accepts_content_of_exactly_the_largest_size() {
    let d = populated();
    assert_eq!(
        d.fit_to_content((pt(-40_000.0, 0.0), pt(60_000.0, 100_000.0))),
        Ok(true)
    );
    assert_eq!(d.size(), size(100_000.0, 100_000.0));
}

#[test]
fn fit_refuses_non_finite_and_reversed_boxes() {
    let d = populated();
    let l = labels(&d);
    assert_eq!(
        d.fit_to_content((pt(f64::NAN, 0.0), pt(1.0, 1.0))),
        Err(DocumentSizeError::NotFinite)
    );
    assert_eq!(
        d.fit_to_content((pt(0.0, 0.0), pt(f64::INFINITY, 1.0))),
        Err(DocumentSizeError::NotFinite)
    );
    assert_eq!(
        d.fit_to_content((pt(10.0, 0.0), pt(5.0, 1.0))),
        Err(DocumentSizeError::InvalidBounds)
    );
    assert_eq!(
        d.fit_to_content((pt(0.0, 10.0), pt(5.0, 1.0))),
        Err(DocumentSizeError::InvalidBounds)
    );
    assert_eq!(labels(&d), l);
    assert_eq!(d.size(), size(210.0, 297.0));
}

/// AC 24: when the box already is at the origin with the right size, but the
/// document size differs, only the size changes (no object moves).
#[test]
fn fit_with_the_box_already_at_the_origin_only_changes_the_size() {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(40.0),
        height: mm(30.0),
    });
    let before = sig_of(&d, id);
    assert_eq!(d.fit_to_content((pt(0.0, 0.0), pt(40.0, 30.0))), Ok(true));
    assert_eq!(d.size(), size(40.0, 30.0));
    assert_eq!(sig_of(&d, id), before);
}

/// AC 39 (model): fit, pack, unpack.
#[test]
fn fit_survives_save_and_open() {
    let d = populated();
    d.fit_to_content((pt(-80.0, -10.0), pt(150.0, 525.0)))
        .unwrap();
    let e = reopen(&d);
    assert_eq!(e.size(), d.size());
    assert_eq!(sigs(&e), sigs(&d));
}

// ----------------------------------------------------------- display unit

/// AC 38: the unit change is one commit `set_display_unit`, moves nothing,
/// same unit writes nothing.
#[test]
fn set_display_unit_is_one_labelled_commit_and_moves_nothing() {
    let d = populated();
    let s = sigs(&d);
    let size0 = d.size();
    let l0 = labels(&d);
    assert!(d.set_display_unit(DisplayUnit::In));
    assert_eq!(d.display_unit(), DisplayUnit::In);
    let l1 = labels(&d);
    assert_eq!(l1.len(), l0.len() + 1);
    assert_eq!(l1.last().unwrap(), "set_display_unit");
    assert_eq!(sigs(&d), s);
    assert_eq!(d.size(), size0);
    assert!(!d.set_display_unit(DisplayUnit::In));
    assert_eq!(labels(&d), l1);
    // back to mm is a change (the default is stored explicitly or by removal)
    assert!(d.set_display_unit(DisplayUnit::Mm));
    assert_eq!(d.display_unit(), DisplayUnit::Mm);
    assert!(!d.set_display_unit(DisplayUnit::Mm));
    assert!(d.set_display_unit(DisplayUnit::Cm));
    assert_eq!(d.display_unit(), DisplayUnit::Cm);
}

/// A new document asked for mm writes nothing.
#[test]
fn setting_the_default_unit_on_a_new_document_writes_nothing() {
    let d = Document::new(1);
    let l = labels(&d);
    assert!(!d.set_display_unit(DisplayUnit::Mm));
    assert_eq!(labels(&d), l);
}

/// AC 36: the unit round-trips through save and open, for every unit.
#[test]
fn display_unit_round_trips_through_the_file() {
    for unit in DisplayUnit::ALL {
        let d = populated();
        let _ = d.set_display_unit(unit);
        d.resize(size(215.9, 279.4)).unwrap();
        let e = reopen(&d);
        assert_eq!(e.display_unit(), unit);
        assert_eq!(e.size(), d.size());
        // twice
        let f = reopen(&e);
        assert_eq!(f.display_unit(), unit);
    }
}

/// AC 35: typing 8.5 x 11 in sets 215.9 x 279.4 mm in the document, and
/// showing it back does not rewrite it.
#[test]
fn inch_size_is_stored_in_millimetres() {
    let d = Document::new(1);
    let _ = d.set_display_unit(DisplayUnit::In);
    d.resize(DocumentSize::new(
        Length::from_unit(8.5, DisplayUnit::In),
        Length::from_unit(11.0, DisplayUnit::In),
    ))
    .unwrap();
    assert!(near(d.size().width.as_mm(), 215.9, 1e-12));
    assert!(near(d.size().height.as_mm(), 279.4, 1e-12));
    let s = d.size();
    // reading and showing does not change anything
    let _ = (
        s.width.in_unit(DisplayUnit::In),
        s.height.in_unit(DisplayUnit::In),
    );
    assert_eq!(d.size(), s);
}

/// AC 38: no `format_version` bump, no snapshot-version bump.
#[test]
fn a_file_with_a_display_unit_is_still_format_version_7() {
    assert_eq!(CURRENT_FORMAT_VERSION, 7);
    assert_eq!(curvyo_document_core::CURRENT_LORO_SNAPSHOT_VERSION, 1);
    let d = Document::new(1);
    let _ = d.set_display_unit(DisplayUnit::In);
    let bytes = pack(&d, "t").unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.json").unwrap(), &mut s).unwrap();
    let m: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(m["format_version"], 7);
    assert_eq!(m["loro_snapshot_version"], 1);
}

// ------------------------------------------------------------------- CRDT

/// AC 38/CRDT: a peer's unit change and another peer's resize both survive.
#[test]
fn concurrent_unit_change_and_resize_both_survive() {
    let base = pack(&populated(), "t").unwrap();
    for (pa, pb) in [(2, 3), (3, 2)] {
        let a = unpack(pa, &base).unwrap();
        let b = unpack(pb, &base).unwrap();
        let _ = a.set_display_unit(DisplayUnit::In);
        b.resize(size(300.0, 400.0)).unwrap();
        let m = merged(&a, &b);
        let m2 = merged(&b, &a);
        assert_eq!(m.display_unit(), DisplayUnit::In);
        assert_eq!(m.size(), size(300.0, 400.0));
        assert_eq!(m2.display_unit(), DisplayUnit::In);
        assert_eq!(m2.size(), size(300.0, 400.0));
        assert_eq!(sigs(&m), sigs(&m2));
        assert_shifted(
            &sigs(&unpack(1, &base).unwrap()),
            &sigs(&m),
            45.0,
            51.5,
            1e-9,
        );
    }
}

/// CRDT: two concurrent unit changes converge to one value on both orders.
#[test]
fn concurrent_unit_changes_converge() {
    let base = pack(&Document::new(1), "t").unwrap();
    let a = unpack(2, &base).unwrap();
    let b = unpack(3, &base).unwrap();
    let _ = a.set_display_unit(DisplayUnit::Cm);
    let _ = b.set_display_unit(DisplayUnit::In);
    let m1 = merged(&a, &b);
    let m2 = merged(&b, &a);
    assert_eq!(m1.display_unit(), m2.display_unit());
    assert!(matches!(
        m1.display_unit(),
        DisplayUnit::Cm | DisplayUnit::In
    ));
}

fn check_concurrent_resizes(cases: &[(DocumentSize, DocumentSize)]) {
    let base = pack(&populated(), "t").unwrap();
    let origin = sigs(&unpack(1, &base).unwrap());
    for &(sa, sb) in cases {
        for (pa, pb) in [(2, 3), (3, 2)] {
            let a = unpack(pa, &base).unwrap();
            let b = unpack(pb, &base).unwrap();
            a.resize(sa).unwrap();
            b.resize(sb).unwrap();
            let m1 = merged(&a, &b);
            let m2 = merged(&b, &a);
            assert_eq!(m1.size(), m2.size(), "order independent {sa:?} {sb:?}");
            assert_eq!(sigs(&m1), sigs(&m2));
            let s = m1.size();
            assert!(s.width.as_mm() >= 1.0 && s.width.as_mm() <= 100_000.0);
            assert!(s.height.as_mm() >= 1.0 && s.height.as_mm() <= 100_000.0);
            // Invariant of the operation: objects are consistent with the size.
            let dx = (s.width.as_mm() - 210.0) / 2.0;
            let dy = (s.height.as_mm() - 297.0) / 2.0;
            assert_shifted(&origin, &sigs(&m1), dx, dy, 1e-7);
        }
    }
}

/// CRDT: two concurrent resizes that both change both axes converge, the size
/// is valid, and one winner decides size and object positions together.
#[test]
fn concurrent_resizes_of_both_axes_converge_to_one_consistent_result() {
    check_concurrent_resizes(&[
        (size(300.0, 400.0), size(250.0, 350.0)),
        (size(100.0, 100.0), size(100_000.0, 1.0)),
    ]);
}

/// CRDT, DEFECT: when two peers resize different axes (A: 300 x 297, B:
/// 210 x 400) the merged size is 300 x 400 (width from A, height from B) but
/// the objects get only one peer's shift per axis, so the drawing moves
/// relative to the document centre (criterion 17 broken after a merge).
#[test]
#[ignore = "defect: concurrent resize of different axes merges to a size the objects do not match"]
fn concurrent_resizes_of_different_axes_keep_the_centre() {
    check_concurrent_resizes(&[(size(300.0, 297.0), size(210.0, 400.0))]);
}

// ------------------------------------------------------------ older files

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// AC 13: opening writes nothing: the number of changes after open equals the
/// number in the file, and a size edit then adds exactly one.
#[test]
fn opening_older_files_adds_no_changes() {
    for name in [
        "valid.curvyo",
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "corner_radii_per_corner.curvyo",
        "styles_v7.curvyo",
        "styles_v7_mergeable_stops.curvyo",
        "size_damaged_v7.curvyo",
        "display_unit_in_v7.curvyo",
    ] {
        let bytes = fixture(name);
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
        let mut raw = Vec::new();
        std::io::Read::read_to_end(&mut z.by_name("document.loro").unwrap(), &mut raw).unwrap();
        let original = LoroDoc::new();
        original.import(&raw).unwrap();
        let d = unpack(4, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let after = LoroDoc::new();
        after.import(&d.export_loro_snapshot().unwrap()).unwrap();
        assert_eq!(
            after.len_changes(),
            original.len_changes(),
            "{name}: opening (and reading size/unit) must not write"
        );
        let _ = (d.size(), d.display_unit());
        let after = LoroDoc::new();
        after.import(&d.export_loro_snapshot().unwrap()).unwrap();
        assert_eq!(after.len_changes(), original.len_changes(), "{name}");
    }
}

/// AC 13: the damaged-size fixture opens (not refused) at A4 on both axes
/// with its objects, and its unit is mm.
#[test]
fn the_damaged_size_fixture_opens_at_a4() {
    let d = unpack(4, &fixture("size_damaged_v7.curvyo")).expect("not refused");
    assert_eq!(d.size(), size(210.0, 297.0));
    assert_eq!(d.display_unit(), DisplayUnit::Mm);
}

/// AC 36/38: the unit fixture reads inches and A4.
#[test]
fn the_display_unit_fixture_reads_inches() {
    let d = unpack(4, &fixture("display_unit_in_v7.curvyo")).unwrap();
    assert_eq!(d.display_unit(), DisplayUnit::In);
}
