//! The stop editor's state and the rules of its commands
//! (`specs/0007-stroke-and-fill-styling` criteria 16 to 20, 34, 35).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Color, Document, EllipseFrame, FillMode, FillModeTarget, GradientStop, Length, NodeId,
    ObjectSnapshot, Opacity, Point, PointCount, RectBounds, StarFrame, StopChange, StopId,
    StopPosition,
};
use curvyo_ui_core::{
    AnchorIdMinter, BarValue, StopsPanel, fill_targets, new_stop_values, selected_rank, stop_edits,
    stop_targets, stops_panel,
};

fn rect(document: &Document, x: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn red() -> Color {
    Color { r: 255, g: 0, b: 0 }
}

fn blue() -> Color {
    Color { r: 0, g: 0, b: 255 }
}

fn stop(counter: u64, at: f64, color: Color, alpha: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(7, counter),
        position: StopPosition::new(at).unwrap(),
        color,
        opacity: Opacity::new(alpha).unwrap(),
    }
}

fn set(document: &Document, id: NodeId, mode: FillMode, stops: &[GradientStop]) {
    document
        .set_fill_mode(
            mode,
            &[FillModeTarget {
                id,
                seed_stops: stops.to_vec(),
            }],
        )
        .unwrap();
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn two_stops() -> Vec<GradientStop> {
    vec![stop(1, 0.0, red(), 1.0), stop(2, 1.0, blue(), 1.0)]
}

fn editor(panel: StopsPanel) -> curvyo_ui_core::StopEditorView {
    match panel {
        StopsPanel::Editor(view) => view,
        other => panic!("expected the editor, got {other:?}"),
    }
}

// ---- which state the panel is in (criteria 34, 35) --------------------------

#[test]
fn the_editor_shows_for_a_gradient_and_nothing_else() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    assert!(matches!(
        stops_panel(&objects(&document), &[a]),
        StopsPanel::Hidden
    ));
    assert!(matches!(
        stops_panel(&objects(&document), &[]),
        StopsPanel::Hidden
    ));
    set(&document, a, FillMode::Solid, &[]);
    assert!(matches!(
        stops_panel(&objects(&document), &[a]),
        StopsPanel::Hidden
    ));
    set(&document, a, FillMode::Linear, &two_stops());
    assert_eq!(editor(stops_panel(&objects(&document), &[a])).rows.len(), 2);
    // A gradient switched off keeps its stops and shows no editor.
    set(&document, a, FillMode::None, &[]);
    assert!(matches!(
        stops_panel(&objects(&document), &[a]),
        StopsPanel::Hidden
    ));
    // Linear next to radial, or next to a solid fill: no state, no editor.
    set(&document, a, FillMode::Linear, &[]);
    set(&document, b, FillMode::Radial, &two_stops());
    assert!(matches!(
        stops_panel(&objects(&document), &[a, b]),
        StopsPanel::Hidden
    ));
}

#[test]
fn the_same_mode_with_different_counts_says_so() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    set(&document, a, FillMode::Linear, &two_stops());
    set(&document, b, FillMode::Linear, &two_stops());
    document.add_stop(b, stop(3, 0.5, red(), 1.0)).unwrap();
    assert!(matches!(
        stops_panel(&objects(&document), &[a, b]),
        StopsPanel::DifferentCounts
    ));
}

#[test]
fn zero_one_and_sixteen_plus_stops_are_states_not_errors() {
    let document = Document::new(1);
    let a = rect(&document, 0.0);
    set(&document, a, FillMode::Linear, &[]);
    let none = editor(stops_panel(&objects(&document), &[a]));
    assert_eq!(none.rows.len(), 0);
    assert!(none.can_add && !none.can_remove);

    set(&document, a, FillMode::Linear, &[stop(1, 0.3, red(), 1.0)]);
    let one = editor(stops_panel(&objects(&document), &[a]));
    assert_eq!(one.rows.len(), 1);
    assert!(one.can_add && !one.can_remove, "remove stops at two");

    for n in 2..=16 {
        document.add_stop(a, stop(n, 0.5, red(), 1.0)).unwrap();
    }
    let full = editor(stops_panel(&objects(&document), &[a]));
    assert_eq!(full.rows.len(), 16);
    assert!(!full.can_add && full.can_remove);
}

#[test]
fn several_objects_hide_add_and_remove() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    for id in [a, b] {
        set(&document, id, FillMode::Linear, &two_stops());
        document.add_stop(id, stop(3, 0.5, red(), 1.0)).unwrap();
    }
    let view = editor(stops_panel(&objects(&document), &[a, b]));
    assert_eq!(view.objects, 2);
    assert!(!view.can_add && !view.can_remove);
}

// ---- rows by rank, the bar (criterion 34) -------------------------------------

