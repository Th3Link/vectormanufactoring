//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 1 (style
//! model and format version 7). Written from `specification.md` and the
//! `adrs.md` command contracts before the implementation diff was read.
//!
//! Criteria covered here: 2, 3, 4, 5, 6, 9, 13, 16 (stored side), 17, 18, 19,
//! 20 (command side), 24, 25, 30, 31, 32, 33, plus open-file validation,
//! legacy and future containers, and CRDT merges of the style registers.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::semicolon_if_nothing_returned,
    clippy::assert_is_empty
)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, AnchorKind, CURRENT_FORMAT_VERSION, CURRENT_LORO_SNAPSHOT_VERSION, Color, CopySource,
    CornerRadii, DashPattern, Document, EllipseFrame, FillKind, FillMode, FillModeTarget,
    GradientStop, InnerRatio, Length, LineCap, LineJoin, MAX_GRADIENT_STOPS, NewAnchor, NodeId,
    ObjectSnapshot, Opacity, OpenError, PathEditError, Point, PointCount, RectBounds,
    ShapeEditError, StarFrame, StopChange, StopEdit, StopId, StopPosition, Style, StyleEdit,
    StyleEditError, Vec2, pack, unpack,
};
use loro::{LoroDoc, LoroMap, LoroValue};

// ---------------------------------------------------------------- helpers

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn op(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

fn pos(v: f64) -> StopPosition {
    StopPosition::new(v).unwrap()
}

fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    }
}

fn rect(doc: &Document) -> NodeId {
    doc.create_rect(bounds(0.0, 0.0, 10.0, 6.0))
}

fn ellipse(doc: &Document) -> NodeId {
    doc.create_ellipse(EllipseFrame {
        center: pt(20.0, 20.0),
        rx: mm(5.0),
        ry: mm(3.0),
    })
}

fn polygon(doc: &Document) -> NodeId {
    doc.create_polygon(
        StarFrame {
            center: pt(40.0, 40.0),
            radius: mm(8.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
    )
}

fn star(doc: &Document) -> NodeId {
    doc.create_star(
        StarFrame {
            center: pt(60.0, 40.0),
            radius: mm(8.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        PointCount::new(6).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    )
}

fn anchor(peer: u64, n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor {
        id: AnchorId::new(peer, n),
        point: pt(x, y),
        handle_in: Vec2::new(0.0, 0.0),
        handle_out: Vec2::new(0.0, 0.0),
        kind: AnchorKind::Corner,
    }
}

fn open_path(doc: &Document, peer: u64) -> NodeId {
    doc.create_path(
        &[
            anchor(peer, 1, 0.0, 0.0),
            anchor(peer, 2, 10.0, 0.0),
            anchor(peer, 3, 10.0, 10.0),
            anchor(peer, 4, 0.0, 10.0),
        ],
        false,
    )
}

fn closed_path(doc: &Document, peer: u64) -> NodeId {
    doc.create_path(
        &[
            anchor(peer, 11, 0.0, 0.0),
            anchor(peer, 12, 10.0, 0.0),
            anchor(peer, 13, 10.0, 10.0),
        ],
        true,
    )
}

fn style_of(doc: &Document, id: NodeId) -> Style {
    match doc.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => p.style,
        ObjectSnapshot::Primitive(p) => p.style,
    }
}

fn stop(peer: u64, n: u64, p: f64, color: Color, o: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(peer, n),
        position: pos(p),
        color,
        opacity: op(o),
    }
}

fn target(id: NodeId, seeds: Vec<GradientStop>) -> FillModeTarget {
    FillModeTarget {
        id,
        seed_stops: seeds,
    }
}

fn reopen(doc: &Document) -> Document {
    unpack(77, &pack(doc, "tester").unwrap()).expect("reopens")
}

/// Sum of all version-vector counters: grows by at least one for any write.
fn op_count(doc: &Document) -> i64 {
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    loro.oplog_vv().values().map(|c| i64::from(*c)).sum()
}

/// Loro merges adjacent same-label commits of one peer into one change, so a
/// commit count is only meaningful after a differently labelled commit.
fn separate_label(doc: &Document, id: NodeId) {
    let on = doc.object(id).map(|_| ()).is_some();
    assert!(on);
    let was = style_of(doc, id).fill.enabled;
    let mode = if was { FillMode::None } else { FillMode::Solid };
    doc.set_fill_mode(mode, &[target(id, vec![])]).unwrap();
}

fn change_count(doc: &Document) -> usize {
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    loro.len_changes()
}

fn container_from_loro(format_version: u32, loro_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::json!({
        "format_version": format_version,
        "loro_snapshot_version": CURRENT_LORO_SNAPSHOT_VERSION,
        "app_version": "tester",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes.to_vec()),
        ("document.json", b"{}".to_vec()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn merged(a: &Document, b: &Document) -> Document {
    let loro = LoroDoc::new();
    loro.import(&a.export_loro_snapshot().unwrap()).unwrap();
    loro.import(&b.export_loro_snapshot().unwrap()).unwrap();
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    unpack(99, &container_from_loro(CURRENT_FORMAT_VERSION, &bytes)).expect("merged document opens")
}

/// Runs `mutate` on the meta map of the document's only object and returns a
/// version-7 container built from the result.
fn corrupted(doc: &Document, mutate: impl FnOnce(&LoroMap)) -> Vec<u8> {
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    let tree = loro.get_tree("paths");
    let roots = tree.roots();
    assert_eq!(roots.len(), 1, "helper expects exactly one object");
    let meta = tree.get_meta(roots[0]).unwrap();
    mutate(&meta);
    loro.commit();
    let bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    container_from_loro(CURRENT_FORMAT_VERSION, &bytes)
}

fn ints(values: &[i64]) -> LoroValue {
    LoroValue::from(
        values
            .iter()
            .map(|v| LoroValue::from(*v))
            .collect::<Vec<_>>(),
    )
}

fn floats(values: &[f64]) -> LoroValue {
    LoroValue::from(
        values
            .iter()
            .map(|v| LoroValue::from(*v))
            .collect::<Vec<_>>(),
    )
}

fn push_stop(meta: &LoroMap, id: &str, position: LoroValue, color: LoroValue, opacity: LoroValue) {
    let list = meta.ensure_mergeable_movable_list("fill_stops").unwrap();
    let map = list.push_container(LoroMap::new()).unwrap();
    map.insert("id", id).unwrap();
    map.insert("position", position).unwrap();
    map.insert("color", color).unwrap();
    map.insert("opacity", opacity).unwrap();
}

fn good_stop_id() -> String {
    StopId::new(5, 5).to_hex()
}

/// A fully styled object of every kind, every key off its default.
fn fully_styled(doc: &Document, id: NodeId, peer: u64) {
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(1.75)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeColor(rgb(10, 20, 30)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeOpacity(op(0.37)))
        .unwrap();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 3.0, 1.0, 3.0]).unwrap()),
    )
    .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeJoin(LineJoin::Round))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeCap(LineCap::Square))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::FillColor(rgb(200, 100, 50)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::FillOpacity(op(0.5)))
        .unwrap();
    doc.set_fill_mode(
        FillMode::Radial,
        &[target(
            id,
            vec![
                stop(peer, 1, 0.0, rgb(1, 2, 3), 1.0),
                stop(peer, 2, 0.5, rgb(4, 5, 6), 0.25),
                stop(peer, 3, 0.5, rgb(7, 8, 9), 0.75),
                stop(peer, 4, 1.0, rgb(255, 255, 255), 0.0),
            ],
        )],
    )
    .unwrap();
}

// ------------------------------------------------- AC 3: defaults, legacy

#[test]
fn every_new_object_kind_starts_in_the_frozen_default_style() {
    let doc = Document::new(1);
    let ids = [
        rect(&doc),
        ellipse(&doc),
        polygon(&doc),
        star(&doc),
        open_path(&doc, 1),
        closed_path(&doc, 1),
    ];
    for id in ids {
        let style = style_of(&doc, id);
        assert_eq!(style, Style::default());
        assert!(style.stroke.enabled);
        assert_eq!(style.stroke.width.as_mm(), 0.25);
        assert_eq!(style.stroke.color, Color::BLACK);
        assert_eq!(style.stroke.opacity.get(), 1.0);
        assert!(style.stroke.dash.is_solid());
        assert_eq!(style.stroke.join, LineJoin::Miter);
        assert_eq!(style.stroke.cap, LineCap::Butt);
        assert!(!style.fill.enabled);
        assert_eq!(style.fill.kind, FillKind::Solid);
        assert_eq!(style.fill.color, Color::BLACK);
        assert_eq!(style.fill.opacity.get(), 1.0);
        assert!(style.fill.stops.is_empty());
    }
}

#[test]
fn the_current_format_version_is_seven_and_one_higher_is_refused() {
    assert_eq!(CURRENT_FORMAT_VERSION, 7);
    let doc = Document::new(1);
    let _ = rect(&doc);
    let bytes = container_from_loro(
        CURRENT_FORMAT_VERSION + 1,
        &doc.export_loro_snapshot().unwrap(),
    );
    match unpack(1, &bytes) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!(found, 8);
            assert_eq!(supported, 7);
        }
        other => panic!("expected FormatTooNew, got {:?}", other.err()),
    }
}

#[test]
fn a_u32_max_format_version_is_refused_not_wrapped() {
    let doc = Document::new(1);
    let _ = rect(&doc);
    let bytes = container_from_loro(u32::MAX, &doc.export_loro_snapshot().unwrap());
    assert!(matches!(
        unpack(1, &bytes),
        Err(OpenError::FormatTooNew { .. })
    ));
}

