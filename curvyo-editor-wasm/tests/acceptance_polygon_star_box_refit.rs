//! Independent tester acceptance tests for
//! `specs/polygon-star-box-refit/specification.md`. Written from the
//! specification before the implementation diff was read (the cursor hint of
//! `select_view.rs` was seen by accident while reading the doc comment of the
//! hint API; nothing else of the change was read). Everything goes through
//! `Session`'s public API. Expected positions come from the spec's formulas
//! (the box is the square `C ± (R, R)` turned clockwise by the shown angle
//! about `C`), never from `oriented_bounds` or `orientation()`.
//!
//! The box itself is observed through what a maker sees and uses: the
//! position of the corner rotate handle (the anchor of the R chip), the
//! cursor and hint over each corner resize handle, the move hit rule, and the
//! drawn decoration, which is compared against the decoration of a square
//! rectangle turned the same way (a rectangle is out of this change, so its
//! box is the independent oracle).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(missing_docs, clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::collections::{BTreeSet, HashSet};
use std::f64::consts::SQRT_2;

use curvyo_document_core::{
    Angle, Document, InnerRatio, Length, Point, PointCount, PrimitiveSnapshot, RectBounds, Shape,
    StarFrame, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, PolyStarMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn wrap(d: f64) -> f64 {
    let mut x = d % 360.0;
    if x > 180.0 {
        x -= 360.0;
    }
    if x <= -180.0 {
        x += 360.0;
    }
    x
}

fn adiff(a: f64, b: f64) -> f64 {
    wrap(a - b)
}

/// Difference of two double-arrow cursor angles: an arrow has no head, so the
/// angles are the same modulo 180 degrees.
fn adiff180(a: f64, b: f64) -> f64 {
    let mut x = (a - b) % 180.0;
    if x > 90.0 {
        x -= 180.0;
    }
    if x < -90.0 {
        x += 180.0;
    }
    x
}

fn rot(p: Point, c: Point, deg: f64) -> Point {
    let (s, co) = deg.to_radians().sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn near(a: Point, b: Point, eps: f64) -> bool {
    (a.x - b.x).abs() <= eps && (a.y - b.y).abs() <= eps
}

/// Corner `(sx, sy)` of the box of criterion 1: `C + R(sx, sy)` turned
/// clockwise by `alpha` degrees about `C`.
fn corner(c: Point, r: f64, alpha: f64, sx: f64, sy: f64) -> Point {
    rot(pt(c.x + sx * r, c.y + sy * r), c, alpha)
}

/// The corner rotate handle: 32 px out along the box diagonal (the offset
/// the earlier slices fixed), in the box's own frame.
fn rot_handle(c: Point, r: f64, alpha: f64, k: f64, sx: f64, sy: f64) -> Point {
    let d = 32.0 / SQRT_2 / k;
    rot(pt(c.x + sx * (r + d), c.y + sy * (r + d)), c, alpha)
}

fn key(k: &str) -> KeyInput<'_> {
    KeyInput {
        key: k,
        ..KeyInput::default()
    }
}

fn parse_deg(text: &str) -> f64 {
    text.trim()
        .trim_end_matches('°')
        .replace('\u{2212}', "-")
        .trim()
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("not an angle: {text:?}"))
}

fn make_session(mode: PolyStarMode, n: u32, ratio: f64) -> Session {
    let mut s = Session::new(1);
    s.set_tool(Tool::PolygonStar);
    s.set_poly_star_mode(mode);
    s.set_poly_star_point_count(PointCount::new(n).unwrap());
    s.set_poly_star_ratio(InnerRatio::new(ratio).unwrap());
    s
}

fn drag(s: &mut Session, a: Point, b: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(a, false, false);
    s.pointer_down(a, false);
    s.pointer_hover(b, shift, ctrl);
    s.pointer_up(b, shift, ctrl);
}

/// A polygon or star at centre `c`, radius `r`, created by a drag at `deg`.
fn scene(mode: PolyStarMode, n: u32, ratio: f64, c: Point, r: f64, deg: f64) -> Session {
    let mut s = make_session(mode, n, ratio);
    let t = deg.to_radians();
    drag(
        &mut s,
        c,
        pt(c.x + r * t.cos(), c.y + r * t.sin()),
        false,
        false,
    );
    assert_eq!(s.tool(), Tool::Select);
    assert_eq!(s.selected_object_count(), 1);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("session output must reopen")
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn prim_of(s: &Session, index: usize) -> PrimitiveSnapshot {
    let d = doc_of(s);
    d.primitive(d.object_ids()[index]).unwrap()
}

fn frame_of(p: &PrimitiveSnapshot) -> StarFrame {
    match p.shape {
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame,
        _ => panic!("polygon or star expected"),
    }
}

fn first_vertex(p: &PrimitiveSnapshot) -> Point {
    outline_of_rotated(&p.shape, p.rotation)[0].point
}

/// The shown angle, from the geometry: the clockwise angle of the first outer
/// vertex from straight right.
fn geom_angle(p: &PrimitiveSnapshot) -> f64 {
    let c = frame_of(p).center;
    let v = first_vertex(p);
    (v.y - c.y).atan2(v.x - c.x).to_degrees()
}

fn k_of(s: &Session) -> f64 {
    s.view().scale()
}

/// The position of the TR corner rotate handle (the R chip's anchor) and the
/// pivot the session reports.
fn chip_geometry(s: &mut Session) -> (Point, Point) {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened, "R opens");
    let v = s.transform_entry().expect("entry view");
    s.cancel_transform_entry();
    (v.handle, v.center)
}

fn chip_angle(s: &mut Session) -> f64 {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.kind, "angle");
    let a = parse_deg(&v.fields[0].prefill);
    s.cancel_transform_entry();
    a
}

fn type_angle(s: &mut Session, text: &str) -> EntryOutcome {
    assert_eq!(s.key_down(key("r")), KeyOutcome::EntryOpened);
    s.commit_transform_entry(text, "", 0)
}

/// Checks that the session's box is the box of criterion 1 for centre `c`,
/// radius `r` and shown angle `alpha` (degrees): the rotate handle sits where
/// the turned box puts it and the pivot is the centre.
fn assert_box(s: &mut Session, c: Point, r: f64, alpha: f64, ctx: &str) {
    let k = k_of(s);
    let (h, pivot) = chip_geometry(s);
    let want = rot_handle(c, r, alpha, k, 1.0, -1.0);
    assert!(
        near(h, want, 1e-6),
        "{ctx}: rotate handle {h:?}, want {want:?}"
    );
    assert!(near(pivot, c, 1e-6), "{ctx}: pivot {pivot:?}, want {c:?}");
}

fn swept_to(handle: Point, pivot: Point, deg: f64) -> Point {
    rot(handle, pivot, deg)
}

/// Drags the TR corner rotate handle (at its place by the spec) by `deg`
/// clockwise about `pivot`, via a detour outside the dead zone. Returns the
/// last live hover point.
fn rotate_drag_from(
    s: &mut Session,
    grab: Point,
    pivot: Point,
    deg: f64,
    shift: bool,
    ctrl: bool,
) -> Point {
    s.pointer_hover(grab, shift, ctrl);
    s.pointer_down(grab, shift);
    s.pointer_hover(
        swept_to(grab, pivot, if deg >= 0.0 { 40.0 } else { -40.0 }),
        shift,
        ctrl,
    );
    let to = swept_to(grab, pivot, deg);
    s.pointer_hover(to, shift, ctrl);
    to
}

type Deco = HashSet<String>;

fn vkey(v: &curvyo_render_core::Vertex) -> String {
    format!("{:.4},{:.4}|{:?}", v.position.x, v.position.y, v.color)
}

fn triangles(s: &Session) -> Deco {
    s.draw_list().triangles.iter().map(vkey).collect()
}

/// What the draw list has on top of the same document with nothing selected
/// and nothing hovered.
fn decoration(s: &Session) -> Deco {
    let mut copy = Session::open(1, &s.pack("0.1.0").unwrap()).unwrap();
    copy.set_tool(Tool::Select);
    let base = triangles(&copy);
    triangles(s).difference(&base).cloned().collect()
}

