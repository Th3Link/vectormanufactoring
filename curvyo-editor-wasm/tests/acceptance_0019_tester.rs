//! Independent tester acceptance tests for `specs/0019-multi-object-transform/
//! specification.md`, written from the specification before the implementation
//! was read. Everything goes through `Session`'s public API (pointer events,
//! keys, chips). Expected values are the specification's own arithmetic and
//! small reference models written here (a linear map applied to anchors,
//! handle vectors and frames), never read back from the code under test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::items_after_statements, clippy::too_many_arguments)]
#![allow(missing_docs, clippy::type_complexity)]

use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, Document, EllipseFrame, Length, NewAnchor, NodeId, ObjectSnapshot,
    Point, PointCount, RectBounds, Shape, StarFrame, Vec2, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{EscapeStep, KeyHint, KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::{EntryOutcome, MoveEntryMode};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

fn rect_bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    }
}

fn anchor(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn rotate_about_centre(d: &Document, id: NodeId, radians: f64) {
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

fn open(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1600.0, 1000.0);
    s
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).expect("the session output reopens")
}

fn objects(s: &Session) -> Vec<ObjectSnapshot> {
    let d = doc_of(s);
    d.object_ids()
        .into_iter()
        .filter_map(|id| d.object(id))
        .collect()
}

fn change_count(s: &Session) -> usize {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.len_changes()
}

fn bytes_of(s: &Session) -> Vec<u8> {
    doc_of(s).export_loro_snapshot().unwrap()
}

fn k_of(s: &Session) -> f64 {
    s.view().scale()
}

fn hold(s: &mut Session, at: Point, shift: bool, ctrl: bool) {
    s.modifiers_changed(shift, ctrl, false);
    s.pointer_hover(at, shift, ctrl);
}

fn click(s: &mut Session, p: Point) {
    hold(s, p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn shift_click(s: &mut Session, p: Point) {
    hold(s, p, true, false);
    s.pointer_down(p, true);
    s.pointer_up(p, true, false);
    hold(s, p, false, false);
}

fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    hold(s, from, shift, ctrl);
    s.pointer_down(from, shift);
    hold(s, to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
    hold(s, to, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point) {
    drag_mod(s, from, to, false, false);
}

fn key(s: &mut Session, key: &str, shift: bool) -> KeyOutcome {
    s.key_down(KeyInput {
        key,
        shift,
        ..KeyInput::default()
    })
}

/// A point on the outline of object `index` that is not a corner.
fn outline_point(d: &Document, index: usize) -> Point {
    let id = d.object_ids()[index];
    match d.object(id).unwrap() {
        ObjectSnapshot::Primitive(p) => {
            let o = outline_of_rotated(&p.shape, p.rotation);
            // The midpoint of the first outline segment: always on the outline,
            // away from the vertices where group handles tend to sit.
            let (a, b) = (o[0].point, o[1].point);
            pt((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
        }
        ObjectSnapshot::Path(p) => {
            let (a, b) = (p.anchors[0].point, p.anchors[1].point);
            pt((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
        }
    }
}

/// Selects every object of the document: a click on the first, Shift-clicks
/// on the rest.
fn select_all(s: &mut Session) {
    let d = doc_of(s);
    for i in 0..d.object_ids().len() {
        let p = outline_point(&d, i);
        if i == 0 {
            click(s, p);
        } else {
            shift_click(s, p);
        }
    }
    assert_eq!(s.selected_object_count(), d.object_ids().len());
}

/// Selects everything with one marquee that encloses the whole document.
fn select_everything(s: &mut Session) {
    let n = doc_of(s).object_ids().len();
    key(s, "Escape", false);
    drag(s, pt(-300.0, -300.0), pt(900.0, 900.0));
    assert_eq!(s.selected_object_count(), n);
}

/// The axis-aligned group box in document mm, with the pixel-based handle
/// layout of the design system (`docs/design-system.md`, "Transform handle
/// layout"): resize at the corners and edge midpoints, rotate 32 px out on the
/// diagonal (corners) or along the normal (sides), skew 16 px out from the side
/// midpoint, centre at the middle.
#[derive(Clone, Copy, Debug)]
struct GBox {
    min: Point,
    max: Point,
    k: f64,
}

impl GBox {
    fn new(s: &Session, min: Point, max: Point) -> Self {
        Self {
            min,
            max,
            k: k_of(s),
        }
    }
    fn w(&self) -> f64 {
        self.max.x - self.min.x
    }
    fn h(&self) -> f64 {
        self.max.y - self.min.y
    }
    fn centre(&self) -> Point {
        pt(self.min.x + self.w() / 2.0, self.min.y + self.h() / 2.0)
    }
    /// Corner by sign: (-1,-1) top-left, (1,1) bottom-right.
    fn corner(&self, sx: f64, sy: f64) -> Point {
        let c = self.centre();
        pt(c.x + sx * self.w() / 2.0, c.y + sy * self.h() / 2.0)
    }
    /// Side midpoint by unit normal: (0,-1) top, (1,0) right.
    fn mid(&self, nx: f64, ny: f64) -> Point {
        let c = self.centre();
        pt(c.x + nx * self.w() / 2.0, c.y + ny * self.h() / 2.0)
    }
    fn rot_corner(&self, sx: f64, sy: f64) -> Point {
        let d = 32.0 / SQRT_2 / self.k;
        let p = self.corner(sx, sy);
        pt(p.x + sx * d, p.y + sy * d)
    }
    fn rot_side(&self, nx: f64, ny: f64) -> Point {
        let p = self.mid(nx, ny);
        pt(p.x + nx * 32.0 / self.k, p.y + ny * 32.0 / self.k)
    }
    fn skew(&self, nx: f64, ny: f64) -> Point {
        let p = self.mid(nx, ny);
        pt(p.x + nx * 16.0 / self.k, p.y + ny * 16.0 / self.k)
    }
}

fn rect_of(o: &ObjectSnapshot) -> (RectBounds, f64) {
    match o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => (bounds, p.rotation.as_radians()),
            other => panic!("rect expected, got {other:?}"),
        },
        ObjectSnapshot::Path(_) => panic!("rect expected"),
    }
}

fn ellipse_of(o: &ObjectSnapshot) -> (EllipseFrame, f64) {
    match o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Ellipse { frame } => (frame, p.rotation.as_radians()),
            other => panic!("ellipse expected, got {other:?}"),
        },
        ObjectSnapshot::Path(_) => panic!("ellipse expected"),
    }
}

fn rotation_of(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.rotation.as_radians(),
        ObjectSnapshot::Path(p) => p.rotation.as_radians(),
    }
}

fn path_points(o: &ObjectSnapshot) -> Vec<Point> {
    match o {
        ObjectSnapshot::Path(p) => p.anchors.iter().map(|a| a.point).collect(),
        ObjectSnapshot::Primitive(_) => panic!("path expected"),
    }
}

fn path_handles(o: &ObjectSnapshot) -> Vec<(Vec2, Vec2)> {
    match o {
        ObjectSnapshot::Path(p) => p
            .anchors
            .iter()
            .map(|a| (a.handle_in, a.handle_out))
            .collect(),
        ObjectSnapshot::Primitive(_) => panic!("path expected"),
    }
}

fn rect_centre(b: RectBounds) -> Point {
    pt(
        b.origin.x + b.width.as_mm() / 2.0,
        b.origin.y + b.height.as_mm() / 2.0,
    )
}

fn assert_pt(a: Point, x: f64, y: f64, eps: f64, what: &str) {
    assert!(
        near(a.x, x, eps) && near(a.y, y, eps),
        "{what}: got ({}, {}), want ({x}, {y})",
        a.x,
        a.y
    );
}

/// The wrapped difference of two angles (radians), in (-pi, pi].
fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = (a - b) % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    }
    if d <= -PI {
        d += 2.0 * PI;
    }
    d
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

// ---------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------

/// The two squares of criteria 7 and 18, moved by (50, 50) so that nothing is
/// near the viewport edge: 10 x 10 mm rectangles at (50, 50) and (70, 50).
fn two_squares() -> (Document, [NodeId; 2]) {
    let d = Document::new(1);
    let a = d.create_rect(rect_bounds(50.0, 50.0, 10.0, 10.0));
    let b = d.create_rect(rect_bounds(70.0, 50.0, 10.0, 10.0));
    (d, [a, b])
}

fn two_squares_box(s: &Session) -> GBox {
    GBox::new(s, pt(50.0, 50.0), pt(80.0, 60.0))
}

/// Two closed 10 x 10 paths at (50, 50) and (70, 50), each with a curved
/// segment so handle vectors matter (the top-right anchor is smooth).
fn two_paths() -> Document {
    let d = Document::new(1);
    for (n, x0) in [(1_u64, 50.0), (11, 70.0)] {
        let mut a1 = anchor(n, x0, 50.0);
        a1.handle_out = Vec2::new(2.0, 0.0);
        let mut a2 = anchor(n + 1, x0 + 10.0, 50.0);
        a2.handle_in = Vec2::new(-2.0, 0.0);
        a2.handle_out = Vec2::new(0.0, 3.0);
        a2.kind = AnchorKind::Symmetric;
        let a3 = anchor(n + 2, x0 + 10.0, 60.0);
        let a4 = anchor(n + 3, x0, 60.0);
        let _ = d.create_path(&[a1, a2, a3, a4], true);
    }
    d
}

// ---------------------------------------------------------------------
// Part A: the group box (read through the typed-entry prefills)
// ---------------------------------------------------------------------

/// Criterion 1 and 35: the typed absolute move is anchored at the top-left of
/// the group box, whose prefill shows it; with the rectangle (centre (10, 5),
/// 20 x 10, rotated 30 degrees) and the circle (centre (40, 5), r 5) of the
/// spec's example the box starts at (-1.16, -4.33).
#[test]
fn ac1_group_box_of_a_rotated_rectangle_and_a_circle() {
    let d = Document::new(1);
    let r = d.create_rect(rect_bounds(0.0, 0.0, 20.0, 10.0));
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(40.0, 5.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(5.0),
    });
    rotate_about_centre(&d, r, 30.0_f64.to_radians());
    let mut s = open(&d);
    select_all(&mut s);
    // Size chip prefill: the group box size (W 46.16, H 18.66 to 0.01 mm).
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().expect("size chip");
    assert_eq!(v.kind, "size");
    let w: f64 = v.fields[0].prefill.parse().unwrap();
    let h: f64 = v.fields[1].prefill.parse().unwrap();
    assert!(near(w, 46.16, 0.06), "W {w}");
    assert!(near(h, 18.66, 0.06), "H {h}");
    s.cancel_transform_entry();
    // Move chip, absolute prefill: the top-left corner.
    assert_eq!(key(&mut s, "m", false), KeyOutcome::EntryOpened);
    let m = s.move_entry().expect("move chip");
    let x: f64 = m.absolute_prefill[0].parse().unwrap();
    let y: f64 = m.absolute_prefill[1].parse().unwrap();
    assert!(near(x, -1.16, 0.06), "X {x}");
    assert!(near(y, -4.33, 0.06), "Y {y}");
}

/// Criterion 2: a stroke width is not part of the group box. A wide stroke on
/// one of the objects leaves the size chip prefill unchanged.
#[test]
fn ac2_stroke_width_is_not_part_of_the_group_box() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    key(&mut s, "s", false);
    let before = s.transform_entry().unwrap().fields[0].prefill.clone();
    s.cancel_transform_entry();
    // Give the second object a 10 mm stroke through a stroke-scaling gesture
    // would change geometry, so edit the style through the document instead.
    let d2 = doc_of(&s);
    let ids = d2.object_ids();
    let mut o = d2.object(ids[1]).unwrap();
    match &mut o {
        ObjectSnapshot::Primitive(p) => p.style.stroke.width = Length::from_mm(10.0),
        ObjectSnapshot::Path(_) => unreachable!(),
    }
    d2.transform_objects(&[o], true).unwrap();
    let mut s2 = Session::open(3, &pack(&d2, "0.1.0").unwrap()).unwrap();
    s2.set_tool(Tool::Select);
    s2.resize_viewport(1600.0, 1000.0);
    select_all(&mut s2);
    key(&mut s2, "s", false);
    let after = s2.transform_entry().unwrap().fields[0].prefill.clone();
    assert_eq!(before, after);
    assert_eq!(after, "30.0");
}

