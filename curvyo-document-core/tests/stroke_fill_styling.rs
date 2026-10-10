//! Acceptance tests for the model part of `stroke-and-fill-styling`
//! (`specs/0007-stroke-and-fill-styling/specification.md`, PR 1): storage and
//! commands for criteria 2 to 6, 9, 13, 16 to 20, 24, 25 and 30 to 33.
//! Rendering, hit-testing and the panel are later PRs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use std::io::{Cursor, Write};

use curvyo_document_core::{
    AnchorId, AnchorKind, CURRENT_FORMAT_VERSION, Color, CopySource, DashPattern, Document,
    EllipseFrame, Length, LineCap, LineJoin, NewAnchor, NodeId, ObjectSnapshot, Opacity,
    PathEditError, Point, PointCount, RectBounds, Shape, ShapeEditError, StarFrame, Style,
    StyleEdit, Vec2, pack, unpack,
};
use loro::LoroDoc;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn red() -> Color {
    Color { r: 255, g: 0, b: 0 }
}

fn blue() -> Color {
    Color { r: 0, g: 0, b: 255 }
}

fn opacity(v: f64) -> Opacity {
    Opacity::new(v).unwrap()
}

fn rect(document: &Document) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(10.0),
        height: mm(5.0),
    })
}

fn ellipse(document: &Document) -> NodeId {
    document.create_ellipse(EllipseFrame {
        center: pt(30.0, 30.0),
        rx: mm(4.0),
        ry: mm(3.0),
    })
}

fn star(document: &Document) -> NodeId {
    document.create_star(
        StarFrame {
            center: pt(60.0, 60.0),
            radius: mm(8.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    )
}

fn open_path(document: &Document) -> NodeId {
    document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(10.0, 10.0)),
        ],
        false,
    )
}

fn style_of(document: &Document, id: NodeId) -> Style {
    match document.object(id).expect("object exists") {
        ObjectSnapshot::Path(p) => p.style,
        ObjectSnapshot::Primitive(p) => p.style,
    }
}

fn reopen(document: &Document) -> Document {
    unpack(9, &pack(document, "test").unwrap()).unwrap()
}

/// Two replicas of one document, as two peers that opened the same file.
fn two_peers(setup: impl FnOnce(&Document) -> NodeId) -> (Document, Document, NodeId) {
    let base = Document::new(1);
    let id = setup(&base);
    let bytes = pack(&base, "test").unwrap();
    (unpack(2, &bytes).unwrap(), unpack(3, &bytes).unwrap(), id)
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
        "app_version": "merge",
    });
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("document.loro", loro_bytes),
        ("document.json", b"{}".to_vec()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    unpack(9, &writer.finish().unwrap().into_inner()).expect("merged document opens")
}

// ---------------------------------------------------------------------
// AC 1, 2, 3: one property set; shape parameters untouched; defaults
// ---------------------------------------------------------------------

#[test]
fn ac3_every_object_kind_is_created_with_the_frozen_default_style() {
    let document = Document::new(1);
    let ids = [
        rect(&document),
        ellipse(&document),
        star(&document),
        document.create_polygon(
            StarFrame {
                center: pt(0.0, 0.0),
                radius: mm(5.0),
                angle: curvyo_document_core::Angle::from_radians(0.0),
            },
            PointCount::new(3).unwrap(),
        ),
        open_path(&document),
    ];
    for id in ids {
        let style = style_of(&document, id);
        assert_eq!(style, Style::default());
        assert!(style.stroke.enabled);
        assert_eq!(style.stroke.width, mm(0.25));
        assert_eq!(style.stroke.color, Color::BLACK);
        assert!(!style.fill.enabled);
    }
}

// ---------------------------------------------------------------------
// AC 4, 5: width and on/off
// ---------------------------------------------------------------------

#[test]
fn ac4_a_width_is_stored_and_survives_reselecting_and_reopening() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(1.5)))
        .unwrap();
    assert_eq!(style_of(&document, id).stroke.width, mm(1.5));
    assert_eq!(style_of(&reopen(&document), id).stroke.width, mm(1.5));
}