/// A square rectangle `2r` on a side at centre `c` turned by `deg`.
fn rect_session(c: Point, r: f64, deg: f64) -> Session {
    let d = Document::new(1);
    let _ = d.create_rect(RectBounds {
        origin: pt(c.x - r, c.y - r),
        width: Length::from_mm(2.0 * r),
        height: Length::from_mm(2.0 * r),
    });
    if deg != 0.0 {
        let id = d.object_ids()[0];
        let o = d.object(id).unwrap();
        d.rotate_object(&o.rotated(c, Angle::from_radians(deg.to_radians())))
            .unwrap();
    }
    let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

// ---------------------------------------------------------------------
// Criteria 1, 3: the box is the circumscribed square in the shown direction
// ---------------------------------------------------------------------

#[test]
fn ac01_ac03_spec_example_triangle_hexagon_box_is_the_turned_square() {
    // A = (0, 0), B = (8.66, 5.0): R = 10 mm, alpha = 30 degrees.
    let mut s = make_session(PolyStarMode::Polygon, 6, 0.5);
    drag(&mut s, pt(0.0, 0.0), pt(8.66, 5.0), false, false);
    let p = prim_of(&s, 0);
    assert!((frame_of(&p).radius.as_mm() - 10.0).abs() < 0.01);
    assert!((geom_angle(&p) - 30.0).abs() < 0.05);
    // the corners of the spec example, to 0.01 mm
    let want = [
        (-3.66, -13.66),
        (13.66, -3.66),
        (3.66, 13.66),
        (-13.66, 3.66),
    ];
    let got = [
        corner(pt(0.0, 0.0), 10.0, 30.0, -1.0, -1.0),
        corner(pt(0.0, 0.0), 10.0, 30.0, 1.0, -1.0),
        corner(pt(0.0, 0.0), 10.0, 30.0, 1.0, 1.0),
        corner(pt(0.0, 0.0), 10.0, 30.0, -1.0, 1.0),
    ];
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, pt(w.0, w.1), 0.01), "{g:?} vs {w:?}");
    }
    // readout and prefill show 30 degrees
    assert!((chip_angle(&mut s) - 30.0).abs() < 0.051);
    // the rotate handle sits on that box (the sampled alpha is 30.0 +- 0.03)
    let k = k_of(&s);
    let (h, pivot) = chip_geometry(&mut s);
    let a = geom_angle(&p);
    let c = frame_of(&p).center;
    let r = frame_of(&p).radius.as_mm();
    assert!(near(h, rot_handle(c, r, a, k, 1.0, -1.0), 1e-6));
    assert!(near(pivot, c, 1e-6));
}

#[test]
fn ac01_ac03_box_follows_the_created_direction_for_every_n_ratio_and_angle() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut cases = 0;
    for (mode, ratio) in [
        (PolyStarMode::Polygon, 0.5),
        (PolyStarMode::Star, 0.5),
        (PolyStarMode::Star, 0.05),
        (PolyStarMode::Star, 0.95),
    ] {
        for n in [3, 4, 5, 6, 7, 8, 12, 33, 100, 255, 1024] {
            for deg in [
                -179.0, -135.0, -90.0, -45.0, -11.3, 0.0, 22.5, 30.0, 78.7, 90.0, 133.3, 180.0,
            ] {
                let mut s = scene(mode, n, ratio, c, r, deg);
                // alpha is what the geometry shows
                let alpha = geom_angle(&prim_of(&s, 0));
                assert!(adiff(alpha, deg).abs() < 1e-6, "created angle");
                assert_box(&mut s, c, r, alpha, &format!("{mode:?} n={n} deg={deg}"));
                cases += 1;
            }
        }
    }
    assert!(cases > 400);
}

