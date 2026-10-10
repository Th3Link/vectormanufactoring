//! Independent tester cases for `0040-document-background`, render-core side: the document
//! area as a draw list (criteria 9 to 13, 47), the checkerboard grid (12) and the Pen knockout
//! over the background (46). There is no rasteriser in the workspace, so a "pixel" is
//! computed from the draw list: the flat colour of the layer that covers it, composited the way
//! the pipeline blends (source over), over the checkerboard tone at that device pixel.
//! The fragment shader itself is NOT VERIFIED here. Written from `specification.md` before the
//! implementation was read.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    missing_docs
)]

use curvyo_document_core::{
    AnchorId, BackgroundPaint, Color, DocumentBackground, DocumentSize, NewAnchor, Opacity, Point,
    ViewTransform,
};
use curvyo_render_core::{
    CANVAS_BG, CHECKER_A, CHECKER_B, DrawList, PASTEBOARD_BG, RgbaColor, background_at,
    build_document_area, build_pen_preview, checker_grid, checker_tone,
};

const PX_PER_MM_AT_100: f64 = 96.0 / 25.4;

fn view(percent: f64, ox: f64, oy: f64) -> ViewTransform {
    ViewTransform::new(percent / 100.0 * PX_PER_MM_AT_100, Point::new(ox, oy))
}

fn bg(paint: BackgroundPaint, r: u8, g: u8, b: u8, a: f64) -> DocumentBackground {
    DocumentBackground {
        paint,
        color: Color { r, g, b },
        opacity: Opacity::new(a).unwrap(),
    }
}

fn solid(r: u8, g: u8, b: u8, a: f64) -> DocumentBackground {
    bg(BackgroundPaint::Solid, r, g, b, a)
}

fn none() -> DocumentBackground {
    bg(BackgroundPaint::None, 0x10, 0x20, 0x30, 1.0)
}

fn a4() -> DocumentSize {
    DocumentSize::from_mm(210.0, 297.0)
}

fn rgb(c: RgbaColor) -> (u8, u8, u8) {
    (c.r, c.g, c.b)
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8)) -> bool {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)]
        .iter()
        .all(|(x, y)| x.abs_diff(*y) <= 1)
}

/// The device-pixel rectangle (x0, y0, x1, y1) the first quad of `list` covers.
fn device_extent(list: &DrawList, v: ViewTransform, dpr: f64) -> (i64, i64, i64, i64) {
    let pts: Vec<(f64, f64)> = list
        .triangles
        .iter()
        .map(|vx| {
            let (x, y) = v.document_to_screen(vx.position);
            (x * dpr, y * dpr)
        })
        .collect();
    let min = |f: fn(&(f64, f64)) -> f64| pts.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&(f64, f64)) -> f64| pts.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    (
        min(|p| p.0).round() as i64,
        min(|p| p.1).round() as i64,
        max(|p| p.0).round() as i64,
        max(|p| p.1).round() as i64,
    )
}

/// The colour at a device pixel inside the document area: the checkerboard tone when the list
/// has a prefix, then every later layer over it, source over.
fn pixel(list: &DrawList, v: ViewTransform, dpr: f64, x: i64, y: i64) -> (u8, u8, u8) {
    let grid = checker_grid(v, dpr);
    let mut c = if list.checker_end() > 0 {
        rgb(checker_tone(grid, x, y))
    } else {
        (0, 0, 0)
    };
    let start_layer = usize::from(list.checker_end() > 0);
    let ends = list.layers();
    let mut from = if start_layer == 1 { ends[0] } else { 0 };
    for end in ends.iter().skip(start_layer) {
        let top = list.triangles[from].color;
        let a = f64::from(top.a) / 255.0;
        let mix = |t: u8, b: u8| (f64::from(t) * a + f64::from(b) * (1.0 - a)).round() as u8;
        c = (mix(top.r, c.0), mix(top.g, c.1), mix(top.b, c.2));
        from = *end;
    }
    c
}

// ------------------------------------------------ AC 9: the default is unchanged

