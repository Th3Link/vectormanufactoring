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
    EllipseFrame, FillKind, FillMode, FillModeTarget, GradientStop, Length, LineCap, LineJoin,
    MAX_GRADIENT_STOPS, NewAnchor, NodeId, ObjectSnapshot, Opacity, PathEditError, Point,
    PointCount, RectBounds, Shape, ShapeEditError, StarFrame, StopChange, StopEdit, StopId,
    StopPosition, Style, StyleEdit, StyleEditError, Vec2, pack, unpack,
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

fn position(v: f64) -> StopPosition {
    StopPosition::new(v).unwrap()
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

fn stop(counter: u64, at: f64, color: Color, alpha: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(7, counter),
        position: position(at),
        color,
        opacity: opacity(alpha),
    }
}

fn set_mode(document: &Document, mode: FillMode, id: NodeId, seed: &[GradientStop]) {
    document
        .set_fill_mode(
            mode,
            &[FillModeTarget {
                id,
                seed_stops: seed.to_vec(),
            }],
        )
        .unwrap();
}

/// A gradient fill with stops at 0, 0.5, 1 on `id`.
fn linear_with_three_stops(document: &Document, id: NodeId) -> [GradientStop; 3] {
    let stops = [
        stop(1, 0.0, red(), 1.0),
        stop(2, 0.5, Color::BLACK, 0.5),
        stop(3, 1.0, blue(), 1.0),
    ];
    set_mode(document, FillMode::Linear, id, &stops[..2]);
    document.add_stop(id, stops[2]).unwrap();
    document
        .edit_stops(&[StopEdit {
            id,
            stop: stops[1].id,
            change: StopChange::Position(position(0.5)),
        }])
        .unwrap();
    stops
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
    set_mode(&document, FillMode::Solid, path, &[]);
    for id in primitives {
        set_mode(&document, FillMode::Solid, id, &[]);
    }

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
// AC 13, 14: fill modes keep what they do not use
// ---------------------------------------------------------------------

#[test]
fn ac13_fill_none_keeps_colour_and_stops_and_re_enabling_restores_them() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::FillColor(red()))
        .unwrap();
    set_mode(&document, FillMode::Solid, id, &[]);
    set_mode(&document, FillMode::None, id, &[]);
    let none = style_of(&document, id);
    assert!(!none.fill.enabled);
    assert_eq!(none.fill.color, red());
    set_mode(&document, FillMode::Solid, id, &[]);
    let solid = style_of(&document, id);
    assert!(solid.fill.enabled);
    assert_eq!(
        (solid.fill.kind, solid.fill.color),
        (FillKind::Solid, red())
    );
}

#[test]
fn ac13_solid_to_linear_and_back_loses_neither_the_colour_nor_the_stops() {
    let document = Document::new(1);
    let id = rect(&document);
    document
        .edit_style(&[id], &StyleEdit::FillColor(red()))
        .unwrap();
    document
        .edit_style(&[id], &StyleEdit::FillOpacity(opacity(0.4)))
        .unwrap();
    set_mode(&document, FillMode::Solid, id, &[]);
    let stops = GradientStop::default_pair(red(), StopId::new(7, 1), StopId::new(7, 2));
    set_mode(&document, FillMode::Linear, id, &stops);
    set_mode(&document, FillMode::Solid, id, &[]);
    let back = style_of(&reopen(&document), id);
    assert_eq!(back.fill.kind, FillKind::Solid);
    assert_eq!(back.fill.color, red());
    assert_eq!(back.fill.opacity.get(), 0.4);
    assert_eq!(back.fill.stops, stops);
}

#[test]
fn ac17_a_gradient_without_stops_gets_the_seed_and_one_with_stops_keeps_them() {
    let document = Document::new(1);
    let (a, b) = (rect(&document), ellipse(&document));
    document
        .edit_style(&[a], &StyleEdit::FillColor(red()))
        .unwrap();
    let seeds = [
        FillModeTarget {
            id: a,
            seed_stops: GradientStop::default_pair(red(), StopId::new(7, 1), StopId::new(7, 2))
                .to_vec(),
        },
        FillModeTarget {
            id: b,
            seed_stops: GradientStop::default_pair(
                Color::BLACK,
                StopId::new(7, 3),
                StopId::new(7, 4),
            )
            .to_vec(),
        },
    ];
    document.set_fill_mode(FillMode::Radial, &seeds).unwrap();
    let (sa, sb) = (style_of(&document, a), style_of(&document, b));
    assert_eq!(sa.fill.kind, FillKind::Radial);
    assert_eq!(sa.fill.stops.len(), 2);
    assert_eq!(sa.fill.stops[0].color, red());
    assert_eq!(
        sa.fill.stops[1].color,
        Color {
            r: 255,
            g: 255,
            b: 255
        }
    );
    assert_eq!(sb.fill.stops[0].color, Color::BLACK, "each takes its own");

    // Switching kind keeps the stops; a second seed is ignored.
    document.set_fill_mode(FillMode::Linear, &seeds).unwrap();
    assert_eq!(style_of(&document, a).fill.stops, sa.fill.stops);
    assert_eq!(style_of(&document, a).fill.kind, FillKind::Linear);
}

