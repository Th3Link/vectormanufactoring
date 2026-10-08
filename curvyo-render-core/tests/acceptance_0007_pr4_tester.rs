//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 4 (gradient
//! fills), render side: the ramp (criteria 16, 17, 18, 35), the gradient
//! coordinates of linear and radial fills (21, 22), the vertex ranges of the
//! draw list, mixed fill types, the 1024-gradient cap with its flat fallback,
//! and the degenerate stop lists. Written from `specification.md` and the doc
//! comments of the public items before the PR 4 bodies were read.
//!
//! GPU shader code is wasm32-only and not exercised here: only the pure part
//! (texels, coordinates, vertex ranges, attributes) is.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss, clippy::similar_names, clippy::doc_markdown)]
#![allow(
    missing_docs,
    clippy::assert_is_empty,
    clippy::semicolon_if_nothing_returned,
    clippy::needless_range_loop
)]

use curvyo_document_core::{
    AnchorId, Angle, Color, Document, FillMode, FillModeTarget, GradientStop, Length, NewAnchor,
    NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StopId, StopPosition, StyleEdit,
    ViewTransform, ramp_at, sorted_stops,
};
use curvyo_render_core::{
    DrawList, GradientFrame, MAX_GRADIENTS, RAMP_TEXELS, Ramp, build_artwork,
};

// ---------------------------------------------------------------- helpers

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn stop(n: u64, p: f64, c: Color, o: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(9, n),
        position: StopPosition::new(p).unwrap(),
        color: c,
        opacity: Opacity::new(o).unwrap(),
    }
}

fn view() -> ViewTransform {
    ViewTransform::new(1.0, pt(0.0, 0.0))
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    })
}

fn gradient(d: &Document, id: NodeId, mode: FillMode, stops: Vec<GradientStop>) {
    d.set_fill_mode(
        mode,
        &[FillModeTarget {
            id,
            seed_stops: stops,
        }],
    )
    .unwrap();
}

fn no_stroke(d: &Document, id: NodeId) {
    d.edit_style(&[id], &StyleEdit::StrokeEnabled(false))
        .unwrap();
}

fn objects(d: &Document) -> Vec<ObjectSnapshot> {
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn frame(w: f64, h: f64) -> GradientFrame {
    GradientFrame {
        min: pt(0.0, 0.0),
        max: pt(w, h),
        angle: Angle::from_radians(0.0),
        pivot: pt(w / 2.0, h / 2.0),
    }
}

fn texel_t(i: usize) -> f64 {
    i as f64 / (RAMP_TEXELS - 1) as f64
}

fn close(a: u8, b: f64, tol: f64) -> bool {
    (f64::from(a) - b).abs() <= tol
}

/// Tiny deterministic generator, so the property loops need no dependency.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64) / ((1_u64 << 53) as f64)
    }
}

// ------------------------------------------------------------ the ramp

#[test]
fn ac17_black_to_white_ramp_runs_linearly_in_encoded_values() {
    let ramp = Ramp::from_stops(&[
        stop(1, 0.0, rgb(0, 0, 0), 1.0),
        stop(2, 1.0, rgb(255, 255, 255), 1.0),
    ])
    .unwrap();
    assert_eq!(ramp.0.len(), RAMP_TEXELS);
    assert_eq!(ramp.0[0], [0, 0, 0, 255]);
    assert_eq!(ramp.0[RAMP_TEXELS - 1], [255, 255, 255, 255]);
    assert_eq!(ramp.first(), [0, 0, 0, 255]);
    for i in 0..RAMP_TEXELS {
        let want = texel_t(i) * 255.0;
        for ch in 0..3 {
            assert!(close(ramp.0[i][ch], want, 1.0), "texel {i} ch {ch}");
        }
        assert_eq!(ramp.0[i][3], 255);
    }
}

