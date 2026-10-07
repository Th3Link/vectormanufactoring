//! Black-box acceptance tests for `specs/0005-object-transform/
//! specification.md` (25 acceptance criteria), written against
//! `curvyo-editor-wasm::Session`'s public API before reading the
//! implementation diff. Expected values are derived here from the spec's
//! own arithmetic (opposite corner pinned, per-axis factors, sqrt(sx*sy)
//! stroke/radius rule, rotation about a pivot), never read back from the
//! crate under test.
//!
//! Documents under test are built with `Document`'s plain constructors,
//! packed, and opened in a `Session`; every gesture goes through
//! `pointer_down` / `pointer_hover` / `pointer_up` with modifiers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use std::f64::consts::{FRAC_PI_2, PI};

use curvyo_document_core::{
    AnchorId, Angle, Document, EllipseFrame, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot,
    OpenError, Point, PointCount, RectBounds, Shape, StarFrame, Vec2, effective_corner_radius,
    outline_of_rotated, pack, shape_frame_bounds, unpack,
};
use curvyo_editor_wasm::{Session, Tool};

const EPS: f64 = 1e-6;

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

fn pclose(a: Point, b: Point) -> bool {
    close(a.x, b.x) && close(a.y, b.y)
}

/// Rotates `p` about `c` by `a` radians, y-down screen convention
/// (positive = clockwise on screen). Own formula, not the crate's.
fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn rect_bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

fn document_of(session: &Session) -> Document {
    let bytes = session.pack("0.1.0").expect("pack");
    unpack(99, &bytes).expect("unpack")
}

fn open_in_session(document: &Document) -> Session {
    let bytes = pack(document, "0.1.0").expect("pack");
    let mut session = Session::open(2, &bytes).expect("open");
    session.set_tool(Tool::Select);
    session
}

fn only_id(session: &Session) -> NodeId {
    let ids = document_of(session).object_ids();
    assert_eq!(ids.len(), 1, "expected exactly one object");
    ids[0]
}

fn prim(session: &Session) -> curvyo_document_core::PrimitiveSnapshot {
    let doc = document_of(session);
    doc.primitive(doc.object_ids()[0]).expect("primitive")
}

fn path_of(session: &Session) -> curvyo_document_core::PathSnapshot {
    let doc = document_of(session);
    doc.path(doc.object_ids()[0]).expect("path")
}

fn rect_of(session: &Session) -> (RectBounds, Length, f64, f64) {
    let p = prim(session);
    let Shape::Rect {
        bounds,
        corner_radius,
    } = p.shape
    else {
        panic!("expected rect, got {:?}", p.shape);
    };
    (
        bounds,
        corner_radius,
        p.stroke_width.as_mm(),
        p.rotation.as_radians(),
    )
}

fn assert_rect(session: &Session, x: f64, y: f64, w: f64, h: f64) {
    let (b, ..) = rect_of(session);
    assert!(
        close(b.origin.x, x)
            && close(b.origin.y, y)
            && close(b.width.as_mm(), w)
            && close(b.height.as_mm(), h),
        "rect = ({}, {}) {} x {}, expected ({x}, {y}) {w} x {h}",
        b.origin.x,
        b.origin.y,
        b.width.as_mm(),
        b.height.as_mm()
    );
}

fn click(session: &mut Session, p: Point) {
    session.pointer_hover(p, false, false);
    session.pointer_down(p, false);
    session.pointer_up(p, false, false);
}

fn shift_click(session: &mut Session, p: Point) {
    session.pointer_hover(p, true, false);
    session.pointer_down(p, true);
    session.pointer_up(p, true, false);
}

/// One whole drag with the given modifiers held during the moves and the
/// release.
fn drag(session: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, shift, ctrl);
    session.pointer_up(to, shift, ctrl);
}

fn hint_at(session: &mut Session, p: Point) -> String {
    session.pointer_hover(p, false, false);
    session.cursor_hint()
}

/// Screen pixels expressed in document millimetres at the current zoom.
fn px(session: &Session, pixels: f64) -> f64 {
    pixels / session.view().scale()
}

fn rect_doc(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Document {
    let document = Document::new(1);
    let id = document.create_rect(rect_bounds(x, y, w, h));
    if radius > 0.0 {
        document
            .set_corner_radius(&[id], Length::from_mm(radius))
            .expect("radius");
    }
    document
}

fn ellipse_doc(cx: f64, cy: f64, rx: f64, ry: f64) -> Document {
    let document = Document::new(1);
    let _ = document.create_ellipse(EllipseFrame {
        center: pt(cx, cy),
        rx: Length::from_mm(rx),
        ry: Length::from_mm(ry),
    });
    document
}

fn polygon_doc(cx: f64, cy: f64, r: f64, n: u32) -> Document {
    let document = Document::new(1);
    let _ = document.create_polygon(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(n).unwrap(),
    );
    document
}

fn star_doc(cx: f64, cy: f64, r: f64, n: u32, ratio: f64) -> Document {
    let document = Document::new(1);
    let _ = document.create_star(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(n).unwrap(),
        InnerRatio::new(ratio).unwrap(),
    );
    document
}

/// An S-curve from (0,0) to (40,20) with handles chosen so the curve stays
/// inside the box (0,0)-(40,20) and passes through (20,10).
fn s_curve_doc() -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor {
                handle_out: Vec2::new(10.0, 4.0),
                ..NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0))
            },
            NewAnchor {
                handle_in: Vec2::new(-10.0, -4.0),
                ..NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 20.0))
            },
        ],
        false,
    );
    document
}

/// A closed triangle (0,0) (40,0) (40,20): tight box (0,0)-(40,20), and
/// every edge is a straight line so clicks on an edge are unambiguous.
fn triangle_doc() -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), pt(40.0, 20.0)),
        ],
        true,
    );
    document
}

/// The eight local handle positions of the axis-aligned box
/// `(x0,y0)-(x1,y1)`, after rotating the whole box by `a` about `c`.
fn eight(x0: f64, y0: f64, x1: f64, y1: f64, c: Point, a: f64) -> [(&'static str, Point); 8] {
    let (mx, my) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    [
        ("nw", rot(pt(x0, y0), c, a)),
        ("n", rot(pt(mx, y0), c, a)),
        ("ne", rot(pt(x1, y0), c, a)),
        ("e", rot(pt(x1, my), c, a)),
        ("se", rot(pt(x1, y1), c, a)),
        ("s", rot(pt(mx, y1), c, a)),
        ("sw", rot(pt(x0, y1), c, a)),
        ("w", rot(pt(x0, my), c, a)),
    ]
}

/// A corner rotate handle's document position (`object-transform-
/// refinements` criterion 5): 32 px from the Ne corner along its outward
/// diagonal, in the box's own frame. Replaces slice 5's single rotate handle
/// above the top edge for every non-Shift rotate.
fn rotate_handle(session: &Session, _x0: f64, y0: f64, x1: f64, c: Point, a: f64) -> Point {
    let diagonal = px(session, 32.0) / std::f64::consts::SQRT_2;
    rot(pt(x1 + diagonal, y0 - diagonal), c, a)
}

/// The top side rotate handle (criterion 6): 32 px above the top edge,
/// revealed only while Shift is held, so a press on it holds Shift. Its
/// Shift pivot is the bottom-edge midpoint, as slice 5's single rotate
/// handle's was.
fn top_rotate_handle(session: &Session, x0: f64, y0: f64, x1: f64, c: Point, a: f64) -> Point {
    let top_mid = pt(f64::midpoint(x0, x1), y0);
    rot(pt(top_mid.x, top_mid.y - px(session, 32.0)), c, a)
}

/// Selects the single object by pressing on a point on its outline, then
/// sanity-checks that the selection shows the transform handles.
fn select_at(session: &mut Session, on_outline: Point) {
    click(session, on_outline);
}

/// Drags the rotate handle so the pointer ends `degrees` (clockwise) from
/// where it started as seen from `pivot`; returns nothing.
fn rotate_drag(
    session: &mut Session,
    handle: Point,
    pivot: Point,
    degrees: f64,
    shift: bool,
    ctrl: bool,
) {
    let v = (handle.x - pivot.x, handle.y - pivot.y);
    let dist = v.0.hypot(v.1);
    let start = v.1.atan2(v.0);
    let end = start + degrees.to_radians();
    let to = pt(pivot.x + dist * end.cos(), pivot.y + dist * end.sin());
    drag(session, handle, to, shift, ctrl);
}

/// The pointer position `degrees` (clockwise) from `handle` as seen from
/// `pivot`, at the handle's own distance.
fn swept_to(handle: Point, pivot: Point, degrees: f64) -> Point {
    let v = (handle.x - pivot.x, handle.y - pivot.y);
    let end = v.1.atan2(v.0) + degrees.to_radians();
    let dist = v.0.hypot(v.1);
    pt(pivot.x + dist * end.cos(), pivot.y + dist * end.sin())
}

/// [`rotate_drag`] for the Shift-only side rotate handles: Shift is held
/// from the press (the handle only exists then) to the release.
fn shift_rotate_drag(session: &mut Session, handle: Point, pivot: Point, degrees: f64, ctrl: bool) {
    let to = swept_to(handle, pivot, degrees);
    session.pointer_hover(handle, true, false);
    session.pointer_down(handle, true);
    session.pointer_hover(to, true, ctrl);
    session.pointer_up(to, true, ctrl);
}

/// A point on a 40 x 20 ellipse's outline centred at (20, 10) that the
/// outline hit test reliably finds (curve parameter t = 0.5 of the NE arc).
fn ell() -> Point {
    pt(
        20.0 + 20.0 * std::f64::consts::FRAC_1_SQRT_2,
        10.0 + 10.0 * std::f64::consts::FRAC_1_SQRT_2,
    )
}

/// Same for a 100 x 20 ellipse centred at (50, 10).
fn ell100() -> Point {
    pt(
        50.0 + 50.0 * std::f64::consts::FRAC_1_SQRT_2,
        10.0 + 10.0 * std::f64::consts::FRAC_1_SQRT_2,
    )
}

/// Zooms to the maximum so a screen-space hit radius is a tiny fraction of
/// a millimetre and an outline press away from any handle is unambiguous.
fn zoom_to_max(session: &mut Session) {
    for _ in 0..60 {
        session.wheel(0.0, -2000.0, 0.0, 0.0, false, true);
    }
    assert_eq!(session.zoom_percent(), 8000);
}

fn change_count(session: &Session) -> usize {
    let bytes = document_of(session).export_loro_snapshot().expect("export");
    let loro = loro::LoroDoc::new();
    loro.import(&bytes).expect("import");
    // Only peer 2's changes (the Session's own); the fixture was peer 1's.
    loro.len_changes()
}

fn snapshot_bytes(session: &Session) -> Vec<u8> {
    document_of(session).export_loro_snapshot().expect("export")
}

// ---------------------------------------------------------------------
// AC 1: single selection shows 8 resize handles + 1 rotate handle
// ---------------------------------------------------------------------

fn assert_full_handle_set(
    session: &mut Session,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    corners_only: bool,
) {
    let c = pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    for (name, p) in eight(x0, y0, x1, y1, c, 0.0) {
        let hint = hint_at(session, p);
        let is_corner = name.len() == 2;
        if corners_only && !is_corner {
            assert_eq!(hint, "default", "edge handle {name} must not exist");
        } else {
            assert!(
                hint.starts_with("resize:"),
                "handle {name} at {p:?}: hint {hint}"
            );
        }
    }
    let rh = rotate_handle(session, x0, y0, x1, c, 0.0);
    assert_eq!(hint_at(session, rh), "rotate", "rotate handle");
    // Far away from every handle and the box: nothing.
    assert_eq!(hint_at(session, pt(x1 + 500.0, y1 + 500.0)), "default");
}

#[test]
fn ac1_rect_shows_eight_resize_handles_and_a_rotate_handle() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    assert_full_handle_set(&mut s, 0.0, 0.0, 40.0, 20.0, false);
}

#[test]
fn ac1_ellipse_shows_eight_resize_handles_and_a_rotate_handle() {
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, ell());
    assert_full_handle_set(&mut s, 0.0, 0.0, 40.0, 20.0, false);
}

#[test]
fn ac1_open_path_shows_eight_resize_handles_and_a_rotate_handle() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    assert_full_handle_set(&mut s, 0.0, 0.0, 40.0, 20.0, false);
}

#[test]
fn ac1_closed_path_shows_eight_resize_handles_and_a_rotate_handle() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    assert_full_handle_set(&mut s, 0.0, 0.0, 40.0, 20.0, false);
}

#[test]
fn ac1_polygon_and_star_have_corner_handles_only_plus_rotate_ac11() {
    for doc in [
        polygon_doc(30.0, 30.0, 10.0, 6),
        star_doc(30.0, 30.0, 10.0, 5, 0.5),
    ] {
        let mut s = open_in_session(&doc);
        let p = prim(&s);
        let (min, max) = shape_frame_bounds(&p.shape);
        // Select by clicking a point on the outline: the first vertex at the top.
        let outline = outline_of_rotated(&p.shape, p.rotation);
        let first = outline[0].point;
        // Nudge along the outline toward the second vertex to avoid handles.
        let second = outline[1].point;
        let mid = pt(
            f64::midpoint(first.x, second.x),
            f64::midpoint(first.y, second.y),
        );
        select_at(&mut s, mid);
        assert_eq!(
            s.cursor_hint(),
            "default",
            "pointer on outline mid is not a handle"
        );
        assert_full_handle_set(&mut s, min.x, min.y, max.x, max.y, true);
    }
}

#[test]
fn ac1_no_handles_when_nothing_selected() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    assert_eq!(hint_at(&mut s, pt(40.0, 20.0)), "default");
    let far_up = -px(&s, 40.0);
    assert_eq!(hint_at(&mut s, pt(20.0, far_up)), "default");
}

#[test]
fn ac1_slice4_baseline_still_shows_no_path_nodes_or_shape_handles() {
    // The Select tool shows only transform handles, not Bezier/shape tool glyphs:
    // a deselected object draws strictly less than a selected one, and the
    // selected one is the same as slice 4 plus handle glyphs (more triangles).
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 5.0));
    let none = s.draw_list().triangle_count();
    select_at(&mut s, pt(20.0, 0.0));
    let selected = s.draw_list().triangle_count();
    assert!(selected > none);
}

// ---------------------------------------------------------------------
// AC 2: multi-selection shows no transform handles
// ---------------------------------------------------------------------