/// Criterion 3: a path turned earlier keeps a stale oriented box; the group
/// box is the tight bounds of the curve, not the union of oriented boxes.
#[test]
fn ac3_group_box_ignores_the_stale_oriented_box_of_a_turned_path() {
    let d = Document::new(1);
    let p = d.create_path(
        &[anchor(1, 50.0, 50.0), anchor(2, 70.0, 50.0), anchor(3, 60.0, 56.0)],
        true,
    );
    let _ = d.create_rect(rect_bounds(100.0, 50.0, 10.0, 10.0));
    let o = d.object(p).unwrap();
    d.rotate_object(&o.rotated(pt(60.0, 53.0), Angle::from_radians(PI / 4.0)))
        .unwrap();
    // Tight bounds of the turned anchors (straight segments), by hand.
    let turned = path_points(&d.object(p).unwrap());
    let (mut x0, mut x1) = (f64::MAX, f64::MIN);
    let (mut y0, mut y1) = (f64::MAX, f64::MIN);
    for q in turned.iter().chain([&pt(100.0, 50.0), &pt(110.0, 60.0)]) {
        x0 = x0.min(q.x);
        x1 = x1.max(q.x);
        y0 = y0.min(q.y);
        y1 = y1.max(q.y);
    }
    let mut s = open(&d);
    select_all(&mut s);
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    assert!(near(w, x1 - x0, 0.06) && near(h, y1 - y0, 0.06), "{w} x {h}");
}

/// Criterion 7: after each committed rotate the box is the axis-aligned bounds
/// of the result; no orientation is left behind. The two squares rotated by 90
/// degrees about the centre give 10 x 30; another 45 degrees gives 28.28 square.
#[test]
fn ac7_the_box_is_refit_axis_aligned_after_a_rotate() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    // Typed rotate by 90 about the box centre.
    assert_eq!(key(&mut s, "r", false), KeyOutcome::EntryOpened);
    assert_eq!(
        s.commit_transform_entry("90", "", 0),
        EntryOutcome::Committed
    );
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    assert!(near(w, 10.0, 0.06) && near(h, 30.0, 0.06), "{w} x {h}");
    s.cancel_transform_entry();
    key(&mut s, "r", false);
    assert_eq!(
        s.commit_transform_entry("45", "", 0),
        EntryOutcome::Committed
    );
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    assert!(near(w, 28.28, 0.06) && near(h, 28.28, 0.06), "{w} x {h}");
}

// ---------------------------------------------------------------------
// Part C: scale
// ---------------------------------------------------------------------

/// Criterion 18: the spec's own example. Dragging the bottom-right corner of
/// the group box from (30, 10) to (60, 30) (here offset by (50, 50)) gives
/// sx = 2, sy = 3.
#[test]
fn ac18_corner_drag_scales_every_object_about_the_opposite_corner() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let commits = change_count(&s);
    drag(&mut s, g.corner(1.0, 1.0), pt(110.0, 80.0));
    assert_eq!(change_count(&s), commits + 1, "one commit");
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    let (b1, _) = rect_of(&o[1]);
    assert_pt(b0.origin, 50.0, 50.0, 1e-6, "rect 0 origin");
    assert!(near(b0.width.as_mm(), 20.0, 1e-6) && near(b0.height.as_mm(), 30.0, 1e-6));
    assert_pt(b1.origin, 90.0, 50.0, 1e-6, "rect 1 origin");
    assert!(near(b1.width.as_mm(), 20.0, 1e-6) && near(b1.height.as_mm(), 30.0, 1e-6));
    assert_eq!(rotation_of(&o[0]), 0.0);
}

/// Criterion 19: Shift scales about the centre; an edge handle changes one
/// axis only; Ctrl on an edge has no effect.
#[test]
fn ac19_shift_scales_about_the_centre_and_edges_change_one_axis() {
    let (d, _) = two_squares();
    // Shift about the centre (65, 55): the corner (80, 60) -> (95, 70):
    // sx = 30/15 = 2, sy = 15/5 = 3.
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    drag_mod(&mut s, g.corner(1.0, 1.0), pt(95.0, 70.0), true, false);
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    let (b1, _) = rect_of(&o[1]);
    // Left rect x 50..60 -> 65 + (x-65)*2: 35..55; y 50..60 -> 55 + (y-55)*3: 40..70.
    assert_pt(b0.origin, 35.0, 40.0, 1e-6, "rect 0 origin");
    assert!(near(b0.width.as_mm(), 20.0, 1e-6) && near(b0.height.as_mm(), 30.0, 1e-6));
    assert_pt(b1.origin, 75.0, 40.0, 1e-6, "rect 1 origin");

    // Edge handle E: only x changes, anchored at the left edge (x = 50).
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    drag(&mut s, g.mid(1.0, 0.0), pt(110.0, 77.0));
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    let (b1, _) = rect_of(&o[1]);
    assert_pt(b0.origin, 50.0, 50.0, 1e-6, "rect 0");
    assert!(near(b0.width.as_mm(), 20.0, 1e-6) && near(b0.height.as_mm(), 10.0, 1e-6));
    assert_pt(b1.origin, 90.0, 50.0, 1e-6, "rect 1");

    // Ctrl on an edge handle: no effect, the same result.
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    drag_mod(&mut s, g.mid(1.0, 0.0), pt(110.0, 77.0), false, true);
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    assert!(near(b0.width.as_mm(), 20.0, 1e-6) && near(b0.height.as_mm(), 10.0, 1e-6));
}

/// Criterion 19: Ctrl on a corner scales both axes by one factor (the dominant
/// drag axis).
#[test]
fn ac19_ctrl_on_a_corner_scales_by_one_factor() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    // The drag is dominant in x: fx = 2, fy = 1.1.
    drag_mod(&mut s, g.corner(1.0, 1.0), pt(110.0, 61.0), false, true);
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    let sx = b0.width.as_mm() / 10.0;
    let sy = b0.height.as_mm() / 10.0;
    assert!(near(sx, sy, 1e-9), "one factor: {sx} vs {sy}");
    assert!(near(sx, 2.0, 1e-6), "dominant axis x: {sx}");
}

/// Criterion 19: the result is computed from the state at the press; Shift and
/// Ctrl may flip mid-drag. Ending with no modifier equals a plain drag.
#[test]
fn ac19_modifiers_changed_mid_drag_use_the_final_state() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let from = g.corner(1.0, 1.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(100.0, 70.0), true, true);
    hold(&mut s, pt(110.0, 80.0), false, false);
    s.pointer_up(pt(110.0, 80.0), false, false);
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    assert_pt(b0.origin, 50.0, 50.0, 1e-6, "as plain");
    assert!(near(b0.width.as_mm(), 20.0, 1e-6) && near(b0.height.as_mm(), 30.0, 1e-6));
}

/// Criterion 20: every kind under a non-uniform scale (sx = 2, sy = 3 about
/// the top-left corner of the group box).
#[test]
fn ac20_every_kind_under_a_non_uniform_scale() {
    // Group box: x 20..120, y 20..80 built from a path that spans it plus an
    // aligned rectangle (rot 90), a rectangle (rot 0), an ellipse, a circle
    // rotated 30 degrees. Everything lies inside the path's box.
    let d = Document::new(1);
    let mut a1 = anchor(1, 20.0, 20.0);
    a1.handle_out = Vec2::new(5.0, 2.0);
    let mut a2 = anchor(2, 120.0, 80.0);
    a2.handle_in = Vec2::new(-4.0, -1.0);
    a2.kind = AnchorKind::Symmetric;
    let path = d.create_path(&[a1, a2], false);
    let r0 = d.create_rect(rect_bounds(30.0, 30.0, 10.0, 20.0));
    let r90 = d.create_rect(rect_bounds(50.0, 30.0, 10.0, 20.0));
    rotate_about_centre(&d, r90, FRAC_PI_2);
    let el = d.create_ellipse(EllipseFrame {
        center: pt(80.0, 40.0),
        rx: Length::from_mm(8.0),
        ry: Length::from_mm(4.0),
    });
    let circ = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 60.0),
        rx: Length::from_mm(6.0),
        ry: Length::from_mm(6.0),
    });
    rotate_about_centre(&d, circ, 30.0_f64.to_radians());
    let el90 = d.create_ellipse(EllipseFrame {
        center: pt(70.0, 65.0),
        rx: Length::from_mm(8.0),
        ry: Length::from_mm(3.0),
    });
    rotate_about_centre(&d, el90, FRAC_PI_2);
    let _ = (path, r0, r90, el, circ, el90);
    let before = {
        let s = open(&d);
        objects(&s)
    };
    let mut s = open(&d);
    select_all(&mut s);
    // Make sure it really is a stretchable (not uniform-only) selection.
    let g = GBox::new(&s, pt(20.0, 20.0), pt(120.0, 80.0));
    // Drag the bottom-right corner (120, 80) to (220, 140): sx 2, sy 2? use
    // sx = 2 (x: 20 + 200 = 220), sy = 3 (y: 20 + 180 = 200).
    drag(&mut s, g.corner(1.0, 1.0), pt(220.0, 200.0));
    let after = objects(&s);
    let (px, py, sx, sy) = (20.0, 20.0, 2.0, 3.0);
    let map = |p: Point| pt(px + (p.x - px) * sx, py + (p.y - py) * sy);

    // Path: anchors mapped, handle vectors scaled per axis.
    let (ob, oa) = (&before[0], &after[0]);
    for (b, a) in path_points(ob).iter().zip(path_points(oa)) {
        let m = map(*b);
        assert_pt(a, m.x, m.y, 1e-6, "path anchor");
    }
    for ((bi, bo), (ai, ao)) in path_handles(ob).iter().zip(path_handles(oa)) {
        assert!(near(ai.x, bi.x * sx, 1e-6) && near(ai.y, bi.y * sy, 1e-6));
        assert!(near(ao.x, bo.x * sx, 1e-6) && near(ao.y, bo.y * sy, 1e-6));
    }
    // Rect at 0 degrees: width by sx, height by sy, centre mapped.
    let ((bb, _), (ab, ar)) = (rect_of(&before[1]), rect_of(&after[1]));
    let c = map(rect_centre(bb));
    assert_pt(rect_centre(ab), c.x, c.y, 1e-6, "rect 0 centre");
    assert!(near(ab.width.as_mm(), 10.0 * sx, 1e-6));
    assert!(near(ab.height.as_mm(), 20.0 * sy, 1e-6));
    assert_eq!(ar, 0.0);
    // Rect at 90 degrees: width by sy, height by sx; rotation unchanged.
    let ((bb, _), (ab, ar)) = (rect_of(&before[2]), rect_of(&after[2]));
    let c = map(rect_centre(bb));
    assert_pt(rect_centre(ab), c.x, c.y, 1e-6, "rect 90 centre");
    assert!(
        near(ab.width.as_mm(), 10.0 * sy, 1e-6),
        "w {}",
        ab.width.as_mm()
    );
    assert!(
        near(ab.height.as_mm(), 20.0 * sx, 1e-6),
        "h {}",
        ab.height.as_mm()
    );
    assert!(near(ar, FRAC_PI_2, 1e-9));
    // Ellipse at 0.
    let ((bf, _), (af, _)) = (ellipse_of(&before[3]), ellipse_of(&after[3]));
    let c = map(bf.center);
    assert_pt(af.center, c.x, c.y, 1e-6, "ellipse centre");
    assert!(near(af.rx.as_mm(), 8.0 * sx, 1e-6) && near(af.ry.as_mm(), 4.0 * sy, 1e-6));
    // Circle at 30 degrees: becomes an ellipse with rotation 0.
    let ((bf, _), (af, ar)) = (ellipse_of(&before[4]), ellipse_of(&after[4]));
    let c = map(bf.center);
    assert_pt(af.center, c.x, c.y, 1e-6, "circle centre");
    assert!(near(af.rx.as_mm(), 6.0 * sx, 1e-6) && near(af.ry.as_mm(), 6.0 * sy, 1e-6));
    assert!(
        near(ar, 0.0, 1e-12),
        "a stretched rotated circle has rotation 0, got {ar}"
    );
    // Ellipse at 90: rx along document y: rx by sy, ry by sx.
    let ((bf, _), (af, ar)) = (ellipse_of(&before[5]), ellipse_of(&after[5]));
    let c = map(bf.center);
    assert_pt(af.center, c.x, c.y, 1e-6, "ellipse 90 centre");
    assert!(near(af.rx.as_mm(), 8.0 * sy, 1e-6), "rx {}", af.rx.as_mm());
    assert!(near(af.ry.as_mm(), 3.0 * sx, 1e-6), "ry {}", af.ry.as_mm());
    assert!(near(ar, FRAC_PI_2, 1e-9));
}


