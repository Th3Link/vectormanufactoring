//! Independent tester acceptance tests for the stretch-with-conversion milestone of
//! `specs/0019-multi-object-transform/specification.md` (criteria 18 to 20, 22 to
//! 24, 29 to 32, 34, 38, 39, 48 to 50, 52 to 56). Written from the specification
//! before the implementation was read. Everything goes through `Session`'s public
//! API (pointer events, keys, chips). Expected values are reference models written
//! here: a linear map applied to the outline of the old object, an analytic
//! ellipse test, and the explicit "Object to path" of a second session for the
//! Bezier approximation the spec names as the only allowed difference.

// Test code: the arithmetic is written the way the specification states it.
#![allow(
    clippy::manual_assert_eq,
    clippy::manual_midpoint,
    clippy::collapsible_if
)]
#![allow(clippy::unneeded_wildcard_pattern, clippy::unreadable_literal)]
#![allow(clippy::used_underscore_binding, clippy::useless_conversion)]
#![allow(clippy::suboptimal_flops, clippy::imprecise_flops)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::many_single_char_names, clippy::similar_names)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]
#![allow(clippy::items_after_statements, clippy::too_many_arguments)]
#![allow(
    missing_docs,
    clippy::type_complexity,
    clippy::cast_possible_truncation
)]
#![allow(clippy::cast_sign_loss, clippy::needless_range_loop)]
#![allow(
    clippy::manual_range_patterns,
    clippy::assert_is_empty,
    clippy::if_not_else
)]

use std::f64::consts::{FRAC_PI_2, PI};

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, CornerRadii, DashPattern, Document, EllipseFrame, InnerRatio,
    Length, NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, PointCount, RectBounds, Shape,
    StarFrame, StyleEdit, Vec2, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{KeyInput, KeyOutcome, Session, Tool};
use curvyo_ui_core::EntryOutcome;

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

fn frame_centre(o: &ObjectSnapshot) -> Point {
    match o {
        ObjectSnapshot::Primitive(p) => match p.shape {
            Shape::Rect { bounds, .. } => rect_centre(bounds),
            Shape::Ellipse { frame } => frame.center,
            Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => frame.center,
        },
        ObjectSnapshot::Path(_) => panic!("primitives only"),
    }
}