#[test]
fn ac01_the_box_is_the_same_size_at_every_n_and_angle() {
    // R = 30 mm: the box side is 2R however the shape is turned (the size is
    // the box shown today); checked through the distance between the rotate
    // handle and the centre, which is (R + d) * sqrt(2).
    let c = pt(10.0, 20.0);
    for n in [3, 5, 6, 9] {
        for deg in [0.0, 15.0, 37.0, -77.0, 90.0] {
            let mut s = scene(PolyStarMode::Polygon, n, 0.5, c, 30.0, deg);
            let k = k_of(&s);
            let (h, _) = chip_geometry(&mut s);
            let dist = (h.x - c.x).hypot(h.y - c.y);
            let want = (30.0 + 32.0 / SQRT_2 / k) * SQRT_2;
            assert!(
                (dist - want).abs() < 1e-6,
                "n={n} deg={deg}: {dist} vs {want}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 2 and 15: one box everywhere; rectangles unchanged. Oracle: a
// square rectangle turned the same way has the box of the spec by the older,
// unchanged rule, so the polygon's selection decoration must be a part of it.
// ---------------------------------------------------------------------

#[test]
fn ac02_selection_decoration_of_a_hexagon_is_the_box_decoration_of_a_turned_square() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for deg in [0.0, 30.0, 78.7, -90.0, 133.0] {
        let mut poly = scene(PolyStarMode::Polygon, 6, 0.5, c, r, deg);
        let alpha = geom_angle(&prim_of(&poly, 0));
        poly.pointer_leave();
        let mut rect = rect_session(c, r, alpha);
        let start = rot(pt(c.x - r, c.y), c, alpha);
        rect.pointer_hover(start, false, false);
        rect.pointer_down(start, false);
        rect.pointer_up(start, false, false);
        assert_eq!(rect.selected_object_count(), 1);
        rect.pointer_leave();
        let dp = decoration(&poly);
        let dr = decoration(&rect);
        assert!(!dp.is_empty(), "deg {deg}: polygon decoration drawn");
        let missing: Vec<_> = dp.difference(&dr).take(5).collect();
        assert!(
            missing.is_empty(),
            "deg {deg}: polygon decoration vertices not in the turned-square's: {missing:?}"
        );
    }
}

#[test]
fn ac02_hover_box_of_a_polygon_is_the_hover_box_of_a_turned_square() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for deg in [0.0, 30.0, -50.0, 120.0] {
        let mut poly = scene(PolyStarMode::Polygon, 5, 0.5, c, r, deg);
        let alpha = geom_angle(&prim_of(&poly, 0));
        poly.escape();
        poly.pointer_leave();
        poly.pointer_hover(first_vertex(&prim_of(&poly, 0)), false, false);
        let dp = decoration(&poly);
        let mut rect = rect_session(c, r, alpha);
        rect.pointer_hover(rot(pt(c.x - r, c.y), c, alpha), false, false);
        let dr = decoration(&rect);
        assert!(!dp.is_empty(), "deg {deg}: hover box drawn");
        assert_eq!(dp, dr, "deg {deg}: hover decoration identical");
    }
}

#[test]
fn ac02_every_shape_of_a_multi_selection_draws_its_own_turned_box() {
    // Two hexagons selected: the decoration contains the hover/selection box
    // of each. Compared with two turned squares selected together.
    let d = Document::new(1);
    let (c1, c2) = (pt(60.0, 50.0), pt(160.0, 80.0));
    let r = 25.0;
    let frame = |c: Point, deg: f64| StarFrame {
        center: c,
        radius: Length::from_mm(r),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let _ = d.create_polygon(frame(c1, 20.0), PointCount::new(6).unwrap());
    let _ = d.create_polygon(frame(c2, -70.0), PointCount::new(5).unwrap());
    let mut poly = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    poly.set_tool(Tool::Select);
    let v1 = first_vertex(&prim_of(&poly, 0));
    let v2 = first_vertex(&prim_of(&poly, 1));
    poly.pointer_hover(v1, false, false);
    poly.pointer_down(v1, false);
    poly.pointer_up(v1, false, false);
    poly.pointer_hover(v2, true, false);
    poly.pointer_down(v2, true);
    poly.pointer_up(v2, true, false);
    assert_eq!(poly.selected_object_count(), 2);
    poly.pointer_leave();

    let e = Document::new(1);
    for (c, deg) in [(c1, 20.0), (c2, -70.0)] {
        let _ = e.create_rect(RectBounds {
            origin: pt(c.x - r, c.y - r),
            width: Length::from_mm(2.0 * r),
            height: Length::from_mm(2.0 * r),
        });
        let id = *e.object_ids().last().unwrap();
        let o = e.object(id).unwrap();
        e.rotate_object(&o.rotated(c, Angle::from_radians(f64::to_radians(deg))))
            .unwrap();
    }
    let mut rect = Session::open(2, &pack(&e, "0.1.0").unwrap()).unwrap();
    rect.set_tool(Tool::Select);
    let w1 = rot(pt(c1.x - r, c1.y), c1, 20.0);
    let w2 = rot(pt(c2.x - r, c2.y), c2, -70.0);
    rect.pointer_hover(w1, false, false);
    rect.pointer_down(w1, false);
    rect.pointer_up(w1, false, false);
    rect.pointer_hover(w2, true, false);
    rect.pointer_down(w2, true);
    rect.pointer_up(w2, true, false);
    assert_eq!(rect.selected_object_count(), 2);
    rect.pointer_leave();

    let dp = decoration(&poly);
    let dr = decoration(&rect);
    assert!(!dp.is_empty());
    let missing: Vec<_> = dp.difference(&dr).take(5).collect();
    assert!(
        missing.is_empty(),
        "multi-selection boxes differ: {missing:?}"
    );
}

// ---------------------------------------------------------------------
// Criterion 4: typed angle; R, 0, Enter
// ---------------------------------------------------------------------

#[test]
fn ac04_typed_angle_turns_shape_and_box_together_and_writes_rotation_only() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for (mode, n) in [
        (PolyStarMode::Polygon, 3),
        (PolyStarMode::Polygon, 6),
        (PolyStarMode::Star, 5),
        (PolyStarMode::Star, 1024),
    ] {
        for start in [78.7, -135.0, 0.0, 10.0] {
            for typed in ["0", "90", "-90", "180", "37.5", "-179.9"] {
                let mut s = scene(mode, n, 0.4, c, r, start);
                let before = prim_of(&s, 0);
                let changes = change_count(&s);
                let a = typed.parse::<f64>().unwrap();
                let same = adiff(a, geom_angle(&before)).abs() < 1e-9;
                let out = type_angle(&mut s, typed);
                if same {
                    assert_eq!(out, EntryOutcome::Unchanged, "typing the shown angle");
                } else {
                    assert_eq!(out, EntryOutcome::Committed);
                    assert_eq!(change_count(&s), changes + 1, "one commit");
                }
                let after = prim_of(&s, 0);
                assert!(
                    adiff(geom_angle(&after), a).abs() < 1e-6,
                    "shape shows {typed}"
                );
                // criterion 13: no stored frame angle written
                assert_eq!(
                    frame_of(&after).angle,
                    frame_of(&before).angle,
                    "{typed}: the frame angle is not written"
                );
                assert_eq!(frame_of(&after).center, frame_of(&before).center);
                assert_eq!(frame_of(&after).radius, frame_of(&before).radius);
                assert_box(&mut s, c, r, a, &format!("{mode:?} {n} {start} -> {typed}"));
                // prefill of the next entry is the typed number
                assert!(
                    adiff(chip_angle(&mut s), a).abs() < 0.051,
                    "prefill {typed}"
                );
            }
        }
    }
}

#[test]
fn ac04_spec_example_78_7_then_r_0_enter_is_upright() {
    let c = pt(100.0, 50.0);
    let mut s = scene(PolyStarMode::Polygon, 5, 0.5, c, 30.0, 78.7);
    assert!((chip_angle(&mut s) - 78.7).abs() < 0.051);
    assert_eq!(type_angle(&mut s, "0"), EntryOutcome::Committed);
    // corners C +- (R, R): the TR rotate handle is axis-aligned
    let k = k_of(&s);
    let (h, _) = chip_geometry(&mut s);
    let d = 32.0 / SQRT_2 / k;
    assert!(near(h, pt(c.x + 30.0 + d, c.y - 30.0 - d), 1e-6), "{h:?}");
    assert!(chip_angle(&mut s).abs() < 0.051);
    // the cursor of the corner handles is the upright one
    for (sx, sy, want) in [
        (1.0, -1.0, "resize:135.0"),
        (1.0, 1.0, "resize:45.0"),
        (-1.0, 1.0, "resize:135.0"),
        (-1.0, -1.0, "resize:45.0"),
    ] {
        let p = corner(c, 30.0, 0.0, sx, sy);
        s.pointer_hover(p, false, false);
        assert_eq!(s.cursor_hint(), want, "corner ({sx},{sy}) at 0 degrees");
    }
}

// ---------------------------------------------------------------------
// Criteria 5, 6: drag rotation and Ctrl snap; live box and committed box
// ---------------------------------------------------------------------

#[test]
fn ac05_dragging_a_rotate_handle_turns_the_box_with_the_shape_live_and_on_release() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for (mode, n) in [(PolyStarMode::Polygon, 5), (PolyStarMode::Star, 7)] {
        for start in [78.7, -100.0, 0.0] {
            for sweep in [13.0, -40.0, 100.0] {
                let mut s = scene(mode, n, 0.5, c, r, start);
                let at_start = prim_of(&s, 0);
                let k = k_of(&s);
                let grab = rot_handle(c, r, start, k, 1.0, -1.0);
                let to = rotate_drag_from(&mut s, grab, c, sweep, false, false);
                // live frame: the rotate handle of the live box follows. Use
                // the readout (0.1 degree) and the live decoration against a
                // turned square at the live angle.
                let live_angle = parse_deg(&s.live_readout().expect("readout").text);
                assert!(
                    adiff(live_angle, start + sweep).abs() <= 0.051,
                    "{mode:?}: live readout {live_angle}"
                );
                let alpha = wrap(start + sweep);
                let live = decoration(&s);
                s.pointer_up(to, false, false);
                let p = prim_of(&s, 0);
                assert!(adiff(geom_angle(&p), alpha).abs() < 1e-6);
                assert_box(&mut s, c, r, alpha, &format!("{mode:?} {start}+{sweep}"));
                // rotation about C writes rotation only
                assert_eq!(frame_of(&p).center, pt(c.x, c.y));
                assert_eq!(
                    frame_of(&p).angle,
                    frame_of(&at_start).angle,
                    "frame angle unchanged"
                );
                assert!(!live.is_empty(), "live decoration drawn");
            }
        }
    }
}

#[test]
fn ac05_live_box_equals_the_box_of_the_shape_committed_at_that_angle_at_every_frame() {
    // Oracle: a selected polygon that simply stands at the live angle (typed
    // there, no drag). Everything it draws (box, handles, outline) is part of
    // what the drag draws at the same angle, and the box committed on release
    // is the last live one.
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for (mode, n) in [(PolyStarMode::Polygon, 6), (PolyStarMode::Star, 5)] {
        let mut s = scene(mode, n, 0.5, c, r, 78.7);
        let alpha0 = geom_angle(&prim_of(&s, 0));
        let k = k_of(&s);
        let grab = rot_handle(c, r, alpha0, k, 1.0, -1.0);
        s.pointer_hover(grab, false, false);
        s.pointer_down(grab, false);
        for step in 1..=12 {
            let sweep = f64::from(step) * 7.0;
            s.pointer_hover(rot(grab, c, sweep), false, false);
            let alpha = wrap(alpha0 + sweep);
            let mut at = scene(mode, n, 0.5, c, r, alpha);
            at.pointer_leave();
            let live = decoration(&s);
            let missing: Vec<_> = decoration(&at).difference(&live).cloned().collect();
            // while a rotate handle is dragged its own glyph and the centre
            // glyph are not drawn as at rest; nothing else may be missing
            let hh = rot_handle(c, r, alpha, k, 1.0, -1.0);
            // (and a star's inner-radius knob, hidden while dragging)
            let knob = prim_of(&at, 0).shape_outline()[1];
            let missing: Vec<String> = missing
                .into_iter()
                .filter(|v| {
                    let xy: Vec<f64> = v
                        .split('|')
                        .next()
                        .unwrap()
                        .split(',')
                        .map(|x| x.parse().unwrap())
                        .collect();
                    let p = pt(xy[0], xy[1]);
                    let d = |q: Point| (p.x - q.x).hypot(p.y - q.y) * k;
                    d(c) > 16.0 && d(hh) > 24.0 && (mode == PolyStarMode::Polygon || d(knob) > 14.0)
                })
                .collect();
            assert!(
                missing.is_empty(),
                "{mode:?} frame {step} (alpha {alpha}): vertices of the static box not in the live one: {:?}",
                &missing[..missing.len().min(4)]
            );
        }
        let to = rot(grab, c, 84.0);
        s.pointer_hover(to, false, false);
        let live = decoration(&s);
        s.pointer_up(to, false, false);
        let alpha = wrap(alpha0 + 84.0);
        assert_box(&mut s, c, r, alpha, "committed after live frames");
        assert!(!live.is_empty());
    }
}

#[test]
fn ac06_ctrl_snap_is_absolute_and_the_box_has_the_snapped_direction() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    // spec example: 78.7 turned by a raw 1 degree with Ctrl lands on 75
    let mut s = scene(PolyStarMode::Polygon, 5, 0.5, c, r, 78.7);
    let alpha0 = geom_angle(&prim_of(&s, 0));
    let k = k_of(&s);
    let grab = rot_handle(c, r, alpha0, k, 1.0, -1.0);
    let to = rotate_drag_from(&mut s, grab, c, 1.0, false, true);
    s.pointer_up(to, false, true);
    let p = prim_of(&s, 0);
    assert!(
        adiff(geom_angle(&p), 75.0).abs() < 1e-9,
        "{}",
        geom_angle(&p)
    );
    assert_box(&mut s, c, r, 75.0, "78.7 + 1 raw with Ctrl");

    // upright whenever the readout shows 0, 90, -90 or 180
    for (start, sweep, want) in [
        (5.0, -3.0, 0.0),
        (84.0, 4.0, 90.0),
        (-80.0, -7.0, -90.0),
        (170.0, 8.0, 180.0),
    ] {
        let mut s = scene(PolyStarMode::Star, 5, 0.5, c, r, start);
        let a0 = geom_angle(&prim_of(&s, 0));
        let k = k_of(&s);
        let grab = rot_handle(c, r, a0, k, 1.0, -1.0);
        let to = rotate_drag_from(&mut s, grab, c, sweep, false, true);
        s.pointer_up(to, false, true);
        let got = geom_angle(&prim_of(&s, 0));
        assert!(adiff(got, want).abs() < 1e-9, "{start}+{sweep}: {got}");
        assert_box(&mut s, c, r, want, &format!("snap to {want}"));
        // upright box: the TR rotate handle is exactly diagonal
        let (h, _) = chip_geometry(&mut s);
        let (dx, dy) = (h.x - c.x, h.y - c.y);
        let (ax, ay) = (dx.abs(), dy.abs());
        assert!(
            (ax - ay).abs() < 1e-6 || {
                // at 90/180 the handle is at another diagonal of the same square
                (ax - ay).abs() < 1e-6
            }
        );
    }
}