#[test]
fn ac5_width_zero_switches_the_stroke_off_and_keeps_every_stored_value() {
    let document = Document::new(1);
    let id = open_path(&document);
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(3.0)))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeColor(red()))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeJoin(LineJoin::Round))
        .unwrap();
    let before = style_of(&document, id);

    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
        .unwrap();
    let off = style_of(&document, id);
    assert!(!off.stroke.enabled);
    assert_eq!(off.stroke.width, mm(3.0), "the last positive width stays");
    assert_eq!(off.stroke.color, red());
    assert_eq!(off.stroke.join, LineJoin::Round);

    // Turning it back on restores everything unchanged.
    document
        .edit_style(&[id], &StyleEdit::StrokeEnabled(true))
        .unwrap();
    assert_eq!(style_of(&document, id), before);
}

#[test]
fn ac5_the_paint_switch_off_keeps_the_values_and_colour_or_width_turn_it_on_again() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeCap(LineCap::Round))
        .unwrap();

    document
        .edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let off = style_of(&document, id);
    assert!(!off.stroke.enabled);
    assert_eq!(
        (off.stroke.width, off.stroke.cap),
        (mm(2.0), LineCap::Round)
    );

    // A colour edit turns the stroke on in the same commit; the other values
    // are unchanged.
    document
        .edit_style(&[id], &StyleEdit::StrokeColor(blue()))
        .unwrap();
    let on = style_of(&document, id);
    assert!(on.stroke.enabled);
    assert_eq!(on.stroke.color, blue());
    assert_eq!((on.stroke.width, on.stroke.cap), (mm(2.0), LineCap::Round));

    // So does a non-zero width.
    document
        .edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.5)))
        .unwrap();
    let again = style_of(&document, id);
    assert!(again.stroke.enabled);
    assert_eq!(again.stroke.width, mm(0.5));
    assert_eq!(again.stroke.color, blue());
}

#[test]
fn ac5_dash_join_and_cap_edits_do_not_switch_a_stroke_on() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeJoin(LineJoin::Bevel))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeCap(LineCap::Square))
        .unwrap();
    assert!(!style_of(&document, id).stroke.enabled);
}

// ---------------------------------------------------------------------
// AC 6, 9: colour, opacity, dash
// ---------------------------------------------------------------------

#[test]
fn ac6_colour_and_opacity_are_independent_and_read_back_unchanged() {
    let document = Document::new(1);
    let id = ellipse(&document);
    document
        .edit_style(&[id], &StyleEdit::StrokeColor(red()))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeOpacity(opacity(0.37)))
        .unwrap();
    let style = style_of(&reopen(&document), id);
    assert_eq!(style.stroke.color, red());
    assert_eq!(style.stroke.opacity.get(), 0.37);
    document
        .edit_style(&[id], &StyleEdit::StrokeColor(blue()))
        .unwrap();
    assert_eq!(style_of(&document, id).stroke.opacity.get(), 0.37);
}

#[test]
fn ac6_a_percent_is_stored_as_exactly_n_over_100() {
    let document = Document::new(1);
    let id = rect(&document);
    for percent in 0..=100u32 {
        let value = f64::from(percent) / 100.0;
        document
            .edit_style(&[id], &StyleEdit::FillOpacity(opacity(value)))
            .unwrap();
        let stored = style_of(&reopen(&document), id).fill.opacity.get();
        assert_eq!(stored, value);
        assert_eq!((stored * 100.0).round() as u32, percent);
    }
}

#[test]
fn ac9_a_dash_pattern_is_an_unlimited_ordered_list_of_width_multiples() {
    let document = Document::new(1);
    let id = rect(&document);
    let long: Vec<f64> = (1..=40).map(f64::from).collect();
    for lengths in [vec![], vec![6.0, 4.0], vec![6.0, 3.0, 1.0, 3.0], long] {
        let pattern = DashPattern::new(lengths.clone()).unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeDash(pattern))
            .unwrap();
        let stored = style_of(&reopen(&document), id).stroke.dash;
        assert_eq!(stored.as_slice(), lengths.as_slice());
    }
    // A width change writes only the width: the dash list is untouched, so
    // the pattern rescales with it.
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(4.0)))
        .unwrap();
    assert_eq!(style_of(&document, id).stroke.dash.as_slice().len(), 40);
}

#[test]
fn join_and_cap_are_stored_and_read_back() {
    let document = Document::new(1);
    let id = open_path(&document);
    for (join, cap) in [
        (LineJoin::Round, LineCap::Round),
        (LineJoin::Bevel, LineCap::Square),
        (LineJoin::Miter, LineCap::Butt),
    ] {
        document
            .edit_style(&[id], &StyleEdit::StrokeJoin(join))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeCap(cap))
            .unwrap();
        let style = style_of(&reopen(&document), id);
        assert_eq!((style.stroke.join, style.stroke.cap), (join, cap));
    }
}

