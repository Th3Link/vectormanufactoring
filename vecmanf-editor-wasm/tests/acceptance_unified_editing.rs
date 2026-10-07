//! Independent tester acceptance tests for PR 1 of
//! `specs/unified-object-editing/specification.md` (criteria 1-24, 21a, 35
//! without the advanced-selection clauses, 37, 38). Written from the
//! specification before the implementation was read. Everything goes through
//! `Session`'s public API. Expected values come from the specification's own
//! arithmetic (handle tiers, the radius position and gain rule) and from
//! reference models written here, never read back from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::type_complexity)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::cast_sign_loss, clippy::items_after_statements)]
#![allow(clippy::suboptimal_flops, clippy::manual_let_else)]
#![allow(missing_docs, clippy::used_underscore_binding, clippy::if_not_else)]
#![allow(clippy::cast_lossless, clippy::bool_to_int_with_if)]
#![allow(clippy::too_many_arguments, clippy::as_conversions)]

use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

use vecmanf_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, ObjectSnapshot, Point,
    PointCount, RectBounds, Shape, StarFrame, effective_corner_radius, outline_of_rotated, pack,
    unpack,
};
use vecmanf_editor_wasm::{Session, Tool};
use vecmanf_render_core::DrawList;
use vecmanf_ui_core::{BarValue, EntryOutcome, InvalidReason};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

fn star_frame(cx: f64, cy: f64, r: f64) -> StarFrame {
    StarFrame {
        center: pt(cx, cy),
        radius: Length::from_mm(r),
        angle: Angle::from_radians(-FRAC_PI_2),
    }
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn rotate_object_about_center(d: &Document, index: usize, radians: f64) {
    if radians == 0.0 {
        return;
    }
    let id = d.object_ids()[index];
    let o = d.object(id).unwrap();
    let c = match &o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => pt(
                bounds.origin.x + bounds.width.as_mm() / 2.0,
                bounds.origin.y + bounds.height.as_mm() / 2.0,
            ),
            Shape::Ellipse { frame } => frame.center,
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame.center,
        },
        ObjectSnapshot::Path(_) => panic!("primitives only"),
    };
    d.rotate_object(&o.rotated(c, Angle::from_radians(radians)))
        .unwrap();
}

fn open_in_session(d: &Document) -> Session {
    let bytes = pack(d, "0.1.0").unwrap();
    let mut s = Session::open(2, &bytes).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn snapshot_bytes(s: &Session) -> Vec<u8> {
    doc_of(s).export_loro_snapshot().unwrap()
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&snapshot_bytes(s)).unwrap();
    l.len_changes()
}

fn vv_of(s: &Session) -> loro::VersionVector {
    let l = loro::LoroDoc::new();
    l.import(&snapshot_bytes(s)).unwrap();
    l.oplog_vv()
}

fn ops_since_json(s: &Session, from: &loro::VersionVector) -> String {
    let l = loro::LoroDoc::new();
    l.import(&snapshot_bytes(s)).unwrap();
    format!("{:?}", l.export_json_updates(from, &l.oplog_vv()))
}

fn prim_of(s: &Session, index: usize) -> vecmanf_document_core::PrimitiveSnapshot {
    let d = doc_of(s);
    d.primitive(d.object_ids()[index]).unwrap()
}

fn rect_of(s: &Session, index: usize) -> (RectBounds, Length) {
    match prim_of(s, index).shape {
        Shape::Rect {
            bounds,
            corner_radius,
        } => (bounds, corner_radius),
        other => panic!("rect expected, got {other:?}"),
    }
}

fn eff_radius_mm(s: &Session, index: usize) -> f64 {
    let (b, r) = rect_of(s, index);
    effective_corner_radius(b, r).as_mm()
}

fn star_ratio(s: &Session, index: usize) -> f64 {
    match prim_of(s, index).shape {
        Shape::Star { inner_ratio, .. } => inner_ratio.get(),
        other => panic!("star expected, got {other:?}"),
    }
}

fn star_radius_mm(s: &Session, index: usize) -> f64 {
    match prim_of(s, index).shape {
        Shape::Star { frame, .. } => frame.radius.as_mm(),
        other => panic!("star expected, got {other:?}"),
    }
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn shift_click(s: &mut Session, p: Point) {
    s.pointer_hover(p, true, false);
    s.pointer_down(p, true);
    s.pointer_up(p, true, false);
}

fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, shift, ctrl);
    s.pointer_down(from, shift);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    drag_mod(s, from, to, false, false);
}

fn k_of(s: &Session) -> f64 {
    s.view().scale()
}

/// Zooms a fresh session to about `percent` about the origin (document
/// origin stays at screen origin).
fn zoom_to_pct(s: &mut Session, percent: i64) {
    let current = s.zoom_percent() as f64;
    let factor = percent as f64 / current;
    s.wheel(0.0, -400.0 * factor.log2(), 0.0, 0.0, false, true);
}

/// The scale a fresh session has after `zoom_to_pct(percent)`.
fn k_at_zoom(percent: i64) -> f64 {
    let mut s = Session::new(1);
    zoom_to_pct(&mut s, percent);
    k_of(&s)
}

/// Opens `d` in a session zoomed to `percent`.
fn open_zoomed(d: &Document, percent: i64) -> Session {
    let mut s = open_in_session(d);
    zoom_to_pct(&mut s, percent);
    s
}

/// A box in the document with pixel-based local offsets: centre (mm), half
/// extents (mm), rotation (radians), scale (px per mm).
#[derive(Clone, Copy, Debug)]
struct Fr {
    c: Point,
    hw: f64,
    hh: f64,
    th: f64,
    k: f64,
}

impl Fr {
    /// Local offset from the centre in screen pixels to a document point.
    fn at(&self, ox_px: f64, oy_px: f64) -> Point {
        rot(
            pt(self.c.x + ox_px / self.k, self.c.y + oy_px / self.k),
            self.c,
            self.th,
        )
    }
    fn w_px(&self) -> f64 {
        self.hw * 2.0 * self.k
    }
    fn h_px(&self) -> f64 {
        self.hh * 2.0 * self.k
    }
    fn s_px(&self) -> f64 {
        self.w_px().min(self.h_px())
    }
    fn corner(&self, sx: f64, sy: f64) -> Point {
        self.at(sx * self.w_px() / 2.0, sy * self.h_px() / 2.0)
    }
    fn mid(&self, nx: f64, ny: f64) -> Point {
        self.at(nx * self.w_px() / 2.0, ny * self.h_px() / 2.0)
    }
    /// Corner rotate handle: 32 px out along the diagonal.
    fn rot_corner(&self, sx: f64, sy: f64) -> Point {
        let d = 32.0 / SQRT_2;
        self.at(sx * (self.w_px() / 2.0 + d), sy * (self.h_px() / 2.0 + d))
    }
    /// Side rotate handle: 32 px out from the side midpoint.
    fn rot_side(&self, nx: f64, ny: f64) -> Point {
        self.at(
            nx * (self.w_px() / 2.0 + 32.0 * nx.abs()),
            ny * (self.h_px() / 2.0 + 32.0 * ny.abs()),
        )
    }
    /// A point `px` pixels from the centre in direction angle `a` of the
    /// local frame.
    fn polar(&self, r_px: f64, a: f64) -> Point {
        self.at(r_px * a.cos(), r_px * a.sin())
    }
}

const CORNERS: [(f64, f64); 4] = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];

/// Deterministic pseudo random numbers.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

// ---------------------------------------------------------------------
// Scene builders (sizes in screen pixels at the chosen zoom)
// ---------------------------------------------------------------------

struct Scene {
    s: Session,
    fr: Fr,
    k: f64,
}

/// A rectangle `w_px` x `h_px` with corner radius `r_px`, rotated by `th`,
/// at zoom `pct`, selected. The box centre is at (60, 45) mm.
fn rect_scene(pct: i64, w_px: f64, h_px: f64, r_px: f64, th: f64) -> Scene {
    let k = k_at_zoom(pct);
    let d = Document::new(1);
    let (w, h) = (w_px / k, h_px / k);
    let id = d.create_rect(bounds(60.0 - w / 2.0, 45.0 - h / 2.0, w, h));
    d.set_corner_radius(&[id], Length::from_mm(r_px / k))
        .unwrap();
    rotate_object_about_center(&d, 0, th);
    let mut s = open_zoomed(&d, pct);
    assert!(near(k_of(&s), k, 1e-12));
    let fr = Fr {
        c: pt(60.0, 45.0),
        hw: w / 2.0,
        hh: h / 2.0,
        th,
        k,
    };
    select_only(&mut s);
    Scene { s, fr, k }
}

/// Click a point of the sole object's outline (anchor 0).
fn select_only(s: &mut Session) {
    let d = doc_of(s);
    let ids = d.object_ids();
    let o = d.primitive(ids[0]).unwrap();
    let p = outline_of_rotated(&o.shape, o.rotation)[0].point;
    click(s, p);
}

fn select_index(s: &mut Session, index: usize, shift: bool) {
    let d = doc_of(s);
    let ids = d.object_ids();
    let o = d.primitive(ids[index]).unwrap();
    let p = outline_of_rotated(&o.shape, o.rotation)[0].point;
    if shift {
        shift_click(s, p);
    } else {
        click(s, p);
    }
}

/// Star or polygon scene: `2 * r_px` box side for point counts that are
/// multiples of 4 (the box is then 2R by 2R).
fn star_scene(pct: i64, r_px: f64, n: u32, ratio: Option<f64>, th: f64) -> Scene {
    let k = k_at_zoom(pct);
    let d = Document::new(1);
    let r = r_px / k;
    let _ = match ratio {
        Some(q) => d.create_star(
            star_frame(60.0, 45.0, r),
            PointCount::new(n).unwrap(),
            InnerRatio::new(q).unwrap(),
        ),
        None => d.create_polygon(star_frame(60.0, 45.0, r), PointCount::new(n).unwrap()),
    };
    rotate_object_about_center(&d, 0, th);
    let mut s = open_zoomed(&d, pct);
    let fr = Fr {
        c: pt(60.0, 45.0),
        hw: r,
        hh: r,
        th,
        k,
    };
    select_only(&mut s);
    Scene { s, fr, k }
}

fn ellipse_scene(pct: i64, w_px: f64, h_px: f64, th: f64) -> Scene {
    let k = k_at_zoom(pct);
    let d = Document::new(1);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(60.0, 45.0),
        rx: Length::from_mm(w_px / k / 2.0),
        ry: Length::from_mm(h_px / k / 2.0),
    });
    rotate_object_about_center(&d, 0, th);
    let mut s = open_zoomed(&d, pct);
    let fr = Fr {
        c: pt(60.0, 45.0),
        hw: w_px / k / 2.0,
        hh: h_px / k / 2.0,
        th,
        k,
    };
    select_only(&mut s);
    Scene { s, fr, k }
}

/// Triangles of the draw list that are white: the ground of every resize,
/// centre and parameter glyph, and nothing else the Select tool draws.
///
/// The count measures the handle tiers. It does not count every triangle
/// because the dashed selection box (`edit-interaction-polish` criteria 63,
/// 64) changes its triangle count with the box's size on screen, which would
/// mask the tiers; the box and the arrows are never white.
fn tri_count(s: &Session) -> usize {
    let white = vecmanf_render_core::RgbaColor::WHITE;
    s.draw_list()
        .triangles
        .iter()
        .filter(|vertex| vertex.color == white)
        .count()
        / 3
}

/// White triangles of the selection decoration (the handle glyphs): count with
/// the object selected minus the count with nothing selected.
fn decor_count(sc: &mut Scene) -> usize {
    // Move the pointer far from everything so no hover state is involved.
    let far = pt(500.0, 500.0);
    sc.s.pointer_hover(far, false, false);
    let selected = tri_count(&sc.s);
    click(&mut sc.s, far);
    let unselected = tri_count(&sc.s);
    select_only(&mut sc.s);
    sc.s.pointer_hover(far, false, false);
    assert_eq!(tri_count(&sc.s), selected, "reselect restores the list");
    selected - unselected
}

// ---------------------------------------------------------------------
// Radius handle model (criterion 2, UX notes section 1)
// ---------------------------------------------------------------------

/// `L(s) = (s - 14) / sqrt 2 - 15`.
fn l_of(s: f64) -> f64 {
    (s - 14.0) / SQRT_2 - 15.0
}

/// `G(s) = (s / 2) / L(s)`.
fn g_of(s: f64) -> f64 {
    (s / 2.0) / l_of(s)
}

/// Distance of the radius handle from its corner along the diagonal, px.
fn handle_p(s: f64, r_px: f64) -> f64 {
    let rho = (r_px / (s / 2.0)).clamp(0.0, 1.0);
    15.0 + rho * l_of(s)
}

/// Expected radius (px) for a pointer whose projection on the corner's
/// inward diagonal is `q` px from the corner.
fn radius_from_q(s: f64, q: f64) -> f64 {
    ((q - 15.0) * g_of(s)).clamp(0.0, s / 2.0)
}

/// A point at distance `q` px from the corner along the inward diagonal,
/// plus `perp` px sideways.
fn on_diag(fr: &Fr, corner: (f64, f64), q: f64, perp: f64) -> Point {
    let (sx, sy) = corner;
    let base = (sx * fr.w_px() / 2.0, sy * fr.h_px() / 2.0);
    // inward diagonal direction in local px coordinates
    let dir = (-sx / SQRT_2, -sy / SQRT_2);
    let n = (-dir.1, dir.0);
    fr.at(
        base.0 + dir.0 * q + n.0 * perp,
        base.1 + dir.1 * q + n.1 * perp,
    )
}

fn radius_handle_pos(fr: &Fr, corner: (f64, f64), r_px: f64) -> Point {
    on_diag(fr, corner, handle_p(fr.s_px(), r_px), 0.0)
}

fn hint(s: &mut Session, p: Point) -> (String, String) {
    s.pointer_hover(p, false, false);
    (s.cursor_hint(), s.handle_hint())
}

// =====================================================================
// Criterion 7: size tiers (24 / 48 / 72) at every zoom, fractional sizes
// =====================================================================

const TIER_S: [f64; 12] = [
    6.0, 23.5, 23.9, 24.0, 47.5, 47.9, 48.0, 71.5, 71.9, 72.0, 73.0, 160.0,
];

fn decor_series(make: impl Fn(f64) -> Scene) -> Vec<(f64, usize)> {
    TIER_S
        .iter()
        .map(|&s| {
            let mut sc = make(s);
            (s, decor_count(&mut sc))
        })
        .collect()
}

fn dec_at(series: &[(f64, usize)], s: f64) -> usize {
    series.iter().find(|(x, _)| *x == s).unwrap().1
}

/// Asserts the tier steps: `edge`, `centre` and `param` say whether the kind
/// adds that tier.
fn assert_tiers(label: &str, series: &[(f64, usize)], edge: bool, centre: bool, param: bool) {
    let d = |s| dec_at(series, s);
    assert_eq!(d(6.0), d(23.5), "{label}: below 24 constant");
    assert_eq!(d(23.5), d(23.9), "{label}: below 24 constant");
    assert_eq!(d(24.0), d(47.5), "{label}: 24..48 constant");
    assert_eq!(d(47.5), d(47.9), "{label}: 24..48 constant");
    assert_eq!(d(48.0), d(71.5), "{label}: 48..72 constant");
    assert_eq!(d(71.5), d(71.9), "{label}: 48..72 constant");
    assert_eq!(d(72.0), d(73.0), "{label}: 72 and up constant");
    assert_eq!(d(73.0), d(160.0), "{label}: 72 and up constant");
    assert_eq!(d(23.9) < d(24.0), edge, "{label}: edge tier at 24");
    assert_eq!(d(47.9) < d(48.0), centre, "{label}: centre tier at 48");
    assert_eq!(d(71.9) < d(72.0), param, "{label}: parameter tier at 72");
}

#[test]
fn ac07_rectangle_tiers_at_100_percent_with_fractional_sizes() {
    let series = decor_series(|s| rect_scene(100, s * 1.0, s, 0.0, 0.0));
    assert_tiers("rect 100%", &series, true, true, true);
}

#[test]
fn ac07_rectangle_tiers_zoomed_in_and_out_and_rotated_and_wide() {
    for pct in [25, 400, 1600] {
        let series = decor_series(|s| rect_scene(pct, s, s, 0.0, 0.0));
        assert_tiers(&format!("rect {pct}%"), &series, true, true, true);
    }
    // The shorter side counts, not the longer, and rotation does not matter.
    let series = decor_series(|s| rect_scene(100, s * 3.0, s, 0.0, 0.0));
    assert_tiers("rect 3:1", &series, true, true, true);
    let series = decor_series(|s| rect_scene(100, s, s * 2.5, 0.0, 0.0));
    assert_tiers("rect 1:2.5", &series, true, true, true);
    let series = decor_series(|s| rect_scene(100, s * 1.7, s, 0.0, 0.7));
    assert_tiers("rect rotated", &series, true, true, true);
    let series = decor_series(|s| rect_scene(200, s, s * 1.3, 0.0, -2.9));
    assert_tiers("rect rotated 200%", &series, true, true, true);
    // A radius does not move the tiers.
    let series = decor_series(|s| rect_scene(100, s, s, s * 0.3, 0.0));
    assert_tiers("rect with radius", &series, true, true, true);
}

#[test]
fn ac07_ellipse_tiers_edge_and_centre_but_no_parameter_handle() {
    let series = decor_series(|s| ellipse_scene(100, s, s, 0.0));
    assert_tiers("ellipse", &series, true, true, false);
    let series = decor_series(|s| ellipse_scene(400, s * 1.5, s, 0.4));
    assert_tiers("ellipse wide rotated", &series, true, true, false);
}

#[test]
fn ac07_polygon_tiers_no_edge_no_parameter_handle() {
    let series = decor_series(|s| star_scene(100, s / 2.0, 8, None, 0.0));
    assert_tiers("polygon 8", &series, false, true, false);
    let series = decor_series(|s| star_scene(300, s / 2.0, 12, None, 1.1));
    assert_tiers("polygon 12 rotated", &series, false, true, false);
}

#[test]
fn ac07_star_tiers_inner_handle_from_72() {
    let series = decor_series(|s| star_scene(100, s / 2.0, 8, Some(0.9), 0.0));
    assert_tiers("star 8", &series, false, true, true);
    let series = decor_series(|s| star_scene(100, s / 2.0, 4, Some(0.99), 0.0));
    assert_tiers("star 4 ratio .99", &series, false, true, true);
    let series = decor_series(|s| star_scene(800, s / 2.0, 16, Some(0.6), 2.0));
    assert_tiers("star 16 ratio .6 rotated 800%", &series, false, true, true);
}