#[test]
fn every_older_fixture_opens_with_every_object_at_the_default_style() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut opened_with_objects = 0;
    for name in [
        "format_version_1.curvyo",
        "paths_v2.curvyo",
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "legacy_corner_radius_v5.curvyo",
        "corner_radii_per_corner.curvyo",
        "valid.curvyo",
    ] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let doc = unpack(5, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        for id in doc.object_ids() {
            opened_with_objects += 1;
            let style = style_of(&doc, id);
            // The stroke width of an old object is whatever was stored;
            // everything else must be a frozen default.
            assert_eq!(style.stroke.opacity.get(), 1.0, "{name}");
            assert!(style.stroke.enabled, "{name}");
            assert!(style.stroke.dash.is_solid(), "{name}");
            assert_eq!(style.stroke.join, LineJoin::Miter, "{name}");
            assert_eq!(style.stroke.cap, LineCap::Butt, "{name}");
            assert!(!style.fill.enabled, "{name}");
            assert_eq!(style.fill.kind, FillKind::Solid, "{name}");
            assert_eq!(style.fill.color, Color::BLACK, "{name}");
            assert_eq!(style.fill.opacity.get(), 1.0, "{name}");
            assert!(style.fill.stops.is_empty(), "{name}");
            assert!(style.stroke.width.as_mm() > 0.0, "{name}");
        }
        // Opening is read-only: a second save/open changes no style.
        let again = reopen(&doc);
        for id in doc.object_ids() {
            assert_eq!(style_of(&doc, id), style_of(&again, id), "{name}");
        }
    }
    assert!(opened_with_objects > 0, "fixtures held no objects at all");
}

#[test]
fn opening_a_legacy_fixture_writes_nothing() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let bytes = std::fs::read(dir.join("primitives_v3.curvyo")).unwrap();
    let a = unpack(5, &bytes).unwrap();
    let before = op_count(&a);
    for id in a.object_ids() {
        let _ = a.object(id);
    }
    assert_eq!(op_count(&a), before);
}

// ------------------------------------------- AC 4, 5: width and on/off

#[test]
fn width_zero_turns_the_stroke_off_and_keeps_the_last_positive_width() {
    let doc = Document::new(1);
    let id = rect(&doc);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(2.5)))
        .unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
        .unwrap();
    let s = style_of(&doc, id);
    assert!(!s.stroke.enabled);
    assert_eq!(s.stroke.width.as_mm(), 2.5);
    // Typing a non-zero width while off switches it on in the same commit.
    separate_label(&doc, id);
    let before = change_count(&doc);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.5)))
        .unwrap();
    assert_eq!(change_count(&doc), before + 1);
    let s = style_of(&doc, id);
    assert!(s.stroke.enabled);
    assert_eq!(s.stroke.width.as_mm(), 0.5);
}

#[test]
fn negative_zero_width_counts_as_zero() {
    let doc = Document::new(1);
    let id = rect(&doc);
    doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(-0.0)))
        .unwrap();
    let s = style_of(&doc, id);
    assert!(!s.stroke.enabled);
    assert_eq!(s.stroke.width.as_mm(), 0.25);
}

#[test]
fn invalid_widths_are_refused_with_no_write() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    let ops = op_count(&doc);
    for bad in [-1.0, -1e-12, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            doc.edit_style(&[a, b], &StyleEdit::StrokeWidth(mm(bad))),
            Err(StyleEditError::InvalidWidth),
            "{bad}"
        );
    }
    assert_eq!(op_count(&doc), ops);
    assert_eq!(style_of(&doc, a), Style::default());
}

#[test]
fn extreme_but_valid_widths_round_trip_exactly() {
    for w in [f64::MIN_POSITIVE, 1e-300, 1e-9, 0.1 + 0.2, 1e6, 1e300] {
        let doc = Document::new(1);
        let id = rect(&doc);
        doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(w)))
            .unwrap();
        let back = reopen(&doc);
        assert_eq!(style_of(&back, id).stroke.width.as_mm(), w);
    }
}

#[test]
fn stroke_off_keeps_every_other_stored_value_and_turning_on_restores_them() {
    let doc = Document::new(1);
    let id = open_path(&doc, 1);
    fully_styled(&doc, id, 1);
    let before = style_of(&doc, id);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let off = style_of(&doc, id);
    assert!(!off.stroke.enabled);
    let mut expected = before.clone();
    expected.stroke.enabled = false;
    assert_eq!(off, expected);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    assert_eq!(style_of(&doc, id), before);
}

#[test]
fn colour_and_opacity_edits_turn_an_off_stroke_on_but_dash_join_cap_do_not() {
    let doc = Document::new(1);
    let id = rect(&doc);
    for edit in [
        StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
        StyleEdit::StrokeJoin(LineJoin::Bevel),
        StyleEdit::StrokeCap(LineCap::Round),
        StyleEdit::FillColor(rgb(1, 2, 3)),
        StyleEdit::FillOpacity(op(0.2)),
    ] {
        doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
            .unwrap();
        doc.edit_style(&[id], &edit).unwrap();
        assert!(!style_of(&doc, id).stroke.enabled, "{edit:?} switched on");
    }
    for edit in [
        StyleEdit::StrokeColor(rgb(9, 9, 9)),
        StyleEdit::StrokeOpacity(op(0.5)),
        StyleEdit::StrokeWidth(mm(3.0)),
    ] {
        doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
            .unwrap();
        doc.edit_style(&[id], &edit).unwrap();
        assert!(style_of(&doc, id).stroke.enabled, "{edit:?} left it off");
    }
}

#[test]
fn colour_edit_on_an_off_stroke_is_one_commit() {
    let doc = Document::new(1);
    let id = rect(&doc);
    doc.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    separate_label(&doc, id);
    let before = change_count(&doc);
    doc.edit_style(&[id], &StyleEdit::StrokeColor(rgb(255, 0, 0)))
        .unwrap();
    assert_eq!(change_count(&doc), before + 1);
}

// ------------------------------------------------ AC 6: colour and alpha

#[test]
fn every_whole_percent_opacity_reads_back_as_exactly_n_over_100() {
    let doc = Document::new(1);
    let id = rect(&doc);
    for n in 0..=100_u32 {
        let value = f64::from(n) / 100.0;
        doc.edit_style(&[id], &StyleEdit::StrokeOpacity(op(value)))
            .unwrap();
        doc.edit_style(&[id], &StyleEdit::FillOpacity(op(value)))
            .unwrap();
        let back = reopen(&doc);
        let s = style_of(&back, id);
        assert_eq!(s.stroke.opacity.get(), value);
        assert_eq!(s.fill.opacity.get(), value);
        assert_eq!((s.stroke.opacity.get() * 100.0).round() as u32, n);
    }
}

#[test]
fn opacity_and_position_constructors_reject_out_of_range_and_non_finite() {
    for bad in [
        -1e-9,
        1.0 + 1e-12,
        2.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(Opacity::new(bad).is_err(), "{bad}");
        assert!(StopPosition::new(bad).is_err(), "{bad}");
    }
    for ok in [0.0, 1.0, 0.5, f64::MIN_POSITIVE] {
        assert!(Opacity::new(ok).is_ok());
        assert!(StopPosition::new(ok).is_ok());
    }
}

#[test]
fn every_colour_channel_value_round_trips() {
    let doc = Document::new(1);
    let id = rect(&doc);
    for (r, g, b) in [(0, 0, 0), (255, 255, 255), (1, 254, 128), (255, 0, 0)] {
        doc.edit_style(&[id], &StyleEdit::StrokeColor(rgb(r, g, b)))
            .unwrap();
        doc.edit_style(&[id], &StyleEdit::FillColor(rgb(b, r, g)))
            .unwrap();
        let s = style_of(&reopen(&doc), id);
        assert_eq!(s.stroke.color, rgb(r, g, b));
        assert_eq!(s.fill.color, rgb(b, r, g));
    }
}

// --------------------------------------------------- AC 9: dash pattern

#[test]
fn dash_pattern_validation_edges() {
    assert!(DashPattern::new(vec![]).unwrap().is_solid());
    assert!(DashPattern::new(vec![6.0, 4.0]).is_ok());
    assert!(DashPattern::new(vec![0.0, 1.0]).is_ok());
    assert!(DashPattern::new(vec![1.0, 0.0]).is_ok());
    for bad in [
        vec![1.0],
        vec![1.0, 2.0, 3.0],
        vec![0.0, 0.0],
        vec![-1.0, 2.0],
        vec![1.0, -0.5],
        vec![f64::NAN, 1.0],
        vec![1.0, f64::NAN],
        vec![f64::INFINITY, 1.0],
        vec![1.0, f64::NEG_INFINITY],
    ] {
        assert!(DashPattern::new(bad.clone()).is_err(), "{bad:?}");
    }
}

#[test]
fn a_very_long_dash_list_is_stored_and_read_back_unchanged() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let list: Vec<f64> = (0..20_000).map(|i| f64::from(i % 7) + 0.5).collect();
    doc.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(list.clone()).unwrap()),
    )
    .unwrap();
    assert_eq!(
        style_of(&reopen(&doc), id).stroke.dash.as_slice(),
        list.as_slice()
    );
}

#[test]
fn a_width_edit_never_rewrites_the_dash_list() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let dash = DashPattern::new(vec![6.0, 4.0]).unwrap();
    doc.edit_style(&[id], &StyleEdit::StrokeDash(dash.clone()))
        .unwrap();
    for w in [0.1, 5.0, 123.0] {
        doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(w)))
            .unwrap();
        assert_eq!(style_of(&doc, id).stroke.dash, dash);
    }
}

#[test]
fn dash_join_and_cap_choices_all_round_trip() {
    for join in [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel] {
        for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
            let doc = Document::new(1);
            let id = ellipse(&doc);
            doc.edit_style(&[id], &StyleEdit::StrokeJoin(join)).unwrap();
            doc.edit_style(&[id], &StyleEdit::StrokeCap(cap)).unwrap();
            let s = style_of(&reopen(&doc), id);
            assert_eq!((s.stroke.join, s.stroke.cap), (join, cap));
        }
    }
}

// ----------------------------------------- AC 2, 24: edits and batches

