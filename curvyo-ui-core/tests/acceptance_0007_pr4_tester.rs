//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 4, UI-core
//! side: the stop editor's state across multi-selections and degenerate lists
//! (criteria 34, 35), the add-stop rule (17, 18) including a property test that
//! adding a stop changes nothing on screen, and rank addressing (20, 34).
//! Written from `specification.md` before the implementation was read.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, missing_docs)]
#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use curvyo_document_core::{
    Color, Document, Fill, FillMode, FillModeTarget, GradientStop, Length, NodeId, ObjectSnapshot,
    Opacity, Point, RectBounds, StopChange, StopId, StopPosition, StyleEdit, ramp_at, sorted_stops,
};
use curvyo_ui_core::{
    AnchorIdMinter, BarValue, StopsPanel, fill_targets, new_stop_values, selected_rank, stop_edits,
    stop_targets, stops_panel,
};
use proptest::prelude::*;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn stop(n: u64, p: f64, c: Color, o: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(7, n),
        position: StopPosition::new(p).unwrap(),
        color: c,
        opacity: Opacity::new(o).unwrap(),
    }
}

fn rect(d: &Document) -> NodeId {
    d.create_rect(RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn set(d: &Document, id: NodeId, mode: FillMode, stops: Vec<GradientStop>) {
    d.set_fill_mode(
        mode,
        &[FillModeTarget {
            id,
            seed_stops: stops,
        }],
    )
    .unwrap();
}

fn objs(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn fill_of(d: &Document, id: NodeId) -> Fill {
    d.object(id).unwrap().style().fill.clone()
}

fn editor(p: StopsPanel) -> curvyo_ui_core::StopEditorView {
    match p {
        StopsPanel::Editor(v) => v,
        o => panic!("expected the editor, got {o:?}"),
    }
}

// ----------------------------------------------------- AC 17 starting stops

#[test]
fn ac17_each_object_in_a_selection_seeds_from_its_own_colour_and_white_pairs_with_black() {
    let d = Document::new(1);
    let ids: Vec<_> = (0..4).map(|_| rect(&d)).collect();
    let colours = [
        rgb(255, 0, 0),
        Color::BLACK,
        rgb(255, 255, 255),
        rgb(1, 2, 3),
    ];
    for (id, c) in ids.iter().zip(colours) {
        d.edit_style(&[*id], &StyleEdit::FillColor(c)).unwrap();
    }
    let mut minter = AnchorIdMinter::new(5);
    let targets = fill_targets(&mut minter, &objs(&d), &ids);
    assert_eq!(targets.len(), 4);
    let want_end = [
        rgb(255, 255, 255),
        rgb(255, 255, 255),
        rgb(0, 0, 0),
        rgb(255, 255, 255),
    ];
    let mut seen = std::collections::HashSet::new();
    for (k, t) in targets.iter().enumerate() {
        assert_eq!(t.seed_stops.len(), 2);
        let [a, b] = [t.seed_stops[0], t.seed_stops[1]];
        assert_eq!((a.position.get(), b.position.get()), (0.0, 1.0));
        assert_eq!(a.color, colours[k]);
        assert_eq!(b.color, want_end[k]);
        assert_eq!((a.opacity.get(), b.opacity.get()), (1.0, 1.0));
        assert!(seen.insert(a.id) && seen.insert(b.id), "ids are fresh");
    }
}

#[test]
fn ac17_an_object_that_already_has_stops_gets_no_seed() {
    let d = Document::new(1);
    let a = rect(&d);
    let b = rect(&d);
    set(
        &d,
        a,
        FillMode::Linear,
        vec![
            stop(1, 0.2, rgb(1, 1, 1), 1.0),
            stop(2, 0.4, rgb(2, 2, 2), 1.0),
        ],
    );
    let mut minter = AnchorIdMinter::new(5);
    let t = fill_targets(&mut minter, &objs(&d), &[a, b]);
    assert!(t.iter().find(|x| x.id == a).unwrap().seed_stops.is_empty());
    assert_eq!(t.iter().find(|x| x.id == b).unwrap().seed_stops.len(), 2);
}

// ------------------------------------------------ AC 18 add-stop position

fn fill_with(stops: Vec<GradientStop>) -> Fill {
    let d = Document::new(1);
    let id = rect(&d);
    set(&d, id, FillMode::Linear, stops);
    fill_of(&d, id)
}

#[test]
fn ac18_add_button_takes_the_midpoint_of_the_widest_gap() {
    let c = rgb(0, 0, 0);
    let cases: Vec<(Vec<f64>, f64)> = vec![
        (vec![0.0, 1.0], 0.5),
        (vec![0.0, 0.3, 1.0], 0.65),
        (vec![0.1, 0.9], 0.5),
        (vec![0.25, 0.75], 0.5),
        (vec![0.5], 0.25),
        (vec![0.0, 0.5, 1.0], 0.25),
        (vec![0.0, 0.0, 1.0], 0.5),
        (vec![0.7, 0.2, 0.9], 0.45),
        (vec![0.5, 0.5], 0.25),
        (vec![0.0, 1.0, 1.0], 0.5),
    ];
    for (positions, want) in cases {
        let stops = positions
            .iter()
            .enumerate()
            .map(|(k, p)| stop(k as u64 + 1, *p, c, 1.0))
            .collect();
        let got = new_stop_values(&fill_with(stops), None).position.get();
        assert!(
            (got - want).abs() < 6e-4,
            "{positions:?}: got {got}, want {want}"
        );
        assert!(
            (got * 1000.0 - (got * 1000.0).round()).abs() < 1e-6,
            "rounded to 0.1 %: {got}"
        );
    }
}

#[test]
fn ac18_a_tie_goes_to_the_gap_nearest_the_start() {
    // Gaps 0.25, 0.5 (middle) and 0.25: the widest is unique. Make a true tie:
    // 0.25 / 0.25 / 0.5 -> stops 0.25 and 0.5 -> gaps 0.25, 0.25, 0.5; and
    // 0.5 / 0.5 with one stop at 0.5 is the first case above. Here: 3 equal gaps.
    let c = rgb(0, 0, 0);
    let stops = vec![
        stop(1, 0.25, c, 1.0),
        stop(2, 0.5, c, 1.0),
        stop(3, 0.75, c, 1.0),
    ];
    // Gaps: 0.25, 0.25, 0.25, 0.25 (start, 1-2, 2-3, end): nearest the start.
    let got = new_stop_values(&fill_with(stops), None).position.get();
    assert!((got - 0.125).abs() < 6e-4, "got {got}");
}

#[test]
fn ac18_no_stops_creates_one_at_fifty_percent_in_the_stored_colour() {
    let d = Document::new(1);
    let id = rect(&d);
    d.edit_style(&[id], &StyleEdit::FillColor(rgb(12, 34, 56)))
        .unwrap();
    d.edit_style(&[id], &StyleEdit::FillOpacity(Opacity::new(0.4).unwrap()))
        .unwrap();
    d.set_fill_mode(
        FillMode::Linear,
        &[FillModeTarget {
            id,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let n = new_stop_values(&fill_of(&d, id), None);
    assert_eq!(n.position.get(), 0.5);
    assert_eq!(n.color, rgb(12, 34, 56));
    assert_eq!(n.opacity.get(), 1.0, "100 % even if the solid had less");
    // A bar click at a position on an empty gradient: still the stored colour.
    let n = new_stop_values(&fill_of(&d, id), Some(0.2));
    assert_eq!(n.color, rgb(12, 34, 56));
    assert!((n.position.get() - 0.2).abs() < 6e-4 || (n.position.get() - 0.5).abs() < 1e-9);
}

#[test]
fn ac18_a_bar_click_clamps_and_survives_junk() {
    let f = fill_with(vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ]);
    assert_eq!(new_stop_values(&f, Some(-3.0)).position.get(), 0.0);
    assert_eq!(new_stop_values(&f, Some(7.0)).position.get(), 1.0);
    assert_eq!(new_stop_values(&f, Some(0.0)).position.get(), 0.0);
    assert_eq!(new_stop_values(&f, Some(1.0)).position.get(), 1.0);
    for junk in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let p = new_stop_values(&f, Some(junk)).position.get();
        assert!((0.0..=1.0).contains(&p), "{junk} -> {p}");
    }
    let p = new_stop_values(&f, Some(0.123_45)).position.get();
    assert!((p - 0.123_45).abs() <= 5.1e-4, "{p}");
}

#[test]
fn ac18_a_new_stop_takes_the_translucent_ramp_value_there() {
    let f = fill_with(vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 0.0),
    ]);
    let n = new_stop_values(&f, Some(0.25));
    assert!((f64::from(n.color.r) - 191.0).abs() <= 1.5, "{:?}", n.color);
    assert!((f64::from(n.color.b) - 64.0).abs() <= 1.5, "{:?}", n.color);
    assert!((n.opacity.get() - 0.75).abs() < 0.005);
}

#[test]
fn ac18_adding_at_a_hard_edge_with_the_real_rule_changes_nothing() {
    let d = Document::new(1);
    let id = rect(&d);
    set(
        &d,
        id,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(0, 0, 0), 1.0),
            stop(2, 0.5, rgb(255, 0, 0), 1.0),
            stop(3, 0.5, rgb(0, 0, 255), 1.0),
            stop(4, 1.0, rgb(255, 255, 255), 1.0),
        ],
    );
    let before = sorted_stops(&fill_of(&d, id).stops);
    let n = new_stop_values(&fill_of(&d, id), Some(0.5));
    d.add_stop(
        id,
        GradientStop {
            id: StopId::new(8, 1),
            position: n.position,
            color: n.color,
            opacity: n.opacity,
        },
    )
    .unwrap();
    let after = sorted_stops(&fill_of(&d, id).stops);
    for k in 0..=1000 {
        let t = f64::from(k) / 1000.0;
        assert_eq!(
            ramp_at(&before, t).unwrap().0,
            ramp_at(&after, t).unwrap().0,
            "t {t}"
        );
    }
}