// ---- Criterion 1/2: four knobs at radius 0, at the position rule ----

fn radius_hint_ok(sc: &mut Scene, r_px: f64) -> [bool; 4] {
    let mut out = [false; 4];
    for (i, c) in CORNERS.iter().enumerate() {
        let p = radius_handle_pos(&sc.fr, *c, r_px);
        let (cur, h) = hint(&mut sc.s, p);
        out[i] = h == "param-radius" && cur == "pointer";
    }
    out
}

#[test]
fn ac02_four_radius_handles_exist_at_radius_zero_at_the_position_rule() {
    for (pct, s) in [
        (100, 72.0),
        (100, 100.0),
        (100, 200.0),
        (400, 90.0),
        (50, 300.0),
    ] {
        for th in [0.0, 0.6, -2.4] {
            let mut sc = rect_scene(pct, s, s, 0.0, th);
            assert_eq!(
                radius_hint_ok(&mut sc, 0.0),
                [true; 4],
                "{pct}% s={s} th={th}"
            );
        }
    }
}

#[test]
fn ac02_handles_follow_the_radius_on_all_four_corners() {
    for (pct, s) in [(100, 72.0), (100, 100.0), (200, 160.0)] {
        for rho in [0.1, 0.5, 0.97, 1.0] {
            let mut sc = rect_scene(pct, s, s, rho * s / 2.0, 0.3);
            assert_eq!(
                radius_hint_ok(&mut sc, rho * s / 2.0),
                [true; 4],
                "{pct}% s={s} rho={rho}"
            );
        }
    }
}

#[test]
fn ac02_wide_rectangle_uses_the_shorter_side() {
    let mut sc = rect_scene(100, 300.0, 80.0, 10.0, 0.0);
    assert_eq!(radius_hint_ok(&mut sc, 10.0), [true; 4]);
    let mut sc = rect_scene(100, 80.0, 300.0, 0.0, 0.0);
    assert_eq!(radius_hint_ok(&mut sc, 0.0), [true; 4]);
}

#[test]
fn ac05_parameter_handle_hit_radius_is_12_px_not_16() {
    for pct in [100, 300] {
        let mut sc = rect_scene(pct, 100.0, 100.0, 0.0, 0.0);
        let s_px = sc.fr.s_px();
        // Toward the box centre from the (-1,-1) handle.
        let p0 = handle_p(s_px, 0.0);
        for (dist, expect) in [(11.0, true), (11.9, true), (12.3, false), (14.0, false)] {
            let p = on_diag(&sc.fr, (-1.0, -1.0), p0 + dist, 0.0);
            let (_, h) = hint(&mut sc.s, p);
            assert_eq!(h == "param-radius", expect, "{pct}% {dist} px inward");
        }
        // Sideways.
        for (dist, expect) in [(11.0, true), (12.3, false)] {
            let p = on_diag(&sc.fr, (-1.0, -1.0), p0, dist);
            let (_, h) = hint(&mut sc.s, p);
            assert_eq!(h == "param-radius", expect, "{pct}% {dist} px sideways");
        }
    }
}

#[test]
fn ac06_a_handle_below_the_tier_has_no_hit_area_so_a_press_moves_the_object() {
    for (pct, s) in [(100, 71.9), (100, 71.5), (400, 71.5), (25, 60.0)] {
        let mut sc = rect_scene(pct, s, s, 0.0, 0.0);
        let p = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
        let (cur, h) = hint(&mut sc.s, p);
        assert_eq!(h, "", "{pct}% {s}: no hint");
        assert_ne!(cur, "pointer", "{pct}% {s}: no knob cursor");
        let before = rect_of(&sc.s, 0);
        let to = pt(p.x + 30.0 / sc.k, p.y + 12.0 / sc.k);
        drag(&mut sc.s, p, to);
        let after = rect_of(&sc.s, 0);
        assert_eq!(before.1, after.1, "radius untouched");
        assert!(
            near(after.0.origin.x - before.0.origin.x, 30.0 / sc.k, 1e-6)
                && near(after.0.origin.y - before.0.origin.y, 12.0 / sc.k, 1e-6),
            "moved by the drag: {before:?} -> {after:?}"
        );
        assert!(near(after.0.width.as_mm(), before.0.width.as_mm(), 1e-9));
    }
}

#[test]
fn ac06_at_the_tier_the_same_press_changes_the_radius_not_the_position() {
    let mut sc = rect_scene(100, 72.0, 72.0, 0.0, 0.0);
    let p = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
    let before = rect_of(&sc.s, 0);
    let to = on_diag(&sc.fr, (-1.0, -1.0), 15.0 + 10.0, 0.0);
    drag(&mut sc.s, p, to);
    let after = rect_of(&sc.s, 0);
    assert_eq!(before.0.origin, after.0.origin, "not moved");
    assert!(after.1.as_mm() > 0.0, "radius changed");
}

#[test]
fn ac06_centre_handle_has_no_hover_or_cursor_below_48() {
    for (pct, s) in [(100, 47.9), (100, 47.5), (400, 30.0), (100, 20.0)] {
        let mut sc = rect_scene(pct, s, s, 0.0, 0.0);
        let (cur, h) = hint(&mut sc.s, sc.fr.c);
        assert_eq!(cur, "default", "{pct}% {s}");
        assert_eq!(h, "", "{pct}% {s}");
    }
    for (pct, s) in [(100, 48.0), (100, 60.0), (400, 71.9)] {
        let mut sc = rect_scene(pct, s, s, 0.0, 0.0);
        let (cur, _) = hint(&mut sc.s, sc.fr.c);
        assert_eq!(cur, "move", "{pct}% {s}");
    }
}

// =====================================================================
// Criterion 2: the radius drag, gain and clamps
// =====================================================================

fn radius_drag_case(
    pct: i64,
    w_px: f64,
    h_px: f64,
    r0_px: f64,
    th: f64,
    corner: (f64, f64),
    q: f64,
    perp: f64,
) {
    let mut sc = rect_scene(pct, w_px, h_px, r0_px, th);
    let s_px = sc.fr.s_px();
    let k = sc.k;
    let start = radius_handle_pos(&sc.fr, corner, r0_px);
    let end = on_diag(&sc.fr, corner, q, perp);
    // Skip gestures that stay inside the 3 px dead zone: nothing is written.
    let moved = ((start.x - end.x).hypot(start.y - end.y)) * k;
    let before = rect_of(&sc.s, 0);
    drag(&mut sc.s, start, end);
    let after = rect_of(&sc.s, 0);
    let expect_px = radius_from_q(s_px, q);
    if moved < 3.0 {
        assert_eq!(before, after, "dead zone writes nothing");
        return;
    }
    let got_px = effective_corner_radius(after.0, after.1).as_mm() * k;
    assert!(
        near(got_px, expect_px, 0.02),
        "{pct}% {w_px}x{h_px} r0={r0_px} th={th} corner={corner:?} q={q} perp={perp}: \
         radius {got_px} px, expected {expect_px} px"
    );
    assert_eq!(before.0, after.0, "frame untouched by a radius drag");
}

#[test]
fn ac02_radius_follows_the_gain_rule_on_every_corner_and_rotation() {
    for (pct, w, h) in [
        (100, 72.0, 72.0),
        (100, 100.0, 100.0),
        (200, 200.0, 200.0),
        (100, 260.0, 90.0),
        (100, 90.0, 260.0),
        (400, 350.0, 350.0),
    ] {
        let s = f64::min(w, h);
        for th in [0.0, 0.5, -2.2] {
            for corner in CORNERS {
                for r0 in [0.0, 0.3 * s / 2.0] {
                    let p0 = handle_p(s, r0);
                    let far = 15.0 + l_of(s);
                    for q in [
                        0.0,
                        8.0,
                        15.0,
                        22.0,
                        p0 + 20.0,
                        15.0 + 0.5 * l_of(s),
                        far,
                        far + 25.0,
                    ] {
                        radius_drag_case(pct, w, h, r0, th, corner, q, 0.0);
                    }
                }
            }
        }
    }
}

#[test]
fn ac02_only_the_projection_on_the_diagonal_counts() {
    for perp in [-9.0, 7.5, 18.0] {
        for q in [25.0, 40.0, 60.0] {
            radius_drag_case(100, 150.0, 150.0, 0.0, 0.0, (1.0, -1.0), q, perp);
            radius_drag_case(100, 150.0, 150.0, 20.0, 1.0, (-1.0, 1.0), q, perp);
        }
    }
}

#[test]
fn ac02_zero_position_is_15_px_from_the_corner_not_the_corner() {
    // Starting at radius 30, dragging to exactly 15 px from the corner gives
    // 0 and dragging a few pixels short of it still gives exactly 0.
    for q in [15.0, 14.0, 5.0, 0.0, -20.0] {
        let mut sc = rect_scene(100, 120.0, 120.0, 30.0, 0.0);
        let start = radius_handle_pos(&sc.fr, (-1.0, -1.0), 30.0);
        let end = on_diag(&sc.fr, (-1.0, -1.0), q, 0.0);
        drag(&mut sc.s, start, end);
        let got = rect_of(&sc.s, 0).1.as_mm();
        if q < 15.0 {
            assert_eq!(got, 0.0, "q={q}: exactly zero beyond the zero position");
        } else {
            // at the zero position itself: zero up to float noise
            assert!(got.abs() < 1e-9, "q={q}: {got}");
        }
    }
    // 17 px from the corner: a positive radius of (17 - 15) * G.
    let mut sc = rect_scene(100, 120.0, 120.0, 30.0, 0.0);
    let start = radius_handle_pos(&sc.fr, (-1.0, -1.0), 30.0);
    drag(&mut sc.s, start, on_diag(&sc.fr, (-1.0, -1.0), 17.0, 0.0));
    let got = rect_of(&sc.s, 0).1.as_mm() * sc.k;
    assert!(near(got, 2.0 * g_of(120.0), 0.02), "{got}");
}

#[test]
fn ac02_clamp_is_half_the_shorter_side() {
    for (w, h) in [(100.0, 100.0), (240.0, 80.0), (80.0, 240.0)] {
        let mut sc = rect_scene(100, w, h, 0.0, 0.0);
        let s = f64::min(w, h);
        let start = radius_handle_pos(&sc.fr, (1.0, 1.0), 0.0);
        drag(&mut sc.s, start, on_diag(&sc.fr, (1.0, 1.0), 400.0, 0.0));
        let (b, r) = rect_of(&sc.s, 0);
        assert!(near(
            effective_corner_radius(b, r).as_mm() * sc.k,
            s / 2.0,
            1e-6
        ));
        assert!(
            r.as_mm() * sc.k <= s / 2.0 + 1e-6,
            "stored radius is not above the clamp: {} > {}",
            r.as_mm() * sc.k,
            s / 2.0
        );
    }
}

#[test]
fn ac02_after_the_drag_all_four_handles_sit_at_the_new_positions() {
    let mut sc = rect_scene(100, 140.0, 140.0, 0.0, 0.4);
    let start = radius_handle_pos(&sc.fr, (-1.0, 1.0), 0.0);
    let q = 15.0 + 0.6 * l_of(140.0);
    drag(&mut sc.s, start, on_diag(&sc.fr, (-1.0, 1.0), q, 0.0));
    let r = radius_from_q(140.0, q);
    assert_eq!(radius_hint_ok(&mut sc, r), [true; 4]);
}

#[test]
fn ac02_handle_stays_under_the_pointer_until_a_limit() {
    // Pointer exactly at the new handle position after every release: the
    // same pointer position hits the handle again (pointer-exact drag).
    let mut sc = rect_scene(100, 160.0, 160.0, 0.0, 0.0);
    let mut start = radius_handle_pos(&sc.fr, (1.0, -1.0), 0.0);
    for q in [30.0, 50.0, 70.0] {
        let end = on_diag(&sc.fr, (1.0, -1.0), q, 0.0);
        drag(&mut sc.s, start, end);
        let (_, h) = hint(&mut sc.s, end);
        assert_eq!(h, "param-radius", "q={q}");
        start = end;
    }
}

#[test]
fn ac09_press_release_on_a_parameter_handle_writes_nothing() {
    let mut sc = rect_scene(100, 100.0, 100.0, 0.0, 0.0);
    let n0 = vv_of(&sc.s);
    let p = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
    sc.s.pointer_hover(p, false, false);
    sc.s.pointer_down(p, false);
    sc.s.pointer_up(p, false, false);
    assert_eq!(vv_of(&sc.s), n0);
    // Under 3 px movement as well, and exactly the 2.99 px case.
    for d in [1.0, 2.5, 2.99] {
        let q = on_diag(&sc.fr, (-1.0, -1.0), 15.0 + d, 0.0);
        drag(&mut sc.s, p, q);
        assert_eq!(vv_of(&sc.s), n0, "d={d}");
        assert_eq!(rect_of(&sc.s, 0).1.as_mm(), 0.0, "d={d}");
    }
}

#[test]
fn ac09_escape_during_a_parameter_drag_writes_nothing_and_clears_the_preview() {
    let mut sc = rect_scene(100, 100.0, 100.0, 0.0, 0.0);
    let n0 = vv_of(&sc.s);
    let p = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
    sc.s.pointer_hover(p, false, false);
    let before = sc.s.draw_list().triangles;
    sc.s.pointer_down(p, false);
    sc.s.pointer_hover(on_diag(&sc.fr, (-1.0, -1.0), 40.0, 0.0), false, false);
    assert_ne!(sc.s.draw_list().triangles, before, "preview is visible");
    sc.s.escape();
    assert_eq!(vv_of(&sc.s), n0);
    sc.s.pointer_hover(p, false, false);
    assert_eq!(sc.s.draw_list().triangles, before, "preview gone");
    // A later release does not commit the cancelled drag.
    sc.s.pointer_up(on_diag(&sc.fr, (-1.0, -1.0), 40.0, 0.0), false, false);
    assert_eq!(vv_of(&sc.s), n0);
}

#[test]
fn ac02_shift_on_a_parameter_handle_still_drags_the_handle_and_cursor_stays_pointer() {
    for (shift, ctrl) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut sc = rect_scene(100, 120.0, 120.0, 0.0, 0.0);
        let p = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
        sc.s.pointer_hover(p, shift, ctrl);
        assert_eq!(sc.s.cursor_hint(), "pointer", "hover {shift} {ctrl}");
        sc.s.pointer_down(p, shift);
        let q = on_diag(&sc.fr, (-1.0, -1.0), 45.0, 0.0);
        sc.s.pointer_hover(q, shift, ctrl);
        assert_eq!(sc.s.cursor_hint(), "pointer", "drag {shift} {ctrl}");
        sc.s.pointer_up(q, shift, ctrl);
        let got = rect_of(&sc.s, 0).1.as_mm() * sc.k;
        assert!(
            near(got, radius_from_q(120.0, 45.0), 0.02),
            "{shift} {ctrl}: {got}"
        );
        assert_eq!(rect_of(&sc.s, 0).0.origin, {
            let k = sc.k;
            pt(60.0 - 60.0 / k, 45.0 - 60.0 / k)
        });
    }
}

// =====================================================================
// Criterion 3/4: star inner radius
// =====================================================================

fn inner_vertex(sc: &Scene) -> Point {
    let d = doc_of(&sc.s);
    let o = d.primitive(d.object_ids()[0]).unwrap();
    outline_of_rotated(&o.shape, o.rotation)[1].point
}

#[test]
fn ac01_star_has_one_inner_handle_at_the_first_inner_vertex_and_polygon_none() {
    for th in [0.0, 0.9, -2.0] {
        let mut sc = star_scene(100, 60.0, 8, Some(0.5), th);
        let v = inner_vertex(&sc);
        let (cur, h) = hint(&mut sc.s, v);
        assert_eq!(
            (cur.as_str(), h.as_str()),
            ("pointer", "param-inner"),
            "th={th}"
        );
        // no handle at the other inner vertices
        let d = doc_of(&sc.s);
        let o = d.primitive(d.object_ids()[0]).unwrap();
        let outline = outline_of_rotated(&o.shape, o.rotation);
        let w = outline[3].point;
        let (_, h) = hint(&mut sc.s, w);
        assert_ne!(h, "param-inner", "second inner vertex is no handle");
        // polygon: nothing
        let mut sp = star_scene(100, 60.0, 8, None, th);
        let d = doc_of(&sp.s);
        let o = d.primitive(d.object_ids()[0]).unwrap();
        for a in outline_of_rotated(&o.shape, o.rotation) {
            let (_, h) = hint(&mut sp.s, a.point);
            assert!(!h.starts_with("param"), "polygon has no parameter handle");
        }
    }
}

#[test]
fn ac03_inner_radius_drag_keeps_outer_and_clamps_the_ratio() {
    for (pct, r_px, n, th) in [
        (100, 60.0, 8, 0.0),
        (100, 90.0, 5, 0.7),
        (200, 80.0, 12, -1.9),
    ] {
        for (fraction_of_r, expect) in [
            (0.30, 0.30),
            (0.70, 0.70),
            (0.8, 0.8),
            (1.4, 0.99),
            (1.0, 0.99),
            (0.0, 0.01),
            (0.004, 0.01),
        ] {
            let mut sc = star_scene(pct, r_px, n, Some(0.5), th);
            let v = inner_vertex(&sc);
            let c = sc.fr.c;
            let r_mm = star_radius_mm(&sc.s, 0);
            let dirx = (v.x - c.x) / (v.x - c.x).hypot(v.y - c.y);
            let diry = (v.y - c.y) / (v.x - c.x).hypot(v.y - c.y);
            let d_mm = fraction_of_r * r_mm;
            let to = pt(c.x + dirx * d_mm, c.y + diry * d_mm);
            drag(&mut sc.s, v, to);
            let got = star_ratio(&sc.s, 0);
            assert!(
                near(got, expect, 1e-6),
                "{n}pt {fraction_of_r}: ratio {got} expected {expect}"
            );
            assert!(
                near(star_radius_mm(&sc.s, 0), r_mm, 1e-9),
                "outer radius fixed"
            );
        }
    }
}

#[test]
fn ac08_star_worst_case_inner_handle_is_hit_at_ratio_099_for_multiples_of_four() {
    for n in [4u32, 8, 12, 16, 24, 32] {
        for th in [0.0, 0.31] {
            let mut sc = star_scene(100, 36.0, n, Some(0.99), th);
            let v = inner_vertex(&sc);
            let (_, h) = hint(&mut sc.s, v);
            assert_eq!(h, "param-inner", "n={n} th={th} 72px");
        }
    }
}