#[test]
fn style_edits_leave_primitive_shape_parameters_untouched_and_convert_nothing() {
    let doc = Document::new(1);
    let ids = [rect(&doc), ellipse(&doc), polygon(&doc), star(&doc)];
    let before: Vec<ObjectSnapshot> = ids.iter().map(|i| doc.object(*i).unwrap()).collect();
    for id in ids {
        fully_styled(&doc, id, 1);
        doc.edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
            .unwrap();
    }
    for (id, old) in ids.iter().zip(&before) {
        let (ObjectSnapshot::Primitive(old), ObjectSnapshot::Primitive(new)) =
            (old, &doc.object(*id).unwrap())
        else {
            panic!("a style edit converted a primitive to a path");
        };
        assert_eq!(old.shape, new.shape);
        assert_eq!(old.rotation, new.rotation);
        assert!(doc.path(*id).is_none());
    }
}

#[test]
fn a_batch_edit_changes_only_the_named_property_of_each_object_independently() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = open_path(&doc, 1);
    let c = star(&doc);
    doc.edit_style(&[a], &StyleEdit::StrokeWidth(mm(1.0)))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::StrokeCap(LineCap::Round))
        .unwrap();
    doc.edit_style(&[c], &StyleEdit::FillColor(rgb(9, 8, 7)))
        .unwrap();
    separate_label(&doc, a);
    let before: Vec<Style> = [a, b, c].iter().map(|i| style_of(&doc, *i)).collect();
    let changes = change_count(&doc);
    doc.edit_style(&[a, b, c], &StyleEdit::StrokeColor(rgb(255, 0, 0)))
        .unwrap();
    assert_eq!(change_count(&doc), changes + 1, "one commit for N objects");
    for (id, old) in [a, b, c].iter().zip(&before) {
        let mut want = old.clone();
        want.stroke.color = rgb(255, 0, 0);
        assert_eq!(style_of(&doc, *id), want);
    }
}

#[test]
fn a_batch_with_one_stale_id_writes_nothing_to_any_object() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    let gone = star(&doc);
    doc.delete_objects(&[gone]).unwrap();
    let ops = op_count(&doc);
    let want_a = style_of(&doc, a);
    assert_eq!(
        doc.edit_style(&[a, gone, b], &StyleEdit::StrokeColor(rgb(1, 1, 1))),
        Err(StyleEditError::NoSuchObject)
    );
    assert_eq!(
        doc.edit_style(&[gone], &StyleEdit::StrokeEnabled(false)),
        Err(StyleEditError::NoSuchObject)
    );
    assert_eq!(op_count(&doc), ops);
    assert_eq!(style_of(&doc, a), want_a);
}

#[test]
fn an_edit_that_changes_nothing_writes_nothing() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    doc.edit_style(&[a], &StyleEdit::StrokeColor(rgb(5, 5, 5)))
        .unwrap();
    let ops = op_count(&doc);
    // Same value on a, b already default black -> both unchanged except b.
    doc.edit_style(&[a], &StyleEdit::StrokeColor(rgb(5, 5, 5)))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::StrokeWidth(mm(0.25)))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::FillOpacity(Opacity::OPAQUE))
        .unwrap();
    doc.edit_style(&[a, b], &StyleEdit::StrokeDash(DashPattern::solid()))
        .unwrap();
    doc.edit_style(&[], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    assert_eq!(
        op_count(&doc),
        ops,
        "no-op edits must not create operations"
    );
}

#[test]
fn a_batch_where_only_one_object_changes_writes_only_that_one() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    doc.edit_style(&[a], &StyleEdit::StrokeColor(rgb(5, 5, 5)))
        .unwrap();
    // Concurrent peer edits b's colour; our no-op on b must not clobber it.
    let bytes = pack(&doc, "t").unwrap();
    let ours = unpack(2, &bytes).unwrap();
    let theirs = unpack(3, &bytes).unwrap();
    theirs
        .edit_style(&[b], &StyleEdit::StrokeColor(rgb(0, 200, 0)))
        .unwrap();
    ours.edit_style(&[a, b], &StyleEdit::StrokeColor(rgb(5, 5, 5)))
        .unwrap();
    let m = merged(&ours, &theirs);
    assert_eq!(style_of(&m, b).stroke.color, rgb(0, 200, 0));
}

#[test]
fn duplicate_ids_in_a_batch_are_harmless() {
    let doc = Document::new(1);
    let a = rect(&doc);
    doc.edit_style(&[a, a, a], &StyleEdit::StrokeColor(rgb(1, 2, 3)))
        .unwrap();
    assert_eq!(style_of(&doc, a).stroke.color, rgb(1, 2, 3));
}

// ------------------------------------------------- AC 13, 17: fill modes

#[test]
fn fill_none_keeps_colour_kind_and_stops_and_modes_restore_them() {
    let doc = Document::new(1);
    let id = rect(&doc);
    // First time: None -> Solid gives black.
    doc.set_fill_mode(FillMode::Solid, &[target(id, vec![])])
        .unwrap();
    let s = style_of(&doc, id);
    assert!(s.fill.enabled);
    assert_eq!(s.fill.color, Color::BLACK);
    doc.edit_style(&[id], &StyleEdit::FillColor(rgb(250, 0, 0)))
        .unwrap();
    doc.set_fill_mode(FillMode::None, &[target(id, vec![])])
        .unwrap();
    assert!(!style_of(&doc, id).fill.enabled);
    assert_eq!(style_of(&doc, id).fill.color, rgb(250, 0, 0));
    doc.set_fill_mode(FillMode::Solid, &[target(id, vec![])])
        .unwrap();
    assert_eq!(style_of(&doc, id).fill.color, rgb(250, 0, 0));
    assert!(style_of(&doc, id).fill.enabled);
    // Solid -> Linear with default seed from red.
    let seeds = GradientStop::default_pair(rgb(250, 0, 0), StopId::new(1, 1), StopId::new(1, 2));
    doc.set_fill_mode(FillMode::Linear, &[target(id, seeds.to_vec())])
        .unwrap();
    let s = style_of(&doc, id);
    assert_eq!(s.fill.kind, FillKind::Linear);
    assert_eq!(s.fill.stops.len(), 2);
    assert_eq!(s.fill.stops[0].color, rgb(250, 0, 0));
    assert_eq!(s.fill.stops[0].position.get(), 0.0);
    assert_eq!(s.fill.stops[1].color, rgb(255, 255, 255));
    assert_eq!(s.fill.stops[1].position.get(), 1.0);
    assert_eq!(s.fill.stops[0].opacity.get(), 1.0);
    assert_eq!(s.fill.stops[1].opacity.get(), 1.0);
    // Edit a stop, go back to Solid and to Radial: neither loses anything.
    doc.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 2),
        change: StopChange::Color(rgb(0, 0, 255)),
    }])
    .unwrap();
    let stops_before = style_of(&doc, id).fill.stops;
    doc.set_fill_mode(FillMode::Solid, &[target(id, vec![])])
        .unwrap();
    let s = style_of(&doc, id);
    assert_eq!(s.fill.kind, FillKind::Solid);
    assert_eq!(s.fill.color, rgb(250, 0, 0));
    assert_eq!(s.fill.stops, stops_before);
    // A fresh seed offered now is ignored: the object already has stops.
    let other = GradientStop::default_pair(rgb(1, 1, 1), StopId::new(9, 1), StopId::new(9, 2));
    doc.set_fill_mode(FillMode::Radial, &[target(id, other.to_vec())])
        .unwrap();
    let s = style_of(&doc, id);
    assert_eq!(s.fill.kind, FillKind::Radial);
    assert_eq!(s.fill.stops, stops_before);
}

#[test]
fn default_pair_for_white_is_white_then_black_and_for_near_white_is_white() {
    let a = GradientStop::default_pair(rgb(255, 255, 255), StopId::new(1, 1), StopId::new(1, 2));
    assert_eq!(a[0].color, rgb(255, 255, 255));
    assert_eq!(a[1].color, rgb(0, 0, 0));
    let b = GradientStop::default_pair(rgb(255, 255, 254), StopId::new(1, 1), StopId::new(1, 2));
    assert_eq!(b[1].color, rgb(255, 255, 255));
    let c = GradientStop::default_pair(Color::BLACK, StopId::new(1, 1), StopId::new(1, 2));
    assert_eq!(c[0].color, Color::BLACK);
    assert_eq!(c[1].color, rgb(255, 255, 255));
    assert_eq!((a[0].position.get(), a[1].position.get()), (0.0, 1.0));
}

#[test]
fn a_multi_selection_gradient_switch_gives_each_object_its_own_colour_and_ids() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    let c = open_path(&doc, 1);
    doc.edit_style(&[a], &StyleEdit::FillColor(rgb(255, 0, 0)))
        .unwrap();
    doc.edit_style(&[b], &StyleEdit::FillColor(rgb(255, 255, 255)))
        .unwrap();
    let t = |id, n, col: Color| {
        target(
            id,
            GradientStop::default_pair(col, StopId::new(1, n), StopId::new(1, n + 1)).to_vec(),
        )
    };
    let changes = change_count(&doc);
    doc.set_fill_mode(
        FillMode::Linear,
        &[
            t(a, 1, rgb(255, 0, 0)),
            t(b, 3, rgb(255, 255, 255)),
            t(c, 5, Color::BLACK),
        ],
    )
    .unwrap();
    assert_eq!(change_count(&doc), changes + 1);
    assert_eq!(style_of(&doc, a).fill.stops[0].color, rgb(255, 0, 0));
    assert_eq!(style_of(&doc, a).fill.stops[1].color, rgb(255, 255, 255));
    assert_eq!(style_of(&doc, b).fill.stops[1].color, rgb(0, 0, 0));
    assert_eq!(style_of(&doc, c).fill.stops[0].color, Color::BLACK);
}