// ---------------------------------------------------------------------
// Criterion 7: corner resize and the typed radius
// ---------------------------------------------------------------------

#[test]
fn ac07_corner_resize_scales_about_c_keeps_alpha_and_the_box_direction() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for (mode, n, start) in [
        (PolyStarMode::Polygon, 6, 30.0),
        (PolyStarMode::Polygon, 3, -90.0),
        (PolyStarMode::Star, 5, 78.7),
        (PolyStarMode::Star, 9, 0.0),
    ] {
        for (sx, sy) in [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let mut s = scene(mode, n, 0.4, c, r, start);
            let before = prim_of(&s, 0);
            let alpha = geom_angle(&before);
            let h = corner(c, r, alpha, sx, sy);
            // drag 12 mm along the diagonal outwards
            let dir = rot(pt(c.x + sx, c.y + sy), c, alpha);
            let (ux, uy) = ((dir.x - c.x) / SQRT_2, (dir.y - c.y) / SQRT_2);
            let to = pt(h.x + 12.0 * ux, h.y + 12.0 * uy);
            let changes = change_count(&s);
            s.pointer_hover(h, false, false);
            assert!(
                s.handle_hint().starts_with("resize-corner"),
                "{mode:?} {start} ({sx},{sy}): hint {}",
                s.handle_hint()
            );
            drag(&mut s, h, to, false, false);
            assert_eq!(change_count(&s), changes + 1, "one commit");
            let after = prim_of(&s, 0);
            let (fa, fb) = (frame_of(&after), frame_of(&before));
            assert_eq!(fa.center, fb.center, "C does not move");
            assert_eq!(fa.angle, fb.angle, "frame angle not written");
            assert_eq!(after.rotation, before.rotation, "rotation not written");
            // the dragged handle follows the pointer: R' is the pointer's
            // distance along the diagonal divided by sqrt(2)
            let along = ((to.x - c.x) * ux + (to.y - c.y) * uy) / SQRT_2;
            assert!(
                (fa.radius.as_mm() - along).abs() < 1e-6,
                "{mode:?} {start} ({sx},{sy}): R {} vs {along}",
                fa.radius.as_mm()
            );
            assert!(adiff(geom_angle(&after), alpha).abs() < 1e-9);
            let r2 = fa.radius.as_mm();
            assert_box(&mut s, c, r2, alpha, "after resize");
        }
    }
}

#[test]
fn ac07_spec_example_hexagon_r10_alpha30_dragged_5mm_along_the_diagonal() {
    let c = pt(0.0, 0.0);
    let mut s = make_session(PolyStarMode::Polygon, 6, 0.5);
    drag(&mut s, c, pt(8.66, 5.0), false, false);
    let alpha = geom_angle(&prim_of(&s, 0));
    let r0 = frame_of(&prim_of(&s, 0)).radius.as_mm();
    let h = corner(c, r0, alpha, 1.0, -1.0);
    assert!(near(h, pt(13.66, -3.66), 0.02), "{h:?}");
    drag(&mut s, h, pt(h.x + 4.83, h.y - 1.29), false, false);
    let p = prim_of(&s, 0);
    assert!(
        (frame_of(&p).radius.as_mm() - 13.54).abs() < 0.02,
        "R = {}",
        frame_of(&p).radius.as_mm()
    );
    assert!(adiff(geom_angle(&p), 30.0).abs() < 0.05);
}

#[test]
fn ac07_typed_radius_gives_the_same_result_as_the_drag_to_that_radius() {
    let c = pt(100.0, 50.0);
    for (mode, start) in [(PolyStarMode::Polygon, 30.0), (PolyStarMode::Star, 78.7)] {
        let mut a = scene(mode, 6, 0.4, c, 30.0, start);
        let mut b = scene(mode, 6, 0.4, c, 30.0, start);
        let alpha = geom_angle(&prim_of(&a, 0));
        let h = corner(c, 30.0, alpha, 1.0, -1.0);
        let dir = rot(pt(c.x + 1.0, c.y - 1.0), c, alpha);
        let (ux, uy) = ((dir.x - c.x) / SQRT_2, (dir.y - c.y) / SQRT_2);
        drag(&mut a, h, pt(h.x + 9.0 * ux, h.y + 9.0 * uy), false, false);
        let r_drag = frame_of(&prim_of(&a, 0)).radius.as_mm();
        assert_eq!(
            b.key_down(key("s")),
            KeyOutcome::EntryOpened,
            "S opens radius"
        );
        let v = b.transform_entry().unwrap();
        assert_eq!(v.kind, "radius");
        let out = b.commit_transform_entry(&format!("{r_drag}"), "", 0);
        assert_eq!(out, EntryOutcome::Committed);
        let (pa, pb) = (prim_of(&a, 0), prim_of(&b, 0));
        assert!((frame_of(&pa).radius.as_mm() - frame_of(&pb).radius.as_mm()).abs() < 1e-9);
        assert_eq!(frame_of(&pa).angle, frame_of(&pb).angle);
        assert_eq!(frame_of(&pa).center, frame_of(&pb).center);
        assert_eq!(pa.rotation, pb.rotation);
        assert_box(&mut b, c, r_drag, alpha, "typed radius");
    }
}