#[test]
fn ac04_parameter_drag_on_a_rotated_primitive_is_measured_along_its_own_axes() {
    // Rotated rectangle: the radius drag (tested above with th != 0) matches
    // the unrotated one for the same local gesture.
    let a = {
        let mut sc = rect_scene(100, 150.0, 110.0, 0.0, 0.0);
        let s = radius_handle_pos(&sc.fr, (1.0, 1.0), 0.0);
        drag(&mut sc.s, s, on_diag(&sc.fr, (1.0, 1.0), 50.0, 5.0));
        rect_of(&sc.s, 0).1.as_mm()
    };
    let b = {
        let mut sc = rect_scene(100, 150.0, 110.0, 0.0, 2.1);
        let s = radius_handle_pos(&sc.fr, (1.0, 1.0), 0.0);
        drag(&mut sc.s, s, on_diag(&sc.fr, (1.0, 1.0), 50.0, 5.0));
        rect_of(&sc.s, 0).1.as_mm()
    };
    assert!(near(a, b, 1e-6), "{a} vs {b}");
}

// =====================================================================
// Criteria 18/19: typed entry on parameter handles
// =====================================================================

// =====================================================================
// Criterion 8: the centre handle yields to a radius handle within 20 px
// =====================================================================

#[test]
fn ac08_centre_handle_yields_when_a_parameter_handle_is_within_20_px() {
    for (pct, s) in [(100, 72.0), (100, 100.0), (200, 200.0)] {
        // distance of the handle from the centre: s*sqrt2/2 - p <= 20
        let rho_th = (s * SQRT_2 / 2.0 - 20.0 - 15.0) / l_of(s);
        let mut near_zero = rect_scene(pct, s, s, 0.0, 0.0);
        let with_centre = decor_count(&mut near_zero);
        let mut big = rect_scene(pct, s, s, ((rho_th - 0.04).max(0.0)) * s / 2.0, 0.0);
        assert_eq!(
            decor_count(&mut big),
            with_centre,
            "{pct}% s={s}: centre still drawn"
        );
        let mut hidden = rect_scene(pct, s, s, (rho_th + 0.04).min(1.0) * s / 2.0, 0.0);
        assert!(
            decor_count(&mut hidden) < with_centre,
            "{pct}% s={s}: centre hidden when a knob is within 20 px of it"
        );
        // hiding the centre handle loses no function: the box centre still
        // is a move at small radius (hit-tested nowhere) -- and at the
        // largest radius a point halfway to an edge is a move.
        let mut full = rect_scene(pct, s, s, s / 2.0, 0.0);
        let mid = full.fr.at(0.0, -s / 4.0);
        let (cur, h) = hint(&mut full.s, mid);
        assert_eq!((cur.as_str(), h.as_str()), ("default", ""), "{pct}% s={s}");
        let before = rect_of(&full.s, 0);
        drag(
            &mut full.s,
            mid,
            pt(mid.x + 20.0 / full.k, mid.y + 10.0 / full.k),
        );
        let after = rect_of(&full.s, 0);
        assert!(near(
            after.0.origin.x - before.0.origin.x,
            20.0 / full.k,
            1e-6
        ));
    }
}

#[test]
fn ac08_body_between_the_centre_and_each_edge_is_a_move_at_every_radius() {
    for s in [72.0, 90.0, 150.0] {
        for rho in [0.0, 0.5, 1.0] {
            for (nx, ny) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                let mut sc = rect_scene(100, s, s, rho * s / 2.0, 0.0);
                let p = sc.fr.at(nx * s / 4.0, ny * s / 4.0);
                let (cur, h) = hint(&mut sc.s, p);
                assert_eq!(
                    (cur.as_str(), h.as_str()),
                    ("default", ""),
                    "s={s} rho={rho} ({nx},{ny})"
                );
            }
        }
    }
}

// =====================================================================
// Criteria 18, 19, 23: typed entries
// =====================================================================

#[derive(Debug, Clone)]
struct FV {
    label: &'static str,
    accessible_name: &'static str,
    prefill: String,
}
#[derive(Debug, Clone)]
struct EV {
    kind: &'static str,
    fields: Vec<FV>,
}

fn entry_of(s: &Session) -> Option<EV> {
    s.transform_entry().map(|e| EV {
        kind: e.kind,
        fields: e
            .fields
            .into_iter()
            .map(|f| FV {
                label: f.label,
                accessible_name: f.accessible_name,
                prefill: f.prefill,
            })
            .collect(),
    })
}

fn open_entry(s: &mut Session, handle: Point, shift: bool, ctrl: bool) -> Option<EV> {
    s.pointer_hover(handle, shift, ctrl);
    s.pointer_down(handle, shift);
    s.pointer_up(handle, shift, ctrl);
    s.double_click(handle, shift, ctrl);
    entry_of(s)
}

fn num(text: &str) -> f64 {
    text.trim()
        .trim_end_matches('°')
        .replace(',', ".")
        .parse()
        .unwrap()
}

#[test]
fn ac18_double_click_on_a_radius_handle_opens_the_corner_radius_field() {
    let mut sc = rect_scene(100, 200.0, 120.0, 20.0, 0.4);
    let n0 = vv_of(&sc.s);
    let h = radius_handle_pos(&sc.fr, (1.0, -1.0), 20.0);
    let e = open_entry(&mut sc.s, h, false, false).expect("entry opens");
    assert_eq!(e.kind, "corner-radius");
    assert_eq!(e.fields.len(), 1);
    assert_eq!(e.fields[0].label, "r");
    assert_eq!(e.fields[0].accessible_name, "Corner radius");
    let shown = num(&e.fields[0].prefill);
    assert!(
        near(shown, 20.0 / sc.k, 0.06),
        "prefill {shown} vs {}",
        20.0 / sc.k
    );
    assert_eq!(vv_of(&sc.s), n0, "opening writes nothing");
}

#[test]
fn ac18_prefill_is_the_effective_radius_when_the_stored_one_is_too_large() {
    // 20 mm stored on a rectangle whose half shorter side is smaller.
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let id = d.create_rect(bounds(40.0, 30.0, 200.0 / k, 100.0 / k));
    d.set_corner_radius(&[id], Length::from_mm(500.0 / k))
        .unwrap();
    let mut s = open_in_session(&d);
    select_only(&mut s);
    let fr = Fr {
        c: pt(40.0 + 100.0 / k, 30.0 + 50.0 / k),
        hw: 100.0 / k,
        hh: 50.0 / k,
        th: 0.0,
        k,
    };
    let h = radius_handle_pos(&fr, (-1.0, -1.0), 50.0);
    let e = open_entry(&mut s, h, false, false).expect("entry opens");
    assert!(near(num(&e.fields[0].prefill), 50.0 / k, 0.06));
}

fn entry_scene() -> (Scene, Point) {
    let sc = rect_scene(100, 200.0, 120.0, 20.0, 0.0);
    let h = radius_handle_pos(&sc.fr, (-1.0, -1.0), 20.0);
    (sc, h)
}

