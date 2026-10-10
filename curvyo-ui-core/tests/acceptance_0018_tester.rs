//! Independent tester cases for `0018-stroke-markers`, ui-core side: the Count
//! value field (scale, typed range, reset), the typed-text refusals, and the
//! rule that markers do not take part in hit-testing, bounds or the eyedropper.
//! Written from `specification.md` before the implementation was read.
//!
//! Criteria: 2, 18, 20, 30, 31.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    missing_docs
)]

use curvyo_document_core::{
    AnchorId, Color, Document, Length, MarkerCount, MarkerPlace, MarkerShape, NewAnchor, NodeId,
    ObjectSnapshot, Point, RectBounds, StyleEdit, Tolerance,
};
use curvyo_ui_core::{
    Grid, StyleEntryError, StyleField, ValueField, hit_test_object, hit_test_objects, object_bounds,
};

/// The eyedropper over the objects only: no background to fall back to
/// (`0040-document-background` added that source; these cases are about the objects).
fn pick_colour(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> Option<curvyo_ui_core::PickedColour> {
    curvyo_ui_core::pick_colour(
        objects,
        point,
        tolerance,
        curvyo_document_core::DocumentSize::default(),
        curvyo_document_core::DocumentBackground {
            paint: curvyo_document_core::BackgroundPaint::None,
            ..curvyo_document_core::DocumentBackground::DEFAULT
        },
    )
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

// -------------------------------------------- AC 2/30/31: the Count field

#[test]
fn the_count_scale_is_linear_one_to_fifty_when_dragged() {
    let s = ValueField::MarkerCount.scale();
    assert_eq!(s.value_at(0.0), 1.0);
    assert_eq!(s.value_at(1.0), 50.0);
    assert!((s.value_at(0.5) - 25.5).abs() < 1e-9);
    assert_eq!(s.scale_start(), 1.0);
    assert_eq!(s.scale_end(), 50.0, "Home 1, End 50");
    assert_eq!(s.typed_max(), 500.0);
    assert_eq!(s.default_value(), 1.0);
    // Round trip position <-> value.
    for v in [1.0, 2.0, 10.0, 33.0, 50.0] {
        assert!((s.value_at(s.position_of(v)) - v).abs() < 1e-9, "{v}");
    }
    // A stored count above the drag range shows a full bar, as stored.
    assert_eq!(s.position_of(501.0), 1.0);
    let shown = s.shown(Some(501.0));
    assert_eq!(shown.text, "501");
    assert_eq!(shown.bar, 1.0);
    assert!(shown.resettable);
    assert_eq!(s.shown(Some(1.0)).text, "1");
    assert!(!s.shown(Some(1.0)).resettable);
    let mixed = s.shown(None);
    assert_eq!(mixed.text, "");
    assert!(mixed.resettable);
}

#[test]
fn a_drag_is_whole_numbers_at_every_grid_and_never_below_one_or_above_fifty() {
    let s = ValueField::MarkerCount.scale();
    for grid in [Grid::Normal, Grid::Coarse, Grid::Fine] {
        for i in 0..=1000 {
            let p = f64::from(i) / 1000.0;
            let v = s.round(s.value_at(p), grid);
            assert_eq!(v, v.round(), "{grid:?} p={p}: {v}");
            assert!((1.0..=50.0).contains(&v), "{grid:?} p={p}: {v}");
        }
        for p in [-5.0, 5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let v = s.round(s.value_at(p), grid);
            assert!(v.is_finite() && v >= 1.0, "{grid:?} p={p}: {v}");
        }
    }
    assert_eq!(s.round(0.2, Grid::Normal), 1.0);
    assert_eq!(s.round(-4.0, Grid::Normal), 1.0);
}

#[test]
fn arrow_steps_stay_inside_one_to_five_hundred() {
    let s = ValueField::MarkerCount.scale();
    assert_eq!(s.step(1.0, -1, Grid::Normal), 1.0);
    assert_eq!(s.step(1.0, 1, Grid::Normal), 2.0);
    assert_eq!(s.step(500.0, 1, Grid::Normal), 500.0);
    assert_eq!(s.step(499.0, 5, Grid::Normal), 500.0);
    assert_eq!(s.step(7.0, 3, Grid::Fine), 10.0);
    assert_eq!(s.step(1.0, -50, Grid::Coarse), 1.0);
    let up = s.step(5.0, 1, Grid::Coarse);
    assert!(up > 5.0 && up == up.round());
}

#[test]
fn the_count_edit_writes_a_whole_count_and_reset_writes_one() {
    let e = ValueField::MarkerCount.edit(7.0);
    assert_eq!(e, StyleEdit::MarkerCount(MarkerCount::new(7).unwrap()));
    assert_eq!(
        ValueField::MarkerCount.reset_edit(),
        StyleEdit::MarkerCount(MarkerCount::new(1).unwrap())
    );
    // Garbage in, no panic, a valid count out.
    for v in [0.0, -3.0, f64::NAN, f64::INFINITY, 1.0e30, 2.49] {
        match ValueField::MarkerCount.edit(v) {
            StyleEdit::MarkerCount(c) => assert!(c.get() >= 1, "{v}"),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        ValueField::from_name("marker-count"),
        Some(ValueField::MarkerCount)
    );
    assert_eq!(
        StyleField::from_name("marker-count"),
        Some(StyleField::MarkerCount)
    );
}

#[test]
fn typed_counts_accept_whole_numbers_one_to_five_hundred() {
    for (text, want) in [("1", 1), ("2", 2), ("37", 37), ("500", 500), (" 12 ", 12)] {
        match StyleField::MarkerCount.parse_text(text) {
            Ok(StyleEdit::MarkerCount(c)) => assert_eq!(c.get(), want, "{text:?}"),
            other => panic!("{text:?}: {other:?}"),
        }
    }
}

#[test]
fn typed_counts_of_zero_fraction_501_or_text_are_refused() {
    for text in [
        "0",
        "2.5",
        "501",
        "1000",
        "abc",
        "",
        " ",
        "-1",
        "-0",
        "1,5",
        "three",
        "NaN",
        "inf",
        "1.5e1",
        "0x10",
        "500.5",
        "99999999999999999999",
    ] {
        assert_eq!(
            StyleField::MarkerCount.parse_text(text),
            Err(StyleEntryError::Count),
            "{text:?}"
        );
    }
    assert_eq!(StyleEntryError::Count.code(), "count");
}

#[test]
fn ambiguous_count_spellings_do_not_panic() {
    for text in ["+3", "3.0", "03", "1e2", "٣", "３", "3 ", "\t4\n", "4_0"] {
        let r = StyleField::MarkerCount.parse_text(text);
        println!("count text {text:?} -> {r:?}");
    }
}

// ------------------------------------- AC 18/20: markers are not geometry

fn big_marker_path() -> (Document, NodeId, ObjectSnapshot, ObjectSnapshot) {
    let doc = Document::new(1);
    let p = doc.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 0.0)),
        ],
        false,
    );
    doc.edit_style(&[p], &StyleEdit::StrokeWidth(mm(10.0)))
        .unwrap();
    let plain = doc.object(p).unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerStart(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Arrow))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerMid(MarkerShape::Dot))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerCount(MarkerCount::new(9).unwrap()))
        .unwrap();
    let marked = doc.object(p).unwrap();
    (doc, p, plain, marked)
}

