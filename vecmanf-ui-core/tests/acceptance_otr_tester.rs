//! Independent tester acceptance tests for
//! `specs/object-transform-refinements/specification.md`, `vecmanf-ui-core`
//! share: the 22.5 degree snap stops (criteria 33-36, 47) and the entry
//! parser (criteria 19, 21, 30). Expected values come from the specification
//! text and from a brute-force reference written here, never from the code
//! under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use proptest::prelude::*;
use vecmanf_document_core::Angle;
use vecmanf_ui_core::{format_degrees, parse_entry_number, snap_angle, snap_skew_angle};

fn snap_deg(raw: f64) -> f64 {
    snap_angle(Angle::from_radians(raw.to_radians()))
        .as_radians()
        .to_degrees()
}

fn skew_snap_deg(raw: f64) -> f64 {
    snap_skew_angle(Angle::from_radians(raw.to_radians()))
        .as_radians()
        .to_degrees()
}

/// Every stop of {k*15} u {k*22.5} within +-1080 degrees, ascending.
fn all_stops() -> Vec<f64> {
    let mut v = Vec::new();
    for k in -80..=80 {
        v.push(f64::from(k) * 15.0);
    }
    for k in -50..=50 {
        v.push(f64::from(k) * 22.5);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

/// Reference: nearest stop; exact tie goes to the smaller magnitude.
fn reference(raw: f64) -> f64 {
    let mut best = 0.0_f64;
    let mut best_d = f64::INFINITY;
    for s in all_stops() {
        let d = (s - raw).abs();
        if d < best_d - 1e-9 || ((d - best_d).abs() <= 1e-9 && s.abs() < best.abs()) {
            best = s;
            best_d = d;
        }
    }
    best
}

#[test]
fn ac34_the_ten_listed_raw_angles_snap_as_the_spec_says() {
    let cases = [
        (10.0, 15.0),
        (19.0, 22.5),
        (18.5, 15.0),
        (26.0, 22.5),
        (26.5, 30.0),
        (40.0, 45.0),
        (55.0, 60.0),
        (64.0, 67.5),
        (71.0, 67.5),
        (100.0, 105.0),
    ];
    for (raw, want) in cases {
        let got = snap_deg(raw);
        assert!((got - want).abs() < 1e-9, "{raw} -> {got}, want {want}");
        let got = snap_deg(-raw);
        assert!((got + want).abs() < 1e-9, "-{raw} -> {got}, want -{want}");
    }
}

#[test]
fn ac34_exact_midpoint_goes_to_the_stop_nearer_the_start() {
    // Midpoints between adjacent stops, first quadrant: 7.5, 18.75, 26.25,
    // 37.5 (and then +45k). The stop with the smaller magnitude wins.
    for k in 0..8 {
        let base = 45.0 * f64::from(k);
        for (mid, lower) in [(7.5, 0.0), (18.75, 15.0), (26.25, 22.5), (37.5, 30.0)] {
            let got = snap_deg(base + mid);
            assert!(
                (got - (base + lower)).abs() < 1e-6,
                "{} -> {got}, want {}",
                base + mid,
                base + lower
            );
            let got = snap_deg(-(base + mid));
            assert!(
                (got + (base + lower)).abs() < 1e-6,
                "-{} -> {got}, want -{}",
                base + mid,
                base + lower
            );
        }
    }
}

#[test]
fn ac33_every_listed_stop_is_a_fixed_point_through_all_quadrants_and_negatives() {
    let half_turn = [
        0.0, 15.0, 22.5, 30.0, 45.0, 60.0, 67.5, 75.0, 90.0, 105.0, 112.5, 120.0, 135.0, 150.0,
        157.5, 165.0, 180.0,
    ];
    let second = [
        195.0, 202.5, 210.0, 225.0, 240.0, 247.5, 255.0, 270.0, 285.0, 292.5, 300.0, 315.0, 330.0,
        337.5, 345.0, 360.0,
    ];
    for s in half_turn.iter().chain(second.iter()) {
        assert!((snap_deg(*s) - *s).abs() < 1e-9, "stop {s}");
        assert!((snap_deg(-*s) + *s).abs() < 1e-9, "stop -{s}");
        // slightly off the stop snaps back onto it
        assert!((snap_deg(*s + 0.4) - *s).abs() < 1e-9, "stop {s}+0.4");
        assert!((snap_deg(*s - 0.4) - *s).abs() < 1e-9, "stop {s}-0.4");
    }
    // The spec's own examples: 67.5 and 337.5 exist exactly like 22.5.
    assert!((snap_deg(338.0) - 337.5).abs() < 1e-9);
    assert!((snap_deg(-338.0) + 337.5).abs() < 1e-9);
}

#[test]
fn ac33_matches_a_brute_force_reference_on_a_fine_sweep() {
    let mut raw = -400.0_f64;
    while raw <= 400.0 {
        let want = reference(raw);
        // skip values within 1e-6 deg of a midpoint, where float noise may
        // legitimately decide the tie either way
        let stops = all_stops();
        let near_tie = stops.windows(2).any(|w| {
            let mid = f64::midpoint(w[0], w[1]);
            (raw - mid).abs() < 1e-6
        });
        if !near_tie {
            let got = snap_deg(raw);
            assert!(
                (got - want).abs() < 1e-6,
                "raw {raw}: got {got}, want {want}"
            );
        }
        raw += 0.013;
    }
}

#[test]
fn ac33_snap_of_zero_and_of_non_finite_is_zero_and_stays_finite() {
    assert_eq!(snap_deg(0.0), 0.0);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let a = snap_angle(Angle::from_radians(bad)).as_radians();
        assert!(a.is_finite(), "non-finite raw must not produce {a}");
    }
}

proptest! {
    #[test]
    fn ac33_snap_is_idempotent_odd_and_never_moves_more_than_half_a_gap(raw in -1000.0_f64..1000.0) {
        let s = snap_deg(raw);
        // idempotent
        prop_assert!((snap_deg(s) - s).abs() < 1e-9);
        // odd symmetry
        prop_assert!((snap_deg(-raw) + s).abs() < 1e-9);
        // largest gap between stops is 15 degrees (between 30 and 45), so
        // the nearest stop is at most 7.5 away
        prop_assert!((s - raw).abs() <= 7.5 + 1e-9);
        // result is a stop
        prop_assert!(all_stops().iter().any(|t| (t - s).abs() < 1e-7), "{s} is not a stop");
    }
}

#[test]
fn ac47_skew_snap_is_capped_at_75_degrees() {
    assert!((skew_snap_deg(80.0) - 75.0).abs() < 1e-9);
    assert!((skew_snap_deg(89.0) - 75.0).abs() < 1e-9);
    assert!((skew_snap_deg(89.999) - 75.0).abs() < 1e-9);
    assert!((skew_snap_deg(-80.0) + 75.0).abs() < 1e-9);
    assert!((skew_snap_deg(-89.0) + 75.0).abs() < 1e-9);
    // below the cap it is the ordinary snap
    assert!((skew_snap_deg(40.0) - 45.0).abs() < 1e-9);
    assert!((skew_snap_deg(-19.0) + 22.5).abs() < 1e-9);
    assert!((skew_snap_deg(0.0)).abs() < 1e-9);
    for raw in (-899..=899).map(|d| f64::from(d) / 10.0) {
        let s = skew_snap_deg(raw);
        assert!(s.abs() <= 75.0 + 1e-9, "{raw} -> {s}");
    }
}

#[test]
fn ac35_readout_format_shows_up_to_one_decimal() {
    assert_eq!(format_degrees(22.5), "22.5°");
    assert_eq!(format_degrees(45.0), "45°");
    assert_eq!(format_degrees(67.5), "67.5°");
    assert_eq!(format_degrees(0.0), "0°");
}

// ----------------------------------------------------------------------
// Parser (criteria 19, 21, 30)
// ----------------------------------------------------------------------

#[test]
fn ac19_parser_accepts_dot_comma_degree_sign_and_sign() {
    assert_eq!(parse_entry_number("37.5", true), Some(37.5));
    assert_eq!(parse_entry_number("37,5", true), Some(37.5));
    assert_eq!(parse_entry_number("37.5°", true), Some(37.5));
    assert_eq!(parse_entry_number(" 45 ", true), Some(45.0));
    assert_eq!(parse_entry_number("-90", true), Some(-90.0));
    assert_eq!(parse_entry_number("0", true), Some(0.0));
    assert_eq!(parse_entry_number("720", true), Some(720.0));
    assert_eq!(parse_entry_number("12,5", false), Some(12.5));
}

#[test]
fn ac21_parser_refuses_everything_that_is_not_a_finite_number() {
    for bad in [
        "", " ", "abc", "1,2,3", "1.2.3", "1,2.3", "--1", "1e3", "NaN", "inf", "Infinity", "12mm",
        "°", "5°°", "°5", "1 2",
    ] {
        assert_eq!(parse_entry_number(bad, true), None, "angle {bad:?}");
    }
    // the degree sign belongs to the angle field only
    assert_eq!(parse_entry_number("5°", false), None);
}

#[test]
fn ac30_parser_never_returns_non_finite_even_for_huge_digit_strings() {
    let huge = "9".repeat(400);
    if let Some(v) = parse_entry_number(&huge, true) {
        assert!(v.is_finite(), "huge digit string parsed to {v}");
    }
    let huge_size = "9".repeat(400);
    if let Some(v) = parse_entry_number(&huge_size, false) {
        assert!(v.is_finite());
    }
}

// ----------------------------------------------------------------------
// Select tool state: frozen handle set (criterion 6), pivot preview (55),
// skew guide (56)
// ----------------------------------------------------------------------

mod select_tool_state {
    use vecmanf_document_core::{
        AnchorId, Document, Length, NewAnchor, ObjectSnapshot, Point, RectBounds, Tolerance,
    };
    use vecmanf_ui_core::{
        ObjectSelection, ResizeDirection, SelectTool, Side, TransformHandle,
        TransformHandleTolerances, oriented_bounds,
    };

    const K: f64 = 2.0;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn rect_objects() -> (Document, Vec<ObjectSnapshot>, ObjectSelection) {
        let d = Document::new(1);
        let id = d.create_rect(RectBounds {
            origin: pt(0.0, 0.0),
            width: Length::from_mm(60.0),
            height: Length::from_mm(40.0),
        });
        let objects = vec![d.object(id).unwrap()];
        let mut sel = ObjectSelection::new();
        sel.select_single(id);
        (d, objects, sel)
    }

    fn tri_objects() -> (Document, Vec<ObjectSnapshot>, ObjectSelection) {
        let d = Document::new(1);
        let id = d.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), pt(60.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 3), pt(60.0, 40.0)),
            ],
            true,
        );
        let objects = vec![d.object(id).unwrap()];
        let mut sel = ObjectSelection::new();
        sel.select_single(id);
        (d, objects, sel)
    }

    fn tol() -> TransformHandleTolerances {
        TransformHandleTolerances::at_scale(K)
    }

    #[test]
    fn side_handle_set_is_frozen_for_the_whole_drag() {
        let (d, objects, mut sel) = rect_objects();
        let mut tool = SelectTool::new();
        // press a corner rotate handle without Shift
        let handles = SelectTool::transform_handles(&objects, &sel, tol(), false);
        let ne = handles
            .iter()
            .find(|(h, _)| *h == TransformHandle::Rotate(ResizeDirection::Ne))
            .unwrap()
            .1;
        tool.pointer_down(
            &objects,
            &mut sel,
            ne,
            Tolerance::from_mm(1.0),
            tol(),
            false,
        );
        assert!(
            !tool.side_rotate_revealed(true),
            "Shift mid-drag reveals nothing"
        );
        assert!(!tool.side_rotate_revealed(false));
        tool.pointer_up(
            &d,
            &objects,
            &mut sel,
            pt(ne.x + 20.0, ne.y + 20.0),
            true,
            false,
        );
        assert!(
            tool.side_rotate_revealed(true),
            "idle again: live Shift decides"
        );
        assert!(!tool.side_rotate_revealed(false));

        // press a side handle WITH shift: stays revealed when Shift is released
        let handles = SelectTool::transform_handles(&objects, &sel, tol(), true);
        let n = handles
            .iter()
            .find(|(h, _)| *h == TransformHandle::Rotate(ResizeDirection::N))
            .unwrap()
            .1;
        tool.pointer_down(&objects, &mut sel, n, Tolerance::from_mm(1.0), tol(), true);
        assert!(
            tool.side_rotate_revealed(false),
            "dragged side handle stays visible"
        );
        assert!(tool.side_rotate_revealed(true));
        tool.escape();
        assert!(!tool.side_rotate_revealed(false), "gone after the drag");
    }

    #[test]
    fn a_body_drag_started_without_shift_does_not_reveal_side_handles() {
        let (_d, objects, mut sel) = rect_objects();
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut sel,
            pt(20.0, 20.0),
            Tolerance::from_mm(1.0),
            tol(),
            false,
        );
        assert!(!tool.side_rotate_revealed(true));
        // and one started WITH Shift is frozen as revealed? Shift-click on
        // the body is a selection gesture; the set must not depend on it
        // beyond the press.
    }

    #[test]
    fn hover_pivot_previews_the_point_a_press_would_use() {
        let (_d, objects, sel) = rect_objects();
        let object = &objects[0];
        let _ = &sel;
        let b = oriented_bounds(object);
        // rotate corner Ne with Shift -> opposite (Sw) corner (0, 40)
        let p = SelectTool::hover_pivot(
            object,
            &b,
            TransformHandle::Rotate(ResizeDirection::Ne),
            true,
        )
        .unwrap();
        assert!(
            (p.x - 0.0).abs() < 1e-9 && (p.y - 40.0).abs() < 1e-9,
            "{p:?}"
        );
        // rotate side N with Shift -> bottom midpoint (30, 40)
        let p = SelectTool::hover_pivot(
            object,
            &b,
            TransformHandle::Rotate(ResizeDirection::N),
            true,
        )
        .unwrap();
        assert!(
            (p.x - 30.0).abs() < 1e-9 && (p.y - 40.0).abs() < 1e-9,
            "{p:?}"
        );
        // resize with Shift -> centre
        let p = SelectTool::hover_pivot(
            object,
            &b,
            TransformHandle::Resize(ResizeDirection::Se),
            true,
        )
        .unwrap();
        assert!(
            (p.x - 30.0).abs() < 1e-9 && (p.y - 20.0).abs() < 1e-9,
            "{p:?}"
        );
        // the centre handle has no pivot
        assert!(SelectTool::hover_pivot(object, &b, TransformHandle::Move, true).is_none());
    }

    #[test]
    fn skew_guide_runs_along_the_fixed_line_extended_past_both_ends() {
        let (_d, objects, mut sel) = tri_objects();
        let mut tool = SelectTool::new();
        let handles = SelectTool::transform_handles(&objects, &sel, tol(), false);
        let top = handles
            .iter()
            .find(|(h, _)| *h == TransformHandle::Skew(Side::Top))
            .unwrap()
            .1;
        tool.pointer_down(
            &objects,
            &mut sel,
            top,
            Tolerance::from_mm(1.0),
            tol(),
            false,
        );
        let (a, b) = tool
            .skew_guide(false, 6.0)
            .expect("a skew drag shows a guide");
        // fixed edge = bottom edge y = 40, from x = 0..60, extended by 6
        let (lo, hi) = if a.x < b.x { (a, b) } else { (b, a) };
        assert!(
            (lo.x + 6.0).abs() < 1e-9 && (hi.x - 66.0).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        assert!((lo.y - 40.0).abs() < 1e-9 && (hi.y - 40.0).abs() < 1e-9);
        // Shift: the centre line y = 20
        let (a, b) = tool.skew_guide(true, 6.0).unwrap();
        assert!(
            (a.y - 20.0).abs() < 1e-9 && (b.y - 20.0).abs() < 1e-9,
            "{a:?} {b:?}"
        );
        tool.escape();
        assert!(
            tool.skew_guide(false, 6.0).is_none(),
            "gone after Escape / release"
        );
        // no guide while another handle is dragged
        let handles = SelectTool::transform_handles(&objects, &sel, tol(), false);
        let se = handles
            .iter()
            .find(|(h, _)| *h == TransformHandle::Resize(ResizeDirection::Se))
            .unwrap()
            .1;
        tool.pointer_down(
            &objects,
            &mut sel,
            se,
            Tolerance::from_mm(1.0),
            tol(),
            false,
        );
        assert!(tool.skew_guide(false, 6.0).is_none());
    }
}