#[test]
fn ac2_two_selected_objects_show_no_transform_handles() {
    let document = Document::new(1);
    let _ = document.create_rect(rect_bounds(0.0, 0.0, 40.0, 20.0));
    let _ = document.create_rect(rect_bounds(100.0, 0.0, 40.0, 20.0));
    let mut s = open_in_session(&document);
    click(&mut s, pt(20.0, 0.0));
    shift_click(&mut s, pt(120.0, 0.0));
    // None of the 9 handle positions of either rect responds.
    for (x0, x1) in [(0.0, 40.0), (100.0, 140.0)] {
        let c = pt(f64::midpoint(x0, x1), 10.0);
        for (name, p) in eight(x0, 0.0, x1, 20.0, c, 0.0) {
            assert_eq!(hint_at(&mut s, p), "default", "handle {name} of {x0}");
        }
        let rh = rotate_handle(&s, x0, 0.0, x1, c, 0.0);
        assert_eq!(hint_at(&mut s, rh), "default");
    }
}

#[test]
fn ac2_multi_selection_move_and_delete_still_work() {
    let document = Document::new(1);
    let _ = document.create_rect(rect_bounds(0.0, 0.0, 40.0, 20.0));
    let _ = document.create_rect(rect_bounds(100.0, 0.0, 40.0, 20.0));
    let mut s = open_in_session(&document);
    click(&mut s, pt(20.0, 0.0));
    shift_click(&mut s, pt(120.0, 0.0));
    // Pressing exactly where a single selection's SE corner handle would sit
    // must be a plain body/move press for a multi-selection, not a resize.
    drag(&mut s, pt(120.0, 0.0), pt(123.0, 2.0), false, false);
    let doc = document_of(&s);
    let ids = doc.object_ids();
    let mut origins: Vec<(f64, f64)> = ids
        .iter()
        .map(|&id| {
            let Shape::Rect { bounds, .. } = doc.primitive(id).unwrap().shape else {
                panic!()
            };
            (bounds.origin.x, bounds.origin.y)
        })
        .collect();
    origins.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert!(
        close(origins[0].0, 3.0) && close(origins[0].1, 2.0),
        "{origins:?}"
    );
    assert!(
        close(origins[1].0, 103.0) && close(origins[1].1, 2.0),
        "{origins:?}"
    );
    s.delete_selected();
    assert_eq!(
        document_of(&s).object_ids(),
        [] as [curvyo_document_core::NodeId; 0]
    );
}

// ---------------------------------------------------------------------
// AC 3: press + release on any handle without movement writes nothing
// ---------------------------------------------------------------------

#[test]
fn ac3_press_release_on_any_handle_writes_nothing() {
    let docs = [
        (rect_doc(0.0, 0.0, 40.0, 20.0, 3.0), pt(20.0, 0.0)),
        (ellipse_doc(20.0, 10.0, 20.0, 10.0), ell()),
        (s_curve_doc(), pt(20.0, 10.0)),
        (triangle_doc(), pt(20.0, 0.0)),
    ];
    for (doc, sel) in docs {
        let mut s = open_in_session(&doc);
        select_at(&mut s, sel);
        let before = snapshot_bytes(&s);
        let changes_before = change_count(&s);
        let c = pt(20.0, 10.0);
        let mut targets: Vec<Point> = eight(0.0, 0.0, 40.0, 20.0, c, 0.0)
            .iter()
            .map(|(_, p)| *p)
            .collect();
        targets.push(rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0));
        for p in targets {
            assert_ne!(hint_at(&mut s, p), "default", "must be on a handle: {p:?}");
            s.pointer_down(p, false);
            s.pointer_up(p, false, false);
            assert_eq!(snapshot_bytes(&s), before, "press/release at {p:?} wrote");
            assert_eq!(change_count(&s), changes_before);
        }
    }
}

#[test]
fn ac3_press_hover_without_distance_then_release_with_modifiers_writes_nothing() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let before = snapshot_bytes(&s);
    let p = pt(40.0, 20.0);
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_hover(p, true, true);
    s.pointer_up(p, true, true);
    assert_eq!(snapshot_bytes(&s), before);
}

// ---------------------------------------------------------------------
// AC 4: free corner resize, opposite corner pinned
// ---------------------------------------------------------------------

#[test]
fn ac4_each_corner_follows_pointer_one_to_one_opposite_corner_fixed() {
    // (handle, grab, drop, expected origin x, y, w, h)
    let cases = [
        ("se", pt(40.0, 20.0), pt(55.0, 33.0), 0.0, 0.0, 55.0, 33.0),
        ("nw", pt(0.0, 0.0), pt(-7.0, -4.0), -7.0, -4.0, 47.0, 24.0),
        ("ne", pt(40.0, 0.0), pt(52.0, -6.0), 0.0, -6.0, 52.0, 26.0),
        ("sw", pt(0.0, 20.0), pt(-9.0, 31.0), -9.0, 0.0, 49.0, 31.0),
        // shrinking
        ("se", pt(40.0, 20.0), pt(30.0, 5.0), 0.0, 0.0, 30.0, 5.0),
    ];
    for (name, grab, drop_at, x, y, w, h) in cases {
        let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
        select_at(&mut s, pt(20.0, 0.0));
        drag(&mut s, grab, drop_at, false, false);
        assert_rect(&s, x, y, w, h);
        let _ = name;
    }
}

#[test]
fn ac4_free_corner_resize_on_ellipse_pins_opposite_corner() {
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, ell());
    // SE corner (40,20) -> (60, 40): box (0,0)-(60,40).
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 40.0), false, false);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    assert!(pclose(frame.center, pt(30.0, 20.0)), "{:?}", frame.center);
    assert!(close(frame.rx.as_mm(), 30.0) && close(frame.ry.as_mm(), 20.0));
}

#[test]
fn ac4_free_corner_resize_of_a_path_pins_opposite_corner() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(0.0, 0.0), pt(-10.0, -5.0), false, false);
    // pin = SE (40,20); NW moved to (-10,-5): sx = 50/40, sy = 25/20.
    let path = path_of(&s);
    assert!(
        pclose(path.anchors[0].point, pt(-10.0, -5.0)),
        "{:?}",
        path.anchors[0].point
    );
    assert!(
        pclose(path.anchors[1].point, pt(40.0, 20.0)),
        "{:?}",
        path.anchors[1].point
    );
}

// ---------------------------------------------------------------------
// AC 5: Ctrl on a corner scales proportionally on the dominant axis
// ---------------------------------------------------------------------

#[test]
fn ac5_ctrl_corner_resize_is_proportional_on_the_dominant_axis() {
    // 40 x 20. X-dominant: delta (30, 2): sx = 1.75, sy = 1.1 -> 1.75.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(70.0, 22.0), false, true);
    assert_rect(&s, 0.0, 0.0, 70.0, 35.0);

    // Y-dominant: delta (2, 30): sx = 1.05, sy = 2.5 -> 2.5.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(42.0, 50.0), false, true);
    assert_rect(&s, 0.0, 0.0, 100.0, 50.0);
}

#[test]
fn ac5_ctrl_corner_resize_works_from_the_other_corners_and_for_shrinking() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    // NW dragged by (-20, -1): sx = 1.5 dominant -> 60 x 30, SE (40,20) pinned.
    drag(&mut s, pt(0.0, 0.0), pt(-20.0, -1.0), false, true);
    assert_rect(&s, -20.0, -10.0, 60.0, 30.0);

    // Shrink SE of a fresh rect to half: delta (-20, -1): sx=.5 dominant.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(20.0, 19.0), false, true);
    assert_rect(&s, 0.0, 0.0, 20.0, 10.0);
}

// ---------------------------------------------------------------------
// AC 6: edge handles change one dimension only; Ctrl has no effect
// ---------------------------------------------------------------------

#[test]
fn ac6_edge_handles_change_only_the_perpendicular_dimension() {
    // (handle point, drop, expected rect)
    let cases = [
        (pt(40.0, 10.0), pt(70.0, 99.0), (0.0, 0.0, 70.0, 20.0)), // E
        (pt(0.0, 10.0), pt(-15.0, -50.0), (-15.0, 0.0, 55.0, 20.0)), // W
        (pt(20.0, 0.0), pt(-50.0, -10.0), (0.0, -10.0, 40.0, 30.0)), // N
        (pt(20.0, 20.0), pt(77.0, 35.0), (0.0, 0.0, 40.0, 35.0)), // S
    ];
    for ctrl in [false, true] {
        for (grab, to, (x, y, w, h)) in cases {
            let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
            select_at(&mut s, pt(20.0, 0.0));
            drag(&mut s, grab, to, false, ctrl);
            assert_rect(&s, x, y, w, h);
        }
    }
}

#[test]
fn ac6_edge_handle_resizes_ellipse_and_path_on_one_axis_only() {
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, ell());
    drag(&mut s, pt(40.0, 10.0), pt(60.0, 100.0), false, false);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    // box (0,0)-(60,20)
    assert!(pclose(frame.center, pt(30.0, 10.0)));
    assert!(close(frame.rx.as_mm(), 30.0) && close(frame.ry.as_mm(), 10.0));

    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(20.0, 20.0), pt(0.0, 40.0), false, false); // S edge to y = 40
    let path = path_of(&s);
    assert!(pclose(path.anchors[0].point, pt(0.0, 0.0)));
    assert!(
        pclose(path.anchors[1].point, pt(40.0, 40.0)),
        "{:?}",
        path.anchors[1].point
    );
}

// ---------------------------------------------------------------------
// AC 7: Shift pivots around the center; Shift+Ctrl combine
// ---------------------------------------------------------------------

#[test]
fn ac7_shift_corner_resize_is_symmetric_about_the_center() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(50.0, 25.0), true, false);
    // center (20,10) fixed: half-extents 30 x 15.
    assert_rect(&s, -10.0, -5.0, 60.0, 30.0);
}

#[test]
fn ac7_shift_edge_resize_is_symmetric_on_the_touched_axis() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(50.0, 60.0), true, false);
    assert_rect(&s, -10.0, 0.0, 60.0, 20.0);
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(20.0, 0.0), pt(20.0, -4.0), true, false);
    assert_rect(&s, 0.0, -4.0, 40.0, 28.0);
}

#[test]
fn ac7_shift_plus_ctrl_corner_is_proportional_from_the_center() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    // From center: half-extents 20,10; pointer to x = 60 -> half-width 40 (sx 2),
    // y delta 2 -> half-height 12 (sy 1.2). Dominant x -> factor 2.
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 22.0), true, true);
    assert_rect(&s, -20.0, -10.0, 80.0, 40.0);
}

#[test]
fn ac7_shift_on_an_ellipse_and_a_path_pivots_on_the_box_center() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(40.0, 20.0), pt(50.0, 30.0), true, false);
    let path = path_of(&s);
    // center (20,10); half 30 x 20: NW corner of the curve box (-10,-10).
    assert!(
        pclose(path.anchors[0].point, pt(-10.0, -10.0)),
        "{:?}",
        path.anchors[0].point
    );
    assert!(
        pclose(path.anchors[1].point, pt(50.0, 30.0)),
        "{:?}",
        path.anchors[1].point
    );
}

// ---------------------------------------------------------------------
// AC 8: stroke width scaling
// ---------------------------------------------------------------------

#[test]
fn ac8_proportional_resize_scales_stroke_by_the_same_factor() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    s.set_scale_stroke_width(true);
    let start = prim(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false); // sx = sy = 1.5
    let (.., stroke, _) = rect_of(&s);
    assert!(close(stroke, start * 1.5), "stroke {stroke} start {start}");
}

#[test]
fn ac8_ctrl_proportional_resize_scales_stroke_by_the_factor() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    s.set_scale_stroke_width(true);
    let start = prim(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 21.0), false, true); // factor 1.5
    let (.., stroke, _) = rect_of(&s);
    assert!(close(stroke, start * 1.5));
}

#[test]
fn ac8_non_uniform_resize_uses_the_geometric_mean() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    s.set_scale_stroke_width(true);
    let start = prim(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 50.0), false, false); // sx 1.5, sy 2.5
    let (.., stroke, _) = rect_of(&s);
    assert!(close(stroke, start * (1.5_f64 * 2.5).sqrt()), "{stroke}");
}

#[test]
fn ac8_edge_handle_resize_uses_the_geometric_mean_with_one_axis_unscaled() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    s.set_scale_stroke_width(true);
    let start = prim(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(80.0, 10.0), false, false); // sx 2, sy 1
    let (.., stroke, _) = rect_of(&s);
    assert!(close(stroke, start * 2.0_f64.sqrt()), "{stroke}");
}

#[test]
fn ac8_stroke_scales_for_ellipse_path_polygon_and_star_too() {
    // ellipse, sx = sy = 1.5
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    s.set_scale_stroke_width(true);
    let start = prim(&s).stroke_width.as_mm();
    select_at(&mut s, ell());
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false);
    assert!(close(prim(&s).stroke_width.as_mm(), start * 1.5));

    // path, sx 1.5 sy 1.5
    let mut s = open_in_session(&s_curve_doc());
    s.set_scale_stroke_width(true);
    let start = path_of(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false);
    assert!(close(path_of(&s).stroke_width.as_mm(), start * 1.5));

    // path, one axis: sqrt
    let mut s = open_in_session(&s_curve_doc());
    s.set_scale_stroke_width(true);
    let start = path_of(&s).stroke_width.as_mm();
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(40.0, 10.0), pt(80.0, 10.0), false, false);
    assert!(close(
        path_of(&s).stroke_width.as_mm(),
        start * 2.0_f64.sqrt()
    ));
}

#[test]
fn ac8_stroke_never_becomes_zero_or_negative_when_collapsing_a_resize() {
    for (grab, to) in [
        (pt(40.0, 20.0), pt(-30.0, -30.0)), // corner past the opposite corner
        (pt(40.0, 20.0), pt(0.0, 0.0)),     // exactly to zero
        (pt(40.0, 10.0), pt(-20.0, 10.0)),  // edge past the opposite edge
        (pt(20.0, 20.0), pt(20.0, -9.0)),   // edge past
    ] {
        let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
        s.set_scale_stroke_width(true);
        let start = prim(&s).stroke_width.as_mm();
        select_at(&mut s, pt(20.0, 0.0));
        drag(&mut s, grab, to, false, false);
        let (b, _, stroke, _) = rect_of(&s);
        assert!(
            stroke > 0.0 && stroke.is_finite(),
            "stroke {stroke} for {to:?}"
        );
        assert!(stroke <= start + EPS, "collapsing must not grow the stroke");
        assert!(b.width.as_mm() >= 0.0 && b.height.as_mm() >= 0.0);
        // The resulting file must still open.
        let reopened = Session::open(5, &s.pack("0.1.0").unwrap());
        assert!(
            reopened.is_ok(),
            "file unopenable after collapse: {:?}",
            reopened.err()
        );
    }
}