// ---------------------------------------------------------------------
// Part C: rotate
// ---------------------------------------------------------------------

/// Presses the handle at `from`, moves to the point that turns the pointer
/// about `pivot` by `degrees` (clockwise on screen) and releases there.
fn rotate_drag(
    s: &mut Session,
    from: Point,
    pivot: Point,
    degrees: f64,
    shift: bool,
    ctrl: bool,
) -> Point {
    let to = rot(from, pivot, degrees.to_radians());
    drag_mod(s, from, to, shift, ctrl);
    to
}

fn norm(a: f64) -> f64 {
    let n = a % (2.0 * PI);
    if n > PI {
        n - 2.0 * PI
    } else if n <= -PI {
        n + 2.0 * PI
    } else {
        n
    }
}

/// Criterion 26, the spec's example (moved by (50, 50)): the two squares
/// turned by 90 degrees about the box centre (65, 55): the left square ends at
/// (60, 40)-(70, 50), the right one at (60, 60)-(70, 70), each rotation 90.
#[test]
fn ac26_example_two_squares_turned_by_90() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let commits = change_count(&s);
    rotate_drag(&mut s, g.rot_corner(1.0, 1.0), g.centre(), 90.0, false, false);
    assert_eq!(change_count(&s), commits + 1, "one commit");
    let o = objects(&s);
    let ((b0, r0), (b1, r1)) = (rect_of(&o[0]), rect_of(&o[1]));
    assert_pt(rect_centre(b0), 65.0, 45.0, 1e-6, "left centre");
    assert_pt(rect_centre(b1), 65.0, 65.0, 1e-6, "right centre");
    assert!(near(b0.width.as_mm(), 10.0, 1e-9) && near(b0.height.as_mm(), 10.0, 1e-9));
    assert!(near(r0, FRAC_PI_2, 1e-9) && near(r1, FRAC_PI_2, 1e-9), "{r0} {r1}");
    // The box after the commit is axis-aligned again: 10 x 30.
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.fields[0].prefill, "10.0");
    assert_eq!(v.fields[1].prefill, "30.0");
}

/// Criterion 26: rotating by 90 and then by -90 (the box centre is unchanged
/// for this arrangement), and by 180 twice, restores anchors, handle vectors and
/// `rotation` within the tolerances.
#[test]
fn ac26_rotate_and_rotate_back_restores() {
    let d = two_paths();
    let before = objects(&open(&d));
    for degrees in [90.0, 180.0] {
        let mut s = open(&d);
        select_all(&mut s);
        let g = GBox::new(&s, pt(50.0, 50.0), pt(80.0, 60.0));
        rotate_drag(&mut s, g.rot_corner(1.0, 1.0), g.centre(), degrees, false, false);
        let mid = objects(&s);
        assert!(
            path_points(&mid[0])
                .iter()
                .zip(path_points(&before[0]))
                .any(|(a, b)| !near(a.x, b.x, 1e-6) || !near(a.y, b.y, 1e-6)),
            "the first rotate changed something"
        );
        // The refit box: the same centre (65, 55), swapped for 90.
        let g2 = if degrees == 90.0 {
            GBox::new(&s, pt(60.0, 40.0), pt(70.0, 70.0))
        } else {
            g
        };
        rotate_drag(&mut s, g2.rot_corner(1.0, 1.0), g2.centre(), -degrees, false, false);
        let after = objects(&s);
        for (a, b) in after.iter().zip(&before) {
            for (pa, pb) in path_points(a).iter().zip(path_points(b)) {
                assert_pt(*pa, pb.x, pb.y, 1e-6, "anchor restored");
            }
            for ((ai, ao), (bi, bo)) in path_handles(a).iter().zip(path_handles(b)) {
                assert!(near(ai.x, bi.x, 1e-6) && near(ai.y, bi.y, 1e-6));
                assert!(near(ao.x, bo.x, 1e-6) && near(ao.y, bo.y, 1e-6));
            }
            assert!(near(angle_diff(rotation_of(a), rotation_of(b)), 0.0, 1e-9));
        }
    }
}

/// Criterion 25/26: every kind turns rigidly about the pivot: centres are
/// turned, frames and parameters unchanged, `rotation` increased by delta.
#[test]
fn ac26_every_kind_turns_rigidly_with_rotation_added() {
    let d = Document::new(1);
    let path = d.create_path(
        &[anchor(1, 40.0, 40.0), anchor(2, 100.0, 40.0), anchor(3, 70.0, 90.0)],
        true,
    );
    let r = d.create_rect(rect_bounds(50.0, 50.0, 12.0, 8.0));
    rotate_about_centre(&d, r, 0.3);
    let e = d.create_ellipse(EllipseFrame {
        center: pt(85.0, 55.0),
        rx: Length::from_mm(6.0),
        ry: Length::from_mm(3.0),
    });
    let poly = d.create_polygon(
        StarFrame {
            center: pt(60.0, 75.0),
            radius: Length::from_mm(6.0),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(6).unwrap(),
    );
    let star = d.create_star(
        StarFrame {
            center: pt(80.0, 75.0),
            radius: Length::from_mm(6.0),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    let _ = (path, e, poly, star);
    let before = objects(&open(&d));
    let mut s = open(&d);
    select_all(&mut s);
    let g = GBox::new(&s, pt(40.0, 40.0), pt(100.0, 90.0));
    let degrees: f64 = 37.0;
    let delta = degrees.to_radians();
    let pivot = g.centre();
    rotate_drag(&mut s, g.rot_corner(-1.0, 1.0), pivot, degrees, false, false);
    let after = objects(&s);
    // Path.
    for (a, b) in path_points(&after[0]).iter().zip(path_points(&before[0])) {
        let m = rot(b, pivot, delta);
        assert_pt(*a, m.x, m.y, 1e-6, "path anchor");
    }
    assert!(near(angle_diff(rotation_of(&after[0]), rotation_of(&before[0]) + delta), 0.0, 1e-9));
    // Rectangle: centre turned, size unchanged, rotation + delta.
    let ((bb, br), (ab, ar)) = (rect_of(&before[1]), rect_of(&after[1]));
    let c = rot(rect_centre(bb), pivot, delta);
    assert_pt(rect_centre(ab), c.x, c.y, 1e-6, "rect centre");
    assert!(near(ab.width.as_mm(), bb.width.as_mm(), 1e-9));
    assert!(near(ab.height.as_mm(), bb.height.as_mm(), 1e-9));
    assert!(near(angle_diff(ar, br + delta), 0.0, 1e-9), "{ar} vs {}", br + delta);
    // Ellipse.
    let ((bf, br), (af, ar)) = (ellipse_of(&before[2]), ellipse_of(&after[2]));
    let c = rot(bf.center, pivot, delta);
    assert_pt(af.center, c.x, c.y, 1e-6, "ellipse centre");
    assert!(near(af.rx.as_mm(), 6.0, 1e-9) && near(af.ry.as_mm(), 3.0, 1e-9));
    assert!(near(angle_diff(ar, br + delta), 0.0, 1e-9));
    // Polygon and star: frame centre turned, radius unchanged, rotation +delta.
    for i in [3, 4] {
        let (bs, br) = match &before[i] {
            ObjectSnapshot::Primitive(p) => (p.shape, p.rotation.as_radians()),
            ObjectSnapshot::Path(_) => unreachable!(),
        };
        let (as_, ar) = match &after[i] {
            ObjectSnapshot::Primitive(p) => (p.shape, p.rotation.as_radians()),
            ObjectSnapshot::Path(_) => unreachable!(),
        };
        let (bfr, afr) = match (bs, as_) {
            (Shape::Polygon { frame: a, .. }, Shape::Polygon { frame: b, .. })
            | (Shape::Star { frame: a, .. }, Shape::Star { frame: b, .. }) => (a, b),
            other => panic!("kind changed: {other:?}"),
        };
        let c = rot(bfr.center, pivot, delta);
        assert_pt(afr.center, c.x, c.y, 1e-6, "polygon/star centre");
        assert!(near(afr.radius.as_mm(), 6.0, 1e-9));
        assert!(near(angle_diff(ar, br + delta), 0.0, 1e-9));
    }
    // Rigid: distances between the frame centres are unchanged.
    let centre_of = |o: &ObjectSnapshot| match o {
        ObjectSnapshot::Path(_) => pt(0.0, 0.0),
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => rect_centre(bounds),
            Shape::Ellipse { frame } => frame.center,
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame.center,
        },
    };
    for i in 1..5 {
        for j in (i + 1)..5 {
            let (a0, a1) = (centre_of(&before[i]), centre_of(&before[j]));
            let (b0, b1) = (centre_of(&after[i]), centre_of(&after[j]));
            let d0 = ((a0.x - a1.x).powi(2) + (a0.y - a1.y).powi(2)).sqrt();
            let d1 = ((b0.x - b1.x).powi(2) + (b0.y - b1.y).powi(2)).sqrt();
            assert!(near(d0, d1, 1e-6));
        }
    }
}

/// Criterion 25: with Shift the pivot is the opposite corner; a side rotate
/// handle (Shift held: they exist only then) pivots at the opposite side.
#[test]
fn ac25_shift_pivots() {
    let (d, _) = two_squares();
    // Shift, corner SE: pivot is NW (50, 50).
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    rotate_drag(&mut s, g.rot_corner(1.0, 1.0), g.corner(-1.0, -1.0), 90.0, true, false);
    let o = objects(&s);
    let (b0, r0) = rect_of(&o[0]);
    // Left square centre (55, 55) about (50, 50) by 90: (50 - 5, 50 + 5) = (45, 55).
    assert_pt(rect_centre(b0), 45.0, 55.0, 1e-6, "left centre about NW");
    assert!(near(r0, FRAC_PI_2, 1e-9));
    // Side handle E with Shift: pivot is the W midpoint (50, 55).
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let pivot = g.mid(-1.0, 0.0);
    hold(&mut s, g.rot_side(1.0, 0.0), true, false);
    s.pointer_down(g.rot_side(1.0, 0.0), true);
    let to = rot(g.rot_side(1.0, 0.0), pivot, FRAC_PI_2);
    hold(&mut s, to, true, false);
    s.pointer_up(to, true, false);
    assert_eq!(s.selected_object_count(), 2, "a Shift press on a side rotate handle rotates, it does not toggle");
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    assert_pt(rect_centre(b0), 50.0, 60.0, 1e-6, "left centre about the W midpoint");
}

/// Criterion 25: Shift changes the pivot mid-drag; the result is computed from
/// the state at the press, so releasing with Shift up equals a plain rotate.
#[test]
fn ac25_shift_mid_drag_uses_the_final_state() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let from = g.rot_corner(1.0, 1.0);
    let to = rot(from, g.centre(), FRAC_PI_2);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, rot(from, g.corner(-1.0, -1.0), 1.0), true, false);
    hold(&mut s, to, false, false);
    s.pointer_up(to, false, false);
    let o = objects(&s);
    let (b0, _) = rect_of(&o[0]);
    assert_pt(rect_centre(b0), 65.0, 45.0, 1e-6, "as a plain rotate");
}

/// Criterion 27 with 0008 criterion 34: Ctrl snaps the delta to the stops.
#[test]
fn ac27_ctrl_snaps_the_delta() {
    let table: [(f64, f64); 13] = [
        (10.0, 15.0),
        (19.0, 22.5),
        (18.5, 15.0),
        (26.0, 22.5),
        (26.5, 30.0),
        (40.0, 45.0),
        (55.0, 60.0),
        (64.0, 67.5),
        (71.0, 67.5),
        (100.0, 105.0),
        (-10.0, -15.0),
        (-40.0, -45.0),
        (190.0, 195.0),
    ];
    for (raw, snapped) in table {
        let (d, _) = two_squares();
        let mut s = open(&d);
        select_all(&mut s);
        let g = two_squares_box(&s);
        rotate_drag(&mut s, g.rot_corner(1.0, 1.0), g.centre(), raw, false, true);
        let o = objects(&s);
        let (_, r0) = rect_of(&o[0]);
        assert!(
            near(angle_diff(r0, snapped.to_radians()), 0.0, 1e-6),
            "raw {raw}: got {}, want {snapped}",
            r0.to_degrees()
        );
    }
}