#[test]
fn golden_red_to_blue_midpoint_is_srgb_encoded_not_linear_light() {
    // sRGB-encoded halfway is 127.5 per channel; linear-light would give ~188.
    let ramp = Ramp::from_stops(&[
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ])
    .unwrap();
    let mid = ramp.0[127];
    let mid2 = ramp.0[128];
    for m in [mid, mid2] {
        assert!(close(m[0], 127.5, 2.0), "r {m:?}");
        assert!(close(m[2], 127.5, 2.0), "b {m:?}");
        assert_eq!(m[1], 0);
        assert_eq!(m[3], 255);
    }
}

#[test]
fn translucent_stops_interpolate_straight_alpha_not_premultiplied() {
    // Opaque red to fully transparent blue. Straight (non-premultiplied)
    // interpolation gives half red + half blue at half alpha; premultiplied
    // would keep the colour pure red.
    let ramp = Ramp::from_stops(&[
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 0.0),
    ])
    .unwrap();
    assert_eq!(ramp.0[0], [255, 0, 0, 255]);
    assert_eq!(ramp.0[RAMP_TEXELS - 1], [0, 0, 255, 0]);
    let m = ramp.0[128];
    assert!(close(m[0], 127.0, 3.0), "{m:?}");
    assert!(close(m[2], 128.0, 3.0), "{m:?}");
    assert!(close(m[3], 127.5, 3.0), "{m:?}");
}

#[test]
fn stop_opacity_maps_to_alpha_bytes() {
    for (o, want) in [
        (0.0, 0.0),
        (0.01, 2.55),
        (0.5, 127.5),
        (0.99, 252.45),
        (1.0, 255.0),
    ] {
        let ramp = Ramp::from_stops(&[stop(1, 0.5, rgb(1, 2, 3), o)]).unwrap();
        for texel in &ramp.0 {
            assert!(close(texel[3], want, 1.0), "o {o} got {}", texel[3]);
        }
    }
}

#[test]
fn pad_edge_rule_holds_the_end_stops_colour() {
    let ramp = Ramp::from_stops(&[
        stop(1, 0.3, rgb(255, 0, 0), 1.0),
        stop(2, 0.7, rgb(0, 0, 255), 1.0),
    ])
    .unwrap();
    for i in 0..RAMP_TEXELS {
        let t = texel_t(i);
        if t <= 0.3 {
            assert_eq!(ramp.0[i], [255, 0, 0, 255], "before the first, texel {i}");
        }
        if t >= 0.7 {
            assert_eq!(ramp.0[i], [0, 0, 255, 255], "after the last, texel {i}");
        }
    }
    // Between: strictly changing.
    assert!(ramp.0[128][0] > 0 && ramp.0[128][0] < 255);
}

#[test]
fn one_stop_is_a_uniform_ramp_and_no_stop_is_none() {
    let ramp = Ramp::from_stops(&[stop(1, 0.9, rgb(10, 20, 30), 0.4)]).unwrap();
    let first = ramp.0[0];
    assert!(ramp.0.iter().all(|t| *t == first));
    assert_eq!(&first[..3], &[10, 20, 30]);
    assert!(Ramp::from_stops(&[]).is_none());
}

#[test]
fn coincident_stops_make_a_hard_edge_with_the_list_order_deciding_the_sides() {
    let black = stop(1, 0.0, rgb(0, 0, 0), 1.0);
    let white = stop(4, 1.0, rgb(255, 255, 255), 1.0);
    let red = stop(2, 0.5, rgb(255, 0, 0), 1.0);
    let blue = stop(3, 0.5, rgb(0, 0, 255), 1.0);
    let a = Ramp::from_stops(&[black, red, blue, white]).unwrap();
    // Just left of 0.5 (texel 126: t = 0.494) the ramp is black-to-red and
    // almost red; just right (texel 129: t = 0.506) it is blue-to-white and
    // almost blue.
    assert!(a.0[126][0] > 240 && a.0[126][2] < 15, "{:?}", a.0[126]);
    assert!(a.0[129][2] > 240 && a.0[129][0] < 15, "{:?}", a.0[129]);
    // Swapping the two coincident stops in the list swaps the sides.
    let b = Ramp::from_stops(&[black, blue, red, white]).unwrap();
    assert!(b.0[126][2] > 240 && b.0[126][0] < 15, "{:?}", b.0[126]);
    assert!(b.0[129][0] > 240 && b.0[129][2] < 15, "{:?}", b.0[129]);
}

