//! Independent tester tests for the marquee and lasso overlay of
//! `specs/0014-advanced-selection/` (UX notes: 12 % fill, 1.5 px border, green and
//! red, dashed 4/3 green lasso).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    missing_docs
)]

use curvyo_document_core::{Point, ViewTransform};
use curvyo_render_core::{MarqueeOverlay, build_marquee_overlay};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn view() -> ViewTransform {
    ViewTransform::new(1.0, pt(0.0, 0.0))
}

const GREEN: (u8, u8, u8) = (0x1C, 0x93, 0x47);
const RED: (u8, u8, u8) = (0xE5, 0x48, 0x4D);

fn rgb(v: &curvyo_render_core::Vertex) -> (u8, u8, u8) {
    (v.color.r, v.color.g, v.color.b)
}

#[test]
fn box_colours_and_fill_alpha() {
    for (contain, want, other) in [(false, GREEN, RED), (true, RED, GREEN)] {
        let l = build_marquee_overlay(
            view(),
            &MarqueeOverlay::Box {
                from: pt(0.0, 0.0),
                to: pt(40.0, 30.0),
                contain,
            },
            1.0,
        );
        assert!(
            l.triangles.iter().all(|v| rgb(v) == want),
            "contain {contain}"
        );
        assert!(l.triangles.iter().all(|v| rgb(v) != other));
        assert!(l.triangles.iter().any(|v| v.color.a == 255), "border");
        assert!(l.triangles.iter().any(|v| v.color.a == 31), "12 % fill");
    }
}

#[test]
fn box_is_symmetric_in_corner_order_and_degenerate_boxes_do_not_panic() {
    let a = build_marquee_overlay(
        view(),
        &MarqueeOverlay::Box {
            from: pt(0.0, 0.0),
            to: pt(40.0, 30.0),
            contain: false,
        },
        1.0,
    );
    let b = build_marquee_overlay(
        view(),
        &MarqueeOverlay::Box {
            from: pt(40.0, 30.0),
            to: pt(0.0, 0.0),
            contain: false,
        },
        1.0,
    );
    assert_eq!(a.triangle_count(), b.triangle_count());
    for (f, t) in [
        (pt(5.0, 5.0), pt(5.0, 5.0)),
        (pt(5.0, 5.0), pt(5.0, 50.0)),
        (pt(5.0, 5.0), pt(50.0, 5.0)),
        (pt(f64::MAX, 0.0), pt(0.0, f64::MAX)),
    ] {
        let _ = build_marquee_overlay(
            view(),
            &MarqueeOverlay::Box {
                from: f,
                to: t,
                contain: true,
            },
            1.0,
        );
    }
}

#[test]
fn lasso_is_green_and_dashed_four_on_three_off() {
    // A straight 70 px line: ten patterns of 4 + 3 would be 10 dashes.
    let l = build_marquee_overlay(
        view(),
        &MarqueeOverlay::Lasso(vec![pt(0.0, 0.0), pt(70.0, 0.0)]),
        1.0,
    );
    assert!(l.triangles.iter().all(|v| rgb(v) == GREEN));
    // Dashes cover x ranges; collect x extents of triangles and merge.
    let mut spans: Vec<(f64, f64)> = l
        .triangles
        .chunks(3)
        .map(|t| {
            let xs = t.iter().map(|v| v.position.x);
            (
                xs.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
            )
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = vec![];
    for (lo, hi) in spans {
        match merged.last_mut() {
            Some(last) if lo <= last.1 + 1e-6 => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    assert!(
        merged.len() >= 9 && merged.len() <= 11,
        "about 10 dashes, got {}",
        merged.len()
    );
    for w in merged.windows(2) {
        let gap = w[1].0 - w[0].1;
        assert!((gap - 3.0).abs() < 1.0, "gap {gap}");
    }
}

#[test]
fn lasso_degenerate_inputs_do_not_panic() {
    for line in [
        vec![],
        vec![pt(1.0, 1.0)],
        vec![pt(1.0, 1.0), pt(1.0, 1.0)],
        vec![pt(f64::NAN, 0.0), pt(1.0, 1.0)],
    ] {
        let _ = build_marquee_overlay(view(), &MarqueeOverlay::Lasso(line), 1.0);
    }
}

/// Defect (architect review, finding 2): no cap on the dash count.
#[test]
fn a_huge_lasso_line_has_a_bounded_triangle_count() {
    let tris = |len: f64| {
        build_marquee_overlay(
            view(),
            &MarqueeOverlay::Lasso(vec![pt(0.0, 0.0), pt(len, 0.0)]),
            1.0,
        )
        .triangle_count()
    };
    let a = tris(1e5);
    let b = tris(2e6);
    assert!(
        b < a * 4,
        "20x the length gave {b} triangles against {a}: unbounded"
    );
}