fn rotate_about_centre(d: &Document, id: NodeId, radians: f64) {
    let o = d.object(id).unwrap();
    let c = frame_centre(&o);
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

fn ids(s: &Session) -> Vec<NodeId> {
    doc_of(s).object_ids()
}

fn loro_of(d: &Document) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn change_count(s: &Session) -> usize {
    loro_of(&doc_of(s)).len_changes()
}

fn last_label(d: &Document) -> String {
    let l = loro_of(d);
    let vv = l.oplog_vv();
    let mut best: Option<(u32, String)> = None;
    for (p, e) in vv.iter() {
        let c = l.get_change(loro::ID::new(*p, *e - 1)).unwrap();
        if best.as_ref().is_none_or(|(b, _)| c.lamport >= *b) {
            best = Some((c.lamport, c.message().to_string()));
        }
    }
    best.unwrap().1
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

fn drag_mod(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    hold(s, from, shift, ctrl);
    s.pointer_down(from, shift);
    hold(s, to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
    hold(s, to, false, false);
}

fn click(s: &mut Session, p: Point) {
    hold(s, p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn key(s: &mut Session, key: &str, shift: bool) -> KeyOutcome {
    s.key_down(KeyInput {
        key,
        shift,
        ..KeyInput::default()
    })
}

fn select_everything(s: &mut Session) {
    let n = doc_of(s).object_ids().len();
    key(s, "Escape", false);
    drag_mod(s, pt(-300.0, -300.0), pt(900.0, 900.0), false, false);
    assert_eq!(s.selected_object_count(), n);
}

fn rect_centre(b: RectBounds) -> Point {
    pt(
        b.origin.x + b.width.as_mm() / 2.0,
        b.origin.y + b.height.as_mm() / 2.0,
    )
}

fn rotation_of(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.rotation.as_radians(),
        ObjectSnapshot::Path(p) => p.rotation.as_radians(),
    }
}

fn is_path(o: &ObjectSnapshot) -> bool {
    matches!(o, ObjectSnapshot::Path(_))
}

fn style_debug(o: &ObjectSnapshot) -> String {
    match o {
        ObjectSnapshot::Primitive(p) => format!("{:?}", p.style),
        ObjectSnapshot::Path(p) => format!("{:?}", p.style),
    }
}

fn stroke_width(o: &ObjectSnapshot) -> f64 {
    match o {
        ObjectSnapshot::Primitive(p) => p.style.stroke.width.as_mm(),
        ObjectSnapshot::Path(p) => p.style.stroke.width.as_mm(),
    }
}

fn dash_debug(o: &ObjectSnapshot) -> String {
    match o {
        ObjectSnapshot::Primitive(p) => format!("{:?}", p.style.stroke.dash),
        ObjectSnapshot::Path(p) => format!("{:?}", p.style.stroke.dash),
    }
}

/// Dense samples of the closed or open cubic chain through `anchors`.
fn bezier_samples(anchors: &[(Point, Vec2, Vec2)], closed: bool) -> Vec<Point> {
    let n = anchors.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    let mut out = Vec::new();
    for i in 0..segs {
        let (a, ao_in, a_out) = anchors[i];
        let (b, b_in, _) = anchors[(i + 1) % n];
        let _ = ao_in;
        let p0 = a;
        let p1 = pt(a.x + a_out.x, a.y + a_out.y);
        let p2 = pt(b.x + b_in.x, b.y + b_in.y);
        let p3 = b;
        for step in 0..=4000 {
            let t = f64::from(step) / 4000.0;
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
    if out.is_empty() {
        out.extend(anchors.iter().map(|a| a.0));
    }
    out
}

fn flat(o: &ObjectSnapshot) -> Vec<Point> {
    match o {
        ObjectSnapshot::Primitive(p) => {
            let out = outline_of_rotated(&p.shape, p.rotation);
            let a: Vec<_> = out
                .iter()
                .map(|a| (a.point, a.handle_in, a.handle_out))
                .collect();
            bezier_samples(&a, true)
        }
        ObjectSnapshot::Path(p) => p
            .subpaths()
            .flat_map(|sp| {
                let a: Vec<_> = sp
                    .anchors
                    .iter()
                    .map(|a| (a.point, a.handle_in, a.handle_out))
                    .collect();
                bezier_samples(&a, sp.closed)
            })
            .collect(),
    }
}

/// The tight bounds of the drawn outlines of every object (criterion 1).
fn tight_bounds(os: &[ObjectSnapshot]) -> (Point, Point) {
    let mut lo = pt(f64::MAX, f64::MAX);
    let mut hi = pt(f64::MIN, f64::MIN);
    for o in os {
        for p in flat(o) {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
    }
    (lo, hi)
}

#[derive(Clone, Copy, Debug)]
struct GBox {
    min: Point,
    max: Point,
}

impl GBox {
    fn of(s: &Session) -> Self {
        let (min, max) = tight_bounds(&objects(s));
        Self { min, max }
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
    fn corner(&self, sx: f64, sy: f64) -> Point {
        let c = self.centre();
        pt(c.x + sx * self.w() / 2.0, c.y + sy * self.h() / 2.0)
    }
    fn mid(&self, nx: f64, ny: f64) -> Point {
        let c = self.centre();
        pt(c.x + nx * self.w() / 2.0, c.y + ny * self.h() / 2.0)
    }
}

/// The linear map of a scale gesture: factors (sx, sy) about (ax, ay).
#[derive(Clone, Copy, Debug)]
struct Map {
    ax: f64,
    ay: f64,
    sx: f64,
    sy: f64,
}

impl Map {
    fn p(&self, q: Point) -> Point {
        pt(
            self.ax + self.sx * (q.x - self.ax),
            self.ay + self.sy * (q.y - self.ay),
        )
    }
    fn v(&self, v: Vec2) -> Vec2 {
        Vec2::new(self.sx * v.x, self.sy * v.y)
    }
}

/// The map an edge-handle drag of side (nx, ny) to coordinate `target` (the
/// pointer's x for E/W, y for N/S) gives; the factor clamps at 0.
fn edge_map(g: &GBox, nx: f64, ny: f64, target: f64, shift: bool) -> Map {
    let c = g.centre();
    if nx != 0.0 {
        let anchor = if shift { c.x } else { c.x - nx * g.w() / 2.0 };
        let handle = c.x + nx * g.w() / 2.0;
        let f = ((target - anchor) / (handle - anchor)).max(0.0);
        Map {
            ax: anchor,
            ay: 0.0,
            sx: f,
            sy: 1.0,
        }
    } else {
        let anchor = if shift { c.y } else { c.y - ny * g.h() / 2.0 };
        let handle = c.y + ny * g.h() / 2.0;
        let f = ((target - anchor) / (handle - anchor)).max(0.0);
        Map {
            ax: 0.0,
            ay: anchor,
            sx: 1.0,
            sy: f,
        }
    }
}

fn edge_pointer(g: &GBox, nx: f64, ny: f64, target: f64) -> Point {
    let h = g.mid(nx, ny);
    if nx != 0.0 {
        pt(target, h.y)
    } else {
        pt(h.x, target)
    }
}

/// A full edge drag with the maps computed from the state at the press.
fn edge_drag(
    s: &mut Session,
    g: &GBox,
    nx: f64,
    ny: f64,
    target: f64,
    shift: bool,
    ctrl: bool,
) -> Map {
    let from = g.mid(nx, ny);
    let to = edge_pointer(g, nx, ny, target);
    drag_mod(s, from, to, shift, ctrl);
    edge_map(g, nx, ny, target, shift)
}

fn assert_pt(a: Point, b: Point, eps: f64, what: &str) {
    assert!(
        near(a.x, b.x, eps) && near(a.y, b.y, eps),
        "{what}: got ({}, {}), want ({}, {})",
        a.x,
        a.y,
        b.x,
        b.y
    );
}

fn assert_vec(a: Vec2, b: Vec2, eps: f64, what: &str) {
    assert!(
        near(a.x, b.x, eps) && near(a.y, b.y, eps),
        "{what}: got ({}, {}), want ({}, {})",
        a.x,
        a.y,
        b.x,
        b.y
    );
}

/// `new` is `old` (an "Object to path" result) with every anchor mapped and every
/// handle vector scaled (criteria 20 first row, 53.1).
fn assert_path_maps(new: &ObjectSnapshot, otp: &ObjectSnapshot, m: Map, what: &str) {
    let (ObjectSnapshot::Path(n), ObjectSnapshot::Path(o)) = (new, otp) else {
        panic!("{what}: two paths expected, got {new:?} / {otp:?}");
    };
    let ns: Vec<_> = n.subpaths().collect();
    let os: Vec<_> = o.subpaths().collect();
    assert_eq!(ns.len(), os.len(), "{what}: subpath count");
    for (ns, os) in ns.iter().zip(&os) {
        assert_eq!(ns.closed, os.closed, "{what}: closed");
        assert_eq!(ns.anchors.len(), os.anchors.len(), "{what}: node count");
        for (i, (a, b)) in ns.anchors.iter().zip(os.anchors).enumerate() {
            assert_pt(a.point, m.p(b.point), 1e-7, &format!("{what}: anchor {i}"));
            assert_vec(
                a.handle_in,
                m.v(b.handle_in),
                1e-7,
                &format!("{what}: in {i}"),
            );
            assert_vec(
                a.handle_out,
                m.v(b.handle_out),
                1e-7,
                &format!("{what}: out {i}"),
            );
            assert_eq!(a.kind, b.kind, "{what}: kind {i}");
        }
    }
}

/// The straight outline of `old` mapped by `m` is exactly the anchor set of `new`.
fn assert_straight_outline_exact(old: &ObjectSnapshot, new: &ObjectSnapshot, m: Map, what: &str) {
    let ObjectSnapshot::Primitive(p) = old else {
        panic!("primitive expected")
    };
    let out = outline_of_rotated(&p.shape, p.rotation);
    let ObjectSnapshot::Path(n) = new else {
        panic!("{what}: path expected")
    };
    assert_eq!(
        n.anchors.len(),
        out.len(),
        "{what}: node count (straight outline)"
    );
    for a in &out {
        let want = m.p(a.point);
        assert!(
            n.anchors
                .iter()
                .any(|b| near(b.point.x, want.x, 1e-7) && near(b.point.y, want.y, 1e-7)),
            "{what}: the mapped vertex ({}, {}) is missing",
            want.x,
            want.y
        );
    }
    for a in &n.anchors {
        assert!(
            near(a.handle_in.x, 0.0, 1e-9)
                && near(a.handle_in.y, 0.0, 1e-9)
                && near(a.handle_out.x, 0.0, 1e-9)
                && near(a.handle_out.y, 0.0, 1e-9),
            "{what}: a straight outline has no handles"
        );
    }
}

/// An ellipse (centre `c`, radii, `theta`) mapped by `m` equals the sampled `new`
/// path to the Bezier approximation of a quarter arc.
fn assert_stretched_ellipse(
    c: Point,
    rx: f64,
    ry: f64,
    theta: f64,
    m: Map,
    new: &ObjectSnapshot,
    what: &str,
) {
    assert!(m.sx > 1e-6 && m.sy > 1e-6, "inverse needs non-zero factors");
    for q in flat(new) {
        let p = pt(m.ax + (q.x - m.ax) / m.sx, m.ay + (q.y - m.ay) / m.sy);
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        let (s, co) = (-theta).sin_cos();
        let (u, v) = (dx * co - dy * s, dx * s + dy * co);
        let r = (u / rx).powi(2) + (v / ry).powi(2);
        assert!(
            (r.sqrt() - 1.0).abs() < 1.2e-3,
            "{what}: sample ({}, {}) is off the stretched ellipse (r = {})",
            q.x,
            q.y,
            r.sqrt()
        );
    }
}

fn otp_reference(d: &Document) -> Vec<ObjectSnapshot> {
    let mut s = open(d);
    select_everything(&mut s);
    s.convert_selected_to_paths();
    objects(&s)
}

fn set_look(d: &Document, ids: &[NodeId]) {
    d.edit_style(ids, &StyleEdit::StrokeWidth(Length::from_mm(0.8)))
        .unwrap();
    d.edit_style(
        ids,
        &StyleEdit::StrokeDash(DashPattern::new(vec![6.0, 4.0]).unwrap()),
    )
    .unwrap();
    d.edit_style(ids, &StyleEdit::StrokeOpacity(Opacity::new(0.5).unwrap()))
        .unwrap();
    d.edit_style(ids, &StyleEdit::FillEnabled(true)).unwrap();
}

fn star(d: &Document, cx: f64, cy: f64, r: f64, angle: f64) -> NodeId {
    d.create_star(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(angle),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.45).unwrap(),
    )
}

fn polygon(d: &Document, cx: f64, cy: f64, r: f64, angle: f64, n: u32) -> NodeId {
    d.create_polygon(
        StarFrame {
            center: pt(cx, cy),
            radius: Length::from_mm(r),
            angle: Angle::from_radians(angle),
        },
        PointCount::new(n).unwrap(),
    )
}

fn ellipse(d: &Document, cx: f64, cy: f64, rx: f64, ry: f64) -> NodeId {
    d.create_ellipse(EllipseFrame {
        center: pt(cx, cy),
        rx: Length::from_mm(rx),
        ry: Length::from_mm(ry),
    })
}

fn round_rect(d: &Document, x: f64, y: f64, w: f64, h: f64, r: f64) -> NodeId {
    let id = d.create_rect(rect_bounds(x, y, w, h));
    d.set_corner_radii(&[(id, CornerRadii::uniform(Length::from_mm(r)))])
        .unwrap();
    id
}

fn curved_path(d: &Document, n0: u64, cx: f64, cy: f64) -> NodeId {
    let mut a1 = anchor(n0, cx - 9.0, cy - 6.0);
    a1.handle_out = Vec2::new(3.0, -2.0);
    let mut a2 = anchor(n0 + 1, cx + 9.0, cy - 6.0);
    a2.handle_in = Vec2::new(-3.0, -2.0);
    a2.handle_out = Vec2::new(2.0, 4.0);
    a2.kind = AnchorKind::Symmetric;
    let a3 = anchor(n0 + 2, cx + 9.0, cy + 6.0);
    let a4 = anchor(n0 + 3, cx - 9.0, cy + 6.0);
    d.create_path(&[a1, a2, a3, a4], true)
}

// ---------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------

/// The spec's example (criterion 20), moved by (50, 50): a 5-point star whose
/// leftmost point is at x = 50, an unrotated rectangle from (85, 50) to (95, 60).
fn spec_example() -> (Document, NodeId, NodeId) {
    let d = Document::new(1);
    let r = 5.0;
    let st = star(
        &d,
        50.0 + r * (18.0_f64).to_radians().cos(),
        55.0,
        r,
        -FRAC_PI_2,
    );
    let rc = d.create_rect(rect_bounds(85.0, 50.0, 10.0, 10.0));
    set_look(&d, &[st, rc]);
    (d, st, rc)
}

/// Twelve objects on a grid, one per kind and rotation of the spec's table.
const KIND_NAMES: [&str; 12] = [
    "path",
    "aligned rounded rect",
    "rect rot 90",
    "rect rot 180",
    "aligned ellipse",
    "ellipse rot 90",
    "circle rot 30",
    "polygon",
    "star",
    "rect rot 30",
    "ellipse rot 30",
    "rounded rect rot 30",
];

fn kinds_scene() -> Document {
    let d = Document::new(1);
    let cell = |i: usize| (70.0 + 35.0 * (i % 4) as f64, 70.0 + 35.0 * (i / 4) as f64);
    let mut all = Vec::new();
    let (cx, cy) = cell(0);
    all.push(curved_path(&d, 1, cx, cy));
    let (cx, cy) = cell(1);
    all.push(round_rect(&d, cx - 10.0, cy - 6.0, 20.0, 12.0, 3.0));
    let (cx, cy) = cell(2);
    let id = d.create_rect(rect_bounds(cx - 10.0, cy - 6.0, 20.0, 12.0));
    rotate_about_centre(&d, id, FRAC_PI_2);
    all.push(id);
    let (cx, cy) = cell(3);
    let id = d.create_rect(rect_bounds(cx - 10.0, cy - 6.0, 20.0, 12.0));
    rotate_about_centre(&d, id, PI);
    all.push(id);
    let (cx, cy) = cell(4);
    all.push(ellipse(&d, cx, cy, 10.0, 6.0));
    let (cx, cy) = cell(5);
    let id = ellipse(&d, cx, cy, 10.0, 6.0);
    rotate_about_centre(&d, id, FRAC_PI_2);
    all.push(id);
    let (cx, cy) = cell(6);
    let id = ellipse(&d, cx, cy, 8.0, 8.0);
    rotate_about_centre(&d, id, 30.0_f64.to_radians());
    all.push(id);
    let (cx, cy) = cell(7);
    all.push(polygon(&d, cx, cy, 9.0, 0.3, 6));
    let (cx, cy) = cell(8);
    all.push(star(&d, cx, cy, 10.0, -FRAC_PI_2));
    let (cx, cy) = cell(9);
    let id = d.create_rect(rect_bounds(cx - 10.0, cy - 6.0, 20.0, 12.0));
    rotate_about_centre(&d, id, 30.0_f64.to_radians());
    all.push(id);
    let (cx, cy) = cell(10);
    let id = ellipse(&d, cx, cy, 10.0, 6.0);
    rotate_about_centre(&d, id, 30.0_f64.to_radians());
    all.push(id);
    let (cx, cy) = cell(11);
    let id = round_rect(&d, cx - 10.0, cy - 6.0, 20.0, 12.0, 3.0);
    rotate_about_centre(&d, id, 30.0_f64.to_radians());
    all.push(id);
    set_look(&d, &all);
    d
}

// ---------------------------------------------------------------------
// Criteria 20 and 53: the spec's example
// ---------------------------------------------------------------------

/// Criteria 20 and 53, the spec's own example: dragging the right edge handle of
/// a star at the left and a rectangle at the right to twice the width makes the
/// rectangle (70, 0)-(90, 10) in the example's coordinates, still a rectangle,
/// and the star a closed 10-node path, each node twice as far from the left edge.
/// The star keeps id, place in the object list and fill; the bar loses its star
/// groups; the notice counts one star; one commit.
#[test]
fn ac20_ac53_spec_example_star_and_rectangle() {
    let (d, st, rc) = spec_example();
    let before_ids = d.object_ids();
    let before_star = d.object(st).unwrap();
    let before_rect = d.object(rc).unwrap();
    let otp = otp_reference(&d);
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    assert!(
        near(g.min.x, 50.0, 1e-6) && near(g.max.x, 95.0, 1e-6),
        "box {g:?}"
    );
    assert!(
        near(g.min.y, 50.0, 1e-6) && near(g.max.y, 60.0, 1e-6),
        "box {g:?}"
    );
    let bar = s.select_bar_state();
    assert!(bar.points.is_some() && bar.ratio.is_some() && bar.radius.is_some());
    assert!(bar.object_to_path);
    let commits = change_count(&s);
    let m = edge_drag(&mut s, &g, 1.0, 0.0, 140.0, false, false);
    assert!(near(m.sx, 2.0, 1e-9) && near(m.ax, 50.0, 1e-9));
    assert_eq!(change_count(&s), commits + 1, "exactly one commit");
    assert_eq!(ids(&s), before_ids, "same ids in the same tree order");
    let o = objects(&s);
    // The rectangle: still a rectangle, (120, 50) to (140, 60).
    let ObjectSnapshot::Primitive(p) = &o[1] else {
        panic!("the rectangle stays a rectangle")
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!("the rectangle stays a rectangle")
    };
    assert_pt(bounds.origin, pt(120.0, 50.0), 1e-6, "rect origin");
    assert!(near(bounds.width.as_mm(), 20.0, 1e-6) && near(bounds.height.as_mm(), 10.0, 1e-6));
    assert_eq!(style_debug(&o[1]), style_debug(&before_rect));
    // The star: a closed path of 10 nodes at twice the distance from the left edge.
    assert!(is_path(&o[0]), "the star became a path: {:?}", o[0]);
    let ObjectSnapshot::Path(sp) = &o[0] else {
        unreachable!()
    };
    assert!(
        sp.closed && sp.anchors.len() == 10,
        "closed 10-node path, got {} nodes",
        sp.anchors.len()
    );
    assert_eq!(o[0].id(), st, "same id");
    assert_straight_outline_exact(&before_star, &o[0], m, "star");
    assert_path_maps(&o[0], &otp[0], m, "star vs Object to path");
    assert_eq!(
        style_debug(&o[0]),
        style_debug(&before_star),
        "every style field unchanged"
    );
    assert_eq!(s.selected_object_count(), 2, "selection state kept");
    // The bar lost the star groups, kept the rectangle group.
    let bar = s.select_bar_state();
    assert!(
        bar.points.is_none() && bar.ratio.is_none(),
        "no Points/Ratio after the conversion"
    );
    assert!(bar.radius.is_some(), "the rectangle group stays");
    assert!(
        bar.object_to_path,
        "Object to path is still offered for the rectangle"
    );
    // The notice: one star, once.
    assert_eq!(s.take_conversion_notice(), 1);
    assert_eq!(s.take_conversion_notice(), 0, "taken once");
    // The label.
    let label = last_label(&doc_of(&s));
    assert!(label.contains("transform"), "label of the commit: {label}");
    // The new group box is the box of the new outlines.
    let g2 = GBox::of(&s);
    assert!(
        near(g2.min.x, 50.0, 1e-6) && near(g2.max.x, 140.0, 1e-6),
        "{g2:?}"
    );
}

// ---------------------------------------------------------------------
// Criterion 20 table, criterion 53: every kind
// ---------------------------------------------------------------------

fn check_kinds_after(d: &Document, s: &Session, m: Map, otp: &[ObjectSnapshot], what: &str) {
    let before: Vec<ObjectSnapshot> = d
        .object_ids()
        .into_iter()
        .filter_map(|i| d.object(i))
        .collect();
    let after = objects(s);
    assert_eq!(ids(s), d.object_ids(), "{what}: same ids in the same order");
    let axis_swapped = |rot: f64| {
        let q = (rot / FRAC_PI_2).round();
        (q as i64).rem_euclid(2) == 1
    };
    for (i, name) in KIND_NAMES.iter().enumerate() {
        let (old, new) = (&before[i], &after[i]);
        let w = format!("{what}: {name}");
        match i {
            0 => {
                assert_path_maps(new, old, m, &w);
                assert!(
                    near(rotation_of(new), rotation_of(old), 1e-9),
                    "{w}: rotation unchanged"
                );
            }
            1 | 2 | 3 => {
                let ObjectSnapshot::Primitive(po) = old else {
                    panic!()
                };
                let ObjectSnapshot::Primitive(pn) = new else {
                    panic!("{w}: an aligned rectangle must stay a rectangle, got a path")
                };
                let (
                    Shape::Rect {
                        bounds: bo,
                        corner_radii: ro,
                    },
                    Shape::Rect {
                        bounds: bn,
                        corner_radii: rn,
                    },
                ) = (po.shape, pn.shape)
                else {
                    panic!("{w}: rectangle expected")
                };
                let (fw, fh) = if axis_swapped(po.rotation.as_radians()) {
                    (m.sy, m.sx)
                } else {
                    (m.sx, m.sy)
                };
                assert_pt(
                    rect_centre(bn),
                    m.p(rect_centre(bo)),
                    1e-7,
                    &format!("{w} centre"),
                );
                assert!(
                    near(bn.width.as_mm(), bo.width.as_mm() * fw, 1e-7),
                    "{w}: width {} want {}",
                    bn.width.as_mm(),
                    bo.width.as_mm() * fw
                );
                assert!(
                    near(bn.height.as_mm(), bo.height.as_mm() * fh, 1e-7),
                    "{w}: height"
                );
                assert!(
                    near(pn.rotation.as_radians(), po.rotation.as_radians(), 1e-9),
                    "{w}: rotation unchanged"
                );
                // "Scale corner radius" is off: radii are not rewritten.
                assert_eq!(format!("{ro:?}"), format!("{rn:?}"), "{w}: radii untouched");
            }
            4 | 5 => {
                let ObjectSnapshot::Primitive(po) = old else {
                    panic!()
                };
                let ObjectSnapshot::Primitive(pn) = new else {
                    panic!("{w}: an aligned ellipse must stay an ellipse")
                };
                let (Shape::Ellipse { frame: fo }, Shape::Ellipse { frame: fnw }) =
                    (po.shape, pn.shape)
                else {
                    panic!("{w}: ellipse expected")
                };
                let (fx, fy) = if axis_swapped(po.rotation.as_radians()) {
                    (m.sy, m.sx)
                } else {
                    (m.sx, m.sy)
                };
                assert_pt(fnw.center, m.p(fo.center), 1e-7, &format!("{w} centre"));
                assert!(near(fnw.rx.as_mm(), fo.rx.as_mm() * fx, 1e-7), "{w}: rx");
                assert!(near(fnw.ry.as_mm(), fo.ry.as_mm() * fy, 1e-7), "{w}: ry");
                assert!(
                    near(pn.rotation.as_radians(), po.rotation.as_radians(), 1e-9),
                    "{w}: rotation unchanged"
                );
            }
            6 => {
                let ObjectSnapshot::Primitive(po) = old else {
                    panic!()
                };
                let ObjectSnapshot::Primitive(pn) = new else {
                    panic!("{w}: a circle must not become a path")
                };
                let (Shape::Ellipse { frame: fo }, Shape::Ellipse { frame: fnw }) =
                    (po.shape, pn.shape)
                else {
                    panic!("{w}: ellipse expected")
                };
                assert_pt(fnw.center, m.p(fo.center), 1e-7, &format!("{w} centre"));
                assert!(
                    near(fnw.rx.as_mm(), fo.rx.as_mm() * m.sx, 1e-7),
                    "{w}: rx = r*sx, got {}",
                    fnw.rx.as_mm()
                );
                assert!(
                    near(fnw.ry.as_mm(), fo.ry.as_mm() * m.sy, 1e-7),
                    "{w}: ry = r*sy"
                );
                assert!(
                    near(pn.rotation.as_radians(), 0.0, 1e-9),
                    "{w}: a stretched circle has rotation 0 (D3)"
                );
            }
            7 | 8 | 9 => {
                assert!(is_path(new), "{w}: must become a path, got {new:?}");
                assert_eq!(new.id(), old.id(), "{w}: same id");
                assert_straight_outline_exact(old, new, m, &w);
                assert_path_maps(new, &otp[i], m, &w);
            }
            10 => {
                assert!(is_path(new), "{w}: must become a path");
                assert_path_maps(new, &otp[i], m, &w);
                let ObjectSnapshot::Primitive(po) = old else {
                    panic!()
                };
                let Shape::Ellipse { frame } = po.shape else {
                    panic!()
                };
                if m.sx > 1e-6 && m.sy > 1e-6 {
                    assert_stretched_ellipse(
                        frame.center,
                        frame.rx.as_mm(),
                        frame.ry.as_mm(),
                        po.rotation.as_radians(),
                        m,
                        new,
                        &w,
                    );
                }
            }
            11 => {
                assert!(is_path(new), "{w}: must become a path");
                assert_path_maps(new, &otp[i], m, &w);
            }
            _ => unreachable!(),
        }
        // Identity: every style field unchanged (the stroke switch is off).
        assert_eq!(style_debug(new), style_debug(old), "{w}: style fields");
    }
}

/// Criteria 20, 53: every row of the table, for a drag of each edge handle
/// (grow, shrink, with and without Shift). Rotated rectangles and ellipses,
/// polygons and stars convert; aligned ones and a circle do not; the outline of
/// a converted object is the exact image of its old outline.
#[test]
fn ac20_ac53_every_kind_keeps_or_converts_on_an_edge_stretch() {
    let d = kinds_scene();
    let otp = otp_reference(&d);
    // (nx, ny, how far past the handle (+) or inside it (-) the pointer goes, shift)
    let cases: [(f64, f64, f64, bool); 8] = [
        (1.0, 0.0, 40.0, false),
        (1.0, 0.0, -45.0, false),
        (-1.0, 0.0, 30.0, false),
        (-1.0, 0.0, 30.0, true),
        (0.0, 1.0, 35.0, false),
        (0.0, 1.0, -50.0, false),
        (0.0, -1.0, 25.0, true),
        (1.0, 0.0, 25.0, true),
    ];
    for (nx, ny, delta, shift) in cases {
        let mut s = open(&d);
        select_everything(&mut s);
        let g = GBox::of(&s);
        let handle = g.mid(nx, ny);
        let target = if nx != 0.0 {
            handle.x + nx * delta
        } else {
            handle.y + ny * delta
        };
        let commits = change_count(&s);
        let m = edge_drag(&mut s, &g, nx, ny, target, shift, false);
        let what = format!("edge ({nx}, {ny}) delta {delta} shift {shift}");
        assert_eq!(change_count(&s), commits + 1, "{what}: one commit");
        check_kinds_after(&d, &s, m, &otp, &what);
        // Polygon, star, two turned rectangles, one turned ellipse (a circle never counts).
        assert_eq!(s.take_conversion_notice(), 5, "{what}: notice counts");
        // The new box is the tight box of the new outlines (criterion 29).
        s.pointer_hover(pt(5.0, 5.0), false, false);
    }
}

// ---------------------------------------------------------------------
// Criteria 18 and 19: corners
// ---------------------------------------------------------------------

/// Criterion 19: a corner drag of a selection with a star is a uniform scale whatever
/// Ctrl does, the dragged corner follows the diagonal only, Shift scales about the
/// centre; the star stays a star; nothing is reported. The spec's example (45, 10) to
/// (90, 12) scales both by 2.
#[test]
fn ac19_corner_drag_with_a_star_is_proportional_whatever_ctrl_does() {
    for (ctrl, shift) in [(false, false), (true, false), (false, true), (true, true)] {
        let (d, st, _rc) = spec_example();
        let mut s = open(&d);
        select_everything(&mut s);
        let g = GBox::of(&s);
        let before = d.object(st).unwrap();
        // Hover: the corner hint of a proportional-corner selection.
        hold(&mut s, g.corner(1.0, 1.0), false, false);
        assert_eq!(s.handle_hint(), "group");
        assert!(s.hover_conversion_count() == 0, "a corner never converts");
        let to = pt(g.min.x + 90.0, g.min.y + 12.0); // (90, 12) in the example's coordinates
        let from = g.corner(1.0, 1.0);
        let commits = change_count(&s);
        drag_mod(&mut s, from, to, shift, ctrl);
        assert_eq!(change_count(&s), commits + 1);
        let o = objects(&s);
        let ObjectSnapshot::Primitive(p) = &o[0] else {
            panic!("ctrl {ctrl} shift {shift}: a corner drag never converts the star")
        };
        let (Shape::Star { frame, .. }, ObjectSnapshot::Primitive(pb)) = (p.shape, &before) else {
            panic!()
        };
        let Shape::Star { frame: fb, .. } = pb.shape else {
            panic!()
        };
        // Dominant x: factor 2 from the opposite corner, 3 from the centre (Shift): (90 - 22.5) / 22.5.
        let f = if shift { 3.0 } else { 2.0 };
        assert!(
            near(frame.radius.as_mm(), fb.radius.as_mm() * f, 1e-6),
            "ctrl {ctrl} shift {shift}: star radius {}",
            frame.radius.as_mm()
        );
        let ObjectSnapshot::Primitive(pr) = &o[1] else {
            panic!()
        };
        let Shape::Rect { bounds, .. } = pr.shape else {
            panic!()
        };
        assert!(
            near(bounds.width.as_mm(), 10.0 * f, 1e-6)
                && near(bounds.height.as_mm(), 10.0 * f, 1e-6),
            "ctrl {ctrl} shift {shift}: rect {}x{}",
            bounds.width.as_mm(),
            bounds.height.as_mm()
        );
        let (anchor_x, anchor_y) = if shift {
            (g.centre().x, g.centre().y)
        } else {
            (g.min.x, g.min.y)
        };
        assert_pt(
            bounds.origin,
            pt(
                anchor_x + f * (85.0 - anchor_x),
                anchor_y + f * (50.0 - anchor_y),
            ),
            1e-6,
            "rect origin",
        );
        assert_eq!(s.take_conversion_notice(), 0, "no notice");
    }
}

/// Criterion 19: the dominant axis may be y: both axes follow it.
#[test]
fn ac19_corner_drag_dominant_y_scales_both_axes_by_the_y_ratio() {
    let (d, st, _) = spec_example();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let from = g.corner(1.0, 1.0);
    let to = pt(from.x + 3.0, g.min.y + 30.0); // x ratio 1.07, y ratio 3
    drag_mod(&mut s, from, to, false, true);
    let o = objects(&s);
    let ObjectSnapshot::Primitive(p) = &o[0] else {
        panic!("star must stay a star")
    };
    let Shape::Star { frame, .. } = p.shape else {
        panic!()
    };
    let ObjectSnapshot::Primitive(b) = d.object(st).unwrap() else {
        panic!()
    };
    let Shape::Star { frame: fb, .. } = b.shape else {
        panic!()
    };
    assert!(
        near(frame.radius.as_mm(), fb.radius.as_mm() * 3.0, 1e-6),
        "radius {}",
        frame.radius.as_mm()
    );
}

/// Criterion 18/19: a selection with no converting object stretches with a corner
/// drag (no modifier); a circle at 30 degrees becomes an ellipse (not a path), and
/// nothing is reported as converted.
#[test]
fn ac18_corner_drag_without_converting_objects_stretches_and_converts_nothing() {
    let d = Document::new(1);
    let _ = curved_path(&d, 1, 60.0, 60.0);
    let _ = d.create_rect(rect_bounds(80.0, 50.0, 20.0, 12.0));
    let c = ellipse(&d, 120.0, 56.0, 8.0, 8.0);
    rotate_about_centre(&d, c, 30.0_f64.to_radians());
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    hold(&mut s, g.corner(1.0, 1.0), false, false);
    assert_eq!(s.handle_hint(), "group");
    assert_eq!(
        s.corner_hint_lines(),
        vec![
            "Resize selection",
            "Shift: from the centre",
            "Ctrl: keep proportions",
            "Double-click or S: type a size"
        ],
        "criterion 38, corner row: the Ctrl line stays when nothing converts"
    );
    let to = pt(g.max.x + 45.0, g.max.y + 20.0);
    drag_mod(&mut s, g.corner(1.0, 1.0), to, false, false);
    let m = Map {
        ax: g.min.x,
        ay: g.min.y,
        sx: (to.x - g.min.x) / g.w(),
        sy: (to.y - g.min.y) / g.h(),
    };
    assert!((m.sx - m.sy).abs() > 0.05);
    let o = objects(&s);
    let ObjectSnapshot::Primitive(pr) = &o[1] else {
        panic!("rect stays a rect")
    };
    let Shape::Rect { bounds, .. } = pr.shape else {
        panic!()
    };
    assert!(
        near(bounds.width.as_mm(), 20.0 * m.sx, 1e-6)
            && near(bounds.height.as_mm(), 12.0 * m.sy, 1e-6)
    );
    let ObjectSnapshot::Primitive(pc) = &o[2] else {
        panic!("circle must not become a path")
    };
    let Shape::Ellipse { frame } = pc.shape else {
        panic!()
    };
    assert!(near(frame.rx.as_mm(), 8.0 * m.sx, 1e-6) && near(frame.ry.as_mm(), 8.0 * m.sy, 1e-6));
    assert!(near(pc.rotation.as_radians(), 0.0, 1e-9));
    assert_eq!(
        s.take_conversion_notice(),
        0,
        "a circle-to-ellipse write is not a conversion"
    );
    // Ctrl on the corner: one factor.
    let mut s = open(&d);
    select_everything(&mut s);
    drag_mod(&mut s, g.corner(1.0, 1.0), to, false, true);
    let ObjectSnapshot::Primitive(pr) = &objects(&s)[1] else {
        panic!()
    };
    let Shape::Rect { bounds, .. } = pr.shape else {
        panic!()
    };
    assert!(
        near(
            bounds.width.as_mm() / 20.0,
            bounds.height.as_mm() / 12.0,
            1e-9
        ),
        "Ctrl: one factor"
    );
}

// ---------------------------------------------------------------------
// Criterion 53.4 and 54: only a real stretch converts, the notice
// ---------------------------------------------------------------------

fn mixed_scene() -> Document {
    let d = Document::new(1);
    let a = star(&d, 70.0, 70.0, 10.0, -FRAC_PI_2);
    let b = polygon(&d, 100.0, 70.0, 9.0, 0.2, 5);
    let c = d.create_rect(rect_bounds(120.0, 62.0, 20.0, 12.0));
    rotate_about_centre(&d, c, 0.5);
    let e = ellipse(&d, 160.0, 70.0, 9.0, 5.0);
    rotate_about_centre(&d, e, 0.5);
    let f = curved_path(&d, 1, 190.0, 70.0);
    set_look(&d, &[a, b, c, e, f]);
    d
}

/// Criterion 53.4: Escape, a drag back to factor 1 and an (almost) zero movement
/// convert nothing and write nothing; there is no notice; the preview reports the
/// count while the factors differ and nothing before.
#[test]
fn ac53_ac54_no_conversion_and_no_notice_without_a_real_stretch() {
    let d = mixed_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let before = bytes_of(&s);
    let from = g.mid(1.0, 0.0);
    // Escape in the middle of the drag.
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(from.x + 30.0, from.y), false, false);
    assert_eq!(
        s.live_conversion_count(),
        4,
        "the preview names the conversions"
    );
    assert!(bytes_of(&s) == before, "nothing is written during the drag");
    s.escape();
    s.pointer_up(pt(from.x + 30.0, from.y), false, false);
    hold(&mut s, pt(5.0, 5.0), false, false);
    assert!(bytes_of(&s) == before, "Escape writes nothing");
    assert_eq!(s.take_conversion_notice(), 0, "no notice after Escape");
    assert_eq!(s.live_conversion_count(), 0, "no counts after Escape");
    // Out and back to the start (factor 1).
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(from.x + 30.0, from.y), false, false);
    assert_ne!(s.live_conversion_count(), 0);
    hold(&mut s, from, false, false);
    assert_eq!(
        s.live_conversion_count(),
        0,
        "back at factor 1 the preview converts nothing"
    );
    s.pointer_up(from, false, false);
    assert!(
        bytes_of(&s) == before,
        "a drag back to the start writes nothing"
    );
    assert_eq!(s.take_conversion_notice(), 0, "no notice at factor 1");
    // Out and back to within 1e-12 mm of the start: factors equal 1 within 1e-9.
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(from.x + 30.0, from.y), false, false);
    hold(&mut s, pt(from.x + 1e-10, from.y), false, false);
    s.pointer_up(pt(from.x + 1e-10, from.y), false, false);
    assert!(
        bytes_of(&s) == before,
        "a factor of 1 within the tolerance writes nothing (and converts nothing)"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    // A press and release without movement.
    drag_mod(&mut s, from, from, false, false);
    assert!(bytes_of(&s) == before);
    assert_eq!(s.take_conversion_notice(), 0);
}

/// Criterion 54: the counts per kind are the number converted in this commit; a
/// second stretch of the same (now path) selection converts nothing and reports
/// nothing; the converted objects stay paths after a later uniform scale.
#[test]
fn ac54_the_counts_are_per_commit_and_the_conversion_is_permanent() {
    let d = mixed_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
    assert_eq!(s.take_conversion_notice(), 4);
    let kinds: Vec<bool> = objects(&s).iter().map(is_path).collect();
    assert_eq!(kinds, vec![true; 5]);
    // Stretch again: nothing to convert.
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
    assert_eq!(s.take_conversion_notice(), 0, "paths do not convert again");
    // Uniform scale back by the corner: still paths.
    let g = GBox::of(&s);
    drag_mod(
        &mut s,
        g.corner(1.0, 1.0),
        pt(g.max.x - 20.0, g.max.y - 3.0),
        false,
        false,
    );
    let kinds: Vec<bool> = objects(&s).iter().map(is_path).collect();
    assert_eq!(kinds, vec![true; 5], "permanent (53.5)");
    assert_eq!(s.take_conversion_notice(), 0);
}

/// Criterion 54: a move, a rotate, a Ctrl-copy and a uniform scale of a selection with
/// convertible shapes convert nothing and show no notice (and the shapes stay).
#[test]
fn ac53_ac54_move_rotate_and_uniform_scale_never_convert() {
    let d = mixed_scene();
    let n_prim = 4;
    // Move by the centre handle... no centre handle under 48 px at this zoom? Use an outline press.
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    // Move: press on the polygon's outline (a vertex-free point: midpoint of its first edge).
    let o0 = objects(&s);
    let ObjectSnapshot::Primitive(p) = &o0[0] else {
        panic!()
    };
    let out = outline_of_rotated(&p.shape, p.rotation);
    let a = pt(
        (out[0].point.x + out[1].point.x) / 2.0,
        (out[0].point.y + out[1].point.y) / 2.0,
    );
    drag_mod(&mut s, a, pt(a.x + 12.0, a.y + 7.0), false, false);
    assert_eq!(
        objects(&s).iter().filter(|o| !is_path(o)).count(),
        n_prim,
        "a move converts nothing"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    // Rotate by a corner rotate handle.
    let g = {
        let _ = g;
        GBox::of(&s)
    };
    let dd = 32.0 / 2.0_f64.sqrt() / k_of(&s);
    let from = pt(g.max.x + dd, g.max.y + dd);
    drag_mod(&mut s, from, pt(from.x - 20.0, from.y + 15.0), false, false);
    assert_eq!(
        objects(&s).iter().filter(|o| !is_path(o)).count(),
        n_prim,
        "a rotate converts nothing"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    // Uniform scale by a corner.
    let g = GBox::of(&s);
    drag_mod(
        &mut s,
        g.corner(1.0, 1.0),
        pt(g.max.x + 30.0, g.max.y + 30.0 * g.h() / g.w()),
        false,
        false,
    );
    assert_eq!(
        objects(&s).iter().filter(|o| !is_path(o)).count(),
        n_prim,
        "a uniform scale converts nothing"
    );
    assert_eq!(s.take_conversion_notice(), 0);
}

// ---------------------------------------------------------------------
// Criterion 34: typed size, the equal-factor boundary
// ---------------------------------------------------------------------

fn open_size_entry(d: &Document) -> (Session, f64, f64) {
    let mut s = open(d);
    select_everything(&mut s);
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.fields.len(), 2);
    let w: f64 = v.fields[0].prefill.parse().unwrap();
    let h: f64 = v.fields[1].prefill.parse().unwrap();
    (s, w, h)
}

/// Criterion 34 and the Terms: a typed size with equal factors (within 1e-9 times the
/// larger) is a uniform scale and converts nothing; unequal factors convert. The
/// boundary is at |sx - sy| = 1e-9 * max.
#[test]
fn ac34_typed_size_equal_vs_unequal_factors() {
    let (d, st, _) = spec_example();
    // Equal factors 2 and 2: the star stays a star, radius times 2.
    let (mut s, w, h) = open_size_entry(&d);
    assert!(
        near(w, 45.0, 0.051) && near(h, 10.0, 0.051),
        "prefill {w} x {h}"
    );
    let (w, h) = (45.0, 10.0);
    let commits = change_count(&s);
    assert_eq!(
        s.commit_transform_entry(&format!("{}", 2.0 * w), &format!("{}", 2.0 * h), 0),
        EntryOutcome::Committed
    );
    assert_eq!(change_count(&s), commits + 1);
    assert!(
        !is_path(&objects(&s)[0]),
        "equal factors: the star stays a star"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    let ObjectSnapshot::Primitive(p) = &objects(&s)[0] else {
        panic!()
    };
    let Shape::Star { frame, .. } = p.shape else {
        panic!()
    };
    assert!(
        near(frame.radius.as_mm(), 10.0, 1e-4),
        "radius {}",
        frame.radius.as_mm()
    );
    // Equal within the tolerance (relative 1e-11): uniform.
    let (mut s, _, _) = open_size_entry(&d);
    let out = s.commit_transform_entry("90", &format!("{}", 20.0 * (1.0 + 1e-11)), 0);
    assert_eq!(out, EntryOutcome::Committed);
    assert!(
        !is_path(&objects(&s)[0]),
        "factors equal within 1e-9: no conversion"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    // Unequal by 1e-7: a stretch.
    let (mut s, _, _) = open_size_entry(&d);
    let out = s.commit_transform_entry("90", &format!("{}", 20.0 * (1.0 + 1e-7)), 0);
    assert_eq!(out, EntryOutcome::Committed);
    assert!(
        is_path(&objects(&s)[0]),
        "factors unequal by 1e-7: the star converts"
    );
    assert_eq!(s.take_conversion_notice(), 1);
    // Width alone: a stretch.
    let (mut s, _, _) = open_size_entry(&d);
    assert_eq!(
        s.commit_transform_entry("90", "10", 0),
        EntryOutcome::Committed
    );
    let o = objects(&s);
    assert!(is_path(&o[0]));
    let otp = otp_reference(&d);
    // About the box centre (S): centre x = 72.5.
    let m = Map {
        ax: 72.5,
        ay: 55.0,
        sx: 2.0,
        sy: 1.0,
    };
    assert_path_maps(&o[0], &otp[0], m, "typed W only, about the centre");
    assert_eq!(o[0].id(), st);
    // An unedited Enter and an equal size write nothing.
    let (mut s, _, _) = open_size_entry(&d);
    let before = bytes_of(&s);
    assert_eq!(
        s.commit_transform_entry("45", "10", 0),
        EntryOutcome::Unchanged
    );
    assert!(bytes_of(&s) == before);
    assert_eq!(s.take_conversion_notice(), 0);
}

/// Criterion 34 and 22: refusals keep the chip open, write nothing and convert
/// nothing: zero, negative, huge (beyond 1e7 mm) and non-numbers.
#[test]
fn ac34_ac22_typed_refusals_convert_nothing() {
    let d = mixed_scene();
    for (w, h, why) in [
        ("0", "30", "zero width"),
        ("120", "0", "zero height"),
        ("-50", "30", "negative width"),
        ("120", "-1", "negative height"),
        ("1e9", "30", "too large"),
        ("abc", "30", "not a number"),
        ("NaN", "30", "NaN"),
        ("inf", "30", "infinity"),
    ] {
        let mut s = open(&d);
        select_everything(&mut s);
        key(&mut s, "s", false);
        let before = bytes_of(&s);
        let out = s.commit_transform_entry(w, h, 0);
        assert!(
            matches!(out, EntryOutcome::Invalid { .. }),
            "{why}: {out:?}"
        );
        assert!(bytes_of(&s) == before, "{why}: nothing written");
        assert!(s.transform_entry().is_some(), "{why}: the chip stays open");
        assert_eq!(s.take_conversion_notice(), 0, "{why}: no notice");
    }
}

/// Criterion 34 (all or nothing): a size that puts one object (a long path) beyond
/// 1e7 mm refuses the whole selection; the star in it stays a star.
#[test]
fn ac22_ac53_6_one_invalid_object_refuses_the_whole_stretch() {
    let d = Document::new(1);
    let st = star(&d, 70.0, 70.0, 10.0, -FRAC_PI_2);
    let _ = d.create_path(&[anchor(1, 90.0, 70.0), anchor(2, 600_000.0, 70.0)], false);
    let mut s = open(&d);
    // Select both: click the star's outline, Shift-click the path.
    let o0 = d.object(st).unwrap();
    let ObjectSnapshot::Primitive(p) = &o0 else {
        panic!()
    };
    let out = outline_of_rotated(&p.shape, p.rotation);
    let a = pt(
        (out[0].point.x + out[1].point.x) / 2.0,
        (out[0].point.y + out[1].point.y) / 2.0,
    );
    click(&mut s, a);
    hold(&mut s, pt(200.0, 70.0), true, false);
    s.pointer_down(pt(200.0, 70.0), true);
    s.pointer_up(pt(200.0, 70.0), true, false);
    hold(&mut s, pt(5.0, 5.0), false, false);
    assert_eq!(s.selected_object_count(), 2);
    key(&mut s, "s", false);
    let v = s.transform_entry().unwrap();
    let w: f64 = v.fields[0].prefill.parse().unwrap();
    let before = bytes_of(&s);
    // x 20 times: the path's end goes to 12e6 mm.
    let out = s.commit_transform_entry(&format!("{}", w * 30.0), &v.fields[1].prefill, 0);
    assert!(matches!(out, EntryOutcome::Invalid { .. }), "{out:?}");
    assert!(bytes_of(&s) == before, "nothing scaled, nothing converted");
    assert!(!is_path(&objects(&s)[0]), "the star stays a star");
    assert_eq!(s.take_conversion_notice(), 0);
}

/// Criterion 22 (drag): a factor that would put a coordinate beyond 1e7 mm changes
/// nothing at all, converts nothing and reports nothing.
#[test]
fn ac22_a_huge_drag_factor_writes_and_converts_nothing() {
    let d = mixed_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let before = bytes_of(&s);
    let from = g.mid(-1.0, 0.0);
    drag_mod(&mut s, from, pt(-1.0e12, from.y), false, false);
    assert!(
        bytes_of(&s) == before,
        "a result beyond 1e7 mm is no change"
    );
    assert_eq!(s.take_conversion_notice(), 0);
    assert!(objects(&s).iter().filter(|o| !is_path(o)).count() == 4);
    // Non-finite pointer.
    drag_mod(&mut s, from, pt(f64::INFINITY, from.y), false, false);
    drag_mod(&mut s, from, pt(f64::NAN, from.y), false, false);
    assert!(bytes_of(&s) == before);
    assert_eq!(s.take_conversion_notice(), 0);
}

/// Criterion 22 / D5: a factor below 0 clamps to 0 (no flip, no negative size). A
/// converting object becomes a path collapsed onto the line through the fixed
/// side; an aligned rectangle gets width 0.
#[test]
fn ac22_a_negative_factor_clamps_to_zero_and_collapses_onto_the_fixed_side() {
    let (d, st, rc) = spec_example();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    // Drag the right edge to the left of the left side.
    edge_drag(&mut s, &g, 1.0, 0.0, g.min.x - 30.0, false, false);
    let o = objects(&s);
    assert!(
        is_path(&o[0]),
        "a stretch to factor 0 is a stretch like any other"
    );
    let ObjectSnapshot::Path(p) = &o[0] else {
        unreachable!()
    };
    for a in &p.anchors {
        assert!(
            near(a.point.x, 50.0, 1e-6),
            "x collapsed onto the fixed side, got {}",
            a.point.x
        );
        assert!(a.point.x.is_finite() && a.point.y.is_finite());
        assert!(near(a.handle_in.x, 0.0, 1e-9) && near(a.handle_out.x, 0.0, 1e-9));
    }
    let ys: Vec<f64> = p.anchors.iter().map(|a| a.point.y).collect();
    let before = d.object(st).unwrap();
    let ObjectSnapshot::Primitive(pb) = &before else {
        panic!()
    };
    let outl = outline_of_rotated(&pb.shape, pb.rotation);
    for (y, a) in ys.iter().zip(&outl) {
        assert!(near(*y, a.point.y, 1e-9), "y unchanged");
    }
    let ObjectSnapshot::Primitive(pr) = &o[1] else {
        panic!("rect stays a rect")
    };
    let Shape::Rect { bounds, .. } = pr.shape else {
        panic!()
    };
    assert!(
        bounds.width.as_mm() >= 0.0 && near(bounds.width.as_mm(), 0.0, 1e-9),
        "width {}",
        bounds.width.as_mm()
    );
    let _ = rc;
    assert_eq!(s.take_conversion_notice(), 1);
    // A top edge dragged below the bottom edge: the same for y.
    let (d, ..) = spec_example();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 0.0, -1.0, g.max.y + 25.0, false, false);
    let ObjectSnapshot::Path(p) = &objects(&s)[0] else {
        panic!("star converts")
    };
    for a in &p.anchors {
        assert!(
            near(a.point.y, 60.0, 1e-6),
            "y collapsed onto the bottom side, got {}",
            a.point.y
        );
    }
}

// ---------------------------------------------------------------------
// Criterion 24, 53.2: stroke width and styles
// ---------------------------------------------------------------------

/// Criterion 24 and 53.2: with "Scale stroke width" on, every object including a
/// converted one gets width * sqrt(sx*sy) floored at 0.01 mm, dash/opacity/fill are
/// never written; with the switch off no width is written. The radius switch never
/// decides whether a rectangle converts and does not touch a converted rectangle.
#[test]
fn ac24_ac53_2_stroke_switch_on_converted_and_kept_objects() {
    for (stroke_on, radius_on) in [(false, false), (true, false), (true, true), (false, true)] {
        let d = kinds_scene();
        // A thin object to see the floor.
        let thin = d.object_ids()[8];
        d.edit_style(&[thin], &StyleEdit::StrokeWidth(Length::from_mm(0.012)))
            .unwrap();
        let before: Vec<ObjectSnapshot> = d
            .object_ids()
            .into_iter()
            .filter_map(|i| d.object(i))
            .collect();
        let mut s = open(&d);
        s.set_scale_stroke_width(stroke_on);
        s.set_scale_corner_radius(radius_on);
        select_everything(&mut s);
        let g = GBox::of(&s);
        let target = g.max.x - 0.5 * g.w(); // sx = 0.5
        let m = edge_drag(&mut s, &g, 1.0, 0.0, target, false, false);
        assert!(near(m.sx, 0.5, 1e-9));
        let after = objects(&s);
        for (i, (b, a)) in before.iter().zip(&after).enumerate() {
            let w = format!("stroke {stroke_on} radius {radius_on}: {}", KIND_NAMES[i]);
            let want = if stroke_on {
                (stroke_width(b) * (m.sx * m.sy).sqrt()).max(0.01)
            } else {
                stroke_width(b)
            };
            assert!(
                near(stroke_width(a), want, 1e-9),
                "{w}: width {} want {want}",
                stroke_width(a)
            );
            assert_eq!(dash_debug(a), dash_debug(b), "{w}: dash is never written");
            let strip = |o: &ObjectSnapshot| {
                let s = style_debug(o);
                s.replace(&format!("{:?}", stroke_width(o)), "W")
            };
            let _ = strip;
            // Fill and opacity are never written.
            match (a, b) {
                (ObjectSnapshot::Primitive(pa), ObjectSnapshot::Primitive(pb)) => {
                    assert_eq!(
                        format!("{:?}", pa.style.fill),
                        format!("{:?}", pb.style.fill),
                        "{w}: fill"
                    );
                    assert_eq!(
                        format!("{:?}", pa.style.stroke.opacity),
                        format!("{:?}", pb.style.stroke.opacity)
                    );
                }
                (ObjectSnapshot::Path(pa), ObjectSnapshot::Primitive(pb)) => {
                    assert_eq!(
                        format!("{:?}", pa.style.fill),
                        format!("{:?}", pb.style.fill),
                        "{w}: fill"
                    );
                    assert_eq!(
                        format!("{:?}", pa.style.stroke.opacity),
                        format!("{:?}", pb.style.stroke.opacity)
                    );
                    assert_eq!(
                        format!("{:?}", pa.style.stroke.join),
                        format!("{:?}", pb.style.stroke.join)
                    );
                    assert_eq!(
                        format!("{:?}", pa.style.stroke.cap),
                        format!("{:?}", pb.style.stroke.cap)
                    );
                }
                (ObjectSnapshot::Path(pa), ObjectSnapshot::Path(pb)) => {
                    assert_eq!(
                        format!("{:?}", pa.style.fill),
                        format!("{:?}", pb.style.fill),
                        "{w}: fill"
                    );
                }
                _ => panic!("{w}: a path never turns into a primitive"),
            }
        }
        // The aligned rounded rectangle: radii follow the radius switch only.
        let (ObjectSnapshot::Primitive(pb), ObjectSnapshot::Primitive(pa)) =
            (&before[1], &after[1])
        else {
            panic!()
        };
        let (
            Shape::Rect {
                corner_radii: rb, ..
            },
            Shape::Rect {
                corner_radii: ra, ..
            },
        ) = (pb.shape, pa.shape)
        else {
            panic!()
        };
        if radius_on {
            // each radius times sqrt(sx*sy), then clamped to what the rectangle allows
            let want = (rb.tl.as_mm() * (0.5_f64).sqrt()).min(6.0);
            assert!(
                near(ra.tl.as_mm(), want, 1e-6),
                "radius {} want {want}",
                ra.tl.as_mm()
            );
        } else {
            assert!(
                near(ra.tl.as_mm(), rb.tl.as_mm(), 1e-12),
                "radius untouched"
            );
        }
        // The turned rounded rectangle converted whatever the radius switch says.
        assert!(
            is_path(&after[11]),
            "the radius switch never decides conversion"
        );
        assert_eq!(s.take_conversion_notice(), 5);
    }
}

// ---------------------------------------------------------------------
// Criteria 38 and 55: hover chips
// ---------------------------------------------------------------------

/// Criteria 38 and 55: the edge handle hover reports the kinds that would convert,
/// in the order polygons, stars, rotated rectangles, rotated ellipses; a circle
/// never counts; a corner reports none; a selection without converting object
/// reports none and its corner keeps the Ctrl line (hint "resize-corner").
#[test]
fn ac38_ac55_hover_counts_and_hints() {
    let d = kinds_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    for (nx, ny) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        hold(&mut s, g.mid(nx, ny), false, false);
        assert_eq!(s.handle_hint(), "group", "edge ({nx}, {ny})");
        assert_eq!(
            s.corner_hint_lines(),
            vec![
                "Resize selection",
                "Shift: from the centre",
                "Double-click or S: type a size"
            ],
            "criterion 38, edge row (the conversion line is the host's, from the counts)"
        );
        assert_eq!(s.hover_conversion_count(), 5, "edge ({nx}, {ny})");
    }
    for (sx, sy) in [(1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0)] {
        hold(&mut s, g.corner(sx, sy), false, false);
        assert_eq!(s.handle_hint(), "group");
        assert!(s.hover_conversion_count() == 0, "a corner never converts");
    }
    hold(&mut s, pt(5.0, 5.0), false, false);
    assert_eq!(s.hover_conversion_count(), 0, "off a handle: none");
    // A selection without a converting object (path, aligned rect, circle at 30 degrees).
    let d = Document::new(1);
    let _ = curved_path(&d, 1, 60.0, 60.0);
    let _ = d.create_rect(rect_bounds(80.0, 50.0, 20.0, 12.0));
    let c = ellipse(&d, 120.0, 56.0, 8.0, 8.0);
    rotate_about_centre(&d, c, 30.0_f64.to_radians());
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    hold(&mut s, g.mid(1.0, 0.0), false, false);
    assert_eq!(s.handle_hint(), "group");
    assert!(
        s.hover_conversion_count() == 0,
        "a circle never counts as rotated"
    );
    hold(&mut s, g.corner(1.0, 1.0), false, false);
    assert_eq!(s.handle_hint(), "group");
    assert_eq!(
        s.corner_hint_lines().len(),
        4,
        "the corner chip keeps the Ctrl line"
    );
}

/// Criterion 38, row "Corner resize, proportional-corner selection" (as aligned
/// 2026-10-10 with the UX notes): four lines, no Ctrl line, and the line that points
/// to the edge handle.
#[test]
fn ac38_the_corner_chip_of_a_proportional_corner_selection_has_the_four_spec_lines() {
    let (d, _, _) = spec_example();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    hold(&mut s, g.corner(1.0, 1.0), false, false);
    assert_eq!(
        s.corner_hint_lines(),
        vec![
            "Resize selection, proportional",
            "Stretch with an edge handle",
            "Shift: from the centre",
            "Double-click or S: type a size"
        ]
    );
}

/// Criteria 32 and 53.7: the preview of a drag writes nothing; the converting kinds
/// are counted live; the document is unchanged until the release.
#[test]
fn ac32_the_preview_counts_the_conversions_and_writes_nothing() {
    let d = kinds_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let before = bytes_of(&s);
    let from = g.mid(1.0, 0.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    for dx in [5.0, 25.0, 60.0] {
        hold(&mut s, pt(from.x + dx, from.y), false, false);
        assert_eq!(s.live_conversion_count(), 5, "dx {dx}");
        assert!(
            bytes_of(&s) == before,
            "dx {dx}: nothing converted before the release"
        );
        let r = s.live_readout().expect("a readout while dragging");
        assert!(r.text.contains("mm"), "readout {:?}", r.text);
    }
    s.escape();
    s.pointer_up(from, false, false);
}

// ---------------------------------------------------------------------
// Criterion 39: the Select bar after a conversion
// ---------------------------------------------------------------------

/// Criterion 39 and 53.3: a selection of only stars and polygons loses every kind
/// group on conversion; the bar shows the path state (no Points, Ratio, Radius, and
/// no Object to path).
#[test]
fn ac39_the_select_bar_follows_the_kinds_after_a_conversion() {
    let d = Document::new(1);
    let _ = star(&d, 70.0, 70.0, 10.0, -FRAC_PI_2);
    let _ = polygon(&d, 100.0, 70.0, 9.0, 0.3, 6);
    let _ = d.create_rect(rect_bounds(120.0, 62.0, 20.0, 12.0));
    let rid = d.object_ids()[2];
    rotate_about_centre(&d, rid, 0.4);
    let mut s = open(&d);
    select_everything(&mut s);
    let bar = s.select_bar_state();
    assert!(
        bar.points.is_some() && bar.ratio.is_some() && bar.radius.is_some() && bar.object_to_path
    );
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 0.0, 1.0, g.max.y + 12.0, false, false);
    assert_eq!(s.take_conversion_notice(), 3);
    let bar = s.select_bar_state();
    assert!(bar.points.is_none(), "Points gone");
    assert!(bar.ratio.is_none(), "Ratio gone");
    assert!(bar.radius.is_none(), "Radius gone");
    assert!(
        !bar.object_to_path,
        "Object to path is not offered for paths"
    );
    assert!(objects(&s).iter().all(is_path));
    assert_eq!(s.selected_object_count(), 3);
}

// ---------------------------------------------------------------------
// Criterion 53.3 and 0012: the shown angle
// ---------------------------------------------------------------------

/// Criterion 53.1 and 53.3: a polygon and a star with a shown angle (frame angle
/// plus a rotation register) convert to paths whose outline is exactly the old
/// outline stretched; the path's `rotation` is what "Object to path" gives and the
/// stretch leaves it unchanged. (The spec text says the shown angle is not
/// carried into the path; this test reports what the implementation does.)
#[test]
fn ac53_3_shown_angle_polygon_and_star_convert_to_the_stretched_outline() {
    let d = Document::new(1);
    let a = star(&d, 70.0, 70.0, 10.0, 0.4);
    rotate_about_centre(&d, a, 0.35);
    let b = polygon(&d, 110.0, 70.0, 9.0, -1.1, 7);
    rotate_about_centre(&d, b, -0.8);
    let before: Vec<_> = [a, b].iter().map(|i| d.object(*i).unwrap()).collect();
    let otp = otp_reference(&d);
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let m = edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 33.0, false, false);
    let o = objects(&s);
    for i in 0..2 {
        assert_straight_outline_exact(
            &before[i],
            &o[i],
            m,
            if i == 0 { "star" } else { "polygon" },
        );
        assert_path_maps(&o[i], &otp[i], m, "vs Object to path");
        assert!(
            near(rotation_of(&o[i]), rotation_of(&otp[i]), 1e-9),
            "the stretch leaves the rotation register of Object to path unchanged: {} vs {}",
            rotation_of(&o[i]),
            rotation_of(&otp[i])
        );
        println!(
            "path rotation after the stretch of object {i}: {} rad (the shown angle was {} rad)",
            rotation_of(&o[i]),
            match &before[i] {
                ObjectSnapshot::Primitive(p) => match p.shape {
                    Shape::Star { frame, .. } | Shape::Polygon { frame, .. } =>
                        frame.angle.as_radians() + p.rotation.as_radians(),
                    _ => 0.0,
                },
                ObjectSnapshot::Path(_) => 0.0,
            }
        );
    }
    assert_eq!(s.take_conversion_notice(), 2);
}

// ---------------------------------------------------------------------
// Criterion 56: a single polygon or star
// ---------------------------------------------------------------------

fn select_single(s: &mut Session, o: &ObjectSnapshot) {
    let ObjectSnapshot::Primitive(p) = o else {
        panic!()
    };
    let out = outline_of_rotated(&p.shape, p.rotation);
    let a = pt(
        (out[0].point.x + out[1].point.x) / 2.0,
        (out[0].point.y + out[1].point.y) / 2.0,
    );
    click(s, a);
    hold(s, pt(5.0, 5.0), false, false);
    assert_eq!(s.selected_object_count(), 1);
}

/// The oriented box of a polygon or star (`0012` criterion 1): the square of side 2R
/// centred on C whose sides run in the direction alpha (shown angle).
struct OBox {
    c: Point,
    r: f64,
    a: f64,
}

impl OBox {
    fn u(&self) -> (f64, f64) {
        (self.a.cos(), self.a.sin())
    }
    fn v(&self) -> (f64, f64) {
        (-self.a.sin(), self.a.cos())
    }
    fn mid(&self, nx: f64, ny: f64) -> Point {
        let (u, v) = (self.u(), self.v());
        pt(
            self.c.x + self.r * (nx * u.0 + ny * v.0),
            self.c.y + self.r * (nx * u.1 + ny * v.1),
        )
    }
    fn corner(&self, sx: f64, sy: f64) -> Point {
        self.mid(sx, sy)
    }
}

fn obox_of(o: &ObjectSnapshot) -> OBox {
    let ObjectSnapshot::Primitive(p) = o else {
        panic!()
    };
    match p.shape {
        Shape::Star { frame, .. } | Shape::Polygon { frame, .. } => OBox {
            c: frame.center,
            r: frame.radius.as_mm(),
            a: frame.angle.as_radians() + p.rotation.as_radians(),
        },
        _ => panic!("polygon or star expected"),
    }
}

/// Criterion 56: a single polygon or star shows all eight resize handles; corners
/// are proportional (Ctrl or not), edges stretch along the box axes, about the
/// opposite side (or the centre with Shift) and convert; the outline is the old
/// outline scaled along the box axis; one notice with N = 1.
#[test]
fn ac56_single_polygon_and_star_eight_handles_and_edge_stretch_converts() {
    for (name, make) in [("star", 0_u8), ("polygon", 1_u8)] {
        for (alpha, shift) in [
            (0.0_f64, false),
            (30.0_f64.to_radians(), false),
            (-0.9, true),
        ] {
            let d = Document::new(1);
            let id = if make == 0 {
                star(&d, 80.0, 80.0, 12.0, alpha)
            } else {
                polygon(&d, 80.0, 80.0, 12.0, alpha, 5)
            };
            set_look(&d, &[id]);
            let old = d.object(id).unwrap();
            let otp = otp_reference(&d);
            let mut s = open(&d);
            select_single(&mut s, &old);
            let b = obox_of(&old);
            let what = format!("{name} alpha {alpha} shift {shift}");
            for (nx, ny) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                hold(&mut s, b.mid(nx, ny), false, false);
                assert_eq!(
                    s.handle_hint(),
                    "resize-edge",
                    "{what}: edge ({nx}, {ny}) is a handle"
                );
                assert_eq!(
                    s.hover_conversion_count(),
                    1,
                    "{what}: the hover names the kind"
                );
            }
            for (sx, sy) in [(1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0)] {
                hold(&mut s, b.corner(sx, sy), false, false);
                assert!(
                    s.handle_hint().contains("resize-corner"),
                    "{what}: corner hint {}",
                    s.handle_hint()
                );
            }
            // Drag the E handle outward along the axis by 0.7 R.
            let (u, _v) = (b.u(), b.v());
            let from = b.mid(1.0, 0.0);
            let d_along = 0.7 * b.r;
            let to = pt(from.x + d_along * u.0, from.y + d_along * u.1);
            let commits = change_count(&s);
            drag_mod(&mut s, from, to, shift, false);
            assert_eq!(change_count(&s), commits + 1, "{what}: one commit");
            let o = objects(&s);
            assert!(
                is_path(&o[0]),
                "{what}: an edge stretch converts the {name}: {:?}",
                o[0]
            );
            assert_eq!(o[0].id(), id);
            // The map: stretch along u by f about A = C - R u (or C with Shift).
            let (a_pt, ext) = if shift {
                (b.c, b.r)
            } else {
                (pt(b.c.x - b.r * u.0, b.c.y - b.r * u.1), 2.0 * b.r)
            };
            let along = if shift {
                b.r + d_along
            } else {
                2.0 * b.r + d_along
            };
            let f = along / ext;
            let map_p = |q: Point| {
                let (dx, dy) = (q.x - a_pt.x, q.y - a_pt.y);
                let t = dx * u.0 + dy * u.1;
                pt(q.x + (f - 1.0) * t * u.0, q.y + (f - 1.0) * t * u.1)
            };
            let ObjectSnapshot::Primitive(po) = &old else {
                panic!()
            };
            let outl = outline_of_rotated(&po.shape, po.rotation);
            let ObjectSnapshot::Path(np) = &o[0] else {
                unreachable!()
            };
            assert_eq!(np.anchors.len(), outl.len(), "{what}: node count");
            for a in &outl {
                let want = map_p(a.point);
                assert!(
                    np.anchors
                        .iter()
                        .any(|q| near(q.point.x, want.x, 1e-6) && near(q.point.y, want.y, 1e-6)),
                    "{what}: the mapped vertex ({}, {}) is missing",
                    want.x,
                    want.y
                );
            }
            assert_eq!(style_debug(&o[0]), style_debug(&old), "{what}: style");
            assert!(
                near(rotation_of(&o[0]), rotation_of(&otp[0]), 1e-9),
                "{what}: rotation as Object to path"
            );
            assert_eq!(s.take_conversion_notice(), 1, "{what}: N = 1");
            assert_eq!(s.selected_object_count(), 1);
            let bar = s.select_bar_state();
            assert!(
                bar.points.is_none() && bar.ratio.is_none() && !bar.object_to_path,
                "{what}: path bar"
            );
        }
    }
}

/// Criterion 56: a corner drag of a single star is proportional, Ctrl or not, and
/// keeps the kind; so does an edge drag back to its start.
#[test]
fn ac56_single_star_corner_is_proportional_and_keeps_the_kind() {
    for ctrl in [false, true] {
        let d = Document::new(1);
        let id = star(&d, 80.0, 80.0, 12.0, -FRAC_PI_2);
        let old = d.object(id).unwrap();
        let mut s = open(&d);
        select_single(&mut s, &old);
        let b = obox_of(&old);
        let from = b.corner(1.0, 1.0);
        // A y-dominant off-diagonal drag: one factor, the kind stays.
        let (wx, wy) = (from.x - b.c.x, from.y - b.c.y);
        let to = pt(from.x + 0.8 * wx + 2.0, from.y + 0.8 * wy);
        drag_mod(&mut s, from, to, false, ctrl);
        let ObjectSnapshot::Primitive(p) = &objects(&s)[0] else {
            panic!("ctrl {ctrl}: a corner drag keeps the star")
        };
        let Shape::Star { frame, .. } = p.shape else {
            panic!()
        };
        assert!(
            frame.radius.as_mm() > 12.0,
            "grown, radius {}",
            frame.radius.as_mm()
        );
        assert_eq!(s.take_conversion_notice(), 0);
        // An edge drag out and back to the start writes nothing.
        let mut s = open(&d);
        select_single(&mut s, &old);
        let before = bytes_of(&s);
        let e = b.mid(1.0, 0.0);
        hold(&mut s, e, false, false);
        s.pointer_down(e, false);
        hold(&mut s, pt(e.x + 10.0, e.y), false, false);
        hold(&mut s, e, false, false);
        s.pointer_up(e, false, false);
        assert!(bytes_of(&s) == before);
        assert_eq!(s.take_conversion_notice(), 0);
    }
}

/// Criterion 56: the typed size of a single polygon or star has "W" and "H" (the
/// box's size in its own axes); equal factors scale uniformly and keep the kind; a
/// size of another aspect ratio converts.
#[test]
fn ac56_single_star_typed_size_has_w_and_h_and_converts_on_a_stretch() {
    let d = Document::new(1);
    let id = star(&d, 80.0, 80.0, 12.0, -FRAC_PI_2);
    let old = d.object(id).unwrap();
    let mut s = open(&d);
    select_single(&mut s, &old);
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.fields.len(), 2, "W and H");
    assert_eq!((v.fields[0].label, v.fields[1].label), ("W", "H"));
    assert!(near(
        v.fields[0].prefill.parse::<f64>().unwrap(),
        24.0,
        0.051
    ));
    assert!(near(
        v.fields[1].prefill.parse::<f64>().unwrap(),
        24.0,
        0.051
    ));
    // Equal factors: uniform.
    assert_eq!(
        s.commit_transform_entry("36", "36", 0),
        EntryOutcome::Committed
    );
    let ObjectSnapshot::Primitive(p) = &objects(&s)[0] else {
        panic!("equal factors keep the star")
    };
    let Shape::Star { frame, .. } = p.shape else {
        panic!()
    };
    assert!(near(frame.radius.as_mm(), 18.0, 1e-6));
    assert_eq!(s.take_conversion_notice(), 0);
    // Unequal: a stretch, a path, N = 1.
    let mut s = open(&d);
    select_single(&mut s, &old);
    key(&mut s, "s", false);
    assert_eq!(
        s.commit_transform_entry("36", "24", 0),
        EntryOutcome::Committed
    );
    assert!(is_path(&objects(&s)[0]));
    assert_eq!(s.take_conversion_notice(), 1);
}

/// Criterion 56: a single turned rectangle or ellipse needs nothing: it already
/// stretches along its own axes and keeps its kind, with no notice.
#[test]
fn ac56_a_single_turned_rectangle_or_ellipse_keeps_its_kind() {
    let d = Document::new(1);
    let rc = d.create_rect(rect_bounds(60.0, 60.0, 30.0, 14.0));
    rotate_about_centre(&d, rc, 30.0_f64.to_radians());
    let old = d.object(rc).unwrap();
    let mut s = open(&d);
    select_single(&mut s, &old);
    let ObjectSnapshot::Primitive(p) = &old else {
        panic!()
    };
    let Shape::Rect { bounds, .. } = p.shape else {
        panic!()
    };
    let c = rect_centre(bounds);
    let th = p.rotation.as_radians();
    let u = (th.cos(), th.sin());
    let handle = pt(c.x + 15.0 * u.0, c.y + 15.0 * u.1);
    hold(&mut s, handle, false, false);
    assert_eq!(s.handle_hint(), "resize-edge");
    assert!(
        s.hover_conversion_count() == 0,
        "no conversion line for a rectangle"
    );
    drag_mod(
        &mut s,
        handle,
        pt(handle.x + 10.0 * u.0, handle.y + 10.0 * u.1),
        false,
        false,
    );
    let ObjectSnapshot::Primitive(p2) = &objects(&s)[0] else {
        panic!("still a rectangle")
    };
    let Shape::Rect { bounds: b2, .. } = p2.shape else {
        panic!()
    };
    assert!(
        near(b2.width.as_mm(), 40.0, 1e-6) && near(b2.height.as_mm(), 14.0, 1e-9),
        "{}x{}",
        b2.width.as_mm(),
        b2.height.as_mm()
    );
    assert!(near(p2.rotation.as_radians(), th, 1e-9));
    assert_eq!(s.take_conversion_notice(), 0);
}

// ---------------------------------------------------------------------
// Compound paths and boolean results
// ---------------------------------------------------------------------

fn all_points(o: &ObjectSnapshot) -> Vec<(Point, Vec2, Vec2)> {
    match o {
        ObjectSnapshot::Path(p) => p
            .subpaths()
            .flat_map(|sp| {
                sp.anchors
                    .iter()
                    .map(|a| (a.point, a.handle_in, a.handle_out))
                    .collect::<Vec<_>>()
            })
            .collect(),
        ObjectSnapshot::Primitive(_) => panic!("path expected"),
    }
}

/// Criteria 20 and 53: a compound path (a boolean difference with a hole) in a
/// selection with a star and a turned rectangle: every outline of the compound is
/// mapped; the star and the rectangle convert; the compound is not counted.
#[test]
fn compound_path_and_boolean_result_in_a_converting_stretch() {
    let d = Document::new(1);
    let _ = d.create_rect(rect_bounds(50.0, 50.0, 40.0, 40.0));
    let _ = d.create_rect(rect_bounds(60.0, 60.0, 10.0, 10.0));
    let mut s = open(&d);
    select_everything(&mut s);
    let out = s.apply_boolean(curvyo_ui_core::BooleanOp::Difference);
    assert!(
        matches!(
            out,
            curvyo_editor_wasm::BooleanOutcome::Applied { compound: true, .. }
        ),
        "{out:?}"
    );
    let d = doc_of(&s);
    let st = star(&d, 120.0, 70.0, 10.0, -FRAC_PI_2);
    let rc = d.create_rect(rect_bounds(140.0, 62.0, 20.0, 12.0));
    rotate_about_centre(&d, rc, 0.6);
    let _ = st;
    let before: Vec<ObjectSnapshot> = d
        .object_ids()
        .into_iter()
        .filter_map(|i| d.object(i))
        .collect();
    let otp = otp_reference(&d);
    // A second session needs its own peer id: sessions of one project never share
    // one (anchor ids are `(peer, counter)`), and the Boolean result above holds the
    // ids the first session minted.
    let mut s = Session::open(3, &pack(&d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1600.0, 1000.0);
    select_everything(&mut s);
    assert_eq!(s.selected_object_count(), 3);
    let g = GBox::of(&s);
    let m = edge_drag(&mut s, &g, -1.0, 0.0, g.min.x - 25.0, false, false);
    let after = objects(&s);
    let (b, a) = (all_points(&before[0]), all_points(&after[0]));
    assert_eq!(b.len(), a.len());
    assert!(
        matches!(&after[0], ObjectSnapshot::Path(p) if p.extra_subpaths.len() == 1),
        "still a compound with a hole"
    );
    for (i, (x, y)) in b.iter().zip(&a).enumerate() {
        assert_pt(y.0, m.p(x.0), 1e-7, &format!("compound anchor {i}"));
        assert_vec(y.1, m.v(x.1), 1e-7, "in");
        assert_vec(y.2, m.v(x.2), 1e-7, "out");
    }
    assert!(is_path(&after[1]) && is_path(&after[2]));
    assert_path_maps(&after[1], &otp[1], m, "star");
    assert_path_maps(&after[2], &otp[2], m, "rotated rect");
    assert_eq!(s.take_conversion_notice(), 2, "the compound is not counted");
}

// ---------------------------------------------------------------------
// Criterion 50: save and reopen
// ---------------------------------------------------------------------

/// Criterion 50: after a converting stretch the saved file reopens to the same
/// kinds, ids, order, anchors, rotation and style; converted objects are paths with
/// the same id; the file holds no group box and the document format version is the
/// one of the unchanged format.
#[test]
fn ac50_round_trip_after_a_converting_stretch() {
    let d = kinds_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 40.0, false, false);
    let before = objects(&s);
    let bytes = s.pack("0.1.0").unwrap();
    let reopened = unpack(7, &bytes).expect("reopens");
    let after: Vec<ObjectSnapshot> = reopened
        .object_ids()
        .into_iter()
        .filter_map(|i| reopened.object(i))
        .collect();
    assert_eq!(before.len(), after.len());
    for (i, (b, a)) in before.iter().zip(&after).enumerate() {
        assert_eq!(b.id(), a.id(), "{}: id", KIND_NAMES[i]);
        assert_eq!(is_path(b), is_path(a), "{}: kind", KIND_NAMES[i]);
        assert_eq!(
            format!("{b:?}"),
            format!("{a:?}"),
            "{}: exact",
            KIND_NAMES[i]
        );
    }
    let converted = [7, 8, 9, 10, 11];
    for i in converted {
        assert!(
            is_path(&after[i]),
            "{} stays a path after reopening",
            KIND_NAMES[i]
        );
    }
    // A second round trip is stable.
    let again = unpack(8, &pack(&reopened, "0.1.0").unwrap()).unwrap();
    let after2: Vec<ObjectSnapshot> = again
        .object_ids()
        .into_iter()
        .filter_map(|i| again.object(i))
        .collect();
    assert_eq!(format!("{after:?}"), format!("{after2:?}"));
    assert_eq!(reopened.object_ids(), d.object_ids(), "ids and tree order");
}

// ---------------------------------------------------------------------
// Modifiers on edge handles
// ---------------------------------------------------------------------

/// Criterion 19: Ctrl has no effect on an edge handle; the result is the same with
/// and without Ctrl, for a selection with convertible shapes too.
#[test]
fn ac19_ctrl_has_no_effect_on_an_edge_handle() {
    let d = mixed_scene();
    let run = |ctrl: bool| {
        let mut s = open(&d);
        select_everything(&mut s);
        let g = GBox::of(&s);
        edge_drag(&mut s, &g, 0.0, 1.0, g.max.y + 17.0, false, ctrl);
        s.pack("0.1.0").unwrap().len();
        objects(&s)
            .iter()
            .map(|o| format!("{o:?}"))
            .collect::<Vec<_>>()
    };
    // Ids differ between sessions? No: same document, same ids.
    assert_eq!(run(false), run(true));
}

/// Criterion 19: Shift changes between "from the opposite side" and "from the
/// centre" for the stretch and its conversion.
#[test]
fn ac19_shift_stretches_about_the_centre() {
    let d = mixed_scene();
    let otp = otp_reference(&d);
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let m = edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, true, false);
    assert!(near(m.ax, g.centre().x, 1e-9));
    let o = objects(&s);
    for i in 0..4 {
        assert!(is_path(&o[i]));
        assert_path_maps(&o[i], &otp[i], m, "about the centre");
    }
    // The box grew on both sides.
    let g2 = GBox::of(&s);
    assert!(
        near(g2.min.x, g.min.x - 20.0, 1e-6) && near(g2.max.x, g.max.x + 20.0, 1e-6),
        "{g2:?}"
    );
}

// ---------------------------------------------------------------------
// Anchor ids of converted paths
// ---------------------------------------------------------------------

/// Criterion 53.1/50: the nodes of the converted paths get anchor ids that are unique
/// in the whole document (a path's anchor ids are document-wide unique), also after a
/// save and reopen, and across several conversions in one commit.
#[test]
fn ac53_ac50_converted_nodes_have_unique_anchor_ids() {
    let d = kinds_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
    let check = |os: &[ObjectSnapshot], what: &str| {
        let mut seen = std::collections::HashSet::new();
        let mut n = 0;
        for o in os {
            if let ObjectSnapshot::Path(p) = o {
                for sp in p.subpaths() {
                    for a in sp.anchors {
                        n += 1;
                        assert!(
                            seen.insert(format!("{:?}", a.id)),
                            "{what}: duplicate anchor id {:?}",
                            a.id
                        );
                    }
                }
            }
        }
        assert!(n > 20, "{what}: {n} nodes");
    };
    check(&objects(&s), "after the stretch");
    let reopened = unpack(5, &s.pack("0.1.0").unwrap()).unwrap();
    let os: Vec<ObjectSnapshot> = reopened
        .object_ids()
        .into_iter()
        .filter_map(|i| reopened.object(i))
        .collect();
    check(&os, "after reopening");
    // Further edits of a converted path keep working: stretch again, then a second
    // conversion batch from another session of the same file.
    let mut s2 = open(&reopened);
    select_everything(&mut s2);
    let g = GBox::of(&s2);
    edge_drag(&mut s2, &g, 0.0, 1.0, g.max.y + 11.0, false, false);
    check(&objects(&s2), "after a second stretch");
}

// ---------------------------------------------------------------------
// Boundaries, readout, interruptions
// ---------------------------------------------------------------------

fn pair_with_turned_rect(delta: f64) -> (Document, NodeId) {
    let d = Document::new(1);
    let _ = curved_path(&d, 1, 60.0, 60.0);
    let r = d.create_rect(rect_bounds(80.0, 50.0, 20.0, 12.0));
    rotate_about_centre(&d, r, FRAC_PI_2 + delta);
    (d, r)
}

/// The Terms: an aligned rectangle has a rotation of a multiple of 90 degrees within
/// 1e-9 rad. A rectangle 5e-10 rad off the quarter turn keeps its kind under a
/// stretch; one 3e-9 rad off converts. A rectangle at -90 degrees (270) is aligned.
#[test]
fn the_quarter_turn_tolerance_of_a_rectangle_is_1e_9_rad() {
    for (delta, converts) in [
        (0.0, false),
        (5e-10, false),
        (-5e-10, false),
        (3e-9, true),
        (-3e-9, true),
        (1e-4, true),
    ] {
        let (d, r) = pair_with_turned_rect(delta);
        let mut s = open(&d);
        select_everything(&mut s);
        let g = GBox::of(&s);
        edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
        let now = objects(&s);
        let idx = d.object_ids().iter().position(|i| *i == r).unwrap();
        assert_eq!(is_path(&now[idx]), converts, "delta {delta}");
        assert_eq!(
            s.take_conversion_notice(),
            u32::from(converts),
            "delta {delta}"
        );
    }
    // 270 degrees stored as -90.
    let d = Document::new(1);
    let _ = curved_path(&d, 1, 60.0, 60.0);
    let r = d.create_rect(rect_bounds(80.0, 50.0, 20.0, 12.0));
    rotate_about_centre(&d, r, 3.0 * FRAC_PI_2);
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
    assert!(!is_path(&objects(&s)[1]), "-90 degrees is a quarter turn");
}

/// The Terms: a rotated ellipse whose radii differ by less than the geometric
/// tolerance is a circle (becomes an ellipse); a clearly unequal one converts.
#[test]
fn a_nearly_round_rotated_ellipse_is_a_circle_a_clearly_oval_one_converts() {
    for (ry, converts) in [
        (8.0, false),
        (8.0 + 1e-9, false),
        (8.0 + 1e-3, true),
        (9.0, true),
    ] {
        let d = Document::new(1);
        let _ = curved_path(&d, 1, 60.0, 60.0);
        let e = ellipse(&d, 110.0, 56.0, 8.0, ry);
        rotate_about_centre(&d, e, 30.0_f64.to_radians());
        let mut s = open(&d);
        select_everything(&mut s);
        let g = GBox::of(&s);
        edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
        assert_eq!(is_path(&objects(&s)[1]), converts, "ry {ry}");
        assert_eq!(s.take_conversion_notice(), u32::from(converts), "ry {ry}");
    }
}

/// Criterion 23: the live readout of an edge stretch shows the group box size at the
/// pointer, "90.0 mm × 10.0 mm" for the spec example dragged to twice the width.
#[test]
fn ac23_the_readout_of_an_edge_stretch_is_the_box_size_at_the_pointer() {
    let (d, _, _) = spec_example();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let from = g.mid(1.0, 0.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(140.0, from.y), false, false);
    let r = s.live_readout().expect("readout");
    assert!(
        r.text.starts_with("90.0 mm \u{d7} 10.0 mm"),
        "readout {:?}",
        r.text
    );
    s.escape();
    s.pointer_up(pt(140.0, from.y), false, false);
}

/// Criterion 22 at factor 0 for a circle in the selection: it becomes an ellipse
/// with a zero radius in the stretched axis (not a path, not invalid, no notice); the
/// document stays finite and reopens.
#[test]
fn ac22_a_circle_at_factor_zero_stays_an_ellipse() {
    let d = Document::new(1);
    let _ = curved_path(&d, 1, 60.0, 60.0);
    let c = ellipse(&d, 110.0, 56.0, 8.0, 8.0);
    rotate_about_centre(&d, c, 30.0_f64.to_radians());
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.min.x - 40.0, false, false);
    let ObjectSnapshot::Primitive(p) = &objects(&s)[1] else {
        panic!("an ellipse")
    };
    let Shape::Ellipse { frame } = p.shape else {
        panic!()
    };
    assert!(
        frame.rx.as_mm() >= 0.0 && frame.rx.as_mm() < 1e-9 && near(frame.ry.as_mm(), 8.0, 1e-9),
        "rx {} ry {}",
        frame.rx.as_mm(),
        frame.ry.as_mm()
    );
    assert_eq!(s.take_conversion_notice(), 0);
    assert!(unpack(4, &s.pack("0.1.0").unwrap()).is_ok());
}

/// Interruption: Delete pressed in the middle of a converting stretch, then the
/// release. Whatever the outcome, no object that was deleted comes back as a path, the
/// document stays valid, and no notice is shown for a conversion that did not happen.
#[test]
fn delete_during_a_converting_stretch_resurrects_nothing() {
    let d = mixed_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    let from = g.mid(1.0, 0.0);
    hold(&mut s, from, false, false);
    s.pointer_down(from, false);
    hold(&mut s, pt(from.x + 30.0, from.y), false, false);
    let out = key(&mut s, "Delete", false);
    hold(&mut s, pt(from.x + 30.0, from.y), false, false);
    s.pointer_up(pt(from.x + 30.0, from.y), false, false);
    let n = objects(&s).len();
    println!("Delete during a converting stretch: {out:?}, {n} objects left");
    assert!(n == 0 || n == 5, "either deleted or untouched, got {n}");
    if out != KeyOutcome::Ignored {
        assert_eq!(
            n, 0,
            "deleted objects stay deleted (no converted path appears)"
        );
        assert_eq!(s.take_conversion_notice(), 0);
    }
    assert!(unpack(4, &s.pack("0.1.0").unwrap()).is_ok());
}

/// Criterion 54: the notice is per commit and taken once (the host takes it after every
/// event, `useEditorSession.ts`): it is gone after being taken, and a following move
/// reports nothing. (A notice that nobody took stays until the next commit replaces
/// it with that commit's counts; reported as an observation only.)
#[test]
fn ac54_the_notice_is_taken_once_and_a_following_move_reports_nothing() {
    let d = mixed_scene();
    let mut s = open(&d);
    select_everything(&mut s);
    let g = GBox::of(&s);
    edge_drag(&mut s, &g, 1.0, 0.0, g.max.x + 20.0, false, false);
    assert_eq!(s.take_conversion_notice(), 4);
    assert_eq!(s.take_conversion_notice(), 0, "taken once");
    let ObjectSnapshot::Path(p) = &objects(&s)[0] else {
        panic!()
    };
    let a = pt(
        (p.anchors[0].point.x + p.anchors[1].point.x) / 2.0,
        (p.anchors[0].point.y + p.anchors[1].point.y) / 2.0,
    );
    drag_mod(&mut s, a, pt(a.x + 9.0, a.y + 4.0), false, false);
    assert_eq!(
        s.take_conversion_notice(),
        0,
        "a move reports no conversion"
    );
}

/// Criterion 56: the typed size of a single turned polygon is the size of its box in
/// the box's own axes; typing a new W alone is a stretch along the box axis, about
/// the centre (key S), and converts with N = 1.
#[test]
fn ac56_single_turned_polygon_typed_width_stretches_along_its_box_axis() {
    let alpha = 30.0_f64.to_radians();
    let d = Document::new(1);
    let id = polygon(&d, 80.0, 80.0, 12.0, alpha, 5);
    let old = d.object(id).unwrap();
    let mut s = open(&d);
    select_single(&mut s, &old);
    assert_eq!(key(&mut s, "s", false), KeyOutcome::EntryOpened);
    let v = s.transform_entry().unwrap();
    assert_eq!(v.fields.len(), 2);
    let (w, h): (f64, f64) = (
        v.fields[0].prefill.parse().unwrap(),
        v.fields[1].prefill.parse().unwrap(),
    );
    assert!(
        near(w, 24.0, 0.051) && near(h, 24.0, 0.051),
        "box size in its own axes: {w} x {h}"
    );
    assert_eq!(
        s.commit_transform_entry("36", &v.fields[1].prefill, 0),
        EntryOutcome::Committed
    );
    let o = &objects(&s)[0];
    assert!(is_path(o));
    let (u, c) = ((alpha.cos(), alpha.sin()), pt(80.0, 80.0));
    let f = 1.5;
    let ObjectSnapshot::Primitive(po) = &old else {
        panic!()
    };
    let outl = outline_of_rotated(&po.shape, po.rotation);
    let ObjectSnapshot::Path(np) = o else {
        unreachable!()
    };
    assert_eq!(np.anchors.len(), outl.len());
    for a in &outl {
        let t = (a.point.x - c.x) * u.0 + (a.point.y - c.y) * u.1;
        let want = pt(
            a.point.x + (f - 1.0) * t * u.0,
            a.point.y + (f - 1.0) * t * u.1,
        );
        assert!(
            np.anchors
                .iter()
                .any(|q| near(q.point.x, want.x, 1e-6) && near(q.point.y, want.y, 1e-6)),
            "the mapped vertex ({}, {}) is missing",
            want.x,
            want.y
        );
    }
    assert_eq!(s.take_conversion_notice(), 1);
}