#[test]
fn stop_list_order_does_not_matter_when_positions_differ() {
    let s = [
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 0.25, rgb(0, 255, 0), 0.5),
        stop(3, 0.8, rgb(0, 0, 255), 1.0),
        stop(4, 1.0, rgb(9, 9, 9), 0.1),
    ];
    let forward = Ramp::from_stops(&s).unwrap();
    let mut rev = s;
    rev.reverse();
    assert_eq!(forward, Ramp::from_stops(&rev).unwrap());
    let mut shuffled = [s[2], s[0], s[3], s[1]];
    assert_eq!(forward, Ramp::from_stops(&shuffled).unwrap());
    shuffled.swap(0, 3);
    assert_eq!(forward, Ramp::from_stops(&shuffled).unwrap());
}

#[test]
fn two_stops_at_the_same_end_do_not_panic_and_pad_the_rest() {
    let at_zero = Ramp::from_stops(&[
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 0.0, rgb(0, 0, 255), 1.0),
    ])
    .unwrap();
    // Everything after the last stop is the last stop's colour.
    for t in &at_zero.0[1..] {
        assert_eq!(*t, [0, 0, 255, 255]);
    }
    let at_one = Ramp::from_stops(&[
        stop(1, 1.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ])
    .unwrap();
    for t in &at_one.0[..RAMP_TEXELS - 1] {
        assert_eq!(*t, [255, 0, 0, 255]);
    }
}

#[test]
fn many_stops_beyond_sixteen_still_make_a_ramp_in_position_order() {
    for n in [17_usize, 18, 64, 500] {
        let stops: Vec<_> = (0..n)
            .map(|k| {
                let c = if k % 2 == 0 { 0 } else { 255 };
                stop(k as u64 + 1, k as f64 / (n - 1) as f64, rgb(c, c, c), 1.0)
            })
            .collect();
        let ramp = Ramp::from_stops(&stops).unwrap();
        assert_eq!(ramp.0[0][0], 0);
        assert_eq!(
            ramp.0[RAMP_TEXELS - 1][0],
            if (n - 1) % 2 == 0 { 0 } else { 255 }
        );
    }
}

#[test]
fn ramp_texels_agree_with_the_shared_colour_at_t_function() {
    let mut rng = Lcg(7);
    for _ in 0..200 {
        let n = 1 + (rng.next() * 18.0) as usize;
        let stops: Vec<_> = (0..n)
            .map(|k| {
                let p = if rng.next() < 0.3 {
                    // Force coincident positions often.
                    (rng.next() * 4.0).floor() / 4.0
                } else {
                    rng.next()
                };
                stop(
                    k as u64 + 1,
                    p,
                    rgb(
                        (rng.next() * 255.0) as u8,
                        (rng.next() * 255.0) as u8,
                        (rng.next() * 255.0) as u8,
                    ),
                    rng.next(),
                )
            })
            .collect();
        let sorted = sorted_stops(&stops);
        let ramp = Ramp::from_stops(&stops).unwrap();
        for i in 0..RAMP_TEXELS {
            let (c, o) = ramp_at(&sorted, texel_t(i)).unwrap();
            let texel = ramp.0[i];
            assert!(close(texel[0], f64::from(c.r), 1.0), "r texel {i}");
            assert!(close(texel[1], f64::from(c.g), 1.0), "g texel {i}");
            assert!(close(texel[2], f64::from(c.b), 1.0), "b texel {i}");
            assert!(close(texel[3], o.get() * 255.0, 1.0), "a texel {i}");
        }
    }
}