#[test]
fn bounds_ignore_the_markers() {
    let (_, _, plain, marked) = big_marker_path();
    assert_eq!(object_bounds(&plain), object_bounds(&marked));
    let (a, b) = object_bounds(&marked);
    assert!(a.x >= -0.001 && b.x <= 100.001, "{a:?} {b:?}");
    assert!(
        a.y.abs() < 1e-6 && b.y.abs() < 1e-6,
        "outline bounds only: {a:?} {b:?}"
    );
}

#[test]
fn a_click_on_a_marker_away_from_the_outline_hits_nothing() {
    let (_, p, plain, marked) = big_marker_path();
    let tol = Tolerance::from_mm(1.0);
    // The end arrow (w = 10: 40 long, 30 wide) and the start dot (radius 15).
    for at in [
        pt(115.0, 0.0),
        pt(100.0, 12.0),
        pt(-10.0, 10.0),
        pt(0.0, -12.0),
        pt(50.0, 14.0),
    ] {
        assert_eq!(
            hit_test_object(std::slice::from_ref(&marked), at, tol),
            None,
            "{at:?}"
        );
        assert!(
            hit_test_objects(std::slice::from_ref(&marked), at, tol).is_empty(),
            "{at:?}"
        );
        assert_eq!(
            hit_test_object(std::slice::from_ref(&marked), at, tol),
            hit_test_object(std::slice::from_ref(&plain), at, tol)
        );
    }
    // On the outline it still hits.
    assert_eq!(
        hit_test_object(std::slice::from_ref(&marked), pt(30.0, 0.5), tol),
        Some(p)
    );
    // Same answer for a sweep of points.
    for ix in -20..=130 {
        for iy in -20..=20 {
            let at = pt(f64::from(ix), f64::from(iy));
            assert_eq!(
                hit_test_object(std::slice::from_ref(&marked), at, tol),
                hit_test_object(std::slice::from_ref(&plain), at, tol),
                "{at:?}"
            );
        }
    }
}

#[test]
fn the_eyedropper_does_not_pick_a_marker_on_its_own() {
    let (_, p, plain, marked) = big_marker_path();
    let tol = Tolerance::from_mm(1.0);
    for at in [
        pt(115.0, 0.0),
        pt(100.0, 12.0),
        pt(-10.0, 10.0),
        pt(50.0, 14.0),
    ] {
        assert!(
            pick_colour(std::slice::from_ref(&marked), at, tol).is_none(),
            "{at:?}"
        );
    }
    // A point on the stroke still picks.
    let on = pick_colour(std::slice::from_ref(&marked), pt(30.0, 1.0), tol).unwrap();
    assert_eq!(on.color, Color::BLACK);
    let _ = (p, plain);
}

#[test]
fn a_marker_does_not_hide_an_object_below_or_catch_a_click_over_it() {
    // A rectangle under the marker area: the click on the marker area hits the rectangle.
    let doc = Document::new(1);
    let r = doc.create_rect(RectBounds {
        origin: pt(95.0, 5.0),
        width: mm(40.0),
        height: mm(40.0),
    });
    doc.edit_style(&[r], &StyleEdit::FillEnabled(true)).unwrap();
    let p = doc.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(100.0, 0.0)),
        ],
        false,
    );
    doc.edit_style(&[p], &StyleEdit::StrokeWidth(mm(10.0)))
        .unwrap();
    doc.edit_style(&[p], &StyleEdit::MarkerEnd(MarkerShape::Dot))
        .unwrap();
    let objects: Vec<ObjectSnapshot> = doc
        .object_ids()
        .iter()
        .map(|i| doc.object(*i).unwrap())
        .collect();
    let tol = Tolerance::from_mm(1.0);
    // (100, 12) is inside the dot (radius 15) and inside the filled rectangle.
    assert_eq!(hit_test_object(&objects, pt(100.0, 12.0), tol), Some(r));
    let _ = (MarkerPlace::Spaced, p);
}