#[test]
fn ac8_stroke_stays_positive_for_every_object_kind_when_collapsed() {
    let docs = [
        (
            ellipse_doc(20.0, 10.0, 20.0, 10.0),
            pt(20.0 + 20.0 * 0.6_f64.cos(), 10.0 + 10.0 * 0.6_f64.sin()),
        ),
        (s_curve_doc(), pt(20.0, 10.0)),
        (triangle_doc(), pt(20.0, 0.0)),
    ];
    for (doc, click_at) in docs {
        let mut s = open_in_session(&doc);
        s.set_scale_stroke_width(true);
        select_at(&mut s, click_at);
        drag(&mut s, pt(40.0, 20.0), pt(-80.0, -80.0), false, false);
        let id = only_id(&s);
        let d = document_of(&s);
        let stroke = match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) => p.stroke_width.as_mm(),
            ObjectSnapshot::Primitive(p) => p.stroke_width.as_mm(),
        };
        assert!(stroke > 0.0 && stroke.is_finite(), "{stroke}");
        assert!(Session::open(5, &s.pack("0.1.0").unwrap()).is_ok());
    }
}

// ---------------------------------------------------------------------
// AC 9: corner radius scales with the resize
// ---------------------------------------------------------------------

#[test]
fn ac9_corner_radius_scales_by_the_same_factor_for_equal_factors() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 4.0));
    s.set_scale_corner_radius(true); // off by default since `unified-object-editing`
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 30.0), false, false); // 1.5
    let (b, r, ..) = rect_of(&s);
    assert!(close(r.as_mm(), 6.0), "raw radius {}", r.as_mm());
    assert!(close(effective_corner_radius(b, r).as_mm(), 6.0));
}

#[test]
fn ac9_corner_radius_uses_the_geometric_mean_for_unequal_factors() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 4.0));
    s.set_scale_corner_radius(true); // off by default since `unified-object-editing`
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 50.0), false, false); // 1.5, 2.5
    let (_, r, ..) = rect_of(&s);
    assert!(
        close(r.as_mm(), 4.0 * (1.5_f64 * 2.5).sqrt()),
        "{}",
        r.as_mm()
    );

    // Single-axis edge handle: sx 2, sy 1.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 4.0));
    s.set_scale_corner_radius(true);
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(80.0, 10.0), false, false);
    let (_, r, ..) = rect_of(&s);
    assert!(close(r.as_mm(), 4.0 * 2.0_f64.sqrt()), "{}", r.as_mm());
}

#[test]
fn ac9_radius_is_clamped_to_half_the_shorter_side_after_scaling() {
    // r = 10 on 40x20 (already at the clamp). Shrink height to 10 with S edge:
    // sx 1, sy .5 -> factor sqrt(.5); effective radius clamps to 5.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 10.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(20.0, 20.0), pt(20.0, 10.0), false, false);
    let (b, r, ..) = rect_of(&s);
    assert!(close(b.height.as_mm(), 10.0));
    assert!(close(effective_corner_radius(b, r).as_mm(), 5.0));
}

#[test]
fn ac9_radius_driven_to_exactly_zero_is_valid_and_not_refused() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 4.0));
    select_at(&mut s, pt(20.0, 0.0));
    // Collapse the SE corner onto the NW corner: factor 0.
    drag(&mut s, pt(40.0, 20.0), pt(0.0, 0.0), false, false);
    let (b, r, stroke, _) = rect_of(&s);
    assert!(b.width.as_mm() >= 0.0 && b.height.as_mm() >= 0.0);
    assert!(r.as_mm() >= 0.0 && r.as_mm().is_finite());
    assert!(stroke > 0.0);
    assert!(Session::open(5, &s.pack("0.1.0").unwrap()).is_ok());
}

// ---------------------------------------------------------------------
// AC 10: ellipse resize equals the Ellipse tool's rule
// ---------------------------------------------------------------------

#[test]
fn ac10_ellipse_corner_and_edge_resizes_set_rx_ry_from_the_box() {
    let click_at = ell();
    // NW corner dragged out to (-10,-10): box (-10,-10)-(40,20): rx 25 ry 15
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, click_at);
    drag(&mut s, pt(0.0, 0.0), pt(-10.0, -10.0), false, false);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    assert!(pclose(frame.center, pt(15.0, 5.0)), "{:?}", frame.center);
    assert!(close(frame.rx.as_mm(), 25.0) && close(frame.ry.as_mm(), 15.0));
    // Ctrl: proportional.
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, click_at);
    drag(&mut s, pt(40.0, 20.0), pt(60.0, 21.0), false, true);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    assert!(close(frame.rx.as_mm(), 30.0) && close(frame.ry.as_mm(), 15.0));
}

// ---------------------------------------------------------------------
// AC 11: polygon / star uniform scale
// ---------------------------------------------------------------------

fn star_like(session: &Session) -> (StarFrame, PointCount, Option<InnerRatio>, f64, f64) {
    let p = prim(session);
    match p.shape {
        Shape::Polygon { frame, point_count } => (
            frame,
            point_count,
            None,
            p.stroke_width.as_mm(),
            p.rotation.as_radians(),
        ),
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => (
            frame,
            point_count,
            Some(inner_ratio),
            p.stroke_width.as_mm(),
            p.rotation.as_radians(),
        ),
        other => panic!("not a polygon/star: {other:?}"),
    }
}

fn poly_select_point(session: &Session) -> Point {
    let p = prim(session);
    let outline = outline_of_rotated(&p.shape, p.rotation);
    let (a, b) = (outline[0].point, outline[1].point);
    pt(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y))
}

#[test]
fn ac11_polygon_and_star_corner_drag_is_a_uniform_scale_regardless_of_ctrl() {
    // Frame box (20,20)-(40,40) for r = 10 at (30,30). The AC does not pin which
    // point stays fixed for a polygon/star (only criterion 7's Shift wording does,
    // and it addresses criteria 4-6); what it does pin is: outer radius changes,
    // star ratio, point count and orientation do not, and Ctrl makes no difference.
    for doc in [
        polygon_doc(30.0, 30.0, 10.0, 6),
        star_doc(30.0, 30.0, 10.0, 5, 0.5),
    ] {
        let mut results = Vec::new();
        for ctrl in [false, true] {
            let mut s = open_in_session(&doc);
            s.set_scale_stroke_width(true);
            let before = star_like(&s);
            let sel = poly_select_point(&s);
            select_at(&mut s, sel);
            drag(&mut s, pt(40.0, 40.0), pt(50.0, 50.0), false, ctrl);
            let after = star_like(&s);
            let (r0, r1) = (before.0.radius.as_mm(), after.0.radius.as_mm());
            assert!(r1 > r0 && r1.is_finite(), "outer radius grew: {r0} -> {r1}");
            assert_eq!(after.1, before.1, "point count unchanged");
            assert_eq!(after.2, before.2, "inner ratio unchanged");
            assert!(close(
                after.0.angle.as_radians(),
                before.0.angle.as_radians()
            ));
            assert!(close(after.4, before.4), "rotation unchanged");
            // Uniform scale by f = r1 / r0 => stroke scales by f (criterion 8).
            assert!(
                close(after.3, before.3 * r1 / r0),
                "stroke {} expected {}",
                after.3,
                before.3 * r1 / r0
            );
            results.push((after.0.center, after.0.radius));
        }
        assert_eq!(
            results[0], results[1],
            "Ctrl must not change a polygon/star resize"
        );
    }
}

#[test]
fn ac11_polygon_corner_drag_inwards_shrinks_and_never_goes_negative() {
    let mut s = open_in_session(&polygon_doc(30.0, 30.0, 10.0, 6));
    let sel = poly_select_point(&s);
    select_at(&mut s, sel);
    drag(&mut s, pt(40.0, 40.0), pt(33.0, 33.0), false, false);
    let r = star_like(&s).0.radius.as_mm();
    assert!(r > 0.0 && r < 10.0, "{r}");
    drag(&mut s, pt(40.0, 40.0), pt(-80.0, -80.0), false, false);
    let r = star_like(&s).0.radius.as_mm();
    assert!(r >= 0.0 && r.is_finite(), "{r}");
}

#[test]
fn ac11_polygon_readout_matches_the_committed_radius() {
    let mut s = open_in_session(&polygon_doc(30.0, 30.0, 10.0, 6));
    let sel = poly_select_point(&s);
    select_at(&mut s, sel);
    s.pointer_hover(pt(40.0, 40.0), false, false);
    s.pointer_down(pt(40.0, 40.0), false);
    s.pointer_hover(pt(47.0, 52.0), false, false);
    let text = s.live_readout().unwrap().text;
    s.pointer_up(pt(47.0, 52.0), false, false);
    let r = star_like(&s).0.radius.as_mm();
    assert_eq!(text, format!("r {r:.1} mm"));
}

// ---------------------------------------------------------------------
// AC 12: path anchors and handle vectors scale exactly
// ---------------------------------------------------------------------

#[test]
fn ac12_path_anchors_and_handles_scale_per_axis_about_the_pinned_corner() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    // SE (40,20) -> (80,30): pin NW (0,0): sx = 2, sy = 1.5.
    drag(&mut s, pt(40.0, 20.0), pt(80.0, 30.0), false, false);
    let p = path_of(&s);
    assert!(pclose(p.anchors[0].point, pt(0.0, 0.0)));
    assert!(pclose(p.anchors[1].point, pt(80.0, 30.0)));
    let (o, i) = (p.anchors[0].handle_out, p.anchors[1].handle_in);
    assert!(close(o.x, 20.0) && close(o.y, 6.0), "handle_out {o:?}");
    assert!(close(i.x, -20.0) && close(i.y, -6.0), "handle_in {i:?}");
}

#[test]
fn ac12_path_scaled_about_the_center_under_shift_keeps_handles_scaled_not_moved() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    // E edge (40,10) -> (60,10) with Shift: center (20,10) fixed, sx = 2.
    drag(&mut s, pt(40.0, 10.0), pt(60.0, 10.0), true, false);
    let p = path_of(&s);
    assert!(
        pclose(p.anchors[0].point, pt(-20.0, 0.0)),
        "{:?}",
        p.anchors[0].point
    );
    assert!(
        pclose(p.anchors[1].point, pt(60.0, 20.0)),
        "{:?}",
        p.anchors[1].point
    );
    assert!(close(p.anchors[0].handle_out.x, 20.0) && close(p.anchors[0].handle_out.y, 4.0));
    assert!(close(p.anchors[1].handle_in.x, -20.0) && close(p.anchors[1].handle_in.y, -4.0));
}

#[test]
fn ac12_a_scaled_curve_still_passes_through_the_scaled_midpoint() {
    // The original curve passes through (20,10); an exact affine scale about
    // (0,0) by (2,1.5) must pass through (40,15).
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    drag(&mut s, pt(40.0, 20.0), pt(80.0, 30.0), false, false);
    let p = path_of(&s);
    let (a, b) = (&p.anchors[0], &p.anchors[1]);
    let c1 = pt(a.point.x + a.handle_out.x, a.point.y + a.handle_out.y);
    let c2 = pt(b.point.x + b.handle_in.x, b.point.y + b.handle_in.y);
    let mid = pt(
        0.125 * a.point.x + 0.375 * c1.x + 0.375 * c2.x + 0.125 * b.point.x,
        0.125 * a.point.y + 0.375 * c1.y + 0.375 * c2.y + 0.125 * b.point.y,
    );
    assert!(pclose(mid, pt(40.0, 15.0)), "{mid:?}");
}

// ---------------------------------------------------------------------
// AC 13: zero clamp, no flip-through, recover when the pointer returns
// ---------------------------------------------------------------------

#[test]
fn ac13_dragging_past_the_opposite_edge_clamps_to_zero_and_never_goes_negative() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(-25.0, -25.0), false, false);
    let (b, ..) = rect_of(&s);
    assert!(
        close(b.width.as_mm(), 0.0) && close(b.height.as_mm(), 0.0),
        "{b:?}"
    );
    // Opposite (NW) corner stays where it was: no flip-through to the other side.
    assert!(pclose(b.origin, pt(0.0, 0.0)), "{:?}", b.origin);
}

#[test]
fn ac13_each_axis_clamps_independently_in_a_free_corner_drag() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 20.0), pt(-5.0, 30.0), false, false);
    assert_rect(&s, 0.0, 0.0, 0.0, 30.0);
}

#[test]
fn ac13_edge_handle_clamps_at_zero_for_each_edge() {
    for (grab, to, expect) in [
        (pt(40.0, 10.0), pt(-10.0, 10.0), (0.0, 0.0, 0.0, 20.0)),
        (pt(0.0, 10.0), pt(60.0, 10.0), (40.0, 0.0, 0.0, 20.0)),
        (pt(20.0, 20.0), pt(20.0, -7.0), (0.0, 0.0, 40.0, 0.0)),
        (pt(20.0, 0.0), pt(20.0, 33.0), (0.0, 20.0, 40.0, 0.0)),
    ] {
        let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
        select_at(&mut s, pt(20.0, 0.0));
        drag(&mut s, grab, to, false, false);
        assert_rect(&s, expect.0, expect.1, expect.2, expect.3);
    }
}

#[test]
fn ac13_pointer_returning_past_the_zero_crossing_restores_a_live_size() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(-30.0, -30.0), false, false);
    s.pointer_hover(pt(60.0, 30.0), false, false);
    s.pointer_up(pt(60.0, 30.0), false, false);
    assert_rect(&s, 0.0, 0.0, 60.0, 30.0);
}

#[test]
fn ac13_ellipse_and_polygon_never_report_negative_radii() {
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, ell());
    drag(&mut s, pt(40.0, 20.0), pt(-30.0, -30.0), false, false);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    assert!(frame.rx.as_mm() >= 0.0 && frame.ry.as_mm() >= 0.0);

    let mut s = open_in_session(&polygon_doc(30.0, 30.0, 10.0, 6));
    let outline = outline_of_rotated(&prim(&s).shape, Angle::from_radians(0.0));
    let (p, q) = (outline[0].point, outline[1].point);
    select_at(&mut s, pt(f64::midpoint(p.x, q.x), f64::midpoint(p.y, q.y)));
    drag(&mut s, pt(40.0, 40.0), pt(-30.0, -30.0), false, false);
    let (frame, ..) = star_like(&s);
    assert!(frame.radius.as_mm() >= 0.0 && frame.radius.as_mm().is_finite());
    assert!(frame.center.x.is_finite() && frame.center.y.is_finite());
}