#[test]
fn a_fill_mode_switch_with_a_stale_target_writes_nothing() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let gone = ellipse(&doc);
    doc.delete_objects(&[gone]).unwrap();
    let ops = op_count(&doc);
    let seeds = GradientStop::default_pair(Color::BLACK, StopId::new(1, 1), StopId::new(1, 2));
    assert_eq!(
        doc.set_fill_mode(
            FillMode::Linear,
            &[target(a, seeds.to_vec()), target(gone, vec![])]
        ),
        Err(StyleEditError::NoSuchObject)
    );
    assert_eq!(op_count(&doc), ops);
    assert!(!style_of(&doc, a).fill.enabled);
}

#[test]
fn repeating_the_same_fill_mode_writes_nothing() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let seeds = GradientStop::default_pair(Color::BLACK, StopId::new(1, 1), StopId::new(1, 2));
    doc.set_fill_mode(FillMode::Linear, &[target(a, seeds.to_vec())])
        .unwrap();
    let ops = op_count(&doc);
    doc.set_fill_mode(FillMode::Linear, &[target(a, seeds.to_vec())])
        .unwrap();
    doc.set_fill_mode(FillMode::None, &[target(a, vec![])])
        .unwrap();
    let after_none = op_count(&doc);
    assert!(after_none > ops);
    doc.set_fill_mode(FillMode::None, &[target(a, vec![])])
        .unwrap();
    assert_eq!(op_count(&doc), after_none);
}

#[test]
fn switching_to_a_gradient_with_no_seed_leaves_zero_stops_without_error() {
    let doc = Document::new(1);
    let a = rect(&doc);
    doc.set_fill_mode(FillMode::Radial, &[target(a, vec![])])
        .unwrap();
    let s = style_of(&doc, a);
    assert!(s.fill.enabled);
    assert_eq!(s.fill.kind, FillKind::Radial);
    assert!(s.fill.stops.is_empty());
    let back = style_of(&reopen(&doc), a);
    assert_eq!(back, s);
}

// ----------------------------------------------- AC 16-20: stop commands

fn gradient_object(doc: &Document, id: NodeId, positions: &[f64]) {
    let seeds: Vec<GradientStop> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| stop(1, 100 + i as u64, *p, rgb(i as u8, 0, 0), 1.0))
        .collect();
    doc.set_fill_mode(FillMode::Linear, &[target(id, seeds)])
        .unwrap();
}

fn positions(doc: &Document, id: NodeId) -> Vec<f64> {
    style_of(doc, id)
        .fill
        .stops
        .iter()
        .map(|s| s.position.get())
        .collect()
}

#[test]
fn add_stop_inserts_in_position_order_and_ties_go_after_equal_stops() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.5, 1.0]);
    doc.add_stop(id, stop(2, 1, 0.25, rgb(9, 9, 9), 0.5))
        .unwrap();
    assert_eq!(positions(&doc, id), vec![0.0, 0.25, 0.5, 1.0]);
    doc.add_stop(id, stop(2, 2, 0.5, rgb(8, 8, 8), 1.0))
        .unwrap();
    let ids: Vec<StopId> = style_of(&doc, id).fill.stops.iter().map(|s| s.id).collect();
    assert_eq!(positions(&doc, id), vec![0.0, 0.25, 0.5, 0.5, 1.0]);
    assert_eq!(
        ids[3],
        StopId::new(2, 2),
        "the new tie goes after the old one"
    );
    doc.add_stop(id, stop(2, 3, 0.0, rgb(7, 7, 7), 1.0))
        .unwrap();
    doc.add_stop(id, stop(2, 4, 1.0, rgb(6, 6, 6), 1.0))
        .unwrap();
    let ps = positions(&doc, id);
    assert_eq!(ps.first(), Some(&0.0));
    assert_eq!(ps.last(), Some(&1.0));
    assert_eq!(style_of(&doc, id).fill.stops[1].id, StopId::new(2, 3));
    assert_eq!(
        style_of(&doc, id).fill.stops.last().unwrap().id,
        StopId::new(2, 4)
    );
}

#[test]
fn add_stop_refuses_a_seventeenth_and_a_duplicate_id_and_writes_nothing() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let ps: Vec<f64> = (0..16).map(|i| f64::from(i) / 15.0).collect();
    gradient_object(&doc, id, &ps);
    assert_eq!(style_of(&doc, id).fill.stops.len(), MAX_GRADIENT_STOPS);
    let ops = op_count(&doc);
    assert_eq!(
        doc.add_stop(id, stop(2, 1, 0.3, rgb(0, 0, 0), 1.0)),
        Err(StyleEditError::TooManyStops)
    );
    assert_eq!(op_count(&doc), ops);
    // Remove one, then a duplicate id is refused while a fresh one is accepted.
    doc.remove_stop(id, StopId::new(1, 100)).unwrap();
    assert_eq!(
        doc.add_stop(id, stop(1, 101, 0.3, rgb(0, 0, 0), 1.0)),
        Err(StyleEditError::DuplicateStop)
    );
    doc.add_stop(id, stop(2, 1, 0.3, rgb(0, 0, 0), 1.0))
        .unwrap();
    assert_eq!(style_of(&doc, id).fill.stops.len(), 16);
}

#[test]
fn add_stop_on_a_missing_object_is_refused() {
    let doc = Document::new(1);
    let id = rect(&doc);
    doc.delete_objects(&[id]).unwrap();
    assert_eq!(
        doc.add_stop(id, stop(1, 1, 0.5, Color::BLACK, 1.0)),
        Err(StyleEditError::NoSuchObject)
    );
}

#[test]
fn add_stop_to_an_object_without_a_list_is_refused_and_writes_nothing() {
    // Only the fill-mode switch and Split create the stop list (PR 1 review).
    let doc = Document::new(1);
    let id = rect(&doc);
    let ops = op_count(&doc);
    assert_eq!(
        doc.add_stop(id, stop(1, 1, 0.5, rgb(3, 3, 3), 1.0)),
        Err(StyleEditError::NoStopList)
    );
    assert_eq!(op_count(&doc), ops);
    assert_eq!(style_of(&doc, id).fill.stops.len(), 0);
}

#[test]
fn remove_stop_refuses_below_two_and_unknown_ids() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.4, 1.0]);
    doc.remove_stop(id, StopId::new(1, 101)).unwrap();
    assert_eq!(positions(&doc, id), vec![0.0, 1.0]);
    let ops = op_count(&doc);
    assert_eq!(
        doc.remove_stop(id, StopId::new(1, 100)),
        Err(StyleEditError::TooFewStops)
    );
    assert!(matches!(
        doc.remove_stop(id, StopId::new(7, 7)),
        Err(StyleEditError::TooFewStops | StyleEditError::NoSuchStop)
    ));
    assert_eq!(op_count(&doc), ops);
    // 1 stop and 0 stops: remove is refused as well.
    let other = ellipse(&doc);
    doc.set_fill_mode(
        FillMode::Linear,
        &[target(other, vec![stop(1, 1, 0.5, Color::BLACK, 1.0)])],
    )
    .unwrap();
    assert!(matches!(
        doc.remove_stop(other, StopId::new(1, 1)),
        Err(StyleEditError::TooFewStops)
    ));
    assert_eq!(style_of(&doc, other).fill.stops.len(), 1);
}

#[test]
fn remove_stop_of_a_missing_stop_in_a_big_gradient_is_no_such_stop() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.5, 1.0]);
    let ops = op_count(&doc);
    assert_eq!(
        doc.remove_stop(id, StopId::new(9, 9)),
        Err(StyleEditError::NoSuchStop)
    );
    assert_eq!(op_count(&doc), ops);
}

#[test]
fn edit_stops_changes_only_the_named_value_and_never_reorders_the_list() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.5, 1.0]);
    let before = style_of(&doc, id).fill.stops;
    // Drag the first stop past the others.
    doc.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 100),
        change: StopChange::Position(pos(0.9)),
    }])
    .unwrap();
    let after = style_of(&doc, id).fill.stops;
    assert_eq!(
        after.iter().map(|s| s.id).collect::<Vec<_>>(),
        before.iter().map(|s| s.id).collect::<Vec<_>>(),
        "list order is untouched"
    );
    assert_eq!(after[0].position.get(), 0.9);
    assert_eq!(after[0].color, before[0].color);
    assert_eq!(after[0].opacity, before[0].opacity);
    assert_eq!(after[1], before[1]);
    assert_eq!(after[2], before[2]);
    doc.edit_stops(&[
        StopEdit {
            id,
            stop: StopId::new(1, 101),
            change: StopChange::Color(rgb(1, 2, 3)),
        },
        StopEdit {
            id,
            stop: StopId::new(1, 101),
            change: StopChange::Opacity(op(0.5)),
        },
    ])
    .unwrap();
    let s = style_of(&doc, id).fill.stops[1];
    assert_eq!(
        (s.color, s.opacity.get(), s.position.get()),
        (rgb(1, 2, 3), 0.5, 0.5)
    );
}

#[test]
fn edit_stops_is_all_or_nothing_across_objects() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = ellipse(&doc);
    gradient_object(&doc, a, &[0.0, 1.0]);
    // b uses distinct ids.
    doc.set_fill_mode(
        FillMode::Linear,
        &[target(
            b,
            vec![
                stop(1, 200, 0.0, Color::BLACK, 1.0),
                stop(1, 201, 1.0, Color::BLACK, 1.0),
            ],
        )],
    )
    .unwrap();
    let ops = op_count(&doc);
    let changes = change_count(&doc);
    assert_eq!(
        doc.edit_stops(&[
            StopEdit {
                id: a,
                stop: StopId::new(1, 100),
                change: StopChange::Color(rgb(9, 9, 9)),
            },
            StopEdit {
                id: b,
                stop: StopId::new(1, 100),
                change: StopChange::Color(rgb(9, 9, 9)),
            },
        ]),
        Err(StyleEditError::NoSuchStop)
    );
    assert_eq!(op_count(&doc), ops);
    // Valid batch across two objects is one commit.
    doc.edit_stops(&[
        StopEdit {
            id: a,
            stop: StopId::new(1, 100),
            change: StopChange::Color(rgb(9, 9, 9)),
        },
        StopEdit {
            id: b,
            stop: StopId::new(1, 201),
            change: StopChange::Color(rgb(8, 8, 8)),
        },
    ])
    .unwrap();
    assert_eq!(change_count(&doc), changes + 1);
    assert_eq!(style_of(&doc, a).fill.stops[0].color, rgb(9, 9, 9));
    assert_eq!(style_of(&doc, b).fill.stops[1].color, rgb(8, 8, 8));
}