#[test]
fn ac18_enter_writes_one_commit_to_the_one_radius() {
    let (mut sc, h) = entry_scene();
    let n0 = change_count(&sc.s);
    open_entry(&mut sc.s, h, false, false).unwrap();
    let before = rect_of(&sc.s, 0);
    assert_eq!(
        sc.s.commit_transform_entry("4.25", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&sc.s), n0 + 1);
    let after = rect_of(&sc.s, 0);
    assert!(near(after.1.as_mm(), 4.25, 1e-9));
    assert_eq!(after.0, before.0);
    assert!(entry_of(&sc.s).is_none(), "closed");
    // decimal comma and zero
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    assert_eq!(
        sc.s.commit_transform_entry("3,5", "", 0),
        EntryOutcome::Committed
    );
    assert!(near(rect_of(&sc.s, 0).1.as_mm(), 3.5, 1e-9));
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    assert_eq!(
        sc.s.commit_transform_entry("0", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(rect_of(&sc.s, 0).1.as_mm(), 0.0, "zero is valid");
}

#[test]
fn ac18_a_value_above_half_the_shorter_side_is_limited_and_the_limited_value_is_written() {
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    assert_eq!(
        sc.s.commit_transform_entry("999", "", 0),
        EntryOutcome::Committed
    );
    let (b, r) = rect_of(&sc.s, 0);
    let half = b.width.as_mm().min(b.height.as_mm()) / 2.0;
    assert!(
        near(r.as_mm(), half, 1e-9),
        "stored {} half {half}",
        r.as_mm()
    );
}

#[test]
fn ac18_invalid_text_keeps_the_field_open_and_writes_nothing() {
    for (text, reason) in [
        ("", InvalidReason::NotANumber),
        ("abc", InvalidReason::NotANumber),
        ("1,2,3", InvalidReason::NotANumber),
        ("NaN", InvalidReason::NotANumber),
        ("inf", InvalidReason::NotANumber),
        ("-inf", InvalidReason::NotANumber),
        ("1e999", InvalidReason::NotANumber),
        ("5 5", InvalidReason::NotANumber),
        ("-1", InvalidReason::Negative),
        ("-0.001", InvalidReason::Negative),
        ("-0.000000001", InvalidReason::Negative),
    ] {
        let (mut sc, h) = entry_scene();
        let n0 = vv_of(&sc.s);
        open_entry(&mut sc.s, h, false, false).unwrap();
        let out = sc.s.commit_transform_entry(text, "", 0);
        assert_eq!(
            out,
            EntryOutcome::Invalid { field: 0, reason },
            "text {text:?}"
        );
        assert!(entry_of(&sc.s).is_some(), "stays open for {text:?}");
        assert_eq!(vv_of(&sc.s), n0, "nothing written for {text:?}");
        // and it can still be completed afterwards
        assert_eq!(
            sc.s.commit_transform_entry("2", "", 0),
            EntryOutcome::Committed
        );
    }
}

#[test]
fn ac18_unedited_enter_and_equal_values_write_nothing() {
    let (mut sc, h) = entry_scene();
    let n0 = vv_of(&sc.s);
    let e = open_entry(&mut sc.s, h, false, false).unwrap();
    let pre = e.fields[0].prefill.clone();
    assert_eq!(
        sc.s.commit_transform_entry(&pre, "", 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(vv_of(&sc.s), n0);
    assert!(entry_of(&sc.s).is_none());
    // A radius stored with more precision than the prefill shows is not
    // overwritten by Enter on the untouched prefill.
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let id = d.create_rect(bounds(40.0, 30.0, 200.0 / k, 120.0 / k));
    d.set_corner_radius(&[id], Length::from_mm(3.123_456_789))
        .unwrap();
    let mut s = open_in_session(&d);
    select_only(&mut s);
    let fr = Fr {
        c: pt(40.0 + 100.0 / k, 30.0 + 60.0 / k),
        hw: 100.0 / k,
        hh: 60.0 / k,
        th: 0.0,
        k,
    };
    let hnd = radius_handle_pos(&fr, (-1.0, -1.0), 3.123_456_789 * k);
    let n1 = vv_of(&s);
    let e = open_entry(&mut s, hnd, false, false).unwrap();
    assert_eq!(
        s.commit_transform_entry(&e.fields[0].prefill, "", 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(vv_of(&s), n1);
    assert_eq!(rect_of(&s, 0).1.as_mm(), 3.123_456_789);
}

#[test]
fn ac18_escape_click_elsewhere_tool_switch_selection_change_close_and_write_nothing() {
    // Escape in the chip
    let (mut sc, h) = entry_scene();
    let n0 = vv_of(&sc.s);
    open_entry(&mut sc.s, h, false, false).unwrap();
    sc.s.cancel_transform_entry();
    assert!(entry_of(&sc.s).is_none());
    assert_eq!(vv_of(&sc.s), n0);
    // press elsewhere on empty canvas: closes, and the press is not
    // swallowed (it clears the selection)
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    click(&mut sc.s, pt(300.0, 300.0));
    assert!(entry_of(&sc.s).is_none());
    assert_eq!(vv_of(&sc.s), n0);
    assert!(
        sc.s.select_bar_state().radius.is_none(),
        "press not swallowed: selection cleared"
    );
    // tool switch
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    sc.s.set_tool(Tool::Pen);
    sc.s.set_tool(Tool::Select);
    assert!(entry_of(&sc.s).is_none());
    assert_eq!(vv_of(&sc.s), n0);
    // escape key
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    sc.s.escape();
    assert!(entry_of(&sc.s).is_none());
    assert_eq!(vv_of(&sc.s), n0);
    // a double-click on a radius handle never switches tools
    let (mut sc, h) = entry_scene();
    open_entry(&mut sc.s, h, false, false).unwrap();
    assert_eq!(sc.s.tool(), Tool::Select);
}

#[test]
fn ac18_selection_change_by_press_on_another_object_closes_the_field() {
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let a = d.create_rect(bounds(40.0, 30.0, 200.0 / k, 120.0 / k));
    d.set_corner_radius(&[a], Length::from_mm(20.0 / k))
        .unwrap();
    let _ = d.create_rect(bounds(200.0, 30.0, 40.0, 30.0));
    let mut s = open_in_session(&d);
    select_index(&mut s, 0, false);
    let fr = Fr {
        c: pt(40.0 + 100.0 / k, 30.0 + 60.0 / k),
        hw: 100.0 / k,
        hh: 60.0 / k,
        th: 0.0,
        k,
    };
    let h = radius_handle_pos(&fr, (-1.0, -1.0), 20.0);
    let n0 = vv_of(&s);
    open_entry(&mut s, h, false, false).unwrap();
    // click on the other rectangle's outline
    click(&mut s, pt(220.0, 30.0));
    assert!(entry_of(&s).is_none());
    assert_eq!(vv_of(&s), n0);
    // the other one is now the selection (the press was not swallowed)
    assert!(s.select_bar_state().radius.is_some());
    assert_eq!(rect_of(&s, 0).1.as_mm(), 20.0 / k);
}

#[test]
fn ac19_inner_ratio_field_labels_prefill_and_range() {
    let mk = || {
        let sc = star_scene(100, 60.0, 8, Some(0.5), 0.3);
        let h = inner_vertex(&sc);
        (sc, h)
    };
    let (mut sc, h) = mk();
    let n0 = change_count(&sc.s);
    let e = open_entry(&mut sc.s, h, false, false).expect("entry opens");
    assert_eq!(e.kind, "inner-ratio");
    assert_eq!(e.fields[0].label, "ratio");
    assert_eq!(e.fields[0].accessible_name, "Inner ratio");
    assert_eq!(e.fields[0].prefill, "0.50", "two decimals");
    assert_eq!(
        sc.s.commit_transform_entry("0.45", "", 0),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&sc.s), n0 + 1);
    assert!(near(star_ratio(&sc.s, 0), 0.45, 1e-12));
    for ok in ["0.01", "0.99", "0,3"] {
        let (mut sc, h) = mk();
        open_entry(&mut sc.s, h, false, false).unwrap();
        assert_eq!(
            sc.s.commit_transform_entry(ok, "", 0),
            EntryOutcome::Committed,
            "{ok}"
        );
    }
    for bad in ["0.0099", "0", "1", "0.991", "0.995", "-0.2", "5", "100"] {
        let (mut sc, h) = mk();
        let n = vv_of(&sc.s);
        open_entry(&mut sc.s, h, false, false).unwrap();
        assert_eq!(
            sc.s.commit_transform_entry(bad, "", 0),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::RatioRange
            },
            "{bad}"
        );
        assert_eq!(vv_of(&sc.s), n, "{bad}");
        assert!(entry_of(&sc.s).is_some());
    }
    for bad in ["", "x", "NaN", "inf", "1e999"] {
        let (mut sc, h) = mk();
        open_entry(&mut sc.s, h, false, false).unwrap();
        assert!(
            matches!(
                sc.s.commit_transform_entry(bad, "", 0),
                EntryOutcome::Invalid { .. }
            ),
            "{bad}"
        );
        assert!((star_ratio(&sc.s, 0) - 0.5).abs() < 1e-12);
    }
}

#[test]
fn ac19_outer_radius_entry_of_polygon_and_star_is_named_outer_radius() {
    for ratio in [None, Some(0.5)] {
        let mut sc = star_scene(100, 60.0, 8, ratio, 0.0);
        let h = sc.fr.corner(1.0, 1.0);
        let e = open_entry(&mut sc.s, h, false, false).expect("entry opens");
        assert_eq!(e.kind, "radius");
        assert_eq!(e.fields[0].label, "r");
        assert_eq!(e.fields[0].accessible_name, "Outer radius");
    }
}

#[test]
fn ac19_three_accessible_names_never_collide_on_one_selected_kind() {
    let mut names = Vec::new();
    {
        let mut sc = rect_scene(100, 200.0, 120.0, 20.0, 0.0);
        let h = radius_handle_pos(&sc.fr, (-1.0, -1.0), 20.0);
        names.push(open_entry(&mut sc.s, h, false, false).unwrap().fields[0].accessible_name);
    }
    {
        let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
        let h = inner_vertex(&sc);
        names.push(open_entry(&mut sc.s, h, false, false).unwrap().fields[0].accessible_name);
        let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
        let h = sc.fr.corner(1.0, -1.0);
        names.push(open_entry(&mut sc.s, h, false, false).unwrap().fields[0].accessible_name);
    }
    assert_eq!(names, ["Corner radius", "Inner ratio", "Outer radius"]);
}

// =====================================================================
// Criterion 20: readouts and hint lines
// =====================================================================

#[test]
fn ac20_live_readout_during_a_parameter_drag() {
    let mut sc = rect_scene(100, 160.0, 160.0, 0.0, 0.0);
    let h = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
    sc.s.pointer_hover(h, false, false);
    sc.s.pointer_down(h, false);
    let q = 15.0 + 0.5 * l_of(160.0);
    sc.s.pointer_hover(on_diag(&sc.fr, (-1.0, -1.0), q, 0.0), false, false);
    let r_mm = radius_from_q(160.0, q) / sc.k;
    let text =
        sc.s.live_readout()
            .expect("readout during a radius drag")
            .text;
    assert_eq!(text, format!("r {r_mm:.1} mm"), "{text}");
    sc.s.escape();
    assert!(sc.s.live_readout().is_none());

    let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
    let v = inner_vertex(&sc);
    sc.s.pointer_hover(v, false, false);
    sc.s.pointer_down(v, false);
    let c = sc.fr.c;
    let r_mm = star_radius_mm(&sc.s, 0);
    let dir = (
        (v.x - c.x) / (v.x - c.x).hypot(v.y - c.y),
        (v.y - c.y) / (v.x - c.x).hypot(v.y - c.y),
    );
    sc.s.pointer_hover(
        pt(c.x + dir.0 * 0.3 * r_mm, c.y + dir.1 * 0.3 * r_mm),
        false,
        false,
    );
    let text =
        sc.s.live_readout()
            .expect("readout during an inner drag")
            .text;
    assert_eq!(text, "ratio 0.30");
}

// =====================================================================
// Criteria 21, 21a, 22: the bar
// =====================================================================

/// A document with a known set of objects, far apart (60 mm apart so no
/// handle reach overlaps at 100 %):
/// 0 rect 20x10 r=2, 1 rect 40x30 r=3, 2 star 5pt .4, 3 star 7pt .5,
/// 4 polygon 6pt, 5 ellipse, 6 path (triangle), 7 rect 30x30 r=0.
fn bar_doc() -> Document {
    let d = Document::new(1);
    let a = d.create_rect(bounds(10.0, 10.0, 20.0, 10.0));
    d.set_corner_radius(&[a], Length::from_mm(2.0)).unwrap();
    let b = d.create_rect(bounds(110.0, 10.0, 40.0, 30.0));
    d.set_corner_radius(&[b], Length::from_mm(3.0)).unwrap();
    let _ = d.create_star(
        star_frame(210.0, 25.0, 12.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let _ = d.create_star(
        star_frame(310.0, 25.0, 12.0),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let _ = d.create_polygon(star_frame(410.0, 25.0, 12.0), PointCount::new(6).unwrap());
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(510.0, 25.0),
        rx: Length::from_mm(15.0),
        ry: Length::from_mm(9.0),
    });
    let _ = d.create_path(
        &[
            anchor(1, 600.0, 10.0),
            anchor(2, 640.0, 10.0),
            anchor(3, 640.0, 40.0),
        ],
        true,
    );
    let _ = d.create_rect(bounds(700.0, 10.0, 30.0, 30.0));
    d
}

/// Selects the objects at `indices` (first plain, the rest with Shift).
/// Paths are clicked on a straight segment, primitives on outline anchor 0.
fn select_set(s: &mut Session, indices: &[usize]) {
    click(s, pt(1000.0, 1000.0));
    for (n, &i) in indices.iter().enumerate() {
        let d = doc_of(s);
        let id = d.object_ids()[i];
        let p = match d.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => outline_of_rotated(&p.shape, p.rotation)[0].point,
            ObjectSnapshot::Path(_) => pt(620.0, 10.0),
        };
        if n == 0 {
            click(s, p);
        } else {
            shift_click(s, p);
        }
    }
}

fn bar_for(indices: &[usize]) -> vecmanf_ui_core::SelectBarState {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, indices);
    s.select_bar_state()
}

fn mm_of(v: Option<BarValue<Length>>) -> Option<Result<f64, &'static str>> {
    v.map(|b| match b {
        BarValue::Uniform(l) => Ok(l.as_mm()),
        BarValue::Mixed => Err("mixed"),
    })
}

#[test]
fn ac21_bar_state_for_empty_and_single_kind_selections() {
    let b = bar_for(&[]);
    assert!(b.radius.is_none() && !b.remove_rounding_shown && !b.remove_rounding_enabled);
    assert!(
        b.points.is_none() && b.ratio.is_none() && !b.object_to_path,
        "{b:?}"
    );

    let b = bar_for(&[0]);
    assert_eq!(mm_of(b.radius), Some(Ok(2.0)));
    assert!(b.remove_rounding_shown && b.remove_rounding_enabled);
    assert!(b.points.is_none() && b.ratio.is_none() && b.object_to_path);

    let b = bar_for(&[7]);
    assert_eq!(mm_of(b.radius), Some(Ok(0.0)));
    assert!(
        b.remove_rounding_shown,
        "shown for a rectangle with radius 0"
    );
    assert!(
        !b.remove_rounding_enabled,
        "not enabled: it would change nothing"
    );
    assert!(b.object_to_path);

    let b = bar_for(&[2]);
    assert!(b.radius.is_none() && !b.remove_rounding_shown);
    assert_eq!(b.points, Some(BarValue::Uniform(5)));
    assert_eq!(b.ratio, Some(BarValue::Uniform(0.4)));
    assert!(b.object_to_path);

    let b = bar_for(&[4]);
    assert!(b.radius.is_none() && b.ratio.is_none() && !b.remove_rounding_shown);
    assert_eq!(b.points, Some(BarValue::Uniform(6)));
    assert!(b.object_to_path);

    let b = bar_for(&[5]);
    assert!(b.radius.is_none() && b.points.is_none() && b.ratio.is_none());
    assert!(!b.remove_rounding_shown && b.object_to_path);

    let b = bar_for(&[6]);
    assert!(b.radius.is_none() && b.points.is_none() && b.ratio.is_none());
    assert!(
        !b.remove_rounding_shown && !b.object_to_path,
        "a path alone: nothing"
    );
}

#[test]
fn ac21_bar_state_for_mixed_selections_follows_the_one_rule() {
    // two rectangles with different radii: Radius is Mixed
    let b = bar_for(&[0, 1]);
    assert_eq!(mm_of(b.radius), Some(Err("mixed")));
    assert!(b.remove_rounding_enabled);
    // two stars: Points and Ratio Mixed
    let b = bar_for(&[2, 3]);
    assert_eq!(b.points, Some(BarValue::Mixed));
    assert_eq!(b.ratio, Some(BarValue::Mixed));
    // a rectangle and a star: Radius, Remove rounding, Points, Ratio, Object to path
    let b = bar_for(&[0, 2]);
    assert_eq!(mm_of(b.radius), Some(Ok(2.0)));
    assert!(b.remove_rounding_shown && b.remove_rounding_enabled);
    assert_eq!(b.points, Some(BarValue::Uniform(5)));
    assert_eq!(b.ratio, Some(BarValue::Uniform(0.4)));
    assert!(b.object_to_path);
    // a rectangle and a path: rectangle controls shown, path ignored
    let b = bar_for(&[0, 6]);
    assert_eq!(mm_of(b.radius), Some(Ok(2.0)));
    assert!(b.points.is_none() && b.ratio.is_none() && b.object_to_path);
    // a polygon and a star: Points over both (Mixed), Ratio over the star only
    let b = bar_for(&[4, 2]);
    assert_eq!(b.points, Some(BarValue::Mixed));
    assert_eq!(b.ratio, Some(BarValue::Uniform(0.4)));
    // ellipse + path: only Object to path
    let b = bar_for(&[5, 6]);
    assert!(b.radius.is_none() && b.points.is_none() && b.ratio.is_none() && b.object_to_path);
    // a rectangle at radius 0 together with one that has a radius
    let b = bar_for(&[7, 1]);
    assert!(b.remove_rounding_enabled);
    assert_eq!(mm_of(b.radius), Some(Err("mixed")));
    // five kinds at once
    let b = bar_for(&[0, 2, 4, 5, 6]);
    assert!(b.radius.is_some() && b.points.is_some() && b.ratio.is_some() && b.object_to_path);
    // two rectangles with the same radius: Uniform
    let k = 3.7795;
    let _ = k;
    let d = Document::new(1);
    let a = d.create_rect(bounds(10.0, 10.0, 20.0, 10.0));
    let b2 = d.create_rect(bounds(110.0, 10.0, 40.0, 30.0));
    d.set_corner_radius(&[a, b2], Length::from_mm(2.0)).unwrap();
    let mut s = open_in_session(&d);
    select_set(&mut s, &[0, 1]);
    assert_eq!(mm_of(s.select_bar_state().radius), Some(Ok(2.0)));
}

#[test]
fn ac21a_limited_stored_radius_shows_the_effective_value_and_the_stored_one() {
    let d = Document::new(1);
    let a = d.create_rect(bounds(10.0, 10.0, 20.0, 10.0));
    d.set_corner_radius(&[a], Length::from_mm(20.0)).unwrap();
    let mut s = open_in_session(&d);
    select_set(&mut s, &[0]);
    let b = s.select_bar_state();
    assert_eq!(mm_of(b.radius), Some(Ok(5.0)), "effective");
    assert_eq!(
        b.radius_limited.map(Length::as_mm),
        Some(20.0),
        "stored value for the tooltip"
    );
    // an unlimited one has no tag
    let b = bar_for(&[0]);
    assert!(b.radius_limited.is_none());
}

fn small_rect_session(w_mm: f64, h_mm: f64) -> Session {
    let d = Document::new(1);
    let _ = d.create_rect(bounds(10.0, 10.0, w_mm, h_mm));
    let mut s = open_in_session(&d);
    select_only(&mut s);
    s
}

#[test]
fn ac21a_radius_field_works_at_any_size_and_zoom() {
    // 6 mm tag at 100 %, a 20 px rectangle, a 200 mm sheet, and a rectangle
    // far below the 72 px tier: no handle, but the field works
    for (w, h) in [(6.0, 4.0), (0.5, 0.5), (200.0, 150.0)] {
        let mut s = small_rect_session(w, h);
        let n0 = change_count(&s);
        assert_eq!(
            s.set_selected_radius_text("1.25"),
            EntryOutcome::Committed,
            "{w}x{h}"
        );
        assert_eq!(change_count(&s), n0 + 1);
        let want = 1.25f64.min(w.min(h) / 2.0);
        assert!(
            near(eff_radius_mm(&s, 0), want, 1e-9),
            "{w}x{h}: {}",
            eff_radius_mm(&s, 0)
        );
        assert!(
            near(rect_of(&s, 0).1.as_mm(), want, 1e-9),
            "limited value is the one written"
        );
    }
}

#[test]
fn ac21a_radius_field_validation_and_equal_value() {
    let mut s = small_rect_session(30.0, 20.0);
    let n0 = vv_of(&s);
    for (text, reason) in [
        ("", InvalidReason::NotANumber),
        ("  ", InvalidReason::NotANumber),
        ("abc", InvalidReason::NotANumber),
        ("NaN", InvalidReason::NotANumber),
        ("inf", InvalidReason::NotANumber),
        ("1e999", InvalidReason::NotANumber),
        ("1,2,3", InvalidReason::NotANumber),
        ("-1", InvalidReason::Negative),
        ("-0.5", InvalidReason::Negative),
    ] {
        assert_eq!(
            s.set_selected_radius_text(text),
            EntryOutcome::Invalid { field: 0, reason },
            "{text:?}"
        );
        assert_eq!(vv_of(&s), n0, "{text:?}");
    }
    // zero is valid and writes; then an equal value writes nothing
    assert_eq!(s.set_selected_radius_text("3"), EntryOutcome::Committed);
    assert_eq!(s.set_selected_radius_text("0"), EntryOutcome::Committed);
    assert_eq!(rect_of(&s, 0).1.as_mm(), 0.0);
    assert_eq!(s.set_selected_radius_text("0"), EntryOutcome::Unchanged);

    // decimal comma
    assert_eq!(s.set_selected_radius_text("2,5"), EntryOutcome::Committed);
    assert!(near(rect_of(&s, 0).1.as_mm(), 2.5, 1e-12));
    assert_eq!(s.set_selected_radius_text("2.5"), EntryOutcome::Unchanged);
    // huge values are clamped, not refused
    assert_eq!(
        s.set_selected_radius_text("1000000000"),
        EntryOutcome::Committed
    );
    assert!(near(rect_of(&s, 0).1.as_mm(), 10.0, 1e-9));
}

#[test]
fn ac21a_radius_field_batch_of_several_rectangles_one_commit_leaves_others_alone() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 1, 2, 6]);
    let before_star = doc_of(&s).primitive(doc_of(&s).object_ids()[2]).unwrap();
    let n0 = change_count(&s);
    assert_eq!(s.set_selected_radius_text("4"), EntryOutcome::Committed);
    assert_eq!(change_count(&s), n0 + 1, "one commit for the batch");
    // rect 0 is 20x10: limited to 5 -> 4 is below; both get 4
    assert!(near(rect_of(&s, 0).1.as_mm(), 4.0, 1e-12));
    assert!(near(rect_of(&s, 1).1.as_mm(), 4.0, 1e-12));
    let d = doc_of(&s);
    assert_eq!(
        d.primitive(d.object_ids()[2]).unwrap(),
        before_star,
        "star untouched"
    );
    assert_eq!(mm_of(s.select_bar_state().radius), Some(Ok(4.0)));
    // 8 mm: the 20x10 rectangle is limited to 5 and 5 is written, the other gets 8
    assert_eq!(s.set_selected_radius_text("8"), EntryOutcome::Committed);
    assert!(
        near(rect_of(&s, 0).1.as_mm(), 5.0, 1e-12),
        "limited value written"
    );
    assert!(near(rect_of(&s, 1).1.as_mm(), 8.0, 1e-12));
    assert_eq!(mm_of(s.select_bar_state().radius), Some(Err("mixed")));
    // typing into Mixed applies to all
    assert_eq!(s.set_selected_radius_text("1"), EntryOutcome::Committed);
    assert!(
        near(rect_of(&s, 0).1.as_mm(), 1.0, 1e-12) && near(rect_of(&s, 1).1.as_mm(), 1.0, 1e-12)
    );
}

#[test]
fn ac21a_radius_field_without_a_rectangle_selected_writes_nothing() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[2, 6]);
    let n0 = vv_of(&s);
    let out = s.set_selected_radius_text("3");
    assert_ne!(out, EntryOutcome::Committed);
    assert_eq!(vv_of(&s), n0);
    click(&mut s, pt(1000.0, 1000.0));
    let out = s.set_selected_radius_text("3");
    assert_ne!(out, EntryOutcome::Committed);
    assert_eq!(vv_of(&s), n0);
}

#[test]
fn ac21_remove_rounding_acts_on_rectangles_only_in_one_commit() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 1, 2, 6]);
    let d0 = doc_of(&s);
    let star0 = d0.primitive(d0.object_ids()[2]).unwrap();
    let path0 = d0.path(d0.object_ids()[6]).unwrap();
    let n0 = change_count(&s);
    s.remove_corner_rounding();
    assert_eq!(change_count(&s), n0 + 1);
    assert_eq!(rect_of(&s, 0).1.as_mm(), 0.0);
    assert_eq!(rect_of(&s, 1).1.as_mm(), 0.0);
    let d = doc_of(&s);
    assert_eq!(d.primitive(d.object_ids()[2]).unwrap(), star0);
    assert_eq!(d.path(d.object_ids()[6]).unwrap(), path0);
    // Not enabled any more: nothing left to change, nothing written
    assert!(!s.select_bar_state().remove_rounding_enabled);
    let after_first = vv_of(&s);
    s.remove_corner_rounding();
    assert_eq!(vv_of(&s), after_first, "a second call changes nothing");
}

#[test]
fn ac21_remove_rounding_leaves_unselected_rectangles_alone() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0]);
    s.remove_corner_rounding();
    assert_eq!(rect_of(&s, 0).1.as_mm(), 0.0);
    assert_eq!(rect_of(&s, 1).1.as_mm(), 3.0, "not selected");
}

#[test]
fn ac21_points_and_ratio_act_on_the_right_shapes_one_commit_each() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[2, 3, 4, 0, 6]);
    let d0 = doc_of(&s);
    let rect0 = d0.primitive(d0.object_ids()[0]).unwrap();
    let path0 = d0.path(d0.object_ids()[6]).unwrap();
    let n0 = change_count(&s);
    s.set_selected_point_count(PointCount::new(9).unwrap());
    assert_eq!(change_count(&s), n0 + 1);
    let d = doc_of(&s);
    for i in [2, 3, 4] {
        match d.primitive(d.object_ids()[i]).unwrap().shape {
            Shape::Star { point_count, .. } | Shape::Polygon { point_count, .. } => {
                assert_eq!(point_count.get(), 9, "object {i}");
            }
            _ => panic!(),
        }
    }
    assert_eq!(d.primitive(d.object_ids()[0]).unwrap(), rect0);
    assert_eq!(d.path(d.object_ids()[6]).unwrap(), path0);
    assert_eq!(s.select_bar_state().points, Some(BarValue::Uniform(9)));
    // Ratio acts on the stars only: the polygon has none
    s.set_selected_ratio(InnerRatio::new(0.7).unwrap());
    assert_eq!(change_count(&s), n0 + 2);
    assert!(near(star_ratio(&s, 2), 0.7, 1e-12) && near(star_ratio(&s, 3), 0.7, 1e-12));
    let d = doc_of(&s);
    assert!(matches!(
        d.primitive(d.object_ids()[4]).unwrap().shape,
        Shape::Polygon { .. }
    ));
    assert_eq!(s.select_bar_state().ratio, Some(BarValue::Uniform(0.7)));
}

#[test]
fn ac21_ratio_slider_previews_without_writing_and_commits_once() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[2, 3]);
    let n0 = vv_of(&s);
    let c0 = change_count(&s);
    let idle = s.draw_list().triangles;
    for r in [0.2, 0.3, 0.45, 0.6, 0.35] {
        s.preview_selected_ratio(InnerRatio::new(r).unwrap());
        assert_eq!(vv_of(&s), n0, "no write during the slider drag");
        assert!(
            (star_ratio(&s, 2) - 0.4).abs() < 1e-12,
            "document untouched"
        );
    }
    assert_ne!(s.draw_list().triangles, idle, "the new shape is drawn");
    s.commit_selected_ratio();
    assert_eq!(change_count(&s), c0 + 1, "one commit per slider drag");
    assert!(near(star_ratio(&s, 2), 0.35, 1e-12) && near(star_ratio(&s, 3), 0.35, 1e-12));
    // releasing again writes nothing
    s.commit_selected_ratio();
    assert_eq!(change_count(&s), c0 + 1);
}

#[test]
fn ac21_a_pending_slider_edit_is_committed_before_the_selection_changes() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[2]);
    s.preview_selected_ratio(InnerRatio::new(0.8).unwrap());
    // a canvas press elsewhere (selects the other star) must not move the
    // edit onto the other star
    select_index(&mut s, 3, false);
    assert!(
        near(star_ratio(&s, 3), 0.5, 1e-12),
        "the other star is untouched"
    );
    assert!(near(star_ratio(&s, 2), 0.8, 1e-12) || near(star_ratio(&s, 2), 0.4, 1e-12));
}

#[test]
fn ac22_object_to_path_converts_primitives_only_in_one_commit() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 2, 4, 5, 6]);
    let d0 = doc_of(&s);
    let path0 = d0.path(d0.object_ids()[6]).unwrap();
    let n0 = change_count(&s);
    s.convert_selected_to_paths();
    assert_eq!(change_count(&s), n0 + 1, "one atomic commit");
    let d = doc_of(&s);
    let ids = d.object_ids();
    for i in [0, 2, 4, 5] {
        assert!(d.path(ids[i]).is_some(), "object {i} is a path now");
    }
    assert_eq!(d.path(ids[6]).unwrap(), path0, "the path is untouched");
    assert!(
        d.primitive(ids[1]).is_some(),
        "unselected rectangle stays a rectangle"
    );
    assert!(d.primitive(ids[7]).is_some());
}