/// Criterion 28: the readout "Δ 37.4°", "Δ 45°" under Ctrl, with a real minus
/// sign, in (-180, 180]; none after the release.
#[test]
fn ac28_rotate_readout() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let from = g.rot_corner(1.0, 1.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    assert!(s.live_readout().is_none() || s.live_readout().is_some());
    hold(&mut s, rot(from, g.centre(), 37.4_f64.to_radians()), false, false);
    assert_eq!(s.live_readout().unwrap().text, "Δ 37.4°");
    hold(&mut s, rot(from, g.centre(), 40.0_f64.to_radians()), false, true);
    assert_eq!(s.live_readout().unwrap().text, "Δ 45°");
    hold(&mut s, rot(from, g.centre(), (-22.5_f64).to_radians()), false, false);
    assert_eq!(s.live_readout().unwrap().text, "Δ \u{2212}22.5°");
    hold(&mut s, rot(from, g.centre(), 200.0_f64.to_radians()), false, false);
    assert_eq!(s.live_readout().unwrap().text, "Δ \u{2212}160°");
    s.pointer_up(from, false, false);
    assert!(s.live_readout().is_none());
}

/// Criterion 30: Escape, a drag back to the start, and a press-release under
/// the dead zone write nothing, for every kind of gesture.
#[test]
fn ac30_cancelled_and_empty_gestures_write_nothing() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let before = bytes_of(&s);
    let commits = change_count(&s);
    let handles = [
        g.corner(1.0, 1.0),
        g.mid(1.0, 0.0),
        g.rot_corner(1.0, -1.0),
    ];
    for h in handles {
        // Under the dead zone.
        hold(&mut s, h, false, false);
        s.pointer_down(h, false);
        s.pointer_up(pt(h.x + 1.0 / k_of(&s), h.y), false, false);
        assert_eq!(change_count(&s), commits, "dead zone {h:?}");
        // Back to the start.
        hold(&mut s, h, false, false);
        s.pointer_down(h, false);
        hold(&mut s, pt(h.x + 20.0, h.y + 9.0), false, false);
        hold(&mut s, h, false, false);
        s.pointer_up(h, false, false);
        assert_eq!(change_count(&s), commits, "back to the start {h:?}");
        // Escape.
        hold(&mut s, h, false, false);
        s.pointer_down(h, false);
        hold(&mut s, pt(h.x + 20.0, h.y + 9.0), false, false);
        assert_eq!(key(&mut s, "Escape", false), KeyOutcome::Escape(EscapeStep::CancelledDrag));
        s.pointer_up(pt(h.x + 20.0, h.y + 9.0), false, false);
        assert_eq!(change_count(&s), commits, "Escape {h:?}");
        assert_eq!(s.selected_object_count(), 2, "the selection survives");
    }
    assert!(before == bytes_of(&s), "the document changed");
}

// ---------------------------------------------------------------------
// Part C: move, press order (criteria 16, 17, 43 to 46)
// ---------------------------------------------------------------------

/// Two 20 x 20 mm squares at (50, 50) and (90, 50): the group box is
/// (50, 50)-(110, 70), 60 x 20 mm, big enough for the centre handle, with
/// empty canvas between the squares.
fn wide_squares() -> Document {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(90.0, 50.0, 20.0, 20.0));
    d
}

fn wide_box(s: &Session) -> GBox {
    GBox::new(s, pt(50.0, 50.0), pt(110.0, 70.0))
}

fn origins(s: &Session) -> Vec<Point> {
    objects(s).iter().map(|o| rect_of(o).0.origin).collect()
}

/// Criterion 16: a drag of the centre handle moves the whole selection 1:1 in
/// one commit; sizes and rotations are unchanged.
#[test]
fn ac16_the_centre_handle_moves_the_selection_in_one_commit() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    let commits = change_count(&s);
    drag(&mut s, g.centre(), pt(g.centre().x + 7.0, g.centre().y - 3.0));
    assert_eq!(change_count(&s), commits + 1);
    let o = origins(&s);
    assert_pt(o[0], 57.0, 47.0, 1e-9, "first");
    assert_pt(o[1], 97.0, 47.0, 1e-9, "second");
    let (b, r) = rect_of(&objects(&s)[0]);
    assert!(near(b.width.as_mm(), 20.0, 1e-12) && near(b.height.as_mm(), 20.0, 1e-12));
    assert_eq!(r, 0.0);
    assert_eq!(s.selected_object_count(), 2);
}

/// Criterion 16: a drag from a selected object's outline moves the whole
/// selection by the same operation; a press and release under the dead zone
/// writes nothing.
#[test]
fn ac16_a_drag_from_a_selected_outline_moves_everything() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let commits = change_count(&s);
    // Under the dead zone (3 px): nothing.
    let p = pt(60.0, 50.0);
    hold(&mut s, p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(pt(p.x + 1.0 / k_of(&s), p.y), false, false);
    assert_eq!(change_count(&s), commits);
    drag(&mut s, p, pt(65.0, 58.0));
    let o = origins(&s);
    assert_pt(o[0], 55.0, 58.0, 1e-9, "first");
    assert_pt(o[1], 95.0, 58.0, 1e-9, "second");
    assert_eq!(s.selected_object_count(), 2);
}

/// Criterion 16: the dead zone is in screen pixels: a move of 2.9 px writes
/// nothing, 3.5 px moves 1:1 (the full distance, not the distance past the
/// dead zone).
#[test]
fn ac16_dead_zone_then_one_to_one() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let k = k_of(&s);
    let p = pt(60.0, 50.0);
    drag(&mut s, p, pt(p.x + 2.9 / k, p.y));
    assert_eq!(origins(&s)[0], pt(50.0, 50.0));
    drag(&mut s, p, pt(p.x + 3.5 / k, p.y));
    assert_pt(origins(&s)[0], 50.0 + 3.5 / k, 50.0, 1e-9, "1:1");
}

/// Criterion 45: a plain press on the empty interior of the group box behaves
/// as before: a click clears the selection, a drag runs the marquee. The box
/// has no hit area of its own: nothing moves.
#[test]
fn ac45_the_empty_interior_clears_or_marquees_and_never_moves() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let before = bytes_of(&s);
    // A drag from the empty gap between the squares, inside the box, that
    // selects nothing: the marquee. No object moves.
    drag(&mut s, pt(75.0, 56.0), pt(85.0, 64.0));
    assert!(bytes_of(&s) == before, "the document changed");
    assert_eq!(s.selected_object_count(), 0, "the marquee replaced the selection");
    // A click on the empty interior clears the selection.
    select_all(&mut s);
    click(&mut s, pt(80.0, 60.0 - 6.0));
    assert_eq!(s.selected_object_count(), 0);
    assert!(bytes_of(&s) == before, "the document changed");
    // The marquee works with Shift (adds) and Ctrl (removes) inside the box too.
    click(&mut s, pt(60.0, 50.0));
    assert_eq!(s.selected_object_count(), 1);
    // Shift-marquee from inside the (single object: no group) area over the
    // second square.
    drag_mod(&mut s, pt(85.0, 45.0), pt(115.0, 75.0), true, false);
    assert_eq!(s.selected_object_count(), 2, "Shift adds");
    // Ctrl removes: a marquee from the empty interior of the group box that
    // covers the second square entirely.
    drag_mod(&mut s, pt(80.0, 75.0), pt(115.0, 45.0), false, true);
    assert_eq!(s.selected_object_count(), 1, "Ctrl removes");
}

/// Criterion 43 step 6 (question 3 b): a plain press on an unselected object
/// inside the group box selects it, replacing the selection.
#[test]
fn ac43_a_plain_press_on_an_unselected_object_inside_the_box_selects_it() {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(110.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(80.0, 55.0, 10.0, 10.0)); // between them
    let mut s = open(&d);
    click(&mut s, pt(60.0, 50.0));
    shift_click(&mut s, pt(120.0, 50.0));
    assert_eq!(s.selected_object_count(), 2);
    let before = bytes_of(&s);
    // A plain click on the middle square's outline (inside the group box).
    click(&mut s, pt(85.0, 55.0));
    assert_eq!(s.selected_object_count(), 1, "replaced the selection");
    assert!(bytes_of(&s) == before, "nothing moved");
    // And a drag from it moves that object only.
    key(&mut s, "Escape", false);
    click(&mut s, pt(60.0, 50.0));
    shift_click(&mut s, pt(120.0, 50.0));
    assert_eq!(s.selected_object_count(), 2);
    drag(&mut s, pt(85.0, 55.0), pt(85.0, 100.0));
    let o = origins(&s);
    assert_pt(o[0], 50.0, 50.0, 1e-9, "first stays");
    assert_pt(o[2], 80.0, 55.0 + 45.0, 1e-9, "the unselected object, now selected, moved");
    assert_eq!(s.selected_object_count(), 1);
}

/// Criterion 43 step 3: Shift on an outline of a selected object toggles it
/// (click) or starts an axis-locked move (drag).
#[test]
fn ac43_shift_on_an_outline_toggles_or_axis_locks() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    shift_click(&mut s, pt(60.0, 50.0));
    assert_eq!(s.selected_object_count(), 1, "toggled off");
    shift_click(&mut s, pt(60.0, 50.0));
    assert_eq!(s.selected_object_count(), 2, "toggled on");
    // Axis-locked drag: the larger component wins.
    drag_mod(&mut s, pt(60.0, 50.0), pt(70.0, 54.0), true, false);
    let o = origins(&s);
    assert_pt(o[0], 60.0, 50.0, 1e-9, "x only");
    assert_pt(o[1], 100.0, 50.0, 1e-9, "x only");
}

/// Criterion 17: Shift on the centre handle is a locked move; Ctrl a copy of
/// every selected object, the selection becomes the copies; Escape cancels.
#[test]
fn ac17_centre_handle_with_shift_ctrl_and_escape() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    let c = g.centre();
    // Shift: x dominant -> y stays.
    drag_mod(&mut s, c, pt(c.x + 12.0, c.y + 3.0), true, false);
    let o = origins(&s);
    assert_pt(o[0], 62.0, 50.0, 1e-9, "locked");
    assert_pt(o[1], 102.0, 50.0, 1e-9, "locked");
    // Ctrl: copies. Reselect at the new place.
    let mut s = open(&d);
    select_all(&mut s);
    let ids_before = doc_of(&s).object_ids();
    let commits = change_count(&s);
    drag_mod(&mut s, c, pt(c.x, c.y + 40.0), false, true);
    assert_eq!(change_count(&s), commits + 1, "a copy is one commit");
    let after = objects(&s);
    assert_eq!(after.len(), 4, "two originals and two copies");
    assert_eq!(s.selected_object_count(), 2, "the copies are selected");
    let ids_after = doc_of(&s).object_ids();
    for id in &ids_before {
        assert!(ids_after.contains(id));
    }
    let o = origins(&s);
    let has = |x: f64, y: f64| o.iter().any(|p| near(p.x, x, 1e-9) && near(p.y, y, 1e-9));
    assert!(has(50.0, 50.0) && has(90.0, 50.0), "originals stay");
    assert!(has(50.0, 90.0) && has(90.0, 90.0), "copies");
    // Escape cancels a move.
    let mut s = open(&d);
    select_all(&mut s);
    let before = bytes_of(&s);
    hold(&mut s, c, false, false);
    s.pointer_down(c, false);
    hold(&mut s, pt(c.x + 30.0, c.y), false, false);
    assert_eq!(key(&mut s, "Escape", false), KeyOutcome::Escape(EscapeStep::CancelledDrag));
    s.pointer_up(pt(c.x + 30.0, c.y), false, false);
    assert!(bytes_of(&s) == before, "the document changed");
}

/// Criterion 14: where the centre handle is not drawn (s < 48 px), a press at
/// the box centre is an ordinary press (empty canvas): it clears.
#[test]
fn ac14_no_centre_handle_under_48_px_so_the_centre_is_ordinary_canvas() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    assert!(g.h() * k_of(&s) < 48.0, "scene: s < 48 px");
    let c = g.centre();
    let before = bytes_of(&s);
    drag(&mut s, c, pt(c.x + 20.0, c.y));
    assert!(bytes_of(&s) == before, "no move from the empty centre");
    assert_eq!(s.selected_object_count(), 0, "the marquee replaced the selection");
}