// ---------------------------------------------------------------------
// AC 24: one property, each object independently
// ---------------------------------------------------------------------

#[test]
fn ac24_a_batch_changes_one_property_and_leaves_every_other_alone() {
    let document = Document::new(1);
    let (a, b, c) = (open_path(&document), rect(&document), star(&document));
    document
        .edit_style(&[a], &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    document
        .edit_style(&[b], &StyleEdit::StrokeJoin(LineJoin::Round))
        .unwrap();
    document
        .edit_style(
            &[c],
            &StyleEdit::StrokeDash(DashPattern::new(vec![1.0, 3.0]).unwrap()),
        )
        .unwrap();
    let before: Vec<Style> = [a, b, c]
        .iter()
        .map(|id| style_of(&document, *id))
        .collect();

    document
        .edit_style(&[a, b, c], &StyleEdit::StrokeColor(red()))
        .unwrap();
    for (id, old) in [a, b, c].iter().zip(&before) {
        let mut expected = old.clone();
        expected.stroke.color = red();
        assert_eq!(style_of(&document, *id), expected);
    }
}

// ---------------------------------------------------------------------
// AC 25: persistence
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// AC 30 to 33: copy, split, join, object to path
// ---------------------------------------------------------------------

fn style_everything(document: &Document, id: NodeId) {
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillColor(blue()))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeWidth(mm(1.5)))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeColor(red()))
        .unwrap();
    document
        .edit_style(
            &[id],
            &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
        )
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::StrokeCap(LineCap::Round))
        .unwrap();
}

#[test]
fn ac31_splitting_an_open_path_gives_both_halves_the_same_style() {
    let document = Document::new(1);
    let path = open_path(&document);
    style_everything(&document, path);
    let before = style_of(&document, path);
    let ((first, _), (second, _)) = document
        .split_at_anchor(path, AnchorId::new(1, 2), AnchorId::new(9, 1))
        .unwrap();
    assert_eq!(style_of(&document, first), before);
    assert_eq!(style_of(&document, second), before);
    // Independent from here on.
    document
        .edit_style(&[second], &StyleEdit::StrokeColor(blue()))
        .unwrap();
    assert_eq!(style_of(&document, first).stroke.color, red());
}

#[test]
fn ac31_splitting_a_closed_path_keeps_the_object_and_its_style() {
    let document = Document::new(1);
    let path = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(10.0, 10.0)),
        ],
        true,
    );
    style_everything(&document, path);
    let before = style_of(&document, path);
    let ((same, _), _) = document
        .split_at_anchor(path, AnchorId::new(1, 2), AnchorId::new(9, 1))
        .unwrap();
    assert_eq!(same, path);
    assert_eq!(style_of(&document, path), before);
}

#[test]
fn ac32_a_join_keeps_the_surviving_paths_style_and_drops_the_other() {
    let document = Document::new(1);
    let a = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
        ],
        false,
    );
    let b = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 3), pt(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 4), pt(20.0, 0.0)),
        ],
        false,
    );
    style_everything(&document, b);
    let a_style = style_of(&document, a);
    assert!(!a_style.fill.enabled);
    let (survivor, _) = document
        .join_endpoints(a, AnchorId::new(1, 2), b, AnchorId::new(1, 3))
        .unwrap();
    assert_eq!(survivor, a);
    assert_eq!(
        style_of(&document, a),
        a_style,
        "the unfilled path stays unfilled"
    );
    assert!(document.object(b).is_none());
}

#[test]
fn ac32_closing_a_path_onto_itself_keeps_its_style() {
    let document = Document::new(1);
    let path = open_path(&document);
    style_everything(&document, path);
    let before = style_of(&document, path);
    document
        .join_endpoints(path, AnchorId::new(1, 1), path, AnchorId::new(1, 3))
        .unwrap();
    assert!(document.path(path).unwrap().closed);
    assert_eq!(style_of(&document, path), before);
}

// ---------------------------------------------------------------------
// Resize: the width goes through the style
// ---------------------------------------------------------------------