// ---------------------------------------------------------------------
// AC 16 to 20: stops
// ---------------------------------------------------------------------

#[test]
fn ac18_a_stop_is_inserted_in_position_order_and_ties_go_after_equals() {
    let document = Document::new(1);
    let id = rect(&document);
    let base = GradientStop::default_pair(red(), StopId::new(7, 1), StopId::new(7, 2));
    set_mode(&document, FillMode::Linear, id, &base);
    document.add_stop(id, stop(3, 0.5, blue(), 1.0)).unwrap();
    document
        .add_stop(id, stop(4, 0.5, Color::BLACK, 1.0))
        .unwrap();
    document.add_stop(id, stop(5, 0.0, blue(), 0.5)).unwrap();
    document.add_stop(id, stop(6, 1.0, blue(), 0.5)).unwrap();
    let order: Vec<u64> = style_of(&document, id)
        .fill
        .stops
        .iter()
        .map(|s| (s.id.as_u128() & 0xffff_ffff) as u64)
        .collect();
    // Stop 5 joins position 0 after stop 1; stops 3 and 4 sit at 0.5 in the
    // order they were added; stop 6 joins position 1 after stop 2.
    assert_eq!(order, [1, 5, 3, 4, 2, 6]);
}

#[test]
fn ac18_a_17th_stop_is_refused_and_a_file_may_hold_more() {
    let document = Document::new(1);
    let id = rect(&document);
    let base = GradientStop::default_pair(red(), StopId::new(7, 1), StopId::new(7, 2));
    set_mode(&document, FillMode::Linear, id, &base);
    for n in 3..=MAX_GRADIENT_STOPS as u64 {
        document
            .add_stop(id, stop(n, n as f64 / 20.0, blue(), 1.0))
            .unwrap();
    }
    assert_eq!(style_of(&document, id).fill.stops.len(), MAX_GRADIENT_STOPS);
    assert_eq!(
        document.add_stop(id, stop(99, 0.5, blue(), 1.0)),
        Err(StyleEditError::TooManyStops)
    );
    assert_eq!(style_of(&document, id).fill.stops.len(), MAX_GRADIENT_STOPS);
}

#[test]
fn ac19_a_stop_can_be_removed_down_to_two_and_not_below() {
    let document = Document::new(1);
    let id = rect(&document);
    let stops = linear_with_three_stops(&document, id);
    document.remove_stop(id, stops[1].id).unwrap();
    let left = style_of(&document, id).fill.stops;
    assert_eq!(left, [stops[0], stops[2]]);
    assert_eq!(
        document.remove_stop(id, stops[0].id),
        Err(StyleEditError::TooFewStops)
    );
    assert_eq!(style_of(&document, id).fill.stops, left);
}

#[test]
fn ac20_editing_one_stop_changes_only_that_value() {
    let document = Document::new(1);
    let id = rect(&document);
    let stops = linear_with_three_stops(&document, id);
    let edit = |stop: StopId, change| {
        document
            .edit_stops(&[StopEdit { id, stop, change }])
            .unwrap();
    };

    edit(stops[1].id, StopChange::Color(red()));
    let after = style_of(&document, id).fill.stops;
    assert_eq!(after[0], stops[0]);
    assert_eq!(after[2], stops[2]);
    assert_eq!(
        after[1],
        GradientStop {
            color: red(),
            ..stops[1]
        }
    );

    edit(stops[1].id, StopChange::Opacity(opacity(0.25)));
    edit(stops[1].id, StopChange::Position(position(0.8)));
    let after = style_of(&document, id).fill.stops;
    assert_eq!(after[0], stops[0]);
    assert_eq!(after[2], stops[2]);
    assert_eq!(
        after[1],
        GradientStop {
            color: red(),
            opacity: opacity(0.25),
            position: position(0.8),
            ..stops[1]
        }
    );
    // A position edit does not move the stop within the list; the renderer
    // sorts. Here the edited stop now sits after the stop at 1.0 in order
    // of position but keeps its list slot.
    assert_eq!(
        after.iter().map(|s| s.id).collect::<Vec<_>>(),
        stops.iter().map(|s| s.id).collect::<Vec<_>>()
    );
}