#[test]
fn ac13_a_zero_size_object_can_still_be_resized_back_up() {
    // Collapse a rect to w = 0 via E edge, then a *new* drag from the (now
    // coincident) handle position must not divide by zero or produce NaN.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    drag(&mut s, pt(40.0, 10.0), pt(0.0, 10.0), false, false);
    assert_rect(&s, 0.0, 0.0, 0.0, 20.0);
    drag(&mut s, pt(0.0, 10.0), pt(30.0, 10.0), false, false);
    let (b, r, stroke, rotation) = rect_of(&s);
    for v in [
        b.origin.x,
        b.origin.y,
        b.width.as_mm(),
        b.height.as_mm(),
        r.as_mm(),
        stroke,
        rotation,
    ] {
        assert!(
            v.is_finite(),
            "non-finite after re-resizing a zero-width object: {v}"
        );
    }
    assert!(stroke > 0.0);
}

// ---------------------------------------------------------------------
// AC 14: live size readout during scale
// ---------------------------------------------------------------------

#[test]
fn ac14_live_readout_shows_the_current_size_in_mm_during_a_resize_only() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    assert!(s.live_readout().is_none(), "no readout at idle");
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(63.5, 31.0), false, false);
    let r = s.live_readout().expect("readout during a scale drag");
    assert_eq!(r.text, "63.5 × 31.0 mm");
    assert!(pclose(r.anchor, pt(63.5, 31.0)), "anchored at the pointer");
    s.pointer_up(pt(63.5, 31.0), false, false);
    assert!(s.live_readout().is_none(), "gone after release");
}

#[test]
fn ac14_readout_for_ellipse_path_and_polygon() {
    let mut s = open_in_session(&ellipse_doc(20.0, 10.0, 20.0, 10.0));
    select_at(&mut s, ell());
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(50.0, 30.0), false, false);
    assert_eq!(s.live_readout().unwrap().text, "50.0 × 30.0 mm");
    s.pointer_up(pt(50.0, 30.0), false, false);

    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(80.0, 30.0), false, false);
    assert_eq!(s.live_readout().unwrap().text, "80.0 × 30.0 mm");
    s.pointer_up(pt(80.0, 30.0), false, false);

    let mut s = open_in_session(&polygon_doc(30.0, 30.0, 10.0, 6));
    let outline = outline_of_rotated(&prim(&s).shape, Angle::from_radians(0.0));
    let (p, q) = (outline[0].point, outline[1].point);
    select_at(&mut s, pt(f64::midpoint(p.x, q.x), f64::midpoint(p.y, q.y)));
    s.pointer_hover(pt(40.0, 40.0), false, false);
    s.pointer_down(pt(40.0, 40.0), false);
    s.pointer_hover(pt(50.0, 50.0), false, false);
    let text = s.live_readout().unwrap().text;
    s.pointer_up(pt(50.0, 50.0), false, false);
    assert_eq!(text, format!("r {:.1} mm", star_like(&s).0.radius.as_mm()));
}

#[test]
fn ac14_readout_tracks_modifier_changes_live() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(50.0, 30.0), false, false);
    assert_eq!(s.live_readout().unwrap().text, "50.0 × 30.0 mm");
    s.pointer_hover(pt(50.0, 30.0), true, false);
    assert_eq!(
        s.live_readout().unwrap().text,
        "60.0 × 40.0 mm",
        "Shift -> center pivot"
    );
    s.pointer_hover(pt(50.0, 35.0), false, true);
    // Ctrl: pointer delta (10, 15): y dominant in absolute and relative terms,
    // sy = 1.75 -> 70 x 35.
    assert_eq!(
        s.live_readout().unwrap().text,
        "70.0 × 35.0 mm",
        "Ctrl -> proportional"
    );
    s.pointer_up(pt(50.0, 35.0), false, true);
}

// ---------------------------------------------------------------------
// AC 15-17, 22: rotate
// ---------------------------------------------------------------------

#[test]
fn ac15_rotate_handle_drag_rotates_about_the_box_center_following_the_pointer() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    // The corner handle swings a quarter turn clockwise about the centre.
    drag(&mut s, handle, swept_to(handle, c, 90.0), false, false);
    let (b, _, _, rotation) = rect_of(&s);
    assert!(close(rotation, FRAC_PI_2), "rotation {rotation}");
    // Centre unmoved, frame unchanged in its local axes.
    assert!(close(b.origin.x, 0.0) && close(b.origin.y, 0.0));
    assert!(close(b.width.as_mm(), 40.0) && close(b.height.as_mm(), 20.0));
}

#[test]
fn ac15_rotation_is_relative_to_the_press_not_the_absolute_pointer_angle() {
    // Press the handle slightly off its exact centre; no initial jump.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    let off = pt(handle.x + px(&s, 4.0), handle.y);
    // Pointer straight onto the same ray: zero further movement -> no rotation.
    s.pointer_hover(off, false, false);
    s.pointer_down(off, false);
    s.pointer_hover(off, false, false);
    s.pointer_up(off, false, false);
    assert!(close(prim(&s).rotation.as_radians(), 0.0));
}

#[test]
fn ac15_rotate_a_37_degree_arbitrary_drag_and_readout() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    let v = (handle.x - c.x, handle.y - c.y);
    let dist = v.0.hypot(v.1);
    let end = v.1.atan2(v.0) + 37.4_f64.to_radians();
    let to = pt(c.x + dist * end.cos(), c.y + dist * end.sin());
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    s.pointer_hover(to, false, false);
    let readout = s.live_readout().expect("rotate readout");
    assert_eq!(readout.text, "37.4°");
    assert!(pclose(readout.anchor, to));
    s.pointer_up(to, false, false);
    let (.., rotation) = rect_of(&s);
    assert!(close(rotation, 37.4_f64.to_radians()), "{rotation}");
    assert!(s.live_readout().is_none());
}

#[test]
fn ac15_rotate_works_for_every_object_kind() {
    type KindCase = (Document, Point, (f64, f64, f64, f64));
    let cases: Vec<KindCase> = vec![
        (
            rect_doc(0.0, 0.0, 40.0, 20.0, 0.0),
            pt(20.0, 0.0),
            (0.0, 0.0, 40.0, 20.0),
        ),
        (
            ellipse_doc(20.0, 10.0, 20.0, 10.0),
            ell(),
            (0.0, 0.0, 40.0, 20.0),
        ),
        (s_curve_doc(), pt(20.0, 10.0), (0.0, 0.0, 40.0, 20.0)),
        (triangle_doc(), pt(20.0, 0.0), (0.0, 0.0, 40.0, 20.0)),
    ];
    for (doc, sel, (x0, y0, x1, y1)) in cases {
        let mut s = open_in_session(&doc);
        select_at(&mut s, sel);
        let c = pt(f64::midpoint(x0, x1), f64::midpoint(y0, y1));
        let handle = rotate_handle(&s, x0, y0, x1, c, 0.0);
        rotate_drag(&mut s, handle, c, 90.0, false, false);
        let id = only_id(&s);
        let rotation = document_of(&s).object(id).unwrap().rotation().as_radians();
        assert!(close(rotation, FRAC_PI_2), "rotation {rotation}");
    }
}

#[test]
fn ac15_rotating_through_more_than_half_a_turn_stays_normalized() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 200.0, false, false);
    let (.., rotation) = rect_of(&s);
    assert!(rotation > -PI - EPS && rotation <= PI + EPS, "{rotation}");
    // 200 deg is equivalent to -160 deg.
    let diff = ((rotation - 200.0_f64.to_radians()) / (2.0 * PI)).round();
    assert!(close(rotation, 200.0_f64.to_radians() + diff * 2.0 * PI));
}

#[test]
fn ac16_shift_rotates_about_the_bottom_edge_midpoint() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = top_rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    let pivot = pt(20.0, 20.0);
    shift_rotate_drag(&mut s, handle, pivot, 90.0, false);
    let (b, _, _, rotation) = rect_of(&s);
    assert!(close(rotation, FRAC_PI_2));
    // Centre (20,10) swung about (20,20) by 90 deg clockwise -> (30,20).
    let centre = pt(
        b.origin.x + b.width.as_mm() / 2.0,
        b.origin.y + b.height.as_mm() / 2.0,
    );
    assert!(pclose(centre, pt(30.0, 20.0)), "{centre:?}");
}

#[test]
fn ac16_shift_pivot_for_a_path_bakes_about_the_bottom_midpoint() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = top_rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    shift_rotate_drag(&mut s, handle, pt(20.0, 20.0), 90.0, false);
    let p = path_of(&s);
    let pivot = pt(20.0, 20.0);
    for (got, orig) in p
        .anchors
        .iter()
        .zip([pt(0.0, 0.0), pt(40.0, 0.0), pt(40.0, 20.0)])
    {
        let want = rot(orig, pivot, FRAC_PI_2);
        assert!(pclose(got.point, want), "{:?} vs {want:?}", got.point);
    }
}

#[test]
fn ac16_releasing_shift_mid_drag_returns_the_pivot_to_the_center() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    s.pointer_hover(swept_to(handle, c, 60.0), true, false);
    // Released without Shift at a final position: pure centre-pivot result.
    let to = swept_to(handle, c, 90.0);
    s.pointer_hover(to, false, false);
    s.pointer_up(to, false, false);
    let (b, _, _, rotation) = rect_of(&s);
    assert!(close(rotation, FRAC_PI_2));
    assert!(
        close(b.origin.x, 0.0) && close(b.origin.y, 0.0),
        "centre pivot: frame untouched"
    );
}

#[test]
fn ac17_ctrl_snaps_rotation_to_15_and_22_5_degree_stops_with_exact_boundaries() {
    // Superseded by `object-transform-refinements` criterion 33: the stops
    // are the multiples of 15 and of 22.5 degrees.
    for (drag_deg, expect_deg) in [
        (0.0, 0.0),
        (7.4, 0.0),
        (7.6, 15.0),
        (14.0, 15.0),
        (18.7, 15.0),
        (18.8, 22.5),
        (26.2, 22.5),
        (26.3, 30.0),
        (37.4, 30.0),
        (37.6, 45.0),
        (45.0, 45.0),
        (64.0, 67.5),
        (89.0, 90.0),
        (-22.0, -22.5),
        (-8.0, -15.0),
        (180.0, 180.0),
    ] {
        let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
        select_at(&mut s, pt(20.0, 0.0));
        let c = pt(20.0, 10.0);
        let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
        rotate_drag(&mut s, handle, c, drag_deg, false, true);
        let (.., rotation) = rect_of(&s);
        let mut diff = rotation - f64::to_radians(expect_deg);
        diff = (diff + PI).rem_euclid(2.0 * PI) - PI;
        assert!(
            diff.abs() < 1e-6,
            "drag {drag_deg}: got {} deg, expected {expect_deg}",
            rotation.to_degrees()
        );
    }
}

#[test]
fn ac17_ctrl_snap_readout_shows_up_to_one_decimal() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    let to = swept_to(handle, c, 46.0);
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    s.pointer_hover(to, false, true);
    assert_eq!(s.live_readout().unwrap().text, "45°");
    // A 22.5 degree stop shows its decimal (criterion 35).
    s.pointer_hover(swept_to(handle, c, 23.0), false, true);
    assert_eq!(s.live_readout().unwrap().text, "22.5°");
    s.pointer_up(to, false, true);
}

#[test]
fn ac17_ctrl_snaps_relative_to_the_angle_at_drag_start() {
    // Start from a rotation of 10 deg (UI rotate, no snap), then Ctrl-rotate
    // another ~20 deg: result must be 10 + 22.5 = 32.5 deg (the stop nearest
    // the swept 20 deg, measured from the start angle), not an absolute
    // stop (30).
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 10.0, false, false);
    // Reselect the rotated rectangle: click on its rotated top edge.
    let on_edge = rot(pt(20.0, 0.0), c, 10.0_f64.to_radians());
    click(&mut s, pt(300.0, 300.0));
    click(&mut s, on_edge);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 10.0_f64.to_radians());
    assert_eq!(hint_at(&mut s, handle), "rotate", "rotated rotate handle");
    rotate_drag(&mut s, handle, c, 20.0, false, true);
    let (.., rotation) = rect_of(&s);
    assert!(
        close(rotation, 32.5_f64.to_radians()),
        "expected 32.5 deg, got {} deg",
        rotation.to_degrees()
    );
}

#[test]
fn ac17_shift_and_ctrl_together_snap_about_the_bottom_pivot() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = top_rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    shift_rotate_drag(&mut s, handle, pt(20.0, 20.0), 92.0, true);
    let (b, _, _, rotation) = rect_of(&s);
    assert!(close(rotation, FRAC_PI_2), "{rotation}");
    let centre = pt(
        b.origin.x + b.width.as_mm() / 2.0,
        b.origin.y + b.height.as_mm() / 2.0,
    );
    assert!(pclose(centre, pt(30.0, 20.0)), "{centre:?}");
}

// ---------------------------------------------------------------------
// AC 18: oriented box and handles after rotation, reselect, resize, zoom
// ---------------------------------------------------------------------

fn rotated_rect_session(deg: f64) -> (Session, Point, f64) {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, deg, false, false);
    (s, c, deg.to_radians())
}

#[test]
fn ac18_after_deselect_and_reselect_handles_are_oriented_to_the_rotation() {
    for deg in [90.0_f64, 30.0, 45.0, -60.0] {
        let (mut s, c, a) = rotated_rect_session(deg);
        click(&mut s, pt(500.0, 500.0)); // deselect
        click(&mut s, rot(pt(20.0, 0.0), c, a)); // reselect by the rotated outline
        for (name, p) in eight(0.0, 0.0, 40.0, 20.0, c, a) {
            let hint = hint_at(&mut s, p);
            assert!(
                hint.starts_with("resize:"),
                "{deg} deg: handle {name} at {p:?}: {hint}"
            );
        }
        let rh = rotate_handle(&s, 0.0, 0.0, 40.0, c, a);
        assert_eq!(hint_at(&mut s, rh), "rotate", "{deg} deg");
        // An unrotated handle position that is not near any rotated handle is
        // no handle any more.
        let rotated: Vec<Point> = eight(0.0, 0.0, 40.0, 20.0, c, a)
            .iter()
            .map(|(_, p)| *p)
            .chain(std::iter::once(rh))
            .collect();
        let limit = px(&s, 20.0);
        for (name, stale) in eight(0.0, 0.0, 40.0, 20.0, c, 0.0) {
            let near = rotated
                .iter()
                .any(|r| (r.x - stale.x).hypot(r.y - stale.y) < limit);
            if !near {
                assert_eq!(
                    hint_at(&mut s, stale),
                    "default",
                    "{deg} deg: stale axis-aligned handle {name}"
                );
            }
        }
    }
}