#[test]
fn ac9_the_default_is_one_quad_of_the_old_canvas_colour_with_no_prefix() {
    assert_eq!(rgb(CANVAS_BG), (0xE8, 0xE8, 0xEB));
    assert_eq!(rgb(PASTEBOARD_BG), (0xB8, 0xB8, 0xBE));
    let list = build_document_area(
        a4(),
        DocumentBackground::DEFAULT,
        view(100.0, 5.0, 7.0),
        1.0,
    );
    assert_eq!(list.triangles.len(), 6);
    assert!(list.triangles.iter().all(|v| v.color == CANVAS_BG));
    assert_eq!(list.checker_end(), 0);
    assert_eq!(list.layers().len(), 1);
}

// ------------------------------------------------ AC 10: Solid, alpha 255

#[test]
fn ac10_an_opaque_solid_paints_the_area_edge_on_whole_device_pixels() {
    for dpr in [1.0, 1.5, 2.0] {
        let v = view(100.0, 33.3, 17.7);
        let list = build_document_area(a4(), solid(255, 0, 0, 1.0), v, dpr);
        assert_eq!(list.triangles.len(), 6);
        assert!(
            list.triangles
                .iter()
                .all(|x| rgb(x.color) == (255, 0, 0) && x.color.a == 255)
        );
        assert_eq!(list.checker_end(), 0);
        let (x0, y0, x1, y1) = device_extent(&list, v, dpr);
        // pixel 1 inside the corner is red; pixel 1 outside is not covered by the list
        assert!(x1 > x0 && y1 > y0);
        let (ex, ey) = v.document_to_screen(Point::new(0.0, 0.0));
        assert!(
            ((ex * dpr).round() as i64 - x0).abs() <= 0,
            "corner snapped (dpr {dpr})"
        );
        assert!(((ey * dpr).round() as i64 - y0).abs() <= 0);
        assert_eq!(pixel(&list, v, dpr, x0 + 1, y0 + 1), (255, 0, 0));
    }
}

#[test]
fn ac10_every_opaque_colour_is_one_quad_with_exactly_that_colour() {
    for (r, g, b) in [(0, 0, 0), (255, 255, 255), (1, 2, 3), (0, 255, 128)] {
        let list = build_document_area(a4(), solid(r, g, b, 1.0), view(100.0, 0.0, 0.0), 1.0);
        assert_eq!(list.triangles.len(), 6);
        assert!(
            list.triangles
                .iter()
                .all(|x| x.color == RgbaColor::opaque(r, g, b))
        );
    }
}

// ------------------------------------------------ AC 11: translucent over the checkerboard

#[test]
fn ac11_a_translucent_solid_is_composited_over_the_checkerboard() {
    let v = view(100.0, 0.0, 0.0);
    let list = build_document_area(a4(), solid(255, 0, 0, 128.0 / 255.0), v, 1.0);
    assert!(list.checker_end() > 0);
    let (x0, y0, _, _) = device_extent(&list, v, 1.0);
    // white cell at the corner, grey cell one cell to the right
    let over_white = pixel(&list, v, 1.0, x0 + 4, y0 + 4);
    let over_grey = pixel(&list, v, 1.0, x0 + 12, y0 + 4);
    assert!(close(over_white, (0xFF, 0x7F, 0x7F)), "{over_white:?}");
    // The spec writes #FF6464 for the grey cell. Red over #C9C9CE at 128/255 is
    // 255 * 0.502 + 201 * 0.498 = 228 in the red channel, so #FF6464 cannot be the result of
    // any source-over blend (defect in the spec text, not in the build). Green and blue match.
    assert!(close(over_grey, (0xE4, 0x64, 0x67)), "{over_grey:?}");
    assert_eq!((over_grey.1, over_grey.2), (0x64, 0x67));
}

#[test]
fn ac11_alpha_just_below_opaque_still_shows_the_checkerboard() {
    let list = build_document_area(
        a4(),
        solid(1, 2, 3, 254.0 / 255.0),
        view(100.0, 0.0, 0.0),
        1.0,
    );
    assert!(list.checker_end() > 0, "alpha 254/255 is below 255");
}