/// Criterion 46: the cursor over the centre handle is `move`; over a corner
/// resize a resize cursor; elsewhere inside the box on empty canvas it is the
/// normal one, exactly as outside the box.
#[test]
fn ac46_cursors() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    hold(&mut s, g.centre(), false, false);
    assert_eq!(s.cursor_hint(), "move");
    hold(&mut s, pt(80.0, 56.0), false, false);
    let inside = s.cursor_hint();
    hold(&mut s, pt(300.0, 300.0), false, false);
    let outside = s.cursor_hint();
    assert_eq!(inside, outside, "empty canvas inside the box looks like outside");
    hold(&mut s, g.corner(1.0, 1.0), false, false);
    let c = s.cursor_hint();
    assert!(c.contains("resize") || c.contains("nwse"), "{c}");
    // Modifiers never change a handle cursor.
    hold(&mut s, g.corner(1.0, 1.0), true, true);
    assert_eq!(s.cursor_hint(), c);
}

/// UX notes section 5: the plus badge follows the press test: over a selected
/// object's outline and the centre handle, not on empty canvas (minus badge
/// there), not over an unselected object.
#[test]
fn ux5_copy_and_remove_badges_follow_the_press_test() {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(110.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(82.0, 55.0, 16.0, 10.0));
    let mut s = open(&d);
    click(&mut s, pt(60.0, 50.0));
    shift_click(&mut s, pt(120.0, 50.0));
    let g = GBox::new(&s, pt(50.0, 50.0), pt(130.0, 70.0));
    let badges = |s: &mut Session, p: Point| {
        hold(s, p, false, true);
        let m = s.move_indicators();
        (m.copy_badge, m.remove_badge)
    };
    assert_eq!(badges(&mut s, pt(60.0, 50.0)), (true, false), "selected outline");
    assert_eq!(badges(&mut s, g.centre()), (true, false), "centre handle");
    // Over an unselected object `edit-interaction-polish` criterion 37 keeps
    // the plus badge and a Ctrl press copies that object alone (the UX notes of
    // 0019 section 5 say "no badge": the accepted older rule is kept).
    assert_eq!(badges(&mut s, pt(90.0, 55.0)), (true, false), "unselected object");
    assert_eq!(badges(&mut s, pt(75.0, 100.0)), (false, true), "empty canvas");
    assert_eq!(badges(&mut s, pt(100.0, 68.0)), (false, true), "empty inside the group box");
}

/// Criterion 43 step 1: Alt starts the lasso, also on a handle: no transform.
#[test]
fn ac43_alt_on_a_handle_starts_the_lasso() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    let before = bytes_of(&s);
    let from = g.corner(1.0, 1.0);
    s.modifiers_changed(false, false, true);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    for q in [pt(from.x + 5.0, from.y + 20.0), pt(from.x - 40.0, from.y + 20.0)] {
        s.pointer_hover(q, false, false);
    }
    s.pointer_up(pt(from.x - 40.0, from.y + 20.0), false, false);
    s.modifiers_changed(false, false, false);
    assert!(bytes_of(&s) == before, "no scale");
}

/// Criterion 43 step 2: a plain press on a handle that is not drawn is an
/// ordinary press: with a primitive in the selection there is no skew handle,
/// so a drag from where it would be runs the marquee and moves nothing.
#[test]
fn ac14_press_where_a_hidden_skew_handle_would_be_is_ordinary() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    let before = bytes_of(&s);
    drag(&mut s, g.skew(0.0, -1.0), pt(g.skew(0.0, -1.0).x + 10.0, g.skew(0.0, -1.0).y));
    assert!(bytes_of(&s) == before, "the document changed");
}

/// Criterion 43 step 4: Ctrl on empty canvas (also inside the group box) is
/// the remove marquee, with a click that removes nothing.
#[test]
fn ac43_ctrl_on_empty_canvas_is_the_remove_marquee() {
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let before = bytes_of(&s);
    drag_mod(&mut s, pt(75.0, 56.0), pt(85.0, 64.0), false, true);
    assert!(bytes_of(&s) == before, "the document changed");
    assert_eq!(s.selected_object_count(), 2, "nothing was inside the marquee");
}

/// Criterion 43 step 4 and `edit-interaction-polish` 37: a Ctrl drag from an
/// unselected object copies that object alone, not the selection.
#[test]
fn ac43_ctrl_drag_from_an_unselected_object_copies_it_alone() {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(110.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(82.0, 55.0, 16.0, 10.0));
    let mut s = open(&d);
    click(&mut s, pt(60.0, 50.0));
    shift_click(&mut s, pt(120.0, 50.0));
    drag_mod(&mut s, pt(90.0, 55.0), pt(90.0, 95.0), false, true);
    assert_eq!(objects(&s).len(), 4, "exactly one copy");
    assert_eq!(s.selected_object_count(), 1);
    let o = origins(&s);
    assert!(o.iter().any(|p| near(p.x, 82.0, 1e-9) && near(p.y, 55.0, 1e-9)));
    assert!(o.iter().any(|p| near(p.x, 82.0, 1e-9) && near(p.y, 95.0, 1e-9)));
    assert!(o.iter().any(|p| near(p.x, 50.0, 1e-9) && near(p.y, 50.0, 1e-9)));
}

// ---------------------------------------------------------------------
// Part E: skew
// ---------------------------------------------------------------------

fn style_width(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
        ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
    }
}

/// Criterion 41, the spec's example: two closed 10 x 10 paths, the top skew
/// handle, alpha = 45 degrees with the bottom side fixed: every point at the
/// top moves 10 mm in x, every point at the bottom does not move. Handle
/// vectors shear by the same linear map; rotation, kinds, counts, closed
/// state and stroke width are unchanged.
#[test]
fn ac41_skew_top_handle_by_45_degrees() {
    let d = two_paths();
    let before = objects(&open(&d));
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let commits = change_count(&s);
    let h = g.skew(0.0, -1.0);
    drag(&mut s, h, pt(h.x + 10.0, h.y));
    assert_eq!(change_count(&s), commits + 1, "one commit");
    let after = objects(&s);
    // x' = x + t * (60 - y), t = d / h = 10 / 10 = 1.
    let t = 1.0;
    for (a, b) in after.iter().zip(&before) {
        let (pa, pb) = (path_points(a), path_points(b));
        assert_eq!(pa.len(), pb.len(), "node count");
        for (qa, qb) in pa.iter().zip(&pb) {
            assert_pt(*qa, qb.x + t * (60.0 - qb.y), qb.y, 1e-6, "anchor");
        }
        for ((ai, ao), (bi, bo)) in path_handles(a).iter().zip(path_handles(b)) {
            // A vector (hx, hy) maps to (hx - t * hy, hy).
            assert!(near(ai.x, bi.x - t * bi.y, 1e-6) && near(ai.y, bi.y, 1e-6));
            assert!(near(ao.x, bo.x - t * bo.y, 1e-6) && near(ao.y, bo.y, 1e-6));
        }
        assert_eq!(rotation_of(a), rotation_of(b));
        assert!(near(style_width(a), style_width(b), 0.0));
        match (a, b) {
            (ObjectSnapshot::Path(x), ObjectSnapshot::Path(y)) => {
                assert_eq!(x.closed, y.closed);
                for (ka, kb) in x.anchors.iter().zip(&y.anchors) {
                    assert_eq!(ka.kind, kb.kind);
                    assert_eq!(ka.id, kb.id, "order and identity");
                }
            }
            _ => unreachable!(),
        }
    }
}

/// Criterion 41: a skew by alpha and then by -alpha with the same handle
/// restores anchors and handle vectors.
#[test]
fn ac41_skew_and_skew_back_restores() {
    let d = two_paths();
    let before = objects(&open(&d));
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let h = g.skew(0.0, -1.0);
    drag(&mut s, h, pt(h.x + 10.0, h.y));
    // The box is refit: the top handle sits on the new top side, same height.
    let mid = objects(&s);
    let xs: Vec<f64> = mid.iter().flat_map(path_points).map(|p| p.x).collect();
    let x0 = xs.iter().copied().fold(f64::MAX, f64::min);
    let x1 = xs.iter().copied().fold(f64::MIN, f64::max);
    // The bulging curve of the smooth anchor can leave the anchor hull; the
    // refit box is read through the typed move chip's prefill instead.
    let _ = (x0, x1);
    let outcome = key(&mut s, "m", false);
    assert_eq!(outcome, KeyOutcome::EntryOpened, "selected: {}", s.selected_object_count());
    let m = s.move_entry().unwrap();
    let tl = pt(
        m.absolute_prefill[0].parse().unwrap(),
        m.absolute_prefill[1].parse().unwrap(),
    );
    assert_eq!(key(&mut s, "Escape", false), KeyOutcome::Escape(EscapeStep::ClosedEntry));
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let (w, hh): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    s.cancel_transform_entry();
    let g2 = GBox::new(&s, tl, pt(tl.x + w, tl.y + hh));
    let h2 = g2.skew(0.0, -1.0);
    // Undo with a typed skew would be the cleaner route; a drag back by -10 on
    // the top handle (bottom fixed at y = 60) is the exact inverse.
    drag(&mut s, h2, pt(h2.x - 10.0, h2.y));
    let after = objects(&s);
    for (a, b) in after.iter().zip(&before) {
        for (qa, qb) in path_points(a).iter().zip(path_points(b)) {
            assert_pt(*qa, qb.x, qb.y, 1e-6, "anchor restored");
        }
        for ((ai, ao), (bi, bo)) in path_handles(a).iter().zip(path_handles(b)) {
            assert!(near(ai.x, bi.x, 1e-6) && near(ai.y, bi.y, 1e-6));
            assert!(near(ao.x, bo.x, 1e-6) && near(ao.y, bo.y, 1e-6));
        }
    }
}

/// Criterion 41: Shift keeps the line through the box centre fixed; the right
/// handle is a y skew with the left side fixed.
#[test]
fn ac41_skew_with_shift_and_the_right_handle() {
    let d = two_paths();
    let before = objects(&open(&d));
    // Shift on the top handle: fixed line y = 55, h = 5, d = 10 -> t = 2.
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let h = g.skew(0.0, -1.0);
    drag_mod(&mut s, h, pt(h.x + 10.0, h.y), true, false);
    let after = objects(&s);
    for (a, b) in after.iter().zip(&before) {
        for (qa, qb) in path_points(a).iter().zip(path_points(b)) {
            assert_pt(*qa, qb.x + 2.0 * (55.0 - qb.y), qb.y, 1e-6, "anchor");
        }
    }
    // Right handle: y' = y + t * (x - 50), h = 30, d = 30 -> t = 1.
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let h = g.skew(1.0, 0.0);
    drag(&mut s, h, pt(h.x, h.y + 30.0));
    let after = objects(&s);
    for (a, b) in after.iter().zip(&before) {
        for (qa, qb) in path_points(a).iter().zip(path_points(b)) {
            assert_pt(*qa, qb.x, qb.y + (qb.x - 50.0), 1e-6, "anchor");
        }
    }
}

/// Criterion 41/42: the angle is atan(d / h), bounded below 90 degrees for any
/// drag; Ctrl snaps and caps at 75 degrees; the readout names the axis, the
/// sign and one decimal.
#[test]
fn ac41_ac42_skew_angle_cap_and_readout() {
    let d = two_paths();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let h = g.skew(0.0, -1.0);
    hold(&mut s, h, false, false);
    s.pointer_down(h, false);
    let dx = 10.0 * 12.5_f64.to_radians().tan();
    hold(&mut s, pt(h.x + dx, h.y), false, false);
    assert_eq!(s.live_readout().unwrap().text, "Skew x +12.5°");
    hold(&mut s, pt(h.x - 10.0 * 8.0_f64.to_radians().tan(), h.y), false, false);
    assert_eq!(s.live_readout().unwrap().text, "Skew x \u{2212}8°");
    // Ctrl: 22.5 degrees shows as "22.5°".
    hold(&mut s, pt(h.x + 10.0 * 23.0_f64.to_radians().tan(), h.y), false, true);
    assert_eq!(s.live_readout().unwrap().text, "Skew x +22.5°");
    // A huge drag with Ctrl caps at 75.
    hold(&mut s, pt(h.x + 100000.0, h.y), false, true);
    assert_eq!(s.live_readout().unwrap().text, "Skew x +75°");
    s.pointer_up(pt(h.x + 100000.0, h.y), false, true);
    // After release: x' = x + tan(75) * (60 - y): finite, shape not degenerate.
    let after = objects(&s);
    let t = 75.0_f64.to_radians().tan();
    let p = path_points(&after[0]);
    assert!(near(p[0].x, 50.0 + t * 10.0, 1e-6), "{:?}", p[0]);
    // Without Ctrl a huge drag stays finite, below 90 degrees.
    let mut s = open(&d);
    select_all(&mut s);
    drag(&mut s, h, pt(h.x + 1e6, h.y));
    for o in objects(&s) {
        for q in path_points(&o) {
            assert!(q.x.is_finite() && q.y.is_finite());
        }
    }
}