#[test]
fn ramp_values_stay_inside_the_hull_of_the_stop_values() {
    let mut rng = Lcg(99);
    for _ in 0..300 {
        let n = 1 + (rng.next() * 17.0) as usize;
        let stops: Vec<_> = (0..n)
            .map(|k| {
                stop(
                    k as u64 + 1,
                    rng.next(),
                    rgb(
                        (rng.next() * 255.0) as u8,
                        (rng.next() * 255.0) as u8,
                        (rng.next() * 255.0) as u8,
                    ),
                    rng.next(),
                )
            })
            .collect();
        let lo = |f: fn(&GradientStop) -> f64| stops.iter().map(f).fold(f64::INFINITY, f64::min);
        let hi =
            |f: fn(&GradientStop) -> f64| stops.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        let ramp = Ramp::from_stops(&stops).unwrap();
        let chans: [fn(&GradientStop) -> f64; 4] = [
            |s| f64::from(s.color.r),
            |s| f64::from(s.color.g),
            |s| f64::from(s.color.b),
            |s| s.opacity.get() * 255.0,
        ];
        for t in ramp.0 {
            for (k, f) in chans.iter().enumerate() {
                let v = f64::from(t[k]);
                assert!(v >= lo(*f) - 1.0 && v <= hi(*f) + 1.0, "ch {k} {v}");
            }
        }
    }
}

#[test]
fn colour_at_t_pads_for_any_finite_t() {
    let sorted = sorted_stops(&[
        stop(1, 0.2, rgb(255, 0, 0), 1.0),
        stop(2, 0.8, rgb(0, 0, 255), 1.0),
    ]);
    for t in [-1.0, -0.0, 0.0, 1.0, 2.0, 1e300, -1e300] {
        let (c, _) = ramp_at(&sorted, t).unwrap();
        if t <= 0.2 {
            assert_eq!(c, rgb(255, 0, 0), "t {t}");
        }
        if t >= 0.8 {
            assert_eq!(c, rgb(0, 0, 255), "t {t}");
        }
    }
    assert!(ramp_at(&[], 0.5).is_none());
}

/// A shader clamps t, so infinity is not reachable from the GPU path. The
/// implementation maps every non-finite t to 0 (observed, not in the spec);
/// the pin here is only that the answer is one of the two end colours.
#[test]
fn colour_at_infinite_t_is_an_end_colour() {
    let sorted = sorted_stops(&[
        stop(1, 0.2, rgb(255, 0, 0), 1.0),
        stop(2, 0.8, rgb(0, 0, 255), 1.0),
    ]);
    for t in [f64::NEG_INFINITY, f64::INFINITY] {
        let c = ramp_at(&sorted, t).unwrap().0;
        assert!(c == rgb(255, 0, 0) || c == rgb(0, 0, 255));
    }
}

#[test]
fn colour_at_nan_t_does_not_panic() {
    let sorted = sorted_stops(&[
        stop(1, 0.2, rgb(255, 0, 0), 1.0),
        stop(2, 0.8, rgb(0, 0, 255), 1.0),
    ]);
    let _ = ramp_at(&sorted, f64::NAN);
}

#[test]
fn sorted_stops_is_a_stable_idempotent_permutation() {
    let s = [
        stop(1, 0.5, rgb(1, 0, 0), 1.0),
        stop(2, 0.0, rgb(2, 0, 0), 1.0),
        stop(3, 0.5, rgb(3, 0, 0), 1.0),
        stop(4, 0.5, rgb(4, 0, 0), 1.0),
        stop(5, 0.25, rgb(5, 0, 0), 1.0),
    ];
    let sorted = sorted_stops(&s);
    let ids: Vec<StopId> = sorted.iter().map(|x| x.id).collect();
    let want: Vec<StopId> = [2, 5, 1, 3, 4].iter().map(|n| StopId::new(9, *n)).collect();
    assert_eq!(ids, want, "ties keep list order");
    assert_eq!(sorted_stops(&sorted), sorted);
    assert!(sorted_stops(&[]).is_empty());
}

// ------------------------------------------------- coordinates (21, 22)