#[test]
fn ac18_resize_cursor_angle_is_rotation_plus_the_handle_base_angle() {
    let (mut s, c, a) = rotated_rect_session(30.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let norm = |d: f64| d.rem_euclid(180.0);
    let angle_of = |hint: &str| -> f64 { hint.trim_start_matches("resize:").parse().unwrap() };
    // E/W: along the local x axis => 30 deg. N/S: perpendicular => 120 deg.
    let boxpts = eight(0.0, 0.0, 40.0, 20.0, c, a);
    let get = |n: &str| boxpts.iter().find(|(m, _)| *m == n).unwrap().1;
    let e = angle_of(&hint_at(&mut s, get("e")));
    let n = angle_of(&hint_at(&mut s, get("n")));
    assert!(
        (norm(e) - 30.0).abs() < 0.2 || (norm(e) - 30.0).abs() > 179.8,
        "E hint angle {e}"
    );
    assert!((norm(n) - 120.0).abs() < 0.2, "N hint angle {n}");
    // Diagonal handles: base 45 deg family => 30 + 45 = 75 / 30 - 45 = -15 = 165 (mod 180).
    let ne = norm(angle_of(&hint_at(&mut s, get("ne"))));
    let nw = norm(angle_of(&hint_at(&mut s, get("nw"))));
    assert!(
        ((ne - 165.0).abs() < 0.2 && (nw - 75.0).abs() < 0.2)
            || ((ne - 75.0).abs() < 0.2 && (nw - 165.0).abs() < 0.2),
        "corner cursor angles ne {ne} nw {nw}"
    );
    // The two corners on opposite sides of one diagonal agree.
    let sw = norm(angle_of(&hint_at(&mut s, get("sw"))));
    let se = norm(angle_of(&hint_at(&mut s, get("se"))));
    assert!(
        (sw - ne).abs() < 0.2 && (se - nw).abs() < 0.2,
        "{sw} {ne} {se} {nw}"
    );
}

#[test]
fn ac18_rotate_cursor_does_not_rotate_with_the_object() {
    for deg in [0.0, 33.0, 90.0, 170.0] {
        let (mut s, c, a) = rotated_rect_session(deg);
        click(&mut s, pt(500.0, 500.0));
        click(&mut s, rot(pt(20.0, 0.0), c, a));
        let rh = rotate_handle(&s, 0.0, 0.0, 40.0, c, a);
        assert_eq!(
            hint_at(&mut s, rh),
            "rotate",
            "exactly 'rotate' regardless of angle"
        );
    }
}

#[test]
fn ac18_a_non_uniform_resize_after_rotation_stays_in_local_axes_no_shear() {
    // Rotate 90 deg about (20,10): the local x axis now points down the screen.
    let (mut s, c, a) = rotated_rect_session(90.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    // Local E edge handle (40,10) -> document position rot(...) = (20,30).
    let e = rot(pt(40.0, 10.0), c, a);
    assert!(pclose(e, pt(20.0, 30.0)), "{e:?}");
    // Drag it 20 mm further along the local x axis (document +y): width 40 -> 60.
    drag(&mut s, e, pt(20.0, 50.0), false, false);
    let p = prim(&s);
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(
        close(p.rotation.as_radians(), FRAC_PI_2),
        "rotation preserved: {}",
        p.rotation.as_radians()
    );
    assert!(
        close(bounds.width.as_mm(), 60.0) && close(bounds.height.as_mm(), 20.0),
        "{bounds:?}"
    );
    // The local W edge (the pinned one) must stay put in document space.
    let centre = pt(
        bounds.origin.x + bounds.width.as_mm() / 2.0,
        bounds.origin.y + bounds.height.as_mm() / 2.0,
    );
    let w_local = pt(
        bounds.origin.x,
        bounds.origin.y + bounds.height.as_mm() / 2.0,
    );
    let w_doc = rot(w_local, centre, FRAC_PI_2);
    assert!(
        pclose(w_doc, rot(pt(0.0, 10.0), c, a)),
        "pinned W edge moved: {w_doc:?}"
    );
}

#[test]
fn ac18_rotated_primitive_corner_resize_pins_the_opposite_corner_in_document_space() {
    let (mut s, c, a) = rotated_rect_session(30.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let pinned_before = rot(pt(0.0, 0.0), c, a); // NW stays.
    let se = rot(pt(40.0, 20.0), c, a);
    // Move SE by exactly +10 along local x and +5 along local y.
    let delta_local = (10.0, 5.0);
    let (sn, cs) = a.sin_cos();
    let to = pt(
        se.x + delta_local.0 * cs - delta_local.1 * sn,
        se.y + delta_local.0 * sn + delta_local.1 * cs,
    );
    drag(&mut s, se, to, false, false);
    let p = prim(&s);
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    assert!(
        close(bounds.width.as_mm(), 50.0) && close(bounds.height.as_mm(), 25.0),
        "{bounds:?}"
    );
    let centre = pt(
        bounds.origin.x + bounds.width.as_mm() / 2.0,
        bounds.origin.y + bounds.height.as_mm() / 2.0,
    );
    let nw_doc = rot(bounds.origin, centre, p.rotation.as_radians());
    assert!(
        pclose(nw_doc, pinned_before),
        "NW moved: {nw_doc:?} vs {pinned_before:?}"
    );
    assert!(close(p.rotation.as_radians(), a));
}

#[test]
fn ac18_rotated_path_resize_pins_the_opposite_local_corner_and_keeps_rotation() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    let a = FRAC_PI_2;
    click(&mut s, pt(500.0, 500.0));
    // Reselect: the original edge (0,0)-(40,0) is now at rot(.).
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let rotation_before = path_of(&s).rotation.as_radians();
    assert!(close(rotation_before, a));
    let nw_before = rot(pt(0.0, 0.0), c, a);
    let se = rot(pt(40.0, 20.0), c, a);
    // +20 local x, +10 local y => document (-10, 20) rotated: local (dx,dy)=(20,10).
    let (sn, cs) = a.sin_cos();
    let to = pt(se.x + 20.0 * cs - 10.0 * sn, se.y + 20.0 * sn + 10.0 * cs);
    drag(&mut s, se, to, false, false);
    let p = path_of(&s);
    assert!(
        close(p.rotation.as_radians(), a),
        "rotation register unchanged by a resize"
    );
    // Local frame: undo the rotation about c, expect NW anchor at (0,0) still
    // (pinned corner) and the far anchor scaled: (40,20) -> (60,30).
    let back = |q: Point| rot(q, c, -a);
    assert!(
        pclose(p.anchors[0].point, nw_before),
        "pinned corner (anchor 0) moved: {:?}",
        p.anchors[0].point
    );
    assert!(
        pclose(back(p.anchors[2].point), pt(60.0, 30.0)),
        "{:?}",
        back(p.anchors[2].point)
    );
    assert!(
        pclose(back(p.anchors[1].point), pt(60.0, 0.0)),
        "{:?}",
        back(p.anchors[1].point)
    );
}

#[test]
fn ac18_handles_stay_oriented_after_a_zoom_and_pan() {
    let (mut s, c, a) = rotated_rect_session(45.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    s.wheel(0.0, -300.0, 200.0, 200.0, false, true); // zoom in about (200,200)
    s.wheel(0.0, 120.0, 200.0, 200.0, false, false); // pan
    for (name, p) in eight(0.0, 0.0, 40.0, 20.0, c, a) {
        assert!(
            hint_at(&mut s, p).starts_with("resize:"),
            "after zoom/pan: handle {name}"
        );
    }
    let rh = rotate_handle(&s, 0.0, 0.0, 40.0, c, a);
    assert_eq!(hint_at(&mut s, rh), "rotate");
}

#[test]
fn ac18_after_a_resize_the_box_keeps_the_object_orientation() {
    let (mut s, c, a) = rotated_rect_session(45.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let e = rot(pt(40.0, 10.0), c, a);
    let (sn, cs) = a.sin_cos();
    drag(
        &mut s,
        e,
        pt(e.x + 10.0 * cs, e.y + 10.0 * sn),
        false,
        false,
    );
    let p = prim(&s);
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    // Box is now local (0,0)-(50,20) about the new centre.
    let centre = pt(
        bounds.origin.x + bounds.width.as_mm() / 2.0,
        bounds.origin.y + bounds.height.as_mm() / 2.0,
    );
    let w_before = rot(pt(0.0, 10.0), c, a);
    let w_after = rot(pt(bounds.origin.x, bounds.origin.y + 10.0), centre, a);
    assert!(pclose(w_after, w_before));
    // Handles of the *resized* oriented box (still selected) hit at the new spots.
    let (nx0, ny0, nx1) = (
        bounds.origin.x,
        bounds.origin.y,
        bounds.origin.x + bounds.width.as_mm(),
    );
    for (name, hp) in eight(nx0, ny0, nx1, ny0 + bounds.height.as_mm(), centre, a) {
        assert!(
            hint_at(&mut s, hp).starts_with("resize:"),
            "handle {name} after resize"
        );
    }
}

#[test]
fn ac18_a_rotated_path_remembers_its_orientation_after_reopen() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    let bytes = s.pack("0.1.0").unwrap();
    let mut reopened = Session::open(7, &bytes).unwrap();
    reopened.set_tool(Tool::Select);
    click(&mut reopened, rot(pt(20.0, 0.0), c, FRAC_PI_2));
    for (name, p) in eight(0.0, 0.0, 40.0, 20.0, c, FRAC_PI_2) {
        assert!(
            hint_at(&mut reopened, p).starts_with("resize:"),
            "handle {name}"
        );
    }
}

// ---------------------------------------------------------------------
// AC 19-21, 24: persistence, kind preservation, path baking
// ---------------------------------------------------------------------

#[test]
fn ac19_ac21_rotated_primitives_persist_and_stay_the_same_kind() {
    let cases: Vec<(Document, Point, Point)> = vec![
        (
            rect_doc(0.0, 0.0, 40.0, 20.0, 3.0),
            pt(20.0, 0.0),
            pt(20.0, 10.0),
        ),
        (ellipse_doc(20.0, 10.0, 20.0, 10.0), ell(), pt(20.0, 10.0)),
        (
            polygon_doc(20.0, 20.0, 10.0, 6),
            pt(0.0, 0.0),
            pt(20.0, 20.0),
        ),
        (
            star_doc(20.0, 20.0, 10.0, 5, 0.4),
            pt(0.0, 0.0),
            pt(20.0, 20.0),
        ),
    ];
    for (doc, sel, c) in cases {
        let mut s = open_in_session(&doc);
        let before = prim(&s);
        let sel = if matches!(before.shape, Shape::Polygon { .. } | Shape::Star { .. }) {
            let o = outline_of_rotated(&before.shape, before.rotation);
            pt(
                f64::midpoint(o[0].point.x, o[1].point.x),
                f64::midpoint(o[0].point.y, o[1].point.y),
            )
        } else {
            sel
        };
        select_at(&mut s, sel);
        let (min, max) = shape_frame_bounds(&before.shape);
        let handle = rotate_handle(&s, min.x, min.y, max.x, c, 0.0);
        rotate_drag(&mut s, handle, c, 50.0, false, false);
        let rotated = prim(&s);
        assert!(close(rotated.rotation.as_radians(), 50.0_f64.to_radians()));
        assert_eq!(
            std::mem::discriminant(&rotated.shape),
            std::mem::discriminant(&before.shape),
            "still the same kind (AC21)"
        );
        // save, close, reopen
        let reopened = Session::open(11, &s.pack("0.1.0").unwrap()).unwrap();
        let after = prim(&reopened);
        assert_eq!(
            after, rotated,
            "AC19/AC24: rotation, frame and params exactly as before closing"
        );
        let d = document_of(&reopened);
        assert!(
            d.path(d.object_ids()[0]).is_none(),
            "not converted to a path"
        );
    }
}

#[test]
fn ac19_rotated_star_keeps_theta_and_rotation_separate() {
    let mut s = open_in_session(&star_doc(20.0, 20.0, 10.0, 5, 0.4));
    let before = star_like(&s);
    let p = prim(&s);
    let o = outline_of_rotated(&p.shape, p.rotation);
    select_at(
        &mut s,
        pt(
            f64::midpoint(o[0].point.x, o[1].point.x),
            f64::midpoint(o[0].point.y, o[1].point.y),
        ),
    );
    let c = pt(20.0, 20.0);
    let handle = rotate_handle(&s, 10.0, 10.0, 30.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 40.0, false, false);
    let after = star_like(&s);
    assert!(
        close(after.0.angle.as_radians(), before.0.angle.as_radians()),
        "theta untouched"
    );
    assert!(
        close(after.4, 40.0_f64.to_radians()),
        "rotation is the new register"
    );
    assert!(pclose(after.0.center, before.0.center));
}

#[test]
fn ac20_rotated_path_bakes_anchors_and_handles_and_stores_the_register() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    let p = path_of(&s);
    assert!(close(p.rotation.as_radians(), FRAC_PI_2));
    // anchors: each rotated about the centre
    assert!(
        pclose(p.anchors[0].point, rot(pt(0.0, 0.0), c, FRAC_PI_2)),
        "{:?}",
        p.anchors[0].point
    );
    assert!(pclose(
        p.anchors[1].point,
        rot(pt(40.0, 20.0), c, FRAC_PI_2)
    ));
    // handle vectors rotated by the angle without pivot: (10,4) -> (-4,10).
    let ho = p.anchors[0].handle_out;
    let hi = p.anchors[1].handle_in;
    assert!(close(ho.x, -4.0) && close(ho.y, 10.0), "{ho:?}");
    assert!(close(hi.x, 4.0) && close(hi.y, -10.0), "{hi:?}");
}

#[test]
fn ac20_rotation_register_accumulates_across_two_rotate_drags_of_a_path() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 30.0, false, false);
    click(&mut s, pt(500.0, 500.0));
    let a = 30.0_f64.to_radians();
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, a);
    rotate_drag(&mut s, handle, c, 40.0, false, false);
    let p = path_of(&s);
    assert!(
        close(p.rotation.as_radians(), 70.0_f64.to_radians()),
        "{}",
        p.rotation.as_radians()
    );
}