#[test]
fn a_row_is_the_stop_of_that_rank_and_differing_fields_read_mixed() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    set(&document, a, FillMode::Linear, &two_stops());
    set(
        &document,
        b,
        FillMode::Linear,
        &[stop(1, 0.0, red(), 1.0), stop(2, 0.5, blue(), 0.5)],
    );
    let view = editor(stops_panel(&objects(&document), &[a, b]));
    assert_eq!(view.rows[0].position, BarValue::Uniform(0.0));
    assert_eq!(view.rows[0].color, BarValue::Uniform(red()));
    assert_eq!(view.rows[1].position, BarValue::Mixed);
    assert_eq!(view.rows[1].color, BarValue::Uniform(blue()));
    assert_eq!(view.rows[1].opacity, BarValue::Mixed);
    assert!(view.bar.is_none(), "the lists differ: a hatched track");
}

#[test]
fn identical_lists_show_the_ramp_in_position_order() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    // Stored out of order, with different ids: equal in value.
    set(&document, a, FillMode::Linear, &[stop(1, 0.9, blue(), 1.0)]);
    document.add_stop(a, stop(2, 0.1, red(), 1.0)).unwrap();
    set(
        &document,
        b,
        FillMode::Linear,
        &[stop(5, 0.1, red(), 1.0), stop(6, 0.9, blue(), 1.0)],
    );
    let view = editor(stops_panel(&objects(&document), &[a, b]));
    let bar = view.bar.expect("identical in value");
    assert_eq!(bar.len(), 2);
    assert_eq!((bar[0].position, bar[0].color), (0.1, red()));
    assert_eq!((bar[1].position, bar[1].color), (0.9, blue()));
}

#[test]
fn a_polygon_or_star_in_the_selection_carries_the_box_note() {
    let document = Document::new(1);
    let a = rect(&document, 0.0);
    let star = document.create_polygon(
        StarFrame {
            center: Point::new(50.0, 50.0),
            radius: Length::from_mm(10.0),
            angle: curvyo_document_core::Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
    );
    let ellipse = document.create_ellipse(EllipseFrame {
        center: Point::new(0.0, 30.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(5.0),
    });
    for id in [a, star, ellipse] {
        set(&document, id, FillMode::Linear, &two_stops());
    }
    let with = editor(stops_panel(&objects(&document), &[a, star]));
    assert!(with.box_note);
    let without = editor(stops_panel(&objects(&document), &[a, ellipse]));
    assert!(!without.box_note);
}

// ---- seeding and adding (criteria 17, 18) --------------------------------------

#[test]
fn seeds_go_to_objects_without_stops_each_from_its_own_colour() {
    let document = Document::new(1);
    let (a, b, c) = (
        rect(&document, 0.0),
        rect(&document, 20.0),
        rect(&document, 40.0),
    );
    document
        .edit_style(&[a], &curvyo_document_core::StyleEdit::FillColor(red()))
        .unwrap();
    document
        .edit_style(&[b], &curvyo_document_core::StyleEdit::FillColor(blue()))
        .unwrap();
    set(&document, c, FillMode::Linear, &two_stops());
    let mut minter = AnchorIdMinter::new(4);
    let targets = fill_targets(&mut minter, &objects(&document), &[a, b, c]);
    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].seed_stops.len(), 2);
    assert_eq!(targets[0].seed_stops[0].color, red());
    assert_eq!(targets[1].seed_stops[0].color, blue());
    assert_eq!(targets[2].seed_stops.len(), 0, "c keeps its own stops");
    let mut ids: Vec<StopId> = targets[0]
        .seed_stops
        .iter()
        .chain(&targets[1].seed_stops)
        .map(|s| s.id)
        .collect();
    ids.sort_by_key(|id| id.as_u128());
    ids.dedup();
    assert_eq!(ids.len(), 4, "fresh ids");
}

fn fill_of(stops: Vec<GradientStop>, fill_color: Color) -> curvyo_document_core::Fill {
    curvyo_document_core::Fill {
        enabled: true,
        kind: curvyo_document_core::FillKind::Linear,
        color: fill_color,
        opacity: Opacity::OPAQUE,
        stops,
    }
}

#[test]
fn add_takes_the_midpoint_of_the_widest_gap_nearest_the_start() {
    let at =
        |stops: Vec<GradientStop>| new_stop_values(&fill_of(stops, red()), None).position.get();
    assert_eq!(at(two_stops()), 0.5);
    // Equal gaps: the first.
    assert_eq!(
        at(vec![
            stop(1, 0.0, red(), 1.0),
            stop(2, 0.5, red(), 1.0),
            stop(3, 1.0, red(), 1.0)
        ]),
        0.25
    );
    // The end gaps count: 0 to the first stop, the last stop to 1.
    assert_eq!(
        at(vec![stop(1, 0.2, red(), 1.0), stop(2, 0.3, red(), 1.0)]),
        0.65
    );
    // One stop: against the ends.
    assert_eq!(at(vec![stop(1, 0.4, red(), 1.0)]), 0.7);
    // Rounded to 0.1 %.
    assert_eq!(
        at(vec![stop(1, 0.0, red(), 1.0), stop(2, 0.333, red(), 1.0)]),
        0.667
    );
}