fn arb_stops() -> impl Strategy<Value = Vec<(u32, (u8, u8, u8), u32)>> {
    prop::collection::vec(
        (
            0_u32..=8,
            (any::<u8>(), any::<u8>(), any::<u8>()),
            0_u32..=100,
        ),
        2..=15,
    )
}

proptest! {
    /// "Adding a stop changes nothing on screen until it is edited" for any
    /// stop list with distinct positions (given in any list order), any click
    /// position, and the Add button. Coincident positions are the ignored
    /// test below.
    #[test]
    fn adding_a_stop_never_changes_the_ramp(raw in arb_stops(), click in prop::option::of(0_u32..=1000)) {
        let mut seen = std::collections::HashSet::new();
        let stops: Vec<GradientStop> = raw
            .iter()
            .filter(|(p, _, _)| seen.insert(*p))
            .enumerate()
            .map(|(k, (p, (r, g, b), o))| {
                stop(k as u64 + 1, f64::from(*p) / 8.0, rgb(*r, *g, *b), f64::from(*o) / 100.0)
            })
            .collect();
        prop_assume!(stops.len() >= 2);
        let fill = fill_with(stops);
        let at = click.map(|c| f64::from(c) / 1000.0);
        let n = new_stop_values(&fill, at);
        let before = sorted_stops(&fill.stops);
        // Insert as the document does: by position, after earlier equals.
        let d = Document::new(1);
        let id = rect(&d);
        set(&d, id, FillMode::Linear, fill.stops.clone());
        d.add_stop(id, GradientStop { id: StopId::new(8, 1), position: n.position, color: n.color, opacity: n.opacity }).unwrap();
        let after = sorted_stops(&fill_of(&d, id).stops);
        prop_assert_eq!(after.len(), before.len() + 1);
        for k in 0..=400 {
            let t = f64::from(k) / 400.0;
            let (c0, o0) = ramp_at(&before, t).unwrap();
            let (c1, o1) = ramp_at(&after, t).unwrap();
            let dc = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
            prop_assert!(dc(c0.r, c1.r) <= 1 && dc(c0.g, c1.g) <= 1 && dc(c0.b, c1.b) <= 1,
                "t {} {:?} -> {:?} (added at {:?})", t, c0, c1, n.position.get());
            prop_assert!((o0.get() - o1.get()).abs() < 0.011);
        }
    }

    /// Rank addressing: for any list, the stop of rank k is the k-th of the
    /// stable position sort.
    #[test]
    fn rank_is_the_stable_position_order(raw in arb_stops()) {
        let stops: Vec<GradientStop> = raw
            .iter()
            .enumerate()
            .map(|(k, (p, (r, g, b), o))| stop(k as u64 + 1, f64::from(*p) / 8.0, rgb(*r, *g, *b), f64::from(*o) / 100.0))
            .collect();
        let d = Document::new(1);
        let id = rect(&d);
        set(&d, id, FillMode::Radial, stops.clone());
        let o = objs(&d);
        let sorted = sorted_stops(&d.object(id).unwrap().style().fill.stops);
        for (k, s) in sorted.iter().enumerate() {
            prop_assert_eq!(stop_targets(&o, &[id], k), vec![(id, s.id)]);
        }
        prop_assert!(stop_targets(&o, &[id], sorted.len()).is_empty());
        prop_assert_eq!(sorted.len(), stops.len());
    }
}