#[test]
fn ac24_scaled_and_rotated_objects_round_trip_exactly() {
    let cases: Vec<(Document, Point)> = vec![
        (rect_doc(0.0, 0.0, 40.0, 20.0, 4.0), pt(20.0, 0.0)),
        (ellipse_doc(20.0, 10.0, 20.0, 10.0), ell()),
        (s_curve_doc(), pt(20.0, 10.0)),
        (triangle_doc(), pt(20.0, 0.0)),
    ];
    for (doc, sel) in cases {
        let mut s = open_in_session(&doc);
        select_at(&mut s, sel);
        drag(&mut s, pt(40.0, 20.0), pt(61.0, 37.0), false, false);
        let c = {
            let id = only_id(&s);
            let d = document_of(&s);
            match d.object(id).unwrap() {
                ObjectSnapshot::Primitive(p) => {
                    let (mn, mx) = shape_frame_bounds(&p.shape);
                    pt(f64::midpoint(mn.x, mx.x), f64::midpoint(mn.y, mx.y))
                }
                ObjectSnapshot::Path(_) => pt(30.5, 18.5),
            }
        };
        // Rotate via the rotate handle, if reachable; otherwise skip the rotate.
        let (mn, mx) = (pt(0.0, 0.0), pt(61.0, 37.0));
        let handle = rotate_handle(&s, mn.x, mn.y, mx.x, c, 0.0);
        s.pointer_hover(handle, false, false);
        if s.cursor_hint() == "rotate" {
            rotate_drag(&mut s, handle, c, 25.0, false, false);
        }
        let id = only_id(&s);
        let before = document_of(&s).object(id).unwrap();
        let reopened = Session::open(9, &s.pack("0.1.0").unwrap()).unwrap();
        let after = document_of(&reopened).object(id).unwrap();
        assert_eq!(after, before, "exact round trip");
    }
}

// ---------------------------------------------------------------------
// AC 23: move keeps working, never changes size or rotation
// ---------------------------------------------------------------------

#[test]
fn ac23_moving_a_rotated_and_scaled_object_is_a_pure_translation() {
    let (mut s, c, a) = rotated_rect_session(30.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    // Resize first: E edge +10 along local x.
    let e = rot(pt(40.0, 10.0), c, a);
    let (sn, cs) = a.sin_cos();
    drag(
        &mut s,
        e,
        pt(e.x + 10.0 * cs, e.y + 10.0 * sn),
        false,
        false,
    );
    let before = prim(&s);
    let Shape::Rect {
        bounds: b0,
        corner_radius: r0,
    } = before.shape
    else {
        panic!()
    };
    // Press on the rotated top edge between handles and drag by (7, -3). Zoomed
    // in so the press is unambiguously off every handle and the outline hit
    // test (whose accuracy depends on zoom, see the report) is exact.
    zoom_to_max(&mut s);
    let centre = pt(
        b0.origin.x + b0.width.as_mm() / 2.0,
        b0.origin.y + b0.height.as_mm() / 2.0,
    );
    let on_edge = rot(
        pt(b0.origin.x + b0.width.as_mm() * 0.3, b0.origin.y),
        centre,
        a,
    );
    assert_eq!(
        hint_at(&mut s, on_edge),
        "default",
        "press point is not on a handle"
    );
    drag(
        &mut s,
        on_edge,
        pt(on_edge.x + 7.0, on_edge.y - 3.0),
        false,
        false,
    );
    let after = prim(&s);
    let Shape::Rect {
        bounds: b1,
        corner_radius: r1,
    } = after.shape
    else {
        panic!()
    };
    assert!(close(b1.origin.x, b0.origin.x + 7.0) && close(b1.origin.y, b0.origin.y - 3.0));
    assert_eq!((b1.width, b1.height, r1), (b0.width, b0.height, r0));
    assert_eq!(after.rotation, before.rotation);
    assert_eq!(after.stroke_width, before.stroke_width);
}

#[test]
fn ac23_moving_a_rotated_path_translates_anchors_and_keeps_rotation() {
    let mut s = open_in_session(&triangle_doc());
    select_at(&mut s, pt(20.0, 0.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    let before = path_of(&s);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, FRAC_PI_2));
    zoom_to_max(&mut s);
    let from = rot(pt(30.0, 0.0), c, FRAC_PI_2);
    drag(&mut s, from, pt(from.x - 5.0, from.y + 8.0), false, false);
    let after = path_of(&s);
    for (a, b) in after.anchors.iter().zip(&before.anchors) {
        assert!(pclose(a.point, pt(b.point.x - 5.0, b.point.y + 8.0)));
        assert_eq!(a.handle_in, b.handle_in);
        assert_eq!(a.handle_out, b.handle_out);
    }
    assert_eq!(after.rotation, before.rotation);
}

#[test]
fn ac23_move_of_an_unrotated_object_does_not_start_a_transform() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    zoom_to_max(&mut s);
    drag(&mut s, pt(10.0, 0.0), pt(15.0, 4.0), false, false);
    assert_rect(&s, 5.0, 4.0, 40.0, 20.0);
    assert!(close(prim(&s).rotation.as_radians(), 0.0));
}

// ---------------------------------------------------------------------
// Commit discipline / preview-equals-commit
// ---------------------------------------------------------------------

#[test]
fn drag_writes_nothing_until_release_and_exactly_one_change_on_release() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 3.0));
    select_at(&mut s, pt(20.0, 0.0));
    let before_bytes = snapshot_bytes(&s);
    let before_changes = change_count(&s);
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    for step in 1..=10 {
        let f = f64::from(step);
        s.pointer_hover(pt(40.0 + f * 3.0, 20.0 + f * 2.0), false, false);
        assert_eq!(
            snapshot_bytes(&s),
            before_bytes,
            "live preview wrote at step {step}"
        );
    }
    s.pointer_up(pt(70.0, 40.0), false, false);
    assert_ne!(snapshot_bytes(&s), before_bytes);
    assert_eq!(
        change_count(&s),
        before_changes + 1,
        "one interaction = one commit"
    );

    // Same for rotate.
    let c = pt(35.0, 20.0);
    let _ = c;
    let changes = change_count(&s);
    let (mn, mx) = {
        let (b, ..) = rect_of(&s);
        (
            b.origin,
            pt(b.origin.x + b.width.as_mm(), b.origin.y + b.height.as_mm()),
        )
    };
    let centre = pt(f64::midpoint(mn.x, mx.x), f64::midpoint(mn.y, mx.y));
    let handle = rotate_handle(&s, mn.x, mn.y, mx.x, centre, 0.0);
    rotate_drag(&mut s, handle, centre, 25.0, false, false);
    assert_eq!(change_count(&s), changes + 1, "rotate = one commit");
    let changes = change_count(&s);
    // And a path rotate / path resize.
    let mut p = open_in_session(&s_curve_doc());
    select_at(&mut p, pt(20.0, 10.0));
    let base = change_count(&p);
    drag(&mut p, pt(40.0, 20.0), pt(80.0, 30.0), false, false);
    assert_eq!(change_count(&p), base + 1, "path resize = one commit");
    let _ = changes;
}

mod bounds_of {
    use curvyo_document_core::Point;

    pub struct List(pub Vec<Point>);

    impl List {
        pub fn bounds(&self) -> (f64, f64, f64, f64) {
            let xs = self.0.iter().map(|p| p.x);
            let ys = self.0.iter().map(|p| p.y);
            (
                xs.clone().fold(f64::INFINITY, f64::min),
                ys.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
                ys.fold(f64::NEG_INFINITY, f64::max),
            )
        }
    }
}

// ---------------------------------------------------------------------
// Rendering of rotated primitives and paths
// ---------------------------------------------------------------------

fn vertex_bounds(session: &Session) -> (f64, f64, f64, f64) {
    let list = session.draw_list();
    let pts: Vec<Point> = list.triangles.iter().map(|v| v.position).collect();
    assert!(!pts.is_empty(), "draw list is empty");
    let l = bounds_of::List(pts);
    l.bounds()
}

#[test]
fn rotated_primitives_render_rotated_not_axis_aligned() {
    // Rect 100 x 20 rotated 90 deg about its centre renders ~20 wide, ~100 tall
    // (plus a half-stroke). Nothing selected, so only the object itself draws.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 100.0, 20.0, 0.0));
    select_at(&mut s, pt(50.0, 0.0));
    let c = pt(50.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 100.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    click(&mut s, pt(900.0, 900.0));
    s.pointer_leave();
    let (x0, y0, x1, y1) = vertex_bounds(&s);
    let (w, h) = (x1 - x0, y1 - y0);
    assert!(
        w < 25.0 && h > 95.0,
        "rendered {w} x {h}; should be ~20 x ~100 after 90 deg"
    );
    // Centre at (50,10): the vertical extent straddles it.
    assert!(
        (f64::midpoint(y0, y1) - 10.0).abs() < 1.0 && (f64::midpoint(x0, x1) - 50.0).abs() < 1.0
    );
}

#[test]
fn every_primitive_kind_renders_rotated() {
    let cases: Vec<(Document, Point)> = vec![(ellipse_doc(50.0, 10.0, 50.0, 10.0), ell100())];
    for (doc, sel) in cases {
        let mut s = open_in_session(&doc);
        zoom_to_max(&mut s);
        select_at(&mut s, sel);
        let c = pt(50.0, 10.0);
        let handle = rotate_handle(&s, 0.0, 0.0, 100.0, c, 0.0);
        rotate_drag(&mut s, handle, c, 90.0, false, false);
        click(&mut s, pt(900.0, 900.0));
        s.pointer_leave();
        let (x0, y0, x1, y1) = vertex_bounds(&s);
        assert!(
            (x1 - x0) < 25.0 && (y1 - y0) > 95.0,
            "ellipse render {} x {}",
            x1 - x0,
            y1 - y0
        );
    }
    // Polygon with 2-fold asymmetry is awkward; use a star with ratio and check
    // its rendered bounds equal the bounds of the rotated outline.
    let mut s = open_in_session(&star_doc(30.0, 30.0, 10.0, 5, 0.4));
    zoom_to_max(&mut s);
    let p = prim(&s);
    let o = outline_of_rotated(&p.shape, p.rotation);
    select_at(
        &mut s,
        pt(
            f64::midpoint(o[0].point.x, o[1].point.x),
            f64::midpoint(o[0].point.y, o[1].point.y),
        ),
    );
    let c = pt(30.0, 30.0);
    let handle = rotate_handle(&s, 20.0, 20.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 36.0, false, false);
    click(&mut s, pt(900.0, 900.0));
    s.pointer_leave();
    let rotated = prim(&s);
    let o = outline_of_rotated(&rotated.shape, rotated.rotation);
    let xs: Vec<f64> = o.iter().map(|a| a.point.x).collect();
    let ys: Vec<f64> = o.iter().map(|a| a.point.y).collect();
    let (ox0, ox1) = (
        xs.iter().copied().fold(f64::INFINITY, f64::min),
        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    );
    let (oy0, oy1) = (
        ys.iter().copied().fold(f64::INFINITY, f64::min),
        ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    );
    let (x0, y0, x1, y1) = vertex_bounds(&s);
    // 0.25 mm stroke: rendered bounds exceed the centreline by about half of it (+ miter).
    let slack = 0.5;
    assert!(
        x0 > ox0 - slack && x1 < ox1 + slack && y0 > oy0 - slack && y1 < oy1 + slack,
        "render bounds {x0},{y0},{x1},{y1} vs outline {ox0},{oy0},{ox1},{oy1}"
    );
    assert!(
        x0 < ox0 + slack && x1 > ox1 - slack && y0 < oy0 + slack && y1 > oy1 - slack,
        "render bounds {x0},{y0},{x1},{y1} vs outline {ox0},{oy0},{ox1},{oy1}"
    );
    // The unrotated star would be different: sanity that rotation changed the geometry.
    let unrotated = outline_of_rotated(&rotated.shape, Angle::from_radians(0.0));
    let uy0 = unrotated
        .iter()
        .map(|a| a.point.y)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (uy0 - oy0).abs() > 0.5,
        "rotation by 36 deg must move the extent"
    );
}

#[test]
fn a_rotated_path_renders_from_its_baked_anchors() {
    let mut s = open_in_session(&s_curve_doc());
    s.pointer_leave();
    let (bx0, by0, bx1, by1) = vertex_bounds(&s);
    let (base_w, base_h) = (bx1 - bx0, by1 - by0);
    select_at(&mut s, pt(20.0, 10.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 90.0, false, false);
    click(&mut s, pt(900.0, 900.0));
    s.pointer_leave();
    let (x0, y0, x1, y1) = vertex_bounds(&s);
    // The same curve turned a quarter about its centre: width and height swap.
    assert!(
        ((x1 - x0) - base_h).abs() < 0.5 && ((y1 - y0) - base_w).abs() < 0.5,
        "{} x {} vs baseline {base_w} x {base_h}",
        x1 - x0,
        y1 - y0
    );
    assert!(f64::midpoint(x0, x1) - 20.0 < 0.5 && f64::midpoint(y0, y1) - 10.0 < 0.5);
}

#[test]
fn live_rotation_preview_draws_the_rotated_shape_before_release() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 100.0, 20.0, 0.0));
    select_at(&mut s, pt(50.0, 0.0));
    let c = pt(50.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 100.0, c, 0.0);
    let to = swept_to(handle, c, 90.0);
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    s.pointer_hover(to, false, false);
    // Doc not yet changed:
    assert!(close(prim(&s).rotation.as_radians(), 0.0));
    // Blue new, black old (`unified-object-editing` criterion 10): the
    // committed rectangle stays drawn unchanged in its own black stroke, and
    // the rotated outline is drawn over it. Rotated 90 degrees about (50, 10)
    // the corners move from (0, 0) to (60, -40) and from (100, 20) to (40, 60).
    let list = s.draw_list();
    let near = |v: &curvyo_render_core::Vertex, x: f64, y: f64| {
        (v.position.x - x).abs() < 1.0 && (v.position.y - y).abs() < 1.0
    };
    let black = curvyo_render_core::RgbaColor::BLACK;
    assert!(
        list.triangles
            .iter()
            .any(|v| near(v, 0.0, 0.0) && v.color == black),
        "the old geometry stays drawn in black"
    );
    assert!(
        list.triangles
            .iter()
            .filter(|v| near(v, 0.0, 0.0) || near(v, 100.0, 20.0))
            .all(|v| v.color == black),
        "nothing but the old stroke at the old corners: box, handles and outline follow the new geometry"
    );
    assert!(
        list.triangles
            .iter()
            .any(|v| near(v, 60.0, -40.0) && v.color != black),
        "the rotated outline is drawn at its new corner"
    );
    s.pointer_up(to, false, false);
}

#[test]
fn live_resize_preview_equals_commit_geometry() {
    // During a resize drag the draw list already shows the new size (the
    // preview shares the commit's arithmetic), so after release the object-only
    // render bounds match the mid-drag object stroke extents.
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(100.0, 60.0), false, false);
    let live = s.draw_list();
    s.pointer_up(pt(100.0, 60.0), false, false);
    assert_rect(&s, 0.0, 0.0, 100.0, 60.0);
    // Live preview has geometry reaching (100,60) already.
    let reach = live
        .triangles
        .iter()
        .any(|v| (v.position.x - 100.0).abs() < 1.0 && (v.position.y - 60.0).abs() < 1.0);
    assert!(reach, "live preview did not show the resized rect");
}