#[test]
fn add_with_no_stops_makes_one_at_fifty_percent_in_the_fill_colour() {
    let new = new_stop_values(&fill_of(vec![], blue()), None);
    assert_eq!(new.position.get(), 0.5);
    assert_eq!(new.color, blue());
    assert_eq!(new.opacity, Opacity::OPAQUE);
}

/// "Adding a stop changes nothing on screen": the new stop has the ramp's colour
/// and opacity at its position, for the button and for a bar click.
#[test]
fn a_new_stop_has_the_colour_the_ramp_has_there() {
    let fill = fill_of(
        vec![stop(1, 0.0, red(), 1.0), stop(2, 1.0, blue(), 0.0)],
        red(),
    );
    let middle = new_stop_values(&fill, None);
    assert_eq!(
        middle.color,
        Color {
            r: 128,
            g: 0,
            b: 128
        }
    );
    assert!((middle.opacity.get() - 0.5).abs() < 1e-9);
    let clicked = new_stop_values(&fill, Some(0.25));
    assert_eq!(clicked.position.get(), 0.25);
    assert_eq!(
        clicked.color,
        Color {
            r: 191,
            g: 0,
            b: 64
        }
    );
    // A click is clamped into 0 to 1 and rounded to 0.1 %.
    assert_eq!(new_stop_values(&fill, Some(1.7)).position.get(), 1.0);
    assert_eq!(new_stop_values(&fill, Some(-3.0)).position.get(), 0.0);
    assert_eq!(new_stop_values(&fill, Some(0.12345)).position.get(), 0.123);
}

// ---- addressing a stop by rank (criteria 20, 34) ---------------------------------

#[test]
fn a_rank_names_each_objects_own_stop_in_position_order_ties_in_list_order() {
    let document = Document::new(1);
    let (a, b) = (rect(&document, 0.0), rect(&document, 20.0));
    set(
        &document,
        a,
        FillMode::Linear,
        &[stop(1, 0.9, blue(), 1.0), stop(2, 0.5, red(), 1.0)],
    );
    document.add_stop(a, stop(3, 0.5, blue(), 1.0)).unwrap();
    set(
        &document,
        b,
        FillMode::Linear,
        &[stop(1, 0.1, red(), 1.0), stop(2, 0.2, red(), 1.0)],
    );
    document.add_stop(b, stop(3, 0.3, red(), 1.0)).unwrap();
    let objects = objects(&document);
    // `a` is listed [3, 1, 2] (the add went before the first greater stop), so by
    // position: id 3 (0.5, listed first), id 2 (0.5, listed later), id 1 (0.9).
    let ids = |rank| {
        stop_targets(&objects, &[a, b], rank)
            .into_iter()
            .map(|(_, stop)| stop)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(0), vec![StopId::new(7, 3), StopId::new(7, 1)]);
    assert_eq!(ids(1), vec![StopId::new(7, 2), StopId::new(7, 2)]);
    assert_eq!(ids(2), vec![StopId::new(7, 1), StopId::new(7, 3)]);
    assert_eq!(stop_targets(&objects, &[a, b], 3).len(), 0);
}

#[test]
fn a_selected_stop_keeps_its_identity_when_an_edit_re_sorts_the_list() {
    let document = Document::new(1);
    let a = rect(&document, 0.0);
    set(
        &document,
        a,
        FillMode::Linear,
        &[stop(1, 0.0, red(), 1.0), stop(2, 0.5, blue(), 1.0)],
    );
    let targets = stop_targets(&objects(&document), &[a], 0);
    assert_eq!(selected_rank(&objects(&document), &[a], &targets), Some(0));
    // Move stop 1 past stop 2.
    document
        .edit_stops(&stop_edits(
            &targets,
            StopChange::Position(StopPosition::new(0.8).unwrap()),
        ))
        .unwrap();
    assert_eq!(selected_rank(&objects(&document), &[a], &targets), Some(1));
    // A stop that is gone, or an object out of scope, selects nothing.
    document.add_stop(a, stop(9, 0.2, red(), 1.0)).unwrap();
    document.remove_stop(a, StopId::new(7, 1)).unwrap();
    assert_eq!(selected_rank(&objects(&document), &[a], &targets), None);
    assert_eq!(selected_rank(&objects(&document), &[], &targets), None);
}