#[test]
fn ac21_linear_runs_left_edge_to_right_edge_and_is_constant_across() {
    let f = frame(20.0, 10.0);
    let c = |x, y| f.linear(pt(x, y))[0];
    assert!((c(0.0, 0.0) - 0.0).abs() < 1e-6);
    assert!((c(20.0, 10.0) - 1.0).abs() < 1e-6);
    assert!((c(10.0, 5.0) - 0.5).abs() < 1e-6);
    assert!(
        (c(10.0, 0.0) - c(10.0, 10.0)).abs() < 1e-6,
        "no y component"
    );
    // Beyond the box the coordinate runs on (the shader pads).
    assert!(c(-10.0, 5.0) < 0.0 && c(30.0, 5.0) > 1.0);
}

#[test]
fn ac22_radial_is_zero_at_the_centre_and_one_on_the_inscribed_ellipse() {
    let f = frame(20.0, 10.0);
    let len = |x, y| {
        let [a, b] = f.radial(pt(x, y));
        f64::from(a).hypot(f64::from(b))
    };
    assert!(len(10.0, 5.0) < 1e-6);
    assert!((len(20.0, 5.0) - 1.0).abs() < 1e-6, "right edge");
    assert!(
        (len(10.0, 10.0) - 1.0).abs() < 1e-6,
        "bottom edge: elliptical"
    );
    assert!((len(0.0, 5.0) - 1.0).abs() < 1e-6);
    assert!((len(20.0, 10.0) - 2.0_f64.sqrt()).abs() < 1e-5, "corner");
}

#[test]
fn rotated_frame_turns_the_ramp_with_the_object() {
    // Local box 0..20 x 0..10, turned a quarter turn about its centre.
    let mut f = frame(20.0, 10.0);
    f.angle = Angle::from_radians(std::f64::consts::FRAC_PI_2);
    f.pivot = pt(10.0, 5.0);
    // Local points map to document points through the same turn.
    for (lx, ly) in [
        (0.0, 0.0),
        (20.0, 0.0),
        (10.0, 5.0),
        (5.0, 10.0),
        (15.0, 3.0),
    ] {
        let doc = pt(lx, ly).rotated_around(f.pivot, f.angle);
        let got = f.linear(doc)[0];
        assert!(
            (f64::from(got) - lx / 20.0).abs() < 1e-5,
            "({lx},{ly}) -> {got}"
        );
        let [rx, ry] = f.radial(doc);
        assert!((f64::from(rx) - (lx - 10.0) / 10.0).abs() < 1e-5);
        assert!((f64::from(ry) - (ly - 5.0) / 5.0).abs() < 1e-5);
    }
}

#[test]
fn zero_size_and_inverted_boxes_give_finite_coordinates() {
    let boxes = [
        frame(0.0, 0.0),
        frame(0.0, 10.0),
        frame(20.0, 0.0),
        GradientFrame {
            min: pt(5.0, 5.0),
            max: pt(5.0, 5.0),
            angle: Angle::from_radians(1.0),
            pivot: pt(5.0, 5.0),
        },
        GradientFrame {
            min: pt(10.0, 10.0),
            max: pt(0.0, 0.0),
            angle: Angle::from_radians(0.0),
            pivot: pt(0.0, 0.0),
        },
    ];
    for f in boxes {
        for p in [pt(0.0, 0.0), pt(5.0, 5.0), pt(100.0, -100.0), pt(1e6, 1e6)] {
            for v in f.linear(p).into_iter().chain(f.radial(p)) {
                assert!(v.is_finite(), "{f:?} at {p:?} gave {v}");
            }
        }
    }
}

// --------------------------------------------- draw list: ranges, kinds

fn fills_of(list: &DrawList) -> &[curvyo_render_core::GradientFill] {
    &list.gradients
}