#[test]
fn edit_stops_with_unchanged_values_or_an_empty_batch_writes_nothing() {
    let doc = Document::new(1);
    let a = rect(&doc);
    gradient_object(&doc, a, &[0.0, 1.0]);
    let cur = style_of(&doc, a).fill.stops[0];
    let ops = op_count(&doc);
    doc.edit_stops(&[]).unwrap();
    doc.edit_stops(&[StopEdit {
        id: a,
        stop: cur.id,
        change: StopChange::Position(cur.position),
    }])
    .unwrap();
    doc.edit_stops(&[StopEdit {
        id: a,
        stop: cur.id,
        change: StopChange::Color(cur.color),
    }])
    .unwrap();
    doc.edit_stops(&[StopEdit {
        id: a,
        stop: cur.id,
        change: StopChange::Opacity(cur.opacity),
    }])
    .unwrap();
    assert_eq!(op_count(&doc), ops);
}

#[test]
fn edit_stops_on_a_deleted_object_is_no_such_object() {
    let doc = Document::new(1);
    let a = rect(&doc);
    gradient_object(&doc, a, &[0.0, 1.0]);
    doc.delete_objects(&[a]).unwrap();
    assert_eq!(
        doc.edit_stops(&[StopEdit {
            id: a,
            stop: StopId::new(1, 100),
            change: StopChange::Color(Color::BLACK),
        }]),
        Err(StyleEditError::NoSuchObject)
    );
}

#[test]
fn coincident_stops_keep_their_list_order_through_save_and_reopen() {
    let doc = Document::new(1);
    let a = rect(&doc);
    doc.set_fill_mode(
        FillMode::Linear,
        &[target(
            a,
            vec![
                stop(1, 1, 0.0, rgb(1, 0, 0), 1.0),
                stop(1, 2, 0.5, rgb(2, 0, 0), 1.0),
                stop(1, 3, 0.5, rgb(3, 0, 0), 1.0),
                stop(1, 4, 0.5, rgb(4, 0, 0), 1.0),
                stop(1, 5, 1.0, rgb(5, 0, 0), 1.0),
            ],
        )],
    )
    .unwrap();
    let back = style_of(&reopen(&reopen(&doc)), a).fill.stops;
    let reds: Vec<u8> = back.iter().map(|s| s.color.r).collect();
    assert_eq!(reds, vec![1, 2, 3, 4, 5]);
}

// ------------------------------------------------- AC 25: persistence

#[test]
fn a_fully_styled_object_of_every_kind_round_trips_twice() {
    let doc = Document::new(1);
    let ids = [
        rect(&doc),
        ellipse(&doc),
        polygon(&doc),
        star(&doc),
        open_path(&doc, 1),
        closed_path(&doc, 1),
    ];
    for (i, id) in ids.iter().enumerate() {
        fully_styled(&doc, *id, 10 + i as u64);
    }
    let first = reopen(&doc);
    let second = reopen(&first);
    assert_eq!(doc.object_ids(), first.object_ids(), "z-order is kept");
    for id in ids {
        let want = style_of(&doc, id);
        assert_eq!(want.fill.stops.len(), 4);
        assert_eq!(style_of(&first, id), want);
        assert_eq!(style_of(&second, id), want);
    }
}

#[test]
fn document_json_has_a_style_object_per_object_with_the_stop_ids() {
    let doc = Document::new(1);
    let a = rect(&doc);
    let b = open_path(&doc, 1);
    fully_styled(&doc, a, 3);
    let json: serde_json::Value = serde_json::from_slice(&doc.export_json().unwrap()).unwrap();
    let objects = json["objects"].as_array().expect("objects array");
    assert_eq!(objects.len(), 2);
    for o in objects {
        assert!(o["style"].is_object(), "style object missing: {o}");
    }
    let hex = StopId::new(3, 1).to_hex();
    let text = objects[0]["style"].to_string();
    assert!(text.contains(&hex), "stop id hex not in {text}");
    let _ = b;
}

#[test]
fn stop_id_hex_round_trips_for_extreme_values() {
    for id in [
        StopId::new(0, 0),
        StopId::new(u64::MAX, u64::MAX),
        StopId::new(1, u64::MAX),
        StopId::new(u64::MAX, 0),
    ] {
        assert_eq!(StopId::from_hex(&id.to_hex()), Some(id));
    }
    for bad in ["", "zz", "-1", "0x10", " 1", "1 ", &"f".repeat(33)] {
        assert_eq!(StopId::from_hex(bad), None, "{bad:?}");
    }
}

// --------------------------------------------- open-file validation

fn valid_base() -> Document {
    let doc = Document::new(1);
    let _ = rect(&doc);
    doc
}

#[test]
fn control_a_corrupted_container_helper_with_valid_values_still_opens() {
    let doc = valid_base();
    let bytes = corrupted(&doc, |m| {
        m.insert("stroke_enabled", false).unwrap();
        m.insert("stroke_width", 2.0).unwrap();
        m.insert("stroke", ints(&[1, 2, 3])).unwrap();
        m.insert("stroke_opacity", 0.5).unwrap();
        m.insert("stroke_dash", floats(&[2.0, 1.0])).unwrap();
        m.insert("stroke_join", "bevel").unwrap();
        m.insert("stroke_cap", "square").unwrap();
        m.insert("fill_enabled", true).unwrap();
        m.insert("fill_kind", "radial").unwrap();
        m.insert("fill", ints(&[4, 5, 6])).unwrap();
        m.insert("fill_opacity", 0.0).unwrap();
        push_stop(m, &good_stop_id(), 0.0.into(), ints(&[1, 1, 1]), 1.0.into());
    });
    let opened = unpack(1, &bytes).expect("valid values must open");
    let id = opened.object_ids()[0];
    let s = style_of(&opened, id);
    assert_eq!(s.stroke.width.as_mm(), 2.0);
    assert_eq!(s.stroke.join, LineJoin::Bevel);
    assert_eq!(s.stroke.cap, LineCap::Square);
    assert_eq!(s.fill.kind, FillKind::Radial);
    assert_eq!(s.fill.stops.len(), 1);
}

fn assert_damaged(label: &str, mutate: impl FnOnce(&LoroMap)) {
    let bytes = corrupted(&valid_base(), mutate);
    match unpack(1, &bytes) {
        Err(OpenError::Damaged) => {}
        Ok(_) => panic!("{label}: damaged file was accepted"),
        Err(other) => panic!("{label}: wrong error {other:?}"),
    }
}

#[test]
fn every_scalar_style_key_with_a_wrong_type_or_bad_value_is_damaged() {
    assert_damaged("enabled string", |m| {
        m.insert("stroke_enabled", "yes").unwrap()
    });
    assert_damaged("enabled list", |m| {
        m.insert("stroke_enabled", ints(&[1])).unwrap()
    });
    assert_damaged("fill_enabled string", |m| {
        m.insert("fill_enabled", "true").unwrap()
    });
    for w in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_damaged(&format!("width {w}"), |m| {
            m.insert("stroke_width", w).unwrap()
        });
    }
    assert_damaged("width string", |m| m.insert("stroke_width", "2").unwrap());
    assert_damaged("width list", |m| {
        m.insert("stroke_width", floats(&[2.0])).unwrap()
    });
    for o in [-0.001, 1.001, f64::NAN, f64::INFINITY, 255.0] {
        assert_damaged(&format!("stroke_opacity {o}"), |m| {
            m.insert("stroke_opacity", o).unwrap();
        });
        assert_damaged(&format!("fill_opacity {o}"), |m| {
            m.insert("fill_opacity", o).unwrap();
        });
    }
    assert_damaged("opacity string", |m| {
        m.insert("stroke_opacity", "1").unwrap()
    });
    assert_damaged("opacity bool", |m| m.insert("fill_opacity", true).unwrap());
}

#[test]
fn enum_keys_accept_only_the_exact_lowercase_words() {
    for bad in ["MITER", "Miter", "clip", "arcs", "", " miter", "miter "] {
        assert_damaged(&format!("join {bad:?}"), |m| {
            m.insert("stroke_join", bad).unwrap()
        });
    }
    for bad in ["flat", "BUTT", "", "Round"] {
        assert_damaged(&format!("cap {bad:?}"), |m| {
            m.insert("stroke_cap", bad).unwrap()
        });
    }
    for bad in ["conic", "none", "SOLID", "", "pattern"] {
        assert_damaged(&format!("kind {bad:?}"), |m| {
            m.insert("fill_kind", bad).unwrap()
        });
    }
    assert_damaged("join number", |m| m.insert("stroke_join", 1_i64).unwrap());
    assert_damaged("cap bool", |m| m.insert("stroke_cap", true).unwrap());
    assert_damaged("kind number", |m| m.insert("fill_kind", 2.0).unwrap());
}

#[test]
fn colour_keys_must_be_three_integers_between_0_and_255() {
    for key in ["stroke", "fill"] {
        assert_damaged(&format!("{key} len 2"), |m| {
            m.insert(key, ints(&[1, 2])).unwrap()
        });
        assert_damaged(&format!("{key} len 4"), |m| {
            m.insert(key, ints(&[1, 2, 3, 4])).unwrap()
        });
        assert_damaged(&format!("{key} empty"), |m| {
            m.insert(key, ints(&[])).unwrap()
        });
        assert_damaged(&format!("{key} 256"), |m| {
            m.insert(key, ints(&[256, 0, 0])).unwrap()
        });
        assert_damaged(&format!("{key} -1"), |m| {
            m.insert(key, ints(&[0, -1, 0])).unwrap()
        });
        assert_damaged(&format!("{key} string"), |m| {
            m.insert(key, "#ff0000").unwrap()
        });
        assert_damaged(&format!("{key} floats"), |m| {
            m.insert(key, floats(&[1.5, 2.0, 3.0])).unwrap();
        });
        assert_damaged(&format!("{key} huge"), |m| {
            m.insert(key, ints(&[i64::MAX, 0, 0])).unwrap();
        });
    }
}