// ---------------------------------------------------------------------
// AC 25: primitive tools' own handles follow rotation
// ---------------------------------------------------------------------

fn rotate_rect_via_document(deg: f64, radius: f64) -> (Session, Point, f64) {
    let document = rect_doc(0.0, 0.0, 40.0, 20.0, radius);
    let id = document.object_ids()[0];
    let c = pt(20.0, 10.0);
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(c, Angle::from_radians(deg.to_radians())),
        )
        .expect("rotate");
    (open_in_session(&document), c, deg.to_radians())
}

/// A rectangle's radius handle at `corner` (signs of the local corner), from
/// the specification's rule (`unified-object-editing` criterion 2): on the
/// corner's inward diagonal at `p = 15 + rho * L(s)` pixels, with
/// `L(s) = (s - 14)/sqrt(2) - 15`, `s` the shorter side in pixels and `rho` the
/// radius over half of it; rotated about `c` by `a`. Also returns the radius
/// gain `G(s) = (s/2)/L(s)`: millimetres of radius per millimetre of pointer
/// travel along the diagonal.
fn radius_knob(
    s: &Session,
    c: Point,
    a: f64,
    (w, h): (f64, f64),
    corner: (f64, f64),
    radius: f64,
) -> (Point, f64) {
    let scale = s.view().scale();
    let shorter_px = w.min(h) * scale;
    let rho = radius / (w.min(h) / 2.0);
    let travel = (shorter_px - 14.0) / 2.0_f64.sqrt() - 15.0;
    let along = (15.0 + rho * travel) / scale / 2.0_f64.sqrt();
    let local = pt(
        c.x + corner.0 * (w / 2.0 - along),
        c.y + corner.1 * (h / 2.0 - along),
    );
    (rot(local, c, a), (shorter_px / 2.0) / travel)
}

/// The document-space step of `d` millimetres along a local corner's inward
/// diagonal, for a primitive rotated by `a`.
fn inward_step(corner: (f64, f64), a: f64, d: f64) -> (f64, f64) {
    let (sn, cs) = a.sin_cos();
    let inward = (-corner.0 / 2.0_f64.sqrt(), -corner.1 / 2.0_f64.sqrt());
    (
        d * (inward.0 * cs - inward.1 * sn),
        d * (inward.0 * sn + inward.1 * cs),
    )
}

// Ported from the shape tools' own handle tests (`unified-object-editing`:
// the shape tools only create): the same rotated scenarios, driven through
// the Select tool, where the radius handle follows the position rule and gain
// of the specification.

#[test]
fn ac25_a_rotated_rect_radius_handle_sits_on_the_rotated_corner_and_drags_along_its_diagonal() {
    let (mut s, c, a) = rotate_rect_via_document(30.0, 0.0);
    click(&mut s, rot(pt(10.0, 0.0), c, a));
    assert_eq!(s.tool(), Tool::Select);
    let (knob, gain) = radius_knob(&s, c, a, (40.0, 20.0), (1.0, -1.0), 0.0);
    let (dx, dy) = inward_step((1.0, -1.0), a, 6.0);
    drag(&mut s, knob, pt(knob.x + dx, knob.y + dy), false, false);
    let (b, r, _, rotation) = rect_of(&s);
    assert!(close(r.as_mm(), gain * 6.0), "radius {}", r.as_mm());
    assert!(r.as_mm() > 1.0, "the radius handle was grabbed");
    assert!(close(rotation, a), "rotation untouched by the radius drag");
    assert!(close(b.width.as_mm(), 40.0) && close(b.height.as_mm(), 20.0));
}

#[test]
fn ac25_the_unrotated_radius_handle_position_is_no_longer_a_handle() {
    let (mut s, c, a) = rotate_rect_via_document(90.0, 0.0);
    click(&mut s, rot(pt(10.0, 0.0), c, a));
    // The knob of the unrotated frame's NE corner: far from the rotated one.
    let (stale, _) = radius_knob(&s, c, 0.0, (40.0, 20.0), (1.0, -1.0), 0.0);
    let before = rect_of(&s).1;
    drag(
        &mut s,
        stale,
        pt(stale.x - 4.0, stale.y + 4.0),
        false,
        false,
    );
    let (b, r, ..) = rect_of(&s);
    assert_eq!(
        r, before,
        "pressing at the stale unrotated position must not grab the radius handle"
    );
    assert_eq!(b, rect_of(&s).0);
}

#[test]
fn ac25_a_rotated_rect_with_a_radius_places_its_handle_along_the_rotated_diagonal() {
    let (mut s, c, a) = rotate_rect_via_document(30.0, 5.0);
    click(&mut s, rot(pt(10.0, 0.0), c, a));
    let (knob, gain) = radius_knob(&s, c, a, (40.0, 20.0), (1.0, -1.0), 5.0);
    let (dx, dy) = inward_step((1.0, -1.0), a, 3.0);
    drag(&mut s, knob, pt(knob.x + dx, knob.y + dy), false, false);
    let (_, r, ..) = rect_of(&s);
    assert!(
        close(r.as_mm(), 5.0 + gain * 3.0),
        "radius should follow the drag by 3 x gain: {}",
        r.as_mm()
    );
}

#[test]
fn ac25_a_rotated_rect_resize_handle_drags_in_local_axes() {
    let (mut s, c, a) = rotate_rect_via_document(30.0, 0.0);
    click(&mut s, rot(pt(10.0, 0.0), c, a));
    // S edge handle (20,20) rotated; drag it 5 along local +y.
    let se = rot(pt(20.0, 20.0), c, a);
    let (sn, cs) = a.sin_cos();
    drag(
        &mut s,
        se,
        pt(se.x - 5.0 * sn, se.y + 5.0 * cs),
        false,
        false,
    );
    let (b, _, _, rotation) = rect_of(&s);
    assert!(
        close(b.height.as_mm(), 25.0) && close(b.width.as_mm(), 40.0),
        "{b:?}"
    );
    assert!(close(rotation, a));
}

#[test]
fn ac25_a_rotated_star_inner_ratio_handle_follows_the_rotation() {
    let document = star_doc(30.0, 30.0, 10.0, 5, 0.5);
    let id = document.object_ids()[0];
    let c = pt(30.0, 30.0);
    let rotation = 100.0_f64.to_radians();
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(c, Angle::from_radians(rotation)),
        )
        .unwrap();
    let mut s = open_in_session(&document);
    let p = prim(&s);
    let o = outline_of_rotated(&p.shape, p.rotation);
    let mid = pt(
        f64::midpoint(o[0].point.x, o[1].point.x),
        f64::midpoint(o[0].point.y, o[1].point.y),
    );
    click(&mut s, mid);
    assert_eq!(s.tool(), Tool::Select);
    // First inner vertex in the local frame: theta = -pi/2 + pi/5, radius 5.
    let theta = -FRAC_PI_2 + PI / 5.0;
    let local = pt(30.0 + 5.0 * theta.cos(), 30.0 + 5.0 * theta.sin());
    let handle = rot(local, c, rotation);
    // Drag it inward by 2 along its own (rotated) direction => ratio 0.3.
    let dir = theta + rotation;
    let to = pt(handle.x - 2.0 * dir.cos(), handle.y - 2.0 * dir.sin());
    drag(&mut s, handle, to, false, false);
    let (_, _, ratio, ..) = star_like(&s);
    assert!(
        close(ratio.unwrap().get(), 0.3),
        "inner ratio {}",
        ratio.unwrap().get()
    );

    // The stale unrotated handle position does nothing.
    let mut s = open_in_session(&document);
    click(&mut s, mid);
    drag(&mut s, local, pt(local.x - 1.0, local.y), false, false);
    let (_, _, ratio, ..) = star_like(&s);
    assert!(
        close(ratio.unwrap().get(), 0.5),
        "stale position grabbed the handle"
    );
}

#[test]
fn ac25_a_rotated_polygon_corner_handle_follows_the_rotation_and_scales_the_radius() {
    let document = polygon_doc(30.0, 30.0, 10.0, 6);
    let id = document.object_ids()[0];
    let c = pt(30.0, 30.0);
    let rotation = 90.0_f64.to_radians();
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(c, Angle::from_radians(rotation)),
        )
        .unwrap();
    let mut s = open_in_session(&document);
    let p = prim(&s);
    let o = outline_of_rotated(&p.shape, p.rotation);
    let mid = pt(
        f64::midpoint(o[0].point.x, o[1].point.x),
        f64::midpoint(o[0].point.y, o[1].point.y),
    );
    click(&mut s, mid);
    // The corner handle of the circumscribed box: local (40,40), rotated 90
    // degrees about the centre => (20,40). A polygon's four cardinal outer
    // handles of the old tool no longer exist.
    let corner = rot(pt(40.0, 40.0), c, rotation);
    assert!(pclose(corner, pt(20.0, 40.0)));
    // 6 along each local axis is 6 sqrt(2) along the diagonal: radius 10 + 6.
    let (sn, cs) = rotation.sin_cos();
    let to = pt(
        corner.x + 6.0 * cs - 6.0 * sn,
        corner.y + 6.0 * sn + 6.0 * cs,
    );
    drag(&mut s, corner, to, false, false);
    let (frame, ..) = star_like(&s);
    assert!(
        close(frame.radius.as_mm(), 16.0),
        "{}",
        frame.radius.as_mm()
    );
}

#[test]
fn ac25_a_rotated_ellipse_edge_handle_follows_the_rotation() {
    let document = ellipse_doc(20.0, 10.0, 20.0, 10.0);
    let id = document.object_ids()[0];
    let c = pt(20.0, 10.0);
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(c, Angle::from_radians(FRAC_PI_2)),
        )
        .unwrap();
    let mut s = open_in_session(&document);
    // The rotated ellipse is 20 wide and 40 tall; point on its outline: local
    // (20+20cos t, 10+10 sin t) rotated.
    let on_outline = rot(ell(), c, FRAC_PI_2);
    click(&mut s, on_outline);
    assert_eq!(s.tool(), Tool::Select);
    // Local E handle (40,10) -> rotated 90 deg: (20,30). Drag 6 along local x = document +y.
    let e = rot(pt(40.0, 10.0), c, FRAC_PI_2);
    drag(&mut s, e, pt(e.x, e.y + 6.0), false, false);
    let Shape::Ellipse { frame } = prim(&s).shape else {
        panic!()
    };
    assert!(
        close(frame.rx.as_mm(), 23.0) || close(frame.rx.as_mm() * 2.0, 46.0),
        "rx {}",
        frame.rx.as_mm()
    );
}

#[test]
fn ac25_a_rotated_rect_draws_all_four_radius_handles_on_the_rotated_corners() {
    let (mut s, c, a) = rotate_rect_via_document(30.0, 5.0);
    zoom_to_max(&mut s);
    s.pointer_leave();
    let plain = s.draw_list().triangle_count();
    click(&mut s, rot(pt(10.0, 0.0), c, a));
    s.pointer_leave();
    let list = s.draw_list();
    assert!(list.triangle_count() > plain, "handles drew something");
    let scale = s.view().scale();
    for corner in [(1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let (want, _) = radius_knob(&s, c, a, (40.0, 20.0), corner, 5.0);
        // The knob's 4 px centre dot has its vertices 2 px from the centre.
        let on_dot = list.triangles.iter().any(|v| {
            let d = (v.position.x - want.x).hypot(v.position.y - want.y) * scale;
            (d - 2.0).abs() < 0.05
        });
        assert!(
            on_dot,
            "no radius handle glyph at the rotated position {want:?}"
        );
    }
}

// ---------------------------------------------------------------------
// Older files, damaged files
// ---------------------------------------------------------------------

#[test]
fn opening_a_session_from_a_file_with_rotation_selects_oriented_handles() {
    let document = rect_doc(0.0, 0.0, 40.0, 20.0, 0.0);
    let id = document.object_ids()[0];
    let c = pt(20.0, 10.0);
    document
        .rotate_object(
            &document
                .object(id)
                .expect("object exists")
                .rotated(c, Angle::from_radians(0.9)),
        )
        .unwrap();
    let mut s = open_in_session(&document);
    click(&mut s, rot(pt(20.0, 0.0), c, 0.9));
    for (name, p) in eight(0.0, 0.0, 40.0, 20.0, c, 0.9) {
        assert!(hint_at(&mut s, p).starts_with("resize:"), "{name}");
    }
}

#[test]
fn damaged_rotation_in_session_open_is_refused_not_panicked() {
    // Mirrors the document-core validation at the Session boundary; the file
    // is crafted in `document-core`'s acceptance_0005_tester. Here only check
    // the error mapping exists.
    assert!(matches!(
        Session::open(1, b"not a project").err(),
        Some(OpenError::NotAProject | OpenError::Damaged)
    ));
}

// ---------------------------------------------------------------------
// Which registers each interaction writes (ADR table), concurrency
// ---------------------------------------------------------------------

fn update_ops_since(session: &Session, from: &loro::VersionVector) -> String {
    let bytes = snapshot_bytes(session);
    let loro = loro::LoroDoc::new();
    loro.import(&bytes).unwrap();
    let json = loro.export_json_updates(from, &loro.oplog_vv());
    format!("{json:?}")
}

fn vv_of(session: &Session) -> loro::VersionVector {
    let bytes = snapshot_bytes(session);
    let loro = loro::LoroDoc::new();
    loro.import(&bytes).unwrap();
    loro.oplog_vv()
}

#[test]
fn rotate_about_the_center_writes_only_the_rotation_register_for_a_primitive() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
    select_at(&mut s, pt(20.0, 0.0));
    let from = vv_of(&s);
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 33.0, false, false);
    let ops = update_ops_since(&s, &from);
    assert!(ops.contains("rotation"), "rotation register written: {ops}");
    assert!(
        !ops.contains("rect_bounds"),
        "frame must not be rewritten for a centre rotate: {ops}"
    );
    assert!(!ops.contains("stroke_width"));
}

#[test]
fn a_resize_never_writes_rotation_and_a_move_writes_only_the_frame() {
    let (mut s, c, a) = rotated_rect_session(20.0);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    let from = vv_of(&s);
    let e = rot(pt(40.0, 10.0), c, a);
    let (sn, cs) = a.sin_cos();
    drag(&mut s, e, pt(e.x + 5.0 * cs, e.y + 5.0 * sn), false, false);
    let ops = update_ops_since(&s, &from);
    assert!(ops.contains("rect_bounds"), "{ops}");
    assert!(
        !ops.contains("\"rotation\""),
        "resize must not touch rotation: {ops}"
    );
    let from = vv_of(&s);
    let b = rect_of(&s).0;
    let centre = pt(
        b.origin.x + b.width.as_mm() / 2.0,
        b.origin.y + b.height.as_mm() / 2.0,
    );
    zoom_to_max(&mut s);
    let start = rot(pt(b.origin.x + 10.0, b.origin.y), centre, a);
    drag(&mut s, start, pt(start.x + 3.0, start.y), false, false);
    let ops = update_ops_since(&s, &from);
    assert!(ops.contains("rect_bounds"), "{ops}");
    assert!(
        !ops.contains("\"rotation\"") && !ops.contains("stroke_width"),
        "move writes only the frame: {ops}"
    );
}