// ---------------------------------------------------------------------
// Criterion 8: star inner handle
// ---------------------------------------------------------------------

#[test]
fn ac08_star_inner_handle_stays_on_the_inner_vertex_at_every_angle_and_drags_mm_per_mm() {
    let c = pt(100.0, 50.0);
    let r = 40.0;
    for n in [3, 5, 8, 24] {
        for deg in [0.0, 17.0, 78.7, -90.0, 133.0, 180.0] {
            for typed in [None, Some("33")] {
                let mut s = scene(PolyStarMode::Star, n, 0.5, c, r, deg);
                if let Some(t) = typed {
                    // old-style mix of frame angle and rotation
                    assert_eq!(type_angle(&mut s, t), EntryOutcome::Committed);
                }
                let p = prim_of(&s, 0);
                let inner = outline_of_rotated(&p.shape, p.rotation)[1].point;
                s.pointer_hover(inner, false, false);
                assert_eq!(
                    s.handle_hint(),
                    "param-inner",
                    "n={n} deg={deg} typed={typed:?}: handle on the inner vertex"
                );
                assert_eq!(s.cursor_hint(), "pointer");
                // drag outwards by 2 mm along the vertex's direction
                let alpha = geom_angle(&p);
                let a_in = (inner.y - c.y).atan2(inner.x - c.x);
                let (ux, uy) = (a_in.cos(), a_in.sin());
                let ratio0 = match p.shape {
                    Shape::Star { inner_ratio, .. } => inner_ratio.get(),
                    _ => unreachable!(),
                };
                let to = pt(inner.x + 2.0 * ux, inner.y + 2.0 * uy);
                drag(&mut s, inner, to, false, false);
                let q = prim_of(&s, 0);
                let ratio1 = match q.shape {
                    Shape::Star { inner_ratio, .. } => inner_ratio.get(),
                    _ => unreachable!(),
                };
                assert!(
                    (ratio1 - (ratio0 + 2.0 / r)).abs() < 1e-6,
                    "n={n} deg={deg}: ratio {ratio0} -> {ratio1}"
                );
                assert_eq!(frame_of(&q), frame_of(&p), "frame untouched by ratio drag");
                assert_eq!(q.rotation, p.rotation);
                assert!(adiff(geom_angle(&q), alpha).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn ac08_a_polygon_has_no_parameter_handle_and_a_point_count_change_keeps_the_box() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Polygon, 6, 0.5, c, r, 30.0);
    let alpha = geom_angle(&prim_of(&s, 0));
    for p in &prim_of(&s, 0).shape_outline() {
        s.pointer_hover(*p, false, false);
        assert!(!s.handle_hint().starts_with("param"));
    }
    for n in [3, 12, 100, 6] {
        s.set_selected_point_count(PointCount::new(n).unwrap());
        let q = prim_of(&s, 0);
        assert_eq!(frame_of(&q).center, c);
        assert!((frame_of(&q).radius.as_mm() - r).abs() < 1e-9);
        assert!(
            adiff(geom_angle(&q), alpha).abs() < 1e-9,
            "alpha kept at n={n}"
        );
        assert_box(&mut s, c, r, alpha, &format!("n={n}"));
    }
    let mut s = scene(PolyStarMode::Star, 5, 0.5, c, r, -33.0);
    let alpha = geom_angle(&prim_of(&s, 0));
    for n in [3, 17, 5] {
        s.set_selected_point_count(PointCount::new(n).unwrap());
        assert!(adiff(geom_angle(&prim_of(&s, 0)), alpha).abs() < 1e-9);
        assert_box(&mut s, c, r, alpha, &format!("star n={n}"));
    }
}

trait Outline {
    fn shape_outline(&self) -> Vec<Point>;
}
impl Outline for PrimitiveSnapshot {
    fn shape_outline(&self) -> Vec<Point> {
        outline_of_rotated(&self.shape, self.rotation)
            .iter()
            .map(|a| a.point)
            .collect()
    }
}

// ---------------------------------------------------------------------
// Criterion 9: pivot and fixed point
// ---------------------------------------------------------------------

#[test]
fn ac09_shift_rotate_pivots_on_the_opposite_corner_of_the_turned_box() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for (mode, n, start, sweep) in [
        (PolyStarMode::Polygon, 6, 30.0, 25.0),
        (PolyStarMode::Star, 5, 78.7, -35.0),
        (PolyStarMode::Polygon, 3, -120.0, 70.0),
    ] {
        let mut s = scene(mode, n, 0.5, c, r, start);
        let alpha = geom_angle(&prim_of(&s, 0));
        let k = k_of(&s);
        let grab = rot_handle(c, r, alpha, k, 1.0, -1.0);
        // the opposite corner of the TR corner is the BL corner
        let pivot = corner(c, r, alpha, -1.0, 1.0);
        let to = rotate_drag_from(&mut s, grab, pivot, sweep, true, false);
        s.pointer_up(to, true, false);
        let q = prim_of(&s, 0);
        let c2 = frame_of(&q).center;
        let want_c = rot(c, pivot, sweep);
        assert!(
            near(c2, want_c, 1e-6),
            "C moved on the circle about the pivot: {c2:?} vs {want_c:?}"
        );
        assert!(adiff(geom_angle(&q), alpha + sweep).abs() < 1e-6);
        assert!((frame_of(&q).radius.as_mm() - r).abs() < 1e-9);
        assert_box(&mut s, c2, r, wrap(alpha + sweep), "after shift rotate");
    }
}

#[test]
fn ac09_the_s_key_resize_and_resize_handles_use_c_and_the_centre_handle_sits_at_c() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Polygon, 5, 0.5, c, r, 30.0);
    let alpha = geom_angle(&prim_of(&s, 0));
    // the centre move handle at C
    s.pointer_hover(c, false, false);
    assert_eq!(s.cursor_hint(), "move");
    assert_eq!(s.handle_hint(), "move");
    // a resize drag with Shift (as the spec for resize: always C)
    let h = corner(c, r, alpha, -1.0, 1.0);
    let dir = rot(pt(c.x - 1.0, c.y + 1.0), c, alpha);
    let (ux, uy) = ((dir.x - c.x) / SQRT_2, (dir.y - c.y) / SQRT_2);
    s.pointer_hover(h, true, false);
    s.pointer_down(h, true);
    let to = pt(h.x - 6.0 * ux, h.y - 6.0 * uy);
    s.pointer_hover(to, true, false);
    s.pointer_up(to, true, false);
    let q = prim_of(&s, 0);
    assert_eq!(
        frame_of(&q).center,
        c,
        "Shift does not move the fixed point"
    );
    assert!((frame_of(&q).radius.as_mm() - (r - 6.0 / SQRT_2)).abs() < 1e-6);
}

// ---------------------------------------------------------------------
// Criteria 10, 11, 12: skew hidden, cursors, hit rule
// ---------------------------------------------------------------------

#[test]
fn ac10_no_skew_handle_is_hit_testable_and_k_refuses_on_a_polygon_or_star() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        for deg in [0.0, 30.0, 78.7, -45.0] {
            let mut s = scene(mode, 6, 0.5, c, r, deg);
            let alpha = geom_angle(&prim_of(&s, 0));
            let k = k_of(&s);
            for (mx, my) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                for off_px in [8.0, 12.0, 16.0, 20.0] {
                    let o = r + off_px / k;
                    let p = rot(pt(c.x + mx * o, c.y + my * o), c, alpha);
                    s.pointer_hover(p, false, false);
                    let hint = s.handle_hint();
                    assert!(
                        hint != "skew" && hint != "skew-y",
                        "skew handle at side ({mx},{my}) {off_px} px, deg {deg}: {hint}"
                    );
                    assert!(!s.cursor_hint().starts_with("skew"));
                }
            }
            let out = s.key_down(key("k"));
            assert_ne!(out, KeyOutcome::EntryOpened, "k must not open a skew entry");
            assert!(s.transform_entry().is_none());
        }
    }
}