// ------------------------------------------------------------ AC 34 / 35

#[test]
fn ac34_same_mode_same_count_shows_rows_by_rank_with_mixed_fields() {
    let d = Document::new(1);
    let a = rect(&d);
    let b = rect(&d);
    let c = rect(&d);
    set(
        &d,
        a,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(255, 0, 0), 1.0),
            stop(2, 1.0, rgb(0, 0, 255), 1.0),
        ],
    );
    // b lists its stops in the opposite order but has equal values.
    set(
        &d,
        b,
        FillMode::Linear,
        vec![
            stop(3, 1.0, rgb(0, 0, 255), 1.0),
            stop(4, 0.0, rgb(255, 0, 0), 1.0),
        ],
    );
    let v = editor(stops_panel(&objs(&d), &[a, b]));
    assert_eq!(v.objects, 2);
    assert_eq!(v.rows.len(), 2);
    assert_eq!(
        v.rows[0].position,
        BarValue::Uniform(StopPosition::new(0.0).unwrap())
    );
    assert_eq!(v.rows[0].color, BarValue::Uniform(rgb(255, 0, 0)));
    assert_eq!(v.rows[1].color, BarValue::Uniform(rgb(0, 0, 255)));
    assert!(
        v.bar.is_some(),
        "identical in value: the ramp and thumbs show"
    );
    assert!(!v.can_add && !v.can_remove, "hidden for several objects");

    set(
        &d,
        c,
        FillMode::Linear,
        vec![
            stop(5, 0.0, rgb(255, 0, 0), 0.5),
            stop(6, 0.5, rgb(0, 0, 255), 1.0),
        ],
    );
    let v = editor(stops_panel(&objs(&d), &[a, b, c]));
    assert_eq!(v.rows[0].color, BarValue::Uniform(rgb(255, 0, 0)));
    assert_eq!(v.rows[0].opacity, BarValue::Mixed);
    assert_eq!(
        v.rows[0].position,
        BarValue::Uniform(StopPosition::new(0.0).unwrap())
    );
    assert_eq!(v.rows[1].position, BarValue::Mixed);
    assert!(
        v.bar.is_none(),
        "differing values: hatched track, no thumbs"
    );
}

