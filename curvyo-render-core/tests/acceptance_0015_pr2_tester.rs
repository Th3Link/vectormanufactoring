//! Independent tester tests for PR 2 (rulers and pasteboard) of
//! `specs/0015-document-size-and-rulers`, `render-core` side: the document
//! area over the pasteboard (criterion 28) and the Pen's knockout colour
//! (criterion 31).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]

use curvyo_document_core::{AnchorId, DocumentSize, NewAnchor, Point, ViewTransform};
use curvyo_render_core::{
    CANVAS_BG, PASTEBOARD_BG, RgbaColor, background_at, build_document_area, build_pen_preview,
};

const PX_PER_MM_AT_100: f64 = 96.0 / 25.4;

fn view(percent: f64, ox: f64, oy: f64) -> ViewTransform {
    ViewTransform::new(percent / 100.0 * PX_PER_MM_AT_100, Point::new(ox, oy))
}

#[test]
fn ac28_colours_are_the_specified_hex_values() {
    assert_eq!(
        (CANVAS_BG.r, CANVAS_BG.g, CANVAS_BG.b, CANVAS_BG.a),
        (0xE8, 0xE8, 0xEB, 0xFF)
    );
    assert_eq!(
        (
            PASTEBOARD_BG.r,
            PASTEBOARD_BG.g,
            PASTEBOARD_BG.b,
            PASTEBOARD_BG.a
        ),
        (0xB8, 0xB8, 0xBE, 0xFF)
    );
    assert_ne!(CANVAS_BG, PASTEBOARD_BG);
}

#[test]
fn ac28_background_at_inside_edge_outside() {
    let size = DocumentSize::from_mm(210.0, 297.0);
    assert_eq!(background_at(size, Point::new(105.0, 148.0)), CANVAS_BG);
    assert_eq!(background_at(size, Point::new(0.0, 0.0)), CANVAS_BG);
    assert_eq!(background_at(size, Point::new(210.0, 297.0)), CANVAS_BG);
    for p in [
        Point::new(-0.001, 10.0),
        Point::new(10.0, -0.001),
        Point::new(210.001, 10.0),
        Point::new(10.0, 297.001),
        Point::new(-500.0, -500.0),
        Point::new(1e9, 1e9),
    ] {
        assert_eq!(background_at(size, p), PASTEBOARD_BG, "{p:?}");
    }
    // a degenerate point must not pick the document colour
    assert_eq!(
        background_at(size, Point::new(f64::NAN, 5.0)),
        PASTEBOARD_BG
    );
    assert_eq!(
        background_at(size, Point::new(5.0, f64::INFINITY)),
        PASTEBOARD_BG
    );
}

fn tri_area(a: Point, b: Point, c: Point) -> f64 {
    ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
}

#[test]
fn ac28_area_is_one_flat_rect_snapped_to_device_pixels() {
    let sizes = [
        DocumentSize::from_mm(210.0, 297.0),
        DocumentSize::from_mm(1.0, 1.0),
        DocumentSize::from_mm(100_000.0, 100_000.0),
        DocumentSize::from_mm(300.0, 400.0),
    ];
    for size in sizes {
        for pct in [2.0, 7.3, 100.0, 333.3, 8000.0] {
            for (ox, oy) in [
                (0.0, 0.0),
                (-19.0, -19.0),
                (123.456, -987.654),
                (-70.5, 40.25),
            ] {
                for dpr in [1.0, 1.25, 1.5, 2.0, 3.0] {
                    let v = view(pct, ox, oy);
                    let list = build_document_area(size, v, dpr);
                    assert_eq!(list.triangles.len(), 6, "two triangles");
                    assert!(list.triangles.iter().all(|vx| vx.color == CANVAS_BG));
                    assert!(
                        list.triangles
                            .iter()
                            .all(|vx| vx.position.x.is_finite() && vx.position.y.is_finite())
                    );
                    // screen-space corners lie on whole device pixels
                    let screen: Vec<(f64, f64)> = list
                        .triangles
                        .iter()
                        .map(|vx| v.document_to_screen(vx.position))
                        .collect();
                    for (sx, sy) in &screen {
                        let (dx, dy) = (sx * dpr, sy * dpr);
                        // large coordinates lose absolute precision; scale tolerance
                        let tol = 1e-6_f64.max(dx.abs().max(dy.abs()) * 1e-12);
                        assert!(
                            (dx - dx.round()).abs() < tol,
                            "x {sx} not on device px (dpr {dpr}, {pct}%)"
                        );
                        assert!(
                            (dy - dy.round()).abs() < tol,
                            "y {sy} not on device px (dpr {dpr}, {pct}%)"
                        );
                    }
                    // rect extent equals the document within half a device px per edge
                    let (x0, y0) = v.document_to_screen(Point::new(0.0, 0.0));
                    let (x1, y1) =
                        v.document_to_screen(Point::new(size.width.as_mm(), size.height.as_mm()));
                    let min_x = screen.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
                    let max_x = screen.iter().map(|s| s.0).fold(f64::NEG_INFINITY, f64::max);
                    let min_y = screen.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
                    let max_y = screen.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
                    let half = 0.5 / dpr + 1e-6 + x1.abs().max(y1.abs()) * 1e-12;
                    assert!(
                        (min_x - x0).abs() <= half && (max_x - x1).abs() <= half,
                        "x extent {min_x}..{max_x} vs {x0}..{x1}"
                    );
                    assert!(
                        (min_y - y0).abs() <= half && (max_y - y1).abs() <= half,
                        "y extent"
                    );
                    // the two triangles tile the rectangle exactly (no gap, no overlap)
                    let p: Vec<Point> = list.triangles.iter().map(|vx| vx.position).collect();
                    let area = tri_area(p[0], p[1], p[2]) + tri_area(p[3], p[4], p[5]);
                    let doc_w = (max_x - min_x) / v.scale();
                    let doc_h = (max_y - min_y) / v.scale();
                    let rect = doc_w * doc_h;
                    assert!(
                        (area - rect).abs() <= rect.abs() * 1e-9 + 1e-9,
                        "tiling {area} vs {rect}"
                    );
                }
            }
        }
    }
}