#[test]
fn dash_lists_that_break_the_rules_are_damaged() {
    assert_damaged("odd", |m| m.insert("stroke_dash", floats(&[1.0])).unwrap());
    assert_damaged("odd 3", |m| {
        m.insert("stroke_dash", floats(&[1.0, 2.0, 3.0])).unwrap()
    });
    assert_damaged("negative", |m| {
        m.insert("stroke_dash", floats(&[-1.0, 2.0])).unwrap()
    });
    assert_damaged("all zero", |m| {
        m.insert("stroke_dash", floats(&[0.0, 0.0])).unwrap()
    });
    assert_damaged("nan", |m| {
        m.insert("stroke_dash", floats(&[f64::NAN, 1.0])).unwrap()
    });
    assert_damaged("inf", |m| {
        m.insert("stroke_dash", floats(&[f64::INFINITY, 1.0]))
            .unwrap()
    });
    assert_damaged("string", |m| m.insert("stroke_dash", "dashed").unwrap());
    assert_damaged("mixed types", |m| {
        m.insert(
            "stroke_dash",
            LoroValue::from(vec![LoroValue::from(1.0), LoroValue::from("a")]),
        )
        .unwrap();
    });
}

#[test]
fn a_zero_dash_entry_beside_a_positive_one_and_an_empty_list_are_accepted() {
    for list in [
        floats(&[]),
        floats(&[0.0, 1.0]),
        floats(&[1.0, 0.0]),
        floats(&[1e300, 1e300]),
    ] {
        let bytes = corrupted(&valid_base(), |m| m.insert("stroke_dash", list).unwrap());
        assert!(unpack(1, &bytes).is_ok());
    }
}

#[test]
fn a_stop_with_a_missing_or_bad_field_is_damaged() {
    let id = good_stop_id();
    let ok = |m: &LoroMap| push_stop(m, &id, 0.5.into(), ints(&[1, 2, 3]), 1.0.into());
    // Control.
    assert!(unpack(1, &corrupted(&valid_base(), ok)).is_ok());

    for (label, field) in [
        ("id", "id"),
        ("position", "position"),
        ("color", "color"),
        ("opacity", "opacity"),
    ] {
        assert_damaged(&format!("missing {label}"), |m| {
            let list = m.ensure_mergeable_movable_list("fill_stops").unwrap();
            let map = list.push_container(LoroMap::new()).unwrap();
            for (k, v) in [
                ("id", LoroValue::from(id.as_str())),
                ("position", LoroValue::from(0.5)),
                ("color", ints(&[1, 2, 3])),
                ("opacity", LoroValue::from(1.0)),
            ] {
                if k != field {
                    map.insert(k, v).unwrap();
                }
            }
        });
    }
    for p in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert_damaged(&format!("position {p}"), |m| {
            push_stop(m, &id, p.into(), ints(&[1, 2, 3]), 1.0.into());
        });
        assert_damaged(&format!("stop opacity {p}"), |m| {
            push_stop(m, &id, 0.5.into(), ints(&[1, 2, 3]), p.into());
        });
    }
    assert_damaged("stop colour len 2", |m| {
        push_stop(m, &id, 0.5.into(), ints(&[1, 2]), 1.0.into());
    });
    assert_damaged("stop colour 300", |m| {
        push_stop(m, &id, 0.5.into(), ints(&[300, 2, 1]), 1.0.into());
    });
    assert_damaged("stop id not hex", |m| {
        push_stop(m, "not-hex", 0.5.into(), ints(&[1, 2, 3]), 1.0.into());
    });
    assert_damaged("stop id empty", |m| {
        push_stop(m, "", 0.5.into(), ints(&[1, 2, 3]), 1.0.into());
    });
    assert_damaged("stop id number", |m| {
        push_stop(m, &id, 0.5.into(), ints(&[1, 2, 3]), 1.0.into());
        let list = m.ensure_mergeable_movable_list("fill_stops").unwrap();
        let map = list.push_container(LoroMap::new()).unwrap();
        map.insert("id", 7_i64).unwrap();
        map.insert("position", 0.5).unwrap();
        map.insert("color", ints(&[1, 2, 3])).unwrap();
        map.insert("opacity", 1.0).unwrap();
    });
    assert_damaged("stop position as string", |m| {
        push_stop(m, &id, "0.5".into(), ints(&[1, 2, 3]), 1.0.into());
    });
    assert_damaged("only the second stop is bad", |m| {
        push_stop(m, &id, 0.5.into(), ints(&[1, 2, 3]), 1.0.into());
        push_stop(
            m,
            &StopId::new(6, 6).to_hex(),
            2.0.into(),
            ints(&[1, 2, 3]),
            1.0.into(),
        );
    });
}

#[test]
fn fill_stops_that_is_not_a_list_is_damaged() {
    assert_damaged("stops as string", |m| {
        m.insert("fill_stops", "none").unwrap()
    });
    assert_damaged("stops as number", |m| {
        m.insert("fill_stops", 3_i64).unwrap()
    });
    assert_damaged("stops as plain list", |m| {
        m.insert("fill_stops", ints(&[1, 2])).unwrap()
    });
}

#[test]
fn zero_one_seventeen_and_many_stops_all_open_and_keep_their_count() {
    for count in [0_u64, 1, 2, 16, 17, 40, 500] {
        let bytes = corrupted(&valid_base(), |m| {
            m.insert("fill_enabled", true).unwrap();
            m.insert("fill_kind", "linear").unwrap();
            for n in 0..count {
                push_stop(
                    m,
                    &StopId::new(8, n).to_hex(),
                    (f64::from(n as u32) / 500.0).into(),
                    ints(&[1, 2, 3]),
                    1.0.into(),
                );
            }
        });
        let doc = unpack(1, &bytes).unwrap_or_else(|e| panic!("{count} stops: {e:?}"));
        let id = doc.object_ids()[0];
        assert_eq!(style_of(&doc, id).fill.stops.len(), count as usize);
        // And saving it again keeps them.
        assert_eq!(style_of(&reopen(&doc), id).fill.stops.len(), count as usize);
    }
}

#[test]
fn damaged_style_keys_on_a_path_object_are_refused_too() {
    let doc = Document::new(1);
    let _ = open_path(&doc, 1);
    let bytes = corrupted(&doc, |m| m.insert("stroke_width", -3.0).unwrap());
    assert!(matches!(unpack(1, &bytes), Err(OpenError::Damaged)));
    let bytes = corrupted(&doc, |m| m.insert("stroke_join", "zigzag").unwrap());
    assert!(matches!(unpack(1, &bytes), Err(OpenError::Damaged)));
}

#[test]
fn a_damaged_style_key_in_an_old_format_version_file_is_still_not_read_as_valid() {
    // A v6 container carrying a v7 key is a forged mix; it must not panic.
    let doc = valid_base();
    let loro = LoroDoc::new();
    loro.import(&doc.export_loro_snapshot().unwrap()).unwrap();
    let tree = loro.get_tree("paths");
    let meta = tree.get_meta(tree.roots()[0]).unwrap();
    meta.insert("stroke_width", f64::NAN).unwrap();
    loro.commit();
    let bytes = container_from_loro(6, &loro.export(loro::ExportMode::Snapshot).unwrap());
    // Either outcome is acceptable; panicking or opening a NaN width is not.
    if let Ok(opened) = unpack(1, &bytes) {
        for id in opened.object_ids() {
            let w = style_of(&opened, id).stroke.width.as_mm();
            assert!(w.is_finite() && w > 0.0, "NaN width leaked: {w}");
        }
    }
}

// ----------------------------------------------- CRDT merge of registers

#[test]
fn concurrent_edits_to_different_style_properties_both_survive_in_both_orders() {
    let doc = Document::new(1);
    let id = open_path(&doc, 1);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.edit_style(&[id], &StyleEdit::StrokeColor(rgb(255, 0, 0)))
        .unwrap();
    a.edit_style(&[id], &StyleEdit::FillColor(rgb(0, 0, 255)))
        .unwrap();
    b.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    )
    .unwrap();
    b.edit_style(&[id], &StyleEdit::StrokeJoin(LineJoin::Round))
        .unwrap();
    b.edit_style(&[id], &StyleEdit::StrokeOpacity(op(0.5)))
        .unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let s = style_of(&m, id);
        assert_eq!(s.stroke.color, rgb(255, 0, 0));
        assert_eq!(s.fill.color, rgb(0, 0, 255));
        assert_eq!(s.stroke.dash.as_slice(), &[6.0, 4.0]);
        assert_eq!(s.stroke.join, LineJoin::Round);
        assert_eq!(s.stroke.opacity.get(), 0.5);
    }
}

#[test]
fn concurrent_edits_of_the_same_register_converge_to_one_value_everywhere() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.edit_style(&[id], &StyleEdit::StrokeColor(rgb(255, 0, 0)))
        .unwrap();
    b.edit_style(&[id], &StyleEdit::StrokeColor(rgb(0, 255, 0)))
        .unwrap();
    let ab = style_of(&merged(&a, &b), id);
    let ba = style_of(&merged(&b, &a), id);
    assert_eq!(ab, ba);
    assert!(ab.stroke.color == rgb(255, 0, 0) || ab.stroke.color == rgb(0, 255, 0));
}

#[test]
fn a_peer_turning_the_stroke_off_does_not_lose_a_concurrent_colour_or_width() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    b.edit_style(&[id], &StyleEdit::StrokeWidth(mm(4.0)))
        .unwrap();
    let ab = style_of(&merged(&a, &b), id);
    let ba = style_of(&merged(&b, &a), id);
    assert_eq!(ab, ba);
    assert_eq!(ab.stroke.width.as_mm(), 4.0);
}