#[test]
fn ac34_a_multi_object_stop_edit_applies_each_objects_own_stop_in_one_batch() {
    let document = Document::new(1);
    let (a, b) = (rect(&document), ellipse(&document));
    let seeds = [
        FillModeTarget {
            id: a,
            seed_stops: GradientStop::default_pair(red(), StopId::new(7, 1), StopId::new(7, 2))
                .to_vec(),
        },
        FillModeTarget {
            id: b,
            seed_stops: GradientStop::default_pair(red(), StopId::new(8, 1), StopId::new(8, 2))
                .to_vec(),
        },
    ];
    document.set_fill_mode(FillMode::Linear, &seeds).unwrap();
    document
        .edit_stops(&[
            StopEdit {
                id: a,
                stop: StopId::new(7, 2),
                change: StopChange::Color(blue()),
            },
            StopEdit {
                id: b,
                stop: StopId::new(8, 2),
                change: StopChange::Color(blue()),
            },
        ])
        .unwrap();
    for id in [a, b] {
        let stops = style_of(&document, id).fill.stops;
        assert_eq!(stops[0].color, red());
        assert_eq!(stops[1].color, blue());
    }
    // A stale stop refuses the whole batch.
    let bad = document.edit_stops(&[
        StopEdit {
            id: a,
            stop: StopId::new(7, 1),
            change: StopChange::Color(Color::BLACK),
        },
        StopEdit {
            id: b,
            stop: StopId::new(9, 9),
            change: StopChange::Color(Color::BLACK),
        },
    ]);
    assert_eq!(bad, Err(StyleEditError::NoSuchStop));
    assert_eq!(style_of(&document, a).fill.stops[0].color, red());
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

#[test]
fn ac25_every_property_including_the_ordered_stops_reads_back_exactly() {
    let document = Document::new(1);
    let path = open_path(&document);
    let shape = rect(&document);
    for id in [path, shape] {
        linear_with_three_stops(&document, id);
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
        // Two stops at the same position keep the order they were made in.
        document.add_stop(id, stop(10, 0.5, red(), 1.0)).unwrap();
    }
    let reopened = reopen(&document);
    for id in [path, shape] {
        let style = style_of(&document, id);
        assert_eq!(style_of(&reopened, id), style);
        assert!(!style.stroke.enabled);
        assert_eq!(style.fill.stops.len(), 4);
    }
}

// ---------------------------------------------------------------------
// AC 30 to 33: copy, split, join, object to path
// ---------------------------------------------------------------------

fn style_everything(document: &Document, id: NodeId) {
    linear_with_three_stops(document, id);
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
fn ac30_a_copy_has_the_same_style_and_stops_and_is_independent() {
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
        let stop_id = style_of(&document, original).fill.stops[1].id;
        // Editing the copy's stop leaves the original alone.
        document
            .edit_stops(&[StopEdit {
                id: *copy,
                stop: stop_id,
                change: StopChange::Color(blue()),
            }])
            .unwrap();
        document
            .edit_style(&[*copy], &StyleEdit::StrokeJoin(LineJoin::Bevel))
            .unwrap();
        assert_eq!(
            style_of(&document, original).fill.stops[1].color,
            Color::BLACK
        );
        assert_eq!(style_of(&document, original).stroke.join, LineJoin::Miter);
        // And the reverse.
        document
            .edit_stops(&[StopEdit {
                id: original,
                stop: stop_id,
                change: StopChange::Opacity(opacity(0.9)),
            }])
            .unwrap();
        assert_eq!(style_of(&document, *copy).fill.stops[1].opacity.get(), 0.5);
    }
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

#[test]
fn ac33_object_to_path_keeps_the_whole_style_including_the_stops() {
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
    assert_eq!(path.style.fill.stops.len(), 3);
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

#[test]
fn two_peers_editing_different_stops_of_one_gradient_both_survive_the_merge() {
    let (a, b, id) = two_peers(|document| {
        let id = rect(document);
        linear_with_three_stops(document, id);
        id
    });
    a.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(7, 1),
        change: StopChange::Color(Color::BLACK),
    }])
    .unwrap();
    b.edit_stops(&[StopEdit {
        id,
        stop: StopId::new(7, 3),
        change: StopChange::Position(position(0.9)),
    }])
    .unwrap();
    b.add_stop(id, stop(40, 0.2, red(), 1.0)).unwrap();
    let stops = style_of(&merged(&a, &b), id).fill.stops;
    assert_eq!(stops.len(), 4);
    let find = |counter: u64| {
        stops
            .iter()
            .find(|s| s.id == StopId::new(7, counter))
            .unwrap()
    };
    assert_eq!(find(1).color, Color::BLACK);
    assert_eq!(find(3).position.get(), 0.9);
    assert_eq!(find(40).position.get(), 0.2);
}

#[test]
fn two_peers_removing_different_stops_leave_a_one_stop_gradient_that_still_opens() {
    let (a, b, id) = two_peers(|document| {
        let id = rect(document);
        linear_with_three_stops(document, id);
        id
    });
    a.remove_stop(id, StopId::new(7, 1)).unwrap();
    b.remove_stop(id, StopId::new(7, 3)).unwrap();
    let style = style_of(&merged(&a, &b), id);
    assert_eq!(
        style.fill.stops.len(),
        1,
        "a stored count outside 2 to 16 is valid"
    );
}