#[test]
fn ac28_area_follows_pan_and_zoom() {
    let size = DocumentSize::from_mm(210.0, 297.0);
    let a = build_document_area(size, view(100.0, 0.0, 0.0), 1.0);
    let b = build_document_area(size, view(100.0, -50.0, 0.0), 1.0);
    // the document rectangle is in document space, the view moves: the
    // document-space rectangle is the same, only the snapping may differ
    let max_x = |l: &curvyo_render_core::DrawList| {
        l.triangles
            .iter()
            .map(|v| v.position.x)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert!((max_x(&a) - 210.0).abs() < 0.5 && (max_x(&b) - 210.0).abs() < 0.5);
}

fn knockout_colours(x: f64, y: f64, size: DocumentSize, as_pending: bool) -> Vec<RgbaColor> {
    let anchor = NewAnchor::corner(AnchorId::new(1, 1), Point::new(x, y));
    let list = if as_pending {
        build_pen_preview(&[], None, Some(&anchor), ViewTransform::identity(), size)
    } else {
        build_pen_preview(&[anchor], None, None, ViewTransform::identity(), size)
    };
    list.triangles.iter().map(|v| v.color).collect()
}

#[test]
fn ac31_pen_knockout_matches_the_colour_behind_it() {
    let size = DocumentSize::from_mm(100.0, 100.0);
    for pending in [false, true] {
        {
            // inside
            for (x, y) in [(50.0, 50.0), (0.0, 0.0), (100.0, 100.0), (99.9, 0.1)] {
                let c = knockout_colours(x, y, size, pending);
                assert!(
                    c.contains(&CANVAS_BG) && !c.contains(&PASTEBOARD_BG),
                    "({x},{y}) pending {pending}"
                );
            }
            // pasteboard: left, above, right, below, far
            for (x, y) in [
                (-30.0, 50.0),
                (50.0, -30.0),
                (130.0, 50.0),
                (50.0, 130.0),
                (-5e5, 7e5),
            ] {
                let c = knockout_colours(x, y, size, pending);
                assert!(
                    c.contains(&PASTEBOARD_BG) && !c.contains(&CANVAS_BG),
                    "({x},{y}) pending {pending}"
                );
            }
        }
    }
}

#[test]
fn ac31_knockout_follows_a_resized_document() {
    // The same point is pasteboard in a small document and document in a large one.
    let p = (150.0, 150.0);
    let small = knockout_colours(p.0, p.1, DocumentSize::from_mm(100.0, 100.0), false);
    let large = knockout_colours(p.0, p.1, DocumentSize::from_mm(300.0, 400.0), false);
    assert!(small.contains(&PASTEBOARD_BG) && !small.contains(&CANVAS_BG));
    assert!(large.contains(&CANVAS_BG) && !large.contains(&PASTEBOARD_BG));
}

#[test]
fn ac31_pen_preview_with_no_nodes_is_empty_whatever_the_size() {
    let list = build_pen_preview(
        &[],
        None,
        None,
        ViewTransform::identity(),
        DocumentSize::from_mm(1.0, 1.0),
    );
    assert_eq!(list.triangles.len(), 0);
}