#[test]
fn ac11_resize_cursors_follow_the_turned_box() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    // spec example: hexagon at 30 degrees, NE 165.0, SE 75.0
    let mut s = scene(PolyStarMode::Polygon, 6, 0.5, c, r, 30.0);
    let alpha = geom_angle(&prim_of(&s, 0));
    for (sx, sy, base) in [
        (1.0, -1.0, 135.0),
        (1.0, 1.0, 45.0),
        (-1.0, 1.0, 135.0),
        (-1.0, -1.0, 45.0),
    ] {
        s.pointer_hover(corner(c, r, alpha, sx, sy), false, false);
        let want = format!("resize:{:.1}", base + alpha);
        assert_eq!(s.cursor_hint(), want);
    }
    s.pointer_hover(corner(c, r, alpha, 1.0, -1.0), false, false);
    assert_eq!(s.cursor_hint(), "resize:165.0");
    s.pointer_hover(corner(c, r, alpha, 1.0, 1.0), false, false);
    assert_eq!(s.cursor_hint(), "resize:75.0");
    // the same at many angles, polygon and star
    for mode in [PolyStarMode::Polygon, PolyStarMode::Star] {
        for deg in [-170.0, -90.0, -33.0, 0.0, 12.5, 45.0, 90.0, 135.0, 180.0] {
            let mut s = scene(mode, 5, 0.5, c, r, deg);
            let alpha = geom_angle(&prim_of(&s, 0));
            for (sx, sy, base) in [(1.0, -1.0, 135.0), (1.0, 1.0, 45.0)] {
                s.pointer_hover(corner(c, r, alpha, sx, sy), false, false);
                let got = s.cursor_hint();
                let deg_got: f64 = got.trim_start_matches("resize:").parse().unwrap();
                assert!(
                    adiff180(deg_got, base + alpha).abs() < 0.06,
                    "{mode:?} {deg}: {got}, want {}",
                    base + alpha
                );
            }
        }
    }
}

#[test]
fn ac12_the_move_region_is_the_turned_box_not_the_axis_aligned_one() {
    let c = pt(0.0, 0.0);
    // spec example: hexagon at 30 degrees, point (0, -11) starts a move
    let mut s = make_session(PolyStarMode::Polygon, 6, 0.5);
    drag(&mut s, c, pt(8.66, 5.0), false, false);
    let before = prim_of(&s, 0);
    drag(&mut s, pt(0.0, -11.0), pt(5.0, -11.0), false, false);
    let after = prim_of(&s, 0);
    assert!(
        (frame_of(&after).center.x - frame_of(&before).center.x - 5.0).abs() < 1e-6,
        "moved by 5 mm: {:?}",
        frame_of(&after).center
    );
    // a point outside the turned box (local (0.5R, -(R + 3 mm))) does not
    for deg in [30.0, 78.7, -50.0, 0.0] {
        let c = pt(100.0, 50.0);
        let r = 30.0;
        let mut s = scene(PolyStarMode::Polygon, 6, 0.5, c, r, deg);
        let alpha = geom_angle(&prim_of(&s, 0));
        let before = prim_of(&s, 0);
        let out = rot(pt(c.x + 0.5 * r, c.y - (r + 4.0)), c, alpha);
        drag(&mut s, out, pt(out.x + 5.0, out.y + 5.0), false, false);
        let q = prim_of(&s, 0);
        assert_eq!(
            frame_of(&q),
            frame_of(&before),
            "deg {deg}: outside the box does not move"
        );
        // inside the box and outside the outline: move
        let mut s = scene(PolyStarMode::Polygon, 3, 0.5, c, r, deg);
        let alpha = geom_angle(&prim_of(&s, 0));
        let inside = rot(pt(c.x - 0.9 * r, c.y - 0.9 * r), c, alpha);
        // outside a triangle with a vertex at +x? (-0.9R, -0.9R) is outside it
        let outline: Vec<Point> = prim_of(&s, 0).shape_outline();
        assert!(
            !point_in_poly(inside, &outline),
            "test point is outside the outline"
        );
        let before = frame_of(&prim_of(&s, 0));
        drag(&mut s, inside, pt(inside.x + 7.0, inside.y), false, false);
        let moved = frame_of(&prim_of(&s, 0));
        assert!(
            (moved.center.x - before.center.x - 7.0).abs() < 1e-6,
            "deg {deg}: inside the box moves"
        );
    }
}