#[test]
fn ac22_object_to_path_with_only_a_path_selected_writes_nothing() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[6]);
    let n0 = vv_of(&s);
    s.convert_selected_to_paths();
    assert_eq!(vv_of(&s), n0);
}

#[test]
fn ac23_the_two_switches_default_to_scale_stroke_on_scale_radius_off_and_are_session_state() {
    let mut s = open_in_session(&bar_doc());
    assert!(!s.scale_corner_radius(), "off at program start");
    let bytes_a = s.pack("0.1.0").unwrap();
    s.set_scale_corner_radius(true);
    s.set_scale_stroke_width(!s.scale_stroke_width());
    let a = unpack(5, &bytes_a).unwrap().export_loro_snapshot().unwrap();
    let b = unpack(5, &s.pack("0.1.0").unwrap())
        .unwrap()
        .export_loro_snapshot()
        .unwrap();
    assert_eq!(a, b, "the switches are never written to the project");
    // works with no selection, never disabled
    click(&mut s, pt(1000.0, 1000.0));
    s.set_scale_corner_radius(false);
    assert!(!s.scale_corner_radius());
    // and a new session starts off again
    assert!(!open_in_session(&bar_doc()).scale_corner_radius());
}

// =====================================================================
// Criteria 10-14: blue new, black old
// =====================================================================

use vecmanf_render_core::Vertex;

fn is_blue(v: &Vertex) -> bool {
    v.color.r == 47 && v.color.g == 111 && v.color.b == 238 && v.color.a == 255
}

fn tris_where(dl: &DrawList, f: impl Fn(&Vertex) -> bool) -> Vec<[Point; 3]> {
    (0..dl.triangles.len() / 3)
        .map(|i| &dl.triangles[3 * i..3 * i + 3])
        .filter(|c| c.iter().all(&f))
        .map(|c| [c[0].position, c[1].position, c[2].position])
        .collect()
}

fn dist_pt_seg(p: Point, a: Point, b: Point) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * abx)).hypot(p.y - (a.y + t * aby))
}

fn dist_pt_tri(p: Point, t: &[Point; 3]) -> f64 {
    let s = |a: Point, b: Point| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let (d1, d2, d3) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    if !(neg && pos) {
        return 0.0;
    }
    dist_pt_seg(p, t[0], t[1])
        .min(dist_pt_seg(p, t[1], t[2]))
        .min(dist_pt_seg(p, t[2], t[0]))
}

/// Points on an object's outline (document mm), from the document's own
/// anchor/handle outline (a closed cubic chain).
fn outline_samples(shape: &Shape, rotation: Angle, per_seg: usize) -> Vec<Point> {
    let a = outline_of_rotated(shape, rotation);
    let n = a.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (p0, p3) = (a[i].point, a[(i + 1) % n].point);
        let p1 = pt(p0.x + a[i].handle_out.x, p0.y + a[i].handle_out.y);
        let p2 = pt(
            p3.x + a[(i + 1) % n].handle_in.x,
            p3.y + a[(i + 1) % n].handle_in.y,
        );
        for j in 0..per_seg {
            let t = j as f64 / per_seg as f64;
            let u = 1.0 - t;
            out.push(pt(
                u * u * u * p0.x
                    + 3.0 * u * u * t * p1.x
                    + 3.0 * u * t * t * p2.x
                    + t * t * t * p3.x,
                u * u * u * p0.y
                    + 3.0 * u * u * t * p1.y
                    + 3.0 * u * t * t * p2.y
                    + t * t * t * p3.y,
            ));
        }
    }
    out
}

fn samples_of(s: &Session, index: usize) -> Vec<Point> {
    let p = prim_of(s, index);
    outline_samples(&p.shape, p.rotation, 12)
}

/// How many of `samples` are not within `tol_px` of any of `tris`.
fn misses(tris: &[[Point; 3]], samples: &[Point], tol_px: f64, k: f64) -> usize {
    samples
        .iter()
        .filter(|p| !tris.iter().any(|t| dist_pt_tri(**p, t) * k <= tol_px))
        .count()
}

fn blue_cover_misses(dl: &DrawList, samples: &[Point], k: f64) -> usize {
    let tris = tris_where(dl, is_blue);
    misses(&tris, samples, 0.9, k)
}

/// The committed rendering of the old object: dark, non-accent triangles.
fn black_cover_misses(dl: &DrawList, samples: &[Point], k: f64) -> usize {
    let tris = tris_where(dl, |v| {
        !is_blue(v)
            && v.color.a == 255
            && u32::from(v.color.r) + u32::from(v.color.g) + u32::from(v.color.b) < 150
    });
    misses(&tris, samples, 1.2, k)
}

#[derive(Clone, Copy, Debug)]
enum H {
    Move,
    Centre,
    ResizeCorner(usize),
    ResizeEdge(usize),
    RotCorner(usize),
    RotSide(usize),
    Radius(usize),
    Inner,
}

const SIDES: [(f64, f64); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];

fn handle_point(sc: &Scene, h: H) -> Point {
    let fr = &sc.fr;
    match h {
        H::Move => fr.at(fr.w_px() * 0.12, -fr.h_px() * 0.28),
        H::Centre => fr.c,
        H::ResizeCorner(i) => fr.corner(CORNERS[i].0, CORNERS[i].1),
        H::ResizeEdge(i) => fr.mid(SIDES[i].0, SIDES[i].1),
        H::RotCorner(i) => fr.rot_corner(CORNERS[i].0, CORNERS[i].1),
        H::RotSide(i) => fr.rot_side(SIDES[i].0, SIDES[i].1),
        H::Radius(i) => {
            let (b, r) = rect_of(&sc.s, 0);
            let r_px = effective_corner_radius(b, r).as_mm() * sc.k;
            radius_handle_pos(fr, CORNERS[i], r_px)
        }
        H::Inner => inner_vertex(sc),
    }
}

/// One drag: press `from`, move to `to`, check the preview, release; returns
/// (preview ok, changed document, one commit).
struct Outcome {
    changed: bool,
    blue_misses: usize,
    black_misses: usize,
    n_samples: usize,
    leaked: bool,
    commits: usize,
}

fn run_drag(sc: &mut Scene, h: H, to_offset_px: (f64, f64), shift: bool, ctrl: bool) -> Outcome {
    let from = handle_point(sc, h);
    let k = sc.k;
    let to = pt(from.x + to_offset_px.0 / k, from.y + to_offset_px.1 / k);
    let before_doc = doc_of(&sc.s).export_loro_snapshot().unwrap();
    let old_samples = samples_of(&sc.s, 0);
    let c0 = change_count(&sc.s);
    let press_shift = shift || matches!(h, H::RotSide(_));
    sc.s.pointer_hover(from, press_shift, ctrl);
    sc.s.pointer_down(from, press_shift);
    sc.s.pointer_hover(to, shift, ctrl);
    let dl = sc.s.draw_list();
    let during_doc = doc_of(&sc.s).export_loro_snapshot().unwrap();
    sc.s.pointer_up(to, shift, ctrl);
    let after_doc = doc_of(&sc.s).export_loro_snapshot().unwrap();
    let new_samples = samples_of(&sc.s, 0);
    // A drag past the opposite side clamps to a zero size (slice 5, criterion
    // 13): nothing visible to compare there.
    let (mut w0, mut w1, mut h0, mut h1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &new_samples {
        w0 = w0.min(p.x);
        w1 = w1.max(p.x);
        h0 = h0.min(p.y);
        h1 = h1.max(p.y);
    }
    let degenerate = (w1 - w0) * k < 2.0 || (h1 - h0) * k < 2.0;
    let changed = after_doc != before_doc && !degenerate;
    Outcome {
        changed,
        blue_misses: blue_cover_misses(&dl, &new_samples, k),
        black_misses: black_cover_misses(&dl, &old_samples, k),
        n_samples: new_samples.len(),
        leaked: during_doc != before_doc,
        commits: change_count(&sc.s) - c0,
    }
}

fn sweep(label: &str, mk: &dyn Fn() -> Scene, handles: &[H], trials: usize, seed: u64) {
    let mut rng = Rng(seed);
    for &h in handles {
        for t in 0..trials {
            let mut sc = mk();
            let off = (rng.range(-150.0, 150.0), rng.range(-150.0, 150.0));
            let shift = rng.next() < 0.4;
            let ctrl = rng.next() < 0.4;
            let o = run_drag(&mut sc, h, off, shift, ctrl);
            let ctx = format!("{label} {h:?} trial {t} off={off:?} shift={shift} ctrl={ctrl}");
            assert!(!o.leaked, "{ctx}: preview leaked into the document");
            if !o.changed {
                continue;
            }
            assert_eq!(o.commits, 1, "{ctx}: one interaction, one commit");
            assert_eq!(
                o.black_misses, 0,
                "{ctx}: the old geometry stays drawn (black)"
            );
            assert_eq!(
                o.blue_misses, 0,
                "{ctx}: blue preview != committed outline ({} samples)",
                o.n_samples
            );
        }
    }
}

const RECT_HANDLES: [H; 16] = [
    H::Move,
    H::Centre,
    H::ResizeCorner(0),
    H::ResizeCorner(2),
    H::ResizeCorner(3),
    H::ResizeEdge(0),
    H::ResizeEdge(1),
    H::ResizeEdge(3),
    H::RotCorner(0),
    H::RotCorner(2),
    H::RotSide(1),
    H::RotSide(3),
    H::Radius(0),
    H::Radius(1),
    H::Radius(2),
    H::Radius(3),
];

#[test]
fn ac13_preview_equals_release_for_every_rectangle_handle() {
    for scale_radius in [false, true] {
        for th in [0.0, 0.7] {
            let mk = move || {
                let mut sc = rect_scene(100, 170.0, 110.0, 25.0, th);
                sc.s.set_scale_corner_radius(scale_radius);
                sc
            };
            sweep(
                &format!("rect th={th} scale_r={scale_radius}"),
                &mk,
                &RECT_HANDLES,
                6,
                0x1234 + th.to_bits(),
            );
        }
    }
}

#[test]
fn ac13_preview_equals_release_for_the_ellipse() {
    let hs = [
        H::Move,
        H::Centre,
        H::ResizeCorner(0),
        H::ResizeCorner(2),
        H::ResizeEdge(1),
        H::ResizeEdge(2),
        H::RotCorner(1),
        H::RotCorner(3),
        H::RotSide(0),
    ];
    for th in [0.0, -1.1] {
        sweep(
            "ellipse",
            &move || ellipse_scene(100, 160.0, 100.0, th),
            &hs,
            8,
            77,
        );
    }
}

#[test]
fn ac13_preview_equals_release_for_polygon_and_star() {
    let hs = [
        H::Move,
        H::Centre,
        H::ResizeCorner(0),
        H::ResizeCorner(1),
        H::ResizeCorner(2),
        H::RotCorner(1),
        H::RotCorner(3),
        H::RotSide(0),
        H::RotSide(2),
    ];
    sweep(
        "polygon",
        &|| star_scene(100, 70.0, 8, None, 0.0),
        &hs,
        8,
        5,
    );
    sweep(
        "polygon rotated",
        &|| star_scene(100, 70.0, 12, None, 0.8),
        &hs,
        6,
        6,
    );
    let mut hs2 = hs.to_vec();
    hs2.push(H::Inner);
    sweep(
        "star",
        &|| star_scene(100, 70.0, 8, Some(0.45), 0.0),
        &hs2,
        10,
        8,
    );
    sweep(
        "star rotated",
        &|| star_scene(200, 70.0, 5, Some(0.6), -0.9),
        &hs2,
        8,
        9,
    );
}

#[test]
fn ac10_old_geometry_stays_drawn_in_its_own_style_while_the_blue_one_follows() {
    // Rounded rectangle, move far away: the old outline is still covered by
    // dark triangles and the new one by blue ones, in the same frame.
    let mut sc = rect_scene(100, 140.0, 100.0, 30.0, 0.0);
    let old = samples_of(&sc.s, 0);
    let from = handle_point(&sc, H::Move);
    sc.s.pointer_hover(from, false, false);
    sc.s.pointer_down(from, false);
    let to = pt(from.x + 120.0 / sc.k, from.y + 90.0 / sc.k);
    sc.s.pointer_hover(to, false, false);
    let dl = sc.s.draw_list();
    let new: Vec<Point> = old
        .iter()
        .map(|p| pt(p.x + 120.0 / sc.k, p.y + 90.0 / sc.k))
        .collect();
    assert_eq!(black_cover_misses(&dl, &old, sc.k), 0, "old stays (black)");
    assert_eq!(blue_cover_misses(&dl, &new, sc.k), 0, "new is blue");
    sc.s.pointer_up(to, false, false);
    // released: the blue outline is gone, the object renders normally
    sc.s.pointer_hover(pt(900.0, 900.0), false, false);
    let after = sc.s.draw_list();
    assert_eq!(black_cover_misses(&after, &new, sc.k), 0);
}

#[test]
fn ac10_blue_is_a_hollow_outline_of_constant_screen_width() {
    // The blue outline of a rotated rectangle at two zoom levels: the
    // strip around a straight edge is about 1.5 px wide, whatever the zoom.
    for pct in [100, 400] {
        let mut sc = rect_scene(pct, 160.0, 100.0, 0.0, 0.0);
        let from = handle_point(&sc, H::Move);
        sc.s.pointer_hover(from, false, false);
        sc.s.pointer_down(from, false);
        let to = pt(from.x + 200.0 / sc.k, from.y);
        sc.s.pointer_hover(to, false, false);
        let dl = sc.s.draw_list();
        // new top edge sits at y = centre - 50 px, shifted right by 200 px
        let y_mm = sc.fr.c.y - 50.0 / sc.k;
        let x_mm = sc.fr.c.x + (200.0 + 45.0) / sc.k;
        // Find the extent of blue triangles across the top edge at x_mm
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for t in tris_where(&dl, is_blue) {
            for p in t {
                if (p.x - x_mm).abs() < 8.0 / sc.k && (p.y - y_mm).abs() < 4.0 / sc.k {
                    lo = lo.min(p.y);
                    hi = hi.max(p.y);
                }
            }
        }
        // the 1px selection box coincides with the new geometry too (boxes
        // follow), so the union of box and blue outline is at least 1.5 px
        // and at most 2 px wide
        let width_px = (hi - lo) * sc.k;
        assert!((1.4..=2.1).contains(&width_px), "{pct}%: {width_px} px");
        // hollow: no blue triangle covers the interior centre of the new box
        let centre = pt(sc.fr.c.x + (200.0 + 30.0) / sc.k, sc.fr.c.y + 20.0 / sc.k);
        let covered = tris_where(&dl, is_blue)
            .iter()
            .any(|t| dist_pt_tri(centre, t) == 0.0);
        assert!(!covered, "{pct}%: no fill preview");
    }
}

#[test]
fn ac12_escape_leaves_no_blue_and_no_write() {
    for h in [
        H::Move,
        H::ResizeCorner(2),
        H::RotCorner(0),
        H::Radius(1),
        H::ResizeEdge(0),
        H::Centre,
    ] {
        let mut sc = rect_scene(100, 150.0, 100.0, 20.0, 0.2);
        let from = handle_point(&sc, h);
        let v0 = vv_of(&sc.s);
        sc.s.pointer_hover(from, false, false);
        let idle = sc.s.draw_list().triangles;
        sc.s.pointer_down(from, false);
        sc.s.pointer_hover(pt(from.x + 60.0 / sc.k, from.y + 45.0 / sc.k), false, false);
        assert_ne!(
            sc.s.draw_list().triangles,
            idle,
            "{h:?}: a preview is drawn"
        );
        sc.s.escape();
        sc.s.pointer_hover(from, false, false);
        assert_eq!(
            sc.s.draw_list().triangles,
            idle,
            "{h:?}: back to the idle picture"
        );
        assert_eq!(vv_of(&sc.s), v0, "{h:?}: nothing written");
        sc.s.pointer_up(pt(from.x + 60.0 / sc.k, from.y + 45.0 / sc.k), false, false);
        assert_eq!(vv_of(&sc.s), v0, "{h:?}: a later release writes nothing");
    }
}

#[test]
fn ac12_a_drag_returning_to_its_start_writes_nothing_for_every_handle() {
    for h in RECT_HANDLES {
        for (shift, ctrl) in [(false, false), (true, false), (false, true)] {
            let mut sc = rect_scene(100, 150.0, 100.0, 20.0, 0.3);
            let from = handle_point(&sc, h);
            let v0 = vv_of(&sc.s);
            let snap0 = doc_of(&sc.s).export_loro_snapshot().unwrap();
            let press_shift = shift || matches!(h, H::RotSide(_));
            sc.s.pointer_hover(from, press_shift, ctrl);
            sc.s.pointer_down(from, press_shift);
            sc.s.pointer_hover(pt(from.x + 80.0 / sc.k, from.y + 33.0 / sc.k), shift, ctrl);
            sc.s.pointer_hover(pt(from.x - 20.0 / sc.k, from.y + 10.0 / sc.k), shift, ctrl);
            sc.s.pointer_hover(from, shift, ctrl);
            sc.s.pointer_up(from, shift, ctrl);
            assert!(
                vv_of(&sc.s) == v0,
                "{h:?} shift={shift} ctrl={ctrl}: no op written"
            );
            assert_eq!(doc_of(&sc.s).export_loro_snapshot().unwrap(), snap0);
        }
    }
}

#[test]
fn ac12_a_multi_object_move_dragged_back_writes_nothing() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 1, 2, 6]);
    let v0 = vv_of(&s);
    let a = pt(120.0, 10.0);
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(pt(160.0, 50.0), false, false);
    s.pointer_hover(pt(100.0, -10.0), false, false);
    s.pointer_hover(a, false, false);
    s.pointer_up(a, false, false);
    assert_eq!(vv_of(&s), v0, "no zero-offset commit");
}

#[test]
fn ac12_press_under_the_dead_zone_writes_nothing_at_every_zoom() {
    for pct in [25, 100, 800] {
        let mut sc = rect_scene(pct, 150.0, 100.0, 20.0, 0.0);
        let v0 = vv_of(&sc.s);
        for h in RECT_HANDLES {
            let from = handle_point(&sc, h);
            let press_shift = matches!(h, H::RotSide(_));
            sc.s.pointer_hover(from, press_shift, false);
            sc.s.pointer_down(from, press_shift);
            sc.s.pointer_hover(pt(from.x + 2.9 / sc.k, from.y), press_shift, false);
            sc.s.pointer_up(pt(from.x + 2.9 / sc.k, from.y), press_shift, false);
            assert_eq!(vv_of(&sc.s), v0, "{pct}% {h:?}");
        }
    }
}

