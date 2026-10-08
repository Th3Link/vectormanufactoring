//! Independent tester acceptance tests for PR 2 of
//! `specs/edit-interaction-polish/specification.md`, Part E (criteria 63 to
//! 67: the dashed selection box, closed corners, pixel-aligned lines, solid
//! hover box). Written from the specification before the implementation was
//! read. The tests drive `build_select_draw_list` (the public entry point of
//! the Select tool's boxes) and measure the draw list geometrically: dashes
//! are recovered from the triangles in device pixels, never from the
//! implementation's own helper functions.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(missing_docs, clippy::doc_markdown, clippy::type_complexity)]
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use curvyo_document_core::{Document, Length, NodeId, Point, RectBounds, ViewTransform};
use curvyo_render_core::{DrawList, RgbaColor, SelectDecorationInput, build_select_draw_list};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

const ACCENT: RgbaColor = RgbaColor::opaque(0x2F, 0x6F, 0xEE);

/// The hover box colour: `--hover-box`, `--accent` at 60% (`0007` criterion
/// 41; it was `--accent-hover` at 20% when this test was written).
fn accent_hover() -> RgbaColor {
    RgbaColor { a: 153, ..ACCENT }
}

/// The white casings drawn under the box lines (`0007` criterion 40): not part
/// of the box lines these tests measure.
fn is_casing(colour: RgbaColor) -> bool {
    colour == RgbaColor::WHITE
        || colour
            == (RgbaColor {
                a: 153,
                ..RgbaColor::WHITE
            })
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn some_id() -> NodeId {
    let d = Document::new(1);
    d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(1.0),
        height: Length::from_mm(1.0),
    })
}

type Quad = [Point; 4];

/// Corners (in screen pixels) of an `w` x `h` box centred at `(cx, cy)` and
/// turned by `deg` degrees, in order around the perimeter.
fn box_px(cx: f64, cy: f64, w: f64, h: f64, deg: f64) -> [(f64, f64); 4] {
    let (s, c) = deg.to_radians().sin_cos();
    let local = [
        (-w / 2.0, -h / 2.0),
        (w / 2.0, -h / 2.0),
        (w / 2.0, h / 2.0),
        (-w / 2.0, h / 2.0),
    ];
    local.map(|(x, y)| (cx + x * c - y * s, cy + x * s + y * c))
}

fn to_doc(view: ViewTransform, p: (f64, f64)) -> Point {
    view.screen_to_document(p.0, p.1)
}

fn doc_quad(view: ViewTransform, corners: [(f64, f64); 4]) -> Quad {
    corners.map(|p| to_doc(view, p))
}

fn view_of(scale: f64, ox: f64, oy: f64) -> ViewTransform {
    ViewTransform::new(scale, pt(ox, oy))
}

fn build(
    view: ViewTransform,
    selected: &[[(f64, f64); 4]],
    hovered: Option<[(f64, f64); 4]>,
    dpr: f64,
) -> DrawList {
    let id = some_id();
    build_select_draw_list(
        view,
        &SelectDecorationInput {
            selected: selected.iter().map(|b| (id, doc_quad(view, *b))).collect(),
            hovered: hovered.map(|b| (id, doc_quad(view, b))),
            device_pixel_ratio: dpr,
            skew_guide: None,
        },
    )
}

type Tri = [(f64, f64); 3];