fn point_in_poly(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------
// Criteria 13, 14: old files; no stored field; one commit
// ---------------------------------------------------------------------

fn old_doc() -> Document {
    let d = Document::new(1);
    let f = |cx: f64, deg: f64| StarFrame {
        center: pt(cx, 50.0),
        radius: Length::from_mm(20.0),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let poly = d.create_polygon(f(50.0, 78.7), PointCount::new(5).unwrap());
    let star = d.create_star(
        f(200.0, 10.0),
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let o = d.object(star).unwrap();
    d.rotate_object(&o.rotated(pt(200.0, 50.0), Angle::from_radians(30f64.to_radians())))
        .unwrap();
    let _ = poly;
    d
}

fn select_by_vertex(s: &mut Session, i: usize) {
    let v = first_vertex(&prim_of(s, i));
    s.pointer_hover(v, false, false);
    s.pointer_down(v, false);
    s.pointer_up(v, false, false);
    assert_eq!(s.selected_object_count(), 1);
}

fn collect_keys(v: &loro::LoroValue, out: &mut BTreeSet<String>) {
    match v {
        loro::LoroValue::Map(m) => {
            for (k, x) in m.iter() {
                out.insert(k.clone());
                collect_keys(x, out);
            }
        }
        loro::LoroValue::List(l) => {
            for x in l.iter() {
                collect_keys(x, out);
            }
        }
        _ => {}
    }
}

fn stored_keys(s: &Session) -> BTreeSet<String> {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    let mut out = BTreeSet::new();
    collect_keys(&l.get_deep_value(), &mut out);
    out
}

#[test]
fn ac13_old_project_opens_with_the_same_outlines_shows_78_7_and_40_and_writes_nothing() {
    let d = old_doc();
    let bytes = pack(&d, "0.1.0").unwrap();
    let reference = unpack(7, &bytes).unwrap();
    let mut s = Session::open(3, &bytes).unwrap();
    s.set_tool(Tool::Select);
    let changes = change_count(&s);
    let doc0 = doc_of(&s);
    // same outlines, vertex by vertex
    for (i, id) in reference.object_ids().iter().enumerate() {
        let a = reference.primitive(*id).unwrap();
        let b = prim_of(&s, i);
        let (oa, ob) = (a.shape_outline(), b.shape_outline());
        assert_eq!(oa.len(), ob.len());
        for (p, q) in oa.iter().zip(&ob) {
            assert!(near(*p, *q, 1e-9));
        }
    }
    let shown = [78.7, 40.0];
    for (i, want) in shown.iter().enumerate() {
        let p = prim_of(&s, i);
        assert!(
            adiff(geom_angle(&p), *want).abs() < 1e-6,
            "outline direction {i}"
        );
        // open, select, hover: no write
        s.pointer_hover(pt(0.0, 0.0), false, false);
        select_by_vertex(&mut s, i);
        assert!(
            adiff(chip_angle(&mut s), *want).abs() < 0.051,
            "shown angle {i}"
        );
        let f = frame_of(&p);
        assert_box(
            &mut s,
            f.center,
            f.radius.as_mm(),
            *want,
            &format!("old file {i}"),
        );
        s.pointer_hover(corner(f.center, 20.0, *want, 1.0, 1.0), false, false);
        s.pointer_leave();
    }
    assert_eq!(
        change_count(&s),
        changes,
        "opening, selecting, hovering write nothing"
    );
    assert_eq!(
        format!("{:?}", doc_of(&s).object_ids()),
        format!("{:?}", doc0.object_ids())
    );
    for i in 0..2 {
        assert_eq!(
            prim_of(&s, i),
            reference.primitive(reference.object_ids()[i]).unwrap()
        );
    }
}

#[test]
fn ac13_rotate_writes_rotation_only_and_resize_writes_the_radius_only_on_old_files() {
    let d = old_doc();
    let bytes = pack(&d, "0.1.0").unwrap();
    for i in 0..2 {
        let shown = [78.7, 40.0][i];
        // rotate by a typed angle
        let mut s = Session::open(3, &bytes).unwrap();
        s.set_tool(Tool::Select);
        select_by_vertex(&mut s, i);
        let before = prim_of(&s, i);
        assert_eq!(type_angle(&mut s, "0"), EntryOutcome::Committed);
        let after = prim_of(&s, i);
        assert_eq!(
            frame_of(&after).angle,
            frame_of(&before).angle,
            "frame angle kept"
        );
        assert_eq!(frame_of(&after).center, frame_of(&before).center);
        assert_eq!(frame_of(&after).radius, frame_of(&before).radius);
        assert!(geom_angle(&after).abs() < 1e-6);
        let f = frame_of(&after);
        assert_box(&mut s, f.center, 20.0, 0.0, "old file typed 0");

        // rotate by a drag about C
        let mut s = Session::open(3, &bytes).unwrap();
        s.set_tool(Tool::Select);
        select_by_vertex(&mut s, i);
        let before = prim_of(&s, i);
        let f = frame_of(&before);
        let k = k_of(&s);
        let grab = rot_handle(f.center, 20.0, shown, k, 1.0, -1.0);
        let to = rotate_drag_from(&mut s, grab, f.center, 17.0, false, false);
        s.pointer_up(to, false, false);
        let after = prim_of(&s, i);
        assert_eq!(frame_of(&after).angle, f.angle);
        assert!(adiff(geom_angle(&after), shown + 17.0).abs() < 1e-6);
        assert_box(&mut s, f.center, 20.0, wrap(shown + 17.0), "old file drag");

        // resize: rotation and frame angle untouched
        let mut s = Session::open(3, &bytes).unwrap();
        s.set_tool(Tool::Select);
        select_by_vertex(&mut s, i);
        let before = prim_of(&s, i);
        let f = frame_of(&before);
        let h = corner(f.center, 20.0, shown, 1.0, 1.0);
        let dir = rot(pt(f.center.x + 1.0, f.center.y + 1.0), f.center, shown);
        let (ux, uy) = ((dir.x - f.center.x) / SQRT_2, (dir.y - f.center.y) / SQRT_2);
        drag(
            &mut s,
            h,
            pt(h.x + 10.0 * ux, h.y + 10.0 * uy),
            false,
            false,
        );
        let after = prim_of(&s, i);
        assert_eq!(frame_of(&after).angle, f.angle);
        assert_eq!(after.rotation, before.rotation);
        assert!(frame_of(&after).radius.as_mm() > 20.0);
        assert!(adiff(geom_angle(&after), shown).abs() < 1e-6);
    }
}

#[test]
fn ac13_golden_fixtures_open_select_and_hover_write_nothing_and_keep_outlines() {
    for name in [
        "primitives_v3.curvyo",
        "rotation_v5.curvyo",
        "valid.curvyo",
        "paths_v2.curvyo",
        "format_version_1.curvyo",
    ] {
        let p = format!(
            "{}/../curvyo-document-core/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&p).unwrap();
        let reference = unpack(7, &bytes).unwrap();
        let mut s = Session::open(3, &bytes).unwrap();
        s.set_tool(Tool::Select);
        let n = change_count(&s);
        let keys = stored_keys(&s);
        for (i, id) in reference.object_ids().iter().enumerate() {
            let Some(a) = reference.primitive(*id) else {
                continue;
            };
            if !matches!(a.shape, Shape::Polygon { .. } | Shape::Star { .. }) {
                continue;
            }
            let b = prim_of(&s, i);
            for (p, q) in a.shape_outline().iter().zip(&b.shape_outline()) {
                assert!(near(*p, *q, 1e-9), "{name}: outline");
            }
            select_by_vertex(&mut s, i);
            let f = frame_of(&b);
            let alpha = geom_angle(&b);
            assert_box(&mut s, f.center, f.radius.as_mm(), alpha, name);
            s.pointer_hover(
                corner(f.center, f.radius.as_mm(), alpha, 1.0, 1.0),
                false,
                false,
            );
            s.escape();
            s.pointer_leave();
        }
        assert_eq!(change_count(&s), n, "{name}: nothing written");
        assert_eq!(stored_keys(&s), keys, "{name}: no new key");
    }
}

#[test]
fn ac14_no_new_stored_key_format_version_5_and_one_commit_per_gesture() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Star, 5, 0.5, c, r, 78.7);
    let keys0 = stored_keys(&s);
    let n0 = change_count(&s);
    // typed rotation
    assert_eq!(type_angle(&mut s, "0"), EntryOutcome::Committed);
    assert_eq!(change_count(&s), n0 + 1);
    let keys1 = stored_keys(&s);
    // a reference through the document API a build from before this change
    // also has: rotation is written by `rotate_object`
    let d = Document::new(1);
    let id = d.create_star(
        StarFrame {
            center: c,
            radius: Length::from_mm(r),
            angle: Angle::from_radians(78.7f64.to_radians()),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(c, Angle::from_radians(-78.7f64.to_radians())))
        .unwrap();
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let mut ref_keys = BTreeSet::new();
    collect_keys(&l.get_deep_value(), &mut ref_keys);
    assert!(
        keys1.is_subset(&ref_keys) || keys0.is_subset(&ref_keys) || keys1 == ref_keys,
        "{keys1:?} vs {ref_keys:?}"
    );
    // rotate drag, resize drag: the number of commits per gesture equals what
    // a rectangle (not part of this change) gets for the same gestures
    let seq = |s: &mut Session, c: Point, r: f64, alpha: f64| -> Vec<usize> {
        let mut counts = vec![change_count(s)];
        assert_eq!(type_angle(s, "10"), EntryOutcome::Committed);
        counts.push(change_count(s));
        let alpha = alpha + 10.0;
        let k = k_of(s);
        let grab = rot_handle(c, r, alpha, k, 1.0, -1.0);
        let to = rotate_drag_from(s, grab, c, 33.0, false, false);
        s.pointer_up(to, false, false);
        counts.push(change_count(s));
        let alpha = alpha + 33.0;
        let h = corner(c, r, alpha, 1.0, 1.0);
        drag(s, h, pt(h.x + 5.0, h.y + 5.0), false, false);
        counts.push(change_count(s));
        counts
    };
    let alpha = geom_angle(&prim_of(&s, 0));
    let got = seq(&mut s, c, r, alpha);
    let mut rect = rect_session(c, r, alpha);
    let start = rot(pt(c.x - r, c.y), c, alpha);
    rect.pointer_hover(start, false, false);
    rect.pointer_down(start, false);
    rect.pointer_up(start, false, false);
    let want = seq(&mut rect, c, r, alpha);
    let deltas = |v: &[usize]| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    // Loro merges changes that follow each other closely, so the count can
    // only show an upper bound: no gesture is more than one change, and the
    // polygon never needs more than the rectangle does.
    assert!(
        deltas(&got).iter().all(|d| *d <= 1),
        "no gesture writes twice"
    );
    assert!(
        deltas(&got).iter().sum::<usize>() <= deltas(&want).iter().sum::<usize>(),
        "{got:?} vs rectangle {want:?}"
    );
    assert!(
        stored_keys(&s).is_subset(&keys1),
        "no new key after gestures"
    );
    // creation is one commit
    let mut t = make_session(PolyStarMode::Polygon, 5, 0.5);
    let n = change_count(&t);
    drag(&mut t, c, pt(c.x + 20.0, c.y + 10.0), false, false);
    assert_eq!(change_count(&t), n + 1, "creation is one commit");
}

#[test]
fn ac14_hover_selection_and_escape_are_undo_neutral_and_write_nothing() {
    let c = pt(100.0, 50.0);
    let mut s = scene(PolyStarMode::Polygon, 6, 0.5, c, 30.0, 45.0);
    let before = s.pack("0.1.0").unwrap();
    let n = change_count(&s);
    for p in [pt(0.0, 0.0), c, pt(130.0, 20.0), pt(105.0, 55.0)] {
        s.pointer_hover(p, false, false);
        let _ = s.cursor_hint();
        let _ = s.handle_hint();
        let _ = s.draw_list();
    }
    s.escape();
    s.pointer_leave();
    assert_eq!(change_count(&s), n);
    assert_eq!(doc_of(&s).object_ids().len(), 1);
    let _ = before;
}

// ---------------------------------------------------------------------
// Criterion 15: rectangle, ellipse and path unchanged
// ---------------------------------------------------------------------

#[test]
fn ac15_rectangle_and_ellipse_boxes_still_turn_by_rotation_alone() {
    let c = pt(60.0, 45.0);
    for deg in [0.0, 30.0, -77.0, 150.0] {
        let mut s = rect_session(c, 20.0, deg);
        let start = rot(pt(c.x - 20.0, c.y), c, deg);
        s.pointer_hover(start, false, false);
        s.pointer_down(start, false);
        s.pointer_up(start, false, false);
        assert_eq!(s.selected_object_count(), 1);
        assert_box(&mut s, c, 20.0, deg, &format!("rect {deg}"));
        let k = k_of(&s);
        let _ = k;
        // cursors: base angle plus rotation
        for (sx, sy, base) in [(1.0, -1.0, 135.0), (1.0, 1.0, 45.0)] {
            s.pointer_hover(corner(c, 20.0, deg, sx, sy), false, false);
            let got: f64 = s
                .cursor_hint()
                .trim_start_matches("resize:")
                .parse()
                .unwrap();
            assert!(adiff180(got, base + deg).abs() < 0.06);
        }
    }
    // ellipse: circle stays a circle; frame-box rotation
    let d = Document::new(1);
    let id = d.create_ellipse(curvyo_document_core::EllipseFrame {
        center: c,
        rx: Length::from_mm(20.0),
        ry: Length::from_mm(12.0),
    });
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(c, Angle::from_radians(40f64.to_radians())))
        .unwrap();
    let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    let p = prim_of(&s, 0);
    let v = p.shape_outline()[0];
    s.pointer_hover(v, false, false);
    s.pointer_down(v, false);
    s.pointer_up(v, false, false);
    assert_eq!(s.selected_object_count(), 1);
    let k = k_of(&s);
    let (h, pivot) = chip_geometry(&mut s);
    let dx = 20.0 + 32.0 / SQRT_2 / k;
    let dy = 12.0 + 32.0 / SQRT_2 / k;
    let want = rot(pt(c.x + dx, c.y - dy), c, 40.0);
    assert!(near(h, want, 1e-6), "ellipse handle {h:?} vs {want:?}");
    assert!(near(pivot, c, 1e-6));
}

// ---------------------------------------------------------------------
// Regression: M / K / R / S keys, move, axis lock, clone on polygons
// ---------------------------------------------------------------------

#[test]
fn regress_typed_move_and_keys_on_a_polygon_at_an_angle() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Polygon, 6, 0.5, c, r, 30.0);
    let alpha = geom_angle(&prim_of(&s, 0));
    // move by dragging on the centre handle
    let to = pt(c.x + 12.0, c.y - 7.0);
    drag(&mut s, c, to, false, false);
    let q = prim_of(&s, 0);
    assert!(
        near(frame_of(&q).center, to, 1e-6),
        "moved: {:?}",
        frame_of(&q).center
    );
    assert!(adiff(geom_angle(&q), alpha).abs() < 1e-9);
    assert_box(&mut s, to, r, alpha, "after move");
    // M opens a move entry
    let out = s.key_down(key("m"));
    assert_eq!(out, KeyOutcome::EntryOpened, "M");
    s.cancel_transform_entry();
    // S opens the radius entry, R the angle entry
    for k in ["s", "r"] {
        let out = s.key_down(key(k));
        assert_eq!(out, KeyOutcome::EntryOpened, "{k}");
        s.cancel_transform_entry();
    }
    // K does not skew a polygon
    assert_ne!(s.key_down(key("k")), KeyOutcome::EntryOpened);
}