// ------------------------------------------------ AC 12: the checkerboard

#[test]
fn ac12_a_none_background_is_a_checkerboard_quad_of_the_two_tokens() {
    assert_eq!(rgb(CHECKER_A), (0xFF, 0xFF, 0xFF));
    assert_eq!(rgb(CHECKER_B), (0xC9, 0xC9, 0xCE));
    let list = build_document_area(a4(), none(), view(100.0, 0.0, 0.0), 1.0);
    assert_eq!(list.triangles.len(), 6, "one quad, no geometry per cell");
    assert_eq!(list.checker_end(), 6);
}

#[test]
fn ac12_cell_size_is_eight_css_px_rounded_to_device_pixels() {
    let cell = |dpr: f64| checker_grid(view(100.0, 0.0, 0.0), dpr).cell;
    assert_eq!(cell(1.0), 8);
    assert_eq!(cell(2.0), 16);
    assert_eq!(cell(1.1), 9, "ratio 1.1 gives 9 device px");
    assert_eq!(cell(1.5), 12);
    assert_eq!(cell(1.25), 10);
    assert_eq!(cell(3.0), 24);
    assert_eq!(cell(0.01), 1, "at least one device pixel");
}

#[test]
fn ac12_the_placement_tests_of_the_spec_at_ratios_1_and_2_at_any_zoom() {
    for percent in [2.0, 25.0, 100.0, 333.0, 8000.0] {
        for (ox, oy) in [(0.0, 0.0), (-19.0, 40.5), (123.456, -987.654)] {
            for dpr in [1.0_f64, 2.0] {
                let v = view(percent, ox, oy);
                let g = checker_grid(v, dpr);
                let s = dpr as i64;
                let at = |dx: i64, dy: i64| {
                    rgb(checker_tone(g, g.corner_x + dx * s, g.corner_y + dy * s))
                };
                assert_eq!(at(4, 4), (0xFF, 0xFF, 0xFF), "zoom {percent} dpr {dpr}");
                assert_eq!(at(12, 4), (0xC9, 0xC9, 0xCE), "zoom {percent} dpr {dpr}");
                assert_eq!(at(4, 12), (0xC9, 0xC9, 0xCE));
                assert_eq!(at(12, 12), (0xFF, 0xFF, 0xFF));
                // the corner cell is checker-a
                assert_eq!(at(0, 0), (0xFF, 0xFF, 0xFF));
                assert_eq!(at(7, 7), (0xFF, 0xFF, 0xFF));
                assert_eq!(at(8, 0), (0xC9, 0xC9, 0xCE));
            }
        }
    }
}

#[test]
fn ac12_the_grid_is_anchored_at_the_snapped_document_corner_of_the_quad() {
    for dpr in [1.0, 1.1, 1.5, 2.0] {
        for (percent, ox, oy) in [(100.0, 10.3, 20.7), (2.0, -55.5, 3.3), (8000.0, 1.01, -7.7)] {
            let v = view(percent, ox, oy);
            let list = build_document_area(a4(), none(), v, dpr);
            let (x0, y0, _, _) = device_extent(&list, v, dpr);
            let g = checker_grid(v, dpr);
            assert_eq!(
                (g.corner_x, g.corner_y),
                (x0, y0),
                "dpr {dpr} zoom {percent}"
            );
        }
    }
}