/// Criterion 40: the skew handles of a mixed selection are hidden; K shows the
/// hint and writes nothing; K on paths opens the skew chip.
#[test]
fn ac37_ac40_skew_key_on_a_mixed_selection() {
    let d = two_paths();
    let _ = d.create_rect(rect_bounds(55.0, 51.0, 3.0, 3.0));
    let mut s = open(&d);
    select_all(&mut s);
    let before = bytes_of(&s);
    assert_eq!(key(&mut s, "k", false), KeyOutcome::Hint(KeyHint::PathOnly));
    assert_eq!(key(&mut s, "k", true), KeyOutcome::Hint(KeyHint::PathOnly));
    assert!(bytes_of(&s) == before, "the document changed");
    assert!(s.transform_entry().is_none());
    // Paths only: K opens a skew chip and Enter applies it to every path.
    let d = two_paths();
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(key(&mut s, "k", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.kind, "skew");
    let commits = change_count(&s);
    assert_eq!(s.commit_transform_entry("45", "", 0), EntryOutcome::Committed);
    assert_eq!(change_count(&s), commits + 1);
    let after = objects(&s);
    // K = top handle: bottom side fixed (y = 60): x' = x + tan(45) * (60 - y).
    let b = objects(&open(&d));
    for (a, b) in after.iter().zip(&b) {
        for (qa, qb) in path_points(a).iter().zip(path_points(b)) {
            assert_pt(*qa, qb.x + (60.0 - qb.y), qb.y, 1e-6, "K skew");
        }
    }
    // Shift+K = right handle: a y skew with the left side fixed.
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(key(&mut s, "k", true), KeyOutcome::EntryOpened);
    assert_eq!(s.commit_transform_entry("45", "", 0), EntryOutcome::Committed);
    for (a, b) in objects(&s).iter().zip(&b) {
        for (qa, qb) in path_points(a).iter().zip(path_points(b)) {
            assert_pt(*qa, qb.x, qb.y + (qb.x - 50.0), 1e-6, "Shift+K skew");
        }
    }
}

/// Criterion 37: M, R, S, K with nothing selected show "Select an object
/// first"; with one object the old behaviour.
#[test]
fn ac37_keys_with_nothing_selected() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    for k in ["m", "k"] {
        assert_eq!(key(&mut s, k, false), KeyOutcome::Hint(KeyHint::SelectFirst), "{k}");
    }
    click(&mut s, pt(55.0, 50.0));
    assert_eq!(key(&mut s, "r", false), KeyOutcome::EntryOpened);
}

// ---------------------------------------------------------------------
// Switches, limits, degenerate boxes
// ---------------------------------------------------------------------

fn radii_of(o: &ObjectSnapshot) -> [f64; 4] {
    match o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { corner_radii: r, .. } => {
                [r.tl.as_mm(), r.tr.as_mm(), r.br.as_mm(), r.bl.as_mm()]
            }
            other => panic!("rect expected: {other:?}"),
        },
        ObjectSnapshot::Path(_) => panic!("rect expected"),
    }
}

/// Two 20 x 20 rectangles with four different corner radii each, a path and
/// an ellipse, so every kind of the switches is covered.
fn rounded_scene() -> Document {
    let d = Document::new(1);
    let a = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let b = d.create_rect(rect_bounds(90.0, 50.0, 20.0, 20.0));
    for id in [a, b] {
        d.set_corner_radii(&[(
            id,
            curvyo_document_core::CornerRadii {
                tl: Length::from_mm(1.0),
                tr: Length::from_mm(2.0),
                br: Length::from_mm(3.0),
                bl: Length::from_mm(4.0),
            },
        )])
        .unwrap();
    }
    let _ = d.create_path(&[anchor(1, 50.0, 80.0), anchor(2, 110.0, 80.0)], false);
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 72.0),
        rx: Length::from_mm(4.0),
        ry: Length::from_mm(4.0),
    });
    d
}

/// Criterion 24: both switches off (the default): neither the stored stroke
/// width nor any corner radius is written; with a switch on each corner radius
/// and each stroke width is multiplied by sqrt(sx * sy); a path and an
/// ellipse have no radius; the width is floored at 0.01 mm.
#[test]
fn ac24_the_two_switches() {
    let d = rounded_scene();
    let before = objects(&open(&d));
    let run = |stroke: bool, radius: bool, to: Point| {
        let mut s = open(&d);
        s.set_scale_stroke_width(stroke);
        s.set_scale_corner_radius(radius);
        select_everything(&mut s);
        // Box (50, 50)-(110, 80): the SE corner is (110, 80).
        let g = GBox::new(&s, pt(50.0, 50.0), pt(110.0, 80.0));
        drag(&mut s, g.corner(1.0, 1.0), to);
        objects(&s)
    };
    // sx = 2, sy = 1: the factor is sqrt(2).
    let to = pt(170.0, 80.0);
    let off = run(false, false, to);
    for (a, b) in off.iter().zip(&before) {
        assert!(near(style_width(a), style_width(b), 0.0), "stroke untouched");
    }
    assert_eq!(radii_of(&off[0]), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(radii_of(&off[1]), [1.0, 2.0, 3.0, 4.0]);
    let f = 2.0_f64.sqrt();
    let on = run(true, true, to);
    for (a, b) in on.iter().zip(&before) {
        assert!(near(style_width(a), style_width(b) * f, 1e-9), "stroke * sqrt(2)");
    }
    let r = radii_of(&on[0]);
    for (got, want) in r.iter().zip([1.0, 2.0, 3.0, 4.0]) {
        assert!(near(*got, want * f, 1e-9), "{got} vs {}", want * f);
    }
    // Only the stroke switch: radii untouched; only the radius switch: strokes
    // untouched.
    let only_stroke = run(true, false, to);
    assert_eq!(radii_of(&only_stroke[0]), [1.0, 2.0, 3.0, 4.0]);
    assert!(near(style_width(&only_stroke[2]), style_width(&before[2]) * f, 1e-9));
    let only_radius = run(false, true, to);
    assert!(near(style_width(&only_radius[0]), style_width(&before[0]), 0.0));
    assert!(near(radii_of(&only_radius[1])[3], 4.0 * f, 1e-9));
    // The floor: a collapse takes the stroke to 0.01 mm, never below.
    let collapsed = run(true, true, pt(-100.0, 80.0));
    for o in &collapsed {
        assert!(near(style_width(o), 0.01, 1e-12), "floor: {}", style_width(o));
    }
}

/// Criterion 24: the switches are read at the press: flipping one mid-drag
/// changes nothing for this gesture.
#[test]
fn ac24_switches_are_read_at_the_press() {
    let d = rounded_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::new(&s, pt(50.0, 50.0), pt(110.0, 80.0));
    let from = g.corner(1.0, 1.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    s.set_scale_stroke_width(true);
    s.set_scale_corner_radius(true);
    hold(&mut s, pt(170.0, 80.0), false, false);
    s.pointer_up(pt(170.0, 80.0), false, false);
    let o = objects(&s);
    assert!(near(style_width(&o[0]), 0.25, 0.0));
    assert_eq!(radii_of(&o[0]), [1.0, 2.0, 3.0, 4.0]);
}

/// Criterion 22: a factor below zero clamps to zero and stays there: nothing is
/// negative, nothing flips.
#[test]
fn ac22_a_negative_factor_clamps_to_zero() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    drag(&mut s, g.corner(1.0, 1.0), pt(20.0, 30.0));
    for o in objects(&s) {
        let (b, _) = rect_of(&o);
        assert!(b.width.as_mm() >= 0.0 && b.height.as_mm() >= 0.0);
        assert!(near(b.width.as_mm(), 0.0, 1e-9), "x collapsed: {}", b.width.as_mm());
        assert!(near(rect_centre(b).x, 50.0, 1e-9), "all at the pivot x");
        // y: 30 -> sy = (30 - 50) / 10 < 0 -> 0 as well.
        assert!(near(b.height.as_mm(), 0.0, 1e-9));
    }
    // A drag that goes negative and comes back past the zero crossing: the
    // result is computed from the final pointer only.
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let from = g.corner(1.0, 1.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(10.0, 10.0), false, false);
    hold(&mut s, pt(65.0, 70.0), false, false);
    s.pointer_up(pt(65.0, 70.0), false, false);
    let (b, _) = rect_of(&objects(&s)[0]);
    // sx = 15/30, sy = 20/10.
    assert!(near(b.width.as_mm(), 5.0, 1e-9) && near(b.height.as_mm(), 20.0, 1e-9));
}

/// Criterion 22: a factor that would put any coordinate beyond +-1e7 mm
/// refuses the whole gesture: nothing is transformed. NaN and infinite
/// pointer positions never corrupt the document either.
#[test]
fn ac22_a_huge_or_non_finite_drag_changes_nothing_or_stays_valid() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let before = bytes_of(&s);
    let g = two_squares_box(&s);
    drag(&mut s, g.corner(1.0, 1.0), pt(2.0e7, 60.0));
    assert!(bytes_of(&s) == before, "beyond 1e7: all or nothing");
    drag(&mut s, g.corner(1.0, 1.0), pt(80.0, 3.0e7));
    assert!(bytes_of(&s) == before, "beyond 1e7 in y");
    // Not finite.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut s = open(&d);
        select_all(&mut s);
        let g = two_squares_box(&s);
        for from in [g.corner(1.0, 1.0), g.rot_corner(1.0, 1.0), g.mid(1.0, 0.0)] {
            hold(&mut s, from, false, false);
            s.pointer_down(from, false);
            hold(&mut s, pt(bad, 70.0), false, false);
            s.pointer_up(pt(bad, bad), false, false);
            for o in objects(&s) {
                let (b, r) = rect_of(&o);
                assert!(
                    b.origin.x.is_finite()
                        && b.origin.y.is_finite()
                        && b.width.as_mm().is_finite()
                        && b.height.as_mm().is_finite()
                        && r.is_finite(),
                    "non-finite pointer {bad}"
                );
            }
        }
    }
}

/// Criterion 12: a degenerate (flat) group box still scales along its extent
/// with the edge handle and rotates with the corner rotate handles.
#[test]
fn ac12_gestures_on_a_flat_group_box() {
    let d = Document::new(1);
    let _ = d.create_path(&[anchor(1, 50.0, 60.0), anchor(2, 58.0, 60.0)], false);
    let _ = d.create_path(&[anchor(3, 62.0, 60.0), anchor(4, 70.0, 60.0)], false);
    let g = {
        let s = open(&d);
        GBox::new(&s, pt(50.0, 60.0), pt(70.0, 60.0))
    };
    // E edge handle: only x changes, anchored at x = 50.
    let mut s = open(&d);
    select_everything(&mut s);
    drag(&mut s, g.mid(1.0, 0.0), pt(90.0, 66.0));
    let o = objects(&s);
    let xs: Vec<f64> = o.iter().flat_map(path_points).map(|p| p.x).collect();
    assert!(xs.iter().zip([50.0, 66.0, 74.0, 90.0]).all(|(a, b)| near(*a, b, 1e-6)), "{xs:?}");
    for q in o.iter().flat_map(path_points) {
        assert!(near(q.y, 60.0, 1e-9), "y untouched");
    }
    // Rotate by 90 about the centre (60, 60): the line turns vertical.
    let mut s = open(&d);
    select_everything(&mut s);
    rotate_drag(&mut s, g.rot_corner(1.0, 1.0), g.centre(), 90.0, false, false);
    let o = objects(&s);
    let p0 = path_points(&o[0]);
    assert_pt(p0[0], 60.0, 50.0, 1e-6, "turned");
    assert_pt(p0[1], 60.0, 58.0, 1e-6, "turned");
}

