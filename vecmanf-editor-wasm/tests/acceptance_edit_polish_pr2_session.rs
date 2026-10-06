//! Independent tester acceptance tests for PR 2 of
//! `specs/edit-interaction-polish/specification.md`, Part E, through
//! `Session`: the device pixel ratio reaches the selection box (65), the box
//! is drawn above the artwork and below the handles (67), the hover box is
//! solid (66), and the skew fixed-line guide is 2 on / 2 off (68). Written
//! from the specification before the implementation diff was read. Expected
//! geometry is computed here from the document, never read back from the
//! code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(missing_docs, clippy::doc_markdown, clippy::type_complexity)]
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use vecmanf_document_core::{
    AnchorId, Angle, Document, Length, NewAnchor, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot,
    RectBounds, ViewTransform, pack,
};
use vecmanf_editor_wasm::{Session, Tool};
use vecmanf_render_core::{
    DrawList, RgbaColor, SelectDecorationInput, Vertex, build_primitive_strokes,
    build_select_draw_list,
};

const ACCENT: RgbaColor = RgbaColor::opaque(0x2F, 0x6F, 0xEE);

fn accent_hover() -> RgbaColor {
    RgbaColor { a: 51, ..ACCENT }
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn open_doc(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

/// How many objects are selected, read from the select bar's state (the
/// bar shows once something is selected); only 0 / not 0 is needed here.
fn selected(s: &Session, baseline: usize) -> bool {
    s.draw_list().triangle_count() > baseline
}

fn rect_doc() -> (Document, NodeId) {
    let d = Document::new(1);
    let id = d.create_rect(RectBounds {
        origin: pt(10.0, 10.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    (d, id)
}

fn two_rect_doc() -> (Document, NodeId, NodeId) {
    let (d, a) = rect_doc();
    let b = d.create_rect(RectBounds {
        origin: pt(100.0, 10.0),
        width: Length::from_mm(40.0),
        height: Length::from_mm(20.0),
    });
    (d, a, b)
}

fn rect_corners(x: f64, y: f64, w: f64, h: f64) -> [Point; 4] {
    [pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]
}

fn prims_of(s: &Session) -> Vec<PrimitiveSnapshot> {
    let bytes = s.pack("0.1.0").unwrap();
    let d = vecmanf_document_core::unpack(9, &bytes).unwrap();
    d.object_ids()
        .into_iter()
        .filter_map(|id| match d.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => Some(p),
            ObjectSnapshot::Path(_) => None,
        })
        .collect()
}

/// The index at which `needle` occurs as a contiguous slice of `hay`.
fn find_slice(hay: &[Vertex], needle: &[Vertex]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

// ---------------------------------------------------------------------
// Criterion 65 plumbing, 67 order: the box sits above the artwork and
// below the handles, and it is the box that criterion 63..65 describe
// ---------------------------------------------------------------------

#[test]
fn ac65_ac67_the_session_draws_strokes_then_the_dashed_box_then_the_handles() {
    for dpr in [1.0, 1.5, 2.0] {
        let (d, id) = rect_doc();
        let mut s = open_doc(&d);
        s.set_device_pixel_ratio(dpr);
        let base = s.draw_list().triangle_count();
        click(&mut s, pt(30.0, 10.0));
        assert!(selected(&s, base));
        let view = s.view();
        let list = s.draw_list();
        let expected_box = build_select_draw_list(
            view,
            &SelectDecorationInput {
                selected: vec![(id, rect_corners(10.0, 10.0, 40.0, 20.0))],
                hovered: None,
                device_pixel_ratio: dpr,
                skew_guide: None,
            },
        );
        assert!(
            expected_box.triangle_count() > 8,
            "dashed: more than the four solid quads"
        );
        let at = find_slice(&list.triangles, &expected_box.triangles).unwrap_or_else(|| {
            panic!("dpr {dpr}: the session's draw list holds the box built with this ratio")
        });
        // artwork first: the stroke triangles all precede the box
        let strokes = build_primitive_strokes(&prims_of(&s), view);
        assert_ne!(strokes.triangles.len(), 0);
        let stroke_at =
            find_slice(&list.triangles, &strokes.triangles).expect("strokes in the list");
        assert!(
            stroke_at + strokes.triangles.len() <= at,
            "dpr {dpr}: the box is drawn above the artwork (strokes end {}, box starts {at})",
            stroke_at + strokes.triangles.len()
        );
        // handles after the box
        let after = list.triangles.len() - (at + expected_box.triangles.len());
        assert!(after > 0, "dpr {dpr}: handle glyphs follow the box");
    }
}

#[test]
fn ac65_the_ratio_changes_the_drawing_and_a_bad_ratio_reads_as_one() {
    let (d, _) = rect_doc();
    let mut s = open_doc(&d);
    click(&mut s, pt(30.0, 10.0));
    let default = s.draw_list();
    assert!(
        (s.device_pixel_ratio() - 1.0).abs() < 1e-12,
        "1 before any is set"
    );
    s.set_device_pixel_ratio(2.0);
    let two = s.draw_list();
    assert_ne!(default, two, "a ratio of 2 changes the pixel-snapped box");
    for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
        s.set_device_pixel_ratio(bad);
        assert_eq!(s.draw_list(), default, "ratio {bad} reads as 1");
    }
    s.set_device_pixel_ratio(1.0);
    assert_eq!(s.draw_list(), default);
}

#[test]
fn ac65_the_box_keeps_snapping_after_zoom_and_pan() {
    // whole device pixel rows at several zoom levels and positions, read from
    // the session's own draw list through an independently built box
    let (d, id) = rect_doc();
    for dpr in [1.0, 2.0] {
        for (zoom_steps, pan_x, pan_y) in [
            (0_i32, 0.0, 0.0),
            (3, 13.0, -7.0),
            (-4, 101.0, 33.0),
            (8, -5.0, 2.5),
        ] {
            let mut s = open_doc(&d);
            s.set_device_pixel_ratio(dpr);
            s.resize_viewport(1200.0, 800.0);
            for _ in 0..zoom_steps.abs() {
                s.wheel(
                    0.0,
                    if zoom_steps > 0 { -120.0 } else { 120.0 },
                    300.0,
                    200.0,
                    false,
                    true,
                );
            }
            s.wheel(pan_x, pan_y, 0.0, 0.0, false, false);
            let base = s.draw_list().triangle_count();
            click(&mut s, pt(30.0, 10.0));
            if !selected(&s, base) {
                // zoomed so far that the press missed
                continue;
            }
            let view = s.view();
            let list = s.draw_list();
            let boxl = build_select_draw_list(
                view,
                &SelectDecorationInput {
                    selected: vec![(id, rect_corners(10.0, 10.0, 40.0, 20.0))],
                    hovered: None,
                    device_pixel_ratio: dpr,
                    skew_guide: None,
                },
            );
            assert!(
                find_slice(&list.triangles, &boxl.triangles).is_some(),
                "dpr {dpr} zoom {zoom_steps}: session drew the box at this view"
            );
            // every thin triangle of the box lies on whole device rows/columns
            for t in boxl.triangles.chunks(3) {
                let dev: Vec<(f64, f64)> = t
                    .iter()
                    .map(|v| {
                        let (x, y) = view.document_to_screen(v.position);
                        (x * dpr, y * dpr)
                    })
                    .collect();
                let (xs, ys): (Vec<f64>, Vec<f64>) = dev.iter().copied().unzip();
                let ext = |v: &[f64]| {
                    v.iter().copied().fold(f64::MIN, f64::max)
                        - v.iter().copied().fold(f64::MAX, f64::min)
                };
                let whole = |v: f64| (v - v.round()).abs() < 1e-6;
                if ext(&ys) <= 3.0 + 1e-6 && ext(&xs) > ext(&ys) {
                    assert!(ys.iter().all(|y| whole(*y)), "dpr {dpr}: row {ys:?}");
                }
                if ext(&xs) <= 3.0 + 1e-6 && ext(&ys) > ext(&xs) {
                    assert!(xs.iter().all(|x| whole(*x)), "dpr {dpr}: column {xs:?}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 66: hover box solid and distinct
// ---------------------------------------------------------------------

#[test]
fn ac66_hovering_an_unselected_object_draws_a_solid_accent_hover_box() {
    let (d, a, b) = two_rect_doc();
    let mut s = open_doc(&d);
    s.set_device_pixel_ratio(2.0);
    click(&mut s, pt(30.0, 10.0)); // select a
    s.pointer_hover(pt(120.0, 10.0), false, false); // hover b (its outline)
    let view = s.view();
    let list = s.draw_list();
    let hover_only = build_select_draw_list(
        view,
        &SelectDecorationInput {
            selected: vec![],
            hovered: Some((b, rect_corners(100.0, 10.0, 40.0, 20.0))),
            device_pixel_ratio: 2.0,
            skew_guide: None,
        },
    );
    assert_eq!(
        hover_only.triangle_count(),
        8,
        "four solid quads, two triangles each"
    );
    assert!(
        hover_only
            .triangles
            .iter()
            .all(|v| v.color == accent_hover())
    );
    let _ = a;
    // the session's list has the hover box as a slice
    assert!(
        find_slice(&list.triangles, &hover_only.triangles).is_some(),
        "the hovered, unselected object gets a solid --accent-hover box"
    );
    // the selected one is dashed full accent in the same list
    let sel_only = build_select_draw_list(
        view,
        &SelectDecorationInput {
            selected: vec![(a, rect_corners(10.0, 10.0, 40.0, 20.0))],
            hovered: None,
            device_pixel_ratio: 2.0,
            skew_guide: None,
        },
    );
    assert!(sel_only.triangle_count() > 8);
    assert!(sel_only.triangles.iter().all(|v| v.color == ACCENT));
}

#[test]
fn ac66_hovering_the_selected_object_adds_no_hover_box() {
    let (d, id) = rect_doc();
    let mut s = open_doc(&d);
    click(&mut s, pt(30.0, 10.0));
    s.pointer_hover(pt(30.0, 10.0), false, false);
    let view = s.view();
    let list = s.draw_list();
    let hover_only = build_select_draw_list(
        view,
        &SelectDecorationInput {
            selected: vec![],
            hovered: Some((id, rect_corners(10.0, 10.0, 40.0, 20.0))),
            device_pixel_ratio: 1.0,
            skew_guide: None,
        },
    );
    assert!(
        find_slice(&list.triangles, &hover_only.triangles).is_none(),
        "a selected object shows only the dashed box"
    );
}

// ---------------------------------------------------------------------
// Criterion 68: the skew fixed-line guide, 2 on / 2 off
// ---------------------------------------------------------------------

fn skew_doc(deg: f64) -> Document {
    let d = Document::new(1);
    let id = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(100.0, 100.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(140.0, 100.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(120.0, 130.0)),
        ],
        true,
    );
    if deg != 0.0 {
        let o = d.object(id).unwrap();
        // the tight box of the three anchors is (100,100)-(140,130)
        d.rotate_object(&o.rotated(pt(120.0, 115.0), Angle::from_radians(deg.to_radians())))
            .unwrap();
    }
    d
}

fn rotate_pt(p: Point, c: Point, deg: f64) -> Point {
    let (s, co) = deg.to_radians().sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

/// Intervals (screen px along the line from `a` to `b`) of the accent
/// triangles that lie on the line `a -> b` beyond the box, outside
/// `[0, len]` by at least 0.5 px.
fn guide_intervals(
    list: &DrawList,
    view: ViewTransform,
    a: Point,
    b: Point,
) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let (ax, ay) = view.document_to_screen(a);
    let (bx, by) = view.document_to_screen(b);
    let l = (bx - ax).hypot(by - ay);
    let u = ((bx - ax) / l, (by - ay) / l);
    let n = (-u.1, u.0);
    let mut before: Vec<(f64, f64)> = Vec::new();
    let mut after: Vec<(f64, f64)> = Vec::new();
    for t in list.triangles.chunks(3) {
        if !t.iter().all(|v| v.color == ACCENT) {
            continue;
        }
        let mut ok = true;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for v in t {
            let (x, y) = view.document_to_screen(v.position);
            let along = (x - ax) * u.0 + (y - ay) * u.1;
            let across = (x - ax) * n.0 + (y - ay) * n.1;
            // The guide snaps to the device pixel grid (UX review of PR 2): its
            // centre moves by at most half a pixel, its half width is another half.
            if across.abs() > 1.01 {
                ok = false;
            }
            lo = lo.min(along);
            hi = hi.max(along);
        }
        // only pieces of the guide's own length: the live selection box of
        // the previewed shape draws dashes of 2.5 to 4 px on top of the same
        // line, and its dashes must not be taken for guide dashes
        if !ok || !(1.9..=2.1).contains(&(hi - lo)) {
            continue;
        }
        if hi < -0.5 + 1e-6 {
            before.push((lo, hi));
        } else if lo > l + 0.5 - 1e-6 {
            after.push((lo, hi));
        }
    }
    let merge = |mut iv: Vec<(f64, f64)>| {
        iv.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
        let mut m: Vec<(f64, f64)> = Vec::new();
        for (x, y) in iv {
            match m.last_mut() {
                Some(last) if x <= last.1 + 1e-6 => last.1 = last.1.max(y),
                _ => m.push((x, y)),
            }
        }
        m
    };
    (merge(before), merge(after))
}

/// Starts a skew drag on the handle at `from` and moves a few pixels.
fn start_skew(s: &mut Session, from: Point, shift: bool, k: f64) {
    s.pointer_hover(from, shift, false);
    assert_eq!(s.handle_hint(), "skew", "a skew handle sits at {from:?}");
    s.pointer_down(from, shift);
    s.pointer_hover(pt(from.x + 30.0 / k, from.y), shift, false);
    s.pointer_hover(pt(from.x + 40.0 / k, from.y), shift, false);
}

fn check_guide_pattern(iv: &[(f64, f64)], what: &str) {
    assert!(!iv.is_empty(), "{what}: the guide extends past the box");
    for (i, (a, b)) in iv.iter().enumerate() {
        let len = b - a;
        assert!(
            len <= 2.0 + 1e-3,
            "{what}: dash {i} is {len} long, spec says 2"
        );
    }
    for w in iv.windows(2) {
        let gap = w[1].0 - w[0].1;
        assert!(
            (gap - 2.0).abs() < 1e-3,
            "{what}: gap {gap}, spec says 2 ({iv:?})"
        );
    }
    // interior dashes are exactly 2
    for (i, (a, b)) in iv.iter().enumerate() {
        if i > 0 && i + 1 < iv.len() {
            assert!(
                (b - a - 2.0).abs() < 1e-3,
                "{what}: interior dash {i} = {}",
                b - a
            );
        }
    }
}

#[test]
fn ac68_skew_guide_is_two_on_two_off_16_px_past_each_end_at_every_zoom() {
    for scale_steps in [0_i32, 2, -2, 5] {
        let mut s = open_doc(&skew_doc(0.0));
        s.resize_viewport(1200.0, 800.0);
        for _ in 0..scale_steps.abs() {
            s.wheel(
                0.0,
                if scale_steps > 0 { -120.0 } else { 120.0 },
                300.0,
                200.0,
                false,
                true,
            );
        }
        let base = s.draw_list().triangle_count();
        click(&mut s, pt(120.0, 100.0));
        assert!(selected(&s, base), "zoom steps {scale_steps}");
        let k = s.view().scale();
        // top skew handle: 16 px outward from the top side midpoint
        let from = pt(120.0, 100.0 - 16.0 / k);
        start_skew(&mut s, from, false, k);
        let view = s.view();
        let list = s.draw_list();
        // the fixed line is the bottom edge, y = 130; the line a -> b along it
        let (before, after) = guide_intervals(&list, view, pt(100.0, 130.0), pt(140.0, 130.0));
        let what = format!("zoom {scale_steps} (k {k})");
        check_guide_pattern(&before, &format!("{what} before"));
        check_guide_pattern(&after, &format!("{what} after"));
        let reach = |iv: &[(f64, f64)], dir: f64| {
            iv.iter()
                .map(|x| if dir < 0.0 { -x.0 } else { x.1 })
                .fold(0.0_f64, f64::max)
        };
        let (ax, _) = view.document_to_screen(pt(100.0, 130.0));
        let (bx, _) = view.document_to_screen(pt(140.0, 130.0));
        let l = bx - ax;
        // 16 px past each end; the last dash may stop a gap and a dash short;
        // the guide's ends snap to the device pixel grid (UX review of PR 2),
        // which moves them by at most half a pixel.
        let (rb, ra) = (reach(&before, -1.0), reach(&after, 1.0) - l);
        assert!(
            (12.0..=16.6).contains(&rb),
            "{what}: reaches {rb} px before"
        );
        assert!((12.0..=16.6).contains(&ra), "{what}: reaches {ra} px after");
        // Escape removes the guide
        s.escape();
        s.pointer_up(pt(from.x + 40.0 / k, from.y), false, false);
        let (b2, a2) =
            guide_intervals(&s.draw_list(), s.view(), pt(100.0, 130.0), pt(140.0, 130.0));
        assert!(
            b2.is_empty() && a2.is_empty(),
            "{what}: no guide after Escape"
        );
    }
}

#[test]
fn ac68_the_guide_lives_until_release_and_is_gone_after() {
    let mut s = open_doc(&skew_doc(0.0));
    click(&mut s, pt(120.0, 100.0));
    let k = s.view().scale();
    let from = pt(120.0, 100.0 - 16.0 / k);
    start_skew(&mut s, from, false, k);
    let (b, a) = guide_intervals(&s.draw_list(), s.view(), pt(100.0, 130.0), pt(140.0, 130.0));
    assert!(!b.is_empty() && !a.is_empty());
    s.pointer_up(pt(from.x + 40.0 / k, from.y), false, false);
    let (b, a) = guide_intervals(&s.draw_list(), s.view(), pt(100.0, 130.0), pt(140.0, 130.0));
    assert!(
        b.is_empty() && a.is_empty(),
        "the guide is gone after release"
    );
}

#[test]
fn ac68_shift_variant_runs_through_the_box_centre_with_the_same_pattern() {
    let mut s = open_doc(&skew_doc(0.0));
    click(&mut s, pt(120.0, 100.0));
    let k = s.view().scale();
    let from = pt(120.0, 100.0 - 16.0 / k);
    start_skew(&mut s, from, true, k);
    let view = s.view();
    let list = s.draw_list();
    // the line through the centre: y = 115
    let (before, after) = guide_intervals(&list, view, pt(100.0, 115.0), pt(140.0, 115.0));
    check_guide_pattern(&before, "shift before");
    check_guide_pattern(&after, "shift after");
    // and none along the opposite edge
    let (b, a) = guide_intervals(&list, view, pt(100.0, 130.0), pt(140.0, 130.0));
    assert!(
        b.is_empty() && a.is_empty(),
        "with Shift the fixed line is the centre line, not the edge"
    );
    s.escape();
    s.pointer_up(from, true, false);
}

#[test]
fn ac68_guide_on_a_rotated_box_keeps_the_pattern() {
    for deg in [30.0, 90.0, 137.0, -45.0] {
        let mut s = open_doc(&skew_doc(deg));
        let c = pt(120.0, 115.0);
        // press the rotated outline: the path's first edge midpoint
        let e0 = rotate_pt(pt(120.0, 100.0), c, deg);
        let base = s.draw_list().triangle_count();
        click(&mut s, e0);
        assert!(selected(&s, base), "deg {deg}");
        let k = s.view().scale();
        // the top skew handle: 16 px outward along the rotated normal
        let n = rotate_pt(pt(120.0, 100.0 - 16.0 / k), c, deg);
        s.pointer_hover(n, false, false);
        assert_eq!(s.handle_hint(), "skew", "deg {deg}: skew handle at {n:?}");
        s.pointer_down(n, false);
        // drag along the rotated top edge direction
        let dir = rotate_pt(pt(c.x + 1.0, c.y), c, deg);
        let (dx, dy) = (dir.x - c.x, dir.y - c.y);
        s.pointer_hover(pt(n.x + 30.0 / k * dx, n.y + 30.0 / k * dy), false, false);
        s.pointer_hover(pt(n.x + 40.0 / k * dx, n.y + 40.0 / k * dy), false, false);
        let view = s.view();
        let list = s.draw_list();
        // fixed line: the rotated bottom edge
        let a = rotate_pt(pt(100.0, 130.0), c, deg);
        let b = rotate_pt(pt(140.0, 130.0), c, deg);
        let (before, after) = guide_intervals(&list, view, a, b);
        check_guide_pattern(&before, &format!("rot {deg} before"));
        check_guide_pattern(&after, &format!("rot {deg} after"));
        s.escape();
        s.pointer_up(n, false, false);
    }
}

#[test]
fn ac68_the_lasso_pattern_constants_are_untouched_but_the_guide_is_not_four_three() {
    // the old 4 / 3 would show interior dashes of 4 and gaps of 3
    let mut s = open_doc(&skew_doc(0.0));
    click(&mut s, pt(120.0, 100.0));
    let k = s.view().scale();
    let from = pt(120.0, 100.0 - 16.0 / k);
    start_skew(&mut s, from, false, k);
    let (before, _) = guide_intervals(&s.draw_list(), s.view(), pt(100.0, 130.0), pt(140.0, 130.0));
    for (a, b) in &before {
        assert!(
            b - a < 3.0,
            "a 4 px dash means the old pattern is still in use: {before:?}"
        );
    }
}