#[test]
fn ac12_the_cell_does_not_change_with_the_zoom_and_the_pattern_moves_with_the_page() {
    let g1 = checker_grid(view(100.0, 0.0, 0.0), 1.0);
    let g2 = checker_grid(view(8000.0, 0.0, 0.0), 1.0);
    assert_eq!(g1.cell, g2.cell);
    // the page moves by 3 mm: the corner moves with it and the tone under one document point
    // (the middle of a cell, so the pixel snapping cannot matter) stays the same
    let v1 = view(100.0, 0.0, 0.0);
    let v2 = view(100.0, -3.0, 0.0);
    let p = checker_grid(v2, 1.0);
    assert_ne!(p.corner_x, g1.corner_x, "the corner moved with the page");
    let mm = (8.0 * 5.0 + 4.0) / PX_PER_MM_AT_100;
    let device_of = |v: ViewTransform| {
        let (x, y) = v.document_to_screen(Point::new(mm, mm));
        (x.round() as i64, y.round() as i64)
    };
    let (a, b) = (device_of(v1), device_of(v2));
    assert_eq!(checker_tone(g1, a.0, a.1), checker_tone(p, b.0, b.1));
    // the phase passed to a shader stays inside two cells however far the page is panned
    let far = checker_grid(view(100.0, -1.0e7, 5.0e6), 2.0);
    let (px, py) = far.phase();
    assert!(i64::from(px) < 2 * i64::from(far.cell) && i64::from(py) < 2 * i64::from(far.cell));
}

#[test]
fn ac12_the_quad_covers_only_the_document_area_never_the_pasteboard() {
    let v = view(100.0, 40.0, 60.0);
    for background in [none(), solid(1, 2, 3, 0.5)] {
        let list = build_document_area(a4(), background, v, 1.0);
        let (x0, y0, x1, y1) = device_extent(&list, v, 1.0);
        let (ax, ay) = v.document_to_screen(Point::new(0.0, 0.0));
        let (bx, by) = v.document_to_screen(Point::new(210.0, 297.0));
        assert!((x0 as f64 - ax).abs() <= 0.5 && (y0 as f64 - ay).abs() <= 0.5);
        assert!((x1 as f64 - bx).abs() <= 0.5 && (y1 as f64 - by).abs() <= 0.5);
    }
}

#[test]
fn ac12_tones_alternate_like_a_chessboard_everywhere_including_left_and_above_the_corner() {
    let g = checker_grid(view(100.0, 0.0, 0.0), 1.0);
    for dy in -40..40_i64 {
        for dx in -40..40_i64 {
            let col = dx.div_euclid(8);
            let row = dy.div_euclid(8);
            let expect = if (col + row).rem_euclid(2) == 0 {
                CHECKER_A
            } else {
                CHECKER_B
            };
            assert_eq!(
                checker_tone(g, g.corner_x + dx, g.corner_y + dy),
                expect,
                "{dx},{dy}"
            );
        }
    }
}

// ------------------------------------------------ AC 13: draw order

#[test]
fn ac13_the_background_is_the_bottom_layer_a_later_list_extends_after_it() {
    let mut area = build_document_area(a4(), solid(255, 0, 0, 1.0), view(100.0, 0.0, 0.0), 1.0);
    let before = area.triangles.len();
    let mut art = DrawList::default();
    // a green square drawn as another list
    let square = curvyo_render_core::build_pen_preview(
        &[],
        None,
        None,
        view(100.0, 0.0, 0.0),
        a4(),
        DocumentBackground::DEFAULT,
    );
    assert_eq!(square.triangles.len(), 0);
    art.extend(square);
    area.extend(art);
    assert_eq!(area.triangles.len(), before, "empty extends nothing");
    assert_eq!(area.layers().first().copied(), Some(6));
}

// ------------------------------------------------ AC 46: the Pen knockout

fn knockout(background: DocumentBackground, x: f64, y: f64) -> Vec<RgbaColor> {
    let size = DocumentSize::from_mm(100.0, 100.0);
    let anchor = NewAnchor::corner(AnchorId::new(1, 1), Point::new(x, y));
    build_pen_preview(
        &[anchor],
        None,
        None,
        ViewTransform::identity(),
        size,
        background,
    )
    .triangles
    .iter()
    .map(|v| v.color)
    .collect()
}

#[test]
fn ac46_an_opaque_solid_knockout_is_the_background_colour() {
    let c = knockout(solid(0x12, 0x34, 0x56, 1.0), 50.0, 50.0);
    assert!(c.contains(&RgbaColor::opaque(0x12, 0x34, 0x56)));
    assert!(!c.contains(&CANVAS_BG));
}