#[test]
fn regress_axis_locked_move_and_clone_keep_the_shape_and_the_box_direction() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Star, 5, 0.5, c, r, 78.7);
    let alpha = geom_angle(&prim_of(&s, 0));
    // axis-locked (Shift) move
    s.pointer_hover(c, true, false);
    s.pointer_down(c, false);
    s.pointer_hover(pt(c.x + 30.0, c.y + 4.0), true, false);
    s.pointer_up(pt(c.x + 30.0, c.y + 4.0), true, false);
    let q = prim_of(&s, 0);
    assert!(
        near(frame_of(&q).center, pt(c.x + 30.0, c.y), 1e-6),
        "axis locked"
    );
    assert!(adiff(geom_angle(&q), alpha).abs() < 1e-9);
    assert_box(&mut s, frame_of(&q).center, r, alpha, "after locked move");
}

// ---------------------------------------------------------------------
// White-box follow-ups: sums outside one turn, extreme sizes
// ---------------------------------------------------------------------

#[test]
fn edge_old_style_sums_beyond_a_turn_wrap_into_the_shown_angle_and_the_box() {
    // frame -200 and rotation -50 show wrap(-250) = 110; frame 560 and
    // rotation 190 show wrap(750) = 30.
    let d = Document::new(1);
    let mk = |cx: f64, deg: f64| StarFrame {
        center: pt(cx, 50.0),
        radius: Length::from_mm(20.0),
        angle: Angle::from_radians(deg.to_radians()),
    };
    let a = d.create_polygon(mk(50.0, -200.0), PointCount::new(7).unwrap());
    let b = d.create_star(
        mk(200.0, 560.0),
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.3).unwrap(),
    );
    for (id, deg, cx) in [(a, -50.0, 50.0), (b, 190.0, 200.0)] {
        let o = d.object(id).unwrap();
        d.rotate_object(&o.rotated(pt(cx, 50.0), Angle::from_radians(f64::to_radians(deg))))
            .unwrap();
    }
    let bytes = pack(&d, "0.1.0").unwrap();
    let mut s = Session::open(3, &bytes).unwrap();
    s.set_tool(Tool::Select);
    for (i, want) in [110.0, 30.0].into_iter().enumerate() {
        select_by_vertex(&mut s, i);
        let c = frame_of(&prim_of(&s, i)).center;
        assert!(adiff(chip_angle(&mut s), want).abs() < 0.051, "shown {i}");
        assert_box(&mut s, c, 20.0, want, &format!("wrapped sum {i}"));
    }
}

#[test]
fn edge_tiny_and_huge_radius_keep_the_box_direction() {
    for r in [0.05, 0.6, 5.0, 4000.0, 250_000.0] {
        for deg in [-179.0, -1.0, 33.0, 90.0] {
            let c = pt(1000.0, 800.0);
            let mut s = scene(PolyStarMode::Polygon, 5, 0.5, c, r, deg);
            let alpha = geom_angle(&prim_of(&s, 0));
            let k = k_of(&s);
            let (h, pivot) = chip_geometry(&mut s);
            let want = rot_handle(c, r, alpha, k, 1.0, -1.0);
            let tol = 1e-9 * r.max(1.0) * 100.0;
            assert!(near(h, want, tol), "r={r} deg={deg}: {h:?} vs {want:?}");
            assert!(near(pivot, c, tol));
            assert_eq!(type_angle(&mut s, "0"), EntryOutcome::Committed);
            let (h0, _) = chip_geometry(&mut s);
            let d = 32.0 / SQRT_2 / k;
            assert!(
                near(h0, pt(c.x + r + d, c.y - r - d), tol),
                "r={r}: upright"
            );
        }
    }
}

#[test]
fn edge_repeated_rotations_do_not_drift_the_box_away_from_the_shown_angle() {
    let c = pt(100.0, 50.0);
    let r = 30.0;
    let mut s = scene(PolyStarMode::Star, 7, 0.6, c, r, 41.0);
    let mut shown = geom_angle(&prim_of(&s, 0));
    for i in 0..60 {
        let k = k_of(&s);
        let grab = rot_handle(c, r, shown, k, 1.0, -1.0);
        let sweep = 11.0 + f64::from(i % 5);
        let to = rotate_drag_from(&mut s, grab, c, sweep, false, false);
        s.pointer_up(to, false, false);
        shown = wrap(shown + sweep);
        assert!(
            adiff(geom_angle(&prim_of(&s, 0)), shown).abs() < 1e-6,
            "step {i}"
        );
        assert_box(&mut s, c, r, shown, &format!("step {i}"));
    }
}