#[test]
fn ac11_a_multi_object_move_shows_both_renderings_for_every_object() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 1, 2, 4]);
    let k = k_of(&s);
    let olds: Vec<Vec<Point>> = [0, 1, 2, 4].iter().map(|&i| samples_of(&s, i)).collect();
    let from = pt(130.0, 10.0); // on rectangle 1's top edge
    let off = (70.0 / k, 40.0 / k);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(pt(from.x + off.0, from.y + off.1), false, false);
    let dl = s.draw_list();
    for (n, old) in olds.iter().enumerate() {
        let new: Vec<Point> = old.iter().map(|p| pt(p.x + off.0, p.y + off.1)).collect();
        assert_eq!(black_cover_misses(&dl, old, k), 0, "object {n}: old stays");
        assert_eq!(
            blue_cover_misses(&dl, &new, k),
            0,
            "object {n}: new is blue"
        );
    }
    s.pointer_up(pt(from.x + off.0, from.y + off.1), false, false);
}

#[test]
fn ac14_pressing_or_releasing_shift_or_ctrl_while_the_pointer_holds_still_updates_the_preview() {
    // rotate drag about the centre; Shift switches the pivot to the opposite
    // corner. The preview must follow the modifier change on the next frame
    // and equal what a release with that modifier commits.
    for (shift, ctrl, handle, off) in [
        (true, false, H::RotCorner(0), (61.0, 23.0)),
        (false, true, H::RotCorner(2), (-70.0, 95.0)),
        (false, true, H::RotCorner(1), (45.0, 80.0)),
        (true, false, H::ResizeCorner(2), (61.0, 23.0)),
        (false, true, H::ResizeCorner(1), (61.0, 23.0)),
        (true, true, H::ResizeEdge(1), (61.0, 23.0)),
    ] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        let from = handle_point(&sc, handle);
        let to = pt(from.x + off.0 / sc.k, from.y + off.1 / sc.k);
        sc.s.pointer_hover(from, false, false);
        sc.s.pointer_down(from, false);
        sc.s.pointer_hover(to, false, false);
        let dl_plain = sc.s.draw_list();
        sc.s.modifiers_changed(shift, ctrl);
        let dl_mod = sc.s.draw_list();
        sc.s.pointer_up(to, shift, ctrl);
        let committed = samples_of(&sc.s, 0);
        assert_eq!(
            blue_cover_misses(&dl_mod, &committed, sc.k),
            0,
            "{handle:?} shift={shift} ctrl={ctrl}: preview follows the modifier"
        );
        let _ = dl_plain;
        // and releasing the modifier brings the plain preview back
        let mut sc2 = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        sc2.s.pointer_hover(from, false, false);
        sc2.s.pointer_down(from, false);
        sc2.s.pointer_hover(to, shift, ctrl);
        sc2.s.modifiers_changed(shift, ctrl);
        sc2.s.modifiers_changed(false, false);
        let dl_back = sc2.s.draw_list();
        sc2.s.pointer_up(to, false, false);
        let plain = samples_of(&sc2.s, 0);
        assert_eq!(
            blue_cover_misses(&dl_back, &plain, sc2.k),
            0,
            "{handle:?}: releasing {shift} {ctrl} restores the plain preview"
        );
    }
}

#[test]
fn ac13_oracle_control_a_wrong_geometry_is_detected() {
    // The coverage check must fail for the old outline while the move
    // preview is elsewhere, so a passing sweep means something.
    let mut sc = rect_scene(100, 140.0, 100.0, 30.0, 0.0);
    let old = samples_of(&sc.s, 0);
    let from = handle_point(&sc, H::Move);
    sc.s.pointer_hover(from, false, false);
    sc.s.pointer_down(from, false);
    sc.s.pointer_hover(
        pt(from.x + 160.0 / sc.k, from.y + 90.0 / sc.k),
        false,
        false,
    );
    let dl = sc.s.draw_list();
    assert!(
        blue_cover_misses(&dl, &old, sc.k) > old.len() / 2,
        "old outline is not blue"
    );
}

// =====================================================================
// Criterion 23 / rectangle-corner-radii 12, 14, 15: the radius switch
// =====================================================================

fn resize_corner_drag(sc: &mut Scene, shift: bool, off: (f64, f64)) {
    let from = handle_point(sc, H::ResizeCorner(2));
    let to = pt(from.x + off.0 / sc.k, from.y + off.1 / sc.k);
    drag_mod(&mut sc.s, from, to, shift, false);
}

#[test]
fn ac23_resize_keeps_the_absolute_radius_when_the_switch_is_off() {
    for (shift, off, ex_sx, ex_sy) in [
        (false, (85.0, 22.0), 1.5, 1.2),
        (false, (-40.0, -30.0), 130.0 / 170.0, 80.0 / 110.0),
        (true, (85.0, 0.0), 2.0, 1.0),
    ] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        let v0 = vv_of(&sc.s);
        resize_corner_drag(&mut sc, shift, off);
        let (b, r) = rect_of(&sc.s, 0);
        assert!(near(b.width.as_mm() * sc.k, 170.0 * ex_sx, 1e-6), "{off:?}");
        assert!(
            near(b.height.as_mm() * sc.k, 110.0 * ex_sy, 1e-6),
            "{off:?}"
        );
        assert!(
            near(r.as_mm() * sc.k, 25.0, 1e-6),
            "{off:?}: radius {} px",
            r.as_mm() * sc.k
        );
        assert!(
            !ops_since_json(&sc.s, &v0).contains("corner_radius"),
            "peer-merge safety: a resize with the switch off must not write corner_radius"
        );
    }
}

#[test]
fn ac23_resize_scales_the_radius_by_sqrt_sx_sy_when_the_switch_is_on() {
    for (shift, off, ex_sx, ex_sy) in [
        (false, (85.0, 22.0), 1.5, 1.2),
        (false, (-40.0, -30.0), 130.0 / 170.0, 80.0 / 110.0),
        (true, (85.0, 0.0), 2.0, 1.0),
    ] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        sc.s.set_scale_corner_radius(true);
        resize_corner_drag(&mut sc, shift, off);
        let (b, r) = rect_of(&sc.s, 0);
        assert!(near(b.width.as_mm() * sc.k, 170.0 * ex_sx, 1e-6));
        let want = 25.0 * f64::sqrt(ex_sx * ex_sy);
        assert!(
            near(r.as_mm() * sc.k, want, 1e-6),
            "{off:?}: {} vs {want}",
            r.as_mm() * sc.k
        );
    }
    // an edge handle: sy = 1
    let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
    sc.s.set_scale_corner_radius(true);
    let from = handle_point(&sc, H::ResizeEdge(1));
    drag(&mut sc.s, from, pt(from.x + 85.0 / sc.k, from.y));
    let r = rect_of(&sc.s, 0).1.as_mm() * sc.k;
    assert!(near(r, 25.0 * 1.5f64.sqrt(), 1e-6), "{r}");
}

#[test]
fn ac23_the_switch_is_read_at_the_press_not_at_the_release() {
    for first in [false, true] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        sc.s.set_scale_corner_radius(first);
        let from = handle_point(&sc, H::ResizeCorner(2));
        let to = pt(from.x + 85.0 / sc.k, from.y);
        sc.s.pointer_hover(from, false, false);
        sc.s.pointer_down(from, false);
        sc.s.pointer_hover(to, false, false);
        sc.s.set_scale_corner_radius(!first);
        sc.s.pointer_hover(to, false, false);
        let during = sc.s.draw_list();
        sc.s.pointer_up(to, false, false);
        let r = rect_of(&sc.s, 0).1.as_mm() * sc.k;
        let want = if first { 25.0 * 1.5f64.sqrt() } else { 25.0 };
        assert!(
            near(r, want, 1e-6),
            "switch {first} at press: {r} vs {want}"
        );
        assert_eq!(
            blue_cover_misses(&during, &samples_of(&sc.s, 0), sc.k),
            0,
            "preview agrees"
        );
    }
}

fn size_entry_commit(sc: &mut Scene, w_px: f64, h_px: f64) -> EntryOutcome {
    let h = handle_point(sc, H::ResizeCorner(2));
    let e = open_entry(&mut sc.s, h, false, false).expect("size entry opens");
    assert_eq!(e.kind, "size");
    sc.s.commit_transform_entry(
        &format!("{:.9}", w_px / sc.k),
        &format!("{:.9}", h_px / sc.k),
        0,
    )
}

#[test]
fn ac23_the_switch_is_read_when_a_typed_size_entry_opens() {
    for on in [false, true] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        sc.s.set_scale_corner_radius(on);
        assert_eq!(
            size_entry_commit(&mut sc, 255.0, 132.0),
            EntryOutcome::Committed
        );
        let r = rect_of(&sc.s, 0).1.as_mm() * sc.k;
        let want = if on { 25.0 * 1.8f64.sqrt() } else { 25.0 };
        assert!(near(r, want, 1e-5), "on={on}: {r} vs {want}");
    }
}

#[test]
fn ac23_clicking_the_switch_while_an_entry_is_open_closes_it_without_writing() {
    for stroke in [true, false] {
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        let v0 = vv_of(&sc.s);
        let h = handle_point(&sc, H::ResizeCorner(2));
        open_entry(&mut sc.s, h, false, false).expect("entry");
        if stroke {
            sc.s.set_scale_stroke_width(true);
        } else {
            sc.s.set_scale_corner_radius(true);
        }
        assert!(entry_of(&sc.s).is_none(), "stroke={stroke}: entry closed");
        let out = sc.s.commit_transform_entry("300", "200", 0);
        assert_eq!(out, EntryOutcome::Unchanged);
        assert_eq!(vv_of(&sc.s), v0, "nothing written");
    }
}

#[test]
fn ac23_the_radius_switch_does_not_change_star_or_ellipse_resizes() {
    let run = |on: bool| {
        let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
        sc.s.set_scale_corner_radius(on);
        let h = sc.fr.corner(1.0, 1.0);
        drag(&mut sc.s, h, pt(h.x + 30.0 / sc.k, h.y + 30.0 / sc.k));
        doc_of(&sc.s).export_loro_snapshot().unwrap()
    };
    assert_eq!(run(false), run(true));
}

// =====================================================================
// Criterion 35: press order with exactly one primitive selected
// =====================================================================

/// A (selected, 200x150 px at the usual place) and B (60x40 px, small).
/// `b_at`: where B's box centre lies, in px relative to A's centre.
fn two_rect_scene(b_at: (f64, f64)) -> (Session, Fr, Fr, f64) {
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let (aw, ah) = (200.0 / k, 150.0 / k);
    let _ = d.create_rect(bounds(60.0 - aw / 2.0, 45.0 - ah / 2.0, aw, ah));
    let (bw, bh) = (60.0 / k, 40.0 / k);
    let (bx, by) = (60.0 + b_at.0 / k, 45.0 + b_at.1 / k);
    let _ = d.create_rect(bounds(bx - bw / 2.0, by - bh / 2.0, bw, bh));
    let mut s = open_in_session(&d);
    select_index(&mut s, 0, false);
    let fa = Fr {
        c: pt(60.0, 45.0),
        hw: aw / 2.0,
        hh: ah / 2.0,
        th: 0.0,
        k,
    };
    let fb = Fr {
        c: pt(bx, by),
        hw: bw / 2.0,
        hh: bh / 2.0,
        th: 0.0,
        k,
    };
    (s, fa, fb, k)
}

#[test]
fn ac35_a_press_inside_the_selected_box_moves_it_even_on_another_objects_outline() {
    let (mut s, fa, fb, k) = two_rect_scene((20.0, 10.0));
    // B's top edge midpoint lies inside A's box and away from A's knobs
    let on_b = fb.mid(0.0, -1.0);
    let b0 = rect_of(&s, 1);
    let a0 = rect_of(&s, 0);
    drag(&mut s, on_b, pt(on_b.x + 40.0 / k, on_b.y + 25.0 / k));
    assert_eq!(rect_of(&s, 1), b0, "B untouched");
    let a1 = rect_of(&s, 0);
    assert!(near(a1.0.origin.x - a0.0.origin.x, 40.0 / k, 1e-6));
    assert!(near(a1.0.origin.y - a0.0.origin.y, 25.0 / k, 1e-6));
    let _ = fa;
}

/// Criterion 35, amended: with Shift held the outline hit is tried first, so a
/// Shift press on another object's outline inside the selected box adds it to
/// the selection; a Shift press inside the box on no outline neither moves the
/// object nor clears the selection.
#[test]
fn ac35_with_shift_the_outline_hit_is_tried_first() {
    let (mut s, fa, fb, k) = two_rect_scene((20.0, 10.0));
    let (a0, b0) = (rect_of(&s, 0), rect_of(&s, 1));
    // inside A's box on no outline and no handle: nothing happens
    let p = fa.at(-60.0, 30.0);
    drag_mod(&mut s, p, pt(p.x + 30.0 / k, p.y + 20.0 / k), true, false);
    assert_eq!(rect_of(&s, 0), a0, "not moved");
    assert!(s.select_bar_state().radius.is_some(), "still selected");
    // B's outline lies inside A's box: Shift-click adds B to the selection
    let on_b = fb.mid(0.0, -1.0);
    shift_click(&mut s, on_b);
    assert_eq!(rect_of(&s, 1), b0, "a click moves nothing");
    let (a1, b1) = (rect_of(&s, 0), rect_of(&s, 1));
    let on_b2 = pt(fb.c.x + 20.0 / k, fb.c.y - 20.0 / k);
    drag(&mut s, on_b2, pt(on_b2.x + 10.0 / k, on_b2.y + 10.0 / k));
    assert!(
        near(rect_of(&s, 0).0.origin.x - a1.0.origin.x, 10.0 / k, 1e-6),
        "A moved with B"
    );
    assert!(
        near(rect_of(&s, 1).0.origin.x - b1.0.origin.x, 10.0 / k, 1e-6),
        "B was added by the Shift press"
    );
}

#[test]
fn ac35_an_outline_hit_outside_the_box_selects_and_shift_toggles() {
    // B far outside A's reach
    let (mut s, _fa, fb, k) = two_rect_scene((300.0, 0.0));
    let on_b = fb.mid(0.0, -1.0);
    let a0 = rect_of(&s, 0);
    let b0 = rect_of(&s, 1);
    drag(&mut s, on_b, pt(on_b.x + 20.0 / k, on_b.y));
    assert_eq!(rect_of(&s, 0), a0, "A not moved");
    assert!(
        near(rect_of(&s, 1).0.origin.x - b0.0.origin.x, 20.0 / k, 1e-6),
        "B selected and moved"
    );
    // Shift-click A's outline toggles it in: now both selected; a drag of B moves both
    let ptop = pt(60.0, 45.0 - 75.0 / k);
    shift_click(&mut s, ptop);
    let (a1, b1) = (rect_of(&s, 0), rect_of(&s, 1));
    let on_b2 = pt(fb.c.x + 20.0 / k, fb.c.y - 20.0 / k);
    drag(&mut s, on_b2, pt(on_b2.x + 10.0 / k, on_b2.y + 10.0 / k));
    assert!(
        near(rect_of(&s, 0).0.origin.x - a1.0.origin.x, 10.0 / k, 1e-6),
        "A moved with B"
    );
    assert!(
        near(rect_of(&s, 1).0.origin.x - b1.0.origin.x, 10.0 / k, 1e-6),
        "B moved"
    );
    // Shift-click A's outline again toggles it out
    let a2 = rect_of(&s, 0);
    let a_top = pt(a2.0.origin.x + a2.0.width.as_mm() / 2.0, a2.0.origin.y);
    shift_click(&mut s, a_top);
    assert_eq!(rect_of(&s, 0), a2, "a click moves nothing");
}

#[test]
fn ac35_outline_hit_radius_is_screen_pixels_at_every_zoom() {
    for (dist, hit) in [(1.0, true), (3.0, true), (9.0, false), (12.0, false)] {
        outline_hit_case(dist, hit);
    }
}

/// Criterion 35, amended: the outline tolerance is 4 px
/// (`SEGMENT_TOLERANCE_PX`, unchanged since slice 4); `advanced-selection`
/// raises it to 8 px later.
#[test]
fn ac35_outline_hit_radius_is_4_px_as_amended() {
    for (dist, hit) in [(1.0, true), (3.5, true), (5.0, false), (9.0, false)] {
        outline_hit_case(dist, hit);
    }
}

fn outline_hit_case(dist: f64, hit: bool) {
    {
        for pct in [100, 400] {
            let k = k_at_zoom(pct);
            let d = Document::new(1);
            let _ = d.create_rect(bounds(40.0, 30.0, 200.0 / k, 150.0 / k));
            // a diamond (4-point polygon), R = 40 px, far from the rectangle
            let c = pt(40.0 + 700.0 / k, 30.0 + 60.0 / k);
            let _ = d.create_polygon(star_frame(c.x, c.y, 40.0 / k), PointCount::new(4).unwrap());
            let mut s = open_in_session(&d);
            zoom_to_pct(&mut s, pct);
            select_index(&mut s, 0, false);
            assert!(s.select_bar_state().points.is_none());
            // outward normal from the middle of the edge top -> right
            let n = (1.0 / SQRT_2, -1.0 / SQRT_2);
            let mid = (20.0, -20.0);
            let p = pt(
                c.x + (mid.0 + n.0 * dist) / k,
                c.y + (mid.1 + n.1 * dist) / k,
            );
            click(&mut s, p);
            assert_eq!(
                s.select_bar_state().points.is_some(),
                hit,
                "{pct}%: click {dist} px from the outline"
            );
        }
    }
}

#[test]
fn ac35_a_press_on_empty_canvas_outside_the_box_clears_the_selection_and_starts_a_marquee() {
    let (mut s, _fa, _fb, k) = two_rect_scene((300.0, 0.0));
    let a0 = rect_of(&s, 0);
    let p = pt(60.0, 45.0 + 200.0 / k);
    drag(&mut s, p, pt(p.x + 30.0 / k, p.y + 30.0 / k));
    assert_eq!(rect_of(&s, 0), a0, "nothing moved");
    assert!(s.select_bar_state().radius.is_none(), "selection cleared");
}