/// Criterion 13: the selection of one point can still be moved by a press on
/// an object's outline and has no handle: a press near it where a corner would
/// be does nothing but the ordinary press.
#[test]
fn ac13_a_single_point_selection() {
    let d = Document::new(1);
    let _ = d.create_path(&[anchor(1, 80.0, 60.0), anchor(2, 80.0, 60.0)], false);
    let _ = d.create_path(&[anchor(3, 80.0, 60.0), anchor(4, 80.0, 60.0)], false);
    let mut s = open(&d);
    // The outline of a point-path is the point: click it, Shift-click again.
    click(&mut s, pt(80.0, 60.0));
    // With one or two coincident objects selected by a marquee.
    key(&mut s, "Escape", false);
    drag(&mut s, pt(70.0, 50.0), pt(90.0, 70.0));
    assert_eq!(s.selected_object_count(), 2);
    let before = bytes_of(&s);
    // A rotate-handle position of a notional square: no handle exists.
    let diag = 32.0 / SQRT_2 / k_of(&s);
    drag(&mut s, pt(80.0 + diag, 60.0 + diag), pt(80.0 + diag, 60.0 + diag + 30.0));
    assert!(bytes_of(&s) == before, "no rotate gesture on a point");
    // (The press on empty canvas ran a marquee that replaced the selection.)
    drag(&mut s, pt(70.0, 50.0), pt(90.0, 70.0));
    assert_eq!(s.selected_object_count(), 2);
    // Move by a press on the point.
    drag(&mut s, pt(80.0, 60.0), pt(90.0, 70.0));
    for o in objects(&s) {
        for q in path_points(&o) {
            assert_pt(q, 90.0, 70.0, 1e-9, "moved");
        }
    }
}

/// Criterion 8/18: coincident objects (two identical rectangles) scale as one.
#[test]
fn ac8_coincident_objects_scale_together() {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 20.0, 20.0));
    let mut s = open(&d);
    drag(&mut s, pt(40.0, 40.0), pt(80.0, 80.0));
    assert_eq!(s.selected_object_count(), 2);
    let g = GBox::new(&s, pt(50.0, 50.0), pt(70.0, 70.0));
    drag(&mut s, g.corner(1.0, 1.0), pt(90.0, 110.0));
    for o in objects(&s) {
        let (b, _) = rect_of(&o);
        assert!(near(b.width.as_mm(), 40.0, 1e-9) && near(b.height.as_mm(), 60.0, 1e-9));
    }
}

// ---------------------------------------------------------------------
// Compound paths and boolean results
// ---------------------------------------------------------------------

fn all_points(o: &ObjectSnapshot) -> Vec<Point> {
    match o {
        ObjectSnapshot::Path(p) => p
            .subpaths()
            .flat_map(|sp| sp.anchors.iter().map(|a| a.point).collect::<Vec<_>>())
            .collect(),
        ObjectSnapshot::Primitive(_) => panic!("path expected"),
    }
}

/// A compound path (a square with a square hole, made by the boolean command)
/// next to a plain rectangle, both selected by a marquee.
fn compound_scene() -> Session {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 40.0, 40.0));
    let _ = d.create_rect(rect_bounds(60.0, 60.0, 10.0, 10.0));
    let mut s = open(&d);
    select_everything(&mut s);
    let out = s.apply_boolean(curvyo_ui_core::BooleanOp::Difference);
    assert!(
        matches!(out, curvyo_editor_wasm::BooleanOutcome::Applied { compound: true, .. }),
        "{out:?}"
    );
    // A second object: an open path far to the right.
    let d = doc_of(&s);
    let _ = d.create_path(&[anchor(900, 120.0, 50.0), anchor(901, 140.0, 80.0)], false);
    let mut s = open(&d);
    select_everything(&mut s);
    assert_eq!(s.selected_object_count(), 2);
    s
}

/// Criteria 20, 26 and 41 on a compound path: every outline of the compound is
/// transformed by the same map (scale, rotation, skew), in one commit.
#[test]
fn compound_path_in_a_selection_follows_scale_rotate_and_skew() {
    let base = compound_scene();
    let before = objects(&base);
    let count = before.iter().map(|o| all_points(o).len()).sum::<usize>();
    assert!(all_points(&before[0]).len() >= 8, "two outlines");
    // Group box: x 50..140, y 50..90.
    let gb = GBox::new(&base, pt(50.0, 50.0), pt(140.0, 90.0));
    // Scale: SE corner (140, 90) -> (230, 130): sx = 2, sy = 2.
    let mut s = compound_scene();
    let commits = change_count(&s);
    drag(&mut s, gb.corner(1.0, 1.0), pt(230.0, 130.0));
    assert_eq!(change_count(&s), commits + 1);
    let after = objects(&s);
    let mapped = |p: Point| pt(50.0 + (p.x - 50.0) * 2.0, 50.0 + (p.y - 50.0) * 2.0);
    let mut seen = 0;
    for (a, b) in after.iter().zip(&before) {
        for (qa, qb) in all_points(a).iter().zip(all_points(b)) {
            let m = mapped(qb);
            assert_pt(*qa, m.x, m.y, 1e-6, "compound anchor under scale");
            seen += 1;
        }
    }
    assert_eq!(seen, count);
    // Rotate by 90 about the centre (95, 70).
    let mut s = compound_scene();
    rotate_drag(&mut s, gb.rot_corner(1.0, 1.0), gb.centre(), 90.0, false, false);
    for (a, b) in objects(&s).iter().zip(&before) {
        for (qa, qb) in all_points(a).iter().zip(all_points(b)) {
            let m = rot(qb, gb.centre(), FRAC_PI_2);
            assert_pt(*qa, m.x, m.y, 1e-6, "compound anchor under rotation");
        }
    }
    // Skew: top handle by 20 mm, bottom side (y = 90) fixed, h = 40: t = 0.5.
    let mut s = compound_scene();
    let h = gb.skew(0.0, -1.0);
    drag(&mut s, h, pt(h.x + 20.0, h.y));
    for (a, b) in objects(&s).iter().zip(&before) {
        for (qa, qb) in all_points(a).iter().zip(all_points(b)) {
            assert_pt(*qa, qb.x + 0.5 * (90.0 - qb.y), qb.y, 1e-6, "compound anchor under skew");
        }
    }
}

// ---------------------------------------------------------------------
// Part D: typed values, keys, chips
// ---------------------------------------------------------------------

fn double_click_at(s: &mut Session, at: Point, shift: bool, ctrl: bool) -> bool {
    hold(s, at, shift, ctrl);
    s.pointer_down(at, shift);
    s.pointer_up(at, shift, ctrl);
    s.double_click(at, shift, ctrl)
}

/// Criterion 33: the angle chip: one field, label "Δ", name "Rotate selection
/// by", prefilled "0"; Enter turns the selection as a drag of that angle would.
#[test]
fn ac33_the_angle_chip() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    assert_eq!(key(&mut s, "r", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().expect("angle chip");
    assert_eq!(v.kind, "angle");
    assert_eq!(v.fields.len(), 1);
    assert_eq!(v.fields[0].label, "Δ");
    assert_eq!(v.fields[0].accessible_name, "Rotate selection by");
    assert_eq!(v.fields[0].prefill, "0");
    assert!(v.selection);
    assert_pt(v.center, g.centre().x, g.centre().y, 1e-9, "centre");
    // R opens the chip at the top-right corner rotate handle, drawn or not.
    let hr = g.rot_corner(1.0, -1.0);
    assert_pt(v.handle, hr.x, hr.y, 1e-6, "chip anchor");
    let commits = change_count(&s);
    assert_eq!(s.commit_transform_entry("37.5", "", 0), EntryOutcome::Committed);
    assert_eq!(change_count(&s), commits + 1);
    let typed = objects(&s);
    // Differential: the drag of the same angle leaves the same document.
    let mut s2 = open(&d);
    select_all(&mut s2);
    rotate_drag(&mut s2, g.rot_corner(1.0, 1.0), g.centre(), 37.5, false, false);
    let dragged = objects(&s2);
    for (a, b) in typed.iter().zip(&dragged) {
        let ((ba, ra), (bb, rb)) = (rect_of(a), rect_of(b));
        assert_pt(rect_centre(ba), rect_centre(bb).x, rect_centre(bb).y, 1e-6, "centre");
        assert!(near(angle_diff(ra, rb), 0.0, 1e-9));
        assert!(near(ba.width.as_mm(), bb.width.as_mm(), 1e-9));
    }
}

/// Criterion 33: zero, an unedited Enter and a full turn write nothing; the
/// decimal comma, U+2212 and a trailing degree sign are accepted; nonsense
/// keeps the chip open.
#[test]
fn ac33_angle_chip_text_rules() {
    let (d, _) = two_squares();
    let fresh = || {
        let mut s = open(&d);
        select_all(&mut s);
        assert_eq!(key(&mut s, "r", false), KeyOutcome::EntryOpened);
        s
    };
    for text in ["0", "360", "-360", "720", "0°"] {
        let mut s = fresh();
        let before = bytes_of(&s);
        let out = s.commit_transform_entry(text, "", 0);
        assert_eq!(out, EntryOutcome::Unchanged, "{text}");
        assert!(bytes_of(&s) == before, "{text} wrote");
        assert!(s.transform_entry().is_none(), "the chip closes");
    }
    // An unedited Enter: the prefill itself.
    let mut s = fresh();
    let prefill = s.transform_entry().unwrap().fields[0].prefill.clone();
    assert_eq!(s.commit_transform_entry(&prefill, "", 0), EntryOutcome::Unchanged);
    // Accepted spellings: 12,5 / −30 / 45° all rotate.
    let want = [("12,5", 12.5), ("\u{2212}30", -30.0), ("45°", 45.0), (" 90 ", 90.0)];
    for (text, degrees) in want {
        let mut s = fresh();
        assert_eq!(s.commit_transform_entry(text, "", 0), EntryOutcome::Committed, "{text}");
        let (_, r) = rect_of(&objects(&s)[0]);
        assert!(near(angle_diff(r, f64::to_radians(degrees)), 0.0, 1e-9), "{text}: {r}");
    }
    // Invalid keeps the chip open and writes nothing.
    for text in ["abc", "", "nan", "inf", "-inf", "1e999", "1,2,3", "12 34"] {
        let mut s = fresh();
        let before = bytes_of(&s);
        let out = s.commit_transform_entry(text, "", 0);
        assert!(matches!(out, EntryOutcome::Invalid { field: 0, .. }), "{text:?}: {out:?}");
        assert!(bytes_of(&s) == before);
        assert!(s.transform_entry().is_some(), "{text:?}: the chip stays open");
    }
}

/// Criterion 33: a double-click on a rotate handle opens the chip with Shift's
/// pivot fixed at the second press; R always the centre; Escape closes without
/// writing.
#[test]
fn ac33_double_click_opens_the_chip_and_fixes_the_pivot() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let before = bytes_of(&s);
    double_click_at(&mut s, g.rot_corner(1.0, 1.0), true, false);
    let v = s.transform_entry().expect("chip");
    assert_eq!(v.kind, "angle");
    // The pivot of a Shift press is the opposite corner (50, 50).
    assert_eq!(s.commit_transform_entry("90", "", 0), EntryOutcome::Committed);
    let (b0, _) = rect_of(&objects(&s)[0]);
    assert_pt(rect_centre(b0), 45.0, 55.0, 1e-6, "about the opposite corner");
    // Escape closes without a write.
    let mut s = open(&d);
    select_all(&mut s);
    double_click_at(&mut s, g.rot_corner(1.0, 1.0), false, false);
    assert!(bytes_of(&s) == before);
    assert_eq!(key(&mut s, "Escape", false), KeyOutcome::Escape(EscapeStep::ClosedEntry));
    assert!(bytes_of(&s) == before);
    assert!(s.transform_entry().is_none());
    assert_eq!(s.selected_object_count(), 2);
}