#[test]
fn ac34_different_counts_message_but_different_modes_or_solid_hide_the_editor() {
    let d = Document::new(1);
    let a = rect(&d);
    let b = rect(&d);
    let s = rect(&d);
    let two = vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ];
    let three = vec![
        stop(3, 0.0, rgb(0, 0, 0), 1.0),
        stop(4, 0.5, rgb(5, 5, 5), 1.0),
        stop(5, 1.0, rgb(9, 9, 9), 1.0),
    ];
    set(&d, a, FillMode::Linear, two.clone());
    set(&d, b, FillMode::Linear, three.clone());
    assert_eq!(stops_panel(&objs(&d), &[a, b]), StopsPanel::DifferentCounts);
    set(&d, b, FillMode::Radial, three);
    assert_eq!(
        stops_panel(&objs(&d), &[a, b]),
        StopsPanel::Hidden,
        "linear + radial"
    );
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id: s,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    assert_eq!(
        stops_panel(&objs(&d), &[a, s]),
        StopsPanel::Hidden,
        "gradient + solid"
    );
    assert_eq!(stops_panel(&objs(&d), &[s]), StopsPanel::Hidden);
    // A gradient that was switched off to None keeps its stops but shows no editor.
    d.set_fill_mode(
        FillMode::None,
        &[FillModeTarget {
            id: a,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    assert_eq!(stops_panel(&objs(&d), &[a]), StopsPanel::Hidden);
    assert_eq!(stops_panel(&objs(&d), &[]), StopsPanel::Hidden);
    // A stale id does not panic.
    let gone = rect(&d);
    let stale = objs(&d);
    d.delete_objects(&[gone]).unwrap();
    let _ = stops_panel(&objs(&d), &[gone]);
    let _ = stops_panel(&objs(&d), &[gone, b]);
    let _ = stops_panel(&stale, &[gone]);
}

#[test]
fn ac35_zero_one_and_many_stops_are_states_with_the_stated_buttons() {
    let d = Document::new(1);
    let z = rect(&d);
    let o = rect(&d);
    let m = rect(&d);
    d.set_fill_mode(
        FillMode::Linear,
        &[FillModeTarget {
            id: z,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    set(
        &d,
        o,
        FillMode::Linear,
        vec![stop(1, 0.5, rgb(1, 2, 3), 0.5)],
    );
    let many: Vec<_> = (0..20)
        .map(|k| stop(k + 1, k as f64 / 19.0, rgb(k as u8, 0, 0), 1.0))
        .collect();
    set(&d, m, FillMode::Linear, many);
    let zero = editor(stops_panel(&objs(&d), &[z]));
    assert!(zero.rows.is_empty());
    assert!(zero.can_add && !zero.can_remove);
    assert_eq!(zero.bar, Some(vec![]), "an empty bar, not a hatched one");
    let one = editor(stops_panel(&objs(&d), &[o]));
    assert_eq!(one.rows.len(), 1);
    assert!(one.can_add && !one.can_remove, "Remove disabled below 3");
    assert_eq!(one.bar.as_ref().unwrap().len(), 1);
    let many = editor(stops_panel(&objs(&d), &[m]));
    assert_eq!(many.rows.len(), 20, "every stop gets a row");
    assert!(!many.can_add, "Add disabled at 16 and above");
    assert!(many.can_remove, "Remove works down to 2");
}

#[test]
fn ac34_ac35_exact_limits_two_three_fifteen_sixteen() {
    for (n, add, remove) in [
        (2_u64, true, false),
        (3, true, true),
        (15, true, true),
        (16, false, true),
        (17, false, true),
    ] {
        let d = Document::new(1);
        let id = rect(&d);
        let st: Vec<_> = (0..n)
            .map(|k| stop(k + 1, k as f64 / 20.0, rgb(0, 0, 0), 1.0))
            .collect();
        set(&d, id, FillMode::Linear, st);
        let v = editor(stops_panel(&objs(&d), &[id]));
        assert_eq!((v.can_add, v.can_remove), (add, remove), "{n} stops");
    }
}

// ---------------------------------------------------- rank addressing

#[test]
fn ac20_ac34_ties_rank_in_list_order_and_a_missing_rank_in_any_object_gives_none() {
    let d = Document::new(1);
    let a = rect(&d);
    let b = rect(&d);
    set(
        &d,
        a,
        FillMode::Linear,
        vec![
            stop(1, 0.5, rgb(1, 0, 0), 1.0),
            stop(2, 0.0, rgb(2, 0, 0), 1.0),
            stop(3, 0.5, rgb(3, 0, 0), 1.0),
        ],
    );
    set(
        &d,
        b,
        FillMode::Linear,
        vec![
            stop(4, 0.5, rgb(4, 0, 0), 1.0),
            stop(5, 0.5, rgb(5, 0, 0), 1.0),
        ],
    );
    let o = objs(&d);
    assert_eq!(stop_targets(&o, &[a], 0), vec![(a, StopId::new(7, 2))]);
    assert_eq!(stop_targets(&o, &[a], 1), vec![(a, StopId::new(7, 1))]);
    assert_eq!(stop_targets(&o, &[a], 2), vec![(a, StopId::new(7, 3))]);
    assert_eq!(stop_targets(&o, &[b], 1), vec![(b, StopId::new(7, 5))]);
    // Rank 2 exists in a but not in b: nothing is edited anywhere.
    assert!(stop_targets(&o, &[a, b], 2).is_empty());
    assert_eq!(stop_targets(&o, &[a, b], 1).len(), 2);
    // selected_rank follows the id after an edit re-sorts the list.
    let targets = stop_targets(&o, &[a], 0);
    d.edit_stops(&stop_edits(
        &targets,
        StopChange::Position(StopPosition::new(0.9).unwrap()),
    ))
    .unwrap();
    let o2 = objs(&d);
    assert_eq!(selected_rank(&o2, &[a], &targets), Some(2));
    // The selection is gone once its stop is.
    d.remove_stop(a, StopId::new(7, 2)).unwrap();
    assert_eq!(selected_rank(&objs(&d), &[a], &targets), None);
    assert_eq!(selected_rank(&objs(&d), &[], &targets), None);
}

#[test]
fn ac34_one_batch_edits_every_object_and_leaves_the_other_ranks_alone() {
    let d = Document::new(1);
    let ids: Vec<_> = (0..3).map(|_| rect(&d)).collect();
    for (k, id) in ids.iter().enumerate() {
        let k = k as u64 * 10;
        set(
            &d,
            *id,
            FillMode::Linear,
            vec![
                stop(k + 1, 0.0, rgb(1, 1, 1), 1.0),
                stop(k + 2, 0.4, rgb(2, 2, 2), 1.0),
                stop(k + 3, 1.0, rgb(3, 3, 3), 1.0),
            ],
        );
    }
    let before: Vec<_> = ids.iter().map(|i| fill_of(&d, *i).stops).collect();
    let targets = stop_targets(&objs(&d), &ids, 1);
    assert_eq!(targets.len(), 3);
    d.edit_stops(&stop_edits(&targets, StopChange::Color(rgb(200, 100, 50))))
        .unwrap();
    for (k, id) in ids.iter().enumerate() {
        let after = fill_of(&d, *id).stops;
        assert_eq!(after[0], before[k][0]);
        assert_eq!(after[2], before[k][2]);
        assert_eq!(after[1].color, rgb(200, 100, 50));
        assert_eq!(after[1].position, before[k][1].position);
        assert_eq!(after[1].opacity, before[k][1].opacity);
    }
}

#[test]
fn polygon_and_star_notes_only_for_polygons_and_stars() {
    use curvyo_document_core::{Angle, PointCount, StarFrame};
    let d = Document::new(1);
    let r = rect(&d);
    let p = d.create_polygon(
        StarFrame {
            center: Point::new(50.0, 50.0),
            radius: Length::from_mm(5.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(3).unwrap(),
    );
    let two = vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ];
    set(&d, r, FillMode::Linear, two.clone());
    set(&d, p, FillMode::Radial, two.clone());
    assert!(!editor(stops_panel(&objs(&d), &[r])).box_note);
    assert!(editor(stops_panel(&objs(&d), &[p])).box_note);
    set(&d, p, FillMode::Linear, two);
    assert!(editor(stops_panel(&objs(&d), &[r, p])).box_note);
}

/// The document inserts a new stop "in position order" by looking at the
/// *list* order, which is not the render order once a position edit has moved a
/// stop past a neighbour (the list is only stably sorted when drawn). With a
/// coincident pair, the new stop can land before the pair and change the pad
/// colour on the left of the hard edge, so adding a stop is not invisible.
fn add_via_the_real_rule(
    list: Vec<GradientStop>,
    at: f64,
) -> (Vec<GradientStop>, Vec<GradientStop>) {
    let fill = fill_with(list.clone());
    let n = new_stop_values(&fill, Some(at));
    let d = Document::new(1);
    let id = rect(&d);
    set(&d, id, FillMode::Linear, list);
    d.add_stop(
        id,
        GradientStop {
            id: StopId::new(8, 1),
            position: n.position,
            color: n.color,
            opacity: n.opacity,
        },
    )
    .unwrap();
    (
        sorted_stops(&fill.stops),
        sorted_stops(&fill_of(&d, id).stops),
    )
}

fn assert_same_ramp(before: &[GradientStop], after: &[GradientStop]) {
    for k in 0..=1000 {
        let t = f64::from(k) / 1000.0;
        let (c0, o0) = ramp_at(before, t).unwrap();
        let (c1, o1) = ramp_at(after, t).unwrap();
        assert_eq!(c0, c1, "colour at t {t}");
        assert!((o0.get() - o1.get()).abs() < 1e-9, "opacity at t {t}");
    }
}

#[test]
fn ac18_defect_a_stop_added_on_a_hard_edge_of_an_unsorted_list_changes_the_ramp() {
    // A user dragged the white stop past the others: the list is
    // [white@1, black@0, red@.5, blue@.5] in list order.
    let list = vec![
        stop(1, 1.0, rgb(255, 255, 255), 1.0),
        stop(2, 0.0, rgb(0, 0, 0), 1.0),
        stop(3, 0.5, rgb(255, 0, 0), 1.0),
        stop(4, 0.5, rgb(0, 0, 255), 1.0),
    ];
    let (before, after) = add_via_the_real_rule(list, 0.5);
    assert_same_ramp(&before, &after);
}

#[test]
fn ac18_defect_pad_region_variant_of_the_unsorted_list_insertion() {
    let list = vec![
        stop(1, 1.0, rgb(0, 0, 0), 0.0),
        stop(2, 0.875, rgb(0, 0, 0), 0.0),
        stop(3, 0.875, rgb(0, 0, 0), 0.02),
    ];
    let (before, after) = add_via_the_real_rule(list, 0.875);
    assert_same_ramp(&before, &after);
}

#[test]
fn ac18_a_stop_added_on_a_hard_edge_of_a_sorted_list_changes_nothing() {
    let list = vec![
        stop(1, 0.0, rgb(0, 0, 0), 1.0),
        stop(2, 0.5, rgb(255, 0, 0), 1.0),
        stop(3, 0.5, rgb(0, 0, 255), 1.0),
        stop(4, 1.0, rgb(255, 255, 255), 1.0),
    ];
    let (before, after) = add_via_the_real_rule(list, 0.5);
    assert_same_ramp(&before, &after);
}