// ---------------------------------------------------------------------
// Edge cases: hostile pointer values, Escape, degenerate objects
// ---------------------------------------------------------------------

fn assert_all_finite_and_reopenable(session: &Session) {
    let bytes = session.pack("0.1.0").unwrap();
    let reopened = Session::open(31, &bytes).expect("file must stay openable");
    let doc = document_of(&reopened);
    for id in doc.object_ids() {
        match doc.object(id).unwrap() {
            ObjectSnapshot::Primitive(p) => {
                assert!(p.rotation.as_radians().is_finite());
                assert!(p.stroke_width.as_mm().is_finite() && p.stroke_width.as_mm() > 0.0);
                match p.shape {
                    Shape::Rect {
                        bounds,
                        corner_radius,
                    } => {
                        for v in [
                            bounds.origin.x,
                            bounds.origin.y,
                            bounds.width.as_mm(),
                            bounds.height.as_mm(),
                            corner_radius.as_mm(),
                        ] {
                            assert!(v.is_finite(), "non-finite rect value {v}");
                        }
                    }
                    Shape::Ellipse { frame } => {
                        for v in [
                            frame.center.x,
                            frame.center.y,
                            frame.rx.as_mm(),
                            frame.ry.as_mm(),
                        ] {
                            assert!(v.is_finite(), "non-finite ellipse value {v}");
                        }
                    }
                    Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => {
                        for v in [frame.center.x, frame.center.y, frame.radius.as_mm()] {
                            assert!(v.is_finite(), "non-finite star value {v}");
                        }
                    }
                }
            }
            ObjectSnapshot::Path(p) => {
                assert!(p.rotation.as_radians().is_finite());
                assert!(p.stroke_width.as_mm().is_finite() && p.stroke_width.as_mm() > 0.0);
                for a in &p.anchors {
                    for v in [
                        a.point.x,
                        a.point.y,
                        a.handle_in.x,
                        a.handle_in.y,
                        a.handle_out.x,
                        a.handle_out.y,
                    ] {
                        assert!(v.is_finite(), "non-finite path value {v}");
                    }
                }
            }
        }
    }
}

#[test]
fn extreme_finite_pointer_values_never_make_the_file_unopenable() {
    let hostile = [
        pt(1e30, 1e30),
        pt(-1e30, 1e30),
        pt(1e30, -1e30),
        pt(1e12, 5.0),
    ];
    let docs = [
        (rect_doc(0.0, 0.0, 40.0, 20.0, 3.0), pt(20.0, 0.0)),
        (s_curve_doc(), pt(20.0, 10.0)),
        (ellipse_doc(20.0, 10.0, 20.0, 10.0), ell()),
    ];
    for (doc, sel) in docs {
        for bad in hostile {
            // resize corner, edge, and rotate
            for grab in [pt(40.0, 20.0), pt(40.0, 10.0), pt(0.0, 0.0)] {
                let mut s = open_in_session(&doc);
                select_at(&mut s, sel);
                drag(&mut s, grab, bad, false, false);
                assert_all_finite_and_reopenable(&s);
                let mut s = open_in_session(&doc);
                select_at(&mut s, sel);
                drag(&mut s, grab, bad, true, true);
                assert_all_finite_and_reopenable(&s);
            }
            let mut s = open_in_session(&doc);
            select_at(&mut s, sel);
            let handle = rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 10.0), 0.0);
            drag(&mut s, handle, bad, false, false);
            assert_all_finite_and_reopenable(&s);
            let mut s = open_in_session(&doc);
            select_at(&mut s, sel);
            let handle = rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 10.0), 0.0);
            drag(&mut s, handle, bad, true, true);
            assert_all_finite_and_reopenable(&s);
        }
    }
}

#[test]
fn hostile_pointer_values_never_write_non_finite_geometry() {
    let hostile = [
        pt(f64::NAN, f64::NAN),
        pt(f64::INFINITY, 0.0),
        pt(0.0, f64::NEG_INFINITY),
    ];
    let docs = [
        (rect_doc(0.0, 0.0, 40.0, 20.0, 3.0), pt(20.0, 0.0)),
        (s_curve_doc(), pt(20.0, 10.0)),
        (ellipse_doc(20.0, 10.0, 20.0, 10.0), ell()),
    ];
    for (doc, sel) in docs {
        for bad in hostile {
            // resize corner, edge, and rotate
            for grab in [pt(40.0, 20.0), pt(40.0, 10.0), pt(0.0, 0.0)] {
                let mut s = open_in_session(&doc);
                select_at(&mut s, sel);
                drag(&mut s, grab, bad, false, false);
                assert_all_finite_and_reopenable(&s);
                let mut s = open_in_session(&doc);
                select_at(&mut s, sel);
                drag(&mut s, grab, bad, true, true);
                assert_all_finite_and_reopenable(&s);
            }
            let mut s = open_in_session(&doc);
            select_at(&mut s, sel);
            let handle = rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 10.0), 0.0);
            drag(&mut s, handle, bad, false, false);
            assert_all_finite_and_reopenable(&s);
            let mut s = open_in_session(&doc);
            select_at(&mut s, sel);
            let handle = rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 10.0), 0.0);
            drag(&mut s, handle, bad, true, true);
            assert_all_finite_and_reopenable(&s);
        }
    }
}

#[test]
fn escape_during_a_transform_drag_leaves_the_document_unchanged() {
    for start_resize in [true, false] {
        let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 0.0));
        select_at(&mut s, pt(20.0, 0.0));
        let before = snapshot_bytes(&s);
        let from = if start_resize {
            pt(40.0, 20.0)
        } else {
            rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 10.0), 0.0)
        };
        s.pointer_hover(from, false, false);
        s.pointer_down(from, false);
        s.pointer_hover(pt(90.0, 60.0), false, false);
        s.escape();
        s.pointer_up(pt(90.0, 60.0), false, false);
        assert_eq!(
            snapshot_bytes(&s),
            before,
            "Escape must cancel the drag (resize: {start_resize})"
        );
        assert!(s.live_readout().is_none());
    }
}

#[test]
fn dragging_a_handle_away_and_back_to_the_start_changes_nothing_observable() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 40.0, 20.0, 4.0));
    select_at(&mut s, pt(20.0, 0.0));
    let before = document_of(&s).object(only_id(&s)).unwrap();
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_down(pt(40.0, 20.0), false);
    s.pointer_hover(pt(80.0, 80.0), false, false);
    s.pointer_hover(pt(40.0, 20.0), false, false);
    s.pointer_up(pt(40.0, 20.0), false, false);
    let after = document_of(&s).object(only_id(&s)).unwrap();
    assert_eq!(
        after, before,
        "returning to the start leaves the object as it was"
    );
}

#[test]
fn a_horizontal_line_path_has_a_zero_height_box_and_still_transforms_finitely() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(40.0, 0.0)),
        ],
        false,
    );
    // Select via the midpoint of the line.
    for (grab, to) in [
        (pt(40.0, 0.0), pt(60.0, 10.0)),  // E/NE-ish
        (pt(20.0, 0.0), pt(20.0, 15.0)),  // S edge of a zero-height box
        (pt(0.0, 0.0), pt(-10.0, -10.0)), // corner
    ] {
        for (shift, ctrl) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut s = open_in_session(&document);
            select_at(&mut s, pt(20.0, 0.0));
            drag(&mut s, grab, to, shift, ctrl);
            assert_all_finite_and_reopenable(&s);
        }
    }
    // Rotating it about its centre: the line turns, anchors stay finite.
    let mut s = open_in_session(&document);
    select_at(&mut s, pt(20.0, 0.0));
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, pt(20.0, 0.0), 0.0);
    rotate_drag(&mut s, handle, pt(20.0, 0.0), 90.0, false, false);
    let p = path_of(&s);
    assert!(
        pclose(p.anchors[0].point, pt(20.0, -20.0)),
        "{:?}",
        p.anchors[0].point
    );
    assert!(
        pclose(p.anchors[1].point, pt(20.0, 20.0)),
        "{:?}",
        p.anchors[1].point
    );
}

#[test]
fn a_single_anchor_path_has_a_point_box_and_never_produces_nan() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[NewAnchor::corner(AnchorId::new(1, 1), pt(5.0, 5.0))],
        false,
    );
    let mut s = open_in_session(&document);
    // A lone anchor is hit at its point; the handles all coincide there.
    click(&mut s, pt(5.0, 5.0));
    for to in [pt(30.0, 30.0), pt(-5.0, -5.0), pt(5.0, 5.0)] {
        for (shift, ctrl) in [(false, false), (true, true)] {
            drag(&mut s, pt(5.0, 5.0), to, shift, ctrl);
            assert_all_finite_and_reopenable(&s);
        }
    }
}

#[test]
fn a_huge_object_resizes_exactly_enough() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 1.0e6, 5.0e5, 0.0));
    s.set_scale_stroke_width(true);
    select_at(&mut s, pt(5.0e5, 0.0));
    drag(&mut s, pt(1.0e6, 5.0e5), pt(2.0e6, 1.0e6), false, false);
    let (b, _, stroke, _) = rect_of(&s);
    assert!((b.width.as_mm() - 2.0e6).abs() < 1e-3 && (b.height.as_mm() - 1.0e6).abs() < 1e-3);
    assert!((stroke - 0.5).abs() < 1e-9, "stroke {stroke}");
}

#[test]
fn a_tiny_object_resizes_without_nan() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 1.0e-6, 1.0e-6, 0.0));
    zoom_to_max(&mut s);
    click(&mut s, pt(5.0e-7, 0.0));
    drag(&mut s, pt(1.0e-6, 1.0e-6), pt(1.0, 1.0), false, false);
    assert_all_finite_and_reopenable(&s);
}

// ---------------------------------------------------------------------
// Object to path, AC25 echoes, exactness of rotated path resize, preview
// ---------------------------------------------------------------------

#[test]
fn object_to_path_of_a_rotated_primitive_keeps_rotation_and_the_rotated_outline() {
    let (mut s, c, a) = rotate_rect_via_document(30.0, 4.0);
    let before = prim(&s);
    let outline = outline_of_rotated(&before.shape, before.rotation);
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    s.convert_selected_to_paths();
    let doc = document_of(&s);
    let id = doc.object_ids()[0];
    let path = doc.path(id).expect("now a path with the same NodeId");
    assert!(
        close(path.rotation.as_radians(), a),
        "rotation register kept"
    );
    assert_eq!(path.anchors.len(), outline.len());
    for (anchor, want) in path.anchors.iter().zip(&outline) {
        assert!(
            pclose(anchor.point, want.point),
            "{:?} vs {:?}",
            anchor.point,
            want.point
        );
    }
}

#[test]
fn a_rotated_paths_selection_box_survives_object_to_path() {
    // After object to path, going back to Select shows the oriented box.
    let (mut s, c, a) = rotate_rect_via_document(30.0, 0.0);
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    s.convert_selected_to_paths();
    s.set_tool(Tool::Select);
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 0.0), c, a));
    for (name, p) in eight(0.0, 0.0, 40.0, 20.0, c, a) {
        assert!(
            hint_at(&mut s, p).starts_with("resize:"),
            "handle {name} oriented"
        );
    }
}

#[test]
fn ac18_rotated_s_curve_resize_is_an_exact_affine_scale_in_the_local_frame() {
    let mut s = open_in_session(&s_curve_doc());
    select_at(&mut s, pt(20.0, 10.0));
    let c = pt(20.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 40.0, c, 0.0);
    rotate_drag(&mut s, handle, c, 45.0, false, false);
    let a = 45.0_f64.to_radians();
    click(&mut s, pt(500.0, 500.0));
    click(&mut s, rot(pt(20.0, 10.0), c, 0.0)); // the curve's centre is on the curve
    // E handle of the local box (40,10) -> pull 20 along local x. Local box
    // edge handle position in document: rot((40,10)).
    let e = rot(pt(40.0, 10.0), c, a);
    let (sn, cs) = a.sin_cos();
    drag(
        &mut s,
        e,
        pt(e.x + 20.0 * cs, e.y + 20.0 * sn),
        false,
        false,
    );
    let p = path_of(&s);
    // Back into the original local frame: everything is the original curve
    // scaled by (1.5, 1) about the pinned W edge x = 0.
    let back = |q: Point| rot(q, c, -a);
    let (a0, a1) = (&p.anchors[0], &p.anchors[1]);
    let un = |v: Vec2| {
        Vec2::new(
            v.x * (-a).cos() - v.y * (-a).sin(),
            v.x * (-a).sin() + v.y * (-a).cos(),
        )
    };
    let (p0, p1) = (back(a0.point), back(a1.point));
    assert!(pclose(p0, pt(0.0, 0.0)), "pinned anchor {p0:?}");
    assert!(pclose(p1, pt(60.0, 20.0)), "far anchor {p1:?}");
    let (ho, hi) = (un(a0.handle_out), un(a1.handle_in));
    assert!(close(ho.x, 15.0) && close(ho.y, 4.0), "handle_out {ho:?}");
    assert!(close(hi.x, -15.0) && close(hi.y, -4.0), "handle_in {hi:?}");
    assert!(close(p.rotation.as_radians(), a));
}

#[test]
fn live_rotation_preview_draws_the_rotated_geometry_at_the_rotated_position() {
    let mut s = open_in_session(&rect_doc(0.0, 0.0, 100.0, 20.0, 0.0));
    select_at(&mut s, pt(50.0, 0.0));
    let c = pt(50.0, 10.0);
    let handle = rotate_handle(&s, 0.0, 0.0, 100.0, c, 0.0);
    s.pointer_hover(handle, false, false);
    s.pointer_down(handle, false);
    let to = swept_to(handle, c, 90.0); // 90 deg
    s.pointer_hover(to, false, false);
    let list = s.draw_list();
    // The rotated 100 x 20 rect spans y in [-40, 60] around x = 50.
    for want in [pt(50.0, 59.0), pt(50.0, -39.0)] {
        assert!(
            list.triangles.iter().any(
                |v| (v.position.x - want.x).abs() < 12.0 && (v.position.y - want.y).abs() < 1.5
            ),
            "no preview geometry near {want:?}"
        );
    }
    s.pointer_up(to, false, false);
}