#[test]
fn a_resize_with_a_width_writes_it_and_a_resize_without_one_leaves_the_style_alone() {
    let document = Document::new(1);
    let id = rect(&document);
    style_everything(&document, id);
    let before = style_of(&document, id);
    let bounds = RectBounds {
        origin: pt(0.0, 0.0),
        width: mm(20.0),
        height: mm(10.0),
    };
    let radii = curvyo_document_core::CornerRadii::uniform(mm(0.0));
    document.resize_rect(id, bounds, radii, None).unwrap();
    assert_eq!(style_of(&document, id), before);
    document
        .resize_rect(id, bounds, radii, Some(mm(3.0)))
        .unwrap();
    let after = style_of(&document, id);
    assert_eq!(after.stroke.width, mm(3.0));
    assert_eq!(
        after.stroke.dash, before.stroke.dash,
        "dashes follow as ratios"
    );
}

#[test]
fn a_resize_with_a_width_that_is_not_above_zero_is_refused_and_writes_nothing() {
    let document = Document::new(1);
    let (shape, e, s, path) = (
        rect(&document),
        ellipse(&document),
        star(&document),
        open_path(&document),
    );
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let width = Some(mm(bad));
        let bounds = RectBounds {
            origin: pt(0.0, 0.0),
            width: mm(20.0),
            height: mm(10.0),
        };
        assert_eq!(
            document.resize_rect(
                shape,
                bounds,
                curvyo_document_core::CornerRadii::uniform(mm(0.0)),
                width
            ),
            Err(ShapeEditError::InvalidStrokeWidth)
        );
        assert_eq!(
            document.resize_ellipse(
                e,
                EllipseFrame {
                    center: pt(0.0, 0.0),
                    rx: mm(9.0),
                    ry: mm(9.0)
                },
                width
            ),
            Err(ShapeEditError::InvalidStrokeWidth)
        );
        assert_eq!(
            document.resize_star_frame(
                s,
                StarFrame {
                    center: pt(0.0, 0.0),
                    radius: mm(9.0),
                    angle: curvyo_document_core::Angle::from_radians(0.0)
                },
                width
            ),
            Err(ShapeEditError::InvalidStrokeWidth)
        );
        assert_eq!(
            document.resize_path(
                path,
                &[(AnchorId::new(1, 1), pt(1.0, 1.0), Vec2::ZERO, Vec2::ZERO)],
                width
            ),
            Err(PathEditError::InvalidStrokeWidth)
        );
    }
    assert_eq!(document.primitive(shape).unwrap().shape, {
        let ObjectSnapshot::Primitive(p) = document.object(shape).unwrap() else {
            panic!()
        };
        p.shape
    });
    assert_eq!(document.path(path).unwrap().anchors[0].point, pt(0.0, 0.0));
}

// ---------------------------------------------------------------------
// Peers: separate registers merge without loss
// ---------------------------------------------------------------------

#[test]
fn two_peers_editing_different_style_properties_both_survive_the_merge() {
    let (a, b, id) = two_peers(rect);
    a.edit_style(&[id], &StyleEdit::StrokeColor(red())).unwrap();
    b.edit_style(
        &[id],
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    )
    .unwrap();
    let merged = merged(&a, &b);
    let style = style_of(&merged, id);
    assert_eq!(style.stroke.color, red());
    assert_eq!(style.stroke.dash.as_slice(), [6.0, 4.0]);
}

#[test]
fn a_peer_turning_the_stroke_off_does_not_lose_a_concurrent_colour_edit() {
    let (a, b, id) = two_peers(rect);
    a.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    // B's colour edit sees the stroke still on, so it writes only the colour.
    b.edit_style(&[id], &StyleEdit::StrokeColor(blue()))
        .unwrap();
    let style = style_of(&merged(&a, &b), id);
    assert!(!style.stroke.enabled);
    assert_eq!(style.stroke.color, blue());
}

// ---- more fill and copy cases