#[test]
fn a_gradient_fill_records_a_vertex_range_flat_coloured_with_the_first_texel() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 20.0, 10.0);
    gradient(
        &d,
        r,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(200, 10, 20), 0.5),
            stop(2, 1.0, rgb(0, 0, 255), 1.0),
        ],
    );
    let list = build_artwork(&objects(&d), &[Some(frame(20.0, 10.0))], view());
    let fills = fills_of(&list);
    assert_eq!(fills.len(), 1);
    let g = &fills[0];
    assert!(!g.radial);
    assert!(g.start < g.end && g.end <= list.triangles.len());
    assert_eq!((g.end - g.start) % 3, 0, "whole triangles");
    let first = g.ramp.first();
    for v in &list.triangles[g.start..g.end] {
        assert_eq!([v.color.r, v.color.g, v.color.b, v.color.a], first);
        let p = v.position;
        assert!(p.x >= -1e-6 && p.x <= 20.0 + 1e-6 && p.y >= -1e-6 && p.y <= 10.0 + 1e-6);
    }
    // The stroke (default on) is another layer outside the range.
    assert!(g.end < list.triangles.len(), "the stroke follows the fill");
    assert_eq!(first[3], 128, "half opacity");
}

#[test]
fn gradient_attributes_carry_the_coordinate_row_and_mode_per_vertex() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 20.0, 10.0);
    let b = rect(&d, 100.0, 0.0, 40.0, 40.0);
    no_stroke(&d, a);
    no_stroke(&d, b);
    let s = vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ];
    gradient(&d, a, FillMode::Linear, s.clone());
    gradient(&d, b, FillMode::Radial, s);
    let list = build_artwork(
        &objects(&d),
        &[Some(frame(20.0, 10.0)), {
            let mut f = frame(40.0, 40.0);
            f.min = pt(100.0, 0.0);
            f.max = pt(140.0, 40.0);
            f.pivot = pt(120.0, 20.0);
            Some(f)
        }],
        view(),
    );
    assert_eq!(list.gradients.len(), 2);
    assert!(!list.gradients[0].radial && list.gradients[1].radial);
    assert!(
        list.gradients[0].end <= list.gradients[1].start,
        "ranges are disjoint and in order"
    );
    let rows = 4;
    let attrs = list.gradient_attributes(rows);
    assert_eq!(attrs.len(), list.triangles.len());
    for (i, g) in list.gradients.iter().enumerate() {
        let want_v = (i as f32 + 0.5) / rows as f32;
        let want_mode = if g.radial { 2.0 } else { 1.0 };
        for k in g.start..g.end {
            let [x, y, v, mode] = attrs[k];
            assert!((v - want_v).abs() < 1e-6);
            assert_eq!(mode, want_mode);
            let [ex, ey] = g.coordinate(list.triangles[k].position);
            assert!((x - ex).abs() < 1e-6 && (y - ey).abs() < 1e-6);
        }
    }
    // Linear: x spans 0..1 over the first rect; radial: length <= sqrt 2.
    let (lo, hi) = (list.gradients[0].start..list.gradients[0].end)
        .map(|k| attrs[k][0])
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(x), h.max(x)));
    assert!(lo.abs() < 1e-5 && (hi - 1.0).abs() < 1e-5);
    for k in list.gradients[1].start..list.gradients[1].end {
        let len = attrs[k][0].hypot(attrs[k][1]);
        assert!(len <= 2.0_f32.sqrt() + 1e-4);
    }
    // Everything outside the ranges is flat (mode 0).
    for (k, a) in attrs.iter().enumerate() {
        let inside = list.gradients.iter().any(|g| (g.start..g.end).contains(&k));
        if !inside {
            assert_eq!(a[3], 0.0, "vertex {k} is not a gradient vertex");
        }
    }
}