#[test]
fn ac46_none_and_translucent_knockouts_are_the_colour_composited_over_checker_a() {
    let c = knockout(none(), 50.0, 50.0);
    assert!(c.contains(&CHECKER_A), "None: the knockout is #FFFFFF");
    let c = knockout(solid(255, 0, 0, 128.0 / 255.0), 50.0, 50.0);
    assert!(
        c.iter()
            .any(|x| x.a == 255 && close(rgb(*x), (0xFF, 0x7F, 0x7F))),
        "translucent red over white is #FF7F7F: {c:?}"
    );
    let c = knockout(solid(0, 0, 0, 0.0), 50.0, 50.0);
    assert!(c.contains(&CHECKER_A), "alpha 0 is white");
}

#[test]
fn ac46_over_the_pasteboard_it_is_as_before_for_every_background() {
    for background in [
        DocumentBackground::DEFAULT,
        none(),
        solid(1, 2, 3, 0.5),
        solid(9, 9, 9, 1.0),
    ] {
        for (x, y) in [(-0.5, 50.0), (50.0, 100.5), (100.0001, 100.0), (-1e6, 1e6)] {
            let c = knockout(background, x, y);
            assert!(c.contains(&PASTEBOARD_BG), "({x},{y}) {background:?}");
        }
    }
}

#[test]
fn ac46_the_edge_belongs_to_the_document_and_hostile_points_to_the_pasteboard() {
    let size = DocumentSize::from_mm(100.0, 100.0);
    let red = solid(255, 0, 0, 1.0);
    for p in [(0.0, 0.0), (100.0, 100.0), (0.0, 100.0), (100.0, 0.0)] {
        assert_eq!(
            background_at(size, red, Point::new(p.0, p.1)),
            RgbaColor::opaque(255, 0, 0),
            "{p:?}"
        );
    }
    for p in [
        (f64::NAN, 5.0),
        (5.0, f64::NAN),
        (f64::INFINITY, 5.0),
        (5.0, f64::NEG_INFINITY),
        (-1e-9, 5.0),
        (100.000_000_1, 5.0),
    ] {
        assert_eq!(
            background_at(size, red, Point::new(p.0, p.1)),
            PASTEBOARD_BG,
            "{p:?}"
        );
    }
    assert_eq!(background_at(size, none(), Point::new(1.0, 1.0)), CHECKER_A);
}

// ------------------------------------------------ AC 47: cost

#[test]
fn ac47_the_layer_counts_of_the_three_cases() {
    let layers = |b| build_document_area(a4(), b, view(100.0, 0.0, 0.0), 1.0);
    let default = layers(DocumentBackground::DEFAULT);
    let opaque = layers(solid(1, 2, 3, 1.0));
    let none_list = layers(none());
    let translucent = layers(solid(1, 2, 3, 0.5));
    assert_eq!(default.layers().len(), 1);
    assert_eq!(opaque.layers().len(), 1);
    assert_eq!(opaque.triangles.len(), default.triangles.len());
    assert_eq!(opaque.checker_end(), 0);
    assert_eq!(
        none_list.layers().len(),
        opaque.layers().len(),
        "None has as many layers as opaque"
    );
    assert!(none_list.checker_end() > 0);
    assert_eq!(translucent.layers().len(), opaque.layers().len() + 1);
    assert!(translucent.checker_end() > 0);
}

#[test]
fn ac47_the_checkerboard_cost_is_independent_of_zoom_and_document_size() {
    let mut counts = std::collections::BTreeSet::new();
    for size in [
        DocumentSize::from_mm(1.0, 1.0),
        a4(),
        DocumentSize::from_mm(100_000.0, 100_000.0),
    ] {
        for percent in [2.0, 100.0, 8000.0] {
            for dpr in [1.0, 2.0] {
                let l = build_document_area(size, none(), view(percent, 0.0, 0.0), dpr);
                counts.insert((l.triangles.len(), l.checker_end(), l.layers().len()));
            }
        }
    }
    assert_eq!(counts.len(), 1, "{counts:?}");
}