#[test]
fn ac1_ac2_the_same_edit_works_on_paths_and_primitives_and_keeps_shape_parameters() {
    let document = Document::new(1);
    let path = open_path(&document);
    let primitives = [rect(&document), ellipse(&document), star(&document)];
    let shapes_before: Vec<Shape> = primitives
        .iter()
        .map(|id| document.primitive(*id).unwrap().shape)
        .collect();
    let anchors_before = document.path(path).unwrap().anchors;
    let all = [path, primitives[0], primitives[1], primitives[2]];

    document
        .edit_style(&all, &StyleEdit::StrokeWidth(mm(2.0)))
        .unwrap();
    document
        .edit_style(
            &all,
            &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
        )
        .unwrap();
    document
        .edit_style(&all, &StyleEdit::FillEnabled(true))
        .unwrap();

    for id in all {
        let style = style_of(&document, id);
        assert_eq!(style.stroke.width, mm(2.0));
        assert_eq!(style.stroke.dash.as_slice(), [6.0, 4.0]);
        assert!(style.fill.enabled);
    }
    // No implicit "object to path": still the same primitives, same shapes.
    for (id, before) in primitives.iter().zip(&shapes_before) {
        assert_eq!(document.primitive(*id).unwrap().shape, *before);
        assert!(document.path(*id).is_none(), "still a primitive");
    }
    assert_eq!(document.path(path).unwrap().anchors, anchors_before);
}

#[test]
fn ac13_fill_none_keeps_colour_and_re_enabling_restores_them() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::FillColor(red()))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(false))
        .unwrap();
    let none = style_of(&document, id);
    assert!(!none.fill.enabled);
    assert_eq!(none.fill.color, red());
    document
        .edit_style(&[id], &StyleEdit::FillEnabled(true))
        .unwrap();
    let solid = style_of(&document, id);
    assert!(solid.fill.enabled);
    assert_eq!(solid.fill.color, red());
}

#[test]
fn ac25_every_property_reads_back_exactly() {
    let document = Document::new(1);
    let path = open_path(&document);
    let shape = rect(&document);
    for id in [path, shape] {
        document
            .edit_style(&[id], &StyleEdit::FillEnabled(true))
            .unwrap();
        document
            .edit_style(
                &[id],
                &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 3.0, 1.0, 3.0]).unwrap()),
            )
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeOpacity(opacity(0.6)))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeJoin(LineJoin::Bevel))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeCap(LineCap::Round))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::FillColor(blue()))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::FillOpacity(opacity(0.25)))
            .unwrap();
        document
            .edit_style(&[id], &StyleEdit::StrokeWidth(mm(0.0)))
            .unwrap();
    }
    let reopened = reopen(&document);
    for id in [path, shape] {
        let style = style_of(&document, id);
        assert_eq!(style_of(&reopened, id), style);
        assert!(!style.stroke.enabled);
        assert!(style.fill.enabled);
    }
}

#[test]
fn ac30_a_copy_has_the_same_style_and_is_independent() {
    let document = Document::new(1);
    let path = open_path(&document);
    let shape = rect(&document);
    style_everything(&document, path);
    style_everything(&document, shape);
    let copies = document
        .duplicate_objects(
            &[
                CopySource {
                    id: path,
                    anchor_ids: vec![
                        AnchorId::new(5, 1),
                        AnchorId::new(5, 2),
                        AnchorId::new(5, 3),
                    ],
                },
                CopySource {
                    id: shape,
                    anchor_ids: vec![],
                },
            ],
            Vec2::new(5.0, 5.0),
        )
        .unwrap();
    for (original, copy) in [path, shape].into_iter().zip(&copies) {
        assert_eq!(style_of(&document, *copy), style_of(&document, original));
        // Editing the copy's fill leaves the original alone.
        document
            .edit_style(&[*copy], &StyleEdit::FillColor(red()))
            .unwrap();
        document
            .edit_style(&[*copy], &StyleEdit::StrokeJoin(LineJoin::Bevel))
            .unwrap();
        assert_eq!(style_of(&document, original).fill.color, blue());
        assert_eq!(style_of(&document, original).stroke.join, LineJoin::Miter);
        // And the reverse.
        document
            .edit_style(&[original], &StyleEdit::FillOpacity(opacity(0.9)))
            .unwrap();
        assert_eq!(style_of(&document, *copy).fill.opacity.get(), 1.0);
    }
}

#[test]
fn ac33_object_to_path_keeps_the_whole_style() {
    let document = Document::new(1);
    let shape = rect(&document);
    style_everything(&document, shape);
    let before = style_of(&document, shape);
    let anchors: Vec<NewAnchor> = (0..4)
        .map(|i| NewAnchor {
            kind: AnchorKind::Corner,
            ..NewAnchor::corner(AnchorId::new(4, i + 1), pt(f64::from(i as u32), 0.0))
        })
        .collect();
    document.convert_to_paths(&[(shape, anchors)]).unwrap();
    let path = document.path(shape).expect("now a path");
    assert_eq!(path.style, before);
    assert!(path.style.fill.enabled);
}