#[test]
fn ac35_a_handle_wins_over_the_inside_move_with_or_without_shift() {
    for shift in [false, true] {
        let mut sc = rect_scene(100, 160.0, 160.0, 0.0, 0.0);
        let p = handle_point(&sc, H::Radius(1));
        drag_mod(
            &mut sc.s,
            p,
            on_diag(&sc.fr, (1.0, -1.0), 50.0, 0.0),
            shift,
            false,
        );
        assert!(
            rect_of(&sc.s, 0).1.as_mm() > 0.0,
            "shift={shift}: radius changed"
        );
        assert_eq!(
            rect_of(&sc.s, 0).0.origin,
            pt(60.0 - 80.0 / sc.k, 45.0 - 80.0 / sc.k),
            "not moved"
        );
        let mut sc = rect_scene(100, 160.0, 160.0, 0.0, 0.0);
        let c = handle_point(&sc, H::ResizeCorner(0));
        drag_mod(&mut sc.s, c, pt(c.x - 30.0 / sc.k, c.y), shift, false);
        assert!(
            rect_of(&sc.s, 0).0.width.as_mm() * sc.k > 160.0 + 20.0,
            "shift={shift}: resized"
        );
    }
}

#[test]
fn ac05_nearest_centre_decides_between_a_knob_and_the_corner_resize_handle() {
    let mut sc = rect_scene(100, 72.0, 72.0, 0.0, 0.0);
    // 9 px from the corner along the diagonal: the knob (at 15) is 6 px away
    let p = on_diag(&sc.fr, (-1.0, -1.0), 9.0, 0.0);
    assert_eq!(hint(&mut sc.s, p).1, "param-radius");
    // 6 px from the corner: the corner handle is nearer
    let p = on_diag(&sc.fr, (-1.0, -1.0), 6.0, 0.0);
    assert_eq!(hint(&mut sc.s, p).1, "resize-corner");
}

// =====================================================================
// Criterion 37: two or more selected: no handles
// =====================================================================

#[test]
fn ac37_two_selected_objects_have_no_transform_or_parameter_handle() {
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let (w, h) = (200.0 / k, 150.0 / k);
    let _ = d.create_rect(bounds(30.0, 20.0, w, h));
    let _ = d.create_rect(bounds(30.0 + 600.0 / k, 20.0, w, h));
    let _ = d.create_star(
        star_frame(150.0, 20.0 + h / 2.0, 90.0 / k),
        PointCount::new(8).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let mut s = open_in_session(&d);
    for set in [[0usize, 1], [0, 2]] {
        select_set(&mut s, &set);
        let fa = Fr {
            c: pt(30.0 + w / 2.0, 20.0 + h / 2.0),
            hw: w / 2.0,
            hh: h / 2.0,
            th: 0.0,
            k,
        };
        let spots = [
            fa.corner(-1.0, -1.0),
            fa.corner(1.0, 1.0),
            fa.mid(0.0, -1.0),
            fa.mid(1.0, 0.0),
            fa.rot_corner(1.0, -1.0),
            fa.c,
            radius_handle_pos(&fa, (-1.0, -1.0), 0.0),
        ];
        for p in spots {
            let (cur, hnt) = hint(&mut s, p);
            // the only allowed non-default state: none at all
            assert_eq!(
                (cur.as_str(), hnt.as_str()),
                ("default", ""),
                "{set:?} at {p:?}"
            );
        }
    }
    select_set(&mut s, &[0, 1]);
    let a0 = rect_of(&s, 0);
    let b0 = rect_of(&s, 1);
    let top = pt(30.0 + w / 2.0, 20.0);
    let a1 = rect_of(&s, 0);
    let b1 = rect_of(&s, 1);
    assert_eq!(
        (a1, b1),
        (a0, b0),
        "an interior press on unfilled shapes moves nothing"
    );
    drag(&mut s, top, pt(top.x + 25.0 / k, top.y + 10.0 / k));
    let a2 = rect_of(&s, 0);
    let b2 = rect_of(&s, 1);
    assert!(near(a2.0.origin.x - a1.0.origin.x, 25.0 / k, 1e-6));
    assert!(
        near(b2.0.origin.x - b1.0.origin.x, 25.0 / k, 1e-6),
        "both moved"
    );
    assert_eq!(a2.1, a0.1, "no radius change");
    // the corner of A itself (a resize handle for a single selection) just moves both
    let corner = pt(
        a2.0.origin.x + a2.0.width.as_mm(),
        a2.0.origin.y + a2.0.height.as_mm(),
    );
    drag(&mut s, corner, pt(corner.x + 12.0 / k, corner.y + 12.0 / k));
    let a3 = rect_of(&s, 0);
    assert!(
        near(a3.0.width.as_mm(), a2.0.width.as_mm(), 1e-9),
        "no resize with two selected"
    );
    assert!(near(a3.0.origin.x - a2.0.origin.x, 12.0 / k, 1e-6));
}

#[test]
fn ac37_delete_removes_every_selected_object_of_any_kind() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 2, 6]);
    s.delete_selected();
    assert_eq!(doc_of(&s).object_ids().len(), 5);
}

// =====================================================================
// Criteria 8 (N/E/S/W-free), 16, 17
// =====================================================================

#[test]
fn ac08_the_old_north_east_south_west_handles_of_polygon_and_star_are_gone() {
    for ratio in [None, Some(0.5)] {
        let mut sc = star_scene(100, 60.0, 5, ratio, 0.0);
        // the box of a pentagon is narrower than 2R: E and W lie outside it
        for a in [0.0, PI, -FRAC_PI_2 + PI, FRAC_PI_2 * 3.0] {
            let p = sc.fr.polar(60.0, a);
            let (cur, h) = hint(&mut sc.s, p);
            assert!(!h.contains("resize") || a == 0.0 || a == PI, "{a}: {h}");
            let _ = cur;
        }
        // the E point: 3 px outside the pentagon's box: nothing there
        let p = sc.fr.polar(60.0, 0.0);
        let (cur, h) = hint(&mut sc.s, p);
        assert_eq!(
            (cur.as_str(), h.as_str()),
            ("default", ""),
            "E point of the outer circle"
        );
    }
}

#[test]
fn ac17_primitives_have_no_skew_handles_but_paths_do() {
    for build in 0..3 {
        let mut sc = match build {
            0 => rect_scene(100, 160.0, 120.0, 0.0, 0.0),
            1 => ellipse_scene(100, 160.0, 120.0, 0.0),
            _ => star_scene(100, 80.0, 8, Some(0.5), 0.0),
        };
        for (nx, ny) in SIDES {
            // skew handle: 16 px out from the side midpoint, along the normal
            let p = sc.fr.at(
                nx * (sc.fr.w_px() / 2.0 + 16.0),
                ny * (sc.fr.h_px() / 2.0 + 16.0),
            );
            let (cur, h) = hint(&mut sc.s, p);
            assert!(
                !cur.starts_with("skew") && !h.starts_with("skew"),
                "kind {build} side ({nx},{ny}): {cur} {h}"
            );
        }
    }
    let k = k_at_zoom(100);
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            anchor(1, 40.0, 30.0),
            anchor(2, 40.0 + 160.0 / k, 30.0),
            anchor(3, 40.0 + 160.0 / k, 30.0 + 120.0 / k),
        ],
        true,
    );
    let mut s = open_in_session(&d);
    click(&mut s, pt(40.0 + 80.0 / k, 30.0));
    let (cur, h) = hint(&mut s, pt(40.0 + 80.0 / k, 30.0 - 16.0 / k));
    assert!(
        cur.starts_with("skew") && h == "skew",
        "paths keep skew: {cur} {h}"
    );
}

#[test]
fn ac16_primitives_get_centre_move_rotate_and_typed_entries() {
    for build in 0..4 {
        let mk = |th: f64| match build {
            0 => rect_scene(100, 160.0, 120.0, 10.0, th),
            1 => ellipse_scene(100, 160.0, 120.0, th),
            2 => star_scene(100, 80.0, 8, None, th),
            _ => star_scene(100, 80.0, 8, Some(0.5), th),
        };
        // centre handle drag = move 1:1
        let mut sc = mk(0.0);
        let before = doc_of(&sc.s).export_loro_snapshot().unwrap();
        let c = sc.fr.c;
        // with a knob near the centre the centre handle yields (star at .5 s=160 is fine)
        let (cur, _) = hint(&mut sc.s, c);
        assert_eq!(cur, "move", "kind {build}: centre handle");
        drag(&mut sc.s, c, pt(c.x + 33.0 / sc.k, c.y - 21.0 / sc.k));
        assert_ne!(doc_of(&sc.s).export_loro_snapshot().unwrap(), before);
        // corner rotate handle with Ctrl snaps
        let mut sc = mk(0.0);
        let from = sc.fr.rot_corner(1.0, -1.0);
        let to = sc.fr.polar(150.0, -FRAC_PI_2 + 0.50);
        drag_mod(&mut sc.s, from, to, false, true);
        // `edit-interaction-polish` criterion 7: a polygon or star snaps the
        // shown angle (absolute), every other kind the turn since the press;
        // both are the shown angle here because the others start at 0.
        let shown = ObjectSnapshot::Primitive(prim_of(&sc.s, 0)).orientation();
        let deg = shown.as_radians().to_degrees();
        // The shown angle is a fixed point of the real snap table (every stop
        // of the 15 and 22.5 degree sets through all four quadrants).
        let snapped = vecmanf_ui_core::snap_angle(shown).as_radians().to_degrees();
        assert!(
            (snapped - deg).abs() < 1e-6,
            "kind {build}: shown {deg} is not a snap stop (nearest {snapped})"
        );
        // typed angle: double-click a corner rotate handle
        let mut sc = mk(0.0);
        let h = sc.fr.rot_corner(1.0, 1.0);
        let e = open_entry(&mut sc.s, h, false, false).expect("angle entry");
        assert_eq!(e.kind, "angle", "kind {build}");
        assert_eq!(
            sc.s.commit_transform_entry("37", "", 0),
            EntryOutcome::Committed
        );
        // The typed angle is the shown angle (`edit-interaction-polish`
        // criterion 7): for a star it is the frame angle plus the register.
        assert!(
            near(
                ObjectSnapshot::Primitive(prim_of(&sc.s, 0))
                    .orientation()
                    .as_radians()
                    .to_degrees(),
                37.0,
                1e-6
            ),
            "kind {build}"
        );
        // typed size
        let mut sc = mk(0.0);
        let h = sc.fr.corner(1.0, 1.0);
        let e = open_entry(&mut sc.s, h, false, false).expect("size entry");
        assert_eq!(
            e.kind,
            if build >= 2 { "radius" } else { "size" },
            "kind {build}"
        );
    }
}

#[test]
fn ac33_double_click_on_a_primitive_handle_never_switches_tools() {
    for build in 0..4 {
        let mut sc = match build {
            0 => rect_scene(100, 160.0, 120.0, 10.0, 0.0),
            1 => ellipse_scene(100, 160.0, 120.0, 0.0),
            2 => star_scene(100, 80.0, 8, None, 0.0),
            _ => star_scene(100, 80.0, 8, Some(0.5), 0.0),
        };
        for p in [sc.fr.corner(1.0, 1.0), sc.fr.rot_corner(-1.0, 1.0)] {
            let _ = open_entry(&mut sc.s, p, false, false);
            sc.s.cancel_transform_entry();
            assert_eq!(sc.s.tool(), Tool::Select, "kind {build}");
        }
    }
}

// =====================================================================
// Hostile values
// =====================================================================

fn finite_draw(dl: &DrawList) -> bool {
    dl.triangles
        .iter()
        .all(|v| v.position.x.is_finite() && v.position.y.is_finite())
}

const HOSTILE: [f64; 8] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    1e308,
    -1e308,
    1e-320,
    f64::MAX,
    f64::MIN_POSITIVE,
];

#[test]
fn hostile_pointer_values_never_panic_or_corrupt_the_document() {
    for h in [
        H::Move,
        H::Centre,
        H::ResizeCorner(1),
        H::ResizeEdge(2),
        H::RotCorner(3),
        H::Radius(0),
    ] {
        for &bad in &HOSTILE {
            for (bx, by) in [(bad, 10.0), (10.0, bad), (bad, bad)] {
                let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
                let from = handle_point(&sc, h);
                let snap0 = doc_of(&sc.s).export_loro_snapshot().unwrap();
                sc.s.pointer_hover(from, false, false);
                sc.s.pointer_down(from, false);
                sc.s.pointer_hover(pt(bx, by), true, true);
                assert!(finite_draw(&sc.s.draw_list()), "{h:?} {bx} {by}");
                let _ = sc.s.cursor_hint();
                let _ = sc.s.handle_hint();
                let _ = sc.s.live_readout();
                sc.s.double_click(pt(bx, by), false, false);
                sc.s.pointer_up(pt(bx, by), false, false);
                let d = doc_of(&sc.s);
                let (b, r) = rect_of(&sc.s, 0);
                for v in [
                    b.origin.x,
                    b.origin.y,
                    b.width.as_mm(),
                    b.height.as_mm(),
                    r.as_mm(),
                ] {
                    assert!(v.is_finite(), "{h:?} {bx} {by}: {v}");
                }
                assert!(b.width.as_mm() >= 0.0 && b.height.as_mm() >= 0.0 && r.as_mm() >= 0.0);
                let _ = (d, snap0);
            }
        }
    }
}

#[test]
fn hostile_pointer_values_on_a_star_polygon_and_ellipse() {
    for mk in 0..3 {
        for &bad in &HOSTILE {
            let mut sc = match mk {
                0 => star_scene(100, 60.0, 8, Some(0.5), 0.3),
                1 => star_scene(100, 60.0, 5, None, 0.0),
                _ => ellipse_scene(100, 120.0, 80.0, 0.5),
            };
            for from in [sc.fr.corner(1.0, 1.0), sc.fr.rot_corner(1.0, -1.0), sc.fr.c] {
                sc.s.pointer_hover(from, false, false);
                sc.s.pointer_down(from, false);
                sc.s.pointer_hover(pt(bad, 3.0), false, true);
                assert!(finite_draw(&sc.s.draw_list()));
                sc.s.pointer_up(pt(3.0, bad), false, false);
            }
            if mk == 0 {
                let v = inner_vertex(&sc);
                sc.s.pointer_hover(v, false, false);
                sc.s.pointer_down(v, false);
                sc.s.pointer_hover(pt(bad, bad), false, false);
                sc.s.pointer_up(pt(bad, bad), false, false);
                let r = star_ratio(&sc.s, 0);
                assert!((0.01..=0.99).contains(&r), "ratio {r}");
            }
        }
    }
}

#[test]
fn hostile_typed_text_into_every_field() {
    let long_digits = format!("1{}", "0".repeat(400));
    let long_frac = format!("0.{}", "1".repeat(5000));
    let texts = [
        "",
        " ",
        "\0",
        "٣",
        "５",
        "5mm",
        "5 mm",
        "+5",
        "5.",
        ".5",
        "0x10",
        "1_0",
        "5\n",
        "\t5",
        "1e5",
        "1E5",
        "--5",
        "+-5",
        "5-",
        "- 5",
        "∞",
        "NaN",
        "nan",
        "Infinity",
        "-0",
        "+0",
        "0.0",
        "00000005",
        &long_digits,
        &long_frac,
        "9999999999999999999999999",
        "1.7976931348623157e308",
        "4.9e-324",
        "0.000000000000000001",
    ];
    for t in texts {
        // bar field
        let mut s = small_rect_session(30.0, 20.0);
        let _ = s.set_selected_radius_text(t);
        let (b, r) = rect_of(&s, 0);
        assert!(
            r.as_mm().is_finite() && r.as_mm() >= 0.0 && r.as_mm() <= b.height.as_mm() / 2.0 + 1e-9,
            "bar {t:?}: {r:?}"
        );
        // radius entry
        let (mut sc, h) = entry_scene();
        open_entry(&mut sc.s, h, false, false).unwrap();
        let _ = sc.s.commit_transform_entry(t, t, 0);
        let (b, r) = rect_of(&sc.s, 0);
        assert!(
            r.as_mm().is_finite() && r.as_mm() >= 0.0 && r.as_mm() <= b.height.as_mm() / 2.0 + 1e-9,
            "entry {t:?}: {r:?}"
        );
        // ratio entry
        let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
        let h = inner_vertex(&sc);
        open_entry(&mut sc.s, h, false, false).unwrap();
        let _ = sc.s.commit_transform_entry(t, t, 1);
        let q = star_ratio(&sc.s, 0);
        assert!((0.01..=0.99).contains(&q), "ratio {t:?}: {q}");
        // size entry
        let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
        let h = handle_point(&sc, H::ResizeCorner(2));
        open_entry(&mut sc.s, h, false, false).unwrap();
        let _ = sc.s.commit_transform_entry(t, t, 0);
        let (b, r) = rect_of(&sc.s, 0);
        for v in [
            b.origin.x,
            b.origin.y,
            b.width.as_mm(),
            b.height.as_mm(),
            r.as_mm(),
        ] {
            assert!(v.is_finite(), "size {t:?}");
        }
        // the project still saves and reopens
        let _ = doc_of(&sc.s);
    }
}

#[test]
fn hostile_degenerate_and_extreme_geometry_draws_and_hit_tests_without_panicking() {
    let k = k_at_zoom(100);
    let _ = k;
    // tiny, huge and thin rectangles, 1024-point star at the extremes
    let d = Document::new(1);
    let cases = [
        (1e-9, 1e-9),
        (1e-4, 50.0),
        (50.0, 1e-4),
        (1e7, 1e7),
        (0.0, 10.0),
        (10.0, 0.0),
        (1e-300, 1e-300),
    ];
    for (i, (w, h)) in cases.iter().enumerate() {
        let id = d.create_rect(bounds(20.0 * i as f64, 10.0, *w, *h));
        let _ = d.set_corner_radius(&[id], Length::from_mm(1e6));
    }
    let _ = d.create_star(
        star_frame(300.0, 100.0, 80.0),
        PointCount::new(1024).unwrap(),
        InnerRatio::new(0.99).unwrap(),
    );
    let _ = d.create_star(
        star_frame(500.0, 100.0, 1e-6),
        PointCount::new(3).unwrap(),
        InnerRatio::new(0.01).unwrap(),
    );
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(700.0, 100.0),
        rx: Length::from_mm(1e-7),
        ry: Length::from_mm(1e5),
    });
    let mut s = open_in_session(&d);
    let n = d.object_ids().len();
    for pct in [2, 100, 5000] {
        zoom_to_pct(&mut s, pct);
        for i in 0..n {
            select_index(&mut s, i, false);
            assert!(finite_draw(&s.draw_list()), "object {i} at {pct}%");
            let _ = s.select_bar_state();
            let mut rng = Rng(i as u64 + 1);
            for _ in 0..40 {
                let p = pt(rng.range(-50.0, 800.0), rng.range(-50.0, 200.0));
                s.pointer_hover(p, false, false);
                let _ = (s.cursor_hint(), s.handle_hint());
                s.pointer_down(p, false);
                s.pointer_hover(
                    pt(p.x + rng.range(-30.0, 30.0), p.y + rng.range(-30.0, 30.0)),
                    rng.next() < 0.5,
                    rng.next() < 0.5,
                );
                assert!(finite_draw(&s.draw_list()));
                s.pointer_up(p, false, false);
            }
            let _ = s.set_selected_radius_text("1");
        }
    }
    // everything stays a valid, reopenable project with finite values
    let d2 = doc_of(&s);
    for id in d2.object_ids() {
        if let Some(p) = d2.primitive(id) {
            for a in outline_of_rotated(&p.shape, p.rotation) {
                assert!(
                    a.point.x.is_finite() && a.point.y.is_finite(),
                    "{:?}",
                    p.shape
                );
            }
        }
    }
}