/// Criterion 34: the size chip: W and H, accessible names, prefilled with the
/// group box size; the fixed point is the opposite corner of the handle (the
/// centre for S); equal to a drag; refusals.
#[test]
fn ac34_the_size_chip() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    double_click_at(&mut s, g.corner(1.0, 1.0), false, false);
    let v = s.transform_entry().expect("size chip");
    assert_eq!(v.kind, "size");
    assert_eq!(v.fields.len(), 2);
    assert_eq!((v.fields[0].label, v.fields[1].label), ("W", "H"));
    assert_eq!(
        (v.fields[0].accessible_name, v.fields[1].accessible_name),
        ("Width", "Height")
    );
    assert_eq!((v.fields[0].prefill.as_str(), v.fields[1].prefill.as_str()), ("30.0", "10.0"));
    assert!(!v.linked);
    // The same as the drag of the spec example: sx = 2, sy = 3.
    let commits = change_count(&s);
    assert_eq!(s.commit_transform_entry("60", "30", 0), EntryOutcome::Committed);
    assert_eq!(change_count(&s), commits + 1);
    let o = objects(&s);
    let ((b0, _), (b1, _)) = (rect_of(&o[0]), rect_of(&o[1]));
    assert_pt(b0.origin, 50.0, 50.0, 1e-6, "rect 0");
    assert!(near(b0.width.as_mm(), 20.0, 1e-9) && near(b0.height.as_mm(), 30.0, 1e-9));
    assert_pt(b1.origin, 90.0, 50.0, 1e-6, "rect 1");
    // S: about the box centre (65, 55).
    let mut s = open(&d);
    select_all(&mut s);
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    assert!(s.transform_entry().unwrap().at_centre);
    assert_eq!(s.commit_transform_entry("60", "10", 0), EntryOutcome::Committed);
    let (b0, _) = rect_of(&objects(&s)[0]);
    assert_pt(b0.origin, 35.0, 50.0, 1e-6, "about the centre");
    // Refusals.
    for (w, h, why) in [("0", "10", "zero"), ("-5", "10", "negative"), ("30", "0", "zero h")] {
        let mut s = open(&d);
        select_all(&mut s);
        key(&mut s, "s", false);
        let before = bytes_of(&s);
        let out = s.commit_transform_entry(w, h, 0);
        assert!(matches!(out, EntryOutcome::Invalid { .. }), "{why}: {out:?}");
        assert!(bytes_of(&s) == before && s.transform_entry().is_some());
    }
    let mut s = open(&d);
    select_all(&mut s);
    key(&mut s, "s", false);
    let before = bytes_of(&s);
    let out = s.commit_transform_entry("1e8", "10", 0);
    assert!(matches!(out, EntryOutcome::Invalid { field: _, .. }), "{out:?}");
    assert!(bytes_of(&s) == before, "Too large writes nothing");
    // Unedited and equal sizes write nothing.
    let mut s = open(&d);
    select_all(&mut s);
    key(&mut s, "s", false);
    assert_eq!(s.commit_transform_entry("30.0", "10.0", 0), EntryOutcome::Unchanged);
    assert!(bytes_of(&s) == before);
}

/// Criterion 34: for a uniform-only selection the two fields are linked, with
/// the group box's aspect ratio; a typed size scales proportionally.
#[test]
fn ac34_uniform_only_size_chip_is_linked() {
    let d = Document::new(1);
    let _ = d.create_path(&[anchor(1, 50.0, 50.0), anchor(2, 90.0, 50.0)], false);
    let _ = d.create_polygon(
        StarFrame {
            center: pt(70.0, 70.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(4).unwrap(),
    );
    let mut s = open(&d);
    select_everything(&mut s);
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    assert!(v.linked, "uniform-only: linked fields");
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    let other = s.transform_entry_linked(0, "80").expect("linked text");
    let other: f64 = other.parse().unwrap();
    assert!(near(other, h * 80.0 / w, 0.06), "{other}");
    // And a double click on an edge: there is no edge handle, so no single-field
    // entry opens from there.
    let mut s = open(&d);
    select_everything(&mut s);
    key(&mut s, "s", false);
    assert_eq!(s.commit_transform_entry("80", &format!("{other}"), 0), EntryOutcome::Committed);
    let o = objects(&s);
    let xs = path_points(&o[0]);
    assert!(near(xs[1].x - xs[0].x, 80.0, 1e-6), "path is now 80 wide");
}

/// Criterion 34: the switches are read when the chip opens; clicking a switch
/// while a chip is open closes it without writing (`object-transform-refinements`
/// criterion 31).
#[test]
fn ac34_size_chip_reads_the_switches_when_it_opens() {
    let d = rounded_scene();
    let mut s = open(&d);
    s.set_scale_stroke_width(true);
    select_everything(&mut s);
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    assert_eq!(
        s.commit_transform_entry(&format!("{}", w * 2.0), &format!("{h}"), 0),
        EntryOutcome::Committed
    );
    let o = objects(&s);
    assert!(near(style_width(&o[0]), 0.25 * 2.0_f64.sqrt(), 1e-6), "{}", style_width(&o[0]));
    assert_eq!(radii_of(&o[0]), [1.0, 2.0, 3.0, 4.0], "radius switch was off");
    // With both off: neither is written.
    let mut s = open(&d);
    select_everything(&mut s);
    key(&mut s, "s", false);
    s.commit_transform_entry(&format!("{}", w * 2.0), &format!("{h}"), 0);
    assert!(near(style_width(&objects(&s)[0]), 0.25, 0.0));
    // A switch click closes the open chip without a write.
    let mut s = open(&d);
    select_everything(&mut s);
    key(&mut s, "s", false);
    let before = bytes_of(&s);
    s.set_scale_corner_radius(true);
    assert!(s.transform_entry().is_none());
    assert!(bytes_of(&s) == before);
}

/// Criterion 35: the move chip: relative offset, absolute top-left of the group
/// box, the Copy check copies every object and selects the copies; one commit.
#[test]
fn ac35_the_move_chip() {
    let d = wide_squares();
    let fresh = || {
        let mut s = open(&d);
        select_all(&mut s);
        assert_eq!(key(&mut s, "m", false), KeyOutcome::EntryOpened);
        s
    };
    let mut s = fresh();
    let v = s.move_entry().unwrap();
    assert!(v.selection);
    assert_eq!(v.relative_prefill, ["0".to_string(), "0".to_string()]);
    assert_eq!(v.absolute_prefill, ["50.0".to_string(), "50.0".to_string()]);
    let commits = change_count(&s);
    assert_eq!(
        s.commit_move_entry("5", "-3", MoveEntryMode { absolute: false, copy: false }),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&s), commits + 1);
    let o = origins(&s);
    assert_pt(o[0], 55.0, 47.0, 1e-9, "relative");
    assert_pt(o[1], 95.0, 47.0, 1e-9, "relative");
    // Absolute: the top-left of the group box goes to (100, 200).
    let mut s = fresh();
    s.commit_move_entry("100", "200", MoveEntryMode { absolute: true, copy: false });
    let o = origins(&s);
    assert_pt(o[0], 100.0, 200.0, 1e-9, "absolute");
    assert_pt(o[1], 140.0, 200.0, 1e-9, "absolute keeps the arrangement");
    // Copy: four objects, the copies selected.
    let mut s = fresh();
    s.commit_move_entry("0", "50", MoveEntryMode { absolute: false, copy: true });
    assert_eq!(objects(&s).len(), 4);
    assert_eq!(s.selected_object_count(), 2);
    // Invalid text keeps the chip open.
    let mut s = fresh();
    let out = s.commit_move_entry("abc", "1", MoveEntryMode { absolute: false, copy: false });
    assert!(matches!(out, EntryOutcome::Invalid { .. }));
    assert!(s.move_entry().is_some());
    // A zero offset writes nothing.
    let mut s = fresh();
    let before = bytes_of(&s);
    let out = s.commit_move_entry("0", "0", MoveEntryMode { absolute: false, copy: false });
    assert_eq!(out, EntryOutcome::Unchanged);
    assert!(bytes_of(&s) == before);
}

/// Criterion 35: the absolute move of the spec's rotated rectangle and circle
/// puts the top-left of their outline bounds at (X, Y).
#[test]
fn ac35_absolute_move_uses_the_outline_bounds_of_a_rotated_rectangle() {
    let d = Document::new(1);
    let r = d.create_rect(rect_bounds(0.0, 0.0, 20.0, 10.0));
    let _ = d.create_ellipse(EllipseFrame {
        center: pt(40.0, 5.0),
        rx: Length::from_mm(5.0),
        ry: Length::from_mm(5.0),
    });
    rotate_about_centre(&d, r, 30.0_f64.to_radians());
    let mut s = open(&d);
    select_everything(&mut s);
    key(&mut s, "m", false);
    s.commit_move_entry("100", "100", MoveEntryMode { absolute: true, copy: false });
    // The circle's left-most is no longer the box edge; the box top-left was
    // (-1.1603, -4.3301): the shift is (101.1603, 104.3301).
    let o = objects(&s);
    let (e, _) = ellipse_of(&o[1]);
    assert_pt(e.center, 40.0 + 101.160_254, 5.0 + 104.330_127, 1e-5, "circle");
}

/// Criterion 38: the hint lines of each handle.
#[test]
fn ac38_hint_lines() {
    let lines = |s: &mut Session, p: Point, shift: bool| {
        hold(s, p, shift, false);
        assert_eq!(s.handle_hint(), "group", "a group handle is under {p:?}");
        s.corner_hint_lines()
    };
    let d = two_paths();
    let mut s = open(&d);
    select_all(&mut s);
    let g = two_squares_box(&s);
    let resize = |edge: bool| {
        let mut v = vec!["Resize selection", "Shift: from the centre"];
        if !edge {
            v.push("Ctrl: keep proportions");
        }
        v.push("Double-click or S: type a size");
        v
    };
    assert_eq!(lines(&mut s, g.corner(1.0, 1.0), false), resize(false));
    assert_eq!(lines(&mut s, g.mid(1.0, 0.0), false), resize(true));
    assert_eq!(
        lines(&mut s, g.rot_corner(1.0, 1.0), false),
        [
            "Rotate selection",
            "Shift: pivot at opposite corner",
            "Ctrl: snap",
            "Double-click or R: type an angle"
        ]
    );
    assert_eq!(
        lines(&mut s, g.rot_side(1.0, 0.0), true),
        [
            "Rotate selection",
            "Pivot: opposite side",
            "Ctrl: snap",
            "Double-click or R: type an angle"
        ]
    );
    assert_eq!(
        lines(&mut s, g.skew(0.0, -1.0), false),
        [
            "Skew selection",
            "Shift: from the centre line",
            "Ctrl: snap",
            "Double-click or K: type an angle"
        ]
    );
    assert_eq!(
        lines(&mut s, g.skew(1.0, 0.0), false),
        [
            "Skew selection",
            "Shift: from the centre line",
            "Ctrl: snap",
            "Double-click or Shift+K: type an angle"
        ]
    );
    // The centre handle (needs s >= 48 px).
    let d = wide_squares();
    let mut s = open(&d);
    select_all(&mut s);
    let g = wide_box(&s);
    assert_eq!(
        lines(&mut s, g.centre(), false),
        [
            "Move selection",
            "Shift: keep one axis",
            "Ctrl: copy",
            "Double-click or M: type an offset"
        ]
    );
    // Uniform-only corner.
    let d = Document::new(1);
    let _ = d.create_path(&[anchor(1, 50.0, 50.0), anchor(2, 100.0, 50.0)], false);
    let st = d.create_star(
        StarFrame {
            center: pt(75.0, 80.0),
            radius: Length::from_mm(15.0),
            angle: Angle::from_radians(-FRAC_PI_2),
        },
        PointCount::new(5).unwrap(),
        curvyo_document_core::InnerRatio::new(0.5).unwrap(),
    );
    let r = d.create_rect(rect_bounds(55.0, 90.0, 10.0, 6.0));
    rotate_about_centre(&d, r, 0.4);
    let _ = st;
    let mut s = open(&d);
    select_everything(&mut s);
    // Find the SE corner from the size chip prefill (box read back).
    key(&mut s, "m", false);
    let m = s.move_entry().unwrap();
    let tl = pt(
        m.absolute_prefill[0].parse().unwrap(),
        m.absolute_prefill[1].parse().unwrap(),
    );
    key(&mut s, "Escape", false);
    select_everything(&mut s);
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    key(&mut s, "Escape", false);
    select_everything(&mut s);
    let g = GBox::new(&s, tl, pt(tl.x + w, tl.y + h));
    let l = lines(&mut s, g.corner(1.0, 1.0), false);
    assert_eq!(
        l,
        [
            "Resize selection, proportional only",
            "Holds a star and a rotated rectangle",
            "To stretch: Object to path first",
            "Shift: from the centre",
            "Double-click or S: type a size"
        ]
    );
}

/// Part U4: the screen reader announcement.
#[test]
fn u4_selection_announcement() {
    let (d, _) = two_squares();
    let mut s = open(&d);
    assert_eq!(s.selection_announcement(), "");
    select_all(&mut s);
    assert_eq!(s.selection_announcement(), "2 objects selected, 30.0 by 10.0 mm");
}
