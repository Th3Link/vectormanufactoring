//! Which outlines touch and how they nest (`specs/0035-combine-and-break-apart` criteria 4, 10,
//! 10a and 19): the touch guarantee at the kernel's 0.001 mm, self-crossing, nesting depths, and the
//! kernel budget on a rectangle holding 1000 circles. The budget is asserted in release builds only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    missing_docs
)]

mod common;

use std::time::{Duration, Instant};

use common::{Anchors, circle, polygon, rect};
use curvyo_geometry_core::{Outline, outline_area_mm2, outline_nesting, touching_outlines};

fn outlines(all: &[Anchors]) -> Vec<Outline<'_>> {
    all.iter()
        .map(|anchors| Outline::new(anchors, true))
        .collect()
}

fn shifted(anchors: &Anchors, dx: f64, dy: f64) -> Anchors {
    anchors
        .iter()
        .map(|&(p, i, o)| (curvyo_document_core::Point::new(p.x + dx, p.y + dy), i, o))
        .collect()
}

/// Criterion 10: overlapping squares and squares sharing an edge or a corner touch; squares
/// 0.01 mm apart do not.
#[test]
fn squares_touch_by_overlap_and_shared_edge_but_not_at_ten_microns() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    for (name, b, touches) in [
        ("overlap", rect(5.0, 5.0, 15.0, 15.0), true),
        ("shared edge", rect(10.0, 0.0, 20.0, 10.0), true),
        ("shared corner", rect(10.0, 10.0, 20.0, 20.0), true),
        ("0.0005 apart", rect(10.0005, 0.0, 20.0, 10.0), true),
        ("0.01 apart", rect(10.01, 0.0, 20.0, 10.0), false),
        ("far", rect(50.0, 50.0, 60.0, 60.0), false),
        ("contained, not touching", rect(2.0, 2.0, 8.0, 8.0), false),
    ] {
        let all = [a.clone(), b];
        let found = touching_outlines(&outlines(&all));
        assert_eq!(found == vec![(0, 1)], touches, "{name}: {found:?}");
    }
}

/// Curves: two circles meeting at one point touch; a circle in a ring's hole does not.
#[test]
fn circles_touch_at_a_tangent_point() {
    let all = [circle(0.0, 0.0, 10.0), circle(20.0, 0.0, 10.0)];
    assert_eq!(touching_outlines(&outlines(&all)), vec![(0, 1)]);
    let apart = [circle(0.0, 0.0, 10.0), circle(20.02, 0.0, 10.0)];
    assert_eq!(touching_outlines(&outlines(&apart)), Vec::new());
    let nested = [circle(0.0, 0.0, 20.0), circle(0.0, 0.0, 10.0)];
    assert_eq!(touching_outlines(&outlines(&nested)), Vec::new());
}

/// Criterion 10a: a figure eight touches itself and only itself.
#[test]
fn a_figure_eight_touches_itself() {
    let eight = polygon(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]);
    let square = rect(30.0, 0.0, 40.0, 10.0);
    let all = [eight, square];
    assert_eq!(touching_outlines(&outlines(&all)), vec![(0, 0)]);
    // A plain square does not touch itself.
    let one = [rect(0.0, 0.0, 10.0, 10.0)];
    assert_eq!(touching_outlines(&outlines(&one)), Vec::new());
}

/// Criterion 4: concentric discs nest by depth, whatever their order; a third disc inside the
/// hole is depth 2; a disc beside is depth 0.
#[test]
fn nesting_depth_follows_enclosure() {
    let all = [
        circle(0.0, 0.0, 5.0),
        circle(0.0, 0.0, 20.0),
        circle(0.0, 0.0, 10.0),
        circle(100.0, 0.0, 5.0),
    ];
    let nesting = outline_nesting(&outlines(&all));
    assert_eq!(nesting[1].depth, 0);
    assert_eq!(nesting[1].parent, None);
    assert_eq!(nesting[2].depth, 1);
    assert_eq!(nesting[2].parent, Some(1));
    assert_eq!(nesting[0].depth, 2);
    assert_eq!(nesting[0].parent, Some(2));
    assert_eq!(nesting[3].depth, 0);
}

/// The area of a flattened disc agrees with the circle to well under a square millimetre, and
/// the sign follows the direction of drawing.
#[test]
fn the_area_of_an_outline_is_signed_by_its_direction() {
    let disc = circle(0.0, 0.0, 20.0);
    let area = outline_area_mm2(&Outline::new(&disc, true));
    assert!(
        (area.abs() - std::f64::consts::PI * 400.0).abs() < 1.0,
        "{area}"
    );
    let reversed: Anchors = disc.iter().rev().map(|&(p, i, o)| (p, o, i)).collect();
    let back = outline_area_mm2(&Outline::new(&reversed, true));
    assert!((area + back).abs() < 1e-6, "opposite signs: {area} {back}");
    assert!(
        outline_area_mm2(&Outline::new(
            &polygon(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]),
            true
        ))
        .abs()
            < 1e-9
    );
}

/// Criterion 19 (a), kernel part: one rectangle with 1000 circles of four nodes inside: no
/// touching, depths 0 and 1, within 500 ms in a release build.
#[test]
fn a_rectangle_with_a_thousand_circles_is_decided_quickly() {
    let mut all = vec![rect(-5.0, -5.0, 400.0, 400.0)];
    for i in 0..1000 {
        let (col, row) = (f64::from(i % 40), f64::from(i / 40));
        all.push(shifted(
            &circle(0.0, 0.0, 3.0),
            col * 9.0 + 5.0,
            row * 9.0 + 5.0,
        ));
    }
    let list = outlines(&all);
    let started = Instant::now();
    let touching = touching_outlines(&list);
    let nesting = outline_nesting(&list);
    let took = started.elapsed();
    println!("touch and nesting of 1001 outlines: {took:?}");
    assert!(touching.is_empty(), "{touching:?}");
    assert_eq!(nesting[0].depth, 0);
    assert!(
        nesting[1..]
            .iter()
            .all(|n| n.depth == 1 && n.parent == Some(0))
    );
    if !cfg!(debug_assertions) {
        assert!(took <= Duration::from_millis(500), "{took:?}");
    }
}

/// Criterion 19 (c): 500 squares each inside the previous: depths 0 to 499, none touching.
#[test]
fn five_hundred_nested_squares() {
    let all: Vec<Anchors> = (0..500)
        .map(|i| {
            let inset = f64::from(i) * 0.5;
            rect(inset, inset, 500.0 - inset, 500.0 - inset)
        })
        .collect();
    let list = outlines(&all);
    assert_eq!(touching_outlines(&list), Vec::new());
    let nesting = outline_nesting(&list);
    assert!(nesting.iter().enumerate().all(|(i, n)| n.depth == i));
    assert_eq!(nesting[499].parent, Some(498));
}