/// The triangles of one colour, in device pixels.
fn device_tris(list: &DrawList, view: ViewTransform, dpr: f64, colour: RgbaColor) -> Vec<Tri> {
    list.triangles
        .chunks(3)
        .filter(|t| t.iter().all(|v| v.color == colour))
        .map(|t| {
            t.iter()
                .map(|v| {
                    let (x, y) = view.document_to_screen(v.position);
                    (x * dpr, y * dpr)
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
        .collect()
}

fn all_colours(list: &DrawList) -> Vec<RgbaColor> {
    let mut out: Vec<RgbaColor> = Vec::new();
    for v in &list.triangles {
        if !out.contains(&v.color) && !is_casing(v.color) {
            out.push(v.color);
        }
    }
    out
}

/// The dashes of the edge `p0 -> p1` (device pixels): triangles lying inside
/// `|perpendicular| <= band` and `t in [-slack, L + slack]` merged into
/// intervals along the edge, as (start, end) from the corner `p0`.
fn dashes_along(tris: &[Tri], p0: (f64, f64), p1: (f64, f64), band: f64) -> Vec<(f64, f64)> {
    let l = (p1.0 - p0.0).hypot(p1.1 - p0.1);
    let u = ((p1.0 - p0.0) / l, (p1.1 - p0.1) / l);
    let n = (-u.1, u.0);
    let mut iv: Vec<(f64, f64)> = Vec::new();
    for t in tris {
        let mut tmin = f64::MAX;
        let mut tmax = f64::MIN;
        let mut ok = true;
        for v in t {
            let d = (v.0 - p0.0, v.1 - p0.1);
            let along = d.0 * u.0 + d.1 * u.1;
            let across = d.0 * n.0 + d.1 * n.1;
            if across.abs() > band || along < -3.5 || along > l + 3.5 {
                ok = false;
            }
            tmin = tmin.min(along);
            tmax = tmax.max(along);
        }
        if ok && tmax - tmin > 1e-9 {
            iv.push((tmin, tmax));
        }
    }
    iv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (a, b) in iv {
        match merged.last_mut() {
            Some(last) if a <= last.1 + 1e-6 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    merged
}

/// Whether some count of dashes of exactly 4 with gaps in 2..=4 spans `l`.
fn exact_fit(l: f64) -> bool {
    (2..=400).any(|n| {
        let g = (l - 4.0 * f64::from(n)) / (f64::from(n) - 1.0);
        (2.0 - 1e-9..=4.0 + 1e-9).contains(&g)
    })
}

/// Checks the pattern of one edge of centre-to-centre length `l` (device px
/// = screen px at dpr 1) against criterion 64. `dashes` are intervals along
/// the edge from the corner; the end dashes reach past the corner by half a
/// line width, so they are clipped to `[0, l]` before measuring.
fn check_pattern(l: f64, dashes: &[(f64, f64)], what: &str) -> Result<(), String> {
    let clipped: Vec<(f64, f64)> = dashes
        .iter()
        .map(|&(a, b)| (a.max(0.0), b.min(l)))
        .collect();
    if l < 10.0 {
        if clipped.len() != 1 || clipped[0].0 > 1e-6 || clipped[0].1 < l - 1e-6 {
            return Err(format!(
                "{what}: edge {l} under 10 px must be solid: {clipped:?}"
            ));
        }
        return Ok(());
    }
    let n = clipped.len();
    if n < 2 {
        return Err(format!("{what}: edge {l} has {n} dashes"));
    }
    let tol = 1e-6;
    // closed corners: a dash at both ends
    if clipped[0].0 > tol || clipped[n - 1].1 < l - tol {
        return Err(format!(
            "{what}: corner not closed at edge {l}: {clipped:?}"
        ));
    }
    let len: Vec<f64> = clipped.iter().map(|d| d.1 - d.0).collect();
    let gap: Vec<f64> = clipped.windows(2).map(|w| w[1].0 - w[0].1).collect();
    for (i, d) in len.iter().enumerate() {
        if *d > 4.0 + tol {
            return Err(format!("{what}: dash {i} = {d} longer than 4 at edge {l}"));
        }
        if *d < 2.5 - tol {
            return Err(format!(
                "{what}: dash {i} = {d} shorter than 2.5 at edge {l}"
            ));
        }
    }
    for (i, g) in gap.iter().enumerate() {
        if *g < 2.0 - tol || *g > 4.0 + tol {
            return Err(format!("{what}: gap {i} = {g} outside 2..4 at edge {l}"));
        }
    }
    // symmetric about the edge centre
    for i in 0..n {
        let (a, b) = (clipped[i], clipped[n - 1 - i]);
        if ((a.0 + b.1) - l).abs() > 1e-5 || ((a.1 + b.0) - l).abs() > 1e-5 {
            return Err(format!(
                "{what}: not symmetric about the centre at edge {l}: {clipped:?}"
            ));
        }
    }
    if exact_fit(l) {
        for (i, d) in len.iter().enumerate() {
            if (d - 4.0).abs() > 1e-5 {
                return Err(format!(
                    "{what}: an exact fit exists at {l} but dash {i} = {d}"
                ));
            }
        }
    } else {
        for (i, g) in gap.iter().enumerate() {
            if (g - 2.0).abs() > 1e-5 {
                return Err(format!(
                    "{what}: no exact fit at {l}: gap {i} = {g}, spec says 2"
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Criterion 63, 64: dash pattern on rotated (unsnapped) boxes, exact edges
// ---------------------------------------------------------------------

fn edge_lengths_sweep() -> Vec<f64> {
    let mut v: Vec<f64> = Vec::new();
    let mut l = 10.0;
    while l <= 400.0 {
        v.push(l);
        l += 0.37;
    }
    for base in [
        10.0, 12.0, 16.0, 20.0, 22.0, 9.99, 10.01, 11.99, 12.01, 15.99, 16.01, 19.99, 20.01, 21.99,
        22.01,
    ] {
        v.push(base);
    }
    for i in 100..=4000 {
        v.push(f64::from(i) / 10.0);
    }
    v
}

#[test]
fn ac63_ac64_pattern_on_every_edge_length_10_to_400_px_rotated_box() {
    let sweep = edge_lengths_sweep();
    let mut failures = Vec::new();
    for (i, w) in sweep.iter().enumerate() {
        let h = 31.0 + f64::from((i % 7) as u32) * 3.3;
        let deg = 17.0 + (i % 5) as f64 * 11.0;
        let view = ViewTransform::identity();
        let b = box_px(200.0, 200.0, *w, h, deg);
        let list = build(view, &[b], None, 1.0);
        let tris = device_tris(&list, view, 1.0, ACCENT);
        assert_ne!(tris.len(), 0);
        for e in 0..4 {
            let (p0, p1) = (b[e], b[(e + 1) % 4]);
            let l = (p1.0 - p0.0).hypot(p1.1 - p0.1);
            let d = dashes_along(&tris, p0, p1, 0.75);
            if let Err(msg) = check_pattern(l, &d, &format!("w={w} h={h} deg={deg} edge {e}")) {
                failures.push(msg);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures, first: {:#?}",
        failures.len(),
        &failures[..failures.len().min(8)]
    );
}

#[test]
fn ac64_edges_under_10_px_are_solid_and_the_box_is_still_drawn() {
    for l in [0.5, 1.0, 3.0, 5.0, 8.0, 9.0, 9.9, 9.999] {
        let view = ViewTransform::identity();
        let b = box_px(100.0, 100.0, l, l + 1.3, 23.0);
        let list = build(view, &[b], None, 1.0);
        let tris = device_tris(&list, view, 1.0, ACCENT);
        assert!(!tris.is_empty(), "edge {l}: something is drawn");
        let (p0, p1) = (b[0], b[1]);
        let d = dashes_along(&tris, p0, p1, 0.75);
        assert_eq!(d.len(), 1, "edge {l} is one solid run: {d:?}");
        assert!(d[0].0 <= 0.0 + 1e-6 && d[0].1 >= l - 1e-6, "{d:?}");
    }
}

#[test]
fn ac64_flex_boundaries_12_16_and_20_22() {
    for (l, flex) in [
        (12.5, true),
        (14.0, true),
        (15.9, true),
        (20.5, true),
        (21.5, true),
        (12.0, false),
        (16.0, false),
        (20.0, false),
        (22.0, false),
    ] {
        let view = ViewTransform::identity();
        let b = box_px(100.0, 100.0, l, 60.0, 30.0);
        let list = build(view, &[b], None, 1.0);
        let tris = device_tris(&list, view, 1.0, ACCENT);
        let d = dashes_along(&tris, b[0], b[1], 0.75);
        check_pattern(l, &d, &format!("edge {l}")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(!exact_fit(l), flex, "my model of exact fits at {l}");
        // flexed dashes are about 2.7 to 4
        if flex {
            let clipped: Vec<f64> = d.iter().map(|x| x.1.min(l) - x.0.max(0.0)).collect();
            for len in clipped {
                assert!((2.5..=4.0 + 1e-6).contains(&len), "edge {l}: dash {len}");
            }
        }
    }
}

#[test]
fn ac64_dash_and_gap_lengths_are_screen_pixels_at_every_zoom() {
    // The same 137 x 61 px box at very different zoom levels: identical
    // patterns in pixels.
    let reference = {
        let view = ViewTransform::identity();
        let b = box_px(300.0, 300.0, 137.0, 61.0, 33.0);
        let list = build(view, &[b], None, 1.0);
        let tris = device_tris(&list, view, 1.0, ACCENT);
        (0..4)
            .map(|e| dashes_along(&tris, b[e], b[(e + 1) % 4], 0.75))
            .collect::<Vec<_>>()
    };
    for scale in [0.002, 0.05, 0.5, 3.7, 40.0, 1000.0] {
        let view = view_of(scale, 12.5, -7.0);
        let b = box_px(300.0, 300.0, 137.0, 61.0, 33.0);
        let list = build(view, &[b], None, 1.0);
        let tris = device_tris(&list, view, 1.0, ACCENT);
        for e in 0..4 {
            let d = dashes_along(&tris, b[e], b[(e + 1) % 4], 0.75);
            assert_eq!(d.len(), reference[e].len(), "scale {scale} edge {e}");
            for (x, y) in d.iter().zip(&reference[e]) {
                assert!(
                    (x.0 - y.0).abs() < 1e-5 && (x.1 - y.1).abs() < 1e-5,
                    "scale {scale}: {x:?} vs {y:?}"
                );
            }
        }
    }
}

#[test]
fn ac63_the_pattern_is_static_and_moves_rigidly_with_the_box() {
    // pan: the same box translated; zoom handled above; repeated builds equal
    let view = ViewTransform::identity();
    let b = box_px(200.0, 150.0, 211.0, 97.0, 12.0);
    let a1 = build(view, &[b], None, 1.0);
    let a2 = build(view, &[b], None, 1.0);
    assert_eq!(a1, a2, "a pure function of its inputs");
    let base: Vec<Vec<(f64, f64)>> = {
        let t = device_tris(&a1, view, 1.0, ACCENT);
        (0..4)
            .map(|e| dashes_along(&t, b[e], b[(e + 1) % 4], 0.75))
            .collect()
    };
    for (dx, dy) in [
        (0.3, 0.7),
        (101.1, -50.9),
        (-33.33, 22.22),
        (1e4, 1e4),
        (0.0001, 0.0),
    ] {
        let moved = box_px(200.0 + dx, 150.0 + dy, 211.0, 97.0, 12.0);
        let list = build(view, &[moved], None, 1.0);
        let t = device_tris(&list, view, 1.0, ACCENT);
        for e in 0..4 {
            let d = dashes_along(&t, moved[e], moved[(e + 1) % 4], 0.75);
            assert_eq!(d.len(), base[e].len(), "move ({dx},{dy}) edge {e}");
            for (x, y) in d.iter().zip(&base[e]) {
                assert!(
                    (x.0 - y.0).abs() < 1e-3 && (x.1 - y.1).abs() < 1e-3,
                    "move ({dx},{dy}): {x:?} vs {y:?}"
                );
            }
        }
    }
}

#[test]
fn ac63_the_dashes_are_one_pixel_wide_full_accent_for_a_rotated_box() {
    let view = ViewTransform::identity();
    let b = box_px(150.0, 150.0, 120.0, 80.0, 29.0);
    let list = build(view, &[b], None, 1.0);
    assert_eq!(all_colours(&list), vec![ACCENT], "only full --accent");
    let tris = device_tris(&list, view, 1.0, ACCENT);
    // perpendicular extent of the top edge's triangles is 1 px
    let (p0, p1) = (b[0], b[1]);
    let l = (p1.0 - p0.0).hypot(p1.1 - p0.1);
    let u = ((p1.0 - p0.0) / l, (p1.1 - p0.1) / l);
    let n = (-u.1, u.0);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for t in &tris {
        let across: Vec<f64> = t
            .iter()
            .map(|v| (v.0 - p0.0) * n.0 + (v.1 - p0.1) * n.1)
            .collect();
        if across.iter().all(|a| a.abs() <= 0.75) {
            for a in across {
                lo = lo.min(a);
                hi = hi.max(a);
            }
        }
    }
    assert!((hi - lo - 1.0).abs() < 1e-6, "width {}", hi - lo);
    assert!((hi + lo).abs() < 1e-6, "centred on the edge: {lo}..{hi}");
}

#[test]
fn ac63_each_selected_object_has_its_own_box() {
    let view = ViewTransform::identity();
    let a = box_px(100.0, 100.0, 80.0, 50.0, 0.0);
    let b = box_px(300.0, 100.0, 80.0, 50.0, 40.0);
    let one = build(view, &[a], None, 1.0);
    let two = build(view, &[a, b], None, 1.0);
    let b_only = build(view, &[b], None, 1.0);
    assert_eq!(
        two.triangles.len(),
        one.triangles.len() + b_only.triangles.len()
    );
}

// ---------------------------------------------------------------------
// Criterion 65: pixel aligned lines (axis-aligned boxes)
// ---------------------------------------------------------------------

/// The top, bottom, left and right edge triangles of an axis-aligned drawing
/// (each edge's triangles are thin in the perpendicular direction).
struct AxisEdges {
    top: Vec<Tri>,
    bottom: Vec<Tri>,
    left: Vec<Tri>,
    right: Vec<Tri>,
    lw: f64,
    bbox: (f64, f64, f64, f64),
}

fn axis_edges(tris: &[Tri]) -> AxisEdges {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for t in tris {
        for v in t {
            x0 = x0.min(v.0);
            y0 = y0.min(v.1);
            x1 = x1.max(v.0);
            y1 = y1.max(v.1);
        }
    }
    // the line width: the thinnest extent of the triangles at the top edge
    let mut lw: f64 = 0.0;
    for t in tris {
        let ymin = t.iter().map(|v| v.1).fold(f64::MAX, f64::min);
        let ymax = t.iter().map(|v| v.1).fold(f64::MIN, f64::max);
        if (ymin - y0).abs() < 1e-9 && ymax - ymin <= 3.5 && ymax - ymin > 1e-9 {
            lw = lw.max(ymax - ymin);
        }
    }
    let mut e = AxisEdges {
        top: vec![],
        bottom: vec![],
        left: vec![],
        right: vec![],
        lw,
        bbox: (x0, y0, x1, y1),
    };
    for t in tris {
        let (xmin, xmax) = (
            t.iter().map(|v| v.0).fold(f64::MAX, f64::min),
            t.iter().map(|v| v.0).fold(f64::MIN, f64::max),
        );
        let (ymin, ymax) = (
            t.iter().map(|v| v.1).fold(f64::MAX, f64::min),
            t.iter().map(|v| v.1).fold(f64::MIN, f64::max),
        );
        let thin_y = ymax - ymin <= lw + 1e-6;
        let thin_x = xmax - xmin <= lw + 1e-6;
        if thin_y && ymax <= y0 + lw + 1e-6 {
            e.top.push(*t);
        } else if thin_y && ymin >= y1 - lw - 1e-6 {
            e.bottom.push(*t);
        } else if thin_x && xmax <= x0 + lw + 1e-6 {
            e.left.push(*t);
        } else if thin_x && xmin >= x1 - lw - 1e-6 {
            e.right.push(*t);
        }
    }
    e
}

fn is_whole(v: f64) -> bool {
    (v - v.round()).abs() < 1e-6
}

fn pseudo(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 33) as f64) / f64::from(1u32 << 31)
}

#[test]
fn ac65_axis_aligned_boxes_cover_whole_device_pixel_rows_and_columns() {
    let mut seed = 7;
    let mut checked = 0;
    for dpr in [1.0, 1.25, 1.5, 2.0, 3.0] {
        for _ in 0..120 {
            let scale = 0.01 + pseudo(&mut seed) * 30.0;
            let (ox, oy) = (
                pseudo(&mut seed) * 500.0 - 250.0,
                pseudo(&mut seed) * 500.0 - 250.0,
            );
            let view = view_of(scale, ox, oy);
            let (cx, cy) = (
                100.0 + pseudo(&mut seed) * 600.0,
                100.0 + pseudo(&mut seed) * 400.0,
            );
            let (w, h) = (
                12.0 + pseudo(&mut seed) * 300.0,
                12.0 + pseudo(&mut seed) * 200.0,
            );
            let b = box_px(cx, cy, w, h, 0.0);
            let list = build(view, &[b], None, dpr);
            let tris = device_tris(&list, view, dpr, ACCENT);
            assert_ne!(tris.len(), 0);
            let e = axis_edges(&tris);
            assert!(
                e.lw >= 1.0 - 1e-6,
                "line width {} device px at dpr {dpr}",
                e.lw
            );
            assert!(
                is_whole(e.lw),
                "line width is a whole number of device px: {} (dpr {dpr})",
                e.lw
            );
            for t in e.top.iter().chain(&e.bottom) {
                for v in t {
                    assert!(
                        is_whole(v.1),
                        "dpr {dpr}: horizontal edge row at {} is not whole",
                        v.1
                    );
                }
            }
            for t in e.left.iter().chain(&e.right) {
                for v in t {
                    assert!(
                        is_whole(v.0),
                        "dpr {dpr}: vertical edge column at {} is not whole",
                        v.0
                    );
                }
            }
            // never more than a device pixel from the true edge
            let x0 = (cx - w / 2.0 - ox * 0.0) * dpr;
            let _ = x0;
            let (bx0, by0, bx1, by1) = e.bbox;
            let true_w = w * dpr;
            let true_h = h * dpr;
            let got_w = bx1 - bx0 - e.lw;
            let got_h = by1 - by0 - e.lw;
            assert!(
                (got_w - true_w).abs() <= 1.0 + 1e-6,
                "dpr {dpr}: snapped width {got_w} vs {true_w}"
            );
            assert!(
                (got_h - true_h).abs() <= 1.0 + 1e-6,
                "dpr {dpr}: snapped height {got_h} vs {true_h}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 600);
}

#[test]
fn ac65_line_width_is_one_css_pixel_at_dpr_1_and_2_and_full_accent() {
    for (dpr, want) in [(1.0, 1.0), (2.0, 2.0)] {
        let view = ViewTransform::identity();
        let list = build(view, &[box_px(100.5, 100.3, 80.0, 60.0, 0.0)], None, dpr);
        assert_eq!(
            all_colours(&list),
            vec![ACCENT],
            "full --accent at dpr {dpr}"
        );
        let tris = device_tris(&list, view, dpr, ACCENT);
        let e = axis_edges(&tris);
        assert!(
            (e.lw - want).abs() < 1e-6,
            "dpr {dpr}: line {} device px",
            e.lw
        );
    }
}

#[test]
fn ac65_pixel_pattern_of_a_snapped_box_is_still_fitted_and_closed() {
    // a snapped axis-aligned box: every edge still shows the dash rules
    let mut seed = 99;
    for dpr in [1.0, 2.0] {
        for _ in 0..80 {
            let view = ViewTransform::identity();
            let (cx, cy) = (
                200.0 + pseudo(&mut seed) * 50.0,
                200.0 + pseudo(&mut seed) * 50.0,
            );
            let (w, h) = (
                10.0 + pseudo(&mut seed) * 250.0,
                10.0 + pseudo(&mut seed) * 250.0,
            );
            let list = build(view, &[box_px(cx, cy, w, h, 0.0)], None, dpr);
            let tris = device_tris(&list, view, dpr, ACCENT);
            let e = axis_edges(&tris);
            let (x0, y0, x1, y1) = e.bbox;
            let half = e.lw / 2.0;
            let top = dashes_along(&e.top, (x0 + half, y0 + half), (x1 - half, y0 + half), e.lw);
            let bottom = dashes_along(
                &e.bottom,
                (x0 + half, y1 - half),
                (x1 - half, y1 - half),
                e.lw,
            );
            let left = dashes_along(
                &e.left,
                (x0 + half, y0 + half),
                (x0 + half, y1 - half),
                e.lw,
            );
            let right = dashes_along(
                &e.right,
                (x1 - half, y0 + half),
                (x1 - half, y1 - half),
                e.lw,
            );
            // in device pixels the nominal 4 / 3 is in screen pixels; at dpr 1
            // they agree. At dpr 2 the lengths scale.
            let k = dpr;
            for (name, d, l) in [
                ("top", top, x1 - x0 - e.lw),
                ("bottom", bottom, x1 - x0 - e.lw),
                ("left", left, y1 - y0 - e.lw),
                ("right", right, y1 - y0 - e.lw),
            ] {
                // scale back to screen px
                let ds: Vec<(f64, f64)> = d.iter().map(|x| (x.0 / k, x.1 / k)).collect();
                if let Err(m) = check_pattern(l / k, &ds, &format!("snapped dpr {dpr} {name}")) {
                    panic!("{m}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 66: hover box solid
// ---------------------------------------------------------------------

#[test]
fn ac66_hover_box_is_solid_hover_box_and_pixel_aligned() {
    for dpr in [1.0, 1.5, 2.0] {
        for deg in [0.0, 25.0] {
            let view = ViewTransform::identity();
            let b = box_px(210.3, 140.7, 133.0, 71.0, deg);
            let list = build(view, &[], Some(b), dpr);
            assert_eq!(
                all_colours(&list),
                vec![accent_hover()],
                "hover colour (dpr {dpr}, {deg} deg)"
            );
            let tris = device_tris(&list, view, dpr, accent_hover());
            if deg == 0.0 {
                let e = axis_edges(&tris);
                assert!(is_whole(e.lw) && e.lw >= 1.0);
                for t in e.top.iter().chain(&e.bottom) {
                    for v in t {
                        assert!(is_whole(v.1), "hover row {} not whole at dpr {dpr}", v.1);
                    }
                }
                for t in e.left.iter().chain(&e.right) {
                    for v in t {
                        assert!(is_whole(v.0), "hover column {} not whole at dpr {dpr}", v.0);
                    }
                }
                let (x0, y0, x1, _) = e.bbox;
                let half = e.lw / 2.0;
                let d = dashes_along(&e.top, (x0 + half, y0 + half), (x1 - half, y0 + half), e.lw);
                assert_eq!(d.len(), 1, "solid: one run along the top edge, got {d:?}");
            } else {
                for e in 0..4 {
                    let (p0, p1) = (b[e], b[(e + 1) % 4]);
                    let d = dashes_along(
                        &tris,
                        (p0.0 * dpr, p0.1 * dpr),
                        (p1.0 * dpr, p1.1 * dpr),
                        0.75 * dpr,
                    );
                    assert_eq!(d.len(), 1, "solid hover edge {e} (dpr {dpr}): {d:?}");
                }
            }
        }
    }
}

#[test]
fn ac66_selected_and_hovered_are_distinct_in_colour_and_dashing() {
    let view = ViewTransform::identity();
    let sel = box_px(100.0, 100.0, 90.0, 60.0, 0.0);
    let hov = box_px(300.0, 100.0, 90.0, 60.0, 0.0);
    let list = build(view, &[sel], Some(hov), 1.0);
    let cols = all_colours(&list);
    assert!(
        cols.contains(&ACCENT) && cols.contains(&accent_hover()),
        "{cols:?}"
    );
    let sel_tris = device_tris(&list, view, 1.0, ACCENT);
    let hov_tris = device_tris(&list, view, 1.0, accent_hover());
    assert!(
        sel_tris.len() > hov_tris.len(),
        "dashed selection has more pieces than the solid hover"
    );
}

// ---------------------------------------------------------------------
// Robustness: hostile inputs
// ---------------------------------------------------------------------

#[test]
fn hostile_device_pixel_ratios_read_as_one_or_a_sane_value() {
    let view = ViewTransform::identity();
    let b = box_px(100.0, 100.0, 80.0, 60.0, 0.0);
    let one = build(view, &[b], None, 1.0);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let list = build(view, &[b], None, bad);
        assert_eq!(list, one, "dpr {bad} reads as 1");
    }
    for big in [8.0, 1e6, 1e12] {
        let list = build(view, &[b], None, big);
        assert!(
            list.triangles.len() < 1_000_000,
            "dpr {big}: {} triangles",
            list.triangles.len()
        );
        assert!(
            list.triangles
                .iter()
                .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
        );
    }
}

#[test]
fn hostile_boxes_never_panic_and_stay_finite_and_bounded() {
    let view = ViewTransform::identity();
    let degenerate: Vec<[(f64, f64); 4]> = vec![
        [(5.0, 5.0); 4],
        [(0.0, 0.0), (0.0, 0.0), (10.0, 0.0), (10.0, 0.0)],
        [(0.0, 0.0), (100.0, 0.0), (100.0, 0.0), (0.0, 0.0)],
        box_px(0.0, 0.0, 1e-9, 1e-9, 12.0),
        box_px(0.0, 0.0, 49_999.0, 20.0, 0.0),
        box_px(0.0, 0.0, 50_001.0, 20.0, 0.0),
        box_px(0.0, 0.0, 1e7, 1e7, 33.0),
        box_px(0.0, 0.0, 1e12, 3.0, 0.0),
        box_px(1e15, -1e15, 100.0, 100.0, 45.0),
        // not a rectangle at all
        [(0.0, 0.0), (100.0, 10.0), (20.0, 90.0), (-30.0, 40.0)],
    ];
    for dpr in [1.0, 2.0] {
        for b in &degenerate {
            let start = std::time::Instant::now();
            let list = build(view, &[*b], Some(*b), dpr);
            assert!(start.elapsed().as_secs() < 5, "slow for {b:?}");
            assert!(
                list.triangles.len() < 2_000_000,
                "{} triangles for {b:?}",
                list.triangles.len()
            );
            assert!(list.triangles.len().is_multiple_of(3));
            assert!(
                list.triangles
                    .iter()
                    .all(|v| v.position.x.is_finite() && v.position.y.is_finite()),
                "non-finite vertex for {b:?}"
            );
        }
    }
}

#[test]
fn hostile_non_finite_corners_do_not_panic_or_hang() {
    let view = ViewTransform::identity();
    let id = some_id();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for corner in 0..4 {
            let mut q = [pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 50.0), pt(0.0, 50.0)];
            q[corner] = pt(bad, 10.0);
            let start = std::time::Instant::now();
            let list = build_select_draw_list(
                view,
                &SelectDecorationInput {
                    selected: vec![(id, q)],
                    hovered: Some((id, q)),
                    device_pixel_ratio: 1.0,
                    skew_guide: None,
                },
            );
            assert!(start.elapsed().as_secs() < 5);
            assert!(
                list.triangles.len() < 2_000_000,
                "{} triangles",
                list.triangles.len()
            );
        }
    }
    // non-finite or zero view scale
    for scale in [0.0, f64::NAN, f64::INFINITY, 1e-300, 1e300] {
        let v = ViewTransform::new(scale, pt(0.0, 0.0));
        let q = [pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 50.0), pt(0.0, 50.0)];
        let start = std::time::Instant::now();
        let list = build_select_draw_list(
            v,
            &SelectDecorationInput {
                selected: vec![(id, q)],
                hovered: None,
                device_pixel_ratio: 2.0,
                skew_guide: None,
            },
        );
        assert!(start.elapsed().as_secs() < 5, "scale {scale}");
        assert!(
            list.triangles.len() < 2_000_000,
            "scale {scale}: {} triangles",
            list.triangles.len()
        );
    }
}

#[test]
fn many_selected_boxes_scale_linearly_in_size() {
    let view = ViewTransform::identity();
    let boxes: Vec<[(f64, f64); 4]> = (0..500)
        .map(|i| {
            box_px(
                50.0 + f64::from(i) * 13.0,
                80.0 + f64::from(i % 17) * 31.0,
                60.0,
                40.0,
                f64::from(i) * 7.0,
            )
        })
        .collect();
    let list = build(view, &boxes, None, 1.0);
    let one = build(view, &boxes[..1], None, 1.0);
    assert!(
        list.triangles.len() < one.triangles.len() * 500 * 4,
        "{} triangles",
        list.triangles.len()
    );
}

#[test]
fn ac63_ac66_rotated_boxes_at_dpr_2_keep_screen_pixel_patterns_and_full_accent() {
    // rotated boxes are not snapped: dash lengths are screen pixels (CSS),
    // so in device pixels at dpr 2 they are twice as long
    for dpr in [1.5, 2.0, 3.0] {
        for l in [10.0, 13.0, 21.0, 37.0, 150.0, 301.7] {
            let view = ViewTransform::identity();
            let b = box_px(300.0, 300.0, l, 77.0, 31.0);
            let list = build(view, &[b], None, dpr);
            assert_eq!(all_colours(&list), vec![ACCENT], "full accent at dpr {dpr}");
            let tris = device_tris(&list, view, dpr, ACCENT);
            let (p0, p1) = (b[0], b[1]);
            let d = dashes_along(
                &tris,
                (p0.0 * dpr, p0.1 * dpr),
                (p1.0 * dpr, p1.1 * dpr),
                0.75 * dpr,
            );
            let ds: Vec<(f64, f64)> = d.iter().map(|x| (x.0 / dpr, x.1 / dpr)).collect();
            check_pattern(l, &ds, &format!("rotated dpr {dpr}")).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}

/// The top edge's pattern of a snapped box: (snapped pixel length, dash count, gap*1000).
fn snapped_top_pattern(view: ViewTransform, b: [(f64, f64); 4]) -> (i64, usize, i64) {
    let list = build(view, &[b], None, 1.0);
    let tris = device_tris(&list, view, 1.0, ACCENT);
    let e = axis_edges(&tris);
    let (x0, y0, x1, _) = e.bbox;
    let half = e.lw / 2.0;
    let d = dashes_along(&e.top, (x0 + half, y0 + half), (x1 - half, y0 + half), e.lw);
    let gap = if d.len() > 1 {
        ((d[1].0 - d[0].1) * 1000.0).round() as i64
    } else {
        0
    };
    (((x1 - x0) * 1000.0).round() as i64, d.len(), gap)
}

#[test]
fn ac63_ac65_a_snapped_box_keeps_its_pattern_rigid_and_refits_only_when_its_pixel_length_changes() {
    // Decision (coordinator, PR 2 review): criteria 63 and 65 pull apart for a
    // sub-pixel pan. A snapped box changes its pixel length by one pixel when
    // its true edges straddle a pixel boundary differently, and that re-fits
    // the dashes (e.g. gap 2.929 vs 2.857 px over 15 dashes). Accepted. What
    // must hold: a whole-pixel translation, and a zoom, of a box that keeps
    // its snapped size leave the pattern identical; the pattern is a function
    // of the snapped pixel length alone.
    let mut by_length: Vec<(i64, (usize, i64))> = Vec::new();
    for step in 0..40 {
        let dx = f64::from(step) * 0.05;
        let b = box_px(200.0 + dx, 150.0, 100.5, 60.0, 0.0);
        let (len, n, gap) = snapped_top_pattern(ViewTransform::identity(), b);
        match by_length.iter().find(|(l, _)| *l == len) {
            Some((_, seen)) => {
                assert_eq!(*seen, (n, gap), "same snapped length {len}, new pattern");
            }
            None => by_length.push((len, (n, gap))),
        }
    }
    assert!(
        by_length.len() <= 2,
        "a 100.5 px box snaps to at most two pixel lengths: {by_length:?}"
    );
    // Whole-pixel translation and zoom of the same snapped size: identical.
    let reference = snapped_top_pattern(
        ViewTransform::identity(),
        box_px(200.5, 150.5, 100.0, 60.0, 0.0),
    );
    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (37.0, -12.0), (-80.0, 400.0)] {
        for scale in [0.5, 1.0, 2.0, 3.37] {
            let view = view_of(scale, 3.0, -2.0);
            let b = box_px(200.5 + dx, 150.5 + dy, 100.0, 60.0, 0.0);
            assert_eq!(
                snapped_top_pattern(view, b),
                reference,
                "translated by ({dx}, {dy}) at scale {scale}"
            );
        }
    }
}