#[test]
fn concurrent_edits_of_different_stops_and_of_different_fields_of_one_stop_survive() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.5, 1.0]);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 100),
        change: StopChange::Color(rgb(250, 0, 0)),
    }])
    .unwrap();
    a.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 101),
        change: StopChange::Position(pos(0.7)),
    }])
    .unwrap();
    b.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 100),
        change: StopChange::Position(pos(0.1)),
    }])
    .unwrap();
    b.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(1, 102),
        change: StopChange::Opacity(op(0.3)),
    }])
    .unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let st = style_of(&m, id).fill.stops;
        assert_eq!(st.len(), 3);
        assert_eq!((st[0].color, st[0].position.get()), (rgb(250, 0, 0), 0.1));
        assert_eq!(st[1].position.get(), 0.7);
        assert_eq!(st[2].opacity.get(), 0.3);
    }
}

#[test]
fn concurrent_adds_keep_both_stops_and_concurrent_removes_can_leave_one() {
    let doc = Document::new(1);
    let id = rect(&doc);
    gradient_object(&doc, id, &[0.0, 0.5, 1.0]);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.add_stop(id, stop(2, 1, 0.25, Color::BLACK, 1.0)).unwrap();
    b.add_stop(id, stop(3, 1, 0.75, Color::BLACK, 1.0)).unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        assert_eq!(style_of(&m, id).fill.stops.len(), 5);
    }
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.remove_stop(id, StopId::new(1, 100)).unwrap();
    b.remove_stop(id, StopId::new(1, 101)).unwrap();
    for m in [merged(&a, &b), merged(&b, &a)] {
        let st = style_of(&m, id).fill.stops;
        assert_eq!(
            st.len(),
            1,
            "two peers each removed a different stop of three"
        );
        // The merged document saves and reopens, and the editor can still add.
        let reopened = reopen(&m);
        assert_eq!(style_of(&reopened, id).fill.stops.len(), 1);
        reopened
            .add_stop(id, stop(5, 1, 0.5, Color::BLACK, 1.0))
            .unwrap();
        assert_eq!(
            reopened.remove_stop(id, StopId::new(5, 1)),
            Err(StyleEditError::TooFewStops)
        );
    }
    // Both peers remove the same stop: still one removal.
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.remove_stop(id, StopId::new(1, 100)).unwrap();
    b.remove_stop(id, StopId::new(1, 100)).unwrap();
    assert_eq!(style_of(&merged(&a, &b), id).fill.stops.len(), 2);
}

#[test]
fn two_peers_each_adding_to_sixteen_stops_merge_to_more_than_sixteen_and_still_open() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let ps: Vec<f64> = (0..15).map(|i| f64::from(i) / 14.0).collect();
    gradient_object(&doc, id, &ps);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.add_stop(id, stop(2, 1, 0.01, Color::BLACK, 1.0)).unwrap();
    b.add_stop(id, stop(3, 1, 0.02, Color::BLACK, 1.0)).unwrap();
    let m = merged(&a, &b);
    let n = style_of(&m, id).fill.stops.len();
    assert_eq!(n, 17);
    let reopened = reopen(&m);
    assert_eq!(style_of(&reopened, id).fill.stops.len(), 17);
    assert_eq!(
        reopened.add_stop(id, stop(5, 1, 0.5, Color::BLACK, 1.0)),
        Err(StyleEditError::TooManyStops)
    );
    reopened.remove_stop(id, StopId::new(2, 1)).unwrap();
    assert_eq!(style_of(&reopened, id).fill.stops.len(), 16);
}

#[test]
fn two_peers_creating_the_first_gradient_concurrently_converge_on_one_list() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    let seed = |peer| {
        GradientStop::default_pair(Color::BLACK, StopId::new(peer, 1), StopId::new(peer, 2))
            .to_vec()
    };
    a.set_fill_mode(FillMode::Linear, &[target(id, seed(2))])
        .unwrap();
    b.set_fill_mode(FillMode::Radial, &[target(id, seed(3))])
        .unwrap();
    let ab = style_of(&merged(&a, &b), id);
    let ba = style_of(&merged(&b, &a), id);
    assert_eq!(ab, ba, "order-independent");
    assert!(ab.fill.stops.len() >= 2);
    assert!(ab.fill.enabled);
    // The merged list must be usable: every stop id addressable.
    let m = merged(&a, &b);
    for s in ab.fill.stops {
        m.edit_stops(&[StopEdit {
            id,
            stop: s.id,
            change: StopChange::Color(rgb(9, 9, 9)),
        }])
        .unwrap();
    }
}

#[test]
fn styling_an_object_a_peer_deleted_merges_without_panic_and_the_object_stays_gone() {
    let doc = Document::new(1);
    let id = rect(&doc);
    let keep = ellipse(&doc);
    let bytes = pack(&doc, "t").unwrap();
    let a = unpack(2, &bytes).unwrap();
    let b = unpack(3, &bytes).unwrap();
    a.delete_objects(&[id]).unwrap();
    b.edit_style(&[id, keep], &StyleEdit::StrokeColor(rgb(1, 2, 3)))
        .unwrap();
    let m = merged(&a, &b);
    assert!(m.object(id).is_none());
    assert_eq!(style_of(&m, keep).stroke.color, rgb(1, 2, 3));
    assert_eq!(
        m.edit_style(&[id], &StyleEdit::StrokeEnabled(false)),
        Err(StyleEditError::NoSuchObject)
    );
}

// -------------------------------- AC 30-33: duplicate, split, join, convert

#[test]
fn a_copy_of_every_kind_has_the_same_style_and_is_independent() {
    let doc = Document::new(1);
    let ids = [
        rect(&doc),
        ellipse(&doc),
        polygon(&doc),
        star(&doc),
        open_path(&doc, 1),
    ];
    for (i, id) in ids.iter().enumerate() {
        fully_styled(&doc, *id, 20 + i as u64);
    }
    let sources: Vec<CopySource> = ids
        .iter()
        .map(|id| CopySource {
            id: *id,
            anchor_ids: if doc.path(*id).is_some() {
                vec![
                    AnchorId::new(50, 1),
                    AnchorId::new(50, 2),
                    AnchorId::new(50, 3),
                    AnchorId::new(50, 4),
                ]
            } else {
                vec![]
            },
        })
        .collect();
    let copies = doc
        .duplicate_objects(&sources, Vec2::new(5.0, 5.0))
        .unwrap();
    assert_eq!(copies.len(), ids.len());
    for (orig, copy) in ids.iter().zip(&copies) {
        let want = style_of(&doc, *orig);
        assert_eq!(style_of(&doc, *copy), want);
        // Edit the copy's stop and every flat value: original unchanged.
        let first = want.fill.stops[0].id;
        doc.edit_stops(&[StopEdit {
            id: *copy,
            stop: first,
            change: StopChange::Color(rgb(11, 22, 33)),
        }])
        .unwrap();
        doc.edit_style(&[*copy], &StyleEdit::StrokeColor(rgb(11, 22, 33)))
            .unwrap();
        assert_eq!(
            style_of(&doc, *orig),
            want,
            "original changed by a copy edit"
        );
        // And the reverse.
        let copy_now = style_of(&doc, *copy);
        doc.edit_stops(&[StopEdit {
            id: *orig,
            stop: first,
            change: StopChange::Position(pos(0.123)),
        }])
        .unwrap();
        assert_eq!(
            style_of(&doc, *copy),
            copy_now,
            "copy changed by an original edit"
        );
    }
}

#[test]
fn splitting_an_open_path_gives_both_halves_the_full_style() {
    let doc = Document::new(1);
    let id = open_path(&doc, 1);
    fully_styled(&doc, id, 30);
    let want = style_of(&doc, id);
    let ((first, _), (second, _)) = doc
        .split_at_anchor(id, AnchorId::new(1, 2), AnchorId::new(60, 1))
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(style_of(&doc, first), want);
    assert_eq!(style_of(&doc, second), want);
    // Independent after the split.
    doc.edit_style(&[second], &StyleEdit::StrokeCap(LineCap::Butt))
        .unwrap();
    assert_eq!(style_of(&doc, first), want);
    let back = reopen(&doc);
    assert_eq!(style_of(&back, first), want);
}

#[test]
fn splitting_a_closed_path_changes_no_style() {
    let doc = Document::new(1);
    let id = closed_path(&doc, 1);
    fully_styled(&doc, id, 31);
    let want = style_of(&doc, id);
    if let Ok(((a, _), (b, _))) =
        doc.split_at_anchor(id, AnchorId::new(1, 12), AnchorId::new(61, 1))
    {
        assert_eq!(style_of(&doc, a), want);
        assert_eq!(style_of(&doc, b), want);
    }
    assert_eq!(style_of(&doc, id), want);
}

#[test]
fn join_keeps_the_survivors_style_and_discards_the_other() {
    for filled_survivor in [true, false] {
        let doc = Document::new(1);
        let a = doc.create_path(&[anchor(1, 1, 0.0, 0.0), anchor(1, 2, 5.0, 0.0)], false);
        let b = doc.create_path(&[anchor(1, 3, 5.0, 0.0), anchor(1, 4, 9.0, 0.0)], false);
        let (filled, plain) = if filled_survivor { (a, b) } else { (b, a) };
        fully_styled(&doc, filled, 40);
        doc.edit_style(&[plain], &StyleEdit::StrokeColor(rgb(7, 7, 7)))
            .unwrap();
        let a_before = style_of(&doc, a);
        let b_before = style_of(&doc, b);
        let (survivor, _) = doc
            .join_endpoints(a, AnchorId::new(1, 2), b, AnchorId::new(1, 3))
            .unwrap();
        let want = if survivor == a { a_before } else { b_before };
        assert_eq!(style_of(&doc, survivor), want);
        assert!(
            doc.object_ids().len() == 1,
            "the other path is deleted with its style"
        );
        if !filled_survivor {
            assert!(!style_of(&doc, survivor).fill.enabled);
            assert!(style_of(&doc, survivor).fill.stops.is_empty());
        }
    }
}