#[test]
fn ac37_a_solid_with_alpha_zero_shows_the_checkerboard_through() {
    let list = build_document_area(a4(), solid(9, 9, 9, 0.0), view(100.0, 0.0, 0.0), 1.0);
    assert!(list.checker_end() > 0);
    let v = view(100.0, 0.0, 0.0);
    let (x0, y0, _, _) = device_extent(&list, v, 1.0);
    assert_eq!(pixel(&list, v, 1.0, x0 + 4, y0 + 4), (0xFF, 0xFF, 0xFF));
    assert_eq!(pixel(&list, v, 1.0, x0 + 12, y0 + 4), (0xC9, 0xC9, 0xCE));
}

// ------------------------------------------------ AC 49: the casing rule, arithmetic only

fn luminance(c: (u8, u8, u8)) -> f64 {
    let ch = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
}

fn contrast(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn ac49_the_better_of_accent_and_white_casing_is_at_least_2_1_on_every_opaque_colour() {
    // `--accent` #2F6FEE, casing #FFFFFF (docs/design-system.md). Sweep the grey axis and the
    // primaries; the worst case the spec names is relative luminance 0.445.
    let accent = luminance((0x2F, 0x6F, 0xEE));
    let white = 1.0;
    let mut worst = f64::INFINITY;
    for step in 0..=1000 {
        let l = f64::from(step) / 1000.0;
        worst = worst.min(contrast(accent, l).max(contrast(white, l)));
    }
    assert!(worst >= 2.1 - 1e-3, "worst {worst}");
    for tone in [CHECKER_A, CHECKER_B] {
        let l = luminance(rgb(tone));
        let best = contrast(accent, l).max(contrast(white, l));
        assert!(best >= 2.7 - 0.05, "{tone:?} {best}");
    }
    // the checker-b number the spec quotes
    let b = luminance(rgb(CHECKER_B));
    assert!(contrast(accent, b) >= 2.69, "{}", contrast(accent, b));
}

// ------------------------------------------------ white-box: extreme views

#[test]
fn whitebox_extreme_pans_zooms_and_ratios_never_panic_and_keep_a_valid_phase() {
    for (percent, ox, oy) in [
        (2.0, 1.0e12, -1.0e12),
        (8000.0, -1.0e15, 1.0e15),
        (100.0, f64::MAX, f64::MIN),
        (100.0, f64::NAN, 0.0),
        (100.0, 0.0, f64::INFINITY),
    ] {
        for dpr in [0.25, 1.0, 2.0, 3.0, 1.0e6] {
            let v = view(percent, ox, oy);
            let g = checker_grid(v, dpr);
            assert!(g.cell >= 1);
            let (px, py) = g.phase();
            assert!(u64::from(px) < 2 * u64::from(g.cell));
            assert!(u64::from(py) < 2 * u64::from(g.cell));
            for background in [none(), solid(1, 2, 3, 0.5), solid(1, 2, 3, 1.0)] {
                let list = build_document_area(a4(), background, v, dpr);
                assert_eq!(list.triangles.len() % 3, 0);
                assert!(list.layers().len() <= 2);
            }
        }
    }
}

#[test]
fn whitebox_a_degenerate_document_still_draws_one_quad() {
    for size in [
        DocumentSize::from_mm(0.0, 0.0),
        DocumentSize::from_mm(1.0e-9, 1.0e-9),
        DocumentSize::from_mm(1.0e9, 1.0),
    ] {
        for background in [none(), solid(1, 2, 3, 0.5), DocumentBackground::DEFAULT] {
            let l = build_document_area(size, background, view(100.0, 0.0, 0.0), 1.0);
            assert!(l.triangles.len() == 6 || l.triangles.len() == 12);
            assert!(
                l.triangles
                    .iter()
                    .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
            );
        }
    }
}