#[test]
fn mixed_fill_types_keep_tree_order_and_only_gradients_get_ranges() {
    let d = Document::new(1);
    let solid = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let lin = rect(&d, 20.0, 0.0, 10.0, 10.0);
    let none = rect(&d, 40.0, 0.0, 10.0, 10.0);
    let rad = rect(&d, 60.0, 0.0, 10.0, 10.0);
    let open_path = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(80.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(90.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(90.0, 10.0)),
        ],
        false,
    );
    for id in [solid, lin, none, rad, open_path] {
        no_stroke(&d, id);
    }
    d.edit_style(&[solid], &StyleEdit::FillColor(rgb(1, 2, 3)))
        .unwrap();
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id: solid,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let s = vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 1.0),
    ];
    gradient(&d, lin, FillMode::Linear, s.clone());
    gradient(&d, rad, FillMode::Radial, s.clone());
    gradient(&d, open_path, FillMode::Linear, s);
    let frames: Vec<_> = (0..5_u32)
        .map(|i| {
            let mut f = frame(10.0, 10.0);
            f.min = pt(f64::from(i) * 20.0, 0.0);
            f.max = pt(f64::from(i) * 20.0 + 10.0, 10.0);
            f.pivot = pt(f64::from(i) * 20.0 + 5.0, 5.0);
            Some(f)
        })
        .collect();
    let list = build_artwork(&objects(&d), &frames, view());
    assert_eq!(list.gradients.len(), 3, "linear, radial, open linear");
    assert_eq!(
        list.gradients.iter().map(|g| g.radial).collect::<Vec<_>>(),
        vec![false, true, false]
    );
    // Ranges ascend, never overlap, and the solid / none objects' vertices are
    // outside every range.
    for w in list.gradients.windows(2) {
        assert!(w[0].end <= w[1].start);
    }
    let solid_vertices = list
        .triangles
        .iter()
        .enumerate()
        .filter(|(_, v)| (v.color.r, v.color.g, v.color.b) == (1, 2, 3))
        .map(|(k, _)| k)
        .collect::<Vec<_>>();
    assert!(!solid_vertices.is_empty());
    for k in solid_vertices {
        assert!(!list.gradients.iter().any(|g| (g.start..g.end).contains(&k)));
    }
    // The open path's fill is closed by a chord: a triangle (3 vertices at least).
    let g = &list.gradients[2];
    assert!(g.end - g.start >= 3);
}