#[test]
fn closing_a_path_onto_itself_keeps_its_style() {
    let doc = Document::new(1);
    let id = open_path(&doc, 1);
    fully_styled(&doc, id, 41);
    let want = style_of(&doc, id);
    let (survivor, _) = doc
        .join_endpoints(id, AnchorId::new(1, 1), id, AnchorId::new(1, 4))
        .unwrap();
    assert_eq!(survivor, id);
    assert!(doc.path(id).unwrap().closed);
    assert_eq!(style_of(&doc, id), want);
}

#[test]
fn object_to_path_keeps_the_whole_style_including_stops() {
    let doc = Document::new(1);
    let r = rect(&doc);
    let e = ellipse(&doc);
    fully_styled(&doc, r, 42);
    fully_styled(&doc, e, 43);
    let want_r = style_of(&doc, r);
    let want_e = style_of(&doc, e);
    let quad = |peer: u64| {
        vec![
            anchor(peer, 1, 0.0, 0.0),
            anchor(peer, 2, 10.0, 0.0),
            anchor(peer, 3, 10.0, 6.0),
            anchor(peer, 4, 0.0, 6.0),
        ]
    };
    doc.convert_to_paths(&[(r, quad(70)), (e, quad(71))])
        .unwrap();
    assert!(doc.primitive(r).is_none());
    assert!(doc.path(r).unwrap().closed);
    assert_eq!(style_of(&doc, r), want_r);
    assert_eq!(style_of(&doc, e), want_e);
    let back = reopen(&doc);
    assert_eq!(style_of(&back, r), want_r);
}

// ------------------------------------------- resize writes (AC 4 support)

#[test]
fn resize_commands_refuse_a_non_positive_or_non_finite_width_before_any_write() {
    let doc = Document::new(1);
    let r = rect(&doc);
    let e = ellipse(&doc);
    let p = polygon(&doc);
    let s = star(&doc);
    let path = open_path(&doc, 1);
    let snapshot = doc.path(path).unwrap();
    let anchors: Vec<_> = snapshot
        .anchors
        .iter()
        .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
        .collect();
    let ops = op_count(&doc);
    for bad in [0.0, -2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let w = Some(mm(bad));
        assert_eq!(
            doc.resize_rect(
                r,
                bounds(1.0, 1.0, 50.0, 50.0),
                CornerRadii::uniform(mm(0.0)),
                w
            ),
            Err(ShapeEditError::InvalidStrokeWidth),
            "rect {bad}"
        );
        assert_eq!(
            doc.resize_ellipse(
                e,
                EllipseFrame {
                    center: pt(1.0, 1.0),
                    rx: mm(9.0),
                    ry: mm(9.0)
                },
                w
            ),
            Err(ShapeEditError::InvalidStrokeWidth),
            "ellipse {bad}"
        );
        for id in [p, s] {
            assert_eq!(
                doc.resize_star_frame(
                    id,
                    StarFrame {
                        center: pt(1.0, 1.0),
                        radius: mm(30.0),
                        angle: curvyo_document_core::Angle::from_radians(0.0)
                    },
                    w
                ),
                Err(ShapeEditError::InvalidStrokeWidth),
                "star {bad}"
            );
        }
        assert_eq!(
            doc.resize_path(path, &anchors, w),
            Err(PathEditError::InvalidStrokeWidth),
            "path {bad}"
        );
    }
    assert_eq!(op_count(&doc), ops, "a refused resize wrote something");
}

#[test]
fn resize_with_a_width_writes_the_width_even_on_an_off_stroke_and_none_writes_no_style() {
    let doc = Document::new(1);
    let r = rect(&doc);
    fully_styled(&doc, r, 44);
    doc.edit_style(&[r], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let before = style_of(&doc, r);
    doc.resize_rect(
        r,
        bounds(0.0, 0.0, 20.0, 12.0),
        CornerRadii::uniform(mm(0.0)),
        None,
    )
    .unwrap();
    assert_eq!(style_of(&doc, r), before, "None must write no style key");
    doc.resize_rect(
        r,
        bounds(0.0, 0.0, 40.0, 24.0),
        CornerRadii::uniform(mm(0.0)),
        Some(mm(3.5)),
    )
    .unwrap();
    let after = style_of(&doc, r);
    let mut want = before;
    want.stroke.width = mm(3.5);
    assert_eq!(
        after, want,
        "only the width changes; stroke stays off; dash untouched"
    );
    assert!(!after.stroke.enabled);
}

#[test]
fn resizing_a_path_with_a_width_keeps_dash_ratios_and_all_other_style() {
    let doc = Document::new(1);
    let id = open_path(&doc, 1);
    fully_styled(&doc, id, 45);
    let before = style_of(&doc, id);
    let anchors: Vec<_> = doc
        .path(id)
        .unwrap()
        .anchors
        .iter()
        .map(|a| {
            (
                a.id,
                a.point.translated(Vec2::new(3.0, 3.0)),
                a.handle_in,
                a.handle_out,
            )
        })
        .collect();
    doc.resize_path(id, &anchors, Some(mm(9.0))).unwrap();
    let mut want = before;
    want.stroke.width = mm(9.0);
    assert_eq!(style_of(&doc, id), want);
}

// ---------------------------------------------------- randomized sequence

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 101) as f64 / 100.0
    }
}

#[test]
fn a_long_random_command_sequence_keeps_every_invariant_and_survives_reopen() {
    for seed in [1_u64, 7, 99, 2024] {
        let mut rng = Lcg(seed);
        let doc = Document::new(1);
        let ids = [rect(&doc), ellipse(&doc), open_path(&doc, 1), star(&doc)];
        let mut next_stop = 1_u64;
        for step in 0..400 {
            let id = ids[rng.pick(ids.len())];
            let group: Vec<NodeId> = if rng.pick(3) == 0 {
                ids.to_vec()
            } else {
                vec![id]
            };
            match rng.pick(12) {
                0 => {
                    let w = [0.0, 0.1, 0.25, 3.0, 40.0][rng.pick(5)];
                    doc.edit_style(&group, &StyleEdit::StrokeWidth(mm(w)))
                        .unwrap();
                }
                1 => doc
                    .edit_style(&group, &StyleEdit::StrokeEnabled(rng.pick(2) == 0))
                    .unwrap(),
                2 => doc
                    .edit_style(
                        &group,
                        &StyleEdit::StrokeColor(rgb(rng.pick(256) as u8, 0, 9)),
                    )
                    .unwrap(),
                3 => doc
                    .edit_style(&group, &StyleEdit::FillOpacity(op(rng.unit())))
                    .unwrap(),
                4 => {
                    let d = [
                        vec![],
                        vec![6.0, 4.0],
                        vec![1.0, 3.0],
                        vec![6.0, 3.0, 1.0, 3.0],
                    ];
                    doc.edit_style(
                        &group,
                        &StyleEdit::StrokeDash(DashPattern::new(d[rng.pick(4)].clone()).unwrap()),
                    )
                    .unwrap();
                }
                5 => {
                    let mode = [
                        FillMode::None,
                        FillMode::Solid,
                        FillMode::Linear,
                        FillMode::Radial,
                    ][rng.pick(4)];
                    let targets: Vec<FillModeTarget> = group
                        .iter()
                        .map(|g| {
                            let c = style_of(&doc, *g).fill.color;
                            next_stop += 2;
                            target(
                                *g,
                                GradientStop::default_pair(
                                    c,
                                    StopId::new(1, next_stop),
                                    StopId::new(1, next_stop + 1),
                                )
                                .to_vec(),
                            )
                        })
                        .collect();
                    doc.set_fill_mode(mode, &targets).unwrap();
                }
                6 | 7 => {
                    next_stop += 1;
                    let r =
                        doc.add_stop(id, stop(1, next_stop, rng.unit(), rgb(1, 2, 3), rng.unit()));
                    let n = style_of(&doc, id).fill.stops.len();
                    assert!(
                        r.is_ok()
                            || (r == Err(StyleEditError::TooManyStops) && n == 16)
                            || (r == Err(StyleEditError::NoStopList) && n == 0)
                    );
                }
                8 => {
                    let st = style_of(&doc, id).fill.stops;
                    if !st.is_empty() {
                        let victim = st[rng.pick(st.len())].id;
                        let r = doc.remove_stop(id, victim);
                        assert!(
                            r.is_ok() || (r == Err(StyleEditError::TooFewStops) && st.len() <= 2),
                            "step {step}: {r:?} with {} stops",
                            st.len()
                        );
                    }
                }
                9 => {
                    let st = style_of(&doc, id).fill.stops;
                    if !st.is_empty() {
                        let s = st[rng.pick(st.len())];
                        doc.edit_stops(&[StopEdit {
                            id,
                            stop: s.id,
                            change: StopChange::Position(pos(rng.unit())),
                        }])
                        .unwrap();
                    }
                }
                10 => doc
                    .edit_style(
                        &group,
                        &StyleEdit::StrokeJoin(
                            [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel][rng.pick(3)],
                        ),
                    )
                    .unwrap(),
                _ => doc
                    .edit_style(
                        &group,
                        &StyleEdit::StrokeCap(
                            [LineCap::Butt, LineCap::Round, LineCap::Square][rng.pick(3)],
                        ),
                    )
                    .unwrap(),
            }
            for i in ids {
                let s = style_of(&doc, i);
                assert!(s.stroke.width.as_mm().is_finite() && s.stroke.width.as_mm() > 0.0);
                assert!(s.fill.stops.len() <= 16, "step {step}");
            }
            if step % 50 == 49 {
                let back = reopen(&doc);
                for i in ids {
                    assert_eq!(
                        style_of(&back, i),
                        style_of(&doc, i),
                        "seed {seed} step {step}"
                    );
                }
            }
        }
    }
}