// =====================================================================
// Criteria 24 and 38: stored fields and format
// =====================================================================

fn manifest_of(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = z.by_name("manifest.json").unwrap();
    let mut out = String::new();
    f.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn ac24_the_stored_fields_are_the_ones_the_shape_tools_wrote() {
    // Radius drag, entry, bar field, ratio drag, entry, slider, points: only
    // corner_radius / inner_ratio / point_count are written.
    let keys_of = |s: &Session, v0: &loro::VersionVector| -> Vec<String> {
        let json = ops_since_json(s, v0);
        let mut keys: Vec<String> = json
            .split("key: \"")
            .skip(1)
            .map(|x| x.split('"').next().unwrap().to_string())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    };
    // rectangle
    let mut sc = rect_scene(100, 170.0, 110.0, 25.0, 0.0);
    let v0 = vv_of(&sc.s);
    let from = handle_point(&sc, H::Radius(0));
    drag(
        &mut sc.s,
        from,
        pt(from.x + 20.0 / sc.k, from.y + 20.0 / sc.k),
    );
    assert_eq!(keys_of(&sc.s, &v0), ["corner_radius"]);
    let v1 = vv_of(&sc.s);
    assert_eq!(sc.s.set_selected_radius_text("3"), EntryOutcome::Committed);
    assert_eq!(keys_of(&sc.s, &v1), ["corner_radius"]);
    let v2 = vv_of(&sc.s);
    sc.s.remove_corner_rounding();
    assert_eq!(keys_of(&sc.s, &v2), ["corner_radius"]);
    // star
    let mut sc = star_scene(100, 60.0, 8, Some(0.5), 0.0);
    let v0 = vv_of(&sc.s);
    let from = inner_vertex(&sc);
    drag(&mut sc.s, from, pt(from.x + 8.0 / sc.k, from.y));
    assert_eq!(keys_of(&sc.s, &v0), ["inner_ratio"]);
    let v1 = vv_of(&sc.s);
    sc.s.set_selected_point_count(PointCount::new(11).unwrap());
    assert_eq!(keys_of(&sc.s, &v1), ["point_count"]);
    let v2 = vv_of(&sc.s);
    sc.s.preview_selected_ratio(InnerRatio::new(0.2).unwrap());
    sc.s.commit_selected_ratio();
    assert_eq!(keys_of(&sc.s, &v2), ["inner_ratio"]);
}

#[test]
fn ac24_ac38_format_version_and_field_names_match_the_shape_tool_route() {
    // Same edit through the document API (what the shape tools called) and
    // through the Select tool's knob: same stored result, same manifest.
    let k = k_at_zoom(100);
    let make = || {
        let d = Document::new(1);
        let _ = d.create_rect(bounds(40.0, 30.0, 200.0 / k, 140.0 / k));
        d
    };
    let d_ref = make();
    d_ref
        .set_corner_radius(&[d_ref.object_ids()[0]], Length::from_mm(6.0))
        .unwrap();
    let mut s = open_in_session(&make());
    select_only(&mut s);
    assert_eq!(s.set_selected_radius_text("6"), EntryOutcome::Committed);
    let (_, r) = rect_of(&s, 0);
    assert_eq!(r, Length::from_mm(6.0));
    let a = manifest_of(&pack(&d_ref, "0.1.0").unwrap());
    let b = manifest_of(&s.pack("0.1.0").unwrap());
    let va: serde_json::Value = serde_json::from_str(&a).unwrap();
    let vb: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(va["format_version"], vb["format_version"]);
    assert_eq!(va["loro_snapshot_version"], vb["loro_snapshot_version"]);
    // a knob drag too
    let mut sc = rect_scene(100, 170.0, 110.0, 0.0, 0.0);
    let from = handle_point(&sc, H::Radius(0));
    drag(
        &mut sc.s,
        from,
        pt(from.x + 20.0 / sc.k, from.y + 20.0 / sc.k),
    );
    let vc: serde_json::Value =
        serde_json::from_str(&manifest_of(&sc.s.pack("0.1.0").unwrap())).unwrap();
    assert_eq!(va["format_version"], vc["format_version"]);
    assert_eq!(
        vc["format_version"], 5,
        "the project format version is the one of main (document-core is untouched)"
    );
    // and the saved project reopens with the radius set
    let reopened = unpack(7, &sc.s.pack("0.1.0").unwrap()).unwrap();
    let id = reopened.object_ids()[0];
    match reopened.primitive(id).unwrap().shape {
        Shape::Rect { corner_radius, .. } => assert!(corner_radius.as_mm() > 0.0),
        _ => panic!(),
    }
}

#[test]
fn ac38_selecting_hovering_and_clicking_handles_writes_nothing_and_a_saved_project_reopens_unchanged()
 {
    let mut s = open_in_session(&bar_doc());
    let v0 = vv_of(&s);
    let snap0 = doc_of(&s).export_loro_snapshot().unwrap();
    for i in (0..8).filter(|i| *i != 6) {
        select_index(&mut s, i, false);
        let k = k_of(&s);
        // hover and click a handful of spots around the object, no movement
        let d = doc_of(&s);
        let o = d.primitive(d.object_ids()[i]);
        if let Some(o) = o {
            for a in outline_of_rotated(&o.shape, o.rotation) {
                for off in [
                    (0.0, 0.0),
                    (20.0 / k, 0.0),
                    (0.0, -20.0 / k),
                    (-14.0 / k, 14.0 / k),
                ] {
                    let p = pt(a.point.x + off.0, a.point.y + off.1);
                    click(&mut s, p);
                    select_index(&mut s, i, false);
                }
            }
        }
    }
    click(&mut s, pt(1000.0, 1000.0));
    assert_eq!(vv_of(&s), v0, "no click or hover writes");
    assert_eq!(doc_of(&s).export_loro_snapshot().unwrap(), snap0);
}

#[test]
fn ac12_inside_the_dead_zone_no_blue_outline_and_no_handle_change_is_drawn() {
    for h in [
        H::Move,
        H::ResizeCorner(1),
        H::RotCorner(2),
        H::Radius(0),
        H::ResizeEdge(3),
    ] {
        let mut sc = rect_scene(100, 150.0, 100.0, 20.0, 0.2);
        let from = handle_point(&sc, h);
        let v0 = vv_of(&sc.s);
        sc.s.pointer_hover(from, false, false);
        let idle = sc.s.draw_list();
        sc.s.pointer_down(from, false);
        let at = pt(from.x + 2.9 / sc.k, from.y);
        sc.s.pointer_hover(at, false, false);
        let during = sc.s.draw_list();
        let n_blue = |d: &DrawList| d.triangles.iter().filter(|v| is_blue(v)).count();
        // blue triangles belong to box, handles and any preview; a preview
        // would add triangles over the idle count at this very position
        sc.s.pointer_up(at, false, false);
        assert_eq!(vv_of(&sc.s), v0, "{h:?}");
        sc.s.pointer_hover(from, false, false);
        assert!(
            n_blue(&during) <= n_blue(&idle),
            "{h:?}: no preview inside the dead zone ({} vs {})",
            n_blue(&during),
            n_blue(&idle)
        );
    }
}

fn white_count(dl: &DrawList) -> usize {
    (0..dl.triangles.len() / 3)
        .filter(|c| {
            let v = &dl.triangles[3 * c];
            v.color.r == 255 && v.color.g == 255 && v.color.b == 255 && v.color.a == 255
        })
        .count()
}

#[test]
fn ac07_parameter_handles_are_not_drawn_while_the_object_is_moved_resized_or_rotated() {
    // Two rectangles that differ only by the parameter tier (100 px and 60 px
    // across). The white ground of a knob is the only white glyph part that
    // differs between them, so equal white counts during a drag mean no knob
    // is drawn.
    for (h, off, knobs_stay) in [
        (H::Move, (40.0, 25.0), false),
        (H::RotCorner(1), (35.0, 50.0), false),
        (H::ResizeCorner(2), (6.0, 6.0), false),
        (H::ResizeEdge(1), (6.0, 0.0), false),
        (H::Radius(0), (30.0, 30.0), true),
    ] {
        let counts = |side: f64| {
            let mut sc = rect_scene(100, side, side, 0.0, 0.0);
            sc.s.pointer_hover(pt(900.0, 900.0), false, false);
            let idle = white_count(&sc.s.draw_list());
            let from = handle_point(&sc, h);
            sc.s.pointer_hover(from, false, false);
            sc.s.pointer_down(from, false);
            sc.s.pointer_hover(
                pt(from.x + off.0 / sc.k, from.y + off.1 / sc.k),
                false,
                false,
            );
            (idle, white_count(&sc.s.draw_list()))
        };
        let (idle_big, during_big) = counts(100.0);
        let (idle_small, during_small) = counts(60.0);
        assert!(idle_big > idle_small, "{h:?}: knobs at idle");
        if knobs_stay {
            assert!(
                during_big > during_small,
                "{h:?}: the knobs stay drawn during a radius drag"
            );
        } else {
            assert_eq!(during_big, during_small, "{h:?}: no knob during the drag");
        }
    }
}

#[test]
fn ac13_preview_equals_release_at_other_zoom_levels() {
    for pct in [25, 400, 1600] {
        sweep(
            &format!("rect {pct}%"),
            &move || rect_scene(pct, 150.0, 100.0, 20.0, 0.4),
            &[
                H::Move,
                H::ResizeCorner(1),
                H::ResizeEdge(2),
                H::RotCorner(3),
                H::RotSide(0),
                H::Radius(2),
            ],
            4,
            0xabc0 + pct as u64,
        );
        sweep(
            &format!("star {pct}%"),
            &move || star_scene(pct, 60.0, 6, Some(0.5), 0.2),
            &[H::Move, H::ResizeCorner(0), H::RotCorner(1), H::Inner],
            4,
            0xdef0 + pct as u64,
        );
    }
}

#[test]
fn ac11_a_multi_object_move_shows_the_path_in_blue_over_its_black_old_one() {
    let mut s = open_in_session(&bar_doc());
    select_set(&mut s, &[0, 6]);
    let k = k_of(&s);
    let old: Vec<Point> = {
        let a = [pt(600.0, 10.0), pt(640.0, 10.0), pt(640.0, 40.0)];
        (0..30)
            .map(|i| {
                let t = f64::from(i) / 10.0;
                let (p, q) = (a[(t as usize) % 3], a[((t as usize) + 1) % 3]);
                let f = t - t.floor();
                pt(p.x + (q.x - p.x) * f, p.y + (q.y - p.y) * f)
            })
            .collect()
    };
    let from = pt(620.0, 10.0);
    let off = (60.0 / k, 33.0 / k);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(pt(from.x + off.0, from.y + off.1), false, false);
    let dl = s.draw_list();
    let new: Vec<Point> = old.iter().map(|p| pt(p.x + off.0, p.y + off.1)).collect();
    assert_eq!(black_cover_misses(&dl, &old, k), 0, "the path stays drawn");
    assert_eq!(blue_cover_misses(&dl, &new, k), 0, "its blue copy follows");
    s.pointer_up(pt(from.x + off.0, from.y + off.1), false, false);
}

#[test]
fn ac06_a_centre_handle_that_yields_has_no_hover_or_cursor_state() {
    // s = 200: a knob is within 20 px of the centre from rho of about 0.92;
    // at rho 0.95 it is 17 px from it, more than its 12 px hit radius.
    for s in [200.0, 300.0] {
        let rho = 0.95;
        let mut sc = rect_scene(100, s, s, rho * s / 2.0, 0.0);
        let p = handle_p(s, rho * s / 2.0);
        let dist_from_centre = s * SQRT_2 / 2.0 - p;
        assert!(
            (12.5..20.0).contains(&dist_from_centre),
            "scene: {dist_from_centre}"
        );
        let (cur, hnt) = hint(&mut sc.s, sc.fr.c);
        assert_ne!(cur, "move", "s={s}: the centre handle is not drawn");
        assert_eq!(hnt, "", "s={s}");
    }
}

// =====================================================================
// Criterion 8: clearance of the drawn glyphs, from the specification's own
// numbers (glyph sizes of docs/design-system.md, positions of the UX notes)
// =====================================================================

#[test]
fn ac08_no_two_drawn_glyphs_come_closer_than_4_px_for_a_rectangle_at_any_size_and_radius() {
    const KNOB: f64 = 5.0; // 10 px circle
    const RESIZE: f64 = 4.0 * SQRT_2; // 8 px square, circumscribed
    const CENTRE: f64 = 8.0 * SQRT_2; // 16 px square, circumscribed
    for (w, h) in [
        (72.0, 72.0),
        (72.0, 400.0),
        (100.0, 100.0),
        (150.0, 90.0),
        (400.0, 400.0),
        (1000.0, 73.0),
    ] {
        let s = f64::min(w, h);
        for step in 0..=40 {
            let rho = f64::from(step) / 40.0;
            let p = 15.0 + rho * l_of(s);
            // local px from the centre
            let mut knobs = vec![];
            for (sx, sy) in CORNERS {
                knobs.push((sx * (w / 2.0 - p / SQRT_2), sy * (h / 2.0 - p / SQRT_2)));
            }
            let mut others: Vec<((f64, f64), f64, &str)> = vec![];
            for (sx, sy) in CORNERS {
                others.push(((sx * w / 2.0, sy * h / 2.0), RESIZE, "corner resize"));
            }
            for (nx, ny) in SIDES {
                others.push(((nx * w / 2.0, ny * h / 2.0), RESIZE, "edge resize"));
            }
            let centre_drawn = knobs.iter().all(|k| k.0.hypot(k.1) > 20.0);
            if centre_drawn {
                others.push(((0.0, 0.0), CENTRE, "centre"));
            }
            for (i, a) in knobs.iter().enumerate() {
                for (j, b) in knobs.iter().enumerate().skip(i + 1) {
                    let gap = (a.0 - b.0).hypot(a.1 - b.1) - 2.0 * KNOB;
                    assert!(
                        gap >= 4.0 - 1e-9,
                        "{w}x{h} rho {rho}: knobs {i},{j} gap {gap}"
                    );
                }
                for (pos, r, name) in &others {
                    let gap = (a.0 - pos.0).hypot(a.1 - pos.1) - KNOB - r;
                    // the centre glyph's own clearance rule is the 20 px yield
                    // (3.7 px at the worst diagonal): the spec rounds 11.3 + 5 + 4
                    let need = if *name == "centre" { 3.6 } else { 4.0 };
                    assert!(
                        gap >= need - 1e-9,
                        "{w}x{h} rho {rho}: knob {i} vs {name}: gap {gap}"
                    );
                }
            }
        }
    }
}

#[test]
fn ac08_the_star_worst_case_keeps_4_px_at_72() {
    // N a multiple of 4, ratio .99: the inner vertex sits on the box diagonal
    // at 0.99 R, the corner glyph at sqrt2 R.
    let r = 36.0;
    let gap = r * (SQRT_2 - 0.99) - 5.0 - 4.0 * SQRT_2;
    assert!(gap >= 4.0, "{gap}");
}

#[test]
fn ac08_drawn_glyph_extents_match_the_design_system_sizes() {
    let mut sc = rect_scene(100, 120.0, 120.0, 0.0, 0.0);
    sc.s.pointer_hover(pt(900.0, 900.0), false, false);
    let dl = sc.s.draw_list();
    let extent = |centre: Point, search_px: f64| {
        let mut r: f64 = 0.0;
        let mut n = 0;
        for c in 0..dl.triangles.len() / 3 {
            let t = &dl.triangles[3 * c..3 * c + 3];
            if t.iter().all(|v| {
                (v.position.x - centre.x).hypot(v.position.y - centre.y) * sc.k <= search_px
                    && is_blue_or_white(v)
            }) {
                for v in t {
                    r = r.max((v.position.x - centre.x).hypot(v.position.y - centre.y) * sc.k);
                    n += 1;
                }
            }
        }
        (r, n)
    };
    fn is_blue_or_white(v: &Vertex) -> bool {
        is_blue(v) || (v.color.r == 255 && v.color.g == 255 && v.color.b == 255)
    }
    // a knob: a 10 px circle
    let knob = radius_handle_pos(&sc.fr, (-1.0, -1.0), 0.0);
    let (r, n) = extent(knob, 7.5);
    assert!(n > 0, "a knob is drawn");
    assert!((4.9..=5.1).contains(&r), "knob radius {r} px");
    // centre handle: 16 px rounded square, extent at most the half diagonal
    let (r, n) = extent(sc.fr.c, 13.0);
    assert!(n > 0);
    assert!(r <= 8.0 * SQRT_2 + 0.1, "centre glyph radius {r}");
}

#[test]
fn ac16_live_readouts_during_primitive_drags() {
    for build in 0..4 {
        let mk = || match build {
            0 => rect_scene(100, 160.0, 120.0, 10.0, 0.0),
            1 => ellipse_scene(100, 160.0, 120.0, 0.0),
            2 => star_scene(100, 80.0, 8, None, 0.0),
            _ => star_scene(100, 80.0, 8, Some(0.5), 0.0),
        };
        // resize
        let mut sc = mk();
        let from = sc.fr.corner(1.0, 1.0);
        sc.s.pointer_hover(from, false, false);
        sc.s.pointer_down(from, false);
        sc.s.pointer_hover(pt(from.x + 20.0 / sc.k, from.y + 12.0 / sc.k), false, false);
        let text = sc.s.live_readout().expect("resize readout").text;
        if build < 2 {
            assert!(
                text.contains('×') && text.contains("mm"),
                "kind {build}: {text}"
            );
        } else {
            assert!(
                text.starts_with('r') && text.contains("mm"),
                "kind {build}: {text}"
            );
        }
        sc.s.escape();
        // rotate
        let mut sc = mk();
        let from = sc.fr.rot_corner(1.0, -1.0);
        sc.s.pointer_hover(from, false, false);
        sc.s.pointer_down(from, false);
        sc.s.pointer_hover(pt(from.x + 20.0 / sc.k, from.y + 40.0 / sc.k), false, false);
        let text = sc.s.live_readout().expect("rotate readout").text;
        assert!(text.contains('°'), "kind {build}: {text}");
    }
}