#[test]
fn zero_stops_paint_nothing_one_stop_paints_uniformly() {
    // A 0-stop gradient can only come from a merge or a hand-made file; the
    // stored list is built through the fill-mode switch with an empty seed.
    let d = Document::new(1);
    let none = rect(&d, 0.0, 0.0, 10.0, 10.0);
    no_stroke(&d, none);
    d.set_fill_mode(
        FillMode::Linear,
        &[FillModeTarget {
            id: none,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let list = build_artwork(&objects(&d), &[Some(frame(10.0, 10.0))], view());
    assert_eq!(list.triangles.len(), 0, "no stops, nothing painted");
    assert!(list.gradients.is_empty());

    let d = Document::new(1);
    let one = rect(&d, 0.0, 0.0, 10.0, 10.0);
    no_stroke(&d, one);
    gradient(
        &d,
        one,
        FillMode::Radial,
        vec![stop(1, 0.3, rgb(7, 8, 9), 0.2)],
    );
    let list = build_artwork(&objects(&d), &[Some(frame(10.0, 10.0))], view());
    assert!(!list.triangles.is_empty(), "one stop paints");
    for v in &list.triangles {
        assert_eq!((v.color.r, v.color.g, v.color.b), (7, 8, 9));
        assert_eq!(v.color.a, 51);
    }
    // Whether recorded as a gradient or painted flat, the ramp is uniform.
    for g in &list.gradients {
        assert!(g.ramp.0.iter().all(|t| *t == g.ramp.0[0]));
    }
}

#[test]
fn gradient_without_a_frame_paints_flat_in_the_first_stops_colour() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    no_stroke(&d, r);
    gradient(
        &d,
        r,
        FillMode::Linear,
        vec![
            stop(1, 0.6, rgb(10, 20, 30), 1.0),
            stop(2, 0.2, rgb(200, 200, 200), 1.0),
        ],
    );
    for frames in [vec![], vec![None]] {
        let list = build_artwork(&objects(&d), &frames, view());
        assert!(!list.triangles.is_empty());
        // "First stop" is the first in position order (the ramp's t = 0).
        for v in &list.triangles {
            assert_eq!((v.color.r, v.color.g, v.color.b), (200, 200, 200));
        }
    }
}

#[test]
fn more_than_1024_gradients_stay_flat_beyond_the_cap() {
    let d = Document::new(1);
    let total = MAX_GRADIENTS + 76;
    let s = vec![
        stop(1, 0.0, rgb(250, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 250), 1.0),
    ];
    for k in 0..total {
        let r = rect(&d, k as f64 * 2.0, 0.0, 1.0, 1.0);
        no_stroke(&d, r);
        gradient(&d, r, FillMode::Linear, s.clone());
    }
    let frames: Vec<_> = (0..total)
        .map(|k| {
            let mut f = frame(1.0, 1.0);
            f.min = pt(k as f64 * 2.0, 0.0);
            f.max = pt(k as f64 * 2.0 + 1.0, 1.0);
            f.pivot = pt(k as f64 * 2.0 + 0.5, 0.5);
            Some(f)
        })
        .collect();
    let list = build_artwork(&objects(&d), &frames, view());
    assert_eq!(list.painted_gradients().len(), MAX_GRADIENTS);
    assert!(list.gradients.len() >= MAX_GRADIENTS);
    let attrs = list.gradient_attributes(MAX_GRADIENTS);
    assert_eq!(attrs.len(), list.triangles.len());
    // Rows are distinct, in order, inside (0, 1).
    let mut last_v = 0.0;
    for g in list.painted_gradients() {
        let v = attrs[g.start][2];
        assert!(v > last_v && v < 1.0);
        last_v = v;
        assert_eq!(attrs[g.start][3], 1.0);
    }
    // Beyond the cap: flat vertices carrying the first colour, no mode.
    for g in &list.gradients[MAX_GRADIENTS..] {
        for k in g.start..g.end {
            assert_eq!(attrs[k][3], 0.0, "vertex {k} beyond the cap is flat");
            let v = list.triangles[k].color;
            assert_eq!([v.r, v.g, v.b, v.a], g.ramp.first());
        }
    }
}

#[test]
fn extending_a_list_moves_the_gradient_ranges_with_the_vertices() {
    let make = |x: f64| {
        let d = Document::new(1);
        let r = rect(&d, x, 0.0, 10.0, 10.0);
        no_stroke(&d, r);
        gradient(
            &d,
            r,
            FillMode::Linear,
            vec![
                stop(1, 0.0, rgb(255, 0, 0), 1.0),
                stop(2, 1.0, rgb(0, 0, 255), 1.0),
            ],
        );
        let mut f = frame(10.0, 10.0);
        f.min = pt(x, 0.0);
        f.max = pt(x + 10.0, 10.0);
        f.pivot = pt(x + 5.0, 5.0);
        build_artwork(&objects(&d), &[Some(f)], view())
    };
    let mut a = make(0.0);
    let b = make(100.0);
    let a_len = a.triangles.len();
    a.extend(b);
    assert_eq!(a.gradients.len(), 2);
    let second = &a.gradients[1];
    assert!(second.start >= a_len, "{} vs {a_len}", second.start);
    for v in &a.triangles[second.start..second.end] {
        assert!(
            v.position.x >= 100.0 - 1e-6,
            "range points at the right vertices"
        );
    }
    let attrs = a.gradient_attributes(2);
    let [x, _, _, _] = attrs[second.start];
    assert!((-1e-4..=1.0 + 1e-4).contains(&x));
}

#[test]
fn draw_lists_with_gradients_survive_a_zoomed_view() {
    // The vertices are in view space; the gradient coordinate is derived from
    // them, so the frame must be in the same space or the ramp would slide.
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 20.0, 10.0);
    no_stroke(&d, r);
    gradient(
        &d,
        r,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(255, 0, 0), 1.0),
            stop(2, 1.0, rgb(0, 0, 255), 1.0),
        ],
    );
    let list = build_artwork(
        &objects(&d),
        &[Some(frame(20.0, 10.0))],
        ViewTransform::new(1.0, pt(0.0, 0.0)),
    );
    let attrs = list.gradient_attributes(1);
    let g = &list.gradients[0];
    let xs: Vec<f32> = (g.start..g.end).map(|k| attrs[k][0]).collect();
    assert!(xs.iter().all(|x| x.is_finite()));
}
